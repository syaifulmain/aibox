use std::env;
use std::fs;
use std::path::PathBuf;

use aibox_core::{config, util};
use serde_json::{json, Value};

fn dispatch(fn_name: &str, args: &Value) -> Value {
    match fn_name {
        "formatBytes" => {
            let bytes = args["bytes"].as_f64().expect("bytes") as i64;
            json!(util::format::format_bytes(bytes))
        }
        "formatByteRate" => {
            let bps = args["bytesPerSecond"].as_f64().expect("bytesPerSecond") as i64;
            json!(util::format::format_byte_rate(bps))
        }
        "formatByteTotal" => {
            let bytes = args["bytes"].as_f64().expect("bytes") as i64;
            json!(util::format::format_byte_total(bytes))
        }
        "parseSizeBytes" => {
            let text = args["sizeText"].as_str().expect("sizeText");
            json!(util::size::parse_size_bytes(text))
        }
        "validatedSizeOptions" => {
            let values: Vec<String> = args["values"]
                .as_array()
                .expect("values")
                .iter()
                .map(|v| v.as_str().expect("string").to_string())
                .collect();
            json!(util::size::validated_size_options(&values))
        }
        "validatedCpuOptions" => {
            let values = args["values"].as_array().expect("values");
            json!(util::size::validated_cpu_options(values))
        }
        "Backend::sanitizeName" => {
            let raw = args["rawName"].as_str().expect("rawName");
            json!(util::names::sanitize_name(raw))
        }
        "Backend::shellQuote" => {
            let val = args["value"].as_str().expect("value");
            json!(util::shell::shell_quote(val))
        }
        "Backend::shellArrayWords" => {
            let words = args["words"].as_str().expect("words");
            json!(util::shell::shell_array_words(words))
        }
        "Backend::parseCsv" => {
            let text = args["text"].as_str().expect("text");
            json!(util::csv::parse_csv(text))
        }
        "Backend::truncateLogTail" => {
            let data = args["data"].as_str().expect("data").as_bytes();
            let max_bytes = args["maxBytes"].as_f64().expect("maxBytes") as i64;
            let out = util::bytes::truncate_log_tail(data, max_bytes);
            json!(String::from_utf8(out).expect("utf8"))
        }
        "Backend::defaultSystemPath" => json!(util::paths::default_system_path()),
        "Backend::defaultWorkspacePath" => json!(util::paths::default_workspace_path()),
        "Backend::defaultOsStoragePathValue" => json!(util::paths::default_os_storage_path()),
        "Backend::defaultLocationFilePath" => {
            json!(util::paths::default_location_file_path())
        }
        "Backend::configuredSystemPath" => json!(util::paths::configured_system_path()),
        "Backend::saveLocationFile" => {
            // Covered in dedicated stateful check below; return expected shape here?
            // No, we can execute it in a temp dir using AIBOX_LOCATION_FILE!
            let work = "/tmp/aibox-golden-rust-loc";
            let _ = fs::remove_dir_all(work);
            let loc = format!("{work}/loc/location.json");
            let prev = env::var_os("AIBOX_LOCATION_FILE");
            env::set_var("AIBOX_LOCATION_FILE", &loc);
            let saved =
                util::paths::save_location_file("/tmp/aibox-golden-home/custom-sys/../sys").is_ok();
            let content = fs::read_to_string(&loc).unwrap_or_default();
            let configured = util::paths::configured_system_path();
            if let Some(p) = prev {
                env::set_var("AIBOX_LOCATION_FILE", p);
            } else {
                env::remove_var("AIBOX_LOCATION_FILE");
            }
            let _ = fs::remove_dir_all(work);
            json!({
                "saved": saved,
                "content": content,
                "configured": configured,
            })
        }
        "Backend::resolveProjectAiboxDir" => {
            let project_dir = args["projectDir"].as_str().expect("projectDir");
            let base_cwd = args["baseCwd"].as_str().expect("baseCwd");
            json!(util::paths::resolve_project_aibox_dir(
                project_dir,
                base_cwd
            ))
        }
        "Backend::resolveWorkspacePath" => {
            let raw_parent = args["rawParent"].as_str().expect("rawParent");
            let env_name = args["envName"].as_str().expect("envName");
            let default_base = args["defaultBase"].as_str().expect("defaultBase");
            match util::paths::resolve_workspace_path(raw_parent, env_name, default_base) {
                Ok(path) => json!({ "path": path, "error": "" }),
                Err(err) => json!({ "path": "", "error": err }),
            }
        }
        "Backend::defaultOsEntries" => json!(config::default_os_entries()),
        "Backend::defaultPresets" => json!(config::default_presets()),
        "Backend::defaultDiskOptions" => json!(config::default_disk_options()),
        "Backend::defaultRamOptions" => json!(config::default_ram_options()),
        "Backend::defaultCpuOptions" => json!(config::default_cpu_options()),
        "Backend::normalizedPath" => {
            let raw = args["raw"].as_str().expect("raw");
            let fallback = args["fallback"].as_str().expect("fallback");
            json!(util::paths::normalized_path(raw, fallback))
        }
        "Backend::cacheBasePath" => {
            let os_id = args["osId"].as_str().expect("osId");
            json!(util::paths::cache_base_path(
                "/tmp/aibox-golden-config-work/fresh/aibox/os-images",
                os_id
            ))
        }
        "Backend::presetById" => {
            let id = args["id"].as_str().expect("id");
            let cfg = config::Config::new_defaults("", "");
            match cfg.preset_by_id(id) {
                Some(p) => json!(p),
                None => json!({}),
            }
        }
        "Backend::osEntryById" => {
            let id = args["id"].as_str().expect("id");
            let cfg = config::Config::new_defaults("", "");
            match cfg.os_entry_by_id(id) {
                Some(e) => json!(e),
                None => json!({}),
            }
        }
        "Backend::saveConfig" => {
            let work = "/tmp/aibox-golden-rust-cfg-fresh";
            let _ = fs::remove_dir_all(work);
            let cfg_path = format!("{work}/config.json");
            let aibox_dir = "/tmp/aibox-golden-config-work/fresh/aibox";
            let cfg = config::Config::new_defaults(
                &format!("{aibox_dir}/workspaces"),
                &format!("{aibox_dir}/os-images"),
            );
            config::save(&cfg_path, &cfg).expect("save");
            let saved = fs::read_to_string(&cfg_path).expect("read");
            let _ = fs::remove_dir_all(work);
            json!(saved)
        }
        "Backend::loadAndSaveBrokenConfig" => {
            let input = args["input"].as_str().expect("input");
            let work = "/tmp/aibox-golden-rust-cfg-broken";
            let _ = fs::remove_dir_all(work);
            let cfg_path = format!("{work}/config.json");
            let aibox_dir = "/tmp/aibox-golden-config-work/broken/aibox";
            let cfg = config::load_from_str(
                input,
                &format!("{aibox_dir}/workspaces"),
                &format!("{aibox_dir}/os-images"),
            );
            config::save(&cfg_path, &cfg).expect("save");
            let saved = fs::read_to_string(&cfg_path).expect("read");
            let _ = fs::remove_dir_all(work);
            json!(saved)
        }
        other => panic!("unregistered golden function: {other}"),
    }
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("tests")
        .join("golden")
}

