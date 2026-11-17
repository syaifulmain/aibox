use std::path::Path;

use aibox_core::config::model::Config;
use aibox_core::engine::{Engine, ResolvedPaths};
use aibox_core::health::checks::SystemRunner;
use aibox_core::secrets::state::SecretsState;
use aibox_core::task::{files, lock, progress, runner};

#[cxx::bridge(namespace = "aibox::ffi")]
mod ffi {
    struct TaskProgressFfi {
        task_progress: i32,
        task_stage: String,
        task_log: String,
        has_exit_code: bool,
        last_exit_code: i32,
        pending_env_name: String,
    }

    struct AttachOutcomeFfi {
        kind: i32, // 0=NoTask, 1=StillRunning, 2=Finished
        description: String,
        env_name: String,
        downloading_os_id: String,
        log_offset: i64,
        exit_code: i32,
        has_exit_code: bool,
        history_json: String,
        progress: TaskProgressFfi,
    }

    struct CheckTaskOutcomeFfi {
        finished: bool,
        log_offset: i64,
        progress_changed: bool,
        exit_code: i32,
        finished_env: String,
        last_log: String,
        history_json: String,
        progress: TaskProgressFfi,
    }

    struct RefreshSyncResultFfi {
        ok: bool,
        environments_json: String,
        error: String,
    }

    extern "Rust" {
        fn read_task_pid(state_dir: &str) -> i64;
        fn is_task_lock_active(state_dir: &str, busy: bool, out_pid: &mut i64) -> bool;
        fn acquire_cli_task_lock(
            state_dir: &str,
            busy: bool,
            description: &str,
            env_name: &str,
        ) -> bool;
        fn release_cli_task_lock(state_dir: &str);
        fn clear_task_files(state_dir: &str);
        fn admin_fix_command(message: &str) -> String;
        fn parse_process_output_chunk(
            state: &mut TaskProgressFfi,
            chunk: &[u8],
            secret_values: &[String],
        ) -> bool;
        fn evaluate_attach_existing_task(
            state_dir: &str,
            secret_values: &[String],
            max_history_entries: i32,
        ) -> AttachOutcomeFfi;
        fn evaluate_check_background_task(
            state_dir: &str,
            log_offset: i64,
            state: &TaskProgressFfi,
            secret_values: &[String],
            max_history_entries: i32,
        ) -> CheckTaskOutcomeFfi;
        fn cancel_running_task(
            state_dir: &str,
            task_stage: &str,
            pending_env_name: &str,
            secret_values: &[String],
            max_history_entries: i32,
        ) -> String;
        fn prepare_shell_task(
            state_dir: &str,
            description: &str,
            script: &str,
            created_env_name: &str,
            target_env_name: &str,
            downloading_os_id: &str,
            reason: &str,
        ) -> String;
        fn engine_refresh_sync(
            workspace_base: &str,
            state_dir: &str,
            auto_snapshot: bool,
            watchdog_auto_stop: bool,
            record_sessions: bool,
        ) -> RefreshSyncResultFfi;
    }
}

fn to_core_progress(p: &ffi::TaskProgressFfi) -> progress::TaskProgressState {
    progress::TaskProgressState {
        task_progress: p.task_progress,
        task_stage: p.task_stage.clone(),
        task_log: p.task_log.clone(),
        has_exit_code: p.has_exit_code,
        last_exit_code: p.last_exit_code,
        pending_env_name: p.pending_env_name.clone(),
        line_buffer: String::new(),
    }
}

fn from_core_progress(p: progress::TaskProgressState) -> ffi::TaskProgressFfi {
    ffi::TaskProgressFfi {
        task_progress: p.task_progress,
        task_stage: p.task_stage,
        task_log: p.task_log,
        has_exit_code: p.has_exit_code,
        last_exit_code: p.last_exit_code,
        pending_env_name: p.pending_env_name,
    }
}

fn read_task_pid(state_dir: &str) -> i64 {
    files::read_task_pid(Path::new(state_dir))
}

fn is_task_lock_active(state_dir: &str, busy: bool, out_pid: &mut i64) -> bool {
    let (active, pid) = lock::is_task_lock_active(Path::new(state_dir), busy);
    *out_pid = pid;
    active
}

fn acquire_cli_task_lock(state_dir: &str, busy: bool, description: &str, env_name: &str) -> bool {
    lock::acquire_cli_task_lock(Path::new(state_dir), busy, description, env_name)
}

fn release_cli_task_lock(state_dir: &str) {
    lock::release_cli_task_lock(Path::new(state_dir));
}

fn clear_task_files(state_dir: &str) {
    files::clear_task_files(Path::new(state_dir));
}

fn admin_fix_command(message: &str) -> String {
    files::admin_fix_command(message)
}

