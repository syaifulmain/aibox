//! Backup export/import, disk space checks, archive scanning, stale reports, and orphan cleanup.

pub mod archive_scan;
pub mod export;
pub mod import;
pub mod orphans;
pub mod scripts;
pub mod space;
pub mod stale;

use std::path::Path;

use serde_json::Value;

use crate::util::json::to_json_compact;

pub use archive_scan::{
    scan_backup_archives, scan_backup_archives_json, scan_backup_archives_with_stats,
    ArchiveScanEntry, ScanCacheStats,
};
pub use export::{
    decide_scheduled_backup, manifest_path_for, scheduled_backups_to_prune, sha256_bytes,
    sha256_file, should_create_scheduled_backup, validate_export_request, write_backup_manifest,
    BackupError, BackupManifest, CompressionCodec, ScheduledBackupPlan, ScheduledBackupPolicy,
};
pub use import::{
    delete_backup_files, read_backup_manifest, validate_import_request, verify_backup_archive,
    ArchiveVerification,
};
pub use orphans::{
    admin_delete_script_text, delete_orphan_script, detect_orphans, execute_orphan_cleanup,
    plan_orphan_cleanup, prepare_delete_orphan_as_admin, validate_orphan_path,
    validate_orphan_request, OrphanCleanupMode, OrphanCleanupPlan, OrphanCleanupReport,
    OrphanPlanItem,
};
pub use scripts::{
    build_export_steps, build_export_steps_with_options, export_and_wipe_environment_script,
    export_and_wipe_environment_script_with_options, export_environment_script,
    export_environment_script_with_options, import_environment_script,
    import_environment_script_with_options,
};
pub use space::{
    available_disk_bytes, check_export_space, check_export_space_json, directory_size_bytes,
    has_sufficient_disk_space,
};
pub use stale::{
    build_stale_report, classify_environment_staleness, StalenessClass, StalenessResult,
};

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

