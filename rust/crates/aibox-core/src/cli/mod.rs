pub mod commands;
pub mod help;
pub mod parse;

use std::io::{self, BufRead, Write};
use std::os::unix::process::CommandExt;
use std::process::{Command as ProcessCommand, Stdio};

use crate::health::checks::{CommandRunner, SystemRunner};

pub use help::help_text;
pub use parse::{argv_without_program, is_cli_invocation, parse_args, parse_command_name};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Command {
    #[default]
    None,
    Help,
    List,
    Check,
    Enter,
    Exec,
    Start,
    Stop,
    Snapshot,
    Snapshots,
    Restore,
    Diff,
}

impl Command {
    pub fn as_str(self) -> &'static str {
        match self {
            Command::None => "none",
            Command::Help => "help",
            Command::List => "list",
            Command::Check => "check",
            Command::Enter => "enter",
            Command::Exec => "exec",
            Command::Start => "start",
            Command::Stop => "stop",
            Command::Snapshot => "snapshot",
            Command::Snapshots => "snapshots",
            Command::Restore => "restore",
            Command::Diff => "diff",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedArgs {
    pub valid: bool,
    pub exit_code: i32,
    pub error_message: String,
    pub command: Command,
    pub help_target: Command,
    pub project_dir: String,
    pub env_name: String,
    pub snapshot_name: String,
    pub exec_command: Vec<String>,
    pub json_output: bool,
    pub fix: bool,
    pub system_only: bool,
    pub workspace_only: bool,
    pub yes: bool,
}

impl Default for ParsedArgs {
    fn default() -> Self {
        Self {
            valid: true,
            exit_code: 0,
            error_message: String::new(),
            command: Command::None,
            help_target: Command::None,
            project_dir: String::new(),
            env_name: String::new(),
            snapshot_name: String::new(),
            exec_command: Vec::new(),
            json_output: false,
            fix: false,
            system_only: false,
            workspace_only: false,
            yes: false,
        }
    }
}

/// Abstracts CLI I/O and interactive/forwarded process execution so tests can
/// capture stdout/stderr and stub process calls.
pub trait CliIo {
    fn write_stdout(&mut self, data: &[u8]);
    fn write_stderr(&mut self, data: &[u8]);
    fn read_line(&mut self) -> Option<String>;
    fn run_forwarded(&mut self, program: &str, args: &[String]) -> i32;
    fn exec_replace(&mut self, program: &str, args: &[String]) -> i32;
}

pub struct StdCliIo;

impl CliIo for StdCliIo {
    fn write_stdout(&mut self, data: &[u8]) {
        let mut out = io::stdout().lock();
        let _ = out.write_all(data);
        let _ = out.flush();
    }

    fn write_stderr(&mut self, data: &[u8]) {
        let mut err = io::stderr().lock();
        let _ = err.write_all(data);
        let _ = err.flush();
    }

    fn read_line(&mut self) -> Option<String> {
        let mut line = String::new();
        let stdin = io::stdin();
        let mut handle = stdin.lock();
        match handle.read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line),
        }
    }

    fn run_forwarded(&mut self, program: &str, args: &[String]) -> i32 {
        self.write_stdout(&[]);
        self.write_stderr(&[]);
        match ProcessCommand::new(program)
            .args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
        {
            Ok(status) => status.code().unwrap_or(1),
            Err(_) => {
                self.write_stderr(format!("Error: Failed to invoke {program} exec.\n").as_bytes());
                1
            }
        }
    }

    fn exec_replace(&mut self, program: &str, args: &[String]) -> i32 {
        self.write_stdout(&[]);
        self.write_stderr(&[]);
        let err = ProcessCommand::new(program).args(args).exec();
        self.write_stderr(format!("Error: Could not exec {program}: {err}\n").as_bytes());
        1
    }
}

pub fn execute_with(
    parsed: &ParsedArgs,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
    cwd: &str,
) -> i32 {
    commands::execute_with(parsed, io, runner, cwd)
}

pub fn execute(parsed: &ParsedArgs) -> i32 {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string());
    let mut io = StdCliIo;
    let runner = SystemRunner;
    execute_with(parsed, &mut io, &runner, &cwd)
}

