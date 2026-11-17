//! Operation history persistence, pruning, redaction, and unread-failure tracking.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use serde_json::{json, Value};

use crate::secrets::redact::{redact_secrets, redact_secrets_bytes};
use crate::util::bytes::truncate_log_tail;
use crate::util::json::to_json_compact;

const DEFAULT_LOG_TAIL_BYTES: i64 = 204_800;

static HISTORY_SEQ: AtomicU32 = AtomicU32::new(0);

/// Returns `<state_dir>/history`.
pub fn history_dir_path(state_dir: &str) -> String {
    format!("{state_dir}/history")
}

/// Returns `<state_dir>/history-seen`.
pub fn history_seen_marker_path(state_dir: &str) -> String {
    format!("{state_dir}/history-seen")
}

/// Reads the last acknowledged history entry ID from `<state_dir>/history-seen`.
pub fn load_history_seen_marker(state_dir: &str) -> String {
    match fs::read(history_seen_marker_path(state_dir)) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).trim().to_string(),
        Err(_) => String::new(),
    }
}

/// Counts failed entries in `entries` whose `id` is newer than `history_seen_id`.
pub fn recent_failed_history_count(entries: &[Value], history_seen_id: &str) -> i32 {
    let mut count = 0i32;
    for entry in entries {
        let Some(obj) = entry.as_object() else {
            continue;
        };
        if obj.get("status").and_then(Value::as_str) != Some("failed") {
            continue;
        }
        let id = obj.get("id").and_then(Value::as_str).unwrap_or("");
        if !history_seen_id.is_empty() && id <= history_seen_id {
            continue;
        }
        count += 1;
    }
    count
}

/// JSON-string wrapper around `recent_failed_history_count` for FFI callers.
pub fn recent_failed_history_count_json(entries_json: &str, history_seen_id: &str) -> i32 {
    let entries: Vec<Value> = serde_json::from_str(entries_json).unwrap_or_default();
    recent_failed_history_count(&entries, history_seen_id)
}

/// Loads all valid `*.json` history entries from `<state_dir>/history`, newest first.
pub fn load_operation_history(state_dir: &str) -> Vec<Value> {
    let dir_path = history_dir_path(state_dir);
    let Ok(read_dir) = fs::read_dir(&dir_path) else {
        return Vec::new();
    };

    let mut json_files: Vec<(String, PathBuf)> = Vec::new();
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if name.ends_with(".json") {
            json_files.push((name.to_string(), path));
        }
    }

    // Reverse lexicographical order matches QDir::Name | QDir::Reversed.
    json_files.sort_by(|a, b| b.0.cmp(&a.0));

    let mut loaded = Vec::new();
    for (_, path) in json_files {
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let Ok(val) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        let Some(obj) = val.as_object() else {
            continue;
        };
        let id = obj.get("id").and_then(Value::as_str).unwrap_or("");
        if id.is_empty() {
            continue;
        }
        loaded.push(val);
    }
    loaded
}

fn parse_iso_datetime(started_at_iso: &str) -> Option<DateTime<Local>> {
    let trimmed = started_at_iso.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        return Some(dt.with_timezone(&Local));
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S"] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(trimmed, fmt) {
            if let Some(dt) = Local.from_local_datetime(&naive).earliest() {
                return Some(dt);
            }
        }
    }
    None
}

/// Parameters for recording a completed or failed operation in history.
pub struct RecordOperationParams<'a> {
    pub state_dir: &'a str,
    pub max_entries: i32,
    pub description: &'a str,
    pub env_name: &'a str,
    pub status: &'a str,
    pub exit_code: i32,
    pub started_at_iso: &'a str,
    pub log_bytes: &'a [u8],
    pub reason: &'a str,
    pub secret_values: &'a [String],
}

