use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cli::commands::{require_environment, run_script_with_task_lock};
use crate::cli::{CliIo, ExitCode, ParsedArgs};
use crate::engine::Engine;
use crate::network::allowlist::{parse_allowlist_entries, resolve_allowlist_cidrs, SystemResolver};
use crate::process::CommandRunner;
use crate::provision::clone::clone_environment_script_with_source;
use crate::provision::is_vm_env_type;
use crate::provision::lifecycle::exec_incus_command_args_with_env;
use crate::snapshot::workspace::container_workspace_path_for;
use crate::util::names::sanitize_name;
use crate::util::paths::workspace_path_for;
use crate::util::shell::shell_quote;

static EPHEMERAL_SEQ: AtomicU64 = AtomicU64::new(1);

fn build_ephemeral_name(template_name: &str, suffix: &str) -> String {
    let base = sanitize_name(template_name);
    let prefix = if base.is_empty() { "box" } else { &base };
    let max_prefix = 63usize.saturating_sub(4 + suffix.len() + 1);
    let truncated = prefix
        .get(..prefix.len().min(max_prefix))
        .unwrap_or(prefix)
        .trim_end_matches('-');
    let safe_prefix = if truncated.is_empty() {
        "box"
    } else {
        truncated
    };
    format!("eph-{safe_prefix}-{suffix}")
}

/// Builds a deterministic ephemeral environment name from `template_name` and `epoch`.
pub fn ephemeral_env_name(template_name: &str, epoch: i64) -> String {
    build_ephemeral_name(template_name, &epoch.to_string())
}

/// Builds the cleanup script that deletes the ephemeral Incus instance and its workspace.
pub fn ephemeral_cleanup_script(env_name: &str, workspace_path: &str) -> String {
    let q_env = shell_quote(env_name);
    let q_ws = shell_quote(workspace_path);
    let q_acl = shell_quote(&format!("aibox-{env_name}"));
    format!(
        "incus delete --force {q_env} >/dev/null 2>&1 || true\n\
         incus network acl delete {q_acl} >/dev/null 2>&1 || true\n\
         btrfs subvolume delete {q_ws} >/dev/null 2>&1 || true\n\
         rm -rf {q_ws}"
    )
}

