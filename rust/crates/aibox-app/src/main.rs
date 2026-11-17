use std::ffi::CString;
use std::os::raw::{c_char, c_int};

use aibox_ffi as _;
use cxx_qt as _;
use cxx_qt_lib as _;

mod qt_backend;
mod qt_backend_config;
mod qt_backend_network;
mod qt_backend_secrets;
mod qt_backend_snapshot;
mod qt_backend_workspace;
mod qt_theme;
mod qt_translator;
mod variant;

extern "C" {
    fn aibox_gui_main(argc: c_int, argv: *mut *mut c_char) -> c_int;
}

fn main() {
    let raw_args: Vec<String> = std::env::args().collect();

    if raw_args.len() >= 3 && raw_args[1] == "--redact-session" {
        let log_path = &raw_args[2];
        let sessions_dir = raw_args.get(3).map(String::as_str).unwrap_or("");
        let code = aibox_core::session::redact_session_main(log_path, sessions_dir);
        std::process::exit(code);
    }

    if raw_args.len() > 1 {
        let args_without_program = &raw_args[1..];
        if aibox_core::cli::is_cli_invocation(args_without_program) {
            let code = aibox_core::cli::run(args_without_program);
            std::process::exit(code);
        }
    }

    let c_args: Vec<CString> = raw_args
        .iter()
        .map(|arg| CString::new(arg.as_str()).unwrap_or_else(|_| CString::new("").unwrap()))
        .collect();
    let mut ptrs: Vec<*mut c_char> = c_args.iter().map(|s| s.as_ptr() as *mut c_char).collect();

    let code = unsafe { aibox_gui_main(ptrs.len() as c_int, ptrs.as_mut_ptr()) };
    std::process::exit(code);
}
