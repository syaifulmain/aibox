use aibox_core::util::json::to_json_compact;
use aibox_core::workspace::diff::{self, FileDiffPrep};

#[cxx::bridge(namespace = "aibox::ffi")]
mod ffi {
    struct CapDiffResultFfi {
        text: String,
        truncated: bool,
    }

    struct FileDiffPrepFfi {
        run_diff: bool,
        text: String,
        info: String,
        args: Vec<String>,
    }

    struct FileDiffOutputFfi {
        text: String,
        info: String,
    }

    struct RestoreSingleFileResultFfi {
        ok: bool,
        message: String,
    }

    extern "Rust" {
        fn default_diff_excludes() -> Vec<String>;
        fn parse_diff_excludes(raw_text: &str) -> Vec<String>;
        fn build_rsync_exclude_args(excludes: &[String]) -> Vec<String>;
        fn build_workspace_diff_args(
            snap_path: &str,
            ws_path: &str,
            excludes: &[String],
        ) -> Vec<String>;
        fn is_safe_relative_path(relative_path: &str) -> bool;
        fn is_likely_binary_data(sample: &[u8]) -> bool;
        fn cap_diff_lines(diff_output: &str, max_lines: i32) -> CapDiffResultFfi;
        fn parse_rsync_itemize_json(output: &str, max_entries: i32) -> String;
        fn process_workspace_diff_output_json(
            snapshot_name: &str,
            ws_path: &str,
            rsync_stdout: &str,
        ) -> String;
        fn prepare_file_diff_ffi(
            name: &str,
            snapshot_name: &str,
            ws_path: &str,
            relative_path: &str,
            status: &str,
        ) -> FileDiffPrepFfi;
        fn shape_file_diff_output_ffi(
            exit_code: i32,
            stdout: &str,
            relative_path: &str,
        ) -> FileDiffOutputFfi;
        fn restore_single_file_ffi(
            name: &str,
            snapshot_name: &str,
            ws_path: &str,
            relative_path: &str,
        ) -> RestoreSingleFileResultFfi;
    }
}

fn default_diff_excludes() -> Vec<String> {
    diff::default_diff_excludes()
}

fn parse_diff_excludes(raw_text: &str) -> Vec<String> {
    diff::parse_diff_excludes(raw_text)
}

fn build_rsync_exclude_args(excludes: &[String]) -> Vec<String> {
    diff::build_rsync_exclude_args(excludes)
}

fn build_workspace_diff_args(snap_path: &str, ws_path: &str, excludes: &[String]) -> Vec<String> {
    diff::build_workspace_diff_args(snap_path, ws_path, excludes)
}

fn is_safe_relative_path(relative_path: &str) -> bool {
    diff::is_safe_relative_path(relative_path)
}

fn is_likely_binary_data(sample: &[u8]) -> bool {
    diff::is_likely_binary_data(sample)
}

fn cap_diff_lines(diff_output: &str, max_lines: i32) -> ffi::CapDiffResultFfi {
    let (text, truncated) = diff::cap_diff_lines(diff_output, max_lines);
    ffi::CapDiffResultFfi { text, truncated }
}

fn parse_rsync_itemize_json(output: &str, max_entries: i32) -> String {
    let parsed = diff::parse_rsync_itemize(output, max_entries);
    to_json_compact(&parsed).unwrap_or_else(|_| "{}".to_string())
}

fn process_workspace_diff_output_json(
    snapshot_name: &str,
    ws_path: &str,
    rsync_stdout: &str,
) -> String {
    let res = diff::process_workspace_diff_output(snapshot_name, ws_path, rsync_stdout);
    to_json_compact(&res).unwrap_or_else(|_| "{}".to_string())
}

fn prepare_file_diff_ffi(
    name: &str,
    snapshot_name: &str,
    ws_path: &str,
    relative_path: &str,
    status: &str,
) -> ffi::FileDiffPrepFfi {
    match diff::prepare_file_diff(name, snapshot_name, ws_path, relative_path, status) {
        FileDiffPrep::Immediate { text, info } => ffi::FileDiffPrepFfi {
            run_diff: false,
            text,
            info,
            args: Vec::new(),
        },
        FileDiffPrep::RunDiff { args } => ffi::FileDiffPrepFfi {
            run_diff: true,
            text: String::new(),
            info: String::new(),
            args,
        },
    }
}

fn shape_file_diff_output_ffi(
    exit_code: i32,
    stdout: &str,
    relative_path: &str,
) -> ffi::FileDiffOutputFfi {
    let out = diff::shape_file_diff_output(exit_code, stdout, relative_path);
    ffi::FileDiffOutputFfi {
        text: out.text,
        info: out.info,
    }
}

fn restore_single_file_ffi(
    name: &str,
    snapshot_name: &str,
    ws_path: &str,
    relative_path: &str,
) -> ffi::RestoreSingleFileResultFfi {
    match diff::restore_single_file(name, snapshot_name, ws_path, relative_path) {
        Ok(message) => ffi::RestoreSingleFileResultFfi { ok: true, message },
        Err(message) => ffi::RestoreSingleFileResultFfi { ok: false, message },
    }
}
