use crate::cli::commands::{require_environment, to_json_qt_indented};
use crate::cli::{CliIo, ExitCode, ParsedArgs};
use crate::engine::Engine;
use crate::health::probe::{
    notify_healthcheck_down_script, probe_target_script, set_healthcheck_script,
    validate_probe_target, HealthProbeResult,
};
use crate::process::CommandRunner;
use crate::util::shell::shell_quote;

pub fn run_healthcheck(
    engine: &mut Engine,
    parsed: &ParsedArgs,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
) -> i32 {
    let env = match require_environment(engine, io, runner, &parsed.env_name, true) {
        Ok(e) => e,
        Err(rc) => return rc,
    };

    if parsed.health_off {
        let script = set_healthcheck_script(&env.name, "", false);
        let out = runner.run("/bin/bash", &["-c", &script]);
        if out.exit_code != 0 {
            io.write_stderr(out.std_err.as_bytes());
            return ExitCode::Failed.as_i32();
        }
        io.write_stdout(format!("Cleared health check for '{}'.\n", env.name).as_bytes());
        return ExitCode::Ok.as_i32();
    }

    if parsed.health_set {
        let target = match validate_probe_target(&parsed.health_url) {
            Ok(t) => t,
            Err(err) => {
                io.write_stderr(format!("Error: {err}\n").as_bytes());
                return ExitCode::Usage.as_i32();
            }
        };
        let script = set_healthcheck_script(&env.name, &target, parsed.auto_restart);
        let out = runner.run("/bin/bash", &["-c", &script]);
        if out.exit_code != 0 {
            io.write_stderr(out.std_err.as_bytes());
            return ExitCode::Failed.as_i32();
        }
        io.write_stdout(
            format!("Configured health check for '{}' -> {}\n", env.name, target).as_bytes(),
        );
        return ExitCode::Ok.as_i32();
    }

    let raw_target = if !parsed.health_url.is_empty() {
        parsed.health_url.clone()
    } else {
        let cfg_out = runner.run(
            "incus",
            &["config", "get", &env.name, "user.aibox.healthcheck"],
        );
        cfg_out.std_out.trim().to_string()
    };

    if raw_target.is_empty() {
        io.write_stderr(
            format!(
                "Error: No health check target configured for '{}'. Use --url or --set.\n",
                env.name
            )
            .as_bytes(),
        );
        return ExitCode::Usage.as_i32();
    }

    let target = match validate_probe_target(&raw_target) {
        Ok(t) => t,
        Err(err) => {
            io.write_stderr(format!("Error: {err}\n").as_bytes());
            return ExitCode::Usage.as_i32();
        }
    };

    let auto_restart = if parsed.auto_restart {
        true
    } else {
        let cfg_out = runner.run(
            "incus",
            &["config", "get", &env.name, "user.aibox.healthcheck_restart"],
        );
        cfg_out.std_out.trim().eq_ignore_ascii_case("true")
    };

    let probe_script = probe_target_script(&target);
    let probe_out = runner.run("/bin/bash", &["-c", &probe_script]);
    let healthy = probe_out.exit_code == 0;
    let mut restarted = false;

    if !healthy {
        if auto_restart {
            let q_name = shell_quote(&env.name);
            let restart_out = runner.run("/bin/bash", &["-c", &format!("incus restart {q_name}")]);
            restarted = restart_out.exit_code == 0;
        }
        let notify = notify_healthcheck_down_script(&env.name, &target, restarted);
        let _ = runner.run("/bin/bash", &["-c", &notify]);
    }

    let detail = if healthy {
        "OK".to_string()
    } else if !probe_out.std_err.trim().is_empty() {
        probe_out.std_err.trim().to_string()
    } else {
        "Probe failed".to_string()
    };

    let result = HealthProbeResult {
        env_name: env.name.clone(),
        target: target.clone(),
        healthy,
        detail: detail.clone(),
        restarted,
    };

    if parsed.json_output {
        let json_text = to_json_qt_indented(&result).unwrap_or_else(|_| "{}\n".to_string());
        io.write_stdout(json_text.as_bytes());
    } else {
        let tag = if healthy { "[OK]" } else { "[DOWN]" };
        let suffix = if restarted { " (restarted)" } else { "" };
        io.write_stdout(
            format!("{tag} {} ({}): {}{suffix}\n", env.name, target, detail).as_bytes(),
        );
    }

    if healthy {
        ExitCode::Ok.as_i32()
    } else {
        ExitCode::Failed.as_i32()
    }
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

    struct ProbeRunner {
        probe_exit: i32,
        bash_scripts: RefCell<Vec<String>>,
    }

    impl CommandRunner for ProbeRunner {
        fn run(&self, program: &str, args: &[&str]) -> CommandOutput {
            if program == "incus" && args.first() == Some(&"list") {
                return CommandOutput {
                    exit_code: 0,
                    std_out:
                        r#"[{"name":"web","type":"container","status":"Running","config":{}}]"#
                            .to_string(),
                    std_err: String::new(),
                };
            }
            if program == "/bin/bash" && args.first() == Some(&"-c") {
                let script = args.get(1).copied().unwrap_or("").to_string();
                self.bash_scripts.borrow_mut().push(script.clone());
                if script.contains("curl") || script.contains("nc -z") {
                    return CommandOutput {
                        exit_code: self.probe_exit,
                        std_out: String::new(),
                        std_err: if self.probe_exit == 0 {
                            String::new()
                        } else {
                            "connection refused".to_string()
                        },
                    };
                }
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
    fn healthcheck_reports_ok_and_restarts_when_down() {
        let dir = std::env::temp_dir().join("aibox_healthcheck_cli_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let mut engine = Engine::new(Some(&dir.to_string_lossy()), true, "/tmp");

        let ok_runner = ProbeRunner {
            probe_exit: 0,
            bash_scripts: RefCell::new(Vec::new()),
        };
        let mut io_ok = DummyIo::new();
        let p_ok = parse_args(&[
            "healthcheck".into(),
            "web".into(),
            "--url".into(),
            "3000".into(),
            "--json".into(),
        ]);
        assert!(p_ok.valid);
        assert_eq!(
            run_healthcheck(&mut engine, &p_ok, &mut io_ok, &ok_runner),
            0
        );
        assert!(String::from_utf8_lossy(&io_ok.stdout).contains("\"healthy\": true"));

        let down_runner = ProbeRunner {
            probe_exit: 1,
            bash_scripts: RefCell::new(Vec::new()),
        };
        let mut io_down = DummyIo::new();
        let p_down = parse_args(&[
            "healthcheck".into(),
            "web".into(),
            "--url".into(),
            "http://127.0.0.1:3000/health".into(),
            "--auto-restart".into(),
        ]);
        assert_eq!(
            run_healthcheck(&mut engine, &p_down, &mut io_down, &down_runner),
            1
        );
        let scripts = down_runner.bash_scripts.borrow().join("\n");
        assert!(scripts.contains("incus restart 'web'"));
        assert!(scripts.contains("notify-send"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
