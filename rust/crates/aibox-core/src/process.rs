//! Host command execution and socket permission abstraction.

use std::ffi::CString;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Captured output and exit code from running an external command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub exit_code: i32,
    pub std_out: String,
    pub std_err: String,
}

impl Default for CommandOutput {
    fn default() -> Self {
        Self {
            exit_code: -1,
            std_out: String::new(),
            std_err: String::new(),
        }
    }
}

/// Abstraction over host process execution and socket access checks for testability.
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> CommandOutput;
    fn socket_writable(&self, path: &str) -> bool;
}

/// Default host command runner using `std::process::Command` and `libc::access`.
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[&str]) -> CommandOutput {
        let Ok(mut child) = Command::new(program)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        else {
            return CommandOutput::default();
        };

        let deadline = Instant::now() + Duration::from_millis(8000);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => {
                    let out = child.wait_with_output().ok();
                    return match out {
                        Some(o) => CommandOutput {
                            exit_code: o.status.code().unwrap_or(-1),
                            std_out: String::from_utf8_lossy(&o.stdout).into_owned(),
                            std_err: String::from_utf8_lossy(&o.stderr).into_owned(),
                        },
                        None => CommandOutput::default(),
                    };
                }
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        return CommandOutput {
                            exit_code: 124,
                            std_out: String::new(),
                            std_err: "Command timed out.".to_string(),
                        };
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => return CommandOutput::default(),
            }
        }
    }

    fn socket_writable(&self, path: &str) -> bool {
        if !Path::new(path).exists() {
            return false;
        }
        CString::new(path)
            .map(|c| unsafe { libc::access(c.as_ptr(), libc::R_OK | libc::W_OK) == 0 })
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_writable_returns_false_for_missing_path() {
        let runner = SystemRunner;
        assert!(!runner.socket_writable("/nonexistent/aibox/unix.socket"));
    }
}
