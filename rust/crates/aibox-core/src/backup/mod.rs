//! Backup export/import, disk space checks, archive scanning, stale reports, and orphan cleanup.

pub mod orphans;
pub mod scripts;
pub mod stale;

use std::collections::BTreeMap;
use std::ffi::CString;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use chrono::{Local, TimeZone};
use regex::Regex;
use serde_json::{json, Value};

use crate::util::format::format_bytes;
use crate::util::json::to_json_compact;
use crate::util::names::sanitize_name;
use crate::util::paths::{clean_path, home_dir};
use crate::util::size::parse_size_bytes;

pub use orphans::{
    admin_delete_script_text, delete_orphan_script, detect_orphans, prepare_delete_orphan_as_admin,
    validate_orphan_path, validate_orphan_request,
};
pub use scripts::{
    build_export_steps, export_and_wipe_environment_script, export_environment_script,
    import_environment_script,
};
pub use stale::{
    build_stale_report, classify_environment_staleness, StalenessClass, StalenessResult,
};

const SAFETY_MARGIN_BYTES: i64 = 100 * 1024 * 1024;
const DEMO_AVAILABLE_BYTES: i64 = 40 * 1024 * 1024 * 1024;
const FALLBACK_INSTANCE_BYTES: i64 = 512 * 1024 * 1024;

pub(crate) fn expand_tilde(path: &str, home: &Path) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        format!("{}/{rest}", home.display())
    } else {
        path.to_string()
    }
}

