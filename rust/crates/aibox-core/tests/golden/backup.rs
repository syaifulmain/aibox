use crate::common;
use aibox_core::*;
use serde_json::{json, Value};
use std::fs;

fn dispatch(fn_name: &str, _case: &str, args: &Value) -> Value {
    match fn_name {
        "Backend::hasSufficientDiskSpace" => {
            let req = args["requiredBytes"].as_i64().unwrap();
            let avail = args["availableBytes"].as_i64().unwrap();
            json!(backup::has_sufficient_disk_space(req, avail))
        }
        "Backend::directorySizeBytes" => {
            let dir_path = args["dirPath"].as_str().unwrap();
            json!(backup::directory_size_bytes(dir_path))
        }
        "Backend::scanBackupArchives" => {
            let backups_dir = args["backupsDir"].as_str().unwrap();
            json!(backup::scan_backup_archives(backups_dir))
        }
        "Backend::exportEnvironmentScript" => {
            json!(backup::export_environment_script(
                args["name"].as_str().unwrap(),
                args["backupFile"].as_str().unwrap(),
                args["workspacePath"].as_str().unwrap(),
                args["workspaceArchiveFile"].as_str().unwrap(),
            ))
        }
        "Backend::exportAndWipeEnvironmentScript" => {
            json!(backup::export_and_wipe_environment_script(
                args["name"].as_str().unwrap(),
                args["backupFile"].as_str().unwrap(),
                args["workspacePath"].as_str().unwrap(),
                args["workspaceArchiveFile"].as_str().unwrap(),
                args["deleteWorkspaceFiles"].as_bool().unwrap(),
                args["sessionsEnvDir"].as_str().unwrap(),
                args["environmentSecretsPath"].as_str().unwrap(),
            ))
        }
        "Backend::importEnvironmentScript" => {
            json!(backup::import_environment_script(
                args["backupFile"].as_str().unwrap(),
                args["targetName"].as_str().unwrap(),
                args["workspacePath"].as_str().unwrap(),
                args["workspaceArchiveFile"].as_str().unwrap(),
            ))
        }
        "Backend::parseActiveIncusExecEnvs" => {
            let envs: Vec<String> =
                health::idle::parse_active_incus_exec_envs(args["pgrepOutput"].as_str().unwrap())
                    .into_iter()
                    .collect();
            json!(envs)
        }
        "Backend::classifyEnvironmentStaleness" => {
            let res = backup::classify_environment_staleness(
                args["lastUsedEpoch"].as_i64().unwrap(),
                args["isTemplate"].as_bool().unwrap(),
                args["staleDays"].as_i64().unwrap() as i32,
                args["nowEpoch"].as_i64().unwrap(),
            );
            json!({
                "classification": res.classification as i32,
                "daysUnused": res.days_unused,
                "statusKey": res.status_key,
            })
        }
        "Backend::detectOrphans" => {
            let envs = args["environments"].as_array().unwrap();
            json!(backup::detect_orphans(
                envs,
                args["workspaceBase"].as_str().unwrap(),
                args["backupsDir"].as_str().unwrap(),
            ))
        }
        "Backend::buildStaleReport" => {
            let envs = args["environments"].as_array().unwrap();
            backup::build_stale_report(
                envs,
                args["workspaceBase"].as_str().unwrap(),
                args["backupsDir"].as_str().unwrap(),
                args["staleDays"].as_i64().unwrap() as i32,
                args["nowEpoch"].as_i64().unwrap(),
            )
        }
        "Backend::deleteOrphanScript" => {
            json!(backup::delete_orphan_script(
                args["kind"].as_str().unwrap(),
                args["targetPath"].as_str().unwrap(),
            ))
        }
        "Backend::adminDeleteScriptText" => {
            json!(backup::admin_delete_script_text(
                args["kind"].as_str().unwrap(),
                args["targetPath"].as_str().unwrap(),
            ))
        }
        "Backend::validateOrphanPath" => {
            let res = backup::validate_orphan_path(
                args["kind"].as_str().unwrap(),
                args["cleanPath"].as_str().unwrap(),
                args["workspaceBase"].as_str().unwrap(),
                args["backupsDir"].as_str().unwrap(),
            )
            .err()
            .unwrap_or_default();
            json!(res)
        }
        "Backend::exportEnvironment" => {
            let res = backup::validate_export_request(false, args["name"].as_str().unwrap(), false)
                .err()
                .unwrap_or_default();
            json!(res)
        }
        "Backend::importEnvironment" => {
            let res = backup::validate_import_request(
                false,
                args["newName"].as_str().unwrap(),
                false,
                args["backupFile"].as_str().unwrap(),
                args["workspaceArchiveFile"].as_str().unwrap(),
                false,
            )
            .err()
            .unwrap_or_default();
            json!(res)
        }
        "Backend::deleteOrphan" => {
            let res = backup::validate_orphan_request(
                args["kind"].as_str().unwrap(),
                args["path"].as_str().unwrap(),
                "/tmp/unused",
                "/tmp/unused/backups",
            )
            .err()
            .unwrap_or_default();
            json!(res)
        }
        other => panic!("unregistered golden function: {other}"),
    }
}

