use crate::common;
use aibox_core::*;
use serde_json::{json, Value};
use std::env;
use std::fs;

fn dispatch(fn_name: &str, case: &str, args: &Value) -> Value {
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
            let _ = fs::create_dir_all("/tmp/aibox-golden-config-work/proj_both/.aibox");
            let _ = fs::create_dir_all("/tmp/aibox-golden-config-work/proj_both/aibox");
        }
        _ => {}
    }
    match fn_name {
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

#[test]
fn replay() {
    let _env = common::GoldenEnvGuard::acquire();
    common::replay_jsonl("config", dispatch);
    let _ = fs::remove_dir_all("/tmp/aibox-golden-config-work");
}
