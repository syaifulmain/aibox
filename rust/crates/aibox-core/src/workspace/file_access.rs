use std::collections::HashSet;
use std::fs;
use std::net::TcpListener;
use std::path::Path;

use serde_json::{json, Value};

use crate::util::names::sanitize_name;
use crate::util::paths::atomic_write;
use crate::util::shell::shell_quote;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileAccessState {
    pub name: String,
    pub pid: i64,
    pub port: i32,
    pub started_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileAccessEnvInput {
    pub name: String,
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileAccessEnvOutput {
    pub active: bool,
    pub port: i32,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingFileAccessKill {
    pub pid: i64,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyFileAccessResult {
    pub envs: Vec<FileAccessEnvOutput>,
    pub pending_kills: Vec<PendingFileAccessKill>,
}

pub fn file_access_dir_path(state_dir: &str) -> String {
    format!("{state_dir}/file-access")
}

pub fn file_access_state_path(state_dir: &str, name: &str) -> String {
    format!(
        "{}/{}.json",
        file_access_dir_path(state_dir),
        sanitize_name(name)
    )
}

pub fn file_access_log_path(state_dir: &str, name: &str) -> String {
    format!(
        "{}/{}.log",
        file_access_dir_path(state_dir),
        sanitize_name(name)
    )
}

pub fn load_file_access_state(state_dir: &str, name: &str) -> Option<FileAccessState> {
    let path = file_access_state_path(state_dir, name);
    load_file_access_state_from_file(Path::new(&path))
}

fn load_file_access_state_from_file(path: &Path) -> Option<FileAccessState> {
    let bytes = fs::read(path).ok()?;
    let val: Value = serde_json::from_slice(&bytes).ok()?;
    let obj = val.as_object()?;
    Some(FileAccessState {
        name: obj
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        pid: obj.get("pid").and_then(Value::as_i64).unwrap_or(0),
        port: obj.get("port").and_then(Value::as_i64).unwrap_or(0) as i32,
        started_at: obj
            .get("startedAt")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    })
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}

pub fn file_access_process_alive(pid: i64, name: &str) -> bool {
    if pid <= 0 {
        return false;
    }
    if unsafe { libc::kill(pid as libc::pid_t, 0) } != 0 {
        return false;
    }

    // Guard against PID reuse: only trust processes that still look like the
    // `incus file mount <name> ...` listener we started.
    let cmdline_path = format!("/proc/{pid}/cmdline");
    let Ok(bytes) = fs::read(cmdline_path) else {
        return false;
    };
    let name_bytes = name.as_bytes();
    let mut has_file = false;
    let mut has_mount = false;
    let mut has_name = false;
    for arg in bytes.split(|&b| b == 0) {
        if arg == b"file" {
            has_file = true;
        } else if arg == b"mount" {
            has_mount = true;
        } else if arg == name_bytes {
            has_name = true;
        } else if contains_bytes(arg, b"incus file mount") && contains_bytes(arg, name_bytes) {
            // The detached `/bin/sh -c 'exec incus file mount ...'` wrapper is
            // still alive for a few milliseconds before exec replaces it.
            has_file = true;
            has_mount = true;
            has_name = true;
        }
    }
    has_file && has_mount && has_name
}

pub fn kill_file_access_if_alive(pid: i64, name: &str, signal: i32) {
    if file_access_process_alive(pid, name) {
        unsafe {
            libc::kill(pid as libc::pid_t, signal as libc::c_int);
        }
    }
}

/// Sends SIGTERM to the listener if alive, removes its state and log files,
/// and returns `Some(pid)` when a follow-up SIGKILL check should be scheduled.
pub fn stop_file_access_process(state_dir: &str, name: &str) -> Option<i64> {
    if name.is_empty() {
        return None;
    }
    let state = load_file_access_state(state_dir, name);
    let pid = state.map(|s| s.pid).unwrap_or(0);
    let killed_pid = if file_access_process_alive(pid, name) {
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGTERM);
        }
        Some(pid)
    } else {
        None
    };
    let _ = fs::remove_file(file_access_state_path(state_dir, name));
    let _ = fs::remove_file(file_access_log_path(state_dir, name));
    killed_pid
}

pub fn apply_file_access_state(
    state_dir: &str,
    envs: &[FileAccessEnvInput],
) -> ApplyFileAccessResult {
    let mut result = ApplyFileAccessResult::default();
    let mut known_files = HashSet::new();

    for env in envs {
        if env.name.is_empty() {
            result.envs.push(FileAccessEnvOutput {
                active: false,
                port: 0,
                url: String::new(),
            });
            continue;
        }
        known_files.insert(sanitize_name(&env.name));

        let state = load_file_access_state(state_dir, &env.name);
        let pid = state.as_ref().map(|s| s.pid).unwrap_or(0);
        let mut port = state.as_ref().map(|s| s.port).unwrap_or(0);
        let mut active = file_access_process_alive(pid, &env.name);

        if !active && state.is_some() {
            let _ = fs::remove_file(file_access_state_path(state_dir, &env.name));
            let _ = fs::remove_file(file_access_log_path(state_dir, &env.name));
            port = 0;
        }
        if active && !env.running {
            if let Some(kpid) = stop_file_access_process(state_dir, &env.name) {
                result.pending_kills.push(PendingFileAccessKill {
                    pid: kpid,
                    name: env.name.clone(),
                });
            }
            active = false;
            port = 0;
        }

        result.envs.push(FileAccessEnvOutput {
            active,
            port: if active { port } else { 0 },
            url: if active {
                file_access_url(port)
            } else {
                String::new()
            },
        });
    }

    // Drop state files whose environment disappeared (deleted outside AIBox).
    let dir = file_access_dir_path(state_dir);
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") || !path.is_file() {
                continue;
            }
            let base = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();
            if known_files.contains(&base) {
                continue;
            }
            let state = load_file_access_state_from_file(&path);
            let state_name = state
                .as_ref()
                .map(|s| s.name.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or(&base)
                .to_string();
            let pid = state.as_ref().map(|s| s.pid).unwrap_or(0);
            if file_access_process_alive(pid, &state_name) {
                unsafe {
                    libc::kill(pid as libc::pid_t, libc::SIGTERM);
                }
            }
            let _ = fs::remove_file(&path);
            let _ = fs::remove_file(file_access_log_path(state_dir, &state_name));
        }
    }

    result
}