fn parse_process_output_chunk(
    state: &mut ffi::TaskProgressFfi,
    chunk: &[u8],
    secret_values: &[String],
) -> bool {
    let mut core = to_core_progress(state);
    let delta = progress::parse_process_output_chunk(&mut core, chunk, secret_values);
    *state = from_core_progress(core);
    delta.progress_changed
}

fn evaluate_attach_existing_task(
    state_dir: &str,
    secret_values: &[String],
    max_history_entries: i32,
) -> ffi::AttachOutcomeFfi {
    match progress::evaluate_attach_existing_task(
        Path::new(state_dir),
        secret_values,
        max_history_entries,
    ) {
        progress::AttachOutcome::NoTask => ffi::AttachOutcomeFfi {
            kind: 0,
            description: String::new(),
            env_name: String::new(),
            downloading_os_id: String::new(),
            log_offset: 0,
            exit_code: -1,
            has_exit_code: false,
            history_json: String::new(),
            progress: ffi::TaskProgressFfi {
                task_progress: 0,
                task_stage: String::new(),
                task_log: String::new(),
                has_exit_code: false,
                last_exit_code: -1,
                pending_env_name: String::new(),
            },
        },
        progress::AttachOutcome::StillRunning {
            description,
            env_name,
            downloading_os_id,
            log_offset,
            progress: p,
        } => ffi::AttachOutcomeFfi {
            kind: 1,
            description,
            env_name,
            downloading_os_id,
            log_offset,
            exit_code: -1,
            has_exit_code: false,
            history_json: String::new(),
            progress: from_core_progress(p),
        },
        progress::AttachOutcome::Finished {
            env_name,
            exit_code,
            has_exit_code,
            task_log,
            history_json,
        } => ffi::AttachOutcomeFfi {
            kind: 2,
            description: String::new(),
            env_name,
            downloading_os_id: String::new(),
            log_offset: 0,
            exit_code,
            has_exit_code,
            history_json,
            progress: ffi::TaskProgressFfi {
                task_progress: 0,
                task_stage: String::new(),
                task_log,
                has_exit_code,
                last_exit_code: exit_code,
                pending_env_name: String::new(),
            },
        },
    }
}

fn evaluate_check_background_task(
    state_dir: &str,
    log_offset: i64,
    state: &ffi::TaskProgressFfi,
    secret_values: &[String],
    max_history_entries: i32,
) -> ffi::CheckTaskOutcomeFfi {
    let core_in = to_core_progress(state);
    let res = progress::evaluate_check_background_task(
        Path::new(state_dir),
        log_offset,
        core_in,
        secret_values,
        max_history_entries,
    );
    ffi::CheckTaskOutcomeFfi {
        finished: res.finished,
        log_offset: res.log_offset,
        progress_changed: res.progress_changed,
        exit_code: res.exit_code,
        finished_env: res.finished_env,
        last_log: res.last_log,
        history_json: res.history_json,
        progress: from_core_progress(res.progress),
    }
}

fn cancel_running_task(
    state_dir: &str,
    task_stage: &str,
    pending_env_name: &str,
    secret_values: &[String],
    max_history_entries: i32,
) -> String {
    progress::cancel_running_task(
        Path::new(state_dir),
        task_stage,
        pending_env_name,
        secret_values,
        max_history_entries,
    )
}

fn prepare_shell_task(
    state_dir: &str,
    description: &str,
    script: &str,
    created_env_name: &str,
    target_env_name: &str,
    downloading_os_id: &str,
    reason: &str,
) -> String {
    runner::prepare_shell_task(&runner::PrepareShellTaskParams {
        state_dir: Path::new(state_dir),
        description,
        script,
        created_env_name,
        target_env_name,
        downloading_os_id,
        reason,
    })
    .unwrap_or_default()
}

fn engine_refresh_sync(
    workspace_base: &str,
    state_dir: &str,
    auto_snapshot: bool,
    watchdog_auto_stop: bool,
    record_sessions: bool,
) -> ffi::RefreshSyncResultFfi {
    let paths = ResolvedPaths {
        project_dir: String::new(),
        system_path: String::new(),
        workspace_base: workspace_base.to_string(),
        state_dir: state_dir.to_string(),
        config_file_path: String::new(),
        os_storage_path: String::new(),
    };
    let mut cfg = Config::new_defaults(workspace_base, "");
    cfg.auto_snapshot_enabled = auto_snapshot;
    cfg.watchdog_auto_stop = watchdog_auto_stop;
    cfg.record_sessions = record_sessions;

    let mut engine = Engine::from_parts(paths, cfg, SecretsState::new(String::new()));
    match engine.refresh_sync(&SystemRunner) {
        Ok(()) => ffi::RefreshSyncResultFfi {
            ok: true,
            environments_json: engine.environments_json(),
            error: String::new(),
        },
        Err(err) => ffi::RefreshSyncResultFfi {
            ok: false,
            environments_json: String::new(),
            error: err,
        },
    }
}
