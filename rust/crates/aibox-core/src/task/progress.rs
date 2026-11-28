use std::fs::{self, File};
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::OnceLock;

use chrono::{Local, NaiveDateTime};
use regex::Regex;

use crate::history::{record_operation_history, RecordOperationParams, DEFAULT_LOG_TAIL_BYTES};
use crate::secrets::redact::{redact_secrets, SecretRedactor, StreamingRedactor};
use crate::task::files::{
    clear_task_files, read_process_start_time, read_task_lock, read_task_meta, task_log_path,
    write_task_result, TaskResult,
};
use crate::task::lock::{is_pid_alive, is_task_lock_active, send_signal_group};

const MAX_LINE_BUFFER_BYTES: usize = 64 * 1024;
const STREAM_CHUNK_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskProgressState {
    pub task_progress: i32,
    pub task_stage: String,
    pub task_log: String,
    pub has_exit_code: bool,
    pub last_exit_code: i32,
    pub pending_env_name: String,
    pub line_buffer: Vec<u8>,
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
    let redactor = SecretRedactor::new(secret_values);
    let mut line_buf = std::mem::take(&mut state.line_buffer);
    line_buf.extend_from_slice(chunk);

    let mut progress_changed = false;
    let mut cursor = 0usize;
    while let Some(rel_pos) = line_buf[cursor..]
        .iter()
        .position(|&b| b == b'\r' || b == b'\n')
    {
        let end = cursor + rel_pos;
        let raw_line = String::from_utf8_lossy(&line_buf[cursor..end]);
        cursor = end + 1;
        let redacted = redactor.redact_str(&raw_line);
        let clean = strip_csi_ansi(&redacted);
        if process_line(state, &clean) {
            progress_changed = true;
        }
    }
    if cursor > 0 {
        line_buf.drain(..cursor);
    }
    if line_buf.len() > MAX_LINE_BUFFER_BYTES {
        let raw_line = String::from_utf8_lossy(&line_buf);
        let redacted = redactor.redact_str(&raw_line);
        let clean = strip_csi_ansi(&redacted);
        if process_line(state, &clean) {
            progress_changed = true;
        }
        line_buf.clear();
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
    let raw_line = String::from_utf8_lossy(&rem);
    let redacted = redact_secrets(&raw_line, secret_values);
    let clean = strip_csi_ansi(&redacted);
    let progress_changed = process_line(state, &clean);
    TaskOutputDelta { progress_changed }
}

fn append_tail_ring(tail: &mut Vec<u8>, chunk: &[u8], limit: usize) {
    if limit == 0 || chunk.is_empty() {
        return;
    }
    if chunk.len() >= limit {
        tail.clear();
        tail.extend_from_slice(&chunk[chunk.len() - limit..]);
        return;
    }
    let total = tail.len() + chunk.len();
    if total > limit {
        tail.drain(..total - limit);
    }
    tail.extend_from_slice(chunk);
}

fn read_log_tail(log_path: &Path, limit: usize) -> Vec<u8> {
    let Ok(mut f) = File::open(log_path) else {
        return Vec::new();
    };
    let Ok(meta) = f.metadata() else {
        return Vec::new();
    };
    let len = meta.len();
    let start = len.saturating_sub(limit as u64);
    if f.seek(SeekFrom::Start(start)).is_err() {
        return Vec::new();
    }
    let mut buf = Vec::with_capacity((len - start) as usize);
    if f.read_to_end(&mut buf).is_err() {
        return Vec::new();
    }
    buf
}

fn redact_log_file_streaming_and_read_tail(
    log_path: &Path,
    secret_values: &[String],
    tail_limit: usize,
) -> Vec<u8> {
    if secret_values.is_empty() {
        return read_log_tail(log_path, tail_limit);
    }
    let Ok(mut src) = File::open(log_path) else {
        return Vec::new();
    };

    let tmp_path = log_path.with_extension("log.redact.tmp");
    let mut writer = File::create(&tmp_path).ok().map(BufWriter::new);
    let mut redactor = StreamingRedactor::new(secret_values);
    let mut tail = Vec::with_capacity(tail_limit);
    let mut buf = vec![0u8; STREAM_CHUNK_BYTES];

    while let Ok(n) = src.read(&mut buf) {
        if n == 0 {
            break;
        }
        let out = redactor.feed(&buf[..n]);
        if !out.is_empty() {
            append_tail_ring(&mut tail, &out, tail_limit);
            if let Some(w) = writer.as_mut() {
                let _ = w.write_all(&out);
            }
        }
    }

    let (rem, modified) = redactor.finish_with_status();
    if !rem.is_empty() {
        append_tail_ring(&mut tail, &rem, tail_limit);
        if let Some(w) = writer.as_mut() {
            let _ = w.write_all(&rem);
        }
    }

    if let Some(mut w) = writer {
        let _ = w.flush();
        drop(w);
        if modified {
            if fs::rename(&tmp_path, log_path).is_err() {
                let _ = fs::remove_file(&tmp_path);
            }
        } else {
            let _ = fs::remove_file(&tmp_path);
        }
    }

    tail
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
    let mut log_offset = 0i64;

    let mut progress = TaskProgressState {
        task_progress: 5,
        task_stage: meta.description.clone(),
        task_log: String::new(),
        has_exit_code: false,
        last_exit_code: -1,
        pending_env_name: meta.env_name.clone(),
        line_buffer: Vec::new(),
    };
    if let Ok(mut f) = File::open(&log_path) {
        let mut buf = vec![0u8; STREAM_CHUNK_BYTES];
        while let Ok(n) = f.read(&mut buf) {
            if n == 0 {
                break;
            }
            log_offset = log_offset.saturating_add(n as i64);
            feed_process_output_streaming(&mut progress, &buf[..n], secret_values);
        }
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
    let log_tail = redact_log_file_streaming_and_read_tail(
        &log_path,
        secret_values,
        DEFAULT_LOG_TAIL_BYTES as usize,
    );

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
        log_bytes: &log_tail,
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

    // Drain any final bytes written between the initial read and process exit.
    if !alive && !progress.has_exit_code {
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
                        if feed_process_output_streaming(&mut progress, &buf, secret_values)
                            .progress_changed
                        {
                            progress_changed = true;
                        }
                    }
                }
            }
        }
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

    let log_tail = redact_log_file_streaming_and_read_tail(
        &log_path,
        secret_values,
        DEFAULT_LOG_TAIL_BYTES as usize,
    );

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
        log_bytes: &log_tail,
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
    let lock = read_task_lock(state_dir);
    let pid_matches = lock.pid > 0
        && is_pid_alive(lock.pid)
        && lock
            .start_time
            .is_none_or(|st| read_process_start_time(lock.pid) == Some(st));
    if pid_matches {
        send_signal_group(lock.pid, libc::SIGTERM);
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(300);
        while std::time::Instant::now() < deadline {
            if !is_pid_alive(lock.pid)
                || lock
                    .start_time
                    .is_some_and(|st| read_process_start_time(lock.pid) != Some(st))
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if is_pid_alive(lock.pid)
            && lock
                .start_time
                .is_none_or(|st| read_process_start_time(lock.pid) == Some(st))
        {
            send_signal_group(lock.pid, libc::SIGKILL);
        }
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
    let log_tail = redact_log_file_streaming_and_read_tail(
        &log_path,
        secret_values,
        DEFAULT_LOG_TAIL_BYTES as usize,
    );
    persist_task_result(state_dir, 130, &started_at, task_stage, "cancelled");

    let state_dir_str = state_dir.to_string_lossy();
    let history_entries = record_operation_history(&RecordOperationParams {
        state_dir: &state_dir_str,
        description: &desc,
        env_name: &target_env,
        status: "cancelled",
        exit_code: 130,
        started_at_iso: &started_at,
        log_bytes: &log_tail,
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
        assert_eq!(tick1.progress.line_buffer, b"@@STAGE:40:Down");

        // Complete the line and start another mid-line split for @@EXIT
        f.write_all(b"loading image...\n@@STAGE:90:Unpacking\n@@EX")
            .expect("write chunk 2");
        f.flush().expect("flush");

        let tick2 = evaluate_check_background_task(&dir, tick1.log_offset, tick1.progress, &[], 50);
        assert!(!tick2.finished);
        assert_eq!(tick2.progress.task_progress, 90);
        assert_eq!(tick2.progress.task_stage, "Unpacking");
        assert_eq!(tick2.progress.line_buffer, b"@@EX");

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
        assert_eq!(state.line_buffer, b"@@STAGE:77:Finalizing");
        flush_process_output_streaming(&mut state, &[]);
        assert_eq!(state.task_progress, 77);
        assert_eq!(state.task_stage, "Finalizing");
        assert!(state.line_buffer.is_empty());
    }

    #[test]
    fn streaming_preserves_multibyte_utf8_across_chunk_boundary() {
        let mut state = TaskProgressState::default();
        let line = "@@STAGE:50:Preparing — étape\n".as_bytes();
        let split = line.iter().position(|&b| b > 0x7f).expect("multibyte") + 1;
        feed_process_output_streaming(&mut state, &line[..split], &[]);
        feed_process_output_streaming(&mut state, &line[split..], &[]);
        assert_eq!(state.task_progress, 50);
        assert_eq!(state.task_stage, "Preparing — étape");
    }
}
