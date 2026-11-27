//! Terminal session recording, ANSI stripping, redaction, retention pruning, and replay helpers.

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::secrets::redact::StreamingRedactor;
use crate::secrets::store::{
    default_secrets_store_path, instance_secrets_dir_path, load_secrets_store,
};
use crate::secrets::zeroize_string;
use crate::util::format::format_bytes;
use crate::util::names::sanitize_name;
use crate::util::paths::{atomic_write_with, atomic_write_with_mode, clean_path};
use crate::util::shell::shell_quote;

pub const DEFAULT_SESSION_VIEW_MAX_BYTES: i32 = 200 * 1024;
pub const DEFAULT_PRUNE_MAX_PER_ENV: i32 = 20;
pub const DEFAULT_PRUNE_MAX_TOTAL_BYTES: i64 = 200 * 1024 * 1024;
const STREAM_CHUNK_BYTES: usize = 64 * 1024;
const SESSIONS_INDEX_FILE: &str = "sessions.idx";
const SESSIONS_INDEX_VERSION: u32 = 1;

const DEMO_SESSION_LOG: &str = concat!(
    "Script started on 2026-11-13 17:10:00+07:00 [COMMAND=\"incus exec desktop-studio --cwd /home/workspace -- bash -l\"]\n",
    "\x1b[32muser@desktop-studio:/home/workspace$\x1b[0m ls -la\n",
    "total 12\n",
    "drwxrwx--- 1 root user 120 Nov 13 17:09 .\n",
    "\x1b[32muser@desktop-studio:/home/workspace$\x1b[0m echo $ANTHROPIC_API_KEY\n",
    "[REDACTED]\n",
    "\x1b[32muser@desktop-studio:/home/workspace$\x1b[0m exit\n",
    "Script done on 2026-11-13 17:12:14+07:00 [COMMAND_EXIT_CODE=\"0\"]\n"
);

/// Returns `<state_dir>/sessions`.
pub fn sessions_dir_path(state_dir: &str) -> String {
    format!("{state_dir}/sessions")
}

/// Creates the per-environment session directory (`0700`) and picks unique `.log`/`.timing` paths.
pub fn prepare_recorded_session_paths(state_dir: &str, env_name: &str) -> Option<(String, String)> {
    let clean_env = sanitize_name(env_name);
    if clean_env.is_empty() {
        return None;
    }
    let sessions_dir = sessions_dir_path(state_dir);
    let env_dir = format!("{sessions_dir}/{clean_env}");
    fs::create_dir_all(&env_dir).ok()?;
    let _ = fs::set_permissions(&sessions_dir, fs::Permissions::from_mode(0o700));
    let _ = fs::set_permissions(&env_dir, fs::Permissions::from_mode(0o700));

    let mut epoch = Utc::now().timestamp();
    while Path::new(&format!("{env_dir}/{epoch}.log")).exists() {
        epoch += 1;
    }
    Some((
        format!("{env_dir}/{epoch}.log"),
        format!("{env_dir}/{epoch}.timing"),
    ))
}

/// Resolves whether session recording is enabled given the global default and per-instance override.
pub fn effective_record_sessions(global_enabled: bool, env_override: &str) -> bool {
    let ov = env_override.trim().to_ascii_lowercase();
    if ov == "true" || ov == "1" {
        return true;
    }
    if ov == "false" || ov == "0" {
        return false;
    }
    global_enabled
}

/// Wraps `inner_command` with `script -q -f` and optional `--redact-session` post-hook.
pub fn build_session_wrap_command(
    inner_command: &str,
    log_file: &str,
    timing_file: &str,
    redact_bin_path: &str,
    sessions_dir: &str,
    script_available: bool,
) -> String {
    if !script_available
        || inner_command.trim().is_empty()
        || log_file.trim().is_empty()
        || timing_file.trim().is_empty()
    {
        return inner_command.to_string();
    }

    let cleaned_log = clean_path(log_file);
    let log_dir = Path::new(&cleaned_log)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or(".");
    let q_log_dir = shell_quote(log_dir);
    let q_log = shell_quote(log_file);
    let q_timing = shell_quote(timing_file);
    let q_inner = shell_quote(inner_command);

    let post_cmd = if redact_bin_path.trim().is_empty() {
        String::new()
    } else {
        let sess_arg = if sessions_dir.trim().is_empty() {
            String::new()
        } else {
            format!(" {}", shell_quote(sessions_dir))
        };
        format!(
            "    {} --redact-session {}{} >/dev/null 2>&1 || true\n",
            shell_quote(redact_bin_path),
            q_log,
            sess_arg
        )
    };

    format!(
        "if command -v script >/dev/null 2>&1; then\n\
         \x20   mkdir -p {q_log_dir} && chmod 700 {q_log_dir} 2>/dev/null || true\n\
         \x20   touch {q_log} {q_timing} && chmod 600 {q_log} {q_timing} 2>/dev/null || true\n\
         \x20   script -q -f --log-out {q_log} --log-timing {q_timing} -c {q_inner}\n\
         \x20   _aibox_rc=$?\n\
         {post_cmd}\
         \x20   exit $_aibox_rc\n\
         else\n\
         \x20   {inner_command}\n\
         fi"
    )
}

const MAX_OSC_PENDING: usize = 4096;
const MAX_CSI_PENDING: usize = 256;

