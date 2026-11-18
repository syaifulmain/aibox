use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use chrono::{Local, Utc};
use regex::Regex;
use serde::Serialize;
use serde_json::ser::{Formatter, Serializer};
use serde_json::{json, Value};

use crate::cli::{help_text, CliIo, Command, ParsedArgs};
use crate::engine::Engine;
use crate::health::checks::{run_health_checks, CommandRunner};
use crate::history::{record_operation_history, RecordOperationParams};
use crate::incus::list::Environment;
use crate::network::acl::build_allowlist_refresh_if_needed;
use crate::network::allowlist::SystemResolver;
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
use crate::snapshot::scripts::{create_snapshot_script, restore_snapshot_script};
use crate::snapshot::workspace::{container_workspace_path_for, workspace_snapshot_dir};
use crate::snapshot::{parse_du_bytes_output, parse_snapshot_sizes_json};
use crate::util::names::sanitize_name;
use crate::util::paths::workspace_path_for;
use crate::workspace::diff::{build_workspace_diff_args, process_workspace_diff_output};

struct QtPrettyFormatter {
    current_indent: usize,
}

impl QtPrettyFormatter {
    fn new() -> Self {
        Self { current_indent: 0 }
    }

    fn write_indent<W: ?Sized + io::Write>(&self, writer: &mut W) -> io::Result<()> {
        for _ in 0..self.current_indent {
            writer.write_all(b"    ")?;
        }
        Ok(())
    }
}

impl Formatter for QtPrettyFormatter {
    fn begin_array<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.current_indent += 1;
        writer.write_all(b"[")
    }

    fn end_array<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.current_indent -= 1;
        writer.write_all(b"\n")?;
        self.write_indent(writer)?;
        writer.write_all(b"]")
    }

    fn begin_array_value<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if !first {
            writer.write_all(b",")?;
        }
        writer.write_all(b"\n")?;
        self.write_indent(writer)
    }

    fn begin_object<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.current_indent += 1;
        writer.write_all(b"{")
    }

    fn end_object<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.current_indent -= 1;
        writer.write_all(b"\n")?;
        self.write_indent(writer)?;
        writer.write_all(b"}")
    }

    fn begin_object_key<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if !first {
            writer.write_all(b",")?;
        }
        writer.write_all(b"\n")?;
        self.write_indent(writer)
    }

    fn begin_object_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        writer.write_all(b": ")
    }
}

pub fn to_json_qt_indented<T: Serialize + ?Sized>(value: &T) -> Result<String, serde_json::Error> {
    let mut buf = Vec::new();
    let mut ser = Serializer::with_formatter(&mut buf, QtPrettyFormatter::new());
    value.serialize(&mut ser)?;
    buf.push(b'\n');
    Ok(String::from_utf8(buf).expect("serde_json emits valid UTF-8"))
}

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

fn workspace_path_for_env(engine: &Engine, env: &Environment) -> String {
    if !env.workspace_path.is_empty() {
        env.workspace_path.clone()
    } else {
        workspace_path_for(&engine.paths.workspace_base, &[], &env.name)
    }
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

fn run_script_with_task_lock(
    engine: &Engine,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
    description: &str,
    env_name: &str,
    script: &str,
    stream_progress: bool,
) -> i32 {
    if !engine.acquire_cli_task_lock(description, env_name) {
        io.write_stderr(b"Error: Another AIBox background task is currently running.\n");
        return 3;
    }

    let started_at = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let out = runner.run("/bin/bash", &["-c", script]);

    let mut combined = Vec::new();
    combined.extend_from_slice(out.std_out.as_bytes());
    if !out.std_err.is_empty() {
        if !combined.is_empty() && !combined.ends_with(b"\n") {
            combined.push(b'\n');
        }
        combined.extend_from_slice(out.std_err.as_bytes());
    }

    if stream_progress {
        let stage_re = Regex::new(r"^@@STAGE:(\d+):(.*)$").expect("valid stage regex");
        let text = String::from_utf8_lossy(&combined);
        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(caps) = stage_re.captures(line) {
                let pct = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let msg = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
                io.write_stdout(format!("[{pct}%] {msg}\n").as_bytes());
            } else {
                io.write_stdout(format!("{line}\n").as_bytes());
            }
        }
    }

    let status = if out.exit_code == 0 {
        "success"
    } else {
        "failed"
    };
    let secret_vals = engine.secrets.all_secret_values();
    let _ = record_operation_history(&RecordOperationParams {
        state_dir: &engine.paths.state_dir,
        description,
        env_name,
        status,
        exit_code: out.exit_code,
        started_at_iso: &started_at,
        log_bytes: &combined,
        secret_values: &secret_vals,
        reason: "",
        max_entries: 50,
    });
    engine.release_cli_task_lock();
    if out.exit_code == 0 {
        0
    } else {
        1
    }
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
        return 3;
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
        return 1;
    }
    0
}

