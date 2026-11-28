//! Backup export validation, compression selection, manifest/SHA-256 generation, and scheduled backups.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use chrono::{TimeZone, Utc};

use crate::backup::archive_scan::{scan_backup_archives_with_stats, ArchiveScanEntry};
use crate::backup::scripts::build_export_steps_with_options;
use crate::util::names::sanitize_name;
use crate::util::paths::atomic_write;
use crate::util::time::{format_utc_iso, now_compact_stamp};

pub const MANIFEST_VERSION: u32 = 1;
pub const DEFAULT_SCHEDULED_INTERVAL_SECS: i64 = 7 * 86_400;
pub const DEFAULT_SCHEDULED_KEEP: usize = 3;

/// Structured errors for backup export, manifest, and archive verification.
#[derive(Debug, Error)]
pub enum BackupError {
    #[error("Backup archive not found: {0}")]
    NotFound(String),
    #[error("Backup manifest is corrupt at '{path}': {reason}")]
    ManifestCorrupt { path: String, reason: String },
    #[error(
        "Backup archive '{file}' size mismatch: expected {expected} bytes, got {actual} bytes"
    )]
    SizeMismatch {
        file: String,
        expected: u64,
        actual: u64,
    },
    #[error("Backup archive '{file}' checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch {
        file: String,
        expected: String,
        actual: String,
    },
    #[error("Backup archive '{path}' is corrupt or truncated: {reason}")]
    ArchiveCorrupt { path: String, reason: String },
    #[error("I/O error on '{path}': {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// Supported compression codecs for backup archives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompressionCodec {
    Gzip,
    Zstd,
}

impl CompressionCodec {
    /// Detects whether `zstd` is available in `PATH`; falls back to `Gzip`.
    pub fn detect() -> Self {
        if has_executable("zstd") {
            Self::Zstd
        } else {
            Self::Gzip
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Gzip => "tar.gz",
            Self::Zstd => "tar.zst",
        }
    }

    pub fn incus_flag(self) -> &'static str {
        match self {
            Self::Gzip => "gzip",
            Self::Zstd => "zstd",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gzip => "gzip",
            Self::Zstd => "zstd",
        }
    }

    pub fn from_path(path: &str) -> Self {
        if path.ends_with(".tar.zst") || path.ends_with(".zst") {
            Self::Zstd
        } else {
            Self::Gzip
        }
    }
}

fn has_executable(name: &str) -> bool {
    let Ok(path_var) = std::env::var("PATH") else {
        return false;
    };
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            if let Ok(out) = Command::new(&candidate).arg("--version").output() {
                if out.status.success() {
                    return true;
                }
            }
        }
    }
    false
}

/// Sidecar manifest written alongside a backup archive for integrity verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub version: u32,
    pub env_name: String,
    pub created_at: String,
    pub compression: String,
    pub archive_file: String,
    pub archive_sha256: String,
    pub archive_size_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_archive_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_archive_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_archive_size_bytes: Option<u64>,
}

/// Returns the manifest sidecar path for `backup_file` (`<backup_file>.manifest.json`).
pub fn manifest_path_for(backup_file: &str) -> PathBuf {
    PathBuf::from(format!("{backup_file}.manifest.json"))
}

