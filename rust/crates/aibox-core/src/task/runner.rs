use std::fs;
use std::path::Path;

use chrono::Local;

use crate::task::files::{
    task_log_path, task_result_path, task_script_path, write_task_meta, TaskMeta,
};
use crate::util::paths::{atomic_write, atomic_write_with_mode};
use crate::util::shell::shell_quote;

pub fn build_task_script_wrapper(script: &str, description: &str) -> String {
    let q_done = shell_quote(&format!("{description} — Completed"));
    let q_fail = shell_quote(&format!("{description} — Failed"));
    format!(
        r#"#!/usr/bin/env bash
(
{script}
)
CODE=$?
echo "@@EXIT:$CODE"
if command -v notify-send >/dev/null 2>&1; then
    if [ "$CODE" -eq 0 ]; then
        notify-send -a "AIBox" "AIBox" {q_done} 2>/dev/null || true
    else
        notify-send -u critical -a "AIBox" "AIBox" {q_fail} 2>/dev/null || true
    fi
fi
exit $CODE
"#
    )
}

pub fn build_detached_launch_command(script_path: &Path, state_dir: &Path) -> String {
    let q_script = shell_quote(&script_path.to_string_lossy());
    let q_log = shell_quote(&task_log_path(state_dir).to_string_lossy());
    let q_pid_tmp = shell_quote(&state_dir.join("task.pid.tmp").to_string_lossy());
    let q_pid = shell_quote(&state_dir.join("task.pid").to_string_lossy());
    format!(
        "setsid {q_script} > {q_log} 2>&1 < /dev/null & PID=$!; START=$(sed 's/.*) //' \"/proc/$PID/stat\" 2>/dev/null | awk '{{print $20}}'); printf '%s %s\\n' \"$PID\" \"$START\" > {q_pid_tmp} && mv -f {q_pid_tmp} {q_pid}"
    )
}

pub struct PrepareShellTaskParams<'a> {
    pub state_dir: &'a Path,
    pub description: &'a str,
    pub script: &'a str,
    pub created_env_name: &'a str,
    pub target_env_name: &'a str,
    pub downloading_os_id: &'a str,
    pub reason: &'a str,
}

/// Persists `task.meta`, truncates `task.log`, writes executable `task.sh`,
/// and returns the `/bin/sh -c` launch command string.
pub fn prepare_shell_task(params: &PrepareShellTaskParams<'_>) -> Result<String, String> {
    fs::create_dir_all(params.state_dir).map_err(|e| format!("Failed to create state dir: {e}"))?;

    let target_env = if params.target_env_name.is_empty() {
        params.created_env_name
    } else {
        params.target_env_name
    };
    let meta = TaskMeta {
        description: params.description.to_string(),
        env_name: params.created_env_name.to_string(),
        target_env_name: target_env.to_string(),
        downloading_os_id: Some(params.downloading_os_id.to_string()),
        started_at: Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
        reason: if params.reason.is_empty() {
            None
        } else {
            Some(params.reason.to_string())
        },
    };
    write_task_meta(params.state_dir, &meta)?;

    let _ = fs::remove_file(task_result_path(params.state_dir));
    atomic_write(&task_log_path(params.state_dir), b"")
        .map_err(|e| format!("Failed to truncate task.log: {e}"))?;

    let script_path = task_script_path(params.state_dir);
    let wrapped = build_task_script_wrapper(params.script, params.description);
    atomic_write_with_mode(&script_path, wrapped.as_bytes(), Some(0o700))
        .map_err(|e| format!("Failed to write task.sh: {e}"))?;

    Ok(build_detached_launch_command(
        &script_path,
        params.state_dir,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("aibox_task_runner_{name}_{nanos}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    #[test]
    fn prepare_shell_task_writes_files_and_executable_script() {
        let dir = temp_dir("prepare");
        let cmd = prepare_shell_task(&PrepareShellTaskParams {
            state_dir: &dir,
            description: "Starting 'dev'",
            script: "echo hello",
            created_env_name: "dev",
            target_env_name: "dev",
            downloading_os_id: "",
            reason: "",
        })
        .expect("prepare");

        assert!(cmd.contains("setsid"));
        assert!(dir.join("task.meta").exists());
        assert!(dir.join("task.log").exists());
        let sh_path = dir.join("task.sh");
        assert!(sh_path.exists());
        let mode = fs::metadata(&sh_path).expect("meta").permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        let content = fs::read_to_string(&sh_path).expect("read sh");
        assert!(content.contains("echo hello"));
        assert!(content.contains("@@EXIT:$CODE"));

        let _ = fs::remove_dir_all(&dir);
    }
}
