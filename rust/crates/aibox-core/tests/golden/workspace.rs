use crate::common;
use aibox_core::*;
use serde_json::{json, Value};
use std::fs;

fn dispatch(fn_name: &str, _case: &str, args: &Value) -> Value {
    match fn_name {
        "Backend::fileAccessUrl" => {
            let port = args["port"].as_i64().expect("port") as i32;
            json!(workspace::file_access::file_access_url(port))
        }
        "Backend::chooseFileAccessPort" => {
            let reserved: std::collections::HashSet<i32> = args["reserved"]
                .as_array()
                .expect("reserved")
                .iter()
                .map(|v| v.as_i64().expect("int") as i32)
                .collect();
            let busy: std::collections::HashSet<i32> = args["busy"]
                .as_array()
                .expect("busy")
                .iter()
                .map(|v| v.as_i64().expect("int") as i32)
                .collect();
            let start_port = args["startPort"].as_i64().expect("startPort") as i32;
            let max_port = args["maxPort"].as_i64().expect("maxPort") as i32;
            if busy.is_empty() {
                json!(workspace::file_access::choose_file_access_port::<
                    fn(i32) -> bool,
                >(&reserved, None, start_port, max_port))
            } else {
                let mut probes = 0;
                let port = workspace::file_access::choose_file_access_port(
                    &reserved,
                    Some(|candidate: i32| {
                        probes += 1;
                        !busy.contains(&candidate)
                    }),
                    start_port,
                    max_port,
                );
                json!({ "port": port, "probes": probes })
            }
        }
        "Backend::fileAccessPaths" => {
            let base_dir = args["stateDir"].as_str().expect("stateDir");
            let state_dir = format!("{base_dir}/.state");
            let name = args["name"].as_str().expect("name");
            json!({
                "dir": workspace::file_access::file_access_dir_path(&state_dir),
                "state": workspace::file_access::file_access_state_path(&state_dir, name),
                "log": workspace::file_access::file_access_log_path(&state_dir, name),
            })
        }
        "Backend::validateExtraMountPath" => {
            let ws_work = "/tmp/aibox-golden-ws-work";
            let _ = fs::remove_dir_all(ws_work);
            fs::create_dir_all(format!("{ws_work}/datasets")).unwrap();
            fs::create_dir_all(format!("{ws_work}/home/.ssh/keys")).unwrap();
            fs::create_dir_all(format!("{ws_work}/proj/.aibox")).unwrap();
            fs::create_dir_all(format!("{ws_work}/cfg/sub")).unwrap();

            let raw_path = args["rawPath"].as_str().expect("rawPath");
            let forbidden: Vec<String> = args["forbidden"]
                .as_array()
                .expect("forbidden")
                .iter()
                .map(|v| v.as_str().expect("str").to_string())
                .collect();
            let res = workspace::mounts::validate_extra_mount_path(raw_path, &forbidden)
                .err()
                .unwrap_or_default();
            let _ = fs::remove_dir_all(ws_work);
            json!(res)
        }
        "Backend::extraMountDeviceName" => {
            let raw_name = args["rawName"].as_str().expect("rawName");
            json!(workspace::mounts::extra_mount_device_name(raw_name))
        }
        "Backend::addExtraMountScript" => {
            let name = args["name"].as_str().expect("name");
            let device_name = args["deviceName"].as_str().expect("deviceName");
            let host_path = args["hostPath"].as_str().expect("hostPath");
            let container_path = args["containerPath"].as_str().expect("containerPath");
            let read_only = args["readOnly"].as_bool().expect("readOnly");
            json!(workspace::mounts::add_extra_mount_script(
                name,
                device_name,
                host_path,
                container_path,
                read_only
            ))
        }
        "Backend::addExtraMount" => {
            let ws_work = "/tmp/aibox-golden-ws-work";
            let _ = fs::remove_dir_all(ws_work);
            fs::create_dir_all(format!("{ws_work}/datasets")).unwrap();

            let name = args["name"].as_str().expect("name");
            let host_path = args["hostPath"].as_str().expect("hostPath");
            let mount_name = args["mountName"].as_str().expect("mountName");
            let read_only = args["readOnly"].as_bool().expect("readOnly");
            let confirm_rw = args["confirmReadWrite"]
                .as_bool()
                .expect("confirmReadWrite");
            let res = workspace::mounts::prepare_add_extra_mount(
                name,
                false,
                host_path,
                mount_name,
                read_only,
                confirm_rw,
                &[],
                false,
                &[],
            )
            .err()
            .unwrap_or_default();
            let _ = fs::remove_dir_all(ws_work);
            json!(res)
        }
        other => panic!("unregistered golden function: {other}"),
    }
}

#[test]
fn replay() {
    common::replay_jsonl("workspace", dispatch);
}
