use aibox_core::session;
use aibox_core::util::json::to_json_compact;

#[cxx::bridge(namespace = "aibox::ffi")]
mod ffi {
    struct SessionPathsFfi {
        ok: bool,
        log_file: String,
        timing_file: String,
    }

    struct ReadSessionLogResultFfi {
        found: bool,
        text: String,
    }

    struct DeleteSessionResultFfi {
        ok: bool,
        invalid_root: String,
    }

    extern "Rust" {
        fn sessions_dir_path(state_dir: &str) -> String;
        fn prepare_recorded_session_paths(state_dir: &str, env_name: &str) -> SessionPathsFfi;
        fn effective_record_sessions(global_enabled: bool, env_override: &str) -> bool;
        fn build_session_wrap_command(
            inner_command: &str,
            log_file: &str,
            timing_file: &str,
            redact_bin_path: &str,
            sessions_dir: &str,
            script_available: bool,
        ) -> String;
        fn strip_ansi_sequences(raw_text: &str) -> String;
        fn format_recorded_session_text(raw_bytes: &[u8], max_bytes: i32) -> String;
        fn redact_session_log_file(log_file_path: &str, secret_values: &[String]) -> bool;
        fn prune_recorded_sessions(
            sessions_dir: &str,
            max_per_env: i32,
            max_total_bytes: i64,
        ) -> i32;
        fn scan_recorded_sessions_json(sessions_dir: &str) -> String;
        fn read_recorded_session_log(log_path: &str, demo_mode: bool) -> ReadSessionLogResultFfi;
        fn build_replay_command(log_path: &str, timing_path: &str) -> String;
        fn delete_recorded_session(
            sessions_dir: &str,
            log_path: &str,
            timing_path: &str,
        ) -> DeleteSessionResultFfi;
        fn redact_session_main(log_path: &str, sessions_dir: &str) -> i32;
    }
}

fn sessions_dir_path(state_dir: &str) -> String {
    session::sessions_dir_path(state_dir)
}

fn prepare_recorded_session_paths(state_dir: &str, env_name: &str) -> ffi::SessionPathsFfi {
    match session::prepare_recorded_session_paths(state_dir, env_name) {
        Some((log_file, timing_file)) => ffi::SessionPathsFfi {
            ok: true,
            log_file,
            timing_file,
        },
        None => ffi::SessionPathsFfi {
            ok: false,
            log_file: String::new(),
            timing_file: String::new(),
        },
    }
}

fn effective_record_sessions(global_enabled: bool, env_override: &str) -> bool {
    session::effective_record_sessions(global_enabled, env_override)
}

fn build_session_wrap_command(
    inner_command: &str,
    log_file: &str,
    timing_file: &str,
    redact_bin_path: &str,
    sessions_dir: &str,
    script_available: bool,
) -> String {
    session::build_session_wrap_command(
        inner_command,
        log_file,
        timing_file,
        redact_bin_path,
        sessions_dir,
        script_available,
    )
}

fn strip_ansi_sequences(raw_text: &str) -> String {
    session::strip_ansi_sequences(raw_text)
}

fn format_recorded_session_text(raw_bytes: &[u8], max_bytes: i32) -> String {
    session::format_recorded_session_text(raw_bytes, max_bytes)
}

fn redact_session_log_file(log_file_path: &str, secret_values: &[String]) -> bool {
    session::redact_session_log_file(log_file_path, secret_values)
}

fn prune_recorded_sessions(sessions_dir: &str, max_per_env: i32, max_total_bytes: i64) -> i32 {
    session::prune_recorded_sessions(sessions_dir, max_per_env, max_total_bytes)
}

fn scan_recorded_sessions_json(sessions_dir: &str) -> String {
    let items = session::scan_recorded_sessions(sessions_dir);
    to_json_compact(&items).unwrap_or_else(|_| "[]".to_string())
}

fn read_recorded_session_log(log_path: &str, demo_mode: bool) -> ffi::ReadSessionLogResultFfi {
    match session::read_recorded_session_log(log_path, demo_mode) {
        Some(text) => ffi::ReadSessionLogResultFfi { found: true, text },
        None => ffi::ReadSessionLogResultFfi {
            found: false,
            text: String::new(),
        },
    }
}

fn build_replay_command(log_path: &str, timing_path: &str) -> String {
    session::build_replay_command(log_path, timing_path)
}

fn delete_recorded_session(
    sessions_dir: &str,
    log_path: &str,
    timing_path: &str,
) -> ffi::DeleteSessionResultFfi {
    match session::delete_recorded_session(sessions_dir, log_path, timing_path) {
        Ok(()) => ffi::DeleteSessionResultFfi {
            ok: true,
            invalid_root: String::new(),
        },
        Err(invalid_root) => ffi::DeleteSessionResultFfi {
            ok: false,
            invalid_root,
        },
    }
}

fn redact_session_main(log_path: &str, sessions_dir: &str) -> i32 {
    session::redact_session_main(log_path, sessions_dir)
}
