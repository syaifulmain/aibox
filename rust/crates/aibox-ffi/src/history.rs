use aibox_core::history::{self, RecordOperationParams};
use aibox_core::util::json::to_json_compact;

#[cxx::bridge(namespace = "aibox::ffi")]
mod ffi {
    struct RecordOperationResultFfi {
        ok: bool,
        entries_json: String,
    }

    struct ReadLogResultFfi {
        found: bool,
        text: String,
    }

    extern "Rust" {
        fn history_dir_path(state_dir: &str) -> String;
        fn history_seen_marker_path(state_dir: &str) -> String;
        fn load_history_seen_marker(state_dir: &str) -> String;
        fn recent_failed_history_count(entries_json: &str, history_seen_id: &str) -> i32;
        fn load_operation_history_json(state_dir: &str) -> String;
        #[allow(clippy::too_many_arguments)]
        fn record_operation_history_json(
            state_dir: &str,
            max_entries: i32,
            description: &str,
            env_name: &str,
            status: &str,
            exit_code: i32,
            started_at_iso: &str,
            log_bytes: &[u8],
            reason: &str,
            secret_values: &[String],
        ) -> RecordOperationResultFfi;
        fn read_operation_log(state_dir: &str, id: &str, demo_mode: bool) -> ReadLogResultFfi;
        fn clear_operation_history(state_dir: &str);
        fn acknowledge_operation_history(
            state_dir: &str,
            newest_id: &str,
            current_seen_id: &str,
        ) -> String;
    }
}

fn history_dir_path(state_dir: &str) -> String {
    history::history_dir_path(state_dir)
}

fn history_seen_marker_path(state_dir: &str) -> String {
    history::history_seen_marker_path(state_dir)
}

fn load_history_seen_marker(state_dir: &str) -> String {
    history::load_history_seen_marker(state_dir)
}

fn recent_failed_history_count(entries_json: &str, history_seen_id: &str) -> i32 {
    history::recent_failed_history_count_json(entries_json, history_seen_id)
}

fn load_operation_history_json(state_dir: &str) -> String {
    let entries = history::load_operation_history(state_dir);
    to_json_compact(&entries).unwrap_or_else(|_| "[]".to_string())
}

#[allow(clippy::too_many_arguments)]
fn record_operation_history_json(
    state_dir: &str,
    max_entries: i32,
    description: &str,
    env_name: &str,
    status: &str,
    exit_code: i32,
    started_at_iso: &str,
    log_bytes: &[u8],
    reason: &str,
    secret_values: &[String],
) -> ffi::RecordOperationResultFfi {
    match history::record_operation_history(&RecordOperationParams {
        state_dir,
        max_entries,
        description,
        env_name,
        status,
        exit_code,
        started_at_iso,
        log_bytes,
        reason,
        secret_values,
    }) {
        Some(entries) => ffi::RecordOperationResultFfi {
            ok: true,
            entries_json: to_json_compact(&entries).unwrap_or_else(|_| "[]".to_string()),
        },
        None => ffi::RecordOperationResultFfi {
            ok: false,
            entries_json: String::new(),
        },
    }
}

fn read_operation_log(state_dir: &str, id: &str, demo_mode: bool) -> ffi::ReadLogResultFfi {
    match history::read_operation_log(state_dir, id, demo_mode) {
        Some(text) => ffi::ReadLogResultFfi { found: true, text },
        None => ffi::ReadLogResultFfi {
            found: false,
            text: String::new(),
        },
    }
}

fn clear_operation_history(state_dir: &str) {
    history::clear_operation_history(state_dir);
}

fn acknowledge_operation_history(
    state_dir: &str,
    newest_id: &str,
    current_seen_id: &str,
) -> String {
    history::acknowledge_operation_history(state_dir, newest_id, current_seen_id)
        .unwrap_or_default()
}