pub fn run_ephemeral(
    engine: &mut Engine,
    parsed: &ParsedArgs,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
) -> i32 {
    let src = match require_environment(engine, io, runner, &parsed.from_template, false) {
        Ok(e) => e,
        Err(rc) => return rc,
    };

    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let seq = EPHEMERAL_SEQ.fetch_add(1, Ordering::Relaxed);
    let suffix = format!("{epoch}-{}-{seq}", std::process::id());
    let eph_name = build_ephemeral_name(&src.name, &suffix);
    let ws_path = workspace_path_for(&engine.paths.workspace_base, &[], &eph_name);
    let is_vm = is_vm_env_type(&src.env_type);

    let resolved_cidrs = if src.network_mode == "allowlist" {
        let entries = parse_allowlist_entries(&src.network_allowlist);
        let (cidrs, warnings) = resolve_allowlist_cidrs(&entries, &SystemResolver);
        for w in &warnings {
            io.write_stderr(format!("Warning: {w}\n").as_bytes());
        }
        cidrs
    } else {
        Vec::new()
    };

    let clone_script = clone_environment_script_with_source(
        &src.name,
        &eph_name,
        &ws_path,
        &src.workspace_path,
        "",
        "",
        0,
        true,
        &src.network_mode,
        &src.network_allowlist,
        &resolved_cidrs,
        "",
        "",
        "",
        is_vm,
    );

    let clone_rc = run_script_with_task_lock(
        engine,
        io,
        runner,
        &format!("Creating ephemeral '{}' from '{}'", eph_name, src.name),
        &eph_name,
        &clone_script,
        false,
    );
    if clone_rc == ExitCode::Busy.as_i32() {
        return ExitCode::Busy.as_i32();
    }
    if clone_rc != 0 {
        let cleanup = ephemeral_cleanup_script(&eph_name, &ws_path);
        let _ = runner.run("/bin/bash", &["-c", &cleanup]);
        return ExitCode::Failed.as_i32();
    }

    let c_ws_path = container_workspace_path_for(&eph_name, is_vm);
    let exec_args =
        exec_incus_command_args_with_env(&eph_name, &parsed.exec_command, "", &c_ws_path, &[]);
    let cmd_rc = io.run_forwarded("incus", &exec_args);

    if cmd_rc == 0 || !parsed.keep_on_fail {
        let cleanup = ephemeral_cleanup_script(&eph_name, &ws_path);
        let _ = runner.run("/bin/bash", &["-c", &cleanup]);
    } else {
        io.write_stderr(
            format!("Kept failed ephemeral environment '{eph_name}' for inspection.\n").as_bytes(),
        );
    }

    cmd_rc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::parse_args;
    use crate::process::CommandOutput;
    use std::cell::RefCell;

    struct TestIo {
        forwarded_exit: i32,
        stderr: Vec<u8>,
    }

    impl TestIo {
        fn new(forwarded_exit: i32) -> Self {
            Self {
                forwarded_exit,
                stderr: Vec::new(),
            }
        }
    }

    impl CliIo for TestIo {
        fn write_stdout(&mut self, _data: &[u8]) {}

        fn write_stderr(&mut self, data: &[u8]) {
            self.stderr.extend_from_slice(data);
        }

        fn read_line(&mut self) -> Option<String> {
            None
        }

        fn run_forwarded(&mut self, _program: &str, _args: &[String]) -> i32 {
            self.forwarded_exit
        }

        fn exec_replace(&mut self, _program: &str, _args: &[String]) -> i32 {
            0
        }
    }

    struct EphRunner {
        bash_scripts: RefCell<Vec<String>>,
    }

    impl EphRunner {
        fn new() -> Self {
            Self {
                bash_scripts: RefCell::new(Vec::new()),
            }
        }
    }

    impl CommandRunner for EphRunner {
        fn run(&self, program: &str, args: &[&str]) -> CommandOutput {
            if program == "incus" && args.first() == Some(&"list") {
                return CommandOutput {
                    exit_code: 0,
                    std_out: r#"[{"name":"base-tpl","type":"container","status":"Stopped","config":{"user.aibox.template":"true"}}]"#.to_string(),
                    std_err: String::new(),
                };
            }
            if program == "/bin/bash" && args.first() == Some(&"-c") {
                self.bash_scripts
                    .borrow_mut()
                    .push(args.get(1).copied().unwrap_or("").to_string());
            }
            CommandOutput {
                exit_code: 0,
                std_out: String::new(),
                std_err: String::new(),
            }
        }

        fn socket_writable(&self, _path: &str) -> bool {
            true
        }
    }

    #[test]
    fn ephemeral_naming_and_cleanup_scripts() {
        assert_eq!(ephemeral_env_name("Base Tpl!", 12345), "eph-base-tpl-12345");
        let cleanup = ephemeral_cleanup_script("eph-box-1", "/tmp/ws/eph-box-1");
        assert!(cleanup.contains("incus delete --force 'eph-box-1'"));
        assert!(cleanup.contains("rm -rf '/tmp/ws/eph-box-1'"));
    }

    #[test]
    fn run_ephemeral_cleans_up_on_success_and_keeps_on_fail_when_requested() {
        let dir = std::env::temp_dir().join("aibox_eph_run_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("aibox/state")).expect("mkdir");
        let proj = dir.to_string_lossy().to_string();

        let mut engine = Engine::new(Some(&proj), true, "/tmp");
        let runner_ok = EphRunner::new();
        let mut io_ok = TestIo::new(0);
        let parsed_ok = parse_args(&[
            "run".into(),
            "--ephemeral".into(),
            "--from".into(),
            "base-tpl".into(),
            "--".into(),
            "true".into(),
        ]);
        assert!(parsed_ok.valid);
        assert_eq!(
            run_ephemeral(&mut engine, &parsed_ok, &mut io_ok, &runner_ok),
            0
        );
        assert!(runner_ok
            .bash_scripts
            .borrow()
            .iter()
            .any(|s| s.contains("incus delete --force")));

        let runner_keep = EphRunner::new();
        let mut io_keep = TestIo::new(2);
        let parsed_keep = parse_args(&[
            "run".into(),
            "--ephemeral".into(),
            "--from".into(),
            "base-tpl".into(),
            "--keep-on-fail".into(),
            "--".into(),
            "false".into(),
        ]);
        assert_eq!(
            run_ephemeral(&mut engine, &parsed_keep, &mut io_keep, &runner_keep),
            2
        );
        assert!(!runner_keep
            .bash_scripts
            .borrow()
            .iter()
            .any(|s| s.contains("incus delete --force")));
        assert!(String::from_utf8_lossy(&io_keep.stderr).contains("Kept failed ephemeral"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