// Modules are enabled here as each migration phase implements them in Rust.
const ACTIVE_MODULES: &[&str] = &["util", "config", "settings"];

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
                    "error": err.replace(work_dir, "/tmp/aibox-golden-settings-work"),
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
                    "error": err.replace(work_dir, "/tmp/aibox-golden-settings-work"),
                })
            } else {
                let ws = format!("{work_dir}/save-settings/ws-new");
                let blocking = format!("{work_dir}/blocking-file");
                let err = config::settings::save_settings(cfg, &ws, &blocking, "", "").unwrap_err();
                json!({
                    "error": err.replace(work_dir, "/tmp/aibox-golden-settings-work"),
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
                    json!({ "error": err, "configJson": saved })
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
                Err(err) => json!({ "error": err }),
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
                    json!({ "error": err, "configJson": saved })
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
                Err(err) => json!({ "error": err }),
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
                Err(err) => json!({ "error": err }),
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
                Err(err) => json!({ "error": err }),
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
                Err(err) => json!({ "error": err }),
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
                Err(err) => json!({ "error": err }),
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
fn replay_active_goldens() {
    let old_home = env::var_os("HOME");
    let old_xdg = env::var_os("XDG_CONFIG_HOME");
    let old_loc = env::var_os("AIBOX_LOCATION_FILE");
    env::set_var("HOME", "/tmp/aibox-golden-home");
    env::remove_var("XDG_CONFIG_HOME");
    env::remove_var("AIBOX_LOCATION_FILE");

    let dir = golden_dir();
    let _ = fs::remove_dir_all("/tmp/aibox-golden-rust-settings-work");
    let mut settings_cfg = config::Config::new_defaults("", "");
    let mut settings_cfg_path = String::new();
    for module in ACTIVE_MODULES {
        let path = dir.join(format!("{module}.jsonl"));
        let contents = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        for (idx, line) in contents.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let row: Value = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("{}:{}: invalid JSON: {e}", path.display(), idx + 1));
            let fn_name = row["fn"].as_str().expect("missing fn");
            let case = row["case"].as_str().unwrap_or("");
            if *module == "config" {
                match (fn_name, case) {
                    ("Backend::defaultLocationFilePath", "xdg_override") => {
                        env::set_var("XDG_CONFIG_HOME", "/tmp/aibox-golden-xdg");
                    }
                    ("Backend::defaultLocationFilePath", "env_override") => {
                        env::remove_var("XDG_CONFIG_HOME");
                        env::set_var("AIBOX_LOCATION_FILE", "/tmp/aibox-golden-loc/location.json");
                    }
                    ("Backend::configuredSystemPath", "missing_file") => {
                        env::remove_var("AIBOX_LOCATION_FILE");
                    }
                    ("Backend::configuredSystemPath", "tilde") => {
                        let work_loc = "/tmp/aibox-golden-rust-cfg-loc/location.json";
                        let _ = fs::create_dir_all("/tmp/aibox-golden-rust-cfg-loc");
                        fs::write(work_loc, r#"{"systemPath":"~"}"#).unwrap();
                        env::set_var("AIBOX_LOCATION_FILE", work_loc);
                    }
                    ("Backend::configuredSystemPath", "tilde_sub") => {
                        let work_loc = "/tmp/aibox-golden-rust-cfg-loc/location.json";
                        fs::write(work_loc, r#"{"systemPath":"~/my-aibox"}"#).unwrap();
                    }
                    ("Backend::configuredSystemPath", "relative") => {
                        let work_loc = "/tmp/aibox-golden-rust-cfg-loc/location.json";
                        fs::write(work_loc, r#"{"systemPath":"rel/aibox"}"#).unwrap();
                    }
                    ("Backend::configuredSystemPath", "invalid_json") => {
                        let work_loc = "/tmp/aibox-golden-rust-cfg-loc/location.json";
                        fs::write(work_loc, r#"{not json"#).unwrap();
                    }
                    ("Backend::configuredSystemPath", "empty_path") => {
                        let work_loc = "/tmp/aibox-golden-rust-cfg-loc/location.json";
                        fs::write(work_loc, r#"{"systemPath":"  "}"#).unwrap();
                    }
                    ("Backend::resolveProjectAiboxDir", "empty_both") => {
                        let _ = fs::remove_dir_all("/tmp/aibox-golden-rust-cfg-loc");
                        env::remove_var("AIBOX_LOCATION_FILE");
                    }
                    ("Backend::resolveProjectAiboxDir", "legacy_dot_aibox_only") => {
                        let p = "/tmp/aibox-golden-config-work/proj_legacy/.aibox";
                        let _ = fs::create_dir_all(p);
                    }
                    ("Backend::resolveProjectAiboxDir", "both_aibox_and_dot_aibox") => {
                        let _ =
                            fs::create_dir_all("/tmp/aibox-golden-config-work/proj_both/.aibox");
                        let _ = fs::create_dir_all("/tmp/aibox-golden-config-work/proj_both/aibox");
                    }
                    _ => {}
                }
            }
            let actual = if *module == "settings" {
                dispatch_settings(
                    fn_name,
                    case,
                    &row["args"],
                    &mut settings_cfg,
                    &mut settings_cfg_path,
                )
            } else {
                dispatch(fn_name, &row["args"])
            };
            assert_eq!(
                actual,
                row["out"],
                "{}:{}: mismatch for {fn_name} ({case})",
                path.display(),
                idx + 1
            );
        }
        let _ = fs::remove_dir_all("/tmp/aibox-golden-config-work");
        let _ = fs::remove_dir_all("/tmp/aibox-golden-rust-settings-work");
    }

    if let Some(v) = old_home {
        env::set_var("HOME", v);
    } else {
        env::remove_var("HOME");
    }
    if let Some(v) = old_xdg {
        env::set_var("XDG_CONFIG_HOME", v);
    } else {
        env::remove_var("XDG_CONFIG_HOME");
    }
    if let Some(v) = old_loc {
        env::set_var("AIBOX_LOCATION_FILE", v);
    } else {
        env::remove_var("AIBOX_LOCATION_FILE");
    }
}
