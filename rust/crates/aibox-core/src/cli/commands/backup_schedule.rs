use crate::backup::schedule::{install_schedule_script, BackupCadence};
use crate::cli::commands::require_environment;
use crate::cli::{CliIo, ExitCode, ParsedArgs};
use crate::engine::Engine;
use crate::process::CommandRunner;

pub fn run_backup_schedule(
    engine: &mut Engine,
    parsed: &ParsedArgs,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
) -> i32 {
    let Some(cadence) = BackupCadence::parse(&parsed.schedule_cadence) else {
        io.write_stderr(b"Error: Cadence must be 'daily', 'weekly', or 'off'.\n");
        return ExitCode::Usage.as_i32();
    };
    let env = match require_environment(engine, io, runner, &parsed.env_name, true) {
        Ok(e) => e,
        Err(rc) => return rc,
    };

    let aibox_bin = std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "aibox".to_string());

    let script = install_schedule_script(
        &env.name,
        cadence,
        &aibox_bin,
        &parsed.project_dir,
        parsed.backup_keep,
    );
    let out = runner.run("/bin/bash", &["-c", &script]);
    if out.exit_code != 0 {
        if !out.std_err.is_empty() {
            io.write_stderr(out.std_err.as_bytes());
        }
        return ExitCode::Failed.as_i32();
    }

    match cadence {
        BackupCadence::Off => {
            io.write_stdout(format!("Disabled scheduled backup for '{}'.\n", env.name).as_bytes());
        }
        BackupCadence::Daily | BackupCadence::Weekly => {
            io.write_stdout(
                format!(
                    "Enabled {} backup schedule for '{}' (keep {}).\n",
                    cadence.on_calendar(),
                    env.name,
                    parsed.backup_keep
                )
                .as_bytes(),
            );
        }
    }
    ExitCode::Ok.as_i32()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::parse_args;
    use crate::process::CommandOutput;
    use std::cell::RefCell;

    struct DummyIo {
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    }

    impl DummyIo {
        fn new() -> Self {
            Self {
                stdout: Vec::new(),
                stderr: Vec::new(),
            }
        }
    }

    impl CliIo for DummyIo {
        fn write_stdout(&mut self, data: &[u8]) {
            self.stdout.extend_from_slice(data);
        }

        fn write_stderr(&mut self, data: &[u8]) {
            self.stderr.extend_from_slice(data);
        }

        fn read_line(&mut self) -> Option<String> {
            None
        }

        fn run_forwarded(&mut self, _program: &str, _args: &[String]) -> i32 {
            0
        }

        fn exec_replace(&mut self, _program: &str, _args: &[String]) -> i32 {
            0
        }
    }

    struct SchedRunner(RefCell<Vec<String>>);

    impl CommandRunner for SchedRunner {
        fn run(&self, program: &str, args: &[&str]) -> CommandOutput {
            if program == "incus" && args.first() == Some(&"list") {
                return CommandOutput {
                    exit_code: 0,
                    std_out: r#"[{"name":"db","type":"container","status":"Running","config":{}}]"#
                        .to_string(),
                    std_err: String::new(),
                };
            }
            if program == "/bin/bash" && args.first() == Some(&"-c") {
                self.0
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
    fn run_backup_schedule_installs_and_disables_timer() {
        let dir = std::env::temp_dir().join("aibox_backup_sched_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let mut engine = Engine::new(Some(&dir.to_string_lossy()), true, "/tmp");
        let runner = SchedRunner(RefCell::new(Vec::new()));

        let mut io_on = DummyIo::new();
        let p_on = parse_args(&[
            "backup-schedule".into(),
            "db".into(),
            "daily".into(),
            "--keep".into(),
            "10".into(),
        ]);
        assert!(p_on.valid);
        assert_eq!(
            run_backup_schedule(&mut engine, &p_on, &mut io_on, &runner),
            0
        );
        assert!(String::from_utf8_lossy(&io_on.stdout).contains("Enabled daily backup schedule"));
        assert!(runner.0.borrow()[0].contains("OnCalendar=daily"));

        let mut io_off = DummyIo::new();
        let p_off = parse_args(&["backup-schedule".into(), "db".into(), "off".into()]);
        assert!(p_off.valid);
        assert_eq!(
            run_backup_schedule(&mut engine, &p_off, &mut io_off, &runner),
            0
        );
        assert!(String::from_utf8_lossy(&io_off.stdout).contains("Disabled scheduled backup"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
