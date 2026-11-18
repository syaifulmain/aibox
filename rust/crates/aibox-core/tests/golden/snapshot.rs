use crate::common;
use aibox_core::*;
use chrono::{Local, NaiveDateTime, TimeZone};
use serde_json::{json, Value};

fn dispatch(fn_name: &str, _case: &str, args: &Value) -> Value {
    match fn_name {
        "Backend::isAutoSnapshotName" => {
            let name = args["snapshotName"].as_str().expect("snapshotName");
            json!(snapshot::is_auto_snapshot_name(name))
        }
        "Backend::parseAutoSnapshotTimestamp" => {
            let name = args["snapshotName"].as_str().expect("snapshotName");
            let dt = snapshot::parse_auto_snapshot_timestamp(name);
            let iso = snapshot::parse_auto_snapshot_timestamp_iso(name);
            json!({
                "valid": dt.is_some(),
                "iso": iso,
            })
        }
        "Backend::shouldCreateAutoSnapshot" => {
            let snaps: Vec<String> = args["snapshots"]
                .as_array()
                .expect("snapshots")
                .iter()
                .map(|v| v.as_str().expect("str").to_string())
                .collect();
            let now_iso = args["nowIso"].as_str().expect("nowIso");
            let ndt = NaiveDateTime::parse_from_str(now_iso, "%Y-%m-%dT%H:%M:%S").expect("iso");
            let now_dt = Local
                .from_local_datetime(&ndt)
                .earliest()
                .expect("local dt");
            let min_interval = args["minIntervalSeconds"]
                .as_i64()
                .expect("minIntervalSeconds");
            json!(snapshot::should_create_auto_snapshot(
                &snaps,
                now_dt.timestamp(),
                min_interval
            ))
        }
        "Backend::autoSnapshotsToPrune" => {
            let snaps: Vec<String> = args["snapshots"]
                .as_array()
                .expect("snapshots")
                .iter()
                .map(|v| v.as_str().expect("str").to_string())
                .collect();
            let keep = args["keepCount"].as_i64().expect("keepCount") as i32;
            json!(snapshot::auto_snapshots_to_prune(&snaps, keep))
        }
        "Backend::workspaceSnapshotDir" => {
            let ws = args["workspacePath"].as_str().expect("workspacePath");
            let env_name = args["envName"].as_str().expect("envName");
            json!(snapshot::workspace_snapshot_dir(ws, env_name))
        }
        "Backend::findOrphanWorkspaceSnapshots" => {
            let incus: Vec<String> = args["incusSnapshots"]
                .as_array()
                .expect("incusSnapshots")
                .iter()
                .map(|v| v.as_str().expect("str").to_string())
                .collect();
            let ws: Vec<String> = args["wsSnapshots"]
                .as_array()
                .expect("wsSnapshots")
                .iter()
                .map(|v| v.as_str().expect("str").to_string())
                .collect();
            json!(snapshot::find_orphan_workspace_snapshots(&incus, &ws))
        }
        "Backend::parseSnapshotSizesJson" => {
            let raw = args["json"].as_str().expect("json");
            json!(snapshot::parse_snapshot_sizes_json(raw.as_bytes()))
        }
        "Backend::parseDuBytesOutput" => {
            let output = args["output"].as_str().expect("output");
            json!(snapshot::parse_du_bytes_output(output))
        }
        "Backend::workspaceCreateSnippet" => {
            let ws = args["workspacePath"].as_str().expect("workspacePath");
            json!(snapshot::workspace_create_snippet(ws))
        }
        "Backend::workspaceMountMigrationScript" => {
            let env_name = args["envName"].as_str().expect("envName");
            json!(snapshot::workspace_mount_migration_script(env_name))
        }
        "Backend::workspaceSnapshotSnippet" => {
            let ws = args["workspacePath"].as_str().expect("workspacePath");
            let snap_dir = args["snapDir"].as_str().expect("snapDir");
            let snap_name = args["snapName"].as_str().expect("snapName");
            json!(snapshot::workspace_snapshot_snippet(
                ws, snap_dir, snap_name
            ))
        }
        "Backend::workspaceDeleteSnapshotSnippet" => {
            let snap_path = args["snapPath"].as_str().expect("snapPath");
            let warn_only = args["warnOnly"].as_bool().expect("warnOnly");
            json!(snapshot::workspace_delete_snapshot_snippet(
                snap_path, warn_only
            ))
        }
        "Backend::workspaceRestoreSnippet" => {
            let ws = args["workspacePath"].as_str().expect("workspacePath");
            let snap_dir = args["snapDir"].as_str().expect("snapDir");
            let snap_name = args["snapName"].as_str().expect("snapName");
            let epoch = args["safetyEpoch"].as_f64().expect("safetyEpoch") as i64;
            json!(snapshot::workspace_restore_snippet(
                ws, snap_dir, snap_name, epoch
            ))
        }
        "Backend::convertWorkspaceToSubvolumeScript" => {
            let env_name = args["envName"].as_str().expect("envName");
            let ws = args["workspacePath"].as_str().expect("workspacePath");
            json!(snapshot::convert_workspace_to_subvolume_script(
                env_name, ws
            ))
        }
        "Backend::autoSnapshotShellSnippet" => {
            let name = args["name"].as_str().expect("name");
            let new_snap = args["newSnapName"].as_str().expect("newSnapName");
            let prune: Vec<String> = args["pruneNames"]
                .as_array()
                .expect("pruneNames")
                .iter()
                .map(|v| v.as_str().expect("str").to_string())
                .collect();
            let ws = args["workspacePath"].as_str().expect("workspacePath");
            let include_ws = args["includeWorkspace"]
                .as_bool()
                .expect("includeWorkspace");
            json!(snapshot::auto_snapshot_shell_snippet(
                name, new_snap, &prune, ws, include_ws
            ))
        }
        "Backend::createSnapshotScript" => {
            let name = args["name"].as_str().expect("name");
            let snap_name = args["snapName"].as_str().expect("snapName");
            let ws = args["workspacePath"].as_str().expect("workspacePath");
            let include_ws = args["includeWorkspace"]
                .as_bool()
                .expect("includeWorkspace");
            json!(snapshot::create_snapshot_script(
                name, snap_name, ws, include_ws
            ))
        }
        "Backend::restoreSnapshotScript" => {
            let name = args["name"].as_str().expect("name");
            let snap_name = args["snapName"].as_str().expect("snapName");
            let ws = args["workspacePath"].as_str().expect("workspacePath");
            let mode = args["mode"].as_str().expect("mode");
            let include_ws = args["includeWorkspace"]
                .as_bool()
                .expect("includeWorkspace");
            let epoch = args["safetyEpoch"].as_f64().expect("safetyEpoch") as i64;
            json!(snapshot::restore_snapshot_script(
                name, snap_name, ws, mode, include_ws, epoch
            ))
        }
        "Backend::deleteSnapshotScript" => {
            let name = args["name"].as_str().expect("name");
            let snap_name = args["snapName"].as_str().expect("snapName");
            let ws = args["workspacePath"].as_str().expect("workspacePath");
            let has_incus = args["hasIncusSnap"].as_bool().expect("hasIncusSnap");
            let has_ws = args["hasWorkspaceSnap"]
                .as_bool()
                .expect("hasWorkspaceSnap");
            json!(snapshot::delete_snapshot_script(
                name, snap_name, ws, has_incus, has_ws
            ))
        }
        "Backend::wipeEnvironmentScript" => {
            let name = args["name"].as_str().expect("name");
            let ws = args["workspacePath"].as_str().expect("workspacePath");
            let del_ws = args["deleteWorkspaceFiles"]
                .as_bool()
                .expect("deleteWorkspaceFiles");
            let sessions_dir = args["sessionsEnvDir"].as_str().expect("sessionsEnvDir");
            let secrets_path = args["environmentSecretsPath"]
                .as_str()
                .expect("environmentSecretsPath");
            json!(snapshot::wipe_environment_script(
                name,
                ws,
                del_ws,
                sessions_dir,
                secrets_path
            ))
        }
        other => panic!("unregistered golden function: {other}"),
    }
}

#[test]
fn replay() {
    let _env = common::GoldenEnvGuard::acquire();
    common::replay_jsonl("snapshot", dispatch);
}