/// Returns the backups directory path under `os_storage_path`.
pub fn backups_dir_path(os_storage_path: &str) -> String {
    format!("{os_storage_path}/backups")
}

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
        let Ok(meta) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        let ft = meta.file_type();
        if ft.is_symlink() {
            continue;
        }
        if ft.is_file() {
            *total = total.saturating_add(meta.len() as i64);
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

#[derive(Clone)]
struct ArchiveFile {
    file_name: String,
    abs_path: String,
    size_bytes: i64,
    mtime_secs: i64,
}

fn format_archive_mtime(epoch_secs: i64) -> String {
    Local
        .timestamp_opt(epoch_secs, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default()
}

/// Scans `backups_dir` for `*.tar.gz` archives and pairs `*-workspace.tar.gz` companions.
pub fn scan_backup_archives(backups_dir: &str) -> Vec<Value> {
    let dir = Path::new(backups_dir);
    if !dir.is_dir() {
        return Vec::new();
    }
    let Ok(read_dir) = fs::read_dir(dir) else {
        return Vec::new();
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
        if !file_name.ends_with(".tar.gz") {
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
        if let Some(key) = fi.file_name.strip_suffix("-workspace.tar.gz") {
            workspace_map.insert(key.to_string(), fi.clone());
        }
    }

    let ts_suffix_re = Regex::new(r"-\d{8}-\d{6}$").expect("valid regex");
    let mut result = Vec::new();
    for fi in &entries {
        if fi.file_name.ends_with("-workspace.tar.gz") {
            continue;
        }
        let base_name = fi
            .file_name
            .strip_suffix(".tar.gz")
            .unwrap_or(&fi.file_name);
        let env_name = ts_suffix_re.replace(base_name, "").into_owned();
        let ws_info = workspace_map.get(base_name);
        let has_ws = ws_info.is_some();
        let ws_path = ws_info.map(|w| w.abs_path.clone()).unwrap_or_default();
        let ws_size = ws_info
            .map(|w| format_bytes(w.size_bytes))
            .unwrap_or_default();

        result.push(json!({
            "fileName": fi.file_name,
            "filePath": fi.abs_path,
            "envName": env_name,
            "sizeBytes": fi.size_bytes,
            "sizeDisplay": format_bytes(fi.size_bytes),
            "createdAt": format_archive_mtime(fi.mtime_secs),
            "hasWorkspaceArchive": has_ws,
            "workspaceArchivePath": ws_path,
            "workspaceArchiveSizeDisplay": ws_size,
        }));
    }

    result
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

/// Deletes a backup archive and its optional workspace archive from disk.
pub fn delete_backup_files(backup_file: &str, workspace_archive_file: &str) {
    let b = backup_file.trim();
    if !b.is_empty() {
        let _ = fs::remove_file(b);
    }
    let w = workspace_archive_file.trim();
    if !w.is_empty() {
        let _ = fs::remove_file(w);
    }
}

fn parse_env_array(environments_json: &str) -> Vec<Value> {
    match serde_json::from_str::<Value>(environments_json) {
        Ok(Value::Array(arr)) => arr,
        _ => Vec::new(),
    }
}

pub fn scan_backup_archives_json(backups_dir: &str) -> String {
    to_json_compact(&Value::Array(scan_backup_archives(backups_dir))).unwrap_or_default()
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

pub fn detect_orphans_json(
    environments_json: &str,
    workspace_base: &str,
    backups_dir: &str,
) -> String {
    let envs = parse_env_array(environments_json);
    to_json_compact(&Value::Array(detect_orphans(
        &envs,
        workspace_base,
        backups_dir,
    )))
    .unwrap_or_default()
}

pub fn build_stale_report_json(
    environments_json: &str,
    workspace_base: &str,
    backups_dir: &str,
    stale_days: i32,
    now_epoch: i64,
) -> String {
    let envs = parse_env_array(environments_json);
    to_json_compact(&build_stale_report(
        &envs,
        workspace_base,
        backups_dir,
        stale_days,
        now_epoch,
    ))
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::process::Command;

    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new(prefix: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "aibox-backup-test-{prefix}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            ));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).expect("create temp dir");
            Self(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn disk_space_check_enforces_safety_margin() {
        let margin = 100 * 1024 * 1024;
        assert!(!has_sufficient_disk_space(
            500 * 1024 * 1024,
            500 * 1024 * 1024
        ));
        assert!(!has_sufficient_disk_space(
            500 * 1024 * 1024,
            500 * 1024 * 1024 + margin - 1
        ));
        assert!(has_sufficient_disk_space(
            500 * 1024 * 1024,
            500 * 1024 * 1024 + margin
        ));
        assert!(!has_sufficient_disk_space(100, -1));
    }

    #[test]
    fn scan_backup_archives_pairs_workspace_tarball() {
        let dir = TempDir::new("scan");
        fs::write(
            dir.path().join("alpha-20261113-120000.tar.gz"),
            b"backup-data",
        )
        .unwrap();
        fs::write(
            dir.path().join("alpha-20261113-120000-workspace.tar.gz"),
            b"ws-data",
        )
        .unwrap();
        fs::write(dir.path().join("beta-20261113-120500.tar.gz"), b"beta-only").unwrap();

        let list = scan_backup_archives(&dir.path().to_string_lossy());
        assert_eq!(list.len(), 2);

        let alpha = list
            .iter()
            .find(|v| v["envName"] == "alpha")
            .expect("alpha");
        let beta = list.iter().find(|v| v["envName"] == "beta").expect("beta");
        assert_eq!(alpha["hasWorkspaceArchive"], true);
        assert!(alpha["workspaceArchivePath"]
            .as_str()
            .unwrap()
            .ends_with("alpha-20261113-120000-workspace.tar.gz"));
        assert_eq!(beta["hasWorkspaceArchive"], false);
    }

    #[test]
    fn export_and_import_scripts_execute_with_workspace_round_trip() {
        let dir = TempDir::new("roundtrip");
        let ws_src = dir.path().join("ws-src");
        let ws_dst = dir.path().join("ws-dst");
        fs::create_dir_all(&ws_src).unwrap();
        fs::write(ws_src.join("notes.txt"), b"hello-aibox-backup").unwrap();

        let stub_dir = dir.path().join("bin");
        fs::create_dir_all(&stub_dir).unwrap();
        let incus_log = dir.path().join("incus-calls.log");
        let incus_bin = stub_dir.join("incus");
        fs::write(
            &incus_bin,
            "#!/bin/sh\n\
             echo \"$@\" >> \"$AIBOX_TEST_LOG\"\n\
             if [ \"$1\" = \"export\" ]; then\n  \
               echo fake-archive > \"$3\"\n\
             fi\n\
             exit 0\n",
        )
        .unwrap();
        fs::set_permissions(&incus_bin, fs::Permissions::from_mode(0o755)).unwrap();

        let backup_file = dir.path().join("backups/myenv-20261113.tar.gz");
        let ws_archive = dir.path().join("backups/myenv-20261113-workspace.tar.gz");
        let export_script = export_environment_script(
            "myenv",
            &backup_file.to_string_lossy(),
            &ws_src.to_string_lossy(),
            &ws_archive.to_string_lossy(),
        );
        let import_script = import_environment_script(
            &backup_file.to_string_lossy(),
            "myenv-copy",
            &ws_dst.to_string_lossy(),
            &ws_archive.to_string_lossy(),
        );

        let runner_path = dir.path().join("roundtrip.sh");
        fs::write(&runner_path, format!("{export_script}\n{import_script}")).unwrap();

        let orig_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!("{}:{orig_path}", stub_dir.display());
        let output = Command::new("/bin/bash")
            .arg(&runner_path)
            .env("PATH", new_path)
            .env("AIBOX_TEST_LOG", &incus_log)
            .output()
            .expect("run bash");
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        assert!(backup_file.exists());
        assert!(ws_archive.exists());
        assert_eq!(
            fs::read(ws_dst.join("notes.txt")).unwrap(),
            b"hello-aibox-backup"
        );

        let log_text = fs::read_to_string(&incus_log).unwrap();
        assert!(log_text.contains("export myenv "));
        assert!(log_text.contains(&format!("import {} myenv-copy", backup_file.display())));
        assert!(log_text.contains("config unset myenv-copy user.aibox.template"));
        assert!(log_text.contains(&format!(
            "config set myenv-copy user.aibox.workspace {}",
            ws_dst.display()
        )));
    }

    #[test]
    fn stale_classification_matches_qt_tests() {
        let now_sec = 1_794_500_000_i64;
        let fresh = classify_environment_staleness(now_sec - 5 * 86_400, false, 30, now_sec);
        assert_eq!(fresh.classification, StalenessClass::Fresh);
        assert_eq!(fresh.days_unused, 5);

        let stale = classify_environment_staleness(now_sec - 35 * 86_400, false, 30, now_sec);
        assert_eq!(stale.classification, StalenessClass::Stale);
        assert_eq!(stale.days_unused, 35);

        let unknown = classify_environment_staleness(0, false, 30, now_sec);
        assert_eq!(unknown.classification, StalenessClass::Unknown);
        assert_eq!(unknown.days_unused, -1);

        let tpl = classify_environment_staleness(now_sec - 120 * 86_400, true, 30, now_sec);
        assert_eq!(tpl.classification, StalenessClass::Template);
    }

    #[test]
    fn stale_orphan_detection_and_export_wipe_safety() {
        let dir = TempDir::new("stale-orphans");
        let ws_base = dir.path().join("ws");
        let snap_base = ws_base.join(".snapshots");
        let backups_dir = dir.path().join("backups");
        fs::create_dir_all(ws_base.join("keep-env")).unwrap();
        fs::create_dir_all(ws_base.join("stale-env")).unwrap();
        fs::create_dir_all(ws_base.join("ghost-ws")).unwrap();
        fs::create_dir_all(snap_base.join("keep-env/snap1")).unwrap();
        fs::create_dir_all(snap_base.join("ghost-snaps/old1")).unwrap();
        fs::create_dir_all(&backups_dir).unwrap();

        fs::write(ws_base.join("keep-env/app.txt"), b"active workspace").unwrap();
        fs::write(ws_base.join("stale-env/readme.md"), vec![b's'; 4096]).unwrap();
        fs::write(ws_base.join("ghost-ws/leak.bin"), vec![b'g'; 2048]).unwrap();
        fs::write(
            snap_base.join("ghost-snaps/old1/data.bin"),
            vec![b'x'; 1024],
        )
        .unwrap();

        fs::write(backups_dir.join("keep-env-20261110-120000.tar.gz"), b"inst").unwrap();
        fs::write(
            backups_dir.join("keep-env-20261110-120000-workspace.tar.gz"),
            b"ws",
        )
        .unwrap();
        fs::write(
            backups_dir.join("ghost-old-20261001-010203-workspace.tar.gz"),
            vec![b'b'; 512],
        )
        .unwrap();

        let now_sec = 1_794_500_000_i64;
        let envs = vec![
            json!({
                "name": "keep-env",
                "running": true,
                "type": "CONTAINER",
                "osName": "Ubuntu",
                "isTemplate": false,
                "lastUsedEpoch": now_sec - 2 * 86_400,
                "diskUsageBytes": 10_485_760_i64,
                "snapshotCount": 1,
                "snapshotTotalBytes": 1_048_576_i64,
                "workspacePath": ws_base.join("keep-env").to_string_lossy(),
            }),
            json!({
                "name": "stale-env",
                "running": false,
                "type": "CONTAINER",
                "osName": "Debian",
                "isTemplate": false,
                "lastUsedEpoch": now_sec - 45 * 86_400,
                "diskUsageBytes": 20_971_520_i64,
                "snapshotCount": 2,
                "snapshotTotalBytes": 5_242_880_i64,
                "workspacePath": ws_base.join("stale-env").to_string_lossy(),
            }),
            json!({
                "name": "tpl-old",
                "running": false,
                "type": "CONTAINER",
                "osName": "Arch",
                "isTemplate": true,
                "lastUsedEpoch": now_sec - 180 * 86_400,
                "diskUsageBytes": 5_242_880_i64,
                "snapshotCount": 0,
                "snapshotTotalBytes": 0_i64,
                "workspacePath": ws_base.join("tpl-old").to_string_lossy(),
            }),
            json!({
                "name": "never-stamped",
                "running": false,
                "type": "CONTAINER",
                "osName": "Alpine",
                "isTemplate": false,
                "lastUsedEpoch": 0_i64,
                "diskUsageBytes": 1_048_576_i64,
                "snapshotCount": 0,
                "snapshotTotalBytes": 0_i64,
                "workspacePath": ws_base.join("never-stamped").to_string_lossy(),
            }),
        ];

        let report = build_stale_report(
            &envs,
            &ws_base.to_string_lossy(),
            &backups_dir.to_string_lossy(),
            30,
            now_sec,
        );
        assert_eq!(report["staleCount"], 1);
        assert_eq!(report["unknownCount"], 1);
        assert_eq!(report["freshCount"], 1);
        assert_eq!(report["templateCount"], 1);
        assert_eq!(report["orphanCount"], 3);

        let entries = report["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["name"], "stale-env");
        assert_eq!(entries[0]["status"], "stale");
        assert_eq!(entries[0]["workspaceSizeBytes"], 4096);
        assert_eq!(entries[1]["name"], "never-stamped");
        assert_eq!(entries[1]["status"], "unknown");

        let kinds: Vec<&str> = report["orphans"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["kind"].as_str().unwrap())
            .collect();
        assert!(kinds.contains(&"workspace"));
        assert!(kinds.contains(&"workspace-snapshot"));
        assert!(kinds.contains(&"backup-workspace"));

        // Hostile / safety validation checks
        let ws_s = ws_base.to_string_lossy();
        let bkp_s = backups_dir.to_string_lossy();
        assert!(validate_orphan_request(
            "workspace",
            &dir.path().join("outside").to_string_lossy(),
            &ws_s,
            &bkp_s
        )
        .is_err());
        assert!(
            validate_orphan_request("workspace", &snap_base.to_string_lossy(), &ws_s, &bkp_s)
                .is_err()
        );
        assert!(validate_orphan_request(
            "backup-workspace",
            &backups_dir
                .join("keep-env-20261110-120000.tar.gz")
                .to_string_lossy(),
            &ws_s,
            &bkp_s
        )
        .is_err());
        // .. traversal & symlink escape
        assert!(validate_orphan_request(
            "workspace",
            &format!("{ws_s}/ghost-ws/../../outside"),
            &ws_s,
            &bkp_s
        )
        .is_err());
        let sym_path = ws_base.join("sym-escape");
        symlink(dir.path(), &sym_path).unwrap();
        assert!(
            validate_orphan_request("workspace", &sym_path.to_string_lossy(), &ws_s, &bkp_s)
                .is_err()
        );

        // Export & Wipe verification gate
        let bin_dir = dir.path().join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let incus_log = dir.path().join("incus_calls.log");
        let incus_bin = bin_dir.join("incus");
        fs::write(
            &incus_bin,
            "#!/bin/sh\n\
             echo \"$*\" >> \"$TEST_INCUS_LOG\"\n\
             if [ \"$1\" = \"export\" ]; then\n  \
               if [ \"${TEST_EXPORT_MODE:-fail}\" = \"ok\" ]; then\n    \
                 printf 'ARCHIVE_DATA' > \"$3\"\n    \
                 exit 0\n  \
               elif [ \"${TEST_EXPORT_MODE:-fail}\" = \"empty\" ]; then\n    \
                 : > \"$3\"\n    \
                 exit 0\n  \
               else\n    \
                 exit 1\n  \
               fi\n\
             fi\n\
             exit 0\n",
        )
        .unwrap();
        fs::set_permissions(&incus_bin, fs::Permissions::from_mode(0o755)).unwrap();

        let target_ws = ws_base.join("stale-env");
        let out_backup = backups_dir.join("stale-env-test.tar.gz");
        let out_ws_backup = backups_dir.join("stale-env-test-workspace.tar.gz");
        let script = export_and_wipe_environment_script(
            "stale-env",
            &out_backup.to_string_lossy(),
            &target_ws.to_string_lossy(),
            &out_ws_backup.to_string_lossy(),
            true,
            "",
            "",
        );

        let orig_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!("{}:{orig_path}", bin_dir.display());

        // Case 1: export exits 1 -> wipe must NOT run
        let out_fail = Command::new("/bin/bash")
            .args(["-c", &script])
            .env("PATH", &new_path)
            .env("TEST_INCUS_LOG", &incus_log)
            .env("TEST_EXPORT_MODE", "fail")
            .output()
            .unwrap();
        assert!(!out_fail.status.success());
        assert!(target_ws.join("readme.md").exists());
        assert!(!fs::read_to_string(&incus_log).unwrap().contains("delete"));

        // Case 2: 0-byte archive -> verification gate blocks wipe
        let _ = fs::remove_file(&incus_log);
        let out_empty = Command::new("/bin/bash")
            .args(["-c", &script])
            .env("PATH", &new_path)
            .env("TEST_INCUS_LOG", &incus_log)
            .env("TEST_EXPORT_MODE", "empty")
            .output()
            .unwrap();
        assert!(!out_empty.status.success());
        assert!(target_ws.join("readme.md").exists());
        assert!(!fs::read_to_string(&incus_log).unwrap().contains("delete"));

        // Case 3: ok -> wipe proceeds
        let _ = fs::remove_file(&incus_log);
        let out_ok = Command::new("/bin/bash")
            .args(["-c", &script])
            .env("PATH", &new_path)
            .env("TEST_INCUS_LOG", &incus_log)
            .env("TEST_EXPORT_MODE", "ok")
            .output()
            .unwrap();
        assert!(out_ok.status.success());
        assert!(!target_ws.exists());
        assert!(fs::read_to_string(&incus_log)
            .unwrap()
            .contains("delete stale-env --force"));
    }

    #[test]
    fn orphan_delete_scripts_handle_root_owned_files() {
        let dir = TempDir::new("orphan-scripts");
        let ws_base = dir.path().join("ws");
        fs::create_dir_all(&ws_base).unwrap();
        let target = ws_base.join("ghost-ws");
        fs::create_dir_all(&target).unwrap();

        let script = delete_orphan_script("workspace", &target.to_string_lossy());
        assert!(script.contains("sudo -n rm -rf"));
        assert!(script.contains("Fix (admin): sudo rm -rf"));

        let snap_script = delete_orphan_script(
            "workspace-snapshot",
            &ws_base.join(".snapshots/ghost").to_string_lossy(),
        );
        assert!(snap_script.contains("sudo -n rm -rf"));

        let admin = admin_delete_script_text("workspace", &target.to_string_lossy());
        assert!(admin.contains("btrfs subvolume show"));
        assert!(admin.contains("rm -rf \"$TARGET\""));

        assert!(validate_orphan_request(
            "workspace",
            "/tmp/not-a-workspace",
            &ws_base.to_string_lossy(),
            "/tmp/backups"
        )
        .is_err());
    }
}
