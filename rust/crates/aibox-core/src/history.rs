//! Operation history persistence, pruning, redaction, and unread-failure tracking.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use serde_json::{json, Value};

use crate::secrets::redact::SecretRedactor;
use crate::util::bytes::truncate_log_tail;
use crate::util::json::to_json_compact;
use crate::util::paths::atomic_write;

pub(crate) const DEFAULT_LOG_TAIL_BYTES: i64 = 204_800;

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

fn entry_id(val: &Value) -> &str {
    val.get("id").and_then(Value::as_str).unwrap_or("")
}

fn is_valid_entry(val: &Value) -> bool {
    val.is_object() && !entry_id(val).is_empty()
}

fn parse_jsonl_bytes(bytes: &[u8], out: &mut Vec<Value>) {
    for raw_line in bytes.split(|&b| b == b'\n') {
        let line = trim_ascii_whitespace(raw_line);
        if line.is_empty() {
            continue;
        }
        if let Ok(val) = serde_json::from_slice::<Value>(line) {
            if is_valid_entry(&val) {
                out.push(val);
            }
        }
    }
}

fn trim_ascii_whitespace(mut slice: &[u8]) -> &[u8] {
    while let Some((&first, rest)) = slice.split_first() {
        if first.is_ascii_whitespace() {
            slice = rest;
        } else {
            break;
        }
    }
    while let Some((&last, rest)) = slice.split_last() {
        if last.is_ascii_whitespace() {
            slice = rest;
        } else {
            break;
        }
    }
    slice
}

enum EntrySource {
    Jsonl(PathBuf),
    LegacyJson(PathBuf),
}

struct LoadedEntry {
    value: Value,
    source: EntrySource,
}

fn load_entries_with_sources(dir_path: &str) -> Vec<LoadedEntry> {
    let Ok(read_dir) = fs::read_dir(dir_path) else {
        return Vec::new();
    };

    let mut jsonl_files: Vec<PathBuf> = Vec::new();
    let mut json_files: Vec<PathBuf> = Vec::new();
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if name.ends_with(".jsonl") {
            jsonl_files.push(path);
        } else if name.ends_with(".json") {
            json_files.push(path);
        }
    }

    let mut loaded = Vec::new();

    for path in jsonl_files {
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let mut parsed = Vec::new();
        parse_jsonl_bytes(&bytes, &mut parsed);
        for value in parsed {
            loaded.push(LoadedEntry {
                value,
                source: EntrySource::Jsonl(path.clone()),
            });
        }
    }

    for path in json_files {
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        if is_valid_entry(&value) {
            loaded.push(LoadedEntry {
                value,
                source: EntrySource::LegacyJson(path),
            });
        }
    }

    // Reverse lexicographical order on IDs matches newest-first ordering.
    loaded.sort_by(|a, b| entry_id(&b.value).cmp(entry_id(&a.value)));
    loaded
}

