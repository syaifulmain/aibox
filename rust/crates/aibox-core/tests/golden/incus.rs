use crate::common;
use aibox_core::*;
use serde_json::{json, Value};
use std::fs;

fn dispatch(fn_name: &str, _case: &str, args: &Value) -> Value {
    match fn_name {
        "Backend::parseIncusListJson" => {
            if args.get("sample1").is_some() {
                let sample1 = args["sample1"].as_str().expect("sample1");
                let sample2 = args["sample2"].as_str().expect("sample2");
                let ws_base = args["workspaceBase"].as_str().expect("workspaceBase");
                let elapsed = args["elapsedWallNs"].as_f64().expect("elapsedWallNs") as i64;
                let mut state = incus::list::CounterState::new();
                let defaults = incus::list::ParseDefaults {
                    auto_snapshot: true,
                    auto_stop: false,
                    record_sessions: false,
                };
                let first = incus::list::parse(
                    sample1.as_bytes(),
                    ws_base,
                    &mut state,
                    elapsed,
                    defaults,
                    true,
                );
                let second = incus::list::parse(
                    sample2.as_bytes(),
                    ws_base,
                    &mut state,
                    elapsed,
                    defaults,
                    true,
                );
                json!({
                    "first": first,
                    "second": second,
                })
            } else {
                let json_str = args["json"].as_str().expect("json");
                let ws_base = args["workspaceBase"].as_str().expect("workspaceBase");
                let prev_cpu = args["prevCpuNs"].as_f64().expect("prevCpuNs") as i64;
                let elapsed = args["elapsedWallNs"].as_f64().expect("elapsedWallNs") as i64;
                let prev_rx = args["prevNetRx"].as_f64().expect("prevNetRx") as i64;
                let prev_tx = args["prevNetTx"].as_f64().expect("prevNetTx") as i64;
                let defaults = incus::list::ParseDefaults {
                    auto_snapshot: args["defaultAutoSnapshot"].as_bool().unwrap_or(true),
                    auto_stop: args["defaultAutoStop"].as_bool().unwrap_or(false),
                    record_sessions: args["defaultRecordSessions"].as_bool().unwrap_or(false),
                };
                let mut state = incus::list::CounterState::new();
                state
                    .cpu_snapshot_ns
                    .insert("box-run".to_string(), prev_cpu);
                state
                    .net_counters
                    .insert("box-run".to_string(), (prev_rx, prev_tx));
                let envs = incus::list::parse(
                    json_str.as_bytes(),
                    ws_base,
                    &mut state,
                    elapsed,
                    defaults,
                    true,
                );
                let (rx, tx) = state.net_counters.get("box-run").copied().unwrap_or((0, 0));
                json!({
                    "envs": envs,
                    "cpuSnapshotBoxRun": state.cpu_snapshot_ns.get("box-run").copied().unwrap_or(0),
                    "netSnapshotRx": rx,
                    "netSnapshotTx": tx,
                })
            }
        }
        "Backend::buildOsCatalog" => {
            let images_json = args["imagesJson"].as_str().expect("imagesJson");
            let downloading_id = args["downloadingId"].as_str().expect("downloadingId");
            json!(incus::catalog::build_os_catalog(
                images_json.as_bytes(),
                downloading_id
            ))
        }
        "Backend::catalogForEntries" => {
            let entries: Vec<config::defaults::OsEntry> = args["entries"]
                .as_array()
                .expect("entries")
                .iter()
                .map(|e| config::defaults::OsEntry {
                    id: e["id"].as_str().unwrap_or("").to_string(),
                    alias: e["alias"].as_str().unwrap_or("").to_string(),
                    name: e["name"].as_str().unwrap_or("").to_string(),
                    badge: e["badge"].as_str().unwrap_or("").to_string(),
                    description: e["description"].as_str().unwrap_or("").to_string(),
                })
                .collect();
            let images_json = args["imagesJson"].as_str().expect("imagesJson");
            let downloading_id = args["downloadingId"].as_str().expect("downloadingId");
            let cache_dir = "/tmp/aibox-golden-rust-os-cache";
            let _ = fs::remove_dir_all(cache_dir);
            fs::create_dir_all(cache_dir).unwrap();
            fs::write(format!("{cache_dir}/cached-os"), b"metadata").unwrap();
            fs::write(format!("{cache_dir}/cached-os.root"), b"rootfs").unwrap();
            let res = json!(incus::catalog::catalog_for_entries(
                &entries,
                images_json.as_bytes(),
                downloading_id,
                cache_dir
            ));
            let _ = fs::remove_dir_all(cache_dir);
            res
        }
        "Backend::loadDemoData" => {
            let record_sessions = args["recordSessions"].as_bool().unwrap_or(false);
            let now_epoch = 1_800_000_000_i64;
            let mut state = incus::demo::build_demo_state(
                record_sessions,
                "/tmp/aibox-golden-home/aibox/.os-storage",
                now_epoch,
            );
            for env in &mut state.environments {
                if let Some(obj) = env.as_object_mut() {
                    if let Some(ep) = obj.get("lastUsedEpoch").and_then(Value::as_i64) {
                        let days_ago = (((now_epoch - ep) as f64) / 86400.0).round() as i64;
                        obj.insert(
                            "lastUsedEpoch".to_string(),
                            json!(format!("<DAYS_AGO:{days_ago}>")),
                        );
                    }
                }
            }
            let template_count = state
                .environments
                .iter()
                .filter(|e| e.get("isTemplate").and_then(Value::as_bool) == Some(true))
                .count();
            json!({
                "environments": state.environments,
                "operationHistory": state.operation_history,
                "poolInfo": state.pool_info,
                "recordedSessions": state.recorded_sessions,
                "templateCount": template_count,
            })
        }
        other => panic!("unregistered golden function: {other}"),
    }
}

#[test]
fn replay() {
    common::replay_jsonl("incus", dispatch);
}
