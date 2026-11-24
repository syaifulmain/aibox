use std::fs;
use std::os::unix::fs::PermissionsExt;

use chrono::{Local, Utc};
use serde_json::Value;

use crate::cli::commands::{require_environment, run_script_with_task_lock, to_json_qt_indented};
use crate::cli::{CliIo, ExitCode, ParsedArgs};
use crate::engine::Engine;
use crate::history::{record_operation_history, RecordOperationParams};
use crate::incus::list::Environment;
use crate::network::acl::build_allowlist_refresh_if_needed;
use crate::network::allowlist::SystemResolver;
use crate::process::CommandRunner;
use crate::provision::lifecycle::{
    build_prepare_script, enter_incus_exec_args, exec_incus_command_args, start_environment_script,
    stop_environment_script,
};
use crate::secrets::inject::push_environment_secrets_to_container;
use crate::session::{
    prepare_recorded_session_paths, prune_recorded_sessions, redact_session_log_file,
    sessions_dir_path,
};
use crate::snapshot::auto::decide_auto_snapshot;
use crate::snapshot::workspace::container_workspace_path_for;
use crate::workspace::file_access::attach_file_access_fields;

fn build_auto_snapshot_if_needed(engine: &Engine, env: &Environment) -> String {
    let now = Local::now();
    let now_epoch = now.timestamp();
    let now_local_stamp = now.format("%Y%m%d-%H%M%S").to_string();
    decide_auto_snapshot(
        &env.name,
        env.is_template,
        env.auto_snapshot_enabled,
        &env.snapshots,
        &env.workspace_path,
        env.workspace_snapshot_enabled,
        engine.config.auto_snapshot_keep,
        None,
        now_epoch,
        &now_local_stamp,
    )
    .unwrap_or_default()
}

fn build_allowlist_refresh_for_env(env: &Environment) -> String {
    let resolver = SystemResolver;
    build_allowlist_refresh_if_needed(
        &env.name,
        &env.network_mode,
        &env.network_allowlist,
        env.egress_recording,
        &resolver,
    )
}

fn prepare_environment_script(engine: &Engine, env: &Environment) -> String {
    let auto_snap = build_auto_snapshot_if_needed(engine, env);
    let allow_refresh = build_allowlist_refresh_for_env(env);
    build_prepare_script(&env.name, &auto_snap, &allow_refresh)
}

fn start_environment_script_for(engine: &Engine, env: &Environment) -> String {
    let auto_snap = build_auto_snapshot_if_needed(engine, env);
    let allow_refresh = build_allowlist_refresh_for_env(env);
    start_environment_script(&env.name, &auto_snap, &allow_refresh)
}

fn stop_environment_script_for(
    engine: &Engine,
    env: &Environment,
    include_auto_snapshot: bool,
) -> String {
    let auto_snap = if include_auto_snapshot {
        build_auto_snapshot_if_needed(engine, env)
    } else {
        String::new()
    };
    stop_environment_script(&env.name, include_auto_snapshot, &auto_snap)
}

fn find_executable(name: &str) -> bool {
    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            if let Ok(meta) = fs::metadata(&candidate) {
                if meta.permissions().mode() & 0o111 != 0 {
                    return true;
                }
            }
        }
    }
    false
}

fn run_pre_steps_for_exec(
    engine: &Engine,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
    env: &Environment,
    label: &str,
) -> i32 {
    if !engine.acquire_cli_task_lock(label, &env.name) {
        io.write_stderr(b"Error: Another AIBox background task is currently running.\n");
        return ExitCode::Busy.as_i32();
    }

    let script = prepare_environment_script(engine, env);
    let out = runner.run("/bin/bash", &["-c", &script]);
    let mut combined = String::new();
    combined.push_str(&out.std_out);
    if !out.std_err.is_empty() {
        combined.push_str(&out.std_err);
    }

    if combined.contains("Warning:") {
        let started_at = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let secret_vals = engine.secrets.all_secret_values();
        let _ = record_operation_history(&RecordOperationParams {
            state_dir: &engine.paths.state_dir,
            description: &format!("Auto-snapshot warning for '{}'", env.name),
            env_name: &env.name,
            status: "warning",
            exit_code: out.exit_code,
            started_at_iso: &started_at,
            log_bytes: combined.as_bytes(),
            secret_values: &secret_vals,
            reason: "",
            max_entries: 50,
        });
        io.write_stderr(combined.as_bytes());
    }
    engine.release_cli_task_lock();

    if out.exit_code != 0 {
        if !combined.is_empty() && !combined.contains("Warning:") {
            io.write_stderr(combined.as_bytes());
        }
        io.write_stderr(
            format!("Error: Failed to prepare environment '{}'.\n", env.name).as_bytes(),
        );
        return ExitCode::Failed.as_i32();
    }
    ExitCode::Ok.as_i32()
}

fn push_secrets_for_env(engine: &Engine, env: &Environment) -> Result<String, String> {
    let resolved =
        engine
            .secrets
            .resolve_environment_secrets(&env.name, &env.secret_names, false)?;
    Ok(push_environment_secrets_to_container(&env.name, &resolved)?.unwrap_or_default())
}