#[derive(Clone, Copy, PartialEq, Eq)]
enum AnsiState {
    Normal,
    Esc,
    Osc,
    OscEsc,
    CsiParams,
    CsiInter,
    Esc2,
}

/// Single-pass streaming ANSI/VT escape sequence stripper.
///
/// Handles sequences split across arbitrary chunk boundaries and strips `\r`
/// and `\x07` while leaving multi-byte UTF-8 sequences untouched.
pub struct StreamingAnsiStripper {
    state: AnsiState,
    pending: Vec<u8>,
}

impl Default for StreamingAnsiStripper {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamingAnsiStripper {
    pub fn new() -> Self {
        Self {
            state: AnsiState::Normal,
            pending: Vec::new(),
        }
    }

    /// Feeds a byte chunk and appends the stripped output to `out`.
    pub fn feed_into(&mut self, chunk: &[u8], out: &mut Vec<u8>) {
        for &b in chunk {
            self.step(b, out);
        }
    }

    /// Flushes any incomplete trailing escape sequence at EOF into `out`.
    pub fn finish_into(mut self, out: &mut Vec<u8>) {
        match self.state {
            AnsiState::Normal | AnsiState::Esc2 => {}
            AnsiState::Esc => out.push(0x1b),
            AnsiState::Osc | AnsiState::OscEsc => {
                for b in self.pending.drain(..) {
                    if b != b'\r' && b != 0x07 {
                        out.push(b);
                    }
                }
            }
            AnsiState::CsiParams | AnsiState::CsiInter => {
                for b in self.pending.drain(..) {
                    if b != b'\r' && b != 0x07 {
                        out.push(b);
                    }
                }
            }
        }
    }

    fn step(&mut self, b: u8, out: &mut Vec<u8>) {
        match self.state {
            AnsiState::Normal => {
                if b == 0x1b {
                    self.state = AnsiState::Esc;
                } else if b != b'\r' && b != 0x07 {
                    out.push(b);
                }
            }
            AnsiState::Esc => match b {
                b']' => {
                    self.pending.clear();
                    self.state = AnsiState::Osc;
                }
                b'[' => {
                    self.pending.clear();
                    self.pending.extend_from_slice(b"\x1b[");
                    self.state = AnsiState::CsiParams;
                }
                b'(' | b')' | b'#' | b'%' | b'*' | b'+' | b'-' | b'/' => {
                    self.state = AnsiState::Esc2;
                }
                0x40..=0x5A | 0x5C..=0x5F => {
                    self.state = AnsiState::Normal;
                }
                _ => {
                    out.push(0x1b);
                    self.state = AnsiState::Normal;
                    self.step(b, out);
                }
            },
            AnsiState::Osc => {
                if b == 0x07 {
                    self.pending.clear();
                    self.state = AnsiState::Normal;
                } else if b == 0x1b {
                    self.state = AnsiState::OscEsc;
                } else if self.pending.len() < MAX_OSC_PENDING {
                    self.pending.push(b);
                } else {
                    // Unterminated/oversized OSC: drop the `\x1b]` prefix and flush payload.
                    let buffered = std::mem::take(&mut self.pending);
                    self.state = AnsiState::Normal;
                    for pb in buffered {
                        self.step(pb, out);
                    }
                    self.step(b, out);
                }
            }
            AnsiState::OscEsc => {
                if b == b'\\' {
                    self.pending.clear();
                    self.state = AnsiState::Normal;
                } else {
                    // Previous `\x1b` was not ST (`\x1b\\`), so OSC was unterminated.
                    let buffered = std::mem::take(&mut self.pending);
                    for pb in buffered {
                        if pb != b'\r' && pb != 0x07 {
                            out.push(pb);
                        }
                    }
                    self.state = AnsiState::Esc;
                    self.step(b, out);
                }
            }
            AnsiState::CsiParams => {
                if matches!(b, b'0'..=b'9' | b';' | b'?') && self.pending.len() < MAX_CSI_PENDING {
                    self.pending.push(b);
                } else if (0x20..=0x2F).contains(&b) && self.pending.len() < MAX_CSI_PENDING {
                    self.pending.push(b);
                    self.state = AnsiState::CsiInter;
                } else if (0x40..=0x7E).contains(&b) {
                    self.pending.clear();
                    self.state = AnsiState::Normal;
                } else {
                    let buffered = std::mem::take(&mut self.pending);
                    self.state = AnsiState::Normal;
                    for pb in buffered {
                        if pb != b'\r' && pb != 0x07 {
                            out.push(pb);
                        }
                    }
                    self.step(b, out);
                }
            }
            AnsiState::CsiInter => {
                if (0x20..=0x2F).contains(&b) && self.pending.len() < MAX_CSI_PENDING {
                    self.pending.push(b);
                } else if (0x40..=0x7E).contains(&b) {
                    self.pending.clear();
                    self.state = AnsiState::Normal;
                } else {
                    let buffered = std::mem::take(&mut self.pending);
                    self.state = AnsiState::Normal;
                    for pb in buffered {
                        if pb != b'\r' && pb != 0x07 {
                            out.push(pb);
                        }
                    }
                    self.step(b, out);
                }
            }
            AnsiState::Esc2 => {
                self.state = AnsiState::Normal;
                if !b.is_ascii_alphanumeric() {
                    self.step(b, out);
                }
            }
        }
    }
}

/// Strips ANSI/VT escape sequences and normalizes `\r\n` / `\r` for plain-text session viewing.
pub fn strip_ansi_sequences(raw_text: &str) -> String {
    let mut stripper = StreamingAnsiStripper::new();
    let mut out = Vec::with_capacity(raw_text.len());
    stripper.feed_into(raw_text.as_bytes(), &mut out);
    stripper.finish_into(&mut out);
    String::from_utf8_lossy(&out).into_owned()
}

fn finalize_truncated_session_text(mut clean: String, cap: usize, mut truncated: bool) -> String {
    let utf8 = clean.as_bytes();
    if utf8.len() > cap {
        truncated = true;
        let mut tail = &utf8[utf8.len() - cap..];
        if let Some(nl) = tail.iter().position(|&b| b == b'\n') {
            if nl < 256 {
                tail = &tail[nl + 1..];
            }
        }
        clean = String::from_utf8_lossy(tail).into_owned();
    }

    if truncated {
        format!("[... truncated to last 200 KB ...]\n{clean}")
    } else {
        clean
    }
}

/// Strips ANSI sequences from `raw_bytes` and caps the output to the tail `max_bytes` (minimum 1024).
pub fn format_recorded_session_text(raw_bytes: &[u8], max_bytes: i32) -> String {
    if raw_bytes.is_empty() {
        return String::new();
    }
    let cap = max_bytes.max(1024) as usize;
    let mut truncated = false;
    let raw_budget = cap * 2;
    let slice = if raw_bytes.len() > raw_budget {
        truncated = true;
        &raw_bytes[raw_bytes.len() - raw_budget..]
    } else {
        raw_bytes
    };

    let clean = strip_ansi_sequences(&String::from_utf8_lossy(slice));
    finalize_truncated_session_text(clean, cap, truncated)
}

/// Atomically redacts `secret_values` in `log_file_path` (preserving `0600` permissions)
/// by streaming in 64 KiB chunks so memory stays constant for large session logs.
/// Returns `true` if the file was modified.
pub fn redact_session_log_file(log_file_path: &str, secret_values: &[String]) -> bool {
    if log_file_path.trim().is_empty() || secret_values.is_empty() {
        return false;
    }
    let path = Path::new(log_file_path);
    let Ok(mut in_file) = File::open(path) else {
        return false;
    };

    // Scan first so logs without matching secrets avoid creating/writing a temp copy.
    {
        let mut reader = BufReader::with_capacity(STREAM_CHUNK_BYTES, &mut in_file);
        let mut scanner = StreamingRedactor::new(secret_values);
        let mut buf = vec![0u8; STREAM_CHUNK_BYTES];
        loop {
            let Ok(n) = reader.read(&mut buf) else {
                return false;
            };
            if n == 0 {
                break;
            }
            let _ = scanner.feed(&buf[..n]);
        }
        let (_, modified) = scanner.finish_with_status();
        if !modified {
            return false;
        }
    }

    if in_file.seek(SeekFrom::Start(0)).is_err() {
        return false;
    }

    let res = atomic_write_with(path, Some(0o600), |out_file| {
        let mut reader = BufReader::with_capacity(STREAM_CHUNK_BYTES, in_file);
        let mut writer = BufWriter::with_capacity(STREAM_CHUNK_BYTES, out_file);
        let mut redactor = StreamingRedactor::new(secret_values);
        let mut buf = vec![0u8; STREAM_CHUNK_BYTES];

        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            let out_chunk = redactor.feed(&buf[..n]);
            if !out_chunk.is_empty() {
                writer.write_all(&out_chunk)?;
            }
        }

        let (tail, modified) = redactor.finish_with_status();
        if !tail.is_empty() {
            writer.write_all(&tail)?;
        }
        writer.flush()?;
        Ok(modified)
    });

