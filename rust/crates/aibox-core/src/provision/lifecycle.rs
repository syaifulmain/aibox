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

/// Legacy grant kept only for the old `build_prepare_script` wrapper.
const LEGACY_XHOST_LOCAL: &str = "if command -v xhost >/dev/null 2>&1 && [[ -n \"${DISPLAY:-}\" ]]; then\n    xhost +local: >/dev/null 2>&1 || true\nfi\n";

pub fn build_prepare_script(name: &str, auto_snap: &str, allow_refresh: &str) -> String {
    build_prepare_script_inner(name, auto_snap, allow_refresh, None)
}

/// Production prepare script: applies the desktop bridge after the container is
/// running, so a changed host socket is refreshed on every Enter.
pub fn build_prepare_script_with_desktop(
    name: &str,
    auto_snap: &str,
    allow_refresh: &str,
    desktop_script: &str,
) -> String {
    build_prepare_script_inner(name, auto_snap, allow_refresh, Some(desktop_script))
}

fn build_prepare_script_inner(
    name: &str,
    auto_snap: &str,
    allow_refresh: &str,
    desktop_script: Option<&str>,
) -> String {
    let q_name = shell_quote(name);
    let migrate = workspace_mount_migration_script(name);
    let (legacy_xhost, desktop) = match desktop_script {
        Some(script) => ("", script),
        None => (LEGACY_XHOST_LOCAL, ""),
    };
    let desktop_line = if desktop.is_empty() {
        String::new()
    } else {
        format!("{desktop}\n")
    };
    format!(
        "\n{legacy_xhost}{auto_snap}\n{allow_refresh}\n{migrate}\nSTATUS=$(incus info {q_name} | awk '/^Status:/ {{print $2}}')\nif [[ \"$STATUS\" != \"RUNNING\" ]]; then\n    incus start {q_name}\n    sleep 1\nfi\n{desktop_line}incus config set {q_name} user.aibox.lastused=\"$(date +%s)\" >/dev/null 2>&1 || true\n"
    )
}

pub fn start_environment_script(name: &str, auto_snap: &str, allow_refresh: &str) -> String {
    start_environment_script_with_desktop(name, auto_snap, allow_refresh, "")
}

/// Production start script that applies the desktop bridge after `incus start`.
pub fn start_environment_script_with_desktop(
    name: &str,
    auto_snap: &str,
    allow_refresh: &str,
    desktop_script: &str,
) -> String {
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
    let desktop_line = if desktop_script.is_empty() {
        String::new()
    } else {
        format!("{desktop_script}\n")
    };
    script.push_str(&format!(
        "echo '@@STAGE:55:Starting container...'\n{migrate}\nincus start {q_name} && sleep 1\n{desktop_line}\
         incus config set {q_name} user.aibox.lastused=\"$(date +%s)\" >/dev/null 2>&1 || true\n\
         echo '@@STAGE:100:Started.'"
    ));
    script
}

pub fn stop_environment_script(name: &str, include_auto_snapshot: bool, auto_snap: &str) -> String {
    stop_environment_script_with_session(name, include_auto_snapshot, auto_snap, "")
}