fn run_list(engine: &mut Engine, io: &mut dyn CliIo, runner: &dyn CommandRunner) -> i32 {
    if let Err(err) = engine.refresh_sync(runner) {
        io.write_stderr(format!("Error: {err}\n").as_bytes());
        return 1;
    }

    let mut rows: Vec<Value> = Vec::with_capacity(engine.environments.len());
    for (idx, env) in engine.environments.iter().enumerate() {
        let mut val = serde_json::to_value(env).unwrap_or(Value::Null);
        if let Some(obj) = val.as_object_mut() {
            let fa = engine.file_access.envs.get(idx);
            obj.insert(
                "fileAccessActive".to_string(),
                json!(fa.map(|f| f.active).unwrap_or(false)),
            );
            obj.insert(
                "fileAccessPort".to_string(),
                json!(fa.map(|f| f.port).unwrap_or(0)),
            );
            obj.insert(
                "fileAccessUrl".to_string(),
                json!(fa.map(|f| f.url.as_str()).unwrap_or("")),
            );
        }
        rows.push(val);
    }

    let json_text = to_json_qt_indented(&rows).unwrap_or_else(|_| "[\n]\n".to_string());
    io.write_stdout(json_text.as_bytes());
    0
}

fn run_check(parsed: &ParsedArgs, io: &mut dyn CliIo, runner: &dyn CommandRunner) -> i32 {
    let checks = run_health_checks(parsed.fix, runner, "", "", "");
    let mut all_ok = true;
    let mut warning_count = 0i32;
    let mut arr = Vec::with_capacity(checks.len());

    for c in &checks {
        if !c.ok {
            all_ok = false;
        }
        if c.warning {
            warning_count += 1;
        }
        arr.push(json!({
            "id": c.id,
            "title": c.title,
            "ok": c.ok,
            "warning": c.warning,
            "repaired": c.repaired,
            "needsRoot": c.needs_root,
            "detail": c.detail,
            "fixCommand": c.fix_command,
        }));
    }

    if parsed.json_output {
        let root = json!({
            "allOk": all_ok,
            "warningCount": warning_count,
            "checks": arr,
        });
        let json_text = to_json_qt_indented(&root).unwrap_or_else(|_| "{\n}\n".to_string());
        io.write_stdout(json_text.as_bytes());
    } else {
        for c in &checks {
            let tag = if !c.ok {
                "[FAIL] "
            } else if c.warning {
                "[WARN] "
            } else if c.repaired {
                "[FIXED]"
            } else {
                "[OK]   "
            };
            io.write_stdout(format!("{tag} {}: {}\n", c.title, c.detail).as_bytes());
            if !c.ok && !c.fix_command.is_empty() {
                let prefix = if c.needs_root {
                    "Run with sudo:"
                } else {
                    "Fix:"
                };
                io.write_stdout(format!("         {prefix} {}\n", c.fix_command).as_bytes());
            }
        }
    }
    if all_ok {
        0
    } else {
        1
    }
}

fn require_environment(
    engine: &mut Engine,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
    name: &str,
    reject_template: bool,
) -> Result<Environment, i32> {
    if let Err(err) = engine.refresh_sync(runner) {
        io.write_stderr(format!("Error: {err}\n").as_bytes());
        return Err(1);
    }
    let Some(env) = engine.environments.iter().find(|e| e.name == name).cloned() else {
        io.write_stderr(format!("Error: Environment '{name}' was not found.\n").as_bytes());
        return Err(4);
    };
    if reject_template && env.is_template {
        io.write_stderr(format!("Error: Environment '{name}' is a template.\n").as_bytes());
        return Err(4);
    }
    Ok(env)
}