    res.unwrap_or(false)
}

#[derive(Clone, Serialize, Deserialize)]
struct SessionFilePair {
    log_path: String,
    timing_path: String,
    mtime_ms: i128,
    total_bytes: i64,
}

#[derive(Clone, Serialize, Deserialize)]
struct IndexedEnv {
    dir_mtime_ms: i128,
    entries: Vec<SessionFilePair>,
}

#[derive(Serialize, Deserialize, Default)]
struct SessionIndex {
    version: u32,
    envs: BTreeMap<String, IndexedEnv>,
}

fn remove_session_pair_files(pair: &SessionFilePair) {
    if !pair.log_path.is_empty() {
        let _ = fs::remove_file(&pair.log_path);
    }
    if !pair.timing_path.is_empty() {
        let _ = fs::remove_file(&pair.timing_path);
    }
}

fn file_mtime_ns(meta: &fs::Metadata) -> i128 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i128)
        .unwrap_or(0)
}

fn scan_env_session_pairs(env_dir: &Path) -> Vec<SessionFilePair> {
    let Ok(log_entries) = fs::read_dir(env_dir) else {
        return Vec::new();
    };
    let mut logs: Vec<(String, PathBuf)> = log_entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.is_file() {
                let name = p.file_name()?.to_str()?.to_string();
                if name.ends_with(".log") {
                    return Some((name, p));
                }
            }
            None
        })
        .collect();
    logs.sort_by(|a, b| a.0.cmp(&b.0));

    let mut pairs = Vec::with_capacity(logs.len());
    for (_, log_path_buf) in logs {
        let Ok(log_meta) = fs::metadata(&log_path_buf) else {
            continue;
        };
        let base = log_path_buf
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let timing_path_buf = env_dir.join(format!("{base}.timing"));
        let timing_meta = fs::metadata(&timing_path_buf).ok();
        let timing_bytes = timing_meta.as_ref().map(|m| m.len() as i64).unwrap_or(0);
        let total_bytes = log_meta.len() as i64 + timing_bytes;
        pairs.push(SessionFilePair {
            log_path: log_path_buf.to_string_lossy().into_owned(),
            timing_path: if timing_meta.is_some() {
                timing_path_buf.to_string_lossy().into_owned()
            } else {
                String::new()
            },
            mtime_ms: file_mtime_ns(&log_meta),
            total_bytes,
        });
    }
    pairs
}

