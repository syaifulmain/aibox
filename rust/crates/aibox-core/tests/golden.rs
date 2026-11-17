use std::env;
use std::fs;
use std::path::PathBuf;

use aibox_core::{config, i18n, secrets, session, snapshot, theme, util};
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
    "util", "config", "settings", "theme", "i18n", "secrets", "session", "snapshot",
];

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