/// Builds and writes `<backup_file>.manifest.json` with SHA-256 digests and sizes.
pub fn write_backup_manifest(
    env_name: &str,
    backup_file: &str,
    workspace_archive_file: Option<&str>,
    compression: CompressionCodec,
    created_at_epoch: i64,
) -> Result<PathBuf, BackupError> {
    let backup_path = Path::new(backup_file);
    let meta = fs::metadata(backup_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            BackupError::NotFound(backup_file.to_string())
        } else {
            BackupError::Io {
                path: backup_file.to_string(),
                source: e,
            }
        }
    })?;
    let archive_sha256 = sha256_file(backup_path)?;
    let archive_name = backup_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| backup_file.to_string());

    let (ws_file, ws_sha, ws_size) = match workspace_archive_file.filter(|s| !s.trim().is_empty()) {
        Some(ws_str) => {
            let ws_path = Path::new(ws_str);
            let ws_meta = fs::metadata(ws_path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    BackupError::NotFound(ws_str.to_string())
                } else {
                    BackupError::Io {
                        path: ws_str.to_string(),
                        source: e,
                    }
                }
            })?;
            let ws_digest = sha256_file(ws_path)?;
            let ws_name = ws_path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| ws_str.to_string());
            (Some(ws_name), Some(ws_digest), Some(ws_meta.len()))
        }
        None => (None, None, None),
    };

    let created_at = Utc
        .timestamp_opt(created_at_epoch, 0)
        .single()
        .map(format_utc_iso)
        .unwrap_or_default();

    let manifest = BackupManifest {
        version: MANIFEST_VERSION,
        env_name: env_name.to_string(),
        created_at,
        compression: compression.as_str().to_string(),
        archive_file: archive_name,
        archive_sha256,
        archive_size_bytes: meta.len(),
        workspace_archive_file: ws_file,
        workspace_archive_sha256: ws_sha,
        workspace_archive_size_bytes: ws_size,
    };

    let manifest_path = manifest_path_for(backup_file);
    let json =
        serde_json::to_string_pretty(&manifest).map_err(|e| BackupError::ManifestCorrupt {
            path: manifest_path.display().to_string(),
            reason: e.to_string(),
        })?;
    atomic_write(&manifest_path, json.as_bytes()).map_err(|e| BackupError::Io {
        path: manifest_path.display().to_string(),
        source: std::io::Error::other(e.to_string()),
    })?;
    Ok(manifest_path)
}

/// Validates preconditions for exporting an environment. Returns the sanitized name on success.
pub fn validate_export_request(busy: bool, name: &str, env_exists: bool) -> Result<String, String> {
    if busy {
        return Err("Another operation is already running.".to_string());
    }
    let clean = sanitize_name(name);
    if clean.is_empty() || !env_exists {
        return Err(format!("Environment '{name}' was not found."));
    }
    Ok(clean)
}

/// Policy for optional periodic backup exports tied to auto-snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledBackupPolicy {
    pub enabled: bool,
    pub interval_secs: i64,
    pub include_workspace: bool,
    pub keep_count: usize,
}

impl Default for ScheduledBackupPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_secs: DEFAULT_SCHEDULED_INTERVAL_SECS,
            include_workspace: true,
            keep_count: DEFAULT_SCHEDULED_KEEP,
        }
    }
}

/// Planned scheduled backup export and optional pruning of older backups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledBackupPlan {
    pub env_name: String,
    pub backup_file: String,
    pub workspace_archive_file: String,
    pub compression: CompressionCodec,
    pub script: String,
    pub prune_archives: Vec<(String, String)>,
}

/// Returns `true` when `env_name` has no backup newer than `interval_secs` before `now_epoch`.
pub fn should_create_scheduled_backup(
    existing_archives: &[ArchiveScanEntry],
    env_name: &str,
    now_epoch: i64,
    interval_secs: i64,
) -> bool {
    let clamped_interval = interval_secs.clamp(3600, 365 * 86_400);
    let latest = existing_archives
        .iter()
        .filter(|a| a.env_name == env_name)
        .map(|a| a.mtime_secs)
        .max();
    match latest {
        None => true,
        Some(ts) => now_epoch.saturating_sub(ts) >= clamped_interval,
    }
}

/// Returns oldest `(filePath, workspaceArchivePath)` pairs for `env_name` exceeding `keep_count`.
pub fn scheduled_backups_to_prune(
    existing_archives: &[ArchiveScanEntry],
    env_name: &str,
    keep_count: usize,
) -> Vec<(String, String)> {
    let keep = keep_count.min(50);
    let mut matching: Vec<&ArchiveScanEntry> = existing_archives
        .iter()
        .filter(|a| a.env_name == env_name)
        .collect();
    // Sort newest first, then prune the tail beyond `keep`.
    matching.sort_by(|a, b| {
        b.mtime_secs
            .cmp(&a.mtime_secs)
            .then_with(|| a.file_name.cmp(&b.file_name))
    });
    if matching.len() <= keep {
        return Vec::new();
    }
    matching[keep..]
        .iter()
        .map(|a| (a.file_path.clone(), a.workspace_archive_path.clone()))
        .collect()
}