fn push_secrets_for_env(engine: &Engine, env: &Environment) -> Result<String, String> {
    let resolved =
        engine
            .secrets
            .resolve_environment_secrets(&env.name, &env.secret_names, false)?;
    push_environment_secrets_to_container(&env.name, &resolved).map(|opt| opt.unwrap_or_default())
}

fn run_enter(
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
            return 1;
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

fn run_exec(
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
            return 1;
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

fn run_start(
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
        return 3;
    }
    if env.running {
        io.write_stdout(
            format!("Environment '{}' is already running.\n", parsed.env_name).as_bytes(),
        );
        return 0;
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

fn run_stop(
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
        return 3;
    }
    if !env.running {
        io.write_stdout(
            format!("Environment '{}' is already stopped.\n", parsed.env_name).as_bytes(),
        );
        return 0;
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

fn run_snapshot(
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
        return 3;
    }

    let snap = if parsed.snapshot_name.is_empty() {
        format!("snap-{}", Local::now().format("%Y%m%d-%H%M%S"))
    } else {
        let s = sanitize_name(&parsed.snapshot_name);
        if s.is_empty() {
            io.write_stderr(b"Error: Invalid snapshot name.\n");
            return 2;
        }
        s
    };

    let ws_path = workspace_path_for_env(engine, &env);
    let script = create_snapshot_script(
        &parsed.env_name,
        &snap,
        &ws_path,
        env.workspace_snapshot_enabled,
    );
    run_script_with_task_lock(
        engine,
        io,
        runner,
        &format!("Saving snapshot '{snap}' for '{}'", parsed.env_name),
        &parsed.env_name,
        &script,
        true,
    )
}

fn run_snapshots(
    engine: &mut Engine,
    parsed: &ParsedArgs,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
) -> i32 {
    let env = match require_environment(engine, io, runner, &parsed.env_name, false) {
        Ok(e) => e,
        Err(rc) => return rc,
    };

    let mut sys_sizes = env.snapshot_sizes.clone();
    if !env.snapshots.is_empty() {
        let query_path = format!("/1.0/instances/{}/snapshots?recursion=1", parsed.env_name);
        let q_out = runner.run("incus", &["query", &query_path]);
        if q_out.exit_code == 0 {
            for (k, v) in parse_snapshot_sizes_json(q_out.std_out.as_bytes()) {
                sys_sizes.insert(k, v);
            }
        }
    }

    let mut ws_sizes: BTreeMap<String, String> = BTreeMap::new();
    if !env.workspace_snapshots.is_empty() && !env.workspace_snapshot_dir.is_empty() {
        let mut args = vec!["-sb".to_string()];
        for s in &env.workspace_snapshots {
            args.push(format!("{}/{s}", env.workspace_snapshot_dir));
        }
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let du_out = runner.run("du", &arg_refs);
        if du_out.exit_code == 0 {
            ws_sizes = parse_du_bytes_output(&du_out.std_out);
        }
    }

    let mut all_names = env.snapshots.clone();
    for ws in &env.workspace_snapshots {
        if !all_names.contains(ws) {
            all_names.push(ws.clone());
        }
    }

    let mut rows = Vec::with_capacity(all_names.len());
    for snap in &all_names {
        let has_sys = env.snapshots.contains(snap);
        let has_ws = env.workspace_snapshots.contains(snap);
        rows.push(json!({
            "name": snap,
            "hasSystem": has_sys,
            "hasWorkspace": has_ws,
            "isOrphanWorkspace": has_ws && !has_sys,
            "isAuto": crate::snapshot::auto::is_auto_snapshot_name(snap),
            "systemSize": sys_sizes.get(snap).map(|s| s.as_str()).unwrap_or(""),
            "workspaceSize": ws_sizes.get(snap).map(|s| s.as_str()).unwrap_or(""),
        }));
    }

    if parsed.json_output {
        let json_text = to_json_qt_indented(&rows).unwrap_or_else(|_| "[\n]\n".to_string());
        io.write_stdout(json_text.as_bytes());
        return 0;
    }

    if rows.is_empty() {
        io.write_stdout(format!("No snapshots for '{}'.\n", parsed.env_name).as_bytes());
        return 0;
    }

    io.write_stdout(
        format!(
            "{:<28} {:<14} {:<12} {:<12}\n",
            "NAME", "SCOPE", "SYSTEM_SIZE", "WORKSPACE_SIZE"
        )
        .as_bytes(),
    );
    for r in &rows {
        let has_sys = r["hasSystem"].as_bool().unwrap_or(false);
        let has_ws = r["hasWorkspace"].as_bool().unwrap_or(false);
        let scope = if has_sys && has_ws {
            "both"
        } else if has_sys {
            "system"
        } else {
            "workspace-only"
        };
        let sys_sz = r["systemSize"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("-");
        let ws_sz = r["workspaceSize"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("-");
        let name = r["name"].as_str().unwrap_or("");
        io.write_stdout(format!("{name:<28} {scope:<14} {sys_sz:<12} {ws_sz:<12}\n").as_bytes());
    }
    0
}

fn run_restore(
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
        return 3;
    }

    let has_sys = env.snapshots.contains(&parsed.snapshot_name);
    let has_ws = env.workspace_snapshots.contains(&parsed.snapshot_name);

    let mut mode = if parsed.system_only {
        "system"
    } else if parsed.workspace_only {
        "workspace"
    } else {
        "both"
    };

    if mode == "workspace" && !has_ws {
        io.write_stderr(
            format!(
                "Error: Workspace snapshot '{}' was not found.\n",
                parsed.snapshot_name
            )
            .as_bytes(),
        );
        return 1;
    }
    if mode == "system" && !has_sys {
        io.write_stderr(
            format!(
                "Error: System snapshot '{}' was not found.\n",
                parsed.snapshot_name
            )
            .as_bytes(),
        );
        return 1;
    }
    if mode == "both" {
        if !has_sys && !has_ws {
            io.write_stderr(
                format!(
                    "Error: Snapshot '{}' was not found.\n",
                    parsed.snapshot_name
                )
                .as_bytes(),
            );
            return 1;
        }
        if !has_sys && has_ws {
            mode = "workspace";
        }
    }

    if !parsed.yes {
        io.write_stdout(
            format!(
                "Restore '{}' to snapshot '{}' ({mode})? [y/N] ",
                parsed.env_name, parsed.snapshot_name
            )
            .as_bytes(),
        );
        let Some(line) = io.read_line() else {
            io.write_stderr(b"Aborted.\n");
            return 1;
        };
        let reply = line.trim().to_lowercase();
        if reply != "y" && reply != "yes" {
            io.write_stderr(b"Aborted.\n");
            return 1;
        }
    }

    let ws_path = workspace_path_for_env(engine, &env);
    let script = restore_snapshot_script(
        &parsed.env_name,
        &parsed.snapshot_name,
        &ws_path,
        mode,
        has_ws,
        Utc::now().timestamp(),
    );
    run_script_with_task_lock(
        engine,
        io,
        runner,
        &format!(
            "Restoring '{}' to snapshot '{}'",
            parsed.env_name, parsed.snapshot_name
        ),
        &parsed.env_name,
        &script,
        true,
    )
}

fn run_diff(
    engine: &mut Engine,
    parsed: &ParsedArgs,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
) -> i32 {
    let env = match require_environment(engine, io, runner, &parsed.env_name, false) {
        Ok(e) => e,
        Err(rc) => return rc,
    };

    let ws_path = workspace_path_for_env(engine, &env);
    let snap_name = if parsed.snapshot_name.is_empty() {
        let Some(last) = env.workspace_snapshots.last() else {
            io.write_stderr(
                format!(
                    "Error: No workspace snapshots found for '{}'.\n",
                    parsed.env_name
                )
                .as_bytes(),
            );
            return 1;
        };
        last.clone()
    } else {
        parsed.snapshot_name.clone()
    };

    let snap_dir = workspace_snapshot_dir(&ws_path, &parsed.env_name);
    let snap_path = format!("{snap_dir}/{snap_name}");
    if snap_dir.is_empty() || !Path::new(&snap_path).is_dir() {
        io.write_stderr(
            format!("Error: Workspace snapshot '{snap_name}' was not found.\n").as_bytes(),
        );
        return 1;
    }

    let excludes = engine.config.diff_excludes.clone();
    let args = build_workspace_diff_args(&snap_path, &ws_path, &excludes);
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let out = runner.run("rsync", &arg_refs);
    if out.exit_code != 0 && out.exit_code != 24 {
        let err = out.std_err.trim();
        let msg = if err.is_empty() {
            "Failed to compare workspace with snapshot."
        } else {
            err
        };
        io.write_stderr(format!("Error: {msg}\n").as_bytes());
        return 1;
    }

    let diff_res = process_workspace_diff_output(&snap_name, &ws_path, &out.std_out);
    if parsed.json_output {
        let root = json!({
            "summary": diff_res.summary,
            "entries": diff_res.entries,
        });
        let json_text = to_json_qt_indented(&root).unwrap_or_else(|_| "{\n}\n".to_string());
        io.write_stdout(json_text.as_bytes());
        return 0;
    }

    let trunc = if diff_res.summary.truncated {
        ", truncated at 5000"
    } else {
        ""
    };
    io.write_stdout(
        format!(
            "Snapshot: {snap_name} (+{} ~{} -{}, total {}{trunc})\n",
            diff_res.summary.added_count,
            diff_res.summary.modified_count,
            diff_res.summary.deleted_count,
            diff_res.summary.total_count
        )
        .as_bytes(),
    );
    for r in &diff_res.entries {
        let tag = match r.status.as_str() {
            "added" => "A",
            "deleted" => "D",
            _ => "M",
        };
        let sz = if r.size_text.is_empty() {
            "-"
        } else {
            r.size_text.as_str()
        };
        io.write_stdout(format!("{tag}  {sz:<10} {}\n", r.path).as_bytes());
    }
    0
}

pub fn execute_with(
    parsed: &ParsedArgs,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
    cwd: &str,
) -> i32 {
    if !parsed.valid {
        io.write_stderr(format!("Error: {}\n\n", parsed.error_message).as_bytes());
        io.write_stderr(help_text(parsed.command).as_bytes());
        return parsed.exit_code;
    }

    if parsed.command == Command::Help {
        io.write_stdout(help_text(parsed.help_target).as_bytes());
        return 0;
    }

    if parsed.command == Command::Check {
        return run_check(parsed, io, runner);
    }

    if parsed.command == Command::Theme {
        return run_theme_dump(parsed, io);
    }

    let path_override = if parsed.project_dir.is_empty() {
        None
    } else {
        Some(parsed.project_dir.as_str())
    };
    let mut engine = Engine::new(path_override, true, cwd);

    match parsed.command {
        Command::List => run_list(&mut engine, io, runner),
        Command::Enter => run_enter(&mut engine, parsed, io, runner),
        Command::Exec => run_exec(&mut engine, parsed, io, runner),
        Command::Start => run_start(&mut engine, parsed, io, runner),
        Command::Stop => run_stop(&mut engine, parsed, io, runner),
        Command::Snapshot => run_snapshot(&mut engine, parsed, io, runner),
        Command::Snapshots => run_snapshots(&mut engine, parsed, io, runner),
        Command::Restore => run_restore(&mut engine, parsed, io, runner),
        Command::Diff => run_diff(&mut engine, parsed, io, runner),
        Command::Check | Command::Help | Command::None | Command::Theme => 2,
    }
}

fn run_theme_dump(parsed: &ParsedArgs, io: &mut dyn CliIo) -> i32 {
    let theme_dir = if !parsed.theme_path.is_empty() {
        std::path::PathBuf::from(&parsed.theme_path)
    } else if let Ok(env_dir) = std::env::var("AIBOX_THEME_DIR") {
        if !env_dir.trim().is_empty() {
            std::path::PathBuf::from(env_dir)
        } else {
            crate::util::paths::home_dir().join(".config/omarchy/current")
        }
    } else {
        crate::util::paths::home_dir().join(".config/omarchy/current")
    };

    let palette = crate::theme::load_palette(&theme_dir);
    if parsed.json_output {
        let out = serde_json::to_string_pretty(&palette).unwrap_or_else(|_| "{}".to_string());
        io.write_stdout(format!("{out}\n").as_bytes());
    } else {
        io.write_stdout(palette.format_dump().as_bytes());
    }
    0
}
