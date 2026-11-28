use crate::common;
use aibox_core::*;
use serde_json::{json, Value};
use std::fs;

fn dispatch(fn_name: &str, _case: &str, args: &Value) -> Value {
    match fn_name {
        "Backend::adminFixCommand" => {
            let msg = args["message"].as_str().expect("message");
            json!(task::files::admin_fix_command(msg))
        }
        "Backend::handleProcessOutput" => {
            let pending_env = args["pendingEnv"].as_str().expect("pendingEnv");
            let chunk = args["chunk"].as_str().expect("chunk");
            let mut state = task::progress::TaskProgressState {
                task_progress: 5,
                task_stage: "Init".to_string(),
                task_log: String::new(),
                has_exit_code: false,
                last_exit_code: -1,
                pending_env_name: pending_env.to_string(),
                line_buffer: Vec::new(),
            };
            task::progress::parse_process_output_chunk(&mut state, chunk.as_bytes(), &[]);
            json!({
                "taskProgress": state.task_progress,
                "taskStage": state.task_stage,
                "taskLog": state.task_log,
                "hasExitCode": state.has_exit_code,
                "lastExitCode": state.last_exit_code,
            })
        }
        "Backend::taskLockLifecycle" => {
            let work = std::path::Path::new("/tmp/aibox-golden-rust-task-lock");
            let _ = fs::remove_dir_all(work);
            fs::create_dir_all(work).expect("mkdir");

            let initial_pid = task::files::read_task_pid(work);
            let initial_active = task::lock::is_task_lock_active(work, false).0;

            task::files::write_task_pid(work, 99_999_999).expect("stale pid");
            let stale_pid = task::files::read_task_pid(work);
            let stale_active = task::lock::is_task_lock_active(work, false).0;

            let acquired = task::lock::acquire_cli_task_lock(work, false, "Starting 'dev'", "dev");
            let active_after_acquire = task::lock::is_task_lock_active(work, false).0;
            let second_acquire = task::lock::acquire_cli_task_lock(work, false, "Other", "dev");
            let own_pid = task::files::read_task_pid(work);
            let pid_matches_self = own_pid == i64::from(unsafe { libc::getpid() });

            let meta_raw =
                fs::read_to_string(task::files::task_meta_path(work)).unwrap_or_default();
            let mut meta_val: Value = serde_json::from_str(&meta_raw).unwrap_or(json!({}));
            let has_started_at = !meta_val["startedAt"].as_str().unwrap_or("").is_empty();
            if let Some(obj) = meta_val.as_object_mut() {
                obj.insert("startedAt".to_string(), json!("<NOW>"));
            }

            task::lock::release_cli_task_lock(work);
            let active_after_release = task::lock::is_task_lock_active(work, false).0;
            let pid_exists_after_release = task::files::task_pid_path(work).exists();
            let meta_exists_after_release = task::files::task_meta_path(work).exists();
            let _ = fs::remove_dir_all(work);

            json!({
                "initialPid": initial_pid,
                "initialActive": initial_active,
                "stalePid": stale_pid,
                "staleActive": stale_active,
                "acquired": acquired,
                "activeAfterAcquire": active_after_acquire,
                "secondAcquire": second_acquire,
                "pidMatchesSelf": pid_matches_self,
                "hasStartedAt": has_started_at,
                "meta": meta_val,
                "activeAfterRelease": active_after_release,
                "pidExistsAfterRelease": pid_exists_after_release,
                "metaExistsAfterRelease": meta_exists_after_release,
            })
        }
        other => panic!("unregistered golden function: {other}"),
    }
}

#[test]
fn replay() {
    common::replay_jsonl("task", dispatch);
}
