use crate::provision::ProvisionError;
use crate::snapshot::workspace::workspace_mount_migration_script;
use crate::util::paths::CONTAINER_WORKSPACE_PATH;
use crate::util::shell::shell_quote;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToggleStartStopPlan {
    pub is_stopping: bool,
    pub title: String,
    pub script: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateTaskPlan {
    pub title: String,
    pub script: String,
}

pub fn build_prepare_script(name: &str, auto_snap: &str, allow_refresh: &str) -> String {
    let q_name = shell_quote(name);
    let migrate = workspace_mount_migration_script(name);
    format!(
        "\nif command -v xhost >/dev/null 2>&1 && [[ -n \"${{DISPLAY:-}}\" ]]; then\n    xhost +local: >/dev/null 2>&1 || true\nfi\n{auto_snap}\n{allow_refresh}\n{migrate}\nSTATUS=$(incus info {q_name} | awk '/^Status:/ {{print $2}}')\nif [[ \"$STATUS\" != \"RUNNING\" ]]; then\n    incus start {q_name}\n    sleep 1\nfi\nincus config set {q_name} user.aibox.lastused \"$(date +%s)\" >/dev/null 2>&1 || true\n"
    )
}

pub fn start_environment_script(name: &str, auto_snap: &str, allow_refresh: &str) -> String {
    let q_name = shell_quote(name);
    let migrate = workspace_mount_migration_script(name);
    let mut script =
        String::from("set -euo pipefail\necho '@@STAGE:20:Preparing environment...'\n");
    if !auto_snap.is_empty() {
        script.push_str(auto_snap);
        script.push('\n');
    }
    if !allow_refresh.is_empty() {
        script.push_str(allow_refresh);
        script.push('\n');
    }
    script.push_str(&format!(
        "echo '@@STAGE:55:Starting container...'\n{migrate}\nincus start {q_name} && sleep 1\n\
         incus config set {q_name} user.aibox.lastused \"$(date +%s)\" >/dev/null 2>&1 || true\n\
         echo '@@STAGE:100:Started.'"
    ));
    script
}

pub fn stop_environment_script(name: &str, include_auto_snapshot: bool, auto_snap: &str) -> String {
    let q_name = shell_quote(name);
    let mut script = String::from("set -euo pipefail\n");
    if include_auto_snapshot && !auto_snap.is_empty() {
        script.push_str(auto_snap);
        script.push('\n');
    }
    script.push_str(&format!(
        "echo '@@STAGE:40:Stopping container...'\nincus stop {q_name}\necho '@@STAGE:100:Stopped.'"
    ));
    script
}

pub fn enter_incus_exec_args(
    name: &str,
    remote_env_path: &str,
    container_workspace_path: &str,
    display: &str,
    term: &str,
) -> Vec<String> {
    let display_val = if display.trim().is_empty() {
        ":0"
    } else {
        display
    };
    let term_val = if term.trim().is_empty() {
        "xterm-256color"
    } else {
        term
    };
    let shell_cmd = if remote_env_path.is_empty() {
        "if command -v bash >/dev/null 2>&1; then exec bash -l; else exec sh; fi".to_string()
    } else {
        let q_env = shell_quote(remote_env_path);
        format!(
            "if [ -f {q_env} ]; then set -a; . {q_env}; set +a; rm -f {q_env}; fi; if command -v bash >/dev/null 2>&1; then exec bash -l; else exec sh; fi"
        )
    };
    vec![
        "exec".to_string(),
        name.to_string(),
        "--cwd".to_string(),
        container_workspace_path.to_string(),
        "--env".to_string(),
        format!("DISPLAY={display_val}"),
        "--env".to_string(),
        format!("TERM={term_val}"),
        "--".to_string(),
        "sh".to_string(),
        "-c".to_string(),
        shell_cmd,
    ]
}

pub fn exec_incus_command_args(
    name: &str,
    cmd: &[String],
    remote_env_path: &str,
    container_workspace_path: &str,
) -> Vec<String> {
    let mut args = vec![
        "exec".to_string(),
        name.to_string(),
        "--cwd".to_string(),
        container_workspace_path.to_string(),
        "--".to_string(),
    ];
    if remote_env_path.is_empty() {
        args.extend(cmd.iter().cloned());
    } else {
        args.push("sh".to_string());
        args.push("-c".to_string());
        args.push(
            "AIBOX_ENV=\"$1\"; shift; if [ -f \"$AIBOX_ENV\" ]; then set -a; . \"$AIBOX_ENV\"; set +a; rm -f \"$AIBOX_ENV\"; fi; exec \"$@\"".to_string(),
        );
        args.push("aibox-exec".to_string());
        args.push(remote_env_path.to_string());
        args.extend(cmd.iter().cloned());
    }
    args
}

pub fn build_enter_command(name: &str, remote_env_path: &str) -> String {
    let q_name = shell_quote(name);
    let q_ws = shell_quote(CONTAINER_WORKSPACE_PATH);
    let ws_mount_arg = format!(
        "\"$(incus config device get {q_name} workspace path 2>/dev/null || echo {q_ws})\""
    );
    if remote_env_path.is_empty() {
        format!(
            "incus exec {q_name} --cwd {ws_mount_arg} --env DISPLAY=\"${{DISPLAY:-:0}}\" --env TERM=\"${{TERM:-xterm-256color}}\" -- bash -l || incus exec {q_name} --cwd {ws_mount_arg} -- sh"
        )
    } else {
        let q_env = shell_quote(remote_env_path);
        format!(
            "incus exec {q_name} --cwd {ws_mount_arg} --env DISPLAY=\"${{DISPLAY:-:0}}\" --env TERM=\"${{TERM:-xterm-256color}}\" -- sh -c 'F=\"$1\"; if [ -f \"$F\" ]; then set -a; . \"$F\"; set +a; rm -f \"$F\"; fi; exec bash -l' aibox-enter {q_env} || incus exec {q_name} --cwd {ws_mount_arg} -- sh -c 'F=\"$1\"; if [ -f \"$F\" ]; then set -a; . \"$F\"; set +a; rm -f \"$F\"; fi; exec sh' aibox-enter {q_env}"
        )
    }
}

pub fn ai_exec_command_text(name: &str, has_secrets: bool, workspace_mount_path: &str) -> String {
    if name.is_empty() {
        return String::new();
    }
    if has_secrets {
        return format!("aibox exec {name} -- bash");
    }
    let trimmed = workspace_mount_path.trim();
    let ws = if trimmed.is_empty() {
        CONTAINER_WORKSPACE_PATH
    } else {
        trimmed
    };
    format!("incus exec {name} --cwd {ws} -- bash")
}

pub fn prepare_toggle_start_stop(
    name: &str,
    is_template: bool,
    is_running: bool,
    auto_snap: &str,
    allow_refresh: &str,
) -> Result<ToggleStartStopPlan, ProvisionError> {
    if is_template {
        return Err(ProvisionError::TemplateStartNotAllowed);
    }
    if is_running {
        Ok(ToggleStartStopPlan {
            is_stopping: true,
            title: format!("Stopping '{name}'"),
            script: stop_environment_script(name, false, auto_snap),
        })
    } else {
        Ok(ToggleStartStopPlan {
            is_stopping: false,
            title: format!("Starting '{name}'"),
            script: start_environment_script(name, auto_snap, allow_refresh),
        })
    }
}

pub fn set_environment_template_script(
    name: &str,
    is_running: bool,
    is_template: bool,
) -> TemplateTaskPlan {
    let q_name = shell_quote(name);
    if is_template {
        let mut script =
            String::from("set -euo pipefail\necho '@@STAGE:30:Preparing template...'\n");
        if is_running {
            script.push_str(&format!("incus stop {q_name}\n"));
        }
        script.push_str(&format!(
            "incus config set {q_name} user.aibox.template true\necho '@@STAGE:100:Marked as template.'"
        ));
        TemplateTaskPlan {
            title: format!("Marking '{name}' as template"),
            script,
        }
    } else {
        let script = format!(
            "set -euo pipefail\necho '@@STAGE:50:Unmarking template...'\n\
             incus config unset {q_name} user.aibox.template >/dev/null 2>&1 || true\n\
             echo '@@STAGE:100:Template unmarked.'"
        );
        TemplateTaskPlan {
            title: format!("Unmarking '{name}' as template"),
            script,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_and_prepare_scripts_migrate_legacy_workspace_mount() {
        let start = start_environment_script("box", "", "");
        assert!(start.contains("workspace path='/home/workspace'"));
        let prepare = build_prepare_script("box", "", "");
        assert!(prepare.contains("workspace path='/home/workspace'"));
    }
}
