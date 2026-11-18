//! Backup import validation, manifest verification, and archive deletion.

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use crate::backup::export::{
    manifest_path_for, sha256_file, BackupError, BackupManifest, MANIFEST_VERSION,
};
use crate::util::names::sanitize_name;

/// Result of verifying a backup archive (and optional workspace archive) before import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveVerification {
    pub manifest_present: bool,
    pub archive_sha256: String,
    pub workspace_sha256: Option<String>,
}

/// Reads `<backup_file>.manifest.json` if it exists. Returns `Ok(None)` for legacy archives without a manifest.
pub fn read_backup_manifest(backup_file: &str) -> Result<Option<BackupManifest>, BackupError> {
    let manifest_path = manifest_path_for(backup_file);
    if !manifest_path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&manifest_path).map_err(|e| BackupError::Io {
        path: manifest_path.display().to_string(),
        source: e,
    })?;
    let manifest: BackupManifest =
        serde_json::from_str(&raw).map_err(|e| BackupError::ManifestCorrupt {
            path: manifest_path.display().to_string(),
            reason: e.to_string(),
        })?;
    if manifest.version == 0 || manifest.version > MANIFEST_VERSION {
        return Err(BackupError::ManifestCorrupt {
            path: manifest_path.display().to_string(),
            reason: format!("unsupported manifest version {}", manifest.version),
        });
    }
    if manifest.archive_sha256.len() != 64
        || !manifest
            .archive_sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
    {
        return Err(BackupError::ManifestCorrupt {
            path: manifest_path.display().to_string(),
            reason: "invalid archiveSha256 digest".to_string(),
        });
    }
    Ok(Some(manifest))
}

fn verify_file_against_expected(
    path_str: &str,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<String, BackupError> {
    let p = Path::new(path_str);
    let meta = fs::metadata(p).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            BackupError::NotFound(path_str.to_string())
        } else {
            BackupError::Io {
                path: path_str.to_string(),
                source: e,
            }
        }
    })?;
    if meta.len() != expected_size {
        return Err(BackupError::SizeMismatch {
            file: path_str.to_string(),
            expected: expected_size,
            actual: meta.len(),
        });
    }
    let actual_sha = sha256_file(p)?;
    if !actual_sha.eq_ignore_ascii_case(expected_sha256) {
        return Err(BackupError::ChecksumMismatch {
            file: path_str.to_string(),
            expected: expected_sha256.to_ascii_lowercase(),
            actual: actual_sha,
        });
    }
    Ok(actual_sha)
}

fn verify_legacy_archive_header(path_str: &str) -> Result<(), BackupError> {
    let p = Path::new(path_str);
    let meta = fs::metadata(p).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            BackupError::NotFound(path_str.to_string())
        } else {
            BackupError::Io {
                path: path_str.to_string(),
                source: e,
            }
        }
    })?;
    if meta.len() == 0 {
        return Err(BackupError::ArchiveCorrupt {
            path: path_str.to_string(),
            reason: "archive file is empty (0 bytes)".to_string(),
        });
    }
    let mut f = File::open(p).map_err(|e| BackupError::Io {
        path: path_str.to_string(),
        source: e,
    })?;
    let mut magic = [0u8; 4];
    let n = f.read(&mut magic).map_err(|e| BackupError::Io {
        path: path_str.to_string(),
        source: e,
    })?;
    if (path_str.ends_with(".tar.zst") || path_str.ends_with(".zst"))
        && (n < 4 || magic != [0x28, 0xb5, 0x2f, 0xfd])
    {
        return Err(BackupError::ArchiveCorrupt {
            path: path_str.to_string(),
            reason: "invalid zstd archive header".to_string(),
        });
    } else if (path_str.ends_with(".tar.gz") || path_str.ends_with(".gz"))
        && (n < 2 || magic[0] != 0x1f || magic[1] != 0x8b)
    {
        return Err(BackupError::ArchiveCorrupt {
            path: path_str.to_string(),
            reason: "invalid gzip archive header".to_string(),
        });
    }
    Ok(())
}

/// Verifies `backup_file` (and optional `workspace_archive_file`) against its manifest + SHA-256
/// checksum when present, or checks legacy archive headers when no manifest exists.
pub fn verify_backup_archive(
    backup_file: &str,
    workspace_archive_file: &str,
) -> Result<ArchiveVerification, BackupError> {
    let clean_backup = backup_file.trim();
    if clean_backup.is_empty() {
        return Err(BackupError::NotFound(String::new()));
    }
    if !Path::new(clean_backup).exists() {
        return Err(BackupError::NotFound(clean_backup.to_string()));
    }

    let manifest_opt = read_backup_manifest(clean_backup)?;
    if let Some(manifest) = manifest_opt {
        let archive_sha = verify_file_against_expected(
            clean_backup,
            manifest.archive_size_bytes,
            &manifest.archive_sha256,
        )?;
        let clean_ws = workspace_archive_file.trim();
        let ws_sha = if !clean_ws.is_empty() {
            match (
                manifest.workspace_archive_size_bytes,
                manifest.workspace_archive_sha256.as_deref(),
            ) {
                (Some(exp_size), Some(exp_sha)) => {
                    Some(verify_file_against_expected(clean_ws, exp_size, exp_sha)?)
                }
                _ => {
                    verify_legacy_archive_header(clean_ws)?;
                    Some(sha256_file(Path::new(clean_ws))?)
                }
            }
        } else {
            None
        };
        return Ok(ArchiveVerification {
            manifest_present: true,
            archive_sha256: archive_sha,
            workspace_sha256: ws_sha,
        });
    }

    verify_legacy_archive_header(clean_backup)?;
    let archive_sha = sha256_file(Path::new(clean_backup))?;
    let clean_ws = workspace_archive_file.trim();
    let ws_sha = if !clean_ws.is_empty() {
        verify_legacy_archive_header(clean_ws)?;
        Some(sha256_file(Path::new(clean_ws))?)
    } else {
        None
    };

    Ok(ArchiveVerification {
        manifest_present: false,
        archive_sha256: archive_sha,
        workspace_sha256: ws_sha,
    })
}

/// Validates preconditions for importing a backup archive. Returns `(target_name, clean_backup, clean_ws_archive)`.
pub fn validate_import_request(
    busy: bool,
    new_name: &str,
    target_exists: bool,
    backup_file: &str,
    workspace_archive_file: &str,
    demo_mode: bool,
) -> Result<(String, String, String), String> {
    if busy {
        return Err("Another operation is already running.".to_string());
    }
    let target_name = sanitize_name(new_name);
    if target_name.is_empty() {
        return Err("New environment name is invalid.".to_string());
    }
    if target_exists {
        return Err(format!("Environment '{target_name}' already exists."));
    }
    let clean_backup = backup_file.trim().to_string();
    if clean_backup.is_empty() {
        return Err("Backup archive path is required.".to_string());
    }
    if !demo_mode && !Path::new(&clean_backup).exists() {
        return Err(format!("Backup archive not found: {clean_backup}"));
    }
    let clean_ws_archive = workspace_archive_file.trim().to_string();
    Ok((target_name, clean_backup, clean_ws_archive))
}

/// Deletes a backup archive, its optional workspace archive, and its manifest sidecar from disk.
pub fn delete_backup_files(backup_file: &str, workspace_archive_file: &str) {
    let b = backup_file.trim();
    if !b.is_empty() {
        let _ = fs::remove_file(b);
        let _ = fs::remove_file(manifest_path_for(b));
    }
    let w = workspace_archive_file.trim();
    if !w.is_empty() {
        let _ = fs::remove_file(w);
    }
}