fn load_session_index(idx_path: &Path) -> Option<SessionIndex> {
    let data = fs::read(idx_path).ok()?;
    let idx: SessionIndex = serde_json::from_slice(&data).ok()?;
    if idx.version != SESSIONS_INDEX_VERSION {
        return None;
    }
    Some(idx)
}

fn save_session_index(idx_path: &Path, idx: &SessionIndex) {
    if let Ok(bytes) = serde_json::to_vec(idx) {
        let _ = atomic_write_with_mode(idx_path, &bytes, Some(0o600));
    }
}

/// Prunes recorded sessions per environment (`max_per_env`) and globally (`max_total_bytes`).
///
/// Uses a lightweight index (`<sessions_dir>/sessions.idx`) so unchanged
/// environment directories do not need to be re-scanned on every run.
pub fn prune_recorded_sessions(sessions_dir: &str, max_per_env: i32, max_total_bytes: i64) -> i32 {
    let root = Path::new(sessions_dir);
    let Ok(env_entries) = fs::read_dir(root) else {
        return 0;
    };

    let mut env_dirs: Vec<(String, PathBuf)> = env_entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.is_dir() {
                let name = p.file_name()?.to_str()?.to_string();
                Some((name, p))
            } else {
                None
            }
        })
        .collect();
    env_dirs.sort_by(|a, b| a.0.cmp(&b.0));

    let idx_path = root.join(SESSIONS_INDEX_FILE);
    let loaded = load_session_index(&idx_path);
    let mut index_dirty = loaded.is_none();
    let prev_envs = loaded.map(|idx| idx.envs).unwrap_or_default();
    if prev_envs.len() != env_dirs.len() {
        index_dirty = true;
    }

    let mut next_envs: BTreeMap<String, IndexedEnv> = BTreeMap::new();
    let mut deleted = 0i32;
    let mut surviving: Vec<(String, SessionFilePair)> = Vec::new();

    for (env_name, env_dir) in env_dirs {
        let Ok(dir_meta) = fs::metadata(&env_dir) else {
            index_dirty = true;
            continue;
        };
        let dir_mtime_ms = file_mtime_ns(&dir_meta);

        let mut pairs = match prev_envs.get(&env_name) {
            Some(cached) if cached.dir_mtime_ms == dir_mtime_ms => {
                let mut refreshed = cached.entries.clone();
                // Re-stat entries so an active session that grew after an initial
                // 0-byte `touch` is accurately counted against `max_total_bytes`.
                for pair in &mut refreshed {
                    let Ok(log_meta) = fs::metadata(&pair.log_path) else {
                        continue;
                    };
                    let timing_bytes = if pair.timing_path.is_empty() {
                        0
                    } else {
                        fs::metadata(&pair.timing_path)
                            .map(|m| m.len() as i64)
                            .unwrap_or(0)
                    };
                    let next_total = log_meta.len() as i64 + timing_bytes;
                    let next_mtime = file_mtime_ns(&log_meta);
                    if next_total != pair.total_bytes || next_mtime != pair.mtime_ms {
                        pair.total_bytes = next_total;
                        pair.mtime_ms = next_mtime;
                        index_dirty = true;
                    }
                }
                refreshed
            }
            _ => {
                index_dirty = true;
                scan_env_session_pairs(&env_dir)
            }
        };

        // Newest first for per-env cap.
        pairs.sort_by(|a, b| {
            b.mtime_ms
                .cmp(&a.mtime_ms)
                .then_with(|| b.log_path.cmp(&a.log_path))
        });

        let mut env_surviving = Vec::with_capacity(pairs.len());
        let mut env_deleted = false;
        for (i, pair) in pairs.into_iter().enumerate() {
            if max_per_env >= 0 && i >= max_per_env as usize {
                remove_session_pair_files(&pair);
                deleted += 1;
                env_deleted = true;
                index_dirty = true;
            } else {
                surviving.push((env_name.clone(), pair.clone()));
                env_surviving.push(pair);
            }
        }

        let final_dir_mtime = if env_deleted {
            fs::metadata(&env_dir)
                .map(|m| file_mtime_ns(&m))
                .unwrap_or(dir_mtime_ms)
        } else {
            dir_mtime_ms
        };
        next_envs.insert(
            env_name,
            IndexedEnv {
                dir_mtime_ms: final_dir_mtime,
                entries: env_surviving,
            },
        );
    }

    if max_total_bytes > 0 {
        let mut total_bytes: i64 = surviving.iter().map(|(_, p)| p.total_bytes).sum();
        if total_bytes > max_total_bytes {
            // Oldest first for global size cap.
            surviving.sort_by(|a, b| {
                a.1.mtime_ms
                    .cmp(&b.1.mtime_ms)
                    .then_with(|| a.1.log_path.cmp(&b.1.log_path))
            });
            for (env_name, pair) in &surviving {
                if total_bytes <= max_total_bytes {
                    break;
                }
                remove_session_pair_files(pair);
                total_bytes -= pair.total_bytes;
                deleted += 1;
                index_dirty = true;
                if let Some(indexed_env) = next_envs.get_mut(env_name) {
                    indexed_env.entries.retain(|e| e.log_path != pair.log_path);
                    if let Ok(m) = fs::metadata(root.join(env_name)) {
                        indexed_env.dir_mtime_ms = file_mtime_ns(&m);
                    }
                }
            }
        }
    }

    if index_dirty {
        save_session_index(
            &idx_path,
            &SessionIndex {
                version: SESSIONS_INDEX_VERSION,
                envs: next_envs,
            },
        );
    }

    deleted
}