#[test]
fn replay() {
    let _env = common::GoldenEnvGuard::acquire();
    let work = "/tmp/aibox-golden-backup-work";
    let _ = fs::remove_dir_all(work);
    fs::create_dir_all(format!("{work}/tree/sub")).unwrap();
    fs::write(format!("{work}/tree/a.bin"), vec![b'a'; 123]).unwrap();
    fs::write(format!("{work}/tree/.hidden"), vec![b'h'; 45]).unwrap();
    fs::write(format!("{work}/tree/sub/b.bin"), vec![b'b'; 67]).unwrap();
    fs::write(format!("{work}/outside.bin"), vec![b'o'; 999]).unwrap();
    std::os::unix::fs::symlink(
        format!("{work}/outside.bin"),
        format!("{work}/tree/link.bin"),
    )
    .unwrap();

    let write_with_mtime = |path: &str, data: &[u8], epoch_secs: i64| {
        let f = fs::File::create(path).unwrap();
        use std::io::Write;
        (&f).write_all(data).unwrap();
        let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(epoch_secs as u64);
        f.set_modified(t).unwrap();
    };

    let scan_dir = format!("{work}/scan");
    fs::create_dir_all(&scan_dir).unwrap();
    write_with_mtime(
        &format!("{scan_dir}/alpha-20261113-120000.tar.gz"),
        b"backup-data",
        1794571200,
    );
    write_with_mtime(
        &format!("{scan_dir}/alpha-20261113-120000-workspace.tar.gz"),
        b"ws-data",
        1794571205,
    );
    write_with_mtime(
        &format!("{scan_dir}/beta-20261113-120500.tar.gz"),
        b"beta-only",
        1794571500,
    );
    write_with_mtime(&format!("{scan_dir}/ignored.txt"), b"ignore", 1794571600);

    let ws_base = format!("{work}/ws");
    let snap_base = format!("{ws_base}/.snapshots");
    let bkp_dir = format!("{work}/backups");
    fs::create_dir_all(format!("{ws_base}/keep-env")).unwrap();
    fs::create_dir_all(format!("{ws_base}/stale-env")).unwrap();
    fs::create_dir_all(format!("{ws_base}/ghost-ws")).unwrap();
    fs::create_dir_all(format!("{snap_base}/keep-env/snap1")).unwrap();
    fs::create_dir_all(format!("{snap_base}/ghost-snaps/old1")).unwrap();
    fs::create_dir_all(&bkp_dir).unwrap();
    write_with_mtime(
        &format!("{ws_base}/keep-env/app.txt"),
        b"active",
        1794500000,
    );
    write_with_mtime(
        &format!("{ws_base}/stale-env/readme.md"),
        &vec![b's'; 4096],
        1794500000,
    );
    write_with_mtime(
        &format!("{ws_base}/ghost-ws/leak.bin"),
        &vec![b'g'; 2048],
        1794500000,
    );
    write_with_mtime(
        &format!("{snap_base}/ghost-snaps/old1/data.bin"),
        &vec![b'x'; 1024],
        1794500000,
    );
    write_with_mtime(
        &format!("{bkp_dir}/keep-env-20261110-120000.tar.gz"),
        b"inst",
        1794500000,
    );
    write_with_mtime(
        &format!("{bkp_dir}/keep-env-20261110-120000-workspace.tar.gz"),
        b"ws",
        1794500000,
    );
    write_with_mtime(
        &format!("{bkp_dir}/ghost-old-20261001-010203-workspace.tar.gz"),
        &vec![b'b'; 512],
        1794500000,
    );

    common::replay_jsonl("backup", dispatch);
    let _ = fs::remove_dir_all("/tmp/aibox-golden-backup-work");
}