pub fn file_access_url(port: i32) -> String {
    if !(1..=65535).contains(&port) {
        return String::new();
    }
    format!("sftp://root@127.0.0.1:{port}/")
}

pub fn choose_file_access_port<F>(
    reserved_ports: &HashSet<i32>,
    mut is_available: Option<F>,
    start_port: i32,
    max_port: i32,
) -> i32
where
    F: FnMut(i32) -> bool,
{
    for port in start_port..=max_port {
        if reserved_ports.contains(&port) {
            continue;
        }
        if let Some(ref mut check) = is_available {
            if !check(port) {
                continue;
            }
        }
        return port;
    }
    0
}

pub fn is_host_port_available(port: i32) -> bool {
    if !(1024..=65535).contains(&port) {
        return false;
    }
    TcpListener::bind(("127.0.0.1", port as u16)).is_ok()
}

pub fn is_local_tcp_port_listening(port: i32) -> bool {
    if !(1..=65535).contains(&port) {
        return false;
    }
    let Ok(content) = fs::read_to_string("/proc/net/tcp") else {
        return false;
    };
    let needle = format!("0100007F:{port:04X}");
    for line in content.lines() {
        let mut fields = line.split_whitespace();
        let _sl = fields.next();
        let Some(local_addr) = fields.next() else {
            continue;
        };
        let _rem_addr = fields.next();
        let Some(state) = fields.next() else {
            continue;
        };
        if local_addr.eq_ignore_ascii_case(&needle) && state == "0A" {
            return true;
        }
    }
    false
}