/// Scans `sessions_dir` for recorded sessions and returns QML-ready JSON entries sorted newest first.
pub fn scan_recorded_sessions(sessions_dir: &str) -> Vec<Value> {
    let root = Path::new(sessions_dir);
    let Ok(env_entries) = fs::read_dir(root) else {
        return Vec::new();
    };

    let mut env_dirs: Vec<(String, PathBuf)> = env_entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.is_dir() {
                let name = p.file_name()?.to_str()?.to_string();
                Some((name, p))
            } else {
                None
            }
        })
        .collect();
    env_dirs.sort_by(|a, b| a.0.cmp(&b.0));

    let mut items: Vec<(i64, Value)> = Vec::new();
    for (env_name, env_dir) in env_dirs {
        let Ok(log_entries) = fs::read_dir(&env_dir) else {
            continue;
        };
        for entry in log_entries.flatten() {
            let log_path_buf = entry.path();
            if !log_path_buf.is_file() {
                continue;
            }
            let Some(file_name) = log_path_buf.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if !file_name.ends_with(".log") {
                continue;
            }
            let Ok(log_meta) = fs::metadata(&log_path_buf) else {
                continue;
            };
            let base = file_name.trim_end_matches(".log");
            let timing_path_buf = env_dir.join(format!("{base}.timing"));
            let timing_meta = fs::metadata(&timing_path_buf).ok();
            let has_timing = timing_meta.as_ref().is_some_and(|m| m.len() > 0);
            let timing_bytes = timing_meta.as_ref().map(|m| m.len() as i64).unwrap_or(0);
            let total_bytes = log_meta.len() as i64 + timing_bytes;

            let dt: DateTime<Local> = match base.trim().parse::<i64>() {
                Ok(epoch) if epoch > 0 => DateTime::from_timestamp(epoch, 0)
                    .map(|u| u.with_timezone(&Local))
                    .unwrap_or_else(Local::now),
                _ => log_meta
                    .modified()
                    .map(DateTime::<Local>::from)
                    .unwrap_or_else(|_| Local::now()),
            };
            let epoch = dt.timestamp();

            let item = json!({
                "id": format!("{env_name}/{base}"),
                "envName": env_name,
                "epoch": epoch,
                "timestamp": dt.format("%Y-%m-%d %H:%M:%S").to_string(),
                "logPath": log_path_buf.to_string_lossy().into_owned(),
                "timingPath": if has_timing { timing_path_buf.to_string_lossy().into_owned() } else { String::new() },
                "hasTiming": has_timing,
                "sizeBytes": total_bytes,
                "sizeDisplay": format_bytes(total_bytes),
            });
            items.push((epoch, item));
        }
    }

    items.sort_by_key(|a| std::cmp::Reverse(a.0));
    items.into_iter().map(|(_, v)| v).collect()
}

/// Reads and formats a recorded session log file by streaming its tail in 64 KiB
/// chunks, returning `None` if missing/unreadable.
pub fn read_recorded_session_log(log_path: &str, demo_mode: bool) -> Option<String> {
    if log_path.trim().is_empty() {
        return Some(String::new());
    }
    if demo_mode && log_path.starts_with("demo://") {
        return Some(format_recorded_session_text(
            DEMO_SESSION_LOG.as_bytes(),
            DEFAULT_SESSION_VIEW_MAX_BYTES,
        ));
    }

    let mut file = File::open(log_path).ok()?;
    let file_len = file.metadata().ok()?.len();
    if file_len == 0 {
        return Some(String::new());
    }

    let cap = DEFAULT_SESSION_VIEW_MAX_BYTES.max(1024) as usize;
    let raw_budget = (cap * 2) as u64;
    let truncated = file_len > raw_budget;
    if truncated {
        file.seek(SeekFrom::End(-(raw_budget as i64))).ok()?;
    }

    let mut reader = BufReader::with_capacity(STREAM_CHUNK_BYTES, file);
    let mut stripper = StreamingAnsiStripper::new();
    let mut clean_bytes = Vec::with_capacity((file_len.min(raw_budget)) as usize);
    let mut chunk = vec![0u8; STREAM_CHUNK_BYTES];

    loop {
        let n = reader.read(&mut chunk).ok()?;
        if n == 0 {
            break;
        }
        stripper.feed_into(&chunk[..n], &mut clean_bytes);
    }
    stripper.finish_into(&mut clean_bytes);

    let clean = String::from_utf8_lossy(&clean_bytes).into_owned();
    Some(finalize_truncated_session_text(clean, cap, truncated))
}

/// Builds the shell command used to replay a recorded session with `scriptreplay`.
pub fn build_replay_command(log_path: &str, timing_path: &str) -> String {
    format!(
        "scriptreplay -t {} -O {}; printf '\\n[Replay complete — press Enter to close]'; read -r _",
        shell_quote(timing_path),
        shell_quote(log_path)
    )
}

