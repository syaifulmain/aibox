use std::collections::BTreeMap;
use std::path::Path;

use chrono::{Local, Utc};
use serde_json::json;

use crate::cli::commands::{
    require_environment, run_script_with_task_lock, to_json_qt_indented, workspace_path_for_env,
};
use crate::cli::{CliIo, ExitCode, ParsedArgs};
use crate::engine::Engine;
use crate::process::CommandRunner;
use crate::snapshot::scripts::{create_snapshot_script, restore_snapshot_script};
use crate::snapshot::workspace::workspace_snapshot_dir;
use crate::snapshot::{parse_du_bytes_output, parse_snapshot_sizes_json};
use crate::util::names::sanitize_name;
use crate::workspace::diff::{build_workspace_diff_args, process_workspace_diff_output};

pub fn run_snapshot(
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

    let snap = if parsed.snapshot_name.is_empty() {
        format!("snap-{}", Local::now().format("%Y%m%d-%H%M%S"))
    } else {
        let s = sanitize_name(&parsed.snapshot_name);
        if s.is_empty() {
            io.write_stderr(b"Error: Invalid snapshot name.\n");
            return ExitCode::Usage.as_i32();
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

pub fn run_snapshots(
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
        return ExitCode::Ok.as_i32();
    }

    if rows.is_empty() {
        io.write_stdout(format!("No snapshots for '{}'.\n", parsed.env_name).as_bytes());
        return ExitCode::Ok.as_i32();
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
    ExitCode::Ok.as_i32()
}

pub fn run_restore(
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
        return ExitCode::Failed.as_i32();
    }
    if mode == "system" && !has_sys {
        io.write_stderr(
            format!(
                "Error: System snapshot '{}' was not found.\n",
                parsed.snapshot_name
            )
            .as_bytes(),
        );
        return ExitCode::Failed.as_i32();
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
            return ExitCode::Failed.as_i32();
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
            return ExitCode::Failed.as_i32();
        };
        let reply = line.trim().to_lowercase();
        if reply != "y" && reply != "yes" {
            io.write_stderr(b"Aborted.\n");
            return ExitCode::Failed.as_i32();
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

pub fn run_diff(
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
            return ExitCode::Failed.as_i32();
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
        return ExitCode::Failed.as_i32();
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
        return ExitCode::Failed.as_i32();
    }

    let diff_res = process_workspace_diff_output(&snap_name, &ws_path, &out.std_out);
    if parsed.json_output {
        let root = json!({
            "summary": diff_res.summary,
            "entries": diff_res.entries,
        });
        let json_text = to_json_qt_indented(&root).unwrap_or_else(|_| "{\n}\n".to_string());
        io.write_stdout(json_text.as_bytes());
        return ExitCode::Ok.as_i32();
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
    ExitCode::Ok.as_i32()
}