/// Loads all valid history entries (`*.jsonl` and legacy `*.json`) from `<state_dir>/history`, newest first.
pub fn load_operation_history(state_dir: &str) -> Vec<Value> {
    let dir_path = history_dir_path(state_dir);
    load_entries_with_sources(&dir_path)
        .into_iter()
        .map(|item| item.value)
        .collect()
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

fn parse_entry_datetime(entry: &Value) -> Option<DateTime<Local>> {
    if let Some(started) = entry.get("startedAt").and_then(Value::as_str) {
        if let Some(dt) = parse_iso_datetime(started) {
            return Some(dt);
        }
    }
    if let Some(ts) = entry.get("timestamp").and_then(Value::as_str) {
        if let Ok(naive) = NaiveDateTime::parse_from_str(ts.trim(), "%Y-%m-%d %H:%M:%S") {
            if let Some(dt) = Local.from_local_datetime(&naive).earliest() {
                return Some(dt);
            }
        }
    }
    let id = entry_id(entry);
    if let Some(prefix) = id.get(..15) {
        if let Ok(naive) = NaiveDateTime::parse_from_str(prefix, "%Y%m%d-%H%M%S") {
            if let Some(dt) = Local.from_local_datetime(&naive).earliest() {
                return Some(dt);
            }
        }
    }
    None
}

fn safe_history_log_file(dir_path: &str, id: &str) -> Option<PathBuf> {
    if id.is_empty() || id.contains('/') || id.contains('\\') || id == "." || id == ".." {
        return None;
    }
    let file_name = Path::new(id).file_name()?.to_str()?;
    if file_name != id {
        return None;
    }
    Some(Path::new(dir_path).join(format!("{file_name}.log")))
}

fn remove_pruned_entries(
    dir_path: &str,
    entries: Vec<LoadedEntry>,
    remove_indices: &HashSet<usize>,
    extra_dirty_jsonl: Option<&Path>,
) -> Vec<Value> {
    if remove_indices.is_empty() && extra_dirty_jsonl.is_none() {
        return entries.into_iter().map(|e| e.value).collect();
    }

    let mut affected_jsonl: BTreeMap<PathBuf, Vec<Value>> = BTreeMap::new();
    let mut dirty_jsonl: HashSet<PathBuf> = HashSet::new();
    if let Some(p) = extra_dirty_jsonl {
        dirty_jsonl.insert(p.to_path_buf());
    }
    let mut kept = Vec::with_capacity(entries.len().saturating_sub(remove_indices.len()));

    for (idx, item) in entries.into_iter().enumerate() {
        if remove_indices.contains(&idx) {
            let id = entry_id(&item.value);
            if let Some(log_path) = safe_history_log_file(dir_path, id) {
                let _ = fs::remove_file(log_path);
            }
            match item.source {
                EntrySource::LegacyJson(path) => {
                    let _ = fs::remove_file(path);
                }
                EntrySource::Jsonl(path) => {
                    dirty_jsonl.insert(path.clone());
                    affected_jsonl.entry(path).or_default();
                }
            }
        } else {
            if let EntrySource::Jsonl(ref path) = item.source {
                affected_jsonl
                    .entry(path.clone())
                    .or_default()
                    .push(item.value.clone());
            }
            kept.push(item.value);
        }
    }

    // Rewrite only the monthly JSONL files that actually had entries removed.
    for (path, mut remaining) in affected_jsonl {
        if !dirty_jsonl.contains(&path) {
            continue;
        }
        if remaining.is_empty() {
            let _ = fs::remove_file(&path);
            continue;
        }
        // Store chronologically (oldest first) inside each monthly file.
        remaining.sort_by(|a, b| entry_id(a).cmp(entry_id(b)));
        let mut buf = String::new();
        for val in &remaining {
            if let Ok(line) = to_json_compact(val) {
                buf.push_str(&line);
                buf.push('\n');
            }
        }
        let _ = atomic_write(&path, buf.as_bytes());
    }

    kept
}

/// Prunes history entries by maximum count and/or maximum age in days, returning remaining entries.
pub fn prune_operation_history(
    state_dir: &str,
    max_entries: Option<usize>,
    max_age_days: Option<i64>,
    now: DateTime<Local>,
) -> Vec<Value> {
    let dir_path = history_dir_path(state_dir);
    let entries = load_entries_with_sources(&dir_path);
    if entries.is_empty() {
        return Vec::new();
    }

    let mut remove_indices = HashSet::new();

    if let Some(days) = max_age_days {
        let max_secs = days.max(0).saturating_mul(86_400);
        for (idx, item) in entries.iter().enumerate() {
            if let Some(dt) = parse_entry_datetime(&item.value) {
                if (now - dt).num_seconds() > max_secs {
                    remove_indices.insert(idx);
                }
            }
        }
    }

    if let Some(limit) = max_entries {
        // `entries` is sorted newest-first, so keep the first `limit` non-age-pruned items.
        let mut kept_count = 0usize;
        for (idx, _) in entries.iter().enumerate() {
            if remove_indices.contains(&idx) {
                continue;
            }
            if kept_count < limit {
                kept_count += 1;
            } else {
                remove_indices.insert(idx);
            }
        }
    }

    remove_pruned_entries(&dir_path, entries, &remove_indices, None)
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

/// Records an operation history entry (`.log` + monthly `.jsonl`), prunes old entries, and returns the updated list.
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

    let redactor = SecretRedactor::new(params.secret_values);
    let safe_description = redactor.redact_str(params.description);
    let safe_log_bytes = redactor.redact_bytes(params.log_bytes);
    let truncated_log = truncate_log_tail(&safe_log_bytes, DEFAULT_LOG_TAIL_BYTES);

    let log_path = Path::new(&dir_path).join(&log_name);
    let _ = atomic_write(&log_path, &truncated_log);

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
        let safe_reason = redactor.redact_str(params.reason);
        entry.insert("reason".to_string(), json!(safe_reason));
    }

    let month_name = format!("{}.jsonl", now.format("%Y-%m"));
    let jsonl_path = Path::new(&dir_path).join(month_name);

    let mut entries = load_entries_with_sources(&dir_path);
    entries.push(LoadedEntry {
        value: Value::Object(entry),
        source: EntrySource::Jsonl(jsonl_path.clone()),
    });
    entries.sort_by(|a, b| entry_id(&b.value).cmp(entry_id(&a.value)));

    let mut remove_indices = HashSet::new();
    if params.max_entries >= 0 {
        let limit = params.max_entries as usize;
        for idx in limit..entries.len() {
            remove_indices.insert(idx);
        }
    }

    Some(remove_pruned_entries(
        &dir_path,
        entries,
        &remove_indices,
        Some(&jsonl_path),
    ))
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

/// Writes `newest_id` to `<state_dir>/history-seen` atomically if non-empty and changed.
pub fn acknowledge_operation_history(
    state_dir: &str,
    newest_id: &str,
    current_seen_id: &str,
) -> Option<String> {
    if newest_id.is_empty() || newest_id == current_seen_id {
        return None;
    }
    let marker_path = history_seen_marker_path(state_dir);
    if atomic_write(Path::new(&marker_path), newest_id.as_bytes()).is_ok() {
        Some(newest_id.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

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

        // Verify monthly JSONL format is used and no legacy .json or .tmp files exist.
        let hist_dir = history_dir_path(state_dir);
        let files: Vec<String> = fs::read_dir(&hist_dir)
            .unwrap()
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(String::from))
            .collect();
        assert!(files.iter().any(|f| f.ends_with(".jsonl")));
        assert!(!files.iter().any(|f| f.ends_with(".json")));
        assert!(!files.iter().any(|f| f.contains(".tmp")));
    }

    #[test]
    fn corrupted_json_and_jsonl_entries_are_skipped() {
        let dir = TempDir::new("corrupt");
        let state_dir = dir.str_path();
        let hist_dir = history_dir_path(state_dir);
        fs::create_dir_all(&hist_dir).unwrap();

        // Legacy corrupted .json files
        fs::write(format!("{hist_dir}/bad.json"), b"{not valid json").unwrap();
        fs::write(format!("{hist_dir}/no-id.json"), br#"{"status":"failed"}"#).unwrap();
        fs::write(format!("{hist_dir}/array.json"), b"[1, 2, 3]").unwrap();

        // Monthly .jsonl with mixed valid, invalid, blank, and missing-id lines
        let jsonl = concat!(
            "\n",
            "{corrupt line\n",
            "[\"not-an-object\"]\n",
            "{\"id\":\"\",\"status\":\"failed\"}\n",
            "{\"id\":\"20261110-100000-001\",\"description\":\"Valid 1\",\"status\":\"success\"}\n",
            "{\"id\":\"20261111-100000-001\",\"description\":\"Valid 2\",\"status\":\"failed\"}\n",
        );
        fs::write(format!("{hist_dir}/2026-11.jsonl"), jsonl).unwrap();

        let loaded = load_operation_history(state_dir);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0]["id"], "20261111-100000-001");
        assert_eq!(loaded[1]["id"], "20261110-100000-001");
    }

    #[test]
    fn prune_by_count_and_age_removes_entries_and_logs() {
        let dir = TempDir::new("prune-age");
        let state_dir = dir.str_path();
        let hist_dir = history_dir_path(state_dir);
        fs::create_dir_all(&hist_dir).unwrap();

        let now = Local::now();
        let old_dt = now - Duration::days(40);
        let mid_dt = now - Duration::days(10);
        let new_dt = now - Duration::days(1);

        let old_id = format!("{}-001", old_dt.format("%Y%m%d-%H%M%S-%3f"));
        let mid_id = format!("{}-002", mid_dt.format("%Y%m%d-%H%M%S-%3f"));
        let new_id = format!("{}-003", new_dt.format("%Y%m%d-%H%M%S-%3f"));

        for (id, dt) in [(&old_id, old_dt), (&mid_id, mid_dt), (&new_id, new_dt)] {
            fs::write(format!("{hist_dir}/{id}.log"), format!("log for {id}")).unwrap();
            let line = to_json_compact(&json!({
                "id": id,
                "startedAt": dt.to_rfc3339(),
                "description": format!("Op {id}"),
                "status": "success"
            }))
            .unwrap();
            let month_file = format!("{hist_dir}/{}.jsonl", dt.format("%Y-%m"));
            let mut existing = fs::read(&month_file).unwrap_or_default();
            if !existing.is_empty() && !existing.ends_with(b"\n") {
                existing.push(b'\n');
            }
            existing.extend_from_slice(line.as_bytes());
            existing.push(b'\n');
            fs::write(&month_file, existing).unwrap();
        }

        // Age-based pruning: drop entries older than 30 days.
        let after_age = prune_operation_history(state_dir, None, Some(30), now);
        assert_eq!(after_age.len(), 2);
        assert!(read_operation_log(state_dir, &old_id, false).is_none());
        assert!(read_operation_log(state_dir, &mid_id, false).is_some());
        assert!(read_operation_log(state_dir, &new_id, false).is_some());

        // Count-based pruning: keep at most 1 entry.
        let after_count = prune_operation_history(state_dir, Some(1), None, now);
        assert_eq!(after_count.len(), 1);
        assert_eq!(after_count[0]["id"], new_id);
        assert!(read_operation_log(state_dir, &mid_id, false).is_none());
        assert!(read_operation_log(state_dir, &new_id, false).is_some());

        // Count-based pruning to 0 clears all entries.
        let after_zero = prune_operation_history(state_dir, Some(0), None, now);
        assert!(after_zero.is_empty());
        assert!(read_operation_log(state_dir, &new_id, false).is_none());
    }

    #[test]
    fn unread_failed_history_count_filters_by_status_and_seen_id() {
        let entries = vec![
            json!({"id": "20261118-120000-003", "status": "failed"}),
            json!({"id": "20261118-110000-002", "status": "warning"}),
            json!({"id": "20261118-100000-001", "status": "failed"}),
            json!({"id": "20261118-090000-000", "status": "success"}),
        ];

        assert_eq!(recent_failed_history_count(&entries, ""), 2);
        assert_eq!(
            recent_failed_history_count(&entries, "20261118-100000-001"),
            1
        );
        assert_eq!(
            recent_failed_history_count(&entries, "20261118-120000-003"),
            0
        );
    }

    #[test]
    fn list_and_count_1000_entries_under_20ms() {
        let dir = TempDir::new("perf-1000");
        let state_dir = dir.str_path();
        let hist_dir = history_dir_path(state_dir);
        fs::create_dir_all(&hist_dir).unwrap();

        let mut buf = String::with_capacity(1000 * 180);
        for i in 0..1000 {
            let status = if i % 5 == 0 { "failed" } else { "success" };
            let id = format!("20261118-120000-{i:04}");
            let line = format!(
                r#"{{"id":"{id}","timestamp":"2026-11-18 12:00:00","description":"Operation {i}","envName":"box1","status":"{status}","exitCode":0,"durationMs":100,"logFile":"{id}.log"}}"#
            );
            buf.push_str(&line);
            buf.push('\n');
        }
        atomic_write(&Path::new(&hist_dir).join("2026-11.jsonl"), buf.as_bytes()).unwrap();

        let start = Instant::now();
        let loaded = load_operation_history(state_dir);
        let unread = recent_failed_history_count(&loaded, "20261118-120000-0500");
        let elapsed = start.elapsed();

        assert_eq!(loaded.len(), 1000);
        assert_eq!(unread, 99);
        assert!(
            elapsed.as_millis() < 20,
            "1000-entry list + count took {elapsed:?}, expected < 20 ms"
        );
    }
}