/// Deletes a recorded session `.log` and optional `.timing` if inside `sessions_dir`.
/// Returns `Err(clean_sessions_root)` if `log_path` is outside `sessions_dir`.
pub fn delete_recorded_session(
    sessions_dir: &str,
    log_path: &str,
    timing_path: &str,
) -> Result<(), String> {
    let clean_root = clean_path(sessions_dir);
    let clean_log = clean_path(log_path.trim());
    let prefix = format!("{clean_root}/");
    if clean_log.is_empty() || !clean_log.starts_with(&prefix) {
        return Err(clean_root);
    }

    let canon_root = fs::canonicalize(&clean_root).map_err(|_| clean_root.clone())?;
    let log_parent = Path::new(&clean_log)
        .parent()
        .ok_or_else(|| clean_root.clone())?;
    let canon_log_parent = fs::canonicalize(log_parent).map_err(|_| clean_root.clone())?;
    if !canon_log_parent.starts_with(&canon_root) {
        return Err(clean_root);
    }

    let _ = fs::remove_file(&clean_log);
    let clean_timing = clean_path(timing_path.trim());
    if !clean_timing.is_empty() && clean_timing.starts_with(&prefix) {
        if let Some(timing_parent) = Path::new(&clean_timing).parent() {
            if let Ok(canon_timing_parent) = fs::canonicalize(timing_parent) {
                if canon_timing_parent.starts_with(&canon_root) {
                    let _ = fs::remove_file(&clean_timing);
                }
            }
        }
    }
    Ok(())
}

