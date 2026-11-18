use crate::common;
use aibox_core::*;
use serde_json::{json, Value};
use std::fs;

fn dispatch(fn_name: &str, _case: &str, args: &Value) -> Value {
    match fn_name {
        "Backend::defaultSecretsStorePath" => json!(secrets::default_secrets_store_path()),
        "Backend::isSafeEnvironmentFileName" => {
            let name = args["name"].as_str().expect("name");
            json!(secrets::is_safe_environment_file_name(name))
        }
        "Backend::isValidSecretName" => {
            let name = args["name"].as_str().expect("name");
            json!(secrets::is_valid_secret_name(name))
        }
        "Backend::parseEnvSecretNames" => {
            let raw = args["rawCsv"].as_str().expect("rawCsv");
            json!(secrets::parse_env_secret_names(raw))
        }
        "Backend::buildSecretEnvFileContent" => {
            let pairs: Vec<(String, String)> = args["secrets"]
                .as_array()
                .expect("secrets")
                .iter()
                .map(|v| {
                    (
                        v["name"].as_str().expect("name").to_string(),
                        v["value"].as_str().expect("value").to_string(),
                    )
                })
                .collect();
            json!(secrets::build_secret_env_file_content(&pairs))
        }
        "Backend::redactSecrets" => {
            let text = args["text"].as_str().expect("text");
            let values: Vec<String> = args["values"]
                .as_array()
                .expect("values")
                .iter()
                .map(|v| v.as_str().expect("value").to_string())
                .collect();
            json!(secrets::redact_secrets(text, &values))
        }
        "Backend::redactSecretsBytes" => {
            let text = args["text"].as_str().expect("text");
            let values: Vec<String> = args["values"]
                .as_array()
                .expect("values")
                .iter()
                .map(|v| v.as_str().expect("value").to_string())
                .collect();
            let out = secrets::redact_secrets_bytes(text.as_bytes(), &values);
            json!(String::from_utf8(out).expect("utf8"))
        }
        "Backend::secretsStoreRoundTrip" => {
            let work = "/tmp/aibox-golden-rust-secrets";
            let _ = fs::remove_dir_all(work);
            let store_path = format!("{work}/cfg/secrets.json");
            let initial = vec![
                ("API_KEY".to_string(), "sk-test-0001".to_string()),
                ("API_KEY".to_string(), "sk-test-dup".to_string()),
                ("SECOND_KEY".to_string(), "sk-test-0002".to_string()),
            ];
            let disabled = vec!["SECOND_KEY".to_string(), "bad-name".to_string()];
            let hidden = vec!["API_KEY".to_string(), "API_KEY".to_string()];
            let save_err = secrets::save_secrets_store(&store_path, &initial, &disabled, &hidden)
                .err()
                .map(|e| e.to_string())
                .unwrap_or_default();
            let raw_json = fs::read_to_string(&store_path).unwrap_or_default();
            let (loaded, load_err) = match secrets::load_secrets_store(&store_path) {
                Ok(v) => (v, String::new()),
                Err(e) => (Vec::new(), e.to_string()),
            };
            let loaded_arr: Vec<Value> = loaded
                .into_iter()
                .map(|(n, v)| json!({ "name": n, "value": v }))
                .collect();
            let disabled_loaded = secrets::load_secret_name_list(&store_path, "disabled");
            let hidden_loaded = secrets::load_secret_name_list(&store_path, "hiddenInInstances");
            let _ = fs::remove_dir_all(work);
            json!({
                "saveErr": save_err,
                "rawJson": raw_json,
                "loadErr": load_err,
                "loaded": loaded_arr,
                "disabled": disabled_loaded,
                "hiddenInInstances": hidden_loaded,
            })
        }
        "Backend::saveSecretsStore" => {
            let work = "/tmp/aibox-golden-rust-secrets-bad";
            let _ = fs::remove_dir_all(work);
            let store_path = format!("{work}/secrets.json");
            let err = secrets::save_secrets_store(
                &store_path,
                &[("bad-name".to_string(), "x".to_string())],
                &[],
                &[],
            )
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
            let _ = fs::remove_dir_all(work);
            json!(err)
        }
        "Backend::loadSecretsStore" => {
            let work = "/tmp/aibox-golden-rust-secrets-badjson";
            let _ = fs::remove_dir_all(work);
            fs::create_dir_all(work).unwrap();
            let p = format!("{work}/bad.json");
            fs::write(&p, "{not valid").unwrap();
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
            let (count, has_err) = match secrets::load_secrets_store(&p) {
                Ok(v) => (v.len(), false),
                Err(_) => (0, true),
            };
            let _ = fs::remove_dir_all(work);
            json!({ "count": count, "hasErr": has_err })
        }
        other => panic!("unregistered golden function: {other}"),
    }
}

#[test]
fn replay() {
    let _env = common::GoldenEnvGuard::acquire();
    common::replay_jsonl("secrets", dispatch);
}
