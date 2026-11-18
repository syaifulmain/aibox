use std::fs;

use aibox_core::config;
use serde_json::{json, Value};

use crate::common;

fn dispatch_settings(
    fn_name: &str,
    case: &str,
    args: &Value,
    cfg: &mut config::Config,
    cfg_path: &mut String,
) -> Value {
    let work_dir = "/tmp/aibox-golden-rust-settings-work";
    match fn_name {
        "Backend::staleDayOptions" => json!(config::settings::stale_day_options()),
        "Backend::completeFirstRun" => {
            let sys_arg = args["systemPath"].as_str().expect("systemPath");
            if case == "empty_default_mode" {
                let dir = format!("{work_dir}/firstrun-empty");
                let _ = fs::create_dir_all(&dir);
                let req = config::settings::FirstRunRequest {
                    system_path: sys_arg,
                    current_system_path: &dir,
                    current_workspace_base: &dir,
                    current_os_storage_path: &format!("{dir}/os-images"),
                    current_state_dir: &format!("{dir}/state"),
                    current_config_path: &format!("{dir}/config.json"),
                    is_project_dir_mode: false,
                    remember_system_location: false,
                };
                let paths = config::settings::complete_first_run(&req).expect("ok");
                // Map back to the golden's workDir path prefix for comparison.
                let norm = |s: &str| s.replace(work_dir, "/tmp/aibox-golden-settings-work");
                json!({
                    "error": "",
                    "firstRun": false,
                    "osStoragePath": norm(&paths.os_storage_path),
                    "systemPath": norm(&paths.system_path),
                    "workspaceBase": norm(&paths.workspace_base),
                })
            } else if case == "structured_custom" {
                let dir = format!("{work_dir}/firstrun-custom");
                let target = format!("{work_dir}/firstrun-custom-target");
                let _ = fs::create_dir_all(&dir);
                let req = config::settings::FirstRunRequest {
                    system_path: &target,
                    current_system_path: &dir,
                    current_workspace_base: &format!("{dir}/workspaces"),
                    current_os_storage_path: &format!("{dir}/os-images"),
                    current_state_dir: &format!("{dir}/state"),
                    current_config_path: &format!("{dir}/config.json"),
                    is_project_dir_mode: true,
                    remember_system_location: false,
                };
                let paths = config::settings::complete_first_run(&req).expect("ok");
                let mut new_cfg = config::load(
                    &paths.config_path,
                    &paths.workspace_base,
                    &paths.os_storage_path,
                );
                new_cfg.first_run_completed = true;
                config::save(&paths.config_path, &new_cfg).expect("save");
                let saved = fs::read_to_string(&paths.config_path).unwrap_or_default();
                let norm = |s: &str| s.replace(work_dir, "/tmp/aibox-golden-settings-work");
                json!({
                    "error": "",
                    "firstRun": false,
                    "osStoragePath": norm(&paths.os_storage_path),
                    "systemPath": norm(&paths.system_path),
                    "workspaceBase": norm(&paths.workspace_base),
                    "configJson": norm(&saved),
                })
            } else {
                let dir = format!("{work_dir}/firstrun-err");
                let _ = fs::create_dir_all(&dir);
                let blocking = format!("{work_dir}/blocking-file");
                fs::write(&blocking, "x").expect("write blocking file");
                let req = config::settings::FirstRunRequest {
                    system_path: &blocking,
                    current_system_path: &dir,
                    current_workspace_base: &format!("{dir}/workspaces"),
                    current_os_storage_path: &format!("{dir}/os-images"),
                    current_state_dir: &format!("{dir}/state"),
                    current_config_path: &format!("{dir}/config.json"),
                    is_project_dir_mode: true,
                    remember_system_location: false,
                };
                let err = config::settings::complete_first_run(&req).unwrap_err();
                json!({
                    "error": err.to_string().replace(work_dir, "/tmp/aibox-golden-settings-work"),
                })
            }
        }
        "Backend::saveSettings" => {
            if case == "ok" {
                let dir = format!("{work_dir}/save-settings/aibox");
                let _ = fs::create_dir_all(&dir);
                *cfg_path = format!("{dir}/config.json");
                let ws_def = format!("{dir}/workspaces");
                let os_def = format!("{dir}/os-images");
                *cfg = config::Config::new_defaults(&ws_def, &os_def);
                let ws = format!("{work_dir}/save-settings/ws-new");
                let os = format!("{work_dir}/save-settings/os-new");
                let next =
                    config::settings::save_settings(cfg, &ws, &os, &ws_def, &os_def).expect("ok");
                *cfg = next;
                config::save(cfg_path, cfg).expect("save");
                let saved = fs::read_to_string(&*cfg_path).unwrap_or_default();
                json!({
                    "error": "",
                    "configJson": saved.replace(work_dir, "/tmp/aibox-golden-settings-work"),
                })
            } else if case == "bad_workspace" {
                let blocking = format!("{work_dir}/blocking-file");
                let os = format!("{work_dir}/save-settings/os-new");
                let err = config::settings::save_settings(cfg, &blocking, &os, "", "").unwrap_err();
                json!({
                    "error": err.to_string().replace(work_dir, "/tmp/aibox-golden-settings-work"),
                })
            } else {
                let ws = format!("{work_dir}/save-settings/ws-new");
                let blocking = format!("{work_dir}/blocking-file");
                let err = config::settings::save_settings(cfg, &ws, &blocking, "", "").unwrap_err();
                json!({
                    "error": err.to_string().replace(work_dir, "/tmp/aibox-golden-settings-work"),
                })
            }
        }
        "Backend::saveOsEntry" => {
            if case == "invalid_id" {
                let dir = format!("{work_dir}/os-catalog/aibox");
                let _ = fs::create_dir_all(&dir);
                *cfg_path = format!("{dir}/config.json");
                *cfg = config::Config::new_defaults(
                    "/tmp/aibox-golden-settings-work/os-catalog/aibox/workspaces",
                    "/tmp/aibox-golden-settings-work/os-catalog/aibox/os-images",
                );
            }
            match config::settings::save_os_entry(
                cfg,
                args["originalId"].as_str().unwrap(),
                args["id"].as_str().unwrap(),
                args["alias"].as_str().unwrap(),
                args["name"].as_str().unwrap(),
                args["badge"].as_str().unwrap(),
                args["description"].as_str().unwrap(),
            ) {
                Ok(next) => {
                    *cfg = next;
                    config::save(cfg_path, cfg).expect("save");
                    let saved = fs::read_to_string(&*cfg_path).unwrap_or_default();
                    json!({ "error": "", "configJson": saved })
                }
                Err(err) => {
                    let saved = fs::read_to_string(&*cfg_path).unwrap_or_default();
                    json!({ "error": err.to_string(), "configJson": saved })
                }
            }
        }
        "Backend::removeOsEntry" => {
            if case == "last_entry" {
                while cfg.os_catalog.len() > 1 {
                    let first_id = cfg.os_catalog[0].id.clone();
                    *cfg = config::settings::remove_os_entry(cfg, &first_id).expect("remove");
                }
            }
            match config::settings::remove_os_entry(cfg, args["id"].as_str().unwrap()) {
                Ok(next) => {
                    *cfg = next;
                    config::save(cfg_path, cfg).expect("save");
                    let saved = fs::read_to_string(&*cfg_path).unwrap_or_default();
                    json!({ "error": "", "configJson": saved })
                }
                Err(err) => json!({ "error": err.to_string() }),
            }
        }
        "Backend::resetOsCatalog" => {
            *cfg = config::settings::reset_os_catalog(cfg);
            config::save(cfg_path, cfg).expect("save");
            let saved = fs::read_to_string(&*cfg_path).unwrap_or_default();
            json!({ "error": "", "configJson": saved })
        }
        "Backend::saveCreateOptions" => {
            if case == "empty_disk" {
                let dir = format!("{work_dir}/create-opts/aibox");
                let _ = fs::create_dir_all(&dir);
                *cfg_path = format!("{dir}/config.json");
                *cfg = config::Config::new_defaults(
                    "/tmp/aibox-golden-settings-work/create-opts/aibox/workspaces",
                    "/tmp/aibox-golden-settings-work/create-opts/aibox/os-images",
                );
            }
            match config::settings::save_create_options(
                cfg,
                args["disk"].as_str().unwrap(),
                args["ram"].as_str().unwrap(),
                args["cpu"].as_str().unwrap(),
            ) {
                Ok(next) => {
                    *cfg = next;
                    config::save(cfg_path, cfg).expect("save");
                    let saved = fs::read_to_string(&*cfg_path).unwrap_or_default();
                    json!({ "error": "", "configJson": saved })
                }
                Err(err) => {
                    let saved = fs::read_to_string(&*cfg_path).unwrap_or_default();
                    json!({ "error": err.to_string(), "configJson": saved })
                }
            }
        }
        "Backend::savePreset" => {
            match config::settings::save_preset(
                cfg,
                args["id"].as_str().unwrap(),
                args["title"].as_str().unwrap(),
                args["sub"].as_str().unwrap(),
            ) {
                Ok(next) => {
                    *cfg = next;
                    config::save(cfg_path, cfg).expect("save");
                    let saved = fs::read_to_string(&*cfg_path).unwrap_or_default();
                    json!({ "error": "", "configJson": saved })
                }
                Err(err) => json!({ "error": err.to_string() }),
            }
        }
        "Backend::resetCreateOptions" => {
            *cfg = config::settings::reset_create_options(cfg);
            config::save(cfg_path, cfg).expect("save");
            let saved = fs::read_to_string(&*cfg_path).unwrap_or_default();
            json!({ "error": "", "configJson": saved })
        }
        "Backend::saveAutoSnapshotSettings" => {
            if case == "low" {
                let dir = format!("{work_dir}/misc-settings/aibox");
                let _ = fs::create_dir_all(&dir);
                *cfg_path = format!("{dir}/config.json");
                *cfg = config::Config::new_defaults(
                    "/tmp/aibox-golden-settings-work/misc-settings/aibox/workspaces",
                    "/tmp/aibox-golden-settings-work/misc-settings/aibox/os-images",
                );
            }
            match config::settings::save_auto_snapshot_settings(
                cfg,
                args["enabled"].as_bool().unwrap(),
                args["keep"].as_i64().unwrap() as i32,
            ) {
                Ok(next) => {
                    *cfg = next;
                    config::save(cfg_path, cfg).expect("save");
                    let saved = fs::read_to_string(&*cfg_path).unwrap_or_default();
                    json!({ "error": "", "configJson": saved })
                }
                Err(err) => json!({ "error": err.to_string() }),
            }
        }
        "Backend::saveWatchdogSettings" => {
            match config::settings::save_watchdog_settings(
                cfg,
                args["enabled"].as_bool().unwrap(),
                args["warn"].as_i64().unwrap() as i32,
                args["crit"].as_i64().unwrap() as i32,
                args["autoStop"].as_bool().unwrap(),
            ) {
                Ok(next) => {
                    *cfg = next;
                    config::save(cfg_path, cfg).expect("save");
                    let saved = fs::read_to_string(&*cfg_path).unwrap_or_default();
                    json!({ "error": "", "configJson": saved })
                }
                Err(err) => json!({ "error": err.to_string() }),
            }
        }
        "Backend::saveIdleStopSettings" => {
            match config::settings::save_idle_stop_settings(
                cfg,
                args["minutes"].as_i64().unwrap() as i32,
            ) {
                Ok(next) => {
                    *cfg = next;
                    config::save(cfg_path, cfg).expect("save");
                    let saved = fs::read_to_string(&*cfg_path).unwrap_or_default();
                    json!({ "error": "", "configJson": saved })
                }
                Err(err) => json!({ "error": err.to_string() }),
            }
        }
        "Backend::saveStaleSettings" => {
            match config::settings::save_stale_settings(cfg, args["days"].as_i64().unwrap() as i32)
            {
                Ok(next) => {
                    *cfg = next;
                    config::save(cfg_path, cfg).expect("save");
                    let saved = fs::read_to_string(&*cfg_path).unwrap_or_default();
                    json!({ "error": "", "configJson": saved })
                }
                Err(err) => json!({ "error": err.to_string() }),
            }
        }
        "Backend::saveSessionRecordingSettings" => {
            *cfg = config::settings::save_session_recording_settings(
                cfg,
                args["enabled"].as_bool().unwrap(),
            );
            config::save(cfg_path, cfg).expect("save");
            let saved = fs::read_to_string(&*cfg_path).unwrap_or_default();
            json!({ "error": "", "configJson": saved })
        }
        "Backend::saveDiffExcludes" => {
            *cfg = config::settings::save_diff_excludes(cfg, args["raw"].as_str().unwrap());
            config::save(cfg_path, cfg).expect("save");
            let saved = fs::read_to_string(&*cfg_path).unwrap_or_default();
            json!({ "error": "", "configJson": saved })
        }
        other => panic!("unregistered settings golden function: {other}"),
    }
}

#[test]
fn replay() {
    let _env = common::GoldenEnvGuard::acquire();
    let _ = fs::remove_dir_all("/tmp/aibox-golden-rust-settings-work");
    let mut settings_cfg = config::Config::new_defaults("", "");
    let mut settings_cfg_path = String::new();
    common::replay_jsonl("settings", |fn_name, case, args| {
        dispatch_settings(
            fn_name,
            case,
            args,
            &mut settings_cfg,
            &mut settings_cfg_path,
        )
    });
    let _ = fs::remove_dir_all("/tmp/aibox-golden-rust-settings-work");
}