pub fn run_list(engine: &mut Engine, io: &mut dyn CliIo, runner: &dyn CommandRunner) -> i32 {
    if let Err(err) = engine.refresh_sync(runner) {
        io.write_stderr(format!("Error: {err}\n").as_bytes());
        return ExitCode::Failed.as_i32();
    }

    let mut rows: Vec<Value> = Vec::with_capacity(engine.environments.len());
    for (idx, env) in engine.environments.iter().enumerate() {
        let mut val = serde_json::to_value(env).unwrap_or(Value::Null);
        attach_file_access_fields(&mut val, engine.file_access.envs.get(idx));
        rows.push(val);
    }

    let json_text = to_json_qt_indented(&rows).unwrap_or_else(|_| "[\n]\n".to_string());
    io.write_stdout(json_text.as_bytes());
    ExitCode::Ok.as_i32()
}

pub fn run_enter(
    engine: &mut Engine,
    parsed: &ParsedArgs,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
) -> i32 {
    let env = match require_environment(engine, io, runner, &parsed.env_name, true) {
        Ok(e) => e,
        Err(rc) => return rc,
    };

    let pre_rc = run_pre_steps_for_exec(
        engine,
        io,
        runner,
        &env,
        &format!("Entering '{}' (CLI)", parsed.env_name),
    );
    if pre_rc != 0 {
        return pre_rc;
    }

    let remote_env_path = match push_secrets_for_env(engine, &env) {
        Ok(p) => p,
        Err(err) => {
            io.write_stderr(format!("Error: {err}\n").as_bytes());
            return ExitCode::Failed.as_i32();
        }
    };

    let display = std::env::var("DISPLAY").unwrap_or_default();
    let term = std::env::var("TERM").unwrap_or_default();
    let c_ws_path = container_workspace_path_for(&parsed.env_name, false);
    let exec_args = enter_incus_exec_args(
        &parsed.env_name,
        &remote_env_path,
        &c_ws_path,
        &display,
        &term,
    );

    if env.record_enabled && find_executable("script") {
        if let Some((log_file, timing_file)) =
            prepare_recorded_session_paths(&engine.paths.state_dir, &parsed.env_name)
        {
            let mut quoted_cmd = String::from("incus");
            for arg in &exec_args {
                let q = arg.replace('\'', "'\\''");
                quoted_cmd.push_str(&format!(" '{q}'"));
            }
            let script_args = vec![
                "-q".to_string(),
                "-f".to_string(),
                "--log-out".to_string(),
                log_file.clone(),
                "--log-timing".to_string(),
                timing_file,
                "-c".to_string(),
                quoted_cmd,
            ];
            let exit_code = io.run_forwarded("script", &script_args);
            redact_session_log_file(&log_file, &engine.secrets.all_secret_values());
            let _ = fs::set_permissions(&log_file, fs::Permissions::from_mode(0o600));
            prune_recorded_sessions(
                &sessions_dir_path(&engine.paths.state_dir),
                20,
                50 * 1024 * 1024,
            );
            return exit_code;
        }
    }

    io.exec_replace("incus", &exec_args)
}

pub fn run_exec(
    engine: &mut Engine,
    parsed: &ParsedArgs,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
) -> i32 {
    let env = match require_environment(engine, io, runner, &parsed.env_name, true) {
        Ok(e) => e,
        Err(rc) => return rc,
    };

    let pre_rc = run_pre_steps_for_exec(
        engine,
        io,
        runner,
        &env,
        &format!("Executing command in '{}' (CLI)", parsed.env_name),
    );
    if pre_rc != 0 {
        return pre_rc;
    }

    let remote_env_path = match push_secrets_for_env(engine, &env) {
        Ok(p) => p,
        Err(err) => {
            io.write_stderr(format!("Error: {err}\n").as_bytes());
            return ExitCode::Failed.as_i32();
        }
    };

    let c_ws_path = container_workspace_path_for(&parsed.env_name, false);
    let args = exec_incus_command_args(
        &parsed.env_name,
        &parsed.exec_command,
        &remote_env_path,
        &c_ws_path,
    );
    io.run_forwarded("incus", &args)
}

pub fn run_start(
    engine: &mut Engine,
    parsed: &ParsedArgs,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
) -> i32 {
    let env = match require_environment(engine, io, runner, &parsed.env_name, true) {
        Ok(e) => e,
        Err(rc) => return rc,
    };
    if engine.is_task_lock_active().0 {
        io.write_stderr(b"Error: Another AIBox background task is currently running.\n");
        return ExitCode::Busy.as_i32();
    }
    if env.running {
        io.write_stdout(
            format!("Environment '{}' is already running.\n", parsed.env_name).as_bytes(),
        );
        return ExitCode::Ok.as_i32();
    }
    let script = start_environment_script_for(engine, &env);
    run_script_with_task_lock(
        engine,
        io,
        runner,
        &format!("Starting '{}'", parsed.env_name),
        &parsed.env_name,
        &script,
        true,
    )
}

pub fn run_stop(
    engine: &mut Engine,
    parsed: &ParsedArgs,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
) -> i32 {
    let env = match require_environment(engine, io, runner, &parsed.env_name, true) {
        Ok(e) => e,
        Err(rc) => return rc,
    };
    if engine.is_task_lock_active().0 {
        io.write_stderr(b"Error: Another AIBox background task is currently running.\n");
        return ExitCode::Busy.as_i32();
    }
    if !env.running {
        io.write_stdout(
            format!("Environment '{}' is already stopped.\n", parsed.env_name).as_bytes(),
        );
        return ExitCode::Ok.as_i32();
    }
    let script = stop_environment_script_for(engine, &env, false);
    run_script_with_task_lock(
        engine,
        io,
        runner,
        &format!("Stopping '{}'", parsed.env_name),
        &parsed.env_name,
        &script,
        true,
    )
}