/// Records an operation history entry (`.log` + `.json`), prunes old entries, and returns the updated list.
pub fn record_operation_history(params: &RecordOperationParams<'_>) -> Option<Vec<Value>> {
    if params.description.is_empty() {
        return None;
    }
    let dir_path = history_dir_path(params.state_dir);
    if fs::create_dir_all(&dir_path).is_err() {
        return None;
    }

    let now = Local::now();
    let duration_ms = match parse_iso_datetime(params.started_at_iso) {
        Some(started) => (now - started).num_milliseconds().max(0),
        None => 0,
    };

    let seq = HISTORY_SEQ.fetch_add(1, Ordering::Relaxed) + 1;
    let id = format!("{}-{seq:03}", now.format("%Y%m%d-%H%M%S-%3f"));
    let log_name = format!("{id}.log");

    let safe_description = redact_secrets(params.description, params.secret_values);
    let safe_log_bytes = redact_secrets_bytes(params.log_bytes, params.secret_values);
    let truncated_log = truncate_log_tail(&safe_log_bytes, DEFAULT_LOG_TAIL_BYTES);

    let log_path = Path::new(&dir_path).join(&log_name);
    let _ = fs::write(&log_path, &truncated_log);

    let mut entry = serde_json::Map::new();
    entry.insert("id".to_string(), json!(id));
    entry.insert(
        "timestamp".to_string(),
        json!(now.format("%Y-%m-%d %H:%M:%S").to_string()),
    );
    entry.insert("startedAt".to_string(), json!(params.started_at_iso));
    entry.insert("durationMs".to_string(), json!(duration_ms));
    entry.insert("description".to_string(), json!(safe_description));
    entry.insert("envName".to_string(), json!(params.env_name));
    entry.insert("status".to_string(), json!(params.status));
    entry.insert("exitCode".to_string(), json!(params.exit_code));
    entry.insert("logFile".to_string(), json!(log_name));
    if !params.reason.is_empty() {
        entry.insert("reason".to_string(), json!(params.reason));
    }

    if let Ok(compact) = to_json_compact(&Value::Object(entry)) {
        let meta_path = Path::new(&dir_path).join(format!("{id}.json"));
        let _ = fs::write(&meta_path, compact.as_bytes());
    }

    // Prune oldest entries beyond max_entries.
    if params.max_entries >= 0 {
        if let Ok(read_dir) = fs::read_dir(&dir_path) {
            let mut json_names: Vec<String> = read_dir
                .flatten()
                .filter_map(|e| {
                    let p = e.path();
                    if !p.is_file() {
                        return None;
                    }
                    let name = p.file_name()?.to_str()?;
                    if name.ends_with(".json") {
                        Some(name.to_string())
                    } else {
                        None
                    }
                })
                .collect();
            json_names.sort();
            let max_entries = params.max_entries as usize;
            if json_names.len() > max_entries {
                let remove_count = json_names.len() - max_entries;
                for name in json_names.into_iter().take(remove_count) {
                    let base = name.trim_end_matches(".json");
                    let _ = fs::remove_file(Path::new(&dir_path).join(format!("{base}.json")));
                    let _ = fs::remove_file(Path::new(&dir_path).join(format!("{base}.log")));
                }
            }
        }
    }

    Some(load_operation_history(params.state_dir))
}

/// Reads an operation log by entry ID, returning `None` when the file is unavailable.
pub fn read_operation_log(state_dir: &str, id: &str, demo_mode: bool) -> Option<String> {
    let safe_id = Path::new(id)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if safe_id.is_empty() {
        return Some(String::new());
    }
    if demo_mode {
        return Some(format!(
            "[demo] Task {safe_id} completed cleanly.\n@@STAGE:100:Done."
        ));
    }
    let path = Path::new(&history_dir_path(state_dir)).join(format!("{safe_id}.log"));
    let bytes = fs::read(&path).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Removes all files in `<state_dir>/history` and `<state_dir>/history-seen`.
pub fn clear_operation_history(state_dir: &str) {
    let dir_path = history_dir_path(state_dir);
    if let Ok(read_dir) = fs::read_dir(&dir_path) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_file() {
                let _ = fs::remove_file(&path);
            }
        }
    }
    let _ = fs::remove_file(history_seen_marker_path(state_dir));
}

