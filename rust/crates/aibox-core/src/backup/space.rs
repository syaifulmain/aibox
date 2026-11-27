//! Disk space checks and directory size calculation for backup operations.

use std::ffi::CString;
use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use crate::backup::expand_tilde;
use crate::util::format::format_bytes;
use crate::util::json::to_json_compact;
use crate::util::paths::home_dir;
use crate::util::size::parse_size_bytes;

const SAFETY_MARGIN_BYTES: i64 = 100 * 1024 * 1024;
const DEMO_AVAILABLE_BYTES: i64 = 40 * 1024 * 1024 * 1024;
const FALLBACK_INSTANCE_BYTES: i64 = 512 * 1024 * 1024;

/// Checks whether `available_bytes` covers `required_bytes` plus a 100 MiB safety margin.
pub fn has_sufficient_disk_space(required_bytes: i64, available_bytes: i64) -> bool {
    if available_bytes < 0 || required_bytes < 0 {
        return false;
    }
    match required_bytes.checked_add(SAFETY_MARGIN_BYTES) {
        Some(needed) => available_bytes >= needed,
        None => false,
    }
}

fn dir_size_recursive(dir: &Path, total: &mut i64) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_file() {
            if let Ok(meta) = entry.metadata() {
                *total = total.saturating_add(meta.len() as i64);
            }
        } else if ft.is_dir() {
            dir_size_recursive(&entry.path(), total);
        }
    }
}

/// Walks `dir_path` without following symlinks and returns total regular-file bytes.
pub fn directory_size_bytes(dir_path: &str) -> i64 {
    if dir_path.is_empty() {
        return 0;
    }
    let root = Path::new(dir_path);
    let Ok(meta) = fs::metadata(root) else {
        return 0;
    };
    if meta.is_file() {
        return meta.len() as i64;
    }
    if !meta.is_dir() {
        return 0;
    }
    let mut total = 0i64;
    dir_size_recursive(root, &mut total);
    total
}

/// Returns available disk bytes for the filesystem containing `path` (walking up until an existing
/// ancestor is found), or `-1` on failure.
pub fn available_disk_bytes(path: &str) -> i64 {
    let mut probe = path.to_string();
    while !probe.is_empty() && !Path::new(&probe).exists() {
        let parent = Path::new(&probe)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        if parent.is_empty() || parent == probe {
            break;
        }
        probe = parent;
    }
    if probe.is_empty() {
        return -1;
    }
    let Ok(c_path) = CString::new(probe) else {
        return -1;
    };
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if rc != 0 {
        return -1;
    }
    (stat.f_bavail as i64).saturating_mul(stat.f_frsize as i64)
}

/// Checks whether `backups_dir` has enough free space to export `env` (plus optional workspace).
pub fn check_export_space(
    disk_usage_bytes: i64,
    disk_usage_str: &str,
    workspace_path: &str,
    include_workspace: bool,
    demo_mode: bool,
    backups_dir: &str,
) -> Value {
    let mut instance_bytes = disk_usage_bytes;
    if instance_bytes <= 0 {
        instance_bytes = parse_size_bytes(disk_usage_str);
    }
    if instance_bytes <= 0 {
        instance_bytes = FALLBACK_INSTANCE_BYTES;
    }

    let mut workspace_bytes = 0i64;
    if include_workspace {
        let ws = expand_tilde(workspace_path.trim(), &home_dir());
        workspace_bytes = directory_size_bytes(&ws);
    }

    let estimated_bytes = instance_bytes + workspace_bytes;
    let available_bytes = if demo_mode {
        DEMO_AVAILABLE_BYTES
    } else {
        available_disk_bytes(backups_dir)
    };
    let ok = has_sufficient_disk_space(estimated_bytes, available_bytes);

    let available_display = if available_bytes >= 0 {
        format_bytes(available_bytes)
    } else {
        "Unknown".to_string()
    };
    let message = if !ok {
        let free_str = if available_bytes >= 0 {
            format_bytes(available_bytes)
        } else {
            "0 MB".to_string()
        };
        format!(
            "Insufficient disk space in {backups_dir} (needs {} + 100 MB margin, only {free_str} free).",
            format_bytes(estimated_bytes)
        )
    } else {
        String::new()
    };

    json!({
        "ok": ok,
        "estimatedBytes": estimated_bytes,
        "estimatedDisplay": format_bytes(estimated_bytes),
        "availableBytes": available_bytes,
        "availableDisplay": available_display,
        "message": message,
    })
}

pub fn check_export_space_json(
    disk_usage_bytes: i64,
    disk_usage_str: &str,
    workspace_path: &str,
    include_workspace: bool,
    demo_mode: bool,
    backups_dir: &str,
) -> String {
    to_json_compact(&check_export_space(
        disk_usage_bytes,
        disk_usage_str,
        workspace_path,
        include_workspace,
        demo_mode,
        backups_dir,
    ))
    .unwrap_or_default()
}
