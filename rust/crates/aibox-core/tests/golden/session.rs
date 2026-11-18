use crate::common;
use aibox_core::*;
use serde_json::{json, Value};
use std::fs;

fn dispatch(fn_name: &str, _case: &str, args: &Value) -> Value {
    match fn_name {
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
        other => panic!("unregistered golden function: {other}"),
    }
}

#[test]
fn replay() {
    let _env = common::GoldenEnvGuard::acquire();
    common::replay_jsonl("session", dispatch);
}