/// Writes `newest_id` to `<state_dir>/history-seen` if non-empty and changed.
pub fn acknowledge_operation_history(
    state_dir: &str,
    newest_id: &str,
    current_seen_id: &str,
) -> Option<String> {
    if newest_id.is_empty() || newest_id == current_seen_id {
        return None;
    }
    if fs::write(history_seen_marker_path(state_dir), newest_id.as_bytes()).is_ok() {
        Some(newest_id.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("aibox-history-{prefix}-{nanos}"));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }

        fn str_path(&self) -> &str {
            self.path.to_str().expect("utf8 path")
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn history_load_acknowledge_reopen_and_clear_match_qt_test() {
        let dir = TempDir::new("persist");
        let state_dir = dir.str_path();
        let hist_dir = history_dir_path(state_dir);
        fs::create_dir_all(&hist_dir).unwrap();

        fs::write(
            format!("{hist_dir}/20261113-130005-001.log"),
            "build failed on step 2\n",
        )
        .unwrap();
        fs::write(
            format!("{hist_dir}/20261113-130005-001.json"),
            r#"{"id":"20261113-130005-001","description":"Create box","envName":"box1","status":"failed","exitCode":1,"timestamp":"2026-11-13 13:00:05","durationMs":5000,"logFile":"20261113-130005-001.log"}"#,
        )
        .unwrap();
        fs::write(
            format!("{hist_dir}/20261113-125501-001.log"),
            "snapshot ok\n",
        )
        .unwrap();
        fs::write(
            format!("{hist_dir}/20261113-125501-001.json"),
            r#"{"id":"20261113-125501-001","description":"Snapshot box","envName":"box1","status":"success","exitCode":0,"timestamp":"2026-11-13 12:55:01","durationMs":1000,"logFile":"20261113-125501-001.log"}"#,
        )
        .unwrap();

        let loaded = load_operation_history(state_dir);
        assert_eq!(loaded.len(), 2);
        let mut seen = load_history_seen_marker(state_dir);
        assert_eq!(recent_failed_history_count(&loaded, &seen), 1);
        assert_eq!(
            read_operation_log(state_dir, "20261113-130005-001", false).unwrap(),
            "build failed on step 2\n"
        );

        let newest_id = loaded[0]["id"].as_str().unwrap();
        seen = acknowledge_operation_history(state_dir, newest_id, &seen).unwrap();
        assert_eq!(recent_failed_history_count(&loaded, &seen), 0);
        assert_eq!(loaded.len(), 2);

        // Reopen: seen marker survives.
        let reloaded = load_operation_history(state_dir);
        let reloaded_seen = load_history_seen_marker(state_dir);
        assert_eq!(recent_failed_history_count(&reloaded, &reloaded_seen), 0);
        assert_eq!(reloaded.len(), 2);

        // Newer failure raises counter again.
        fs::write(
            format!("{hist_dir}/20261113-140000-001.json"),
            r#"{"id":"20261113-140000-001","description":"Start box","envName":"box1","status":"failed","exitCode":1,"timestamp":"2026-11-13 14:00:00","durationMs":1200,"logFile":"20261113-140000-001.log"}"#,
        )
        .unwrap();
        let reopened = load_operation_history(state_dir);
        let reopened_seen = load_history_seen_marker(state_dir);
        assert_eq!(reopened.len(), 3);
        assert_eq!(recent_failed_history_count(&reopened, &reopened_seen), 1);

        clear_operation_history(state_dir);
        assert_eq!(load_operation_history(state_dir).len(), 0);
        assert_eq!(load_history_seen_marker(state_dir), "");
    }

    #[test]
    fn record_operation_history_redacts_secrets_and_prunes() {
        let dir = TempDir::new("redact-prune");
        let state_dir = dir.str_path();
        let secrets = vec!["tok-999-secret".to_string()];

        for i in 1..=3 {
            let desc = format!("Exec #{i} tok-999-secret");
            let log = format!("output tok-999-secret line {i}\n");
            let res = record_operation_history(&RecordOperationParams {
                state_dir,
                max_entries: 2,
                description: &desc,
                env_name: "secbox",
                status: "success",
                exit_code: 0,
                started_at_iso: "",
                log_bytes: log.as_bytes(),
                reason: "",
                secret_values: &secrets,
            })
            .unwrap();
            assert!(res.len() <= 2);
        }

        let entries = load_operation_history(state_dir);
        assert_eq!(entries.len(), 2);
        for entry in &entries {
            let desc = entry["description"].as_str().unwrap();
            assert!(!desc.contains("tok-999-secret"));
            assert!(desc.contains("[REDACTED]"));
            let id = entry["id"].as_str().unwrap();
            let log_text = read_operation_log(state_dir, id, false).unwrap();
            assert!(!log_text.contains("tok-999-secret"));
            assert!(log_text.contains("[REDACTED]"));
        }
    }
}
