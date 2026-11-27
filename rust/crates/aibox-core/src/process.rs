//! Host command execution and socket permission abstraction.

use std::ffi::CString;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

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
    fn run_with_timeout(
        &self,
        program: &str,
        args: &[&str],
        timeout: Option<Duration>,
    ) -> CommandOutput {
        let _ = timeout;
        self.run(program, args)
    }
    fn socket_writable(&self, path: &str) -> bool;
}

/// Default host command runner using `std::process::Command` and `libc::access`.
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[&str]) -> CommandOutput {
        self.run_with_timeout(program, args, Some(Duration::from_millis(8000)))
    }

    fn run_with_timeout(
        &self,
        program: &str,
        args: &[&str],
        timeout: Option<Duration>,
    ) -> CommandOutput {
        let Ok(mut child) = Command::new(program)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        else {
            return CommandOutput::default();
        };

        // Drain stdout/stderr concurrently so >64 KiB pipe output never deadlocks.
        let stdout_handle = child.stdout.take().map(|mut pipe| {
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                let _ = pipe.read_to_end(&mut buf);
                buf
            })
        });
        let stderr_handle = child.stderr.take().map(|mut pipe| {
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                let _ = pipe.read_to_end(&mut buf);
                buf
            })
        });

        let status = match timeout {
            None => child.wait().ok(),
            Some(dur) => {
                let pid = child.id() as libc::pid_t;
                let (tx, rx) = mpsc::channel();
                let waiter = std::thread::spawn(move || {
                    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
                    loop {
                        let rc = unsafe {
                            libc::waitid(
                                libc::P_PID,
                                pid as libc::id_t,
                                &mut info,
                                libc::WEXITED | libc::WNOWAIT,
                            )
                        };
                        if rc == 0
                            || std::io::Error::last_os_error().raw_os_error() != Some(libc::EINTR)
                        {
                            break;
                        }
                    }
                    let _ = tx.send(());
                });

                match rx.recv_timeout(dur) {
                    Ok(()) => {
                        let _ = waiter.join();
                        child.wait().ok()
                    }
                    Err(_) => {
                        // WNOWAIT kept the child unreaped, so `pid` cannot be reused yet.
                        unsafe {
                            libc::kill(pid, libc::SIGKILL);
                        }
                        let _ = waiter.join();
                        let _ = child.wait();
                        return CommandOutput {
                            exit_code: 124,
                            std_out: String::new(),
                            std_err: "Command timed out.".to_string(),
                        };
                    }
                }
            }
        };

        let stdout_bytes = stdout_handle
            .and_then(|h| h.join().ok())
            .unwrap_or_default();
        let stderr_bytes = stderr_handle
            .and_then(|h| h.join().ok())
            .unwrap_or_default();

        match status {
            Some(st) => CommandOutput {
                exit_code: st.code().unwrap_or(-1),
                std_out: String::from_utf8_lossy(&stdout_bytes).into_owned(),
                std_err: String::from_utf8_lossy(&stderr_bytes).into_owned(),
            },
            None => CommandOutput::default(),
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

    #[test]
    fn run_drains_large_stdout_and_stderr_without_deadlocking() {
        let runner = SystemRunner;
        let out = runner.run_with_timeout(
            "/bin/sh",
            &[
                "-c",
                "head -c 98304 /dev/zero | tr '\\0' 'A'; head -c 98304 /dev/zero | tr '\\0' 'B' >&2",
            ],
            Some(Duration::from_secs(5)),
        );
        assert_eq!(out.exit_code, 0);
        assert_eq!(out.std_out.len(), 98_304);
        assert_eq!(out.std_err.len(), 98_304);
    }

    #[test]
    fn run_with_timeout_kills_slow_process_with_exit_124() {
        let runner = SystemRunner;
        let out = runner.run_with_timeout("/bin/sleep", &["5"], Some(Duration::from_millis(80)));
        assert_eq!(out.exit_code, 124);
        assert_eq!(out.std_err, "Command timed out.");
    }
}