pub fn validate_enable_file_access(
    name: &str,
    busy: bool,
    env_found: bool,
    is_template: bool,
    running: bool,
) -> Result<(), String> {
    if name.is_empty() {
        return Err("Select an environment first.".to_string());
    }
    if busy {
        return Err(
            "Wait for the current operation to finish before changing root file access."
                .to_string(),
        );
    }
    if !env_found {
        return Err(format!("Environment '{name}' was not found."));
    }
    if is_template {
        return Err(
            "Templates cannot expose root files. Clone or unmark as template first.".to_string(),
        );
    }
    if !running {
        return Err(format!("Start '{name}' before enabling root file access."));
    }
    Ok(())
}

pub fn collect_reserved_file_access_ports(state_dir: &str) -> Vec<i32> {
    let dir = file_access_dir_path(state_dir);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut ports = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") || !path.is_file() {
            continue;
        }
        if let Some(state) = load_file_access_state_from_file(&path) {
            if state.port > 0 {
                ports.push(state.port);
            }
        }
    }
    ports
}

pub fn build_file_access_mount_script(name: &str, port: i32, log_path: &str) -> String {
    let addr = format!("127.0.0.1:{port}");
    format!(
        "exec incus file mount {} --listen {} --no-auth > {} 2>&1",
        shell_quote(name),
        shell_quote(&addr),
        shell_quote(log_path)
    )
}

pub fn write_file_access_state(
    state_dir: &str,
    name: &str,
    pid: i64,
    port: i32,
    started_at_iso: &str,
) -> Result<(), String> {
    let state = json!({
        "name": name,
        "pid": pid,
        "port": port,
        "startedAt": started_at_iso,
    });
    let bytes = serde_json::to_vec(&state).map_err(|e| e.to_string())?;
    let path = file_access_state_path(state_dir, name);
    atomic_write(Path::new(&path), &bytes).map_err(String::from)
}

pub fn read_file_access_failure_detail(log_path: &str) -> String {
    let Ok(content) = fs::read_to_string(log_path) else {
        return String::new();
    };
    content
        .split(['\r', '\n'])
        .rfind(|s| !s.is_empty())
        .map(str::trim)
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_access_url_and_port_selection_match_qt_test() {
        assert_eq!(file_access_url(2222), "sftp://root@127.0.0.1:2222/");
        assert_eq!(file_access_url(0), "");
        assert_eq!(file_access_url(70000), "");

        let reserved: HashSet<i32> = [2222, 2223, 2224].into_iter().collect();
        let mut probes = 0;
        let port = choose_file_access_port(
            &reserved,
            Some(|candidate: i32| {
                probes += 1;
                candidate != 2225
            }),
            2222,
            2299,
        );
        assert_eq!(port, 2226);
        assert_eq!(probes, 2);

        assert_eq!(
            choose_file_access_port::<fn(i32) -> bool>(&HashSet::new(), None, 2222, 2224),
            2222
        );
        let all_reserved: HashSet<i32> = [2222, 2223, 2224].into_iter().collect();
        assert_eq!(
            choose_file_access_port::<fn(i32) -> bool>(&all_reserved, None, 2222, 2224),
            0
        );

        let blocker = TcpListener::bind(("127.0.0.1", 0)).expect("bind ephemeral");
        let busy_port = blocker.local_addr().unwrap().port() as i32;
        let chosen = choose_file_access_port(
            &HashSet::new(),
            Some(is_host_port_available),
            busy_port,
            busy_port + 50,
        );
        assert!(chosen > busy_port);
        assert!(is_host_port_available(chosen));
        drop(blocker);
        assert!(is_host_port_available(busy_port));
    }

    #[test]
    fn validate_enable_file_access_checks_preconditions() {
        assert!(validate_enable_file_access("", false, false, false, false).is_err());
        assert!(validate_enable_file_access("box", true, true, false, true).is_err());
        assert!(validate_enable_file_access("missing-box", false, false, false, false).is_err());
        assert!(validate_enable_file_access("tpl", false, true, true, true).is_err());
        assert!(validate_enable_file_access("stopped", false, true, false, false).is_err());
        assert!(validate_enable_file_access("running", false, true, false, true).is_ok());
    }
}
