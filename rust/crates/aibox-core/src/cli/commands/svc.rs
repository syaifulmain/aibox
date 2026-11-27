use crate::cli::commands::{require_environment, to_json_qt_indented};
use crate::cli::{CliIo, ExitCode, ParsedArgs};
use crate::engine::Engine;
use crate::process::CommandRunner;
use crate::services::{
    list_services_script, parse_service_list, service_action_script, service_logs_script,
};

pub fn run_svc(
    engine: &mut Engine,
    parsed: &ParsedArgs,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
) -> i32 {
    let env = match require_environment(engine, io, runner, &parsed.env_name, true) {
        Ok(e) => e,
        Err(rc) => return rc,
    };
    if !env.running {
        io.write_stderr(
            format!(
                "Error: Environment '{}' is not running. Start it first.\n",
                env.name
            )
            .as_bytes(),
        );
        return ExitCode::Failed.as_i32();
    }

    let action = parsed.svc_action.as_str();
    let script = match action {
        "list" => list_services_script(&env.name),
        "logs" => match service_logs_script(&env.name, &parsed.svc_unit, parsed.log_lines) {
            Ok(s) => s,
            Err(err) => {
                io.write_stderr(format!("Error: {err}\n").as_bytes());
                return ExitCode::Usage.as_i32();
            }
        },
        _ => match service_action_script(&env.name, action, &parsed.svc_unit) {
            Ok(s) => s,
            Err(err) => {
                io.write_stderr(format!("Error: {err}\n").as_bytes());
                return ExitCode::Usage.as_i32();
            }
        },
    };

    let out = runner.run("/bin/bash", &["-c", &script]);
    if out.exit_code != 0 {
        if !out.std_err.is_empty() {
            io.write_stderr(out.std_err.as_bytes());
        } else if !out.std_out.is_empty() {
            io.write_stderr(out.std_out.as_bytes());
        }
        return ExitCode::Failed.as_i32();
    }

    if action == "list" {
        let services = parse_service_list(&out.std_out);
        if parsed.json_output {
            let json_text = to_json_qt_indented(&services).unwrap_or_else(|_| "[\n]\n".to_string());
            io.write_stdout(json_text.as_bytes());
        } else if services.is_empty() {
            io.write_stdout(b"No services found.\n");
        } else {
            for svc in &services {
                io.write_stdout(
                    format!(
                        "{:<34} {:<10} {:<10} {}\n",
                        svc.unit, svc.active, svc.sub, svc.description
                    )
                    .as_bytes(),
                );
            }
        }
        return ExitCode::Ok.as_i32();
    }

    if !out.std_out.is_empty() {
        io.write_stdout(out.std_out.as_bytes());
    }
    if !out.std_err.is_empty() {
        io.write_stderr(out.std_err.as_bytes());
    }
    ExitCode::Ok.as_i32()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::parse_args;
    use crate::process::CommandOutput;

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

    struct SvcRunner {
        list_json: String,
        bash_stdout: String,
    }

    impl CommandRunner for SvcRunner {
        fn run(&self, program: &str, args: &[&str]) -> CommandOutput {
            if program == "incus" && args.first() == Some(&"list") {
                return CommandOutput {
                    exit_code: 0,
                    std_out: self.list_json.clone(),
                    std_err: String::new(),
                };
            }
            CommandOutput {
                exit_code: 0,
                std_out: self.bash_stdout.clone(),
                std_err: String::new(),
            }
        }

        fn socket_writable(&self, _path: &str) -> bool {
            true
        }
    }

    #[test]
    fn run_svc_lists_json_and_rejects_stopped_env() {
        let dir = std::env::temp_dir().join("aibox_svc_cli_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let mut engine = Engine::new(Some(&dir.to_string_lossy()), true, "/tmp");

        let runner = SvcRunner {
            list_json: r#"[
                {"name":"web","type":"container","status":"Running","config":{}},
                {"name":"off","type":"container","status":"Stopped","config":{}}
            ]"#
            .to_string(),
            bash_stdout: "__MGR:systemd\nnginx.service loaded active running Nginx\n".to_string(),
        };

        let mut io_ok = DummyIo::new();
        let parsed = parse_args(&[
            "svc".to_string(),
            "web".to_string(),
            "list".to_string(),
            "--json".to_string(),
        ]);
        assert!(parsed.valid);
        assert_eq!(run_svc(&mut engine, &parsed, &mut io_ok, &runner), 0);
        let out = String::from_utf8_lossy(&io_ok.stdout);
        assert!(out.contains("\"unit\": \"nginx.service\""));

        let mut io_stopped = DummyIo::new();
        let parsed_stopped =
            parse_args(&["svc".to_string(), "off".to_string(), "list".to_string()]);
        assert_eq!(
            run_svc(&mut engine, &parsed_stopped, &mut io_stopped, &runner),
            ExitCode::Failed.as_i32()
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