pub fn run(args_without_program: &[String]) -> i32 {
    let parsed = parse_args(args_without_program);
    execute(&parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;
    use crate::health::checks::CommandOutput;
    use crate::util::paths::resolve_project_aibox_dir;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestCliIo {
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        stdin_lines: VecDeque<String>,
        forwarded_calls: Vec<(String, Vec<String>)>,
        forwarded_exit_code: i32,
        exec_calls: Vec<(String, Vec<String>)>,
    }

    impl TestCliIo {
        fn new() -> Self {
            Self {
                stdout: Vec::new(),
                stderr: Vec::new(),
                stdin_lines: VecDeque::new(),
                forwarded_calls: Vec::new(),
                forwarded_exit_code: 0,
                exec_calls: Vec::new(),
            }
        }

        fn stdout_str(&self) -> String {
            String::from_utf8_lossy(&self.stdout).to_string()
        }

        fn stderr_str(&self) -> String {
            String::from_utf8_lossy(&self.stderr).to_string()
        }
    }

    impl CliIo for TestCliIo {
        fn write_stdout(&mut self, data: &[u8]) {
            self.stdout.extend_from_slice(data);
        }

        fn write_stderr(&mut self, data: &[u8]) {
            self.stderr.extend_from_slice(data);
        }

        fn read_line(&mut self) -> Option<String> {
            self.stdin_lines.pop_front()
        }

        fn run_forwarded(&mut self, program: &str, args: &[String]) -> i32 {
            self.forwarded_calls
                .push((program.to_string(), args.to_vec()));
            self.forwarded_exit_code
        }

        fn exec_replace(&mut self, program: &str, args: &[String]) -> i32 {
            self.exec_calls.push((program.to_string(), args.to_vec()));
            0
        }
    }

    struct StubRunner {
        list_json: String,
        bash_exit_code: i32,
        bash_stdout: String,
        bash_calls: RefCell<Vec<String>>,
    }

    impl StubRunner {
        fn new(list_json: String) -> Self {
            Self {
                list_json,
                bash_exit_code: 0,
                bash_stdout: String::new(),
                bash_calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl CommandRunner for StubRunner {
        fn run(&self, program: &str, args: &[&str]) -> CommandOutput {
            if program == "incus" && args.first() == Some(&"list") {
                return CommandOutput {
                    exit_code: 0,
                    std_out: self.list_json.clone(),
                    std_err: String::new(),
                };
            }
            if program == "incus" && args.first() == Some(&"query") {
                return CommandOutput {
                    exit_code: 0,
                    std_out: r#"[{"name":"snap-1","size":1048576}]"#.to_string(),
                    std_err: String::new(),
                };
            }
            if program == "/bin/bash" && args.first() == Some(&"-c") {
                self.bash_calls
                    .borrow_mut()
                    .push(args.get(1).copied().unwrap_or("").to_string());
                return CommandOutput {
                    exit_code: self.bash_exit_code,
                    std_out: self.bash_stdout.clone(),
                    std_err: String::new(),
                };
            }
            CommandOutput {
                exit_code: 0,
                std_out: String::new(),
                std_err: String::new(),
            }
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("aibox_cli_test_{name}_{nanos}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    fn svec(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn cli_parses_all_subcommands_and_flags() {
        {
            let p = parse_args(&[]);
            assert!(p.valid);
            assert_eq!(p.command, Command::None);
        }
        {
            let p = parse_args(&svec(&["--project", "/tmp/myproj", "list", "--json"]));
            assert!(p.valid);
            assert_eq!(p.command, Command::List);
            assert_eq!(p.project_dir, "/tmp/myproj");
            assert!(p.json_output);
        }
        {
            let p = parse_args(&svec(&["check", "--fix", "--json"]));
            assert!(p.valid);
            assert_eq!(p.command, Command::Check);
            assert!(p.fix);
            assert!(p.json_output);
        }
        {
            let p = parse_args(&svec(&["enter", "dev-box", "--project=/tmp/p2"]));
            assert!(p.valid);
            assert_eq!(p.command, Command::Enter);
            assert_eq!(p.env_name, "dev-box");
            assert_eq!(p.project_dir, "/tmp/p2");
        }
        {
            let p = parse_args(&svec(&["exec", "dev-box", "--", "ls", "-la", "--help"]));
            assert!(p.valid);
            assert_eq!(p.command, Command::Exec);
            assert_eq!(p.env_name, "dev-box");
            assert_eq!(p.exec_command, svec(&["ls", "-la", "--help"]));
        }
        {
            let p = parse_args(&svec(&["start", "dev-box"]));
            assert!(p.valid);
            assert_eq!(p.command, Command::Start);
            assert_eq!(p.env_name, "dev-box");
        }
        {
            let p = parse_args(&svec(&["stop", "dev-box"]));
            assert!(p.valid);
            assert_eq!(p.command, Command::Stop);
            assert_eq!(p.env_name, "dev-box");
        }
        {
            let p = parse_args(&svec(&["snapshot", "dev-box", "Pre Edit!"]));
            assert!(p.valid);
            assert_eq!(p.command, Command::Snapshot);
            assert_eq!(p.env_name, "dev-box");
            assert_eq!(p.snapshot_name, "pre-edit");
        }
        {
            let p = parse_args(&svec(&["snapshots", "dev-box", "--json"]));
            assert!(p.valid);
            assert_eq!(p.command, Command::Snapshots);
            assert_eq!(p.env_name, "dev-box");
            assert!(p.json_output);
        }
        {
            let p = parse_args(&svec(&[
                "restore",
                "dev-box",
                "snap-1",
                "--system-only",
                "--yes",
            ]));
            assert!(p.valid);
            assert_eq!(p.command, Command::Restore);
            assert_eq!(p.env_name, "dev-box");
            assert_eq!(p.snapshot_name, "snap-1");
            assert!(p.system_only);
            assert!(!p.workspace_only);
            assert!(p.yes);
        }
        {
            let p = parse_args(&svec(&["diff", "dev-box", "snap-1", "--json"]));
            assert!(p.valid);
            assert_eq!(p.command, Command::Diff);
            assert_eq!(p.env_name, "dev-box");
            assert_eq!(p.snapshot_name, "snap-1");
            assert!(p.json_output);
        }
        {
            let p = parse_args(&svec(&["restore", "--help"]));
            assert!(p.valid);
            assert_eq!(p.command, Command::Help);
            assert_eq!(p.help_target, Command::Restore);
        }

        let bad_cases = [
            svec(&["unknowncmd"]),
            svec(&["list", "--bogus"]),
            svec(&["--project"]),
            svec(&["--project="]),
            svec(&["exec", "dev-box", "ls"]),
            svec(&["exec", "dev-box", "--"]),
            svec(&["exec", "--", "ls"]),
            svec(&["start", "dev-box", "--json"]),
            svec(&[
                "restore",
                "dev-box",
                "s1",
                "--system-only",
                "--workspace-only",
            ]),
            svec(&["snapshot", "dev-box", "!!!"]),
        ];
        for bad in &bad_cases {
            let p = parse_args(bad);
            assert!(!p.valid, "expected invalid for {:?}", bad);
            assert_eq!(p.exit_code, 2, "expected exit code 2 for {:?}", bad);
        }
    }

    #[test]
    fn cli_project_resolution_and_lock_check() {
        assert_eq!(
            resolve_project_aibox_dir("", "/home/u/repo"),
            "/home/u/repo/aibox"
        );
        assert_eq!(
            resolve_project_aibox_dir("sub/proj", "/home/u/repo"),
            "/home/u/repo/sub/proj/aibox"
        );
        assert_eq!(
            resolve_project_aibox_dir("/tmp/custom/aibox", "/home/u/repo"),
            "/tmp/custom/aibox"
        );

        let proj_dir = temp_dir("proj_res");
        let aibox_dir = proj_dir.join("aibox");
        let state_dir = aibox_dir.join("state");
        fs::create_dir_all(&state_dir).expect("mkdir");
        fs::write(
            aibox_dir.join("config.json"),
            r#"{"firstRunCompleted":true,"autoSnapshotKeep":9,"diffExcludes":[".git/","out/"]}"#,
        )
        .expect("write config");

        let engine = Engine::new(Some(&proj_dir.to_string_lossy()), true, "/tmp");
        assert_eq!(
            engine.paths.workspace_base,
            format!("{}/workspaces", aibox_dir.display())
        );
        assert_eq!(engine.paths.state_dir, state_dir.to_string_lossy());
        assert_eq!(
            engine.paths.config_file_path,
            format!("{}/config.json", aibox_dir.display())
        );
        assert_eq!(engine.config.auto_snapshot_keep, 9);
        assert_eq!(engine.config.diff_excludes, svec(&[".git/", "out/"]));

        fs::write(state_dir.join("task.pid"), "99999999\n").expect("write stale pid");
        assert!(!engine.is_task_lock_active().0);

        fs::write(
            state_dir.join("task.pid"),
            format!("{}\n", std::process::id()),
        )
        .expect("write live pid");
        assert!(engine.is_task_lock_active().0);
        engine.release_cli_task_lock();
        assert!(!engine.is_task_lock_active().0);

        let _ = fs::remove_dir_all(&proj_dir);
    }

    #[test]
    fn cli_exec_and_exit_codes_with_stub_runner() {
        let dir = temp_dir("exec_codes");
        let proj = dir.join("proj");
        let ws_base = proj.join(".aibox/workspaces");
        let state_dir = proj.join(".aibox/state");
        fs::create_dir_all(ws_base.join("dev")).expect("mkdir ws");
        fs::create_dir_all(&state_dir).expect("mkdir state");
        fs::write(
            proj.join(".aibox/config.json"),
            r#"{"firstRunCompleted":true,"autoSnapshot":true,"autoSnapshotKeep":5}"#,
        )
        .expect("write cfg");

        let list_json = format!(
            r#"[{{"name":"dev","type":"container","status":"Stopped","config":{{"user.aibox.autosnap":"true","user.aibox.wssnap":"false"}},"expanded_devices":{{"workspace":{{"source":"{}/dev","path":"/workspace"}}}},"snapshots":[{{"name":"snap-1"}}]}},{{"name":"tpl","type":"container","status":"Stopped","config":{{"user.aibox.template":"true"}},"expanded_devices":{{}},"snapshots":[]}}]"#,
            ws_base.display()
        );
        let runner = StubRunner::new(list_json);
        let proj_str = proj.to_string_lossy().to_string();

        // exec dev -- true
        let mut io = TestCliIo::new();
        let rc = execute_with(
            &parse_args(&svec(&[
                "--project",
                &proj_str,
                "exec",
                "dev",
                "--",
                "true",
            ])),
            &mut io,
            &runner,
            "/tmp",
        );
        assert_eq!(rc, 0);
        assert_eq!(io.forwarded_calls.len(), 1);
        assert_eq!(io.forwarded_calls[0].0, "incus");
        assert!(io.forwarded_calls[0].1.ends_with(&svec(&["--", "true"])));
        let bash_log = runner.bash_calls.borrow().join("\n");
        assert!(bash_log.contains("incus snapshot create 'dev' 'auto-"));
        assert!(bash_log.contains("incus start 'dev'"));

        // exec dev -- false propagates exit code 1
        let mut io_fail = TestCliIo::new();
        io_fail.forwarded_exit_code = 1;
        let rc_fail = execute_with(
            &parse_args(&svec(&[
                "--project",
                &proj_str,
                "exec",
                "dev",
                "--",
                "false",
            ])),
            &mut io_fail,
            &runner,
            "/tmp",
        );
        assert_eq!(rc_fail, 1);

        // missing & template -> exit 4
        let mut io4 = TestCliIo::new();
        assert_eq!(
            execute_with(
                &parse_args(&svec(&[
                    "--project",
                    &proj_str,
                    "exec",
                    "missing",
                    "--",
                    "true"
                ])),
                &mut io4,
                &runner,
                "/tmp",
            ),
            4
        );
        assert_eq!(
            execute_with(
                &parse_args(&svec(&[
                    "--project",
                    &proj_str,
                    "exec",
                    "tpl",
                    "--",
                    "true"
                ])),
                &mut io4,
                &runner,
                "/tmp",
            ),
            4
        );
        assert_eq!(
            execute_with(
                &parse_args(&svec(&["--project", &proj_str, "start", "tpl"])),
                &mut io4,
                &runner,
                "/tmp",
            ),
            4
        );

        // live background task lock -> mutating commands exit 3
        fs::write(
            state_dir.join("task.pid"),
            format!("{}\n", std::process::id()),
        )
        .expect("write lock");
        let mut io3 = TestCliIo::new();
        assert_eq!(
            execute_with(
                &parse_args(&svec(&["--project", &proj_str, "start", "dev"])),
                &mut io3,
                &runner,
                "/tmp",
            ),
            3
        );
        assert!(io3.stderr_str().contains("background task"));
        let _ = fs::remove_file(state_dir.join("task.pid"));

        // restore confirmation prompt: N aborts (exit 1), yes proceeds (exit 0)
        let mut io_abort = TestCliIo::new();
        io_abort.stdin_lines.push_back("n\n".to_string());
        assert_eq!(
            execute_with(
                &parse_args(&svec(&[
                    "--project",
                    &proj_str,
                    "restore",
                    "dev",
                    "snap-1",
                    "--system-only"
                ])),
                &mut io_abort,
                &runner,
                "/tmp",
            ),
            1
        );
        assert!(io_abort.stderr_str().contains("Aborted."));

        let mut io_restore = TestCliIo::new();
        assert_eq!(
            execute_with(
                &parse_args(&svec(&[
                    "--project",
                    &proj_str,
                    "restore",
                    "dev",
                    "snap-1",
                    "--system-only",
                    "--yes"
                ])),
                &mut io_restore,
                &runner,
                "/tmp",
            ),
            0
        );

        // list --json
        let mut io_list = TestCliIo::new();
        assert_eq!(
            execute_with(
                &parse_args(&svec(&["--project", &proj_str, "list", "--json"])),
                &mut io_list,
                &runner,
                "/tmp",
            ),
            0
        );
        assert!(io_list.stdout_str().contains("\"name\": \"dev\""));

        // snapshots --json
        let mut io_snaps = TestCliIo::new();
        assert_eq!(
            execute_with(
                &parse_args(&svec(&["--project", &proj_str, "snapshots", "dev"])),
                &mut io_snaps,
                &runner,
                "/tmp",
            ),
            0
        );
        assert!(io_snaps.stdout_str().contains("snap-1"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cli_secrets_exec_injection_and_history_redaction() {
        let dir = temp_dir("secrets_exec");
        let proj = dir.join("proj");
        let state_dir = proj.join(".aibox/state");
        fs::create_dir_all(&state_dir).expect("mkdir");
        let secrets_path = dir.join("secrets.json");
        crate::secrets::store::save_secrets_store(
            &secrets_path.to_string_lossy(),
            &[("MY_TOKEN".to_string(), "tok-999-secret".to_string())],
            &[],
            &[],
        )
        .expect("save secrets");

        let bin_dir = dir.join("bin");
        fs::create_dir_all(&bin_dir).expect("mkdir bin");
        let incus_stub = bin_dir.join("incus");
        fs::write(&incus_stub, "#!/bin/sh\nexit 0\n").expect("write stub");
        fs::set_permissions(&incus_stub, fs::Permissions::from_mode(0o755)).expect("chmod");

        let prev_secrets = std::env::var_os("AIBOX_SECRETS_PATH");
        let prev_path = std::env::var_os("PATH");
        std::env::set_var("AIBOX_SECRETS_PATH", &secrets_path);
        let new_path = match &prev_path {
            Some(p) => format!("{}:{}", bin_dir.display(), p.to_string_lossy()),
            None => bin_dir.to_string_lossy().to_string(),
        };
        std::env::set_var("PATH", new_path);

        let list_json = format!(
            r#"[{{"name":"secbox","type":"container","status":"Running","config":{{"user.aibox.project":"{}","user.aibox.secrets":"MY_TOKEN"}},"devices":{{}},"state":{{}}}}]"#,
            proj.display()
        );
        let runner = StubRunner::new(list_json);
        let mut io = TestCliIo::new();
        let proj_str = proj.to_string_lossy().to_string();

        let rc = execute_with(
            &parse_args(&svec(&[
                "--project",
                &proj_str,
                "exec",
                "secbox",
                "--",
                "env",
            ])),
            &mut io,
            &runner,
            "/tmp",
        );
        assert_eq!(rc, 0);
        assert_eq!(io.forwarded_calls.len(), 1);
        assert_eq!(io.forwarded_calls[0].0, "incus");
        // Verify env-file was injected into incus exec args
        assert!(io.forwarded_calls[0]
            .1
            .iter()
            .any(|a| a.contains("/dev/shm/aibox-")));
        // Verify secret value is NOT in history
        let history = crate::history::load_operation_history(&state_dir.to_string_lossy());
        let history_json = serde_json::to_string(&history).unwrap();
        assert!(!history_json.contains("tok-999-secret"));

        if let Some(p) = prev_secrets {
            std::env::set_var("AIBOX_SECRETS_PATH", p);
        } else {
            std::env::remove_var("AIBOX_SECRETS_PATH");
        }
        if let Some(p) = prev_path {
            std::env::set_var("PATH", p);
        } else {
            std::env::remove_var("PATH");
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
