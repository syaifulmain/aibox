use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::OnceLock;

use chrono::{Local, NaiveDateTime};
use regex::Regex;

use crate::history::{record_operation_history, RecordOperationParams};
use crate::secrets::redact::{redact_secrets, redact_secrets_bytes};
use crate::task::files::{
    clear_task_files, read_task_meta, read_task_pid, task_log_path, write_task_result, TaskResult,
};
use crate::task::lock::{is_task_lock_active, send_signal_group};
use crate::util::paths::atomic_write;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskProgressState {
    pub task_progress: i32,
    pub task_stage: String,
    pub task_log: String,
    pub has_exit_code: bool,
    pub last_exit_code: i32,
    pub pending_env_name: String,
    pub line_buffer: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TaskOutputDelta {
    pub progress_changed: bool,
}

fn strip_csi_ansi(text: &str) -> String {
    static ANSI_RE: OnceLock<Regex> = OnceLock::new();
    let re = ANSI_RE
        .get_or_init(|| Regex::new(r"\x1b\[[0-9;?]*[a-zA-Z]").expect("valid ANSI CSI regex"));
    re.replace_all(text, "").into_owned()
}

fn process_line(state: &mut TaskProgressState, raw_line: &str) -> bool {
    static STAGE_RE: OnceLock<Regex> = OnceLock::new();
    static EXIT_RE: OnceLock<Regex> = OnceLock::new();
    static PCT_RE: OnceLock<Regex> = OnceLock::new();

    let line = raw_line.trim();
    if line.is_empty() {
        return false;
    }

    let exit_re = EXIT_RE.get_or_init(|| Regex::new(r"^@@EXIT:(\d+)$").expect("valid exit regex"));
    if let Some(caps) = exit_re.captures(line) {
        state.has_exit_code = true;
        state.last_exit_code = caps
            .get(1)
            .and_then(|m| m.as_str().parse::<i32>().ok())
            .unwrap_or(0);
        return false;
    }

    let stage_re =
        STAGE_RE.get_or_init(|| Regex::new(r"^@@STAGE:(\d+):(.*)$").expect("valid stage regex"));
    if let Some(caps) = stage_re.captures(line) {
        let pct = caps
            .get(1)
            .and_then(|m| m.as_str().parse::<i32>().ok())
            .unwrap_or(0);
        state.task_progress = pct.clamp(0, 100);
        state.task_stage = caps
            .get(2)
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();
        return true;
    }

    let pct_re = PCT_RE.get_or_init(|| Regex::new(r"(\d{1,3})%").expect("valid pct regex"));
    if let Some(caps) = pct_re.captures(line) {
        let sub_pct = caps
            .get(1)
            .and_then(|m| m.as_str().parse::<i32>().ok())
            .unwrap_or(0)
            .clamp(0, 100);
        let lower = line.to_ascii_lowercase();
        if lower.contains("retrieving image") || lower.contains("copying") {
            if !state.pending_env_name.is_empty() {
                state.task_progress = 10 + (sub_pct * 55) / 100;
            } else {
                state.task_progress = sub_pct;
            }
        } else if lower.contains("unpacking") {
            if !state.pending_env_name.is_empty() {
                state.task_progress = 65 + (sub_pct * 15) / 100;
            } else {
                state.task_progress = state.task_progress.max(sub_pct);
            }
        }
    }

    state.task_log = line.to_string();
    true
}

/// Parses a single chunk of process output, matching `Backend::handleProcessOutput`.
pub fn parse_process_output_chunk(
    state: &mut TaskProgressState,
    chunk: &[u8],
    secret_values: &[String],
) -> TaskOutputDelta {
    let raw = String::from_utf8_lossy(chunk);
    let redacted = redact_secrets(&raw, secret_values);
    let clean = strip_csi_ansi(&redacted);

    let mut progress_changed = false;
    for part in clean.split(['\r', '\n']) {
        if process_line(state, part) {
            progress_changed = true;
        }
    }
    TaskOutputDelta { progress_changed }
}

/// Streaming variant that buffers incomplete lines across arbitrary chunk boundaries
/// (including 1-byte chunks). Call `flush_process_output_streaming` at EOF.
pub fn feed_process_output_streaming(
    state: &mut TaskProgressState,
    chunk: &[u8],
    secret_values: &[String],
) -> TaskOutputDelta {
    let raw = String::from_utf8_lossy(chunk);
    let mut line_buf = std::mem::take(&mut state.line_buffer);
    line_buf.push_str(&raw);

    let mut progress_changed = false;
    let mut cursor = 0usize;
    let bytes = line_buf.as_bytes();
    while let Some(rel_pos) = bytes[cursor..]
        .iter()
        .position(|&b| b == b'\r' || b == b'\n')
    {
        let end = cursor + rel_pos;
        let raw_line = &line_buf[cursor..end];
        cursor = end + 1;
        let redacted = redact_secrets(raw_line, secret_values);
        let clean = strip_csi_ansi(&redacted);
        if process_line(state, &clean) {
            progress_changed = true;
        }
    }
    if cursor > 0 {
        line_buf.drain(..cursor);
    }
    state.line_buffer = line_buf;
    TaskOutputDelta { progress_changed }
}

pub fn flush_process_output_streaming(
    state: &mut TaskProgressState,
    secret_values: &[String],
) -> TaskOutputDelta {
    if state.line_buffer.is_empty() {
        return TaskOutputDelta {
            progress_changed: false,
        };
    }
    let rem = std::mem::take(&mut state.line_buffer);
    let redacted = redact_secrets(&rem, secret_values);
    let clean = strip_csi_ansi(&redacted);
    let progress_changed = process_line(state, &clean);
    TaskOutputDelta { progress_changed }
}

fn redact_log_file_in_place(log_path: &Path, raw_log: &[u8], secret_values: &[String]) {
    if secret_values.is_empty() || raw_log.is_empty() {
        return;
    }
    let scrubbed = redact_secrets_bytes(raw_log, secret_values);
    if scrubbed != raw_log {
        let _ = atomic_write(log_path, &scrubbed);
    }
}

fn compute_duration_ms(started_at_iso: &str) -> u64 {
    let Ok(started) = NaiveDateTime::parse_from_str(started_at_iso, "%Y-%m-%dT%H:%M:%S") else {
        return 0;
    };
    let elapsed = Local::now().naive_local().signed_duration_since(started);
    u64::try_from(elapsed.num_milliseconds()).unwrap_or(0)
}

fn persist_task_result(
    state_dir: &Path,
    exit_code: i32,
    started_at: &str,
    last_stage: &str,
    status: &str,
) {
    let result = TaskResult {
        exit_code,
        duration_ms: compute_duration_ms(started_at),
        last_stage: last_stage.to_string(),
        status: status.to_string(),
    };
    let _ = write_task_result(state_dir, &result);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachOutcome {
    NoTask,
    StillRunning {
        description: String,
        env_name: String,
        downloading_os_id: String,
        log_offset: i64,
        progress: TaskProgressState,
    },
    Finished {
        env_name: String,
        exit_code: i32,
        has_exit_code: bool,
        task_log: String,
        history_json: String,
    },
}

pub fn evaluate_attach_existing_task(
    state_dir: &Path,
    secret_values: &[String],
    max_history_entries: i32,
) -> AttachOutcome {
    let Some(meta) = read_task_meta(state_dir) else {
        return AttachOutcome::NoTask;
    };

    let still_running = is_task_lock_active(state_dir, false).0;

    let log_path = task_log_path(state_dir);
    let raw_log = fs::read(&log_path).unwrap_or_default();
    let log_offset = raw_log.len() as i64;

    let mut progress = TaskProgressState {
        task_progress: 5,
        task_stage: meta.description.clone(),
        task_log: String::new(),
        has_exit_code: false,
        last_exit_code: -1,
        pending_env_name: meta.env_name.clone(),
        line_buffer: String::new(),
    };
    if !raw_log.is_empty() {
        feed_process_output_streaming(&mut progress, &raw_log, secret_values);
    }

    if still_running && !progress.has_exit_code {
        return AttachOutcome::StillRunning {
            description: meta.description,
            env_name: meta.env_name,
            downloading_os_id: meta.downloading_os_id.unwrap_or_default(),
            log_offset,
            progress,
        };
    }

    flush_process_output_streaming(&mut progress, secret_values);

    let exit_code = if progress.has_exit_code {
        progress.last_exit_code
    } else {
        1
    };
    redact_log_file_in_place(&log_path, &raw_log, secret_values);

    let target_env = if meta.target_env_name.is_empty() {
        &meta.env_name
    } else {
        &meta.target_env_name
    };
    let status = if exit_code == 0 { "success" } else { "failed" };
    persist_task_result(
        state_dir,
        exit_code,
        &meta.started_at,
        &progress.task_stage,
        status,
    );
    let state_dir_str = state_dir.to_string_lossy();
    let history_entries = record_operation_history(&RecordOperationParams {
        state_dir: &state_dir_str,
        description: &meta.description,
        env_name: target_env,
        status,
        exit_code,
        started_at_iso: &meta.started_at,
        log_bytes: &raw_log,
        secret_values,
        reason: meta.reason.as_deref().unwrap_or(""),
        max_entries: max_history_entries,
    });
    let history_json = history_entries
        .map(|v| serde_json::to_string(&v).unwrap_or_else(|_| "[]".to_string()))
        .unwrap_or_default();

    clear_task_files(state_dir);

    AttachOutcome::Finished {
        env_name: meta.env_name,
        exit_code,
        has_exit_code: progress.has_exit_code,
        task_log: progress.task_log,
        history_json,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckTaskOutcome {
    pub finished: bool,
    pub log_offset: i64,
    pub progress_changed: bool,
    pub progress: TaskProgressState,
    pub exit_code: i32,
    pub finished_env: String,
    pub last_log: String,
    pub history_json: String,
}

pub fn evaluate_check_background_task(
    state_dir: &Path,
    log_offset: i64,
    mut progress: TaskProgressState,
    secret_values: &[String],
    max_history_entries: i32,
) -> CheckTaskOutcome {
    let log_path = task_log_path(state_dir);
    let mut new_offset = log_offset.max(0);
    let mut progress_changed = false;

    if let Ok(mut f) = File::open(&log_path) {
        if let Ok(meta) = f.metadata() {
            let file_len = meta.len() as i64;
            if file_len < new_offset {
                new_offset = 0;
                progress.line_buffer.clear();
            }
            if file_len > new_offset {
                let _ = f.seek(SeekFrom::Start(new_offset as u64));
                let mut buf = Vec::new();
                if f.read_to_end(&mut buf).is_ok() {
                    new_offset = new_offset.saturating_add(buf.len() as i64);
                    let delta = feed_process_output_streaming(&mut progress, &buf, secret_values);
                    progress_changed = delta.progress_changed;
                }
            }
        }
    }

    let alive = is_task_lock_active(state_dir, false).0;

    if alive && !progress.has_exit_code {
        return CheckTaskOutcome {
            finished: false,
            log_offset: new_offset,
            progress_changed,
            progress,
            exit_code: -1,
            finished_env: String::new(),
            last_log: String::new(),
            history_json: String::new(),
        };
    }

    if flush_process_output_streaming(&mut progress, secret_values).progress_changed {
        progress_changed = true;
    }

    let finished_env = progress.pending_env_name.clone();
    let last_log = progress.task_log.clone();
    let exit_code = if progress.has_exit_code {
        progress.last_exit_code
    } else {
        1
    };

    let mut desc = progress.task_stage.clone();
    let mut target_env = finished_env.clone();
    let mut started_at = String::new();
    let mut task_reason = String::new();
    if let Some(meta) = read_task_meta(state_dir) {
        if !meta.description.is_empty() {
            desc = meta.description;
        }
        if !meta.target_env_name.is_empty() {
            target_env = meta.target_env_name;
        }
        started_at = meta.started_at;
        task_reason = meta.reason.unwrap_or_default();
    }

    let full_log = fs::read(&log_path).unwrap_or_default();
    redact_log_file_in_place(&log_path, &full_log, secret_values);

    let status = if exit_code == 0 { "success" } else { "failed" };
    persist_task_result(
        state_dir,
        exit_code,
        &started_at,
        &progress.task_stage,
        status,
    );
    let state_dir_str = state_dir.to_string_lossy();
    let history_entries = record_operation_history(&RecordOperationParams {
        state_dir: &state_dir_str,
        description: &desc,
        env_name: &target_env,
        status,
        exit_code,
        started_at_iso: &started_at,
        log_bytes: &full_log,
        secret_values,
        reason: &task_reason,
        max_entries: max_history_entries,
    });
    let history_json = history_entries
        .map(|v| serde_json::to_string(&v).unwrap_or_else(|_| "[]".to_string()))
        .unwrap_or_default();

    clear_task_files(state_dir);

    CheckTaskOutcome {
        finished: true,
        log_offset: new_offset,
        progress_changed,
        progress,
        exit_code,
        finished_env,
        last_log,
        history_json,
    }
}

pub fn cancel_running_task(
    state_dir: &Path,
    task_stage: &str,
    pending_env_name: &str,
    secret_values: &[String],
    max_history_entries: i32,
) -> String {
    let pid = read_task_pid(state_dir);
    if pid > 0 {
        send_signal_group(pid, libc::SIGTERM);
        send_signal_group(pid, libc::SIGKILL);
    }

    let mut desc = task_stage.to_string();
    let mut target_env = pending_env_name.to_string();
    let mut started_at = String::new();
    let mut task_reason = String::new();
    if let Some(meta) = read_task_meta(state_dir) {
        if !meta.description.is_empty() {
            desc = meta.description;
        }
        if !meta.target_env_name.is_empty() {
            target_env = meta.target_env_name;
        }
        started_at = meta.started_at;
        task_reason = meta.reason.unwrap_or_default();
    }

    let log_path = task_log_path(state_dir);
    let full_log = fs::read(&log_path).unwrap_or_default();
    redact_log_file_in_place(&log_path, &full_log, secret_values);
    persist_task_result(state_dir, 130, &started_at, task_stage, "cancelled");

    let state_dir_str = state_dir.to_string_lossy();
    let history_entries = record_operation_history(&RecordOperationParams {
        state_dir: &state_dir_str,
        description: &desc,
        env_name: &target_env,
        status: "cancelled",
        exit_code: 130,
        started_at_iso: &started_at,
        log_bytes: &full_log,
        secret_values,
        reason: &task_reason,
        max_entries: max_history_entries,
    });
    clear_task_files(state_dir);

    history_entries
        .map(|v| serde_json::to_string(&v).unwrap_or_else(|_| "[]".to_string()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("aibox_task_prog_{name}_{nanos}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    #[test]
    fn byte_by_byte_streaming_matches_whole_chunk() {
        let input = b"\x1b[32m@@STAGE:25:Creating...\x1b[0m\r\nRetrieving image: 50%\rRetrieving image: 100%\n@@STAGE:100:Ready!\n@@EXIT:0\n";
        let secrets = vec!["super-secret".to_string()];

        let mut whole = TaskProgressState {
            pending_env_name: "dev".to_string(),
            ..Default::default()
        };
        parse_process_output_chunk(&mut whole, input, &secrets);

        let mut streamed = TaskProgressState {
            pending_env_name: "dev".to_string(),
            ..Default::default()
        };
        for b in input {
            feed_process_output_streaming(&mut streamed, &[*b], &secrets);
        }
        flush_process_output_streaming(&mut streamed, &secrets);

        assert_eq!(streamed.task_progress, whole.task_progress);
        assert_eq!(streamed.task_stage, whole.task_stage);
        assert_eq!(streamed.task_log, whole.task_log);
        assert_eq!(streamed.has_exit_code, whole.has_exit_code);
        assert_eq!(streamed.last_exit_code, whole.last_exit_code);
    }

    #[test]
    fn attach_existing_task_restores_completion_result() {
        let dir = temp_dir("attach_done");
        fs::write(
            dir.join("task.meta"),
            r#"{"description":"Creating environment 'demo-box'","envName":"demo-box"}"#,
        )
        .expect("write meta");
        fs::write(
            dir.join("task.log"),
            "@@STAGE:50:Launching...\n@@STAGE:100:Ready!\n@@EXIT:0\n",
        )
        .expect("write log");

        let outcome = evaluate_attach_existing_task(&dir, &[], 50);
        match outcome {
            AttachOutcome::Finished {
                env_name,
                exit_code,
                has_exit_code,
                ..
            } => {
                assert_eq!(env_name, "demo-box");
                assert_eq!(exit_code, 0);
                assert!(has_exit_code);
            }
            other => panic!("unexpected attach outcome: {other:?}"),
        }
        assert!(!dir.join("task.meta").exists());
        let res = crate::task::files::read_task_result(&dir).expect("result.json");
        assert_eq!(res.exit_code, 0);
        assert_eq!(res.last_stage, "Ready!");
        assert_eq!(res.status, "success");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn incremental_check_handles_mid_line_splits_and_writes_result() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let dir = temp_dir("mid_line");
        let mut sleeper = Command::new("/bin/sleep")
            .arg("10")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep");
        let pid = i64::from(sleeper.id());
        crate::task::files::write_task_pid(&dir, pid).expect("write pid");
        crate::task::files::write_task_meta(
            &dir,
            &crate::task::files::TaskMeta {
                description: "Creating 'dev'".to_string(),
                env_name: "dev".to_string(),
                target_env_name: "dev".to_string(),
                downloading_os_id: None,
                started_at: Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
                reason: None,
            },
        )
        .expect("write meta");

        let log_path = task_log_path(&dir);
        let mut f = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&log_path)
            .expect("open log");
        // Write a partial @@STAGE line (cut mid-line)
        f.write_all(b"@@STAGE:40:Down").expect("write chunk 1");
        f.flush().expect("flush");

        let state = TaskProgressState {
            task_progress: 5,
            task_stage: "Init".to_string(),
            pending_env_name: "dev".to_string(),
            ..Default::default()
        };
        let tick1 = evaluate_check_background_task(&dir, 0, state, &[], 50);
        assert!(!tick1.finished);
        assert_eq!(tick1.log_offset, 15);
        // Incomplete line stays buffered; stage has not been applied yet.
        assert_eq!(tick1.progress.task_progress, 5);
        assert_eq!(tick1.progress.line_buffer, "@@STAGE:40:Down");

        // Complete the line and start another mid-line split for @@EXIT
        f.write_all(b"loading image...\n@@STAGE:90:Unpacking\n@@EX")
            .expect("write chunk 2");
        f.flush().expect("flush");

        let tick2 = evaluate_check_background_task(&dir, tick1.log_offset, tick1.progress, &[], 50);
        assert!(!tick2.finished);
        assert_eq!(tick2.progress.task_progress, 90);
        assert_eq!(tick2.progress.task_stage, "Unpacking");
        assert_eq!(tick2.progress.line_buffer, "@@EX");

        // Finish @@EXIT without trailing newline and stop the sleeper process
        f.write_all(b"IT:0").expect("write chunk 3");
        f.flush().expect("flush");
        let _ = sleeper.kill();
        let _ = sleeper.wait();

        let tick3 = evaluate_check_background_task(&dir, tick2.log_offset, tick2.progress, &[], 50);
        assert!(tick3.finished);
        assert_eq!(tick3.exit_code, 0);
        assert!(tick3.progress.has_exit_code);
        assert_eq!(tick3.progress.task_stage, "Unpacking");

        let result = crate::task::files::read_task_result(&dir).expect("result.json");
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.last_stage, "Unpacking");
        assert_eq!(result.status, "success");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn streaming_large_chunk_handles_multi_line_and_remainder() {
        let mut state = TaskProgressState {
            pending_env_name: "dev".to_string(),
            ..Default::default()
        };
        let mut payload = Vec::new();
        for i in 0..500 {
            payload.extend_from_slice(format!("line-{i}\n").as_bytes());
        }
        payload.extend_from_slice(b"@@STAGE:77:Finalizing");
        let delta = feed_process_output_streaming(&mut state, &payload, &[]);
        assert!(delta.progress_changed);
        assert_eq!(state.task_log, "line-499");
        assert_eq!(state.line_buffer, "@@STAGE:77:Finalizing");
        flush_process_output_streaming(&mut state, &[]);
        assert_eq!(state.task_progress, 77);
        assert_eq!(state.task_stage, "Finalizing");
        assert!(state.line_buffer.is_empty());
    }
}