/// Entry point for `aibox --redact-session <log> [sessionsDir]`.
pub fn redact_session_main(log_path: &str, sessions_dir: &str) -> i32 {
    let store_path = match env::var("AIBOX_SECRETS_PATH") {
        Ok(val) if !val.trim().is_empty() => val.trim().to_string(),
        _ => default_secrets_store_path(),
    };
    let mut secret_values: Vec<String> = Vec::new();
    if let Ok(pairs) = load_secrets_store(&store_path) {
        for (_, mut v) in pairs {
            if !v.is_empty() && !secret_values.iter().any(|s| s == &v) {
                secret_values.push(v);
            } else {
                zeroize_string(&mut v);
            }
        }
    }

    let instance_dir = instance_secrets_dir_path(&store_path);
    if !instance_dir.is_empty() && Path::new(&instance_dir).is_dir() {
        if let Ok(entries) = fs::read_dir(&instance_dir) {
            let mut paths: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("json"))
                .collect();
            paths.sort();
            for p in paths {
                if let Ok(pairs) = load_secrets_store(&p.to_string_lossy()) {
                    for (_, mut v) in pairs {
                        if !v.is_empty() && !secret_values.iter().any(|s| s == &v) {
                            secret_values.push(v);
                        } else {
                            zeroize_string(&mut v);
                        }
                    }
                }
            }
        }
    }

    redact_session_log_file(log_path, &secret_values);
    for val in &mut secret_values {
        zeroize_string(val);
    }
    let _ = fs::set_permissions(log_path, fs::Permissions::from_mode(0o600));
    if !sessions_dir.is_empty() {
        prune_recorded_sessions(
            sessions_dir,
            DEFAULT_PRUNE_MAX_PER_ENV,
            DEFAULT_PRUNE_MAX_TOTAL_BYTES,
        );
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::SystemTime;

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("aibox-session-{prefix}-{nanos}"));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn effective_override_and_wrap_builder() {
        assert!(effective_record_sessions(true, ""));
        assert!(effective_record_sessions(true, "inherit"));
        assert!(!effective_record_sessions(true, "false"));
        assert!(!effective_record_sessions(false, ""));
        assert!(effective_record_sessions(false, "true"));

        let inner = "incus exec 'my-box' --cwd /workspace -- bash -l";
        let log_file = "/tmp/dir with 'quote'/sessions/my-box/1700000001.log";
        let timing_file = "/tmp/dir with 'quote'/sessions/my-box/1700000001.timing";
        let wrapped = build_session_wrap_command(
            inner,
            log_file,
            timing_file,
            "/usr/bin/aibox",
            "/tmp/sessions",
            true,
        );
        assert!(wrapped.contains("script -q -f --log-out"));
        assert!(wrapped.contains("--log-timing"));
        assert!(wrapped.contains("--redact-session"));
        assert!(wrapped.contains("dir with '\\''quote'\\''"));

        let fallback = build_session_wrap_command(
            inner,
            log_file,
            timing_file,
            "/usr/bin/aibox",
            "/tmp/sessions",
            false,
        );
        assert_eq!(fallback, inner);
    }

    #[test]
    fn ansi_stripping_200kb_cap_and_redaction() {
        let ansi_raw = "\x1b]0;user@dev:/workspace\x07\
             \x1b[32muser@dev\x1b[0m:\x1b[34m/workspace\x1b[0m$ echo ok\r\n\
             \x1b[2J\x1b[1;1Hok\r\n";
        let clean = strip_ansi_sequences(ansi_raw);
        assert!(!clean.contains('\x1b'));
        assert!(!clean.contains('\r'));
        assert!(clean.contains("user@dev:/workspace$ echo ok\nok\n"));

        let mut huge = vec![b'A'; 300 * 1024];
        huge.extend_from_slice(b"\nFINAL_TAIL_MARKER\n");
        let formatted = format_recorded_session_text(&huge, 200 * 1024);
        assert!(formatted.starts_with("[... truncated to last 200 KB ...]"));
        assert!(formatted.contains("FINAL_TAIL_MARKER"));
        assert!(formatted.len() <= 200 * 1024 + 128);

        let dir = TempDir::new("redact");
        let env_alpha = dir.path.join("sessions/alpha");
        fs::create_dir_all(&env_alpha).unwrap();
        let red_path = env_alpha.join("1700000099.log");
        fs::write(&red_path, "token=sk-live-secret-12345\nother=safe\n").unwrap();
        assert!(redact_session_log_file(
            red_path.to_str().unwrap(),
            &["sk-live-secret-12345".to_string()]
        ));
        let after = fs::read_to_string(&red_path).unwrap();
        assert!(!after.contains("sk-live-secret-12345"));
        assert!(after.contains("[REDACTED]"));
        let mode = fs::metadata(&red_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode & 0o077, 0);
    }

    #[test]
    fn retention_pruning_per_env_and_global_size_cap() {
        let dir = TempDir::new("prune");
        let sess_root = dir.path.join("sessions");
        let env_alpha = sess_root.join("alpha");
        let env_beta = sess_root.join("beta");
        fs::create_dir_all(&env_alpha).unwrap();
        fs::create_dir_all(&env_beta).unwrap();

        let base_secs = 1_793_491_200i64; // 2026-11-01 00:00:00 UTC
        for i in 1..=25 {
            let epoch_str = (1_700_000_000 + i).to_string();
            let lp = env_alpha.join(format!("{epoch_str}.log"));
            let tp = env_alpha.join(format!("{epoch_str}.timing"));
            fs::write(&lp, vec![b'L'; 100]).unwrap();
            fs::write(&tp, vec![b'T'; 20]).unwrap();
            let ts = (base_secs + i * 60).to_string();
            let _ = Command::new("touch")
                .args(["-d", &format!("@{ts}"), lp.to_str().unwrap()])
                .status();
        }

        let pruned = prune_recorded_sessions(sess_root.to_str().unwrap(), 20, 200 * 1024 * 1024);
        assert_eq!(pruned, 5);
        assert_eq!(
            scan_recorded_sessions(sess_root.to_str().unwrap()).len(),
            20
        );
        assert!(!env_alpha.join("1700000001.log").exists());
        assert!(!env_alpha.join("1700000001.timing").exists());
        assert!(!env_alpha.join("1700000005.log").exists());
        assert!(env_alpha.join("1700000006.log").exists());
        assert!(env_alpha.join("1700000025.log").exists());

        // Global size cap across envs.
        for i in 1..=4 {
            let epoch_str = (1_700_001_000 + i).to_string();
            let lp = env_beta.join(format!("{epoch_str}.log"));
            fs::write(&lp, vec![b'B'; 1024]).unwrap();
            let ts = (base_secs + 5000 + i * 60).to_string();
            let _ = Command::new("touch")
                .args(["-d", &format!("@{ts}"), lp.to_str().unwrap()])
                .status();
        }
        let pruned_global = prune_recorded_sessions(sess_root.to_str().unwrap(), 20, 3000);
        assert!(pruned_global > 0);
    }

    #[test]
    fn delete_session_rejects_outside_root_and_redact_main_works() {
        let dir = TempDir::new("delete-main");
        let sess_root = dir.path.join("sessions");
        let env_dir = sess_root.join("box1");
        fs::create_dir_all(&env_dir).unwrap();
        let lp = env_dir.join("1700000001.log");
        let tp = env_dir.join("1700000001.timing");
        fs::write(&lp, "ok\n").unwrap();
        fs::write(&tp, "0.1 3\n").unwrap();

        assert!(delete_recorded_session(sess_root.to_str().unwrap(), "/etc/passwd", "").is_err());
        assert!(delete_recorded_session(
            sess_root.to_str().unwrap(),
            lp.to_str().unwrap(),
            tp.to_str().unwrap()
        )
        .is_ok());
        assert!(!lp.exists());
        assert!(!tp.exists());
    }

    #[test]
    fn ansi_split_across_chunk_boundaries_and_real_terminal_sequences() {
        // Mix of OSC (BEL and ST), CSI (256-color, DEC private mode, cursor),
        // ESC2 (G0 charset), ESC_SHORT (reverse index, next line), CR/LF, and UTF-8.
        let sample = "\x1b]0;htop — /workspace\x07\
                      \x1b[?1049h\x1b[?25l\x1b[1;1H\x1b(B\x1b[38;5;196mCPU:\x1b[0m 12%\r\n\
                      \x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\\r\n\
                      \x1bM\x1bE\x1bHcafé 🚀\x07\r\n\
                      \x1b[?25h\x1b[?1049l";

        let whole = strip_ansi_sequences(sample);
        assert!(!whole.contains('\x1b'));
        assert!(!whole.contains('\r'));
        assert!(!whole.contains('\x07'));
        assert_eq!(whole, "CPU: 12%\nlink\ncafé 🚀\n");

        // Feed 1 byte at a time and 3 bytes at a time to split every escape sequence.
        for chunk_size in [1usize, 3, 5, 7] {
            let mut stripper = StreamingAnsiStripper::new();
            let mut out = Vec::new();
            for chunk in sample.as_bytes().chunks(chunk_size) {
                stripper.feed_into(chunk, &mut out);
            }
            stripper.finish_into(&mut out);
            assert_eq!(
                String::from_utf8(out).unwrap(),
                whole,
                "mismatch for chunk_size={chunk_size}"
            );
        }
    }

    #[test]
    fn large_50mb_session_redaction_and_tail_read() {
        let dir = TempDir::new("large-50mb");
        let log_path = dir.path.join("50mb.log");
        let secret = "sk-live-50mb-boundary-secret-99999";

        // Build a ~50 MiB file in 1 MiB blocks without holding 50 MiB in a single String.
        {
            let mut f =
                BufWriter::with_capacity(STREAM_CHUNK_BYTES, File::create(&log_path).unwrap());
            let filler = vec![b'x'; 64 * 1024 - 1];
            // Secret #1 at the very start
            f.write_all(b"start=").unwrap();
            f.write_all(secret.as_bytes()).unwrap();
            f.write_all(b"\n").unwrap();
            // Pad to just before an exact 64 KiB chunk boundary, then split secret #2 across it
            let pos_before_boundary = 64 * 1024 - 7 - (6 + secret.len() + 1);
            f.write_all(&filler[..pos_before_boundary]).unwrap();
            f.write_all(secret.as_bytes()).unwrap();
            f.write_all(b"\n").unwrap();

            // Write remaining ~50 MiB
            let line = vec![b'A'; 1023];
            for _ in 0..(50 * 1024) {
                f.write_all(&line).unwrap();
                f.write_all(b"\n").unwrap();
            }
            // Secret #3 + tail marker at the very end
            f.write_all(b"end=").unwrap();
            f.write_all(secret.as_bytes()).unwrap();
            f.write_all(b"\nFINAL_50MB_TAIL\n").unwrap();
            f.flush().unwrap();
        }

        let file_size = fs::metadata(&log_path).unwrap().len();
        assert!(file_size > 50 * 1024 * 1024);

        let secrets = vec![secret.to_string()];
        assert!(redact_session_log_file(
            log_path.to_str().unwrap(),
            &secrets
        ));
        assert!(!redact_session_log_file(
            log_path.to_str().unwrap(),
            &secrets
        ));

        let mode = fs::metadata(&log_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        // Stream-verify no secret remains and exactly 3 [REDACTED] markers exist.
        let mut reader =
            BufReader::with_capacity(STREAM_CHUNK_BYTES, File::open(&log_path).unwrap());
        let mut content_tail = String::new();
        let mut buf = [0u8; STREAM_CHUNK_BYTES];
        let mut redacted_count = 0usize;
        let mut overlap = String::new();
        loop {
            let n = reader.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            let chunk_str = String::from_utf8_lossy(&buf[..n]);
            let combined = format!("{overlap}{chunk_str}");
            assert!(!combined.contains(secret), "secret leaked in 50MB file");
            redacted_count += combined.matches("[REDACTED]").count();
            // Keep small overlap so we don't double-count or miss boundary markers
            let keep = combined.len().min(16);
            overlap = combined[combined.len() - keep..].to_string();
            redacted_count -= overlap.matches("[REDACTED]").count();
            content_tail = combined;
        }
        redacted_count += content_tail.matches("[REDACTED]").count().min(1);
        assert!(redacted_count >= 3);

        // Tail read on 50 MB file returns truncated 200 KB view with final marker.
        let viewed = read_recorded_session_log(log_path.to_str().unwrap(), false).unwrap();
        assert!(viewed.starts_with("[... truncated to last 200 KB ...]"));
        assert!(viewed.contains("FINAL_50MB_TAIL"));
        assert!(!viewed.contains(secret));
    }

    #[test]
    fn sessions_index_created_reused_and_refreshed() {
        let dir = TempDir::new("idx");
        let sess_root = dir.path.join("sessions");
        let env_alpha = sess_root.join("alpha");
        fs::create_dir_all(&env_alpha).unwrap();

        for i in 1..=3 {
            fs::write(env_alpha.join(format!("170000000{i}.log")), b"hello\n").unwrap();
        }

        let pruned = prune_recorded_sessions(sess_root.to_str().unwrap(), 2, 1024 * 1024);
        assert_eq!(pruned, 1);

        let idx_path = sess_root.join(SESSIONS_INDEX_FILE);
        assert!(idx_path.exists());
        let idx = load_session_index(&idx_path).expect("index loaded");
        assert_eq!(idx.envs.get("alpha").map(|e| e.entries.len()), Some(2));

        // Second prune with no changes reuses index and deletes 0.
        assert_eq!(
            prune_recorded_sessions(sess_root.to_str().unwrap(), 2, 1024 * 1024),
            0
        );

        // Adding a new env dir refreshes the index.
        let env_beta = sess_root.join("beta");
        fs::create_dir_all(&env_beta).unwrap();
        fs::write(env_beta.join("1700000010.log"), b"beta1\n").unwrap();
        fs::write(env_beta.join("1700000011.log"), b"beta2\n").unwrap();
        fs::write(env_beta.join("1700000012.log"), b"beta3\n").unwrap();
        assert_eq!(
            prune_recorded_sessions(sess_root.to_str().unwrap(), 2, 1024 * 1024),
            1
        );
        let idx2 = load_session_index(&idx_path).expect("index reloaded");
        assert_eq!(idx2.envs.get("beta").map(|e| e.entries.len()), Some(2));
    }
}