/// Production stop script with an optional bridge teardown snippet.
///
/// The teardown runs before `incus stop` so host proxy processes are gone even
/// if the instance stops slowly. An empty snippet keeps the legacy golden
/// output byte-for-byte identical.
pub fn stop_environment_script_with_session(
    name: &str,
    include_auto_snapshot: bool,
    auto_snap: &str,
    stop_script: &str,
) -> String {
    let q_name = shell_quote(name);
    let mut script = String::from("set -euo pipefail\n");
    if include_auto_snapshot && !auto_snap.is_empty() {
        script.push_str(auto_snap);
        script.push('\n');
    }
    if !stop_script.is_empty() {
        script.push_str(stop_script);
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
    enter_incus_exec_args_inner(
        name,
        remote_env_path,
        container_workspace_path,
        display_val,
        term,
        &[],
    )
}

/// Same as `enter_incus_exec_args`, plus explicit desktop environment pairs.
pub fn enter_incus_exec_args_with_env(
    name: &str,
    remote_env_path: &str,
    container_workspace_path: &str,
    display: &str,
    term: &str,
    extra_env: &[(String, String)],
) -> Vec<String> {
    enter_incus_exec_args_inner(
        name,
        remote_env_path,
        container_workspace_path,
        display,
        term,
        extra_env,
    )
}

fn enter_incus_exec_args_inner(
    name: &str,
    remote_env_path: &str,
    container_workspace_path: &str,
    display: &str,
    term: &str,
    extra_env: &[(String, String)],
) -> Vec<String> {
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
    let mut args = vec![
        "exec".to_string(),
        name.to_string(),
        "--cwd".to_string(),
        container_workspace_path.to_string(),
    ];
    let display_overridden = extra_env.iter().any(|(key, _)| key == "DISPLAY");
    if !display.trim().is_empty() && !display_overridden {
        args.push("--env".to_string());
        args.push(format!("DISPLAY={display}"));
    }
    args.push("--env".to_string());
    args.push(format!("TERM={term_val}"));
    for (key, value) in extra_env {
        args.push("--env".to_string());
        args.push(format!("{key}={value}"));
    }
    args.extend([
        "--".to_string(),
        "sh".to_string(),
        "-c".to_string(),
        shell_cmd,
    ]);
    args
}

pub fn exec_incus_command_args(
    name: &str,
    cmd: &[String],
    remote_env_path: &str,
    container_workspace_path: &str,
) -> Vec<String> {
    exec_incus_command_args_with_env(name, cmd, remote_env_path, container_workspace_path, &[])
}

/// Same as `exec_incus_command_args`, plus explicit desktop environment pairs.
pub fn exec_incus_command_args_with_env(
    name: &str,
    cmd: &[String],
    remote_env_path: &str,
    container_workspace_path: &str,
    extra_env: &[(String, String)],
) -> Vec<String> {
    let mut args = vec![
        "exec".to_string(),
        name.to_string(),
        "--cwd".to_string(),
        container_workspace_path.to_string(),
    ];
    for (key, value) in extra_env {
        args.push("--env".to_string());
        args.push(format!("{key}={value}"));
    }
    args.push("--".to_string());
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
    build_enter_command_inner(name, remote_env_path, true, &[])
}

/// Production Enter command with explicit desktop environment pairs.
pub fn build_enter_command_with_env(
    name: &str,
    remote_env_path: &str,
    extra_env: &[(String, String)],
) -> String {
    build_enter_command_inner(name, remote_env_path, false, extra_env)
}

fn build_enter_command_inner(
    name: &str,
    remote_env_path: &str,
    legacy_display: bool,
    extra_env: &[(String, String)],
) -> String {
    let q_name = shell_quote(name);
    let q_ws = shell_quote(CONTAINER_WORKSPACE_PATH);
    let ws_mount_arg = format!(
        "\"$(incus config device get {q_name} workspace path 2>/dev/null || echo {q_ws})\""
    );
    let env_flags = build_enter_env_flags(legacy_display, extra_env);
    // The legacy wrapper only passed env flags to the bash branch; the
    // production variant must carry the desktop environment into both branches.
    let fallback_env = if legacy_display {
        String::new()
    } else {
        env_flags.clone()
    };
    if remote_env_path.is_empty() {
        format!(
            "incus exec {q_name} --cwd {ws_mount_arg}{env_flags} -- bash -l || incus exec {q_name} --cwd {ws_mount_arg}{fallback_env} -- sh"
        )
    } else {
        let q_env = shell_quote(remote_env_path);
        format!(
            "incus exec {q_name} --cwd {ws_mount_arg}{env_flags} -- sh -c 'F=\"$1\"; if [ -f \"$F\" ]; then set -a; . \"$F\"; set +a; rm -f \"$F\"; fi; exec bash -l' aibox-enter {q_env} || incus exec {q_name} --cwd {ws_mount_arg}{fallback_env} -- sh -c 'F=\"$1\"; if [ -f \"$F\" ]; then set -a; . \"$F\"; set +a; rm -f \"$F\"; fi; exec sh' aibox-enter {q_env}"
        )
    }
}

fn build_enter_env_flags(legacy_display: bool, extra_env: &[(String, String)]) -> String {
    let mut flags = String::new();
    let display_overridden = extra_env.iter().any(|(key, _)| key == "DISPLAY");
    if legacy_display && !display_overridden {
        flags.push_str(" --env DISPLAY=\"${DISPLAY:-:0}\"");
    }
    flags.push_str(" --env TERM=\"${TERM:-xterm-256color}\"");
    for (key, value) in extra_env {
        flags.push_str(&format!(
            " --env {}",
            shell_quote(&format!("{key}={value}"))
        ));
    }
    flags
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
    prepare_toggle_start_stop_with_session(
        name,
        is_template,
        is_running,
        auto_snap,
        allow_refresh,
        "",
    )
}

/// Toggle plan with a combined display/audio bridge snippet applied on Start.
pub fn prepare_toggle_start_stop_with_session(
    name: &str,
    is_template: bool,
    is_running: bool,
    auto_snap: &str,
    allow_refresh: &str,
    session_script: &str,
) -> Result<ToggleStartStopPlan, ProvisionError> {
    prepare_toggle_start_stop_with_session_and_stop(
        name,
        is_template,
        is_running,
        auto_snap,
        allow_refresh,
        session_script,
        "",
    )
}

/// Toggle plan that also tears down host bridge processes before a Stop.
pub fn prepare_toggle_start_stop_with_session_and_stop(
    name: &str,
    is_template: bool,
    is_running: bool,
    auto_snap: &str,
    allow_refresh: &str,
    session_script: &str,
    stop_script: &str,
) -> Result<ToggleStartStopPlan, ProvisionError> {
    if is_template {
        return Err(ProvisionError::TemplateStartNotAllowed);
    }
    if is_running {
        Ok(ToggleStartStopPlan {
            is_stopping: true,
            title: format!("Stopping '{name}'"),
            script: stop_environment_script_with_session(name, false, auto_snap, stop_script),
        })
    } else {
        Ok(ToggleStartStopPlan {
            is_stopping: false,
            title: format!("Starting '{name}'"),
            script: start_environment_script_with_desktop(
                name,
                auto_snap,
                allow_refresh,
                session_script,
            ),
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
            "incus config set {q_name} user.aibox.template=true\necho '@@STAGE:100:Marked as template.'"
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

    #[test]
    fn desktop_script_runs_after_the_container_is_running() {
        let prepare = build_prepare_script_with_desktop("box", "", "", "DESKTOP_APPLY");
        let sleep_at = prepare.find("sleep 1").expect("sleep");
        let desktop_at = prepare.find("DESKTOP_APPLY").expect("desktop");
        let lastused_at = prepare.find("user.aibox.lastused").expect("lastused");
        assert!(sleep_at < desktop_at);
        assert!(desktop_at < lastused_at);
        assert!(!prepare.contains("xhost +local:"));

        let start = start_environment_script_with_desktop("box", "", "", "DESKTOP_APPLY");
        let start_at = start.find("incus start 'box'").expect("start");
        let desktop_at = start.find("DESKTOP_APPLY").expect("desktop");
        let lastused_at = start.find("user.aibox.lastused").expect("lastused");
        assert!(start_at < desktop_at);
        assert!(desktop_at < lastused_at);
    }

    #[test]
    fn env_pairs_are_inserted_after_cwd_and_before_the_command() {
        let env = vec![("DISPLAY".to_string(), ":0".to_string())];
        let args = enter_incus_exec_args_with_env(
            "box",
            "",
            "/home/workspace",
            "",
            "xterm-256color",
            &env,
        );
        let cwd_at = args.iter().position(|arg| arg == "--cwd").expect("cwd");
        let display_at = args
            .iter()
            .position(|arg| arg == "DISPLAY=:0")
            .expect("display");
        let dash_at = args.iter().position(|arg| arg == "--").expect("dash");
        assert!(cwd_at < display_at);
        assert!(display_at < dash_at);

        let cmd = vec!["firefox".to_string()];
        let exec = exec_incus_command_args_with_env("box", &cmd, "", "/home/workspace", &env);
        let cwd_at = exec.iter().position(|arg| arg == "--cwd").expect("cwd");
        let display_at = exec
            .iter()
            .position(|arg| arg == "DISPLAY=:0")
            .expect("display");
        let dash_at = exec.iter().position(|arg| arg == "--").expect("dash");
        assert!(cwd_at < display_at);
        assert!(display_at < dash_at);

        let enter = build_enter_command_with_env("box", "/run/env", &env);
        assert_eq!(enter.matches("DISPLAY=:0").count(), 2);
        assert!(!build_enter_command_with_env("box", "/run/env", &[]).contains("DISPLAY="));
    }
}
