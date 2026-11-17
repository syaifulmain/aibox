use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use aibox_core::{
    backup, cli, config, health, i18n, incus, network, provision, secrets, session, snapshot, task,
    theme, util, workspace,
};
use chrono::{Local, NaiveDateTime, TimeZone};
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
        "Theme::parseColors" => {
            let toml = args["toml"].as_str().expect("toml");
            let c = theme::parse_colors(toml);
            json!({
                "accent": c.accent,
                "accentForeground": theme::accent_foreground(&c.accent),
                "background": c.background,
                "darkBackground": c.dark_background,
                "lighterBackground": c.lighter_background,
                "foreground": c.foreground,
                "muted": c.muted,
                "green": c.green,
                "red": c.red,
                "yellow": c.yellow,
            })
        }
        "Theme::accentForeground" => {
            let hex = args["accent"].as_str().expect("accent");
            json!(theme::accent_foreground(hex))
        }
        "Translator::readLanguage" => {
            let work = "/tmp/aibox-golden-rust-i18n-lang";
            let _ = fs::remove_dir_all(work);
            fs::create_dir_all(work).expect("mkdir");
            let p = format!("{work}/lang");
            if let Some(content) = args["content"].as_str() {
                fs::write(&p, content).expect("write");
            }
            let out = i18n::read_language(&p);
            let _ = fs::remove_dir_all(work);
            json!(out)
        }
        "Translator::pluralCategory" => {
            let lang = args["language"].as_str().expect("language");
            let count = args["count"].as_f64().expect("count") as i64;
            json!(i18n::plural_category(lang, count))
        }
        "Translator::translate" => {
            let lang = args["language"].as_str().expect("language");
            if lang == "pt-BR" {
                let cat = i18n::Catalog::from_json_layers(&[
                    br#"{"%n clips": {"one": "%n clip", "other": "%n clips"}}"#,
                    br#"{"Save": "Guardar", "Open": "Abrir", "Empty": ""}"#,
                    br#"{"Save": "Salvar"}"#,
                ]);
                json!({
                    "Save": cat.lookup("Save", lang, -1).unwrap_or_default(),
                    "Open": cat.lookup("Open", lang, -1).unwrap_or_default(),
                    "EmptyIsNull": cat.lookup("Empty", lang, -1).is_none(),
                    "QuitIsNull": cat.lookup("Quit", lang, -1).is_none(),
                    "Clips1": cat.lookup("%n clips", lang, 1).unwrap_or_default(),
                    "Clips2": cat.lookup("%n clips", lang, 2).unwrap_or_default(),
                })
            } else {
                let cat = i18n::Catalog::from_json_layers(&[
                    br#"{"%n clips": {"one": "%n clip", "other": "%n clips"}}"#,
                    r#"{"%n clips": {"one": "%n клип", "few": "%n клипа", "many": "%n клипов", "other": "%n клипа"}}"#.as_bytes(),
                ]);
                json!({
                    "n21": cat.lookup("%n clips", lang, 21).unwrap_or_default(),
                    "n22": cat.lookup("%n clips", lang, 22).unwrap_or_default(),
                    "n25": cat.lookup("%n clips", lang, 25).unwrap_or_default(),
                    "nNeg": cat.lookup("%n clips", lang, -1).unwrap_or_default(),
                })
            }
        }
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
                .unwrap_or_default();
            let raw_json = fs::read_to_string(&store_path).unwrap_or_default();
            let (loaded, load_err) = match secrets::load_secrets_store(&store_path) {
                Ok(v) => (v, String::new()),
                Err(e) => (Vec::new(), e),
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
        "Backend::effectiveRecordSessions" => {
            let global_enabled = args["globalEnabled"].as_bool().expect("globalEnabled");
            let env_override = args["envOverride"].as_str().expect("envOverride");
            json!(session::effective_record_sessions(
                global_enabled,
                env_override
            ))
        }
        "Backend::buildSessionWrapCommand" => {
            let inner = args["inner"].as_str().expect("inner");
            let log_file = args["logFile"].as_str().expect("logFile");
            let timing_file = args["timingFile"].as_str().expect("timingFile");
            let redact_bin = args["redactBin"].as_str().expect("redactBin");
            let sessions_dir = args["sessionsDir"].as_str().expect("sessionsDir");
            let script_avail = args["scriptAvailable"].as_bool().expect("scriptAvailable");
            json!(session::build_session_wrap_command(
                inner,
                log_file,
                timing_file,
                redact_bin,
                sessions_dir,
                script_avail
            ))
        }
        "Backend::stripAnsiSequences" => {
            let raw = args["rawText"].as_str().expect("rawText");
            json!(session::strip_ansi_sequences(raw))
        }
        "Backend::formatRecordedSessionText" => {
            let raw = args["raw"].as_str().expect("raw");
            let max_bytes = args["maxBytes"].as_i64().expect("maxBytes") as i32;
            json!(session::format_recorded_session_text(
                raw.as_bytes(),
                max_bytes
            ))
        }
        "Backend::redactSessionLogFile" => {
            let work = "/tmp/aibox-golden-rust-session";
            let _ = fs::remove_dir_all(work);
            fs::create_dir_all(format!("{work}/alpha")).unwrap();
            let red_path = format!("{work}/alpha/1700000099.log");
            fs::write(&red_path, "token=sk-test-12345\nother=safe\n").unwrap();
            let secrets_vec = vec!["sk-test-12345".to_string()];
            let changed = session::redact_session_log_file(&red_path, &secrets_vec);
            let unchanged = session::redact_session_log_file(&red_path, &secrets_vec);
            let after = fs::read_to_string(&red_path).unwrap_or_default();
            let _ = fs::remove_dir_all(work);
            json!({
                "changed": changed,
                "unchanged": unchanged,
                "after": after,
            })
        }
        "Backend::pruneAndScanRecordedSessions" => {
            let work = "/tmp/aibox-golden-rust-session-prune";
            let _ = fs::remove_dir_all(work);
            fs::create_dir_all(format!("{work}/alpha")).unwrap();
            fs::create_dir_all(format!("{work}/beta")).unwrap();
            let base_secs = 1_793_491_200i64; // 2026-11-01 00:00:00 UTC
            for i in 1..=5i64 {
                let epoch_str = (1_700_000_000 + i).to_string();
                let lp = format!("{work}/alpha/{epoch_str}.log");
                let tp = format!("{work}/alpha/{epoch_str}.timing");
                fs::write(&lp, vec![b'L'; 100]).unwrap();
                let ts = (base_secs + i * 60).to_string();
                let _ = std::process::Command::new("touch")
                    .args(["-d", &format!("@{ts}"), &lp])
                    .status();
                if i % 2 == 1 {
                    fs::write(&tp, vec![b'T'; 20]).unwrap();
                }
            }
            let pruned = session::prune_recorded_sessions(work, 3, 1024 * 1024);
            let mut scanned = session::scan_recorded_sessions(work);
            // Normalize work dir prefix to match C++ golden's /tmp/aibox-golden-session
            for item in &mut scanned {
                if let Some(obj) = item.as_object_mut() {
                    for key in ["logPath", "timingPath"] {
                        if let Some(s) = obj.get(key).and_then(Value::as_str) {
                            obj.insert(
                                key.to_string(),
                                json!(s.replace(work, "/tmp/aibox-golden-session")),
                            );
                        }
                    }
                }
            }
            let _ = fs::remove_dir_all(work);
            json!({
                "pruned": pruned,
                "scanned": scanned,
            })
        }
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
        "Backend::defaultDiffExcludes" => json!(workspace::diff::default_diff_excludes()),
        "Backend::parseDiffExcludes" => {
            let raw = args["rawText"].as_str().expect("rawText");
            json!(workspace::diff::parse_diff_excludes(raw))
        }
        "Backend::buildRsyncExcludeArgs" => {
            let excludes: Vec<String> = args["excludes"]
                .as_array()
                .expect("excludes")
                .iter()
                .map(|v| v.as_str().expect("string").to_string())
                .collect();
            json!(workspace::diff::build_rsync_exclude_args(&excludes))
        }
        "Backend::buildWorkspaceDiffArgs" => {
            let snap_path = args["snapPath"].as_str().expect("snapPath");
            let ws_path = args["wsPath"].as_str().expect("wsPath");
            let excludes: Vec<String> = args["excludes"]
                .as_array()
                .expect("excludes")
                .iter()
                .map(|v| v.as_str().expect("string").to_string())
                .collect();
            json!(workspace::diff::build_workspace_diff_args(
                snap_path, ws_path, &excludes
            ))
        }
        "Backend::isSafeRelativePath" => {
            let rel = args["relativePath"].as_str().expect("relativePath");
            json!(workspace::diff::is_safe_relative_path(rel))
        }
        "Backend::isLikelyBinaryData" => {
            let hex = args["sampleHex"].as_str().expect("sampleHex");
            let bytes: Vec<u8> = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex byte"))
                .collect();
            json!(workspace::diff::is_likely_binary_data(&bytes))
        }
        "Backend::capDiffLines" => {
            let diff_output = args["diffOutput"].as_str().expect("diffOutput");
            let max_lines = args["maxLines"].as_i64().expect("maxLines") as i32;
            let (text, truncated) = workspace::diff::cap_diff_lines(diff_output, max_lines);
            json!({
                "text": text,
                "truncated": truncated,
            })
        }
        "Backend::parseRsyncItemize" => {
            let output = args["output"].as_str().expect("output");
            let max_entries = args["maxEntries"].as_i64().expect("maxEntries") as i32;
            json!(workspace::diff::parse_rsync_itemize(output, max_entries))
        }
        "Backend::processWorkspaceDiffOutput" => {
            let snap_name = args["snapshotName"].as_str().expect("snapshotName");
            let ws_path = args["wsPath"].as_str().expect("wsPath");
            let stdout = args["rsyncStdout"].as_str().expect("rsyncStdout");
            let _ = fs::remove_dir_all(ws_path);
            fs::create_dir_all(format!("{ws_path}/new dir")).unwrap();
            fs::write(format!("{ws_path}/added file.txt"), "hello world").unwrap();
            std::os::unix::fs::symlink("added file.txt", format!("{ws_path}/added_link")).unwrap();
            let res = workspace::diff::process_workspace_diff_output(snap_name, ws_path, stdout);
            let _ = fs::remove_dir_all(ws_path);
            json!(res)
        }
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
        "Backend::parseAllowlistEntries" => {
            let raw = args["rawText"].as_str().expect("rawText");
            json!(network::allowlist::parse_allowlist_entries(raw))
        }
        "Backend::resolveAllowlistCidrs" => {
            let entries: Vec<String> = args["entries"]
                .as_array()
                .expect("entries")
                .iter()
                .map(|v| v.as_str().expect("str").to_string())
                .collect();
            let resolver = network::allowlist::StaticResolver::new(BTreeMap::from([
                (
                    "github.com".to_string(),
                    vec![
                        "140.82.121.4".to_string(),
                        "192.168.1.50".to_string(),
                        "140.82.121.4".to_string(),
                    ],
                ),
                (
                    "rebind.test".to_string(),
                    vec![
                        "0.0.0.1".to_string(),
                        "10.1.2.3".to_string(),
                        "100.64.0.1".to_string(),
                        "100.127.255.254".to_string(),
                        "127.0.0.1".to_string(),
                        "169.254.169.254".to_string(),
                        "172.16.0.1".to_string(),
                        "172.31.255.254".to_string(),
                        "192.168.0.1".to_string(),
                        "224.0.0.1".to_string(),
                        "255.255.255.255".to_string(),
                        "::1".to_string(),
                        "not-an-ip".to_string(),
                    ],
                ),
                (
                    "mixed.test".to_string(),
                    vec![
                        "100.63.255.254".to_string(),
                        "172.15.255.254".to_string(),
                        "172.32.0.1".to_string(),
                    ],
                ),
            ]));
            let (cidrs, warnings) =
                network::allowlist::resolve_allowlist_cidrs(&entries, &resolver);
            json!({ "cidrs": cidrs, "warnings": warnings })
        }
        "aclApplyShellSnippet" => {
            let name = args["name"].as_str().expect("name");
            let allowlist_raw = args["allowlistRaw"].as_str().expect("allowlistRaw");
            let resolved_cidrs: Vec<String> = args["resolvedCidrs"]
                .as_array()
                .expect("resolvedCidrs")
                .iter()
                .map(|v| v.as_str().expect("str").to_string())
                .collect();
            let egress_logging = args["egressLogging"].as_bool().expect("egressLogging");
            json!(network::acl::acl_apply_shell_snippet(
                name,
                allowlist_raw,
                &resolved_cidrs,
                egress_logging,
            ))
        }
        "Backend::networkControlScript" => {
            let name = args["name"].as_str().expect("name");
            let mode = args["mode"].as_str().expect("mode");
            let allowlist_raw = args["allowlistRaw"].as_str().expect("allowlistRaw");
            let resolved_cidrs: Vec<String> = args["resolvedCidrs"]
                .as_array()
                .expect("resolvedCidrs")
                .iter()
                .map(|v| v.as_str().expect("str").to_string())
                .collect();
            let egress_logging = args["egressLogging"].as_bool().expect("egressLogging");
            json!(network::acl::network_control_script(
                name,
                mode,
                allowlist_raw,
                &resolved_cidrs,
                egress_logging,
            ))
        }
        "Backend::parseKernelEgressLogs" => {
            let journal_output = args["journalOutput"].as_str().expect("journalOutput");
            let container_ip = args["containerIp"].as_str().expect("containerIp");
            let host_iface = args["hostIface"].as_str().expect("hostIface");
            let max_entries = args["maxEntries"].as_i64().expect("maxEntries") as i32;
            json!(network::egress::parse_kernel_egress_logs(
                journal_output,
                container_ip,
                host_iface,
                max_entries,
            ))
        }
        "Backend::proxyDeviceName" => {
            let host_port = args["hostPort"].as_i64().expect("hostPort") as i32;
            json!(network::ports::proxy_device_name(host_port))
        }
        "Backend::addPortForwardScript" => {
            let name = args["name"].as_str().expect("name");
            let host_port = args["hostPort"].as_i64().expect("hostPort") as i32;
            let guest_port = args["guestPort"].as_i64().expect("guestPort") as i32;
            json!(network::ports::add_port_forward_script(
                name, host_port, guest_port
            ))
        }
        "Backend::addPortForward" => {
            let name = args["name"].as_str().expect("name");
            let host_port = args["hostPort"].as_i64().expect("hostPort") as i32;
            let guest_port = args["guestPort"].as_i64().expect("guestPort") as i32;
            let res = network::ports::validate_add_port_forward(
                name,
                false,
                host_port,
                guest_port,
                false,
                &[],
            )
            .err()
            .unwrap_or_default();
            json!(res)
        }
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
        "Backend::evaluateWatchdog" => {
            let d = health::watchdog::evaluate_watchdog(
                args["percent"].as_i64().expect("percent") as i32,
                args["previousLevel"].as_i64().expect("previousLevel") as i32,
                args["warnThreshold"].as_i64().expect("warnThreshold") as i32,
                args["criticalThreshold"]
                    .as_i64()
                    .expect("criticalThreshold") as i32,
                args["autoStopEnabled"].as_bool().expect("autoStopEnabled"),
            );
            json!({
                "newLevel": d.new_level,
                "shouldNotify": d.should_notify,
                "shouldAutoStop": d.should_auto_stop,
            })
        }
        "Backend::isValidProcessLimit" => {
            let v = args["value"].as_str().expect("value");
            json!(health::limits::is_valid_process_limit(v))
        }
        "Backend::isValidIoLimit" => {
            let v = args["value"].as_str().expect("value");
            json!(health::limits::is_valid_io_limit(v))
        }
        "Backend::parseIdleStopOverride" => {
            let raw = args["raw"].as_str().expect("raw");
            let ov = health::idle::parse_idle_stop_override(raw);
            let mode = match ov.mode {
                health::idle::IdleStopMode::Inherit => "inherit",
                health::idle::IdleStopMode::Off => "off",
                health::idle::IdleStopMode::Minutes => "minutes",
            };
            json!({
                "mode": mode,
                "minutes": ov.minutes,
            })
        }
        "Backend::effectiveIdleStopMinutes" => {
            let g = args["globalMinutes"].as_i64().expect("globalMinutes") as i32;
            let raw = args["raw"].as_str().expect("raw");
            let ov = health::idle::parse_idle_stop_override(raw);
            json!(health::idle::effective_idle_stop_minutes(g, &ov))
        }
        "Backend::evaluateIdleDecision" => {
            let samples = args["samples"]
                .as_array()
                .expect("samples")
                .iter()
                .map(|s| health::idle::IdleCpuSample {
                    timestamp_ms: s["timestampMs"].as_f64().expect("timestampMs") as i64,
                    cpu_percent: s["cpuPercent"].as_i64().expect("cpuPercent") as i32,
                })
                .collect();
            let input = health::idle::IdleDecisionInput {
                running: args["running"].as_bool().expect("running"),
                is_template: args["isTemplate"].as_bool().expect("isTemplate"),
                busy_on_this_env: args["busyOnThisEnv"].as_bool().expect("busyOnThisEnv"),
                has_active_exec_session: args["hasActiveExecSession"]
                    .as_bool()
                    .expect("hasActiveExecSession"),
                port_forward_count: args["portForwardCount"].as_i64().expect("portForwardCount")
                    as i32,
                ignore_port_forwards: args["ignorePortForwards"]
                    .as_bool()
                    .expect("ignorePortForwards"),
                threshold_minutes: args["thresholdMinutes"].as_i64().expect("thresholdMinutes")
                    as i32,
                now_ms: args["nowMs"].as_f64().expect("nowMs") as i64,
                warning_already_sent: args["warningAlreadySent"]
                    .as_bool()
                    .expect("warningAlreadySent"),
                samples,
            };
            let res = health::idle::evaluate_idle_decision(&input);
            let avg_cpu_val = if res.average_cpu_percent.fract() == 0.0 {
                json!(res.average_cpu_percent as i64)
            } else {
                json!(res.average_cpu_percent)
            };
            json!({
                "eligible": res.eligible,
                "idleDurationMs": res.idle_duration_ms,
                "averageCpuPercent": avg_cpu_val,
                "shouldWarnOneMin": res.should_warn_one_min,
                "shouldStopNow": res.should_stop_now,
                "blockedReason": res.blocked_reason,
            })
        }
        "Backend::parseIncusStorageInfo" => {
            let out = args["output"].as_str().expect("output");
            let info = health::pool::parse_incus_storage_info(out);
            pool_info_to_json_val(&info)
        }
        "Backend::sumRootDiskQuotasBytes" => {
            let j = args["incusListJson"].as_str().expect("incusListJson");
            let excl = args["excludeEnvName"].as_str().expect("excludeEnvName");
            let pool = args["poolName"].as_str().expect("poolName");
            let (bytes, unbounded) = health::pool::sum_root_disk_quotas_bytes(j, excl, pool);
            json!({
                "bytes": bytes,
                "unboundedCount": unbounded,
            })
        }
        "Backend::evaluatePoolOvercommitWarning" => {
            let pool = pool_info_from_json_val(&args["pool"]);
            let existing = args["existingQuotasBytes"]
                .as_f64()
                .expect("existingQuotasBytes") as i64;
            let requested = args["requestedBytes"].as_f64().expect("requestedBytes") as i64;
            let unbounded = args["unboundedCount"].as_i64().expect("unboundedCount") as i32;
            json!(health::pool::evaluate_pool_overcommit_warning(
                &pool, existing, requested, unbounded
            ))
        }
        "Backend::evaluatePoolCapacityHealth" => {
            let pool = pool_info_from_json_val(&args["pool"]);
            let allocated = args["allocatedQuotasBytes"]
                .as_f64()
                .expect("allocatedQuotasBytes") as i64;
            let unbounded = args["unboundedCount"].as_i64().expect("unboundedCount") as i32;
            health_result_to_json_val(&health::checks::evaluate_pool_capacity_health(
                &pool, allocated, unbounded,
            ))
        }
        "poolInfoToVariantMap" => {
            let pool = pool_info_from_json_val(&args["pool"]);
            let allocated = args["allocatedBytes"].as_f64().expect("allocatedBytes") as i64;
            let unbounded = args["unboundedCount"].as_i64().expect("unboundedCount") as i32;
            let s = pool.to_json(allocated, unbounded);
            serde_json::from_str(&s).expect("valid pool json")
        }
        "Backend::parsePoolSizeSpec" => {
            let spec = args["spec"].as_str().expect("spec");
            match health::pool::parse_pool_size_spec(spec) {
                Some(bytes) => json!({ "ok": true, "bytes": bytes }),
                None => json!({ "ok": false, "bytes": 0 }),
            }
        }
        "Backend::resizeStoragePoolScript" => {
            let pool = args["poolName"].as_str().expect("poolName");
            let spec = args["sizeSpec"].as_str().expect("sizeSpec");
            json!(health::pool::resize_storage_pool_script(pool, spec))
        }
        "Backend::createEnvironmentScript" => {
            json!(provision::create::create_environment_script(
                args["name"].as_str().expect("name"),
                args["localAlias"].as_str().expect("localAlias"),
                args["remoteImage"].as_str().expect("remoteImage"),
                args["workspace"].as_str().expect("workspace"),
                args["cacheBase"].as_str().expect("cacheBase"),
                args["profileFlags"].as_str().expect("profileFlags"),
                args["ramGiB"].as_str().expect("ramGiB"),
                args["diskGiB"].as_str().expect("diskGiB"),
                args["cpuCores"].as_i64().expect("cpuCores") as i32,
                args["x11"].as_bool().expect("x11"),
                args["processLimit"].as_str().expect("processLimit"),
                args["ioLimit"].as_str().expect("ioLimit"),
                args["isVm"].as_bool().expect("isVm"),
            ))
        }
        "Backend::cloneEnvironmentScript" => {
            let cidrs: Vec<String> = args["resolvedCidrs"]
                .as_array()
                .expect("resolvedCidrs")
                .iter()
                .map(|v| v.as_str().expect("cidr").to_string())
                .collect();
            json!(provision::clone::clone_environment_script(
                args["sourceName"].as_str().expect("sourceName"),
                args["newName"].as_str().expect("newName"),
                args["workspace"].as_str().expect("workspace"),
                args["diskGiB"].as_str().expect("diskGiB"),
                args["ramGiB"].as_str().expect("ramGiB"),
                args["cpuCores"].as_i64().expect("cpuCores") as i32,
                args["startAfter"].as_bool().expect("startAfter"),
                args["netMode"].as_str().expect("netMode"),
                args["netAllowlist"].as_str().expect("netAllowlist"),
                &cidrs,
                args["processLimit"].as_str().expect("processLimit"),
                args["ioLimit"].as_str().expect("ioLimit"),
                args["isVm"].as_bool().expect("isVm"),
            ))
        }
        "Backend::updateResourcesScript" => {
            json!(provision::resources::update_resources_script(
                args["name"].as_str().expect("name"),
                args["cpuCores"].as_i64().expect("cpuCores") as i32,
                args["ramGiB"].as_str().expect("ramGiB"),
                args["diskGiB"].as_str().expect("diskGiB"),
                args["processLimit"].as_str().expect("processLimit"),
                args["ioLimit"].as_str().expect("ioLimit"),
                args["isVm"].as_bool().expect("isVm"),
            ))
        }
        "Backend::buildPrepareScript" => {
            json!(provision::lifecycle::build_prepare_script(
                args["name"].as_str().expect("name"),
                "",
                args["allowRefresh"].as_str().expect("allowRefresh"),
            ))
        }
        "Backend::startEnvironmentScript" => {
            json!(provision::lifecycle::start_environment_script(
                args["name"].as_str().expect("name"),
                "",
                "",
            ))
        }
        "Backend::stopEnvironmentScript" => {
            json!(provision::lifecycle::stop_environment_script(
                args["name"].as_str().expect("name"),
                args["includeAutoSnapshot"]
                    .as_bool()
                    .expect("includeAutoSnapshot"),
                "",
            ))
        }
        "Backend::enterIncusExecArgs" => {
            json!(provision::lifecycle::enter_incus_exec_args(
                args["name"].as_str().expect("name"),
                args["remoteEnvPath"].as_str().expect("remoteEnvPath"),
                util::paths::CONTAINER_WORKSPACE_PATH,
                args["display"].as_str().expect("display"),
                args["term"].as_str().expect("term"),
            ))
        }
        "Backend::execIncusCommandArgs" => {
            let cmd: Vec<String> = args["cmd"]
                .as_array()
                .expect("cmd")
                .iter()
                .map(|v| v.as_str().expect("arg").to_string())
                .collect();
            json!(provision::lifecycle::exec_incus_command_args(
                args["name"].as_str().expect("name"),
                &cmd,
                args["remoteEnvPath"].as_str().expect("remoteEnvPath"),
                util::paths::CONTAINER_WORKSPACE_PATH,
            ))
        }
        "Backend::buildEnterCommand" => {
            json!(provision::lifecycle::build_enter_command(
                args["name"].as_str().expect("name"),
                args["remoteEnvPath"].as_str().expect("remoteEnvPath"),
            ))
        }
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
        "Backend::adminFixCommand" => {
            let msg = args["message"].as_str().expect("message");
            json!(task::files::admin_fix_command(msg))
        }
        "Backend::handleProcessOutput" => {
            let pending_env = args["pendingEnv"].as_str().expect("pendingEnv");
            let chunk = args["chunk"].as_str().expect("chunk");
            let mut state = task::progress::TaskProgressState {
                task_progress: 5,
                task_stage: "Init".to_string(),
                task_log: String::new(),
                has_exit_code: false,
                last_exit_code: -1,
                pending_env_name: pending_env.to_string(),
                line_buffer: String::new(),
            };
            task::progress::parse_process_output_chunk(&mut state, chunk.as_bytes(), &[]);
            json!({
                "taskProgress": state.task_progress,
                "taskStage": state.task_stage,
                "taskLog": state.task_log,
                "hasExitCode": state.has_exit_code,
                "lastExitCode": state.last_exit_code,
            })
        }
        "Backend::taskLockLifecycle" => {
            let work = std::path::Path::new("/tmp/aibox-golden-rust-task-lock");
            let _ = fs::remove_dir_all(work);
            fs::create_dir_all(work).expect("mkdir");

            let initial_pid = task::files::read_task_pid(work);
            let initial_active = task::lock::is_task_lock_active(work, false).0;

            task::files::write_task_pid(work, 99_999_999).expect("stale pid");
            let stale_pid = task::files::read_task_pid(work);
            let stale_active = task::lock::is_task_lock_active(work, false).0;

            let acquired = task::lock::acquire_cli_task_lock(work, false, "Starting 'dev'", "dev");
            let active_after_acquire = task::lock::is_task_lock_active(work, false).0;
            let second_acquire = task::lock::acquire_cli_task_lock(work, false, "Other", "dev");
            let own_pid = task::files::read_task_pid(work);
            let pid_matches_self = own_pid == i64::from(unsafe { libc::getpid() });

            let meta_raw =
                fs::read_to_string(task::files::task_meta_path(work)).unwrap_or_default();
            let mut meta_val: Value = serde_json::from_str(&meta_raw).unwrap_or(json!({}));
            let has_started_at = !meta_val["startedAt"].as_str().unwrap_or("").is_empty();
            if let Some(obj) = meta_val.as_object_mut() {
                obj.insert("startedAt".to_string(), json!("<NOW>"));
            }

            task::lock::release_cli_task_lock(work);
            let active_after_release = task::lock::is_task_lock_active(work, false).0;
            let pid_exists_after_release = task::files::task_pid_path(work).exists();
            let meta_exists_after_release = task::files::task_meta_path(work).exists();
            let _ = fs::remove_dir_all(work);

            json!({
                "initialPid": initial_pid,
                "initialActive": initial_active,
                "stalePid": stale_pid,
                "staleActive": stale_active,
                "acquired": acquired,
                "activeAfterAcquire": active_after_acquire,
                "secondAcquire": second_acquire,
                "pidMatchesSelf": pid_matches_self,
                "hasStartedAt": has_started_at,
                "meta": meta_val,
                "activeAfterRelease": active_after_release,
                "pidExistsAfterRelease": pid_exists_after_release,
                "metaExistsAfterRelease": meta_exists_after_release,
            })
        }
        "Cli::helpText" => {
            let cmd = match args["cmd"].as_str().expect("cmd") {
                "none" => cli::Command::None,
                "help" => cli::Command::Help,
                "list" => cli::Command::List,
                "check" => cli::Command::Check,
                "enter" => cli::Command::Enter,
                "exec" => cli::Command::Exec,
                "start" => cli::Command::Start,
                "stop" => cli::Command::Stop,
                "snapshot" => cli::Command::Snapshot,
                "snapshots" => cli::Command::Snapshots,
                "restore" => cli::Command::Restore,
                "diff" => cli::Command::Diff,
                other => panic!("unknown cmd {other}"),
            };
            json!(cli::help_text(cmd))
        }
        "Cli::parseArgs" => {
            let cli_args: Vec<String> = args["args"]
                .as_array()
                .expect("args")
                .iter()
                .map(|v| v.as_str().expect("str").to_string())
                .collect();
            let p = cli::parse_args(&cli_args);
            json!({
                "valid": p.valid,
                "exitCode": p.exit_code,
                "errorMessage": p.error_message,
                "command": p.command.as_str(),
                "helpTarget": p.help_target.as_str(),
                "projectDir": p.project_dir,
                "envName": p.env_name,
                "snapshotName": p.snapshot_name,
                "execCommand": p.exec_command,
                "jsonOutput": p.json_output,
                "fix": p.fix,
                "systemOnly": p.system_only,
                "workspaceOnly": p.workspace_only,
                "yes": p.yes,
            })
        }
        "Cli::isCliInvocation" => {
            let cli_args: Vec<String> = args["args"]
                .as_array()
                .expect("args")
                .iter()
                .map(|v| v.as_str().expect("str").to_string())
                .collect();
            json!(cli::is_cli_invocation(&cli_args))
        }
        "Cli::execute" => {
            use std::os::unix::fs::PermissionsExt;
            let work_dir = "/tmp/aibox-golden-cli-rust";
            let _ = fs::remove_dir_all(work_dir);
            let stub_dir = format!("{work_dir}/bin");
            let proj_dir = format!("{work_dir}/proj");
            let ws_base = format!("{proj_dir}/.aibox/workspaces");
            let state_dir = format!("{proj_dir}/.aibox/state");
            fs::create_dir_all(&stub_dir).unwrap();
            fs::create_dir_all(format!("{ws_base}/dev")).unwrap();
            fs::create_dir_all(&state_dir).unwrap();
            fs::write(
                format!("{proj_dir}/.aibox/config.json"),
                r#"{"firstRunCompleted":true,"autoSnapshot":true,"autoSnapshotKeep":5}"#,
            )
            .unwrap();
            let incus_log = format!("{work_dir}/incus.log");
            let list_json_file = format!("{work_dir}/list.json");
            let list_json = format!(
                r#"[
  {{
    "name": "dev",
    "type": "container",
    "status": "Stopped",
    "config": {{ "user.aibox.autosnap": "true", "user.aibox.wssnap": "false" }},
    "expanded_devices": {{ "workspace": {{ "source": "{ws_base}/dev", "path": "/workspace" }} }},
    "snapshots": ["snap-1"]
  }},
  {{
    "name": "tpl",
    "type": "container",
    "status": "Stopped",
    "config": {{ "user.aibox.template": "true" }},
    "expanded_devices": {{}},
    "snapshots": []
  }}
]"#
            );
            fs::write(&list_json_file, list_json).unwrap();
            let stub_path = format!("{stub_dir}/incus");
            let script = format!(
                r#"#!/bin/sh
echo "incus $*" >> "{incus_log}"
if [ "$1" = "list" ]; then
    cat "{list_json_file}"
    exit 0
fi
if [ "$1" = "info" ]; then
    echo "Status: STOPPED"
    exit 0
fi
if [ "$1" = "exec" ]; then
    while [ "$#" -gt 0 ]; do
        if [ "$1" = "--" ]; then
            shift
            break
        fi
        shift
    done
    exec "$@"
fi
if [ "$1" = "query" ]; then
    echo '[{{"name":"snap-1","size":1048576}}]'
    exit 0
fi
exit 0
"#
            );
            fs::write(&stub_path, script).unwrap();
            fs::set_permissions(&stub_path, fs::Permissions::from_mode(0o755)).unwrap();

            let cli_args: Vec<String> = args["args"]
                .as_array()
                .expect("args")
                .iter()
                .map(|v| {
                    v.as_str()
                        .expect("str")
                        .replace("/tmp/aibox-golden-cli/proj", &proj_dir)
                })
                .collect();

            struct CaptureIo {
                stdout: Vec<u8>,
                stderr: Vec<u8>,
            }
            impl cli::CliIo for CaptureIo {
                fn write_stdout(&mut self, data: &[u8]) {
                    self.stdout.extend_from_slice(data);
                }
                fn write_stderr(&mut self, data: &[u8]) {
                    self.stderr.extend_from_slice(data);
                }
                fn read_line(&mut self) -> Option<String> {
                    None
                }
                fn run_forwarded(&mut self, program: &str, args: &[String]) -> i32 {
                    std::process::Command::new(program)
                        .args(args)
                        .status()
                        .map(|s| s.code().unwrap_or(1))
                        .unwrap_or(1)
                }
                fn exec_replace(&mut self, _program: &str, _args: &[String]) -> i32 {
                    0
                }
            }

            let old_path = env::var_os("PATH");
            let new_path = match &old_path {
                Some(p) => format!("{stub_dir}:{}", p.to_string_lossy()),
                None => stub_dir.clone(),
            };
            env::set_var("PATH", new_path);

            let mut io = CaptureIo {
                stdout: Vec::new(),
                stderr: Vec::new(),
            };
            let runner = health::checks::SystemRunner;
            let rc = cli::execute_with(&cli::parse_args(&cli_args), &mut io, &runner, "/tmp");

            if let Some(p) = old_path {
                env::set_var("PATH", p);
            } else {
                env::remove_var("PATH");
            }

            let auto_re = regex::Regex::new(r"auto-\d{8}-\d{6}").unwrap();
            let last_used_re = regex::Regex::new(r"user\.aibox\.lastused \d+").unwrap();
            let stdout_str = auto_re
                .replace_all(&String::from_utf8_lossy(&io.stdout), "auto-<TS>")
                .replace(work_dir, "/tmp/aibox-golden-cli");
            let stderr_str = auto_re
                .replace_all(&String::from_utf8_lossy(&io.stderr), "auto-<TS>")
                .replace(work_dir, "/tmp/aibox-golden-cli");
            let log_raw = auto_re
                .replace_all(
                    &fs::read_to_string(&incus_log).unwrap_or_default(),
                    "auto-<TS>",
                )
                .replace(work_dir, "/tmp/aibox-golden-cli");
            let log_str = last_used_re
                .replace_all(&log_raw, "user.aibox.lastused <EPOCH>")
                .to_string();

            let _ = fs::remove_dir_all(work_dir);

            json!({
                "exitCode": rc,
                "stdout": stdout_str,
                "stderr": stderr_str,
                "incusLog": log_str,
            })
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
const ACTIVE_MODULES: &[&str] = &[
    "util",
    "config",
    "settings",
    "theme",
    "i18n",
    "secrets",
    "session",
    "snapshot",
    "diff",
    "workspace",
    "network",
    "backup",
    "health",
    "provision",
    "incus",
    "task",
    "cli",
];

fn pool_info_from_json_val(v: &Value) -> health::pool::StoragePoolInfo {
    health::pool::StoragePoolInfo {
        valid: v["valid"].as_bool().expect("valid"),
        name: v["name"].as_str().expect("name").to_string(),
        driver: v["driver"].as_str().expect("driver").to_string(),
        used_bytes: v["usedBytes"].as_f64().expect("usedBytes") as i64,
        total_bytes: v["totalBytes"].as_f64().expect("totalBytes") as i64,
        used_percent: v["usedPercent"].as_i64().expect("usedPercent") as i32,
        size_text: v["sizeText"].as_str().expect("sizeText").to_string(),
        resizable: v["resizable"].as_bool().expect("resizable"),
    }
}

fn pool_info_to_json_val(p: &health::pool::StoragePoolInfo) -> Value {
    json!({
        "valid": p.valid,
        "name": p.name,
        "driver": p.driver,
        "usedBytes": p.used_bytes,
        "totalBytes": p.total_bytes,
        "usedPercent": p.used_percent,
        "sizeText": p.size_text,
        "resizable": p.resizable,
    })
}

fn health_result_to_json_val(r: &health::checks::HealthCheckResult) -> Value {
    json!({
        "id": r.id,
        "title": r.title,
        "ok": r.ok,
        "warning": r.warning,
        "repaired": r.repaired,
        "needsRoot": r.needs_root,
        "detail": r.detail,
        "fixCommand": r.fix_command,
    })
}

struct GoldenRunner<F>(F);
impl<F: Fn(&str, &[&str]) -> health::checks::CommandOutput> health::checks::CommandRunner
    for GoldenRunner<F>
{
    fn run(&self, program: &str, args: &[&str]) -> health::checks::CommandOutput {
        (self.0)(program, args)
    }
}

fn dispatch_health_checks(
    case: &str,
    args: &Value,
    fix_state: &mut (bool, bool, bool, bool),
) -> Value {
    let fix = args["fix"].as_bool().expect("fix");
    let user = args["targetUser"].as_str().expect("targetUser");
    let ufw = args["ufwRulesContent"].as_str().expect("ufwRulesContent");
    let ws_base = args["workspaceBase"].as_str().expect("workspaceBase");

    let results = match case {
        "all_pass" => {
            let runner = GoldenRunner(|prog: &str, a: &[&str]| {
                match prog {
                "id" => health::checks::CommandOutput {
                    exit_code: 0,
                    std_out: "testuser wheel incus-admin kvm render video\n".to_string(),
                    std_err: String::new(),
                },
                "__socket_rw__" => health::checks::CommandOutput {
                    exit_code: 0,
                    ..Default::default()
                },
                "systemctl" if a.first() == Some(&"is-active") => health::checks::CommandOutput {
                    exit_code: 0,
                    std_out: "active\n".to_string(),
                    std_err: String::new(),
                },
                "incus" => match (a.first().copied(), a.get(1).copied()) {
                    (Some("list"), _) => health::checks::CommandOutput {
                        exit_code: 0,
                        std_out: "[]".to_string(),
                        std_err: String::new(),
                    },
                    (Some("network"), Some("show")) => health::checks::CommandOutput {
                        exit_code: 0,
                        std_out: "name: incusbr0\nmanaged: true\n".to_string(),
                        std_err: String::new(),
                    },
                    (Some("profile"), Some("show")) => health::checks::CommandOutput {
                        exit_code: 0,
                        std_out: "config:\n  security.nesting: \"true\"\ndevices:\n  gpu:\n    type: gpu\n  kvm:\n    path: /dev/kvm\n".to_string(),
                        std_err: String::new(),
                    },
                    (Some("storage"), Some("info")) => health::checks::CommandOutput {
                        exit_code: 0,
                        std_out: "info:\n  driver: btrfs\n  name: default\n  space used: \"1329242112\"\n  total space: \"32212254720\"\n".to_string(),
                        std_err: String::new(),
                    },
                    _ => health::checks::CommandOutput {
                        exit_code: 1,
                        std_out: String::new(),
                        std_err: "unexpected command".to_string(),
                    },
                },
                "findmnt" => health::checks::CommandOutput {
                    exit_code: 0,
                    std_out: "/home btrfs rw,relatime,user_subvol_rm_allowed\n".to_string(),
                    std_err: String::new(),
                },
                _ => health::checks::CommandOutput {
                    exit_code: 1,
                    std_out: String::new(),
                    std_err: "unexpected command".to_string(),
                },
            }
            });
            health::checks::run_health_checks(fix, &runner, user, ufw, ws_base)
        }
        "fix_dry_run" | "fix_repaired" => {
            let cell = std::cell::RefCell::new(*fix_state);
            let runner = GoldenRunner(|prog: &str, a: &[&str]| {
                let mut st = cell.borrow_mut();
                match prog {
                    "id" => health::checks::CommandOutput {
                        exit_code: 0,
                        std_out: "testuser incus-admin kvm render video\n".to_string(),
                        std_err: String::new(),
                    },
                    "__socket_rw__" => health::checks::CommandOutput {
                        exit_code: 0,
                        ..Default::default()
                    },
                    "systemctl" => health::checks::CommandOutput {
                        exit_code: 3,
                        std_out: "inactive\n".to_string(),
                        std_err: String::new(),
                    },
                    "incus" => match (a.first().copied(), a.get(1).copied()) {
                        (Some("list"), _) => health::checks::CommandOutput {
                            exit_code: 0,
                            std_out: "[]".to_string(),
                            std_err: String::new(),
                        },
                        (Some("network"), Some("show")) => {
                            if st.0 {
                                health::checks::CommandOutput {
                                    exit_code: 0,
                                    std_out: "name: incusbr0\n".to_string(),
                                    std_err: String::new(),
                                }
                            } else {
                                health::checks::CommandOutput {
                                    exit_code: 1,
                                    std_out: String::new(),
                                    std_err: "not found".to_string(),
                                }
                            }
                        }
                        (Some("network"), Some("create")) if a.get(2) == Some(&"incusbr0") => {
                            st.0 = true;
                            health::checks::CommandOutput {
                                exit_code: 0,
                                ..Default::default()
                            }
                        }
                        (Some("profile"), Some("show")) => {
                            let mut p_out = String::from("config:\n");
                            if st.1 {
                                p_out.push_str("  security.nesting: \"true\"\n");
                            }
                            p_out.push_str("devices:\n");
                            if st.2 {
                                p_out.push_str("  gpu:\n    type: gpu\n");
                            }
                            if st.3 {
                                p_out.push_str("  kvm:\n    path: /dev/kvm\n");
                            }
                            health::checks::CommandOutput {
                                exit_code: 0,
                                std_out: p_out,
                                std_err: String::new(),
                            }
                        }
                        (Some("profile"), Some("set")) => {
                            st.1 = true;
                            health::checks::CommandOutput {
                                exit_code: 0,
                                ..Default::default()
                            }
                        }
                        (Some("profile"), Some("device")) if a.get(2) == Some(&"add") => {
                            if a.get(4) == Some(&"gpu") {
                                st.2 = true;
                            }
                            if a.get(4) == Some(&"kvm") {
                                st.3 = true;
                            }
                            health::checks::CommandOutput {
                                exit_code: 0,
                                ..Default::default()
                            }
                        }
                        _ => health::checks::CommandOutput {
                            exit_code: 1,
                            std_out: String::new(),
                            std_err: "unhandled".to_string(),
                        },
                    },
                    _ => health::checks::CommandOutput {
                        exit_code: 1,
                        std_out: String::new(),
                        std_err: "unhandled".to_string(),
                    },
                }
            });
            let res = health::checks::run_health_checks(fix, &runner, user, ufw, ws_base);
            *fix_state = *cell.borrow();
            res
        }
        "root_failures" => {
            let runner = GoldenRunner(|prog: &str, a: &[&str]| match prog {
                "id" => health::checks::CommandOutput {
                    exit_code: 0,
                    std_out: "testuser wheel\n".to_string(),
                    std_err: String::new(),
                },
                "__socket_rw__" => health::checks::CommandOutput {
                    exit_code: 1,
                    std_out: String::new(),
                    std_err: "permission denied".to_string(),
                },
                "systemctl" if a.first() == Some(&"is-active") => health::checks::CommandOutput {
                    exit_code: 0,
                    std_out: "active\n".to_string(),
                    std_err: String::new(),
                },
                "findmnt" => health::checks::CommandOutput {
                    exit_code: 0,
                    std_out: "/home btrfs rw,relatime\n".to_string(),
                    std_err: String::new(),
                },
                _ => health::checks::CommandOutput {
                    exit_code: 1,
                    std_out: String::new(),
                    std_err: "daemon down".to_string(),
                },
            });
            health::checks::run_health_checks(fix, &runner, user, ufw, ws_base)
        }
        "pool_warn_90" | "pool_fail_97" => {
            let pct = if case == "pool_warn_90" { "90" } else { "97" };
            let runner = GoldenRunner(|prog: &str, a: &[&str]| {
                match prog {
                "id" => health::checks::CommandOutput {
                    exit_code: 0,
                    std_out: "testuser incus-admin kvm render video\n".to_string(),
                    std_err: String::new(),
                },
                "__socket_rw__" => health::checks::CommandOutput {
                    exit_code: 0,
                    ..Default::default()
                },
                "systemctl" => health::checks::CommandOutput {
                    exit_code: 3,
                    std_out: "inactive\n".to_string(),
                    std_err: String::new(),
                },
                "incus" => match a.first().copied() {
                    Some("list") => health::checks::CommandOutput {
                        exit_code: 0,
                        std_out: "[]".to_string(),
                        std_err: String::new(),
                    },
                    Some("network") => health::checks::CommandOutput {
                        exit_code: 0,
                        std_out: "name: incusbr0\n".to_string(),
                        std_err: String::new(),
                    },
                    Some("profile") => health::checks::CommandOutput {
                        exit_code: 0,
                        std_out: "config:\n  security.nesting: \"true\"\ndevices:\n  gpu:\n    type: gpu\n  kvm:\n    path: /dev/kvm\n".to_string(),
                        std_err: String::new(),
                    },
                    Some("storage") => health::checks::CommandOutput {
                        exit_code: 0,
                        std_out: format!("info:\n  driver: btrfs\n  name: default\n  space used: \"{pct}\"\n  total space: \"100\"\n"),
                        std_err: String::new(),
                    },
                    _ => health::checks::CommandOutput {
                        exit_code: 1,
                        ..Default::default()
                    },
                },
                "findmnt" => health::checks::CommandOutput {
                    exit_code: 0,
                    std_out: "/home btrfs rw,user_subvol_rm_allowed\n".to_string(),
                    std_err: String::new(),
                },
                _ => health::checks::CommandOutput {
                    exit_code: 1,
                    ..Default::default()
                },
            }
            });
            health::checks::run_health_checks(fix, &runner, user, ufw, ws_base)
        }
        other => panic!("unknown health check case: {other}"),
    };

    Value::Array(results.iter().map(health_result_to_json_val).collect())
}

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
    let mut health_fix_state = (false, false, false, false);
    for module in ACTIVE_MODULES {
        if *module == "backup" {
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
        }
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
            let mut actual = if *module == "settings" {
                dispatch_settings(
                    fn_name,
                    case,
                    &row["args"],
                    &mut settings_cfg,
                    &mut settings_cfg_path,
                )
            } else if *module == "health" && fn_name == "Backend::runHealthChecks" {
                dispatch_health_checks(case, &row["args"], &mut health_fix_state)
            } else {
                dispatch(fn_name, &row["args"])
            };
            if fn_name == "Backend::buildStaleReport" {
                if let Some(ts) = actual.get("generatedAt").and_then(Value::as_str) {
                    assert_eq!(ts.len(), 19, "unexpected generatedAt format: {ts}");
                }
                if let Some(obj) = actual.as_object_mut() {
                    obj.insert("generatedAt".to_string(), json!("<NOW>"));
                }
            }
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
        let _ = fs::remove_dir_all("/tmp/aibox-golden-backup-work");
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
