//! Backup archive directory scanning with an on-disk metadata index cache.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use chrono::{Local, TimeZone};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::util::format::format_bytes;
use crate::util::json::to_json_compact;
use crate::util::paths::{atomic_write, clean_path};

const ARCHIVE_INDEX_FILE: &str = ".archive-index.json";
const ARCHIVE_INDEX_VERSION: u32 = 1;

#[derive(Clone)]
struct ArchiveFile {
    file_name: String,
    abs_path: String,
    size_bytes: i64,
    mtime_secs: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachedArchiveMeta {
    mtime_secs: i64,
    size_bytes: i64,
    env_name: String,
    created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveIndex {
    version: u32,
    entries: BTreeMap<String, CachedArchiveMeta>,
}

/// Structured metadata for a scanned backup archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveScanEntry {
    pub file_name: String,
    pub file_path: String,
    pub env_name: String,
    pub size_bytes: i64,
    pub size_display: String,
    pub created_at: String,
    pub mtime_secs: i64,
    pub has_workspace_archive: bool,
    pub workspace_archive_path: String,
    pub workspace_archive_size_display: String,
}

/// Cache hit/miss statistics from a directory scan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanCacheStats {
    pub cache_hits: usize,
    pub cache_misses: usize,
}

fn format_archive_mtime(epoch_secs: i64) -> String {
    Local
        .timestamp_opt(epoch_secs, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default()
}

fn strip_archive_suffix(file_name: &str) -> Option<&str> {
    file_name
        .strip_suffix(".tar.gz")
        .or_else(|| file_name.strip_suffix(".tar.zst"))
}

fn strip_workspace_archive_suffix(file_name: &str) -> Option<&str> {
    file_name
        .strip_suffix("-workspace.tar.gz")
        .or_else(|| file_name.strip_suffix("-workspace.tar.zst"))
}

fn is_archive_file(file_name: &str) -> bool {
    file_name.ends_with(".tar.gz") || file_name.ends_with(".tar.zst")
}

fn is_workspace_archive_file(file_name: &str) -> bool {
    file_name.ends_with("-workspace.tar.gz") || file_name.ends_with("-workspace.tar.zst")
}

fn load_archive_index(dir: &Path) -> ArchiveIndex {
    let index_path = dir.join(ARCHIVE_INDEX_FILE);
    let Ok(raw) = fs::read_to_string(&index_path) else {
        return ArchiveIndex {
            version: ARCHIVE_INDEX_VERSION,
            entries: BTreeMap::new(),
        };
    };
    match serde_json::from_str::<ArchiveIndex>(&raw) {
        Ok(idx) if idx.version == ARCHIVE_INDEX_VERSION => idx,
        _ => ArchiveIndex {
            version: ARCHIVE_INDEX_VERSION,
            entries: BTreeMap::new(),
        },
    }
}

fn save_archive_index(dir: &Path, index: &ArchiveIndex) {
    let index_path = dir.join(ARCHIVE_INDEX_FILE);
    if let Ok(json) = serde_json::to_string_pretty(index) {
        let _ = atomic_write(&index_path, json.as_bytes());
    }
}

/// Scans `backups_dir` for `*.tar.gz` / `*.tar.zst` archives using `.archive-index.json` cache,
/// returning structured entries and cache statistics.
pub fn scan_backup_archives_with_stats(
    backups_dir: &str,
) -> (Vec<ArchiveScanEntry>, ScanCacheStats) {
    let dir = Path::new(backups_dir);
    if !dir.is_dir() {
        return (Vec::new(), ScanCacheStats::default());
    }
    let Ok(read_dir) = fs::read_dir(dir) else {
        return (Vec::new(), ScanCacheStats::default());
    };

    let mut entries: Vec<ArchiveFile> = Vec::new();
    for entry in read_dir.flatten() {
        let Ok(meta) = fs::metadata(entry.path()) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if !is_archive_file(&file_name) {
            continue;
        }
        let mtime_secs = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let abs_path = clean_path(&entry.path().to_string_lossy());
        entries.push(ArchiveFile {
            file_name,
            abs_path,
            size_bytes: meta.len() as i64,
            mtime_secs,
        });
    }

    // Match QDir::Time (most recent first); tie-break by filename for stability.
    entries.sort_by(|a, b| {
        b.mtime_secs
            .cmp(&a.mtime_secs)
            .then_with(|| a.file_name.cmp(&b.file_name))
    });

    let mut workspace_map: BTreeMap<String, ArchiveFile> = BTreeMap::new();
    for fi in &entries {
        if let Some(key) = strip_workspace_archive_suffix(&fi.file_name) {
            workspace_map.insert(key.to_string(), fi.clone());
        }
    }

    let old_index = load_archive_index(dir);
    let mut new_index = ArchiveIndex {
        version: ARCHIVE_INDEX_VERSION,
        entries: BTreeMap::new(),
    };
    let mut stats = ScanCacheStats::default();
    let ts_suffix_re = Regex::new(r"-\d{8}-\d{6}$").expect("valid regex");
    let mut result = Vec::new();

    for fi in &entries {
        if is_workspace_archive_file(&fi.file_name) {
            continue;
        }
        let base_name = strip_archive_suffix(&fi.file_name).unwrap_or(&fi.file_name);

        let cached_meta = old_index
            .entries
            .get(&fi.file_name)
            .filter(|c| c.mtime_secs == fi.mtime_secs && c.size_bytes == fi.size_bytes);

        let (env_name, created_at) = if let Some(cached) = cached_meta {
            stats.cache_hits += 1;
            (cached.env_name.clone(), cached.created_at.clone())
        } else {
            stats.cache_misses += 1;
            (
                ts_suffix_re.replace(base_name, "").into_owned(),
                format_archive_mtime(fi.mtime_secs),
            )
        };

        new_index.entries.insert(
            fi.file_name.clone(),
            CachedArchiveMeta {
                mtime_secs: fi.mtime_secs,
                size_bytes: fi.size_bytes,
                env_name: env_name.clone(),
                created_at: created_at.clone(),
            },
        );

        let ws_info = workspace_map.get(base_name);
        let has_ws = ws_info.is_some();
        let ws_path = ws_info.map(|w| w.abs_path.clone()).unwrap_or_default();
        let ws_size = ws_info
            .map(|w| format_bytes(w.size_bytes))
            .unwrap_or_default();

        result.push(ArchiveScanEntry {
            file_name: fi.file_name.clone(),
            file_path: fi.abs_path.clone(),
            env_name,
            size_bytes: fi.size_bytes,
            size_display: format_bytes(fi.size_bytes),
            created_at,
            mtime_secs: fi.mtime_secs,
            has_workspace_archive: has_ws,
            workspace_archive_path: ws_path,
            workspace_archive_size_display: ws_size,
        });
    }

    if stats.cache_misses > 0 || old_index.entries.len() != new_index.entries.len() {
        save_archive_index(dir, &new_index);
    }

    (result, stats)
}

/// Scans `backups_dir` for `*.tar.gz` / `*.tar.zst` archives and pairs `*-workspace.*` companions.
pub fn scan_backup_archives(backups_dir: &str) -> Vec<Value> {
    let (entries, _) = scan_backup_archives_with_stats(backups_dir);
    entries
        .into_iter()
        .map(|e| {
            json!({
                "fileName": e.file_name,
                "filePath": e.file_path,
                "envName": e.env_name,
                "sizeBytes": e.size_bytes,
                "sizeDisplay": e.size_display,
                "createdAt": e.created_at,
                "hasWorkspaceArchive": e.has_workspace_archive,
                "workspaceArchivePath": e.workspace_archive_path,
                "workspaceArchiveSizeDisplay": e.workspace_archive_size_display,
            })
        })
        .collect()
}

pub fn scan_backup_archives_json(backups_dir: &str) -> String {
    to_json_compact(&Value::Array(scan_backup_archives(backups_dir))).unwrap_or_default()
}