/// Evaluates whether a periodic backup should run alongside auto-snapshot.
pub fn decide_scheduled_backup(
    env_name: &str,
    is_template: bool,
    auto_snapshot_enabled: bool,
    policy: &ScheduledBackupPolicy,
    backups_dir: &str,
    workspace_path: &str,
    now_epoch: i64,
) -> Option<ScheduledBackupPlan> {
    if !policy.enabled || !auto_snapshot_enabled || is_template || env_name.trim().is_empty() {
        return None;
    }
    let clean_name = sanitize_name(env_name);
    if clean_name.is_empty() {
        return None;
    }

    let (entries, _) = scan_backup_archives_with_stats(backups_dir);
    if !should_create_scheduled_backup(&entries, &clean_name, now_epoch, policy.interval_secs) {
        return None;
    }

    let compression = CompressionCodec::detect();
    let ext = compression.extension();
    let stamp = now_compact_stamp();
    let backup_file = format!("{backups_dir}/{clean_name}-{stamp}.{ext}");
    let ws_archive_file = if policy.include_workspace
        && !workspace_path.trim().is_empty()
        && Path::new(workspace_path).exists()
    {
        format!("{backups_dir}/{clean_name}-{stamp}-workspace.{ext}")
    } else {
        String::new()
    };

    let mut script = build_export_steps_with_options(
        &clean_name,
        &backup_file,
        workspace_path,
        &ws_archive_file,
        compression,
        true,
    );
    script.push_str("\necho '@@STAGE:100:Scheduled backup export complete.'\n");

    // Reserve one slot for the new backup being created.
    let keep_existing = policy.keep_count.saturating_sub(1);
    let prune_archives = scheduled_backups_to_prune(&entries, &clean_name, keep_existing);

    Some(ScheduledBackupPlan {
        env_name: clean_name,
        backup_file,
        workspace_archive_file: ws_archive_file,
        compression,
        script,
        prune_archives,
    })
}

/// Computes the lowercase hex SHA-256 digest of a byte slice.
pub fn sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize_hex()
}

/// Streams `path` in 64 KiB chunks and returns its lowercase hex SHA-256 digest.
pub fn sha256_file(path: &Path) -> Result<String, BackupError> {
    let mut file = File::open(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            BackupError::NotFound(path.display().to_string())
        } else {
            BackupError::Io {
                path: path.display().to_string(),
                source: e,
            }
        }
    })?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).map_err(|e| BackupError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize_hex())
}

/// Helper to convert `ArchiveScanEntry` slice from JSON values if needed by callers.
pub fn entries_from_values(values: &[Value]) -> Vec<ArchiveScanEntry> {
    values
        .iter()
        .filter_map(|v| serde_json::from_value::<ArchiveScanEntry>(v.clone()).ok())
        .collect()
}

struct Sha256 {
    state: [u32; 8],
    bit_len: u64,
    buffer: [u8; 64],
    buffer_len: usize,
}

impl Sha256 {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            bit_len: 0,
            buffer: [0; 64],
            buffer_len: 0,
        }
    }

    fn update(&mut self, data: &[u8]) {
        self.bit_len = self.bit_len.wrapping_add((data.len() as u64) * 8);
        let mut offset = 0;
        if self.buffer_len > 0 {
            let need = 64 - self.buffer_len;
            let take = need.min(data.len());
            self.buffer[self.buffer_len..self.buffer_len + take].copy_from_slice(&data[..take]);
            self.buffer_len += take;
            offset += take;
            if self.buffer_len == 64 {
                let block = self.buffer;
                self.compress(&block);
                self.buffer_len = 0;
            }
        }
        while offset + 64 <= data.len() {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[offset..offset + 64]);
            self.compress(&block);
            offset += 64;
        }
        if offset < data.len() {
            let rem = data.len() - offset;
            self.buffer[..rem].copy_from_slice(&data[offset..]);
            self.buffer_len = rem;
        }
    }

    fn finalize_hex(mut self) -> String {
        self.buffer[self.buffer_len] = 0x80;
        self.buffer_len += 1;
        if self.buffer_len > 56 {
            for b in &mut self.buffer[self.buffer_len..] {
                *b = 0;
            }
            let block = self.buffer;
            self.compress(&block);
            self.buffer_len = 0;
        }
        for b in &mut self.buffer[self.buffer_len..56] {
            *b = 0;
        }
        self.buffer[56..64].copy_from_slice(&self.bit_len.to_be_bytes());
        let block = self.buffer;
        self.compress(&block);

        let mut out = String::with_capacity(64);
        for word in self.state {
            out.push_str(&format!("{word:08x}"));
        }
        out
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for (i, chunk) in block.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*chunk);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for (i, &wi) in w.iter().enumerate() {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(Self::K[i])
                .wrapping_add(wi);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}