fn parse_env_array(environments_json: &str) -> Vec<Value> {
    match serde_json::from_str::<Value>(environments_json) {
        Ok(Value::Array(arr)) => arr,
        _ => Vec::new(),
    }
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
    use serde_json::json;
    use std::fs;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::process::Command;
    use std::time::UNIX_EPOCH;

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

    #[test]
    fn sha256_matches_nist_test_vectors() {
        assert_eq!(
            sha256_bytes(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_bytes(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn export_import_manifest_round_trip_and_checksum_verification() {
        let dir = TempDir::new("manifest-roundtrip");
        let ws_src = dir.path().join("ws-src");
        let ws_dst = dir.path().join("ws-dst");
        fs::create_dir_all(&ws_src).unwrap();
        fs::write(ws_src.join("code.rs"), b"fn main() { println!(\"ok\"); }\n").unwrap();

        let stub_dir = dir.path().join("bin");
        fs::create_dir_all(&stub_dir).unwrap();
        let incus_bin = stub_dir.join("incus");
        fs::write(
            &incus_bin,
            "#!/bin/sh\n\
             if [ \"$1\" = \"export\" ]; then\n  \
               printf '\\x1f\\x8bfake-gzip-payload' > \"$3\"\n\
             fi\n\
             exit 0\n",
        )
        .unwrap();
        fs::set_permissions(&incus_bin, fs::Permissions::from_mode(0o755)).unwrap();

        let backup_file = dir.path().join("backups/myenv-20261118-120000.tar.gz");
        let ws_archive = dir
            .path()
            .join("backups/myenv-20261118-120000-workspace.tar.gz");

        let export_script = export_environment_script_with_options(
            "myenv",
            &backup_file.to_string_lossy(),
            &ws_src.to_string_lossy(),
            &ws_archive.to_string_lossy(),
            CompressionCodec::Gzip,
            true,
        );
        let import_script = import_environment_script_with_options(
            &backup_file.to_string_lossy(),
            "myenv-restored",
            &ws_dst.to_string_lossy(),
            &ws_archive.to_string_lossy(),
            true,
        );

        let orig_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!("{}:{orig_path}", stub_dir.display());
        let out_exp = Command::new("/bin/bash")
            .args(["-c", &export_script])
            .env("PATH", &new_path)
            .output()
            .unwrap();
        assert!(
            out_exp.status.success(),
            "export stderr: {}",
            String::from_utf8_lossy(&out_exp.stderr)
        );

        let manifest_path = manifest_path_for(&backup_file.to_string_lossy());
        assert!(manifest_path.exists());

        let verified = verify_backup_archive(
            &backup_file.to_string_lossy(),
            &ws_archive.to_string_lossy(),
        )
        .expect("verified archive");
        assert!(verified.manifest_present);
        assert_eq!(verified.archive_sha256, sha256_file(&backup_file).unwrap());
        assert_eq!(
            verified.workspace_sha256.unwrap(),
            sha256_file(&ws_archive).unwrap()
        );

        let out_imp = Command::new("/bin/bash")
            .args(["-c", &import_script])
            .env("PATH", &new_path)
            .output()
            .unwrap();
        assert!(
            out_imp.status.success(),
            "import stderr: {}",
            String::from_utf8_lossy(&out_imp.stderr)
        );
        assert_eq!(
            fs::read(ws_dst.join("code.rs")).unwrap(),
            b"fn main() { println!(\"ok\"); }\n"
        );
    }

    #[test]
    fn corrupt_and_truncated_archives_return_structured_errors() {
        let dir = TempDir::new("corrupt-archive");
        let backup_file = dir.path().join("env.tar.gz");
        let ws_archive = dir.path().join("env-workspace.tar.gz");
        fs::write(&backup_file, b"\x1f\x8bvalid-archive-bytes").unwrap();
        fs::write(&ws_archive, b"\x1f\x8bvalid-workspace-bytes").unwrap();

        write_backup_manifest(
            "env",
            &backup_file.to_string_lossy(),
            Some(&ws_archive.to_string_lossy()),
            CompressionCodec::Gzip,
            1_794_500_000,
        )
        .unwrap();

        // 1. Corrupt bytes (same length -> ChecksumMismatch)
        fs::write(&backup_file, b"\x1f\x8bXXXXX-archive-bytes").unwrap();
        let err = verify_backup_archive(
            &backup_file.to_string_lossy(),
            &ws_archive.to_string_lossy(),
        )
        .unwrap_err();
        assert!(matches!(err, BackupError::ChecksumMismatch { .. }));

        // 2. Truncated archive (SizeMismatch)
        fs::write(&backup_file, b"\x1f\x8b").unwrap();
        let err = verify_backup_archive(
            &backup_file.to_string_lossy(),
            &ws_archive.to_string_lossy(),
        )
        .unwrap_err();
        assert!(matches!(err, BackupError::SizeMismatch { .. }));

        // 3. Corrupt manifest JSON -> ManifestCorrupt
        fs::write(
            manifest_path_for(&backup_file.to_string_lossy()),
            b"{bad-json",
        )
        .unwrap();
        let err = verify_backup_archive(&backup_file.to_string_lossy(), "").unwrap_err();
        assert!(matches!(err, BackupError::ManifestCorrupt { .. }));

        // 4. Legacy archive without manifest with invalid gzip magic -> ArchiveCorrupt
        let legacy = dir.path().join("legacy.tar.gz");
        fs::write(&legacy, b"not-gzip").unwrap();
        let err = verify_backup_archive(&legacy.to_string_lossy(), "").unwrap_err();
        assert!(matches!(err, BackupError::ArchiveCorrupt { .. }));

        // 5. Missing archive -> NotFound
        let missing = dir.path().join("missing.tar.gz");
        let err = verify_backup_archive(&missing.to_string_lossy(), "").unwrap_err();
        assert!(matches!(err, BackupError::NotFound(_)));
    }

    #[test]
    fn safe_orphan_cleanup_dry_run_and_active_env_protection() {
        let dir = TempDir::new("safe-orphan-cleanup");
        let ws_base = dir.path().join("ws");
        let snap_base = ws_base.join(".snapshots");
        let backups_dir = dir.path().join("backups");
        fs::create_dir_all(ws_base.join("active-env")).unwrap();
        fs::create_dir_all(ws_base.join("active-env.pre-subvol")).unwrap();
        fs::create_dir_all(ws_base.join("orphan-ws")).unwrap();
        fs::create_dir_all(snap_base.join("active-env/s1")).unwrap();
        fs::create_dir_all(&backups_dir).unwrap();

        fs::write(ws_base.join("active-env/important.txt"), b"keep me").unwrap();
        fs::write(ws_base.join("orphan-ws/junk.bin"), vec![b'x'; 256]).unwrap();
        let orphan_arc = backups_dir.join("dead-20261101-010203-workspace.tar.zst");
        fs::write(&orphan_arc, vec![b'z'; 128]).unwrap();

        let envs = vec![json!({
            "name": "active-env",
            "workspacePath": ws_base.join("active-env").to_string_lossy(),
        })];

        // Active env workspace, .pre-subvol, and snapshots must NEVER be detected as orphans.
        let detected = detect_orphans(
            &envs,
            &ws_base.to_string_lossy(),
            &backups_dir.to_string_lossy(),
        );
        assert_eq!(detected.len(), 2);
        assert!(!detected
            .iter()
            .any(|o| o["name"] == "active-env" || o["name"] == "active-env.pre-subvol"));

        let plan = plan_orphan_cleanup(
            &envs,
            &ws_base.to_string_lossy(),
            &backups_dir.to_string_lossy(),
            None,
        );
        assert!(plan.dry_run);
        assert!(plan.requires_confirmation);
        assert_eq!(plan.confirmation_list.len(), 2);

        // Default DryRun mode must not delete anything.
        let dry_report = execute_orphan_cleanup(
            &plan,
            OrphanCleanupMode::default(),
            &envs,
            &ws_base.to_string_lossy(),
            &backups_dir.to_string_lossy(),
        )
        .unwrap();
        assert!(dry_report.dry_run);
        assert_eq!(dry_report.would_delete.len(), 2);
        assert!(orphan_arc.exists());
        assert!(ws_base.join("active-env/important.txt").exists());

        // Execute mode deletes confirmed backup-workspace orphan but never touches active env.
        let exec_report = execute_orphan_cleanup(
            &plan,
            OrphanCleanupMode::Execute,
            &envs,
            &ws_base.to_string_lossy(),
            &backups_dir.to_string_lossy(),
        )
        .unwrap();
        assert!(!exec_report.dry_run);
        assert!(!orphan_arc.exists());
        assert!(ws_base.join("active-env/important.txt").exists());
    }

    #[test]
    fn archive_scan_cache_hits_and_zstd_support() {
        let dir = TempDir::new("scan-cache");
        fs::write(
            dir.path().join("dev-20261118-100000.tar.zst"),
            b"zstd-instance",
        )
        .unwrap();
        fs::write(
            dir.path().join("dev-20261118-100000-workspace.tar.zst"),
            b"zstd-ws",
        )
        .unwrap();

        let (first, stats1) = scan_backup_archives_with_stats(&dir.path().to_string_lossy());
        assert_eq!(first.len(), 1);
        assert!(first[0].has_workspace_archive);
        assert_eq!(stats1.cache_misses, 1);
        assert_eq!(stats1.cache_hits, 0);
        assert!(dir.path().join(".archive-index.json").exists());

        let (second, stats2) = scan_backup_archives_with_stats(&dir.path().to_string_lossy());
        assert_eq!(second.len(), 1);
        assert_eq!(stats2.cache_hits, 1);
        assert_eq!(stats2.cache_misses, 0);

        let zstd_script = build_export_steps_with_options(
            "dev",
            "/tmp/b/dev.tar.zst",
            "/tmp/ws/dev",
            "/tmp/b/dev-workspace.tar.zst",
            CompressionCodec::Zstd,
            true,
        );
        assert!(zstd_script.contains("--compression zstd"));
        assert!(zstd_script.contains("zstd -T0"));
        assert!(zstd_script.contains("sha256sum"));
    }

    #[test]
    fn scheduled_backup_integrates_with_auto_snapshot_and_prunes_old() {
        let dir = TempDir::new("scheduled-backup");
        let backups_dir = dir.path().join("backups");
        let ws_dir = dir.path().join("ws/dev");
        fs::create_dir_all(&backups_dir).unwrap();
        fs::create_dir_all(&ws_dir).unwrap();

        let now = 1_794_500_000_i64;
        let policy = ScheduledBackupPolicy {
            enabled: true,
            interval_secs: 7 * 86_400,
            include_workspace: true,
            keep_count: 2,
        };

        // Disabled auto_snapshot -> None
        assert!(decide_scheduled_backup(
            "dev",
            false,
            false,
            &policy,
            &backups_dir.to_string_lossy(),
            &ws_dir.to_string_lossy(),
            now,
        )
        .is_none());

        // Enabled auto_snapshot & no prior backups -> Some(plan)
        let plan = decide_scheduled_backup(
            "dev",
            false,
            true,
            &policy,
            &backups_dir.to_string_lossy(),
            &ws_dir.to_string_lossy(),
            now,
        )
        .expect("scheduled backup plan");
        assert_eq!(plan.env_name, "dev");
        assert!(plan.script.contains("incus export"));

        // Recent backup exists -> should_create_scheduled_backup is false
        let entries = vec![
            ArchiveScanEntry {
                file_name: "dev-20261118-000000.tar.gz".to_string(),
                file_path: "/b/dev-3.tar.gz".to_string(),
                env_name: "dev".to_string(),
                size_bytes: 10,
                size_display: "10 B".to_string(),
                created_at: "2026-11-18 00:00".to_string(),
                mtime_secs: now - 3600,
                has_workspace_archive: false,
                workspace_archive_path: String::new(),
                workspace_archive_size_display: String::new(),
            },
            ArchiveScanEntry {
                file_name: "dev-20261110-000000.tar.gz".to_string(),
                file_path: "/b/dev-2.tar.gz".to_string(),
                env_name: "dev".to_string(),
                size_bytes: 10,
                size_display: "10 B".to_string(),
                created_at: "2026-11-10 00:00".to_string(),
                mtime_secs: now - 8 * 86_400,
                has_workspace_archive: false,
                workspace_archive_path: String::new(),
                workspace_archive_size_display: String::new(),
            },
            ArchiveScanEntry {
                file_name: "dev-20261101-000000.tar.gz".to_string(),
                file_path: "/b/dev-1.tar.gz".to_string(),
                env_name: "dev".to_string(),
                size_bytes: 10,
                size_display: "10 B".to_string(),
                created_at: "2026-11-01 00:00".to_string(),
                mtime_secs: now - 17 * 86_400,
                has_workspace_archive: false,
                workspace_archive_path: String::new(),
                workspace_archive_size_display: String::new(),
            },
        ];
        assert!(!should_create_scheduled_backup(
            &entries,
            "dev",
            now,
            policy.interval_secs
        ));
        let prune = scheduled_backups_to_prune(&entries, "dev", 2);
        assert_eq!(prune.len(), 1);
        assert_eq!(prune[0].0, "/b/dev-1.tar.gz");
    }
}
