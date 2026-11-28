use crate::common;
use aibox_core::*;
use serde_json::{json, Value};
use std::env;
use std::fs;

fn dispatch(fn_name: &str, _case: &str, args: &Value) -> Value {
    match fn_name {
        "Cli::helpText" => {
            let cmd = match args["cmd"].as_str().expect("cmd") {
                "none" => cli::Command::None,
                "help" => cli::Command::Help,
                "list" => cli::Command::List,
                "check" => cli::Command::Check,
                "enter" => cli::Command::Enter,
                "exec" => cli::Command::Exec,
                "start" => cli::Command::Start,
                "stop" => cli::Command::Stop,
                "snapshot" => cli::Command::Snapshot,
                "snapshots" => cli::Command::Snapshots,
                "restore" => cli::Command::Restore,
                "diff" => cli::Command::Diff,
                "theme" => cli::Command::Theme,
                "completions" => cli::Command::Completions,
                other => panic!("unknown cmd {other}"),
            };
            json!(cli::help_text(cmd))
        }
        "Cli::parseArgs" => {
            let cli_args: Vec<String> = args["args"]
                .as_array()
                .expect("args")
                .iter()
                .map(|v| v.as_str().expect("str").to_string())
                .collect();
            let p = cli::parse_args(&cli_args);
            json!({
                "valid": p.valid,
                "exitCode": p.exit_code,
                "errorMessage": p.error_message,
                "command": p.command.as_str(),
                "helpTarget": p.help_target.as_str(),
                "projectDir": p.project_dir,
                "envName": p.env_name,
                "snapshotName": p.snapshot_name,
                "execCommand": p.exec_command,
                "jsonOutput": p.json_output,
                "fix": p.fix,
                "systemOnly": p.system_only,
                "workspaceOnly": p.workspace_only,
                "yes": p.yes,
            })
        }
        "Cli::isCliInvocation" => {
            let cli_args: Vec<String> = args["args"]
                .as_array()
                .expect("args")
                .iter()
                .map(|v| v.as_str().expect("str").to_string())
                .collect();
            json!(cli::is_cli_invocation(&cli_args))
        }
        "Cli::execute" => {
            use std::os::unix::fs::PermissionsExt;
            let work_dir = "/tmp/aibox-golden-cli-rust";
            let _ = fs::remove_dir_all(work_dir);
            let stub_dir = format!("{work_dir}/bin");
            let proj_dir = format!("{work_dir}/proj");
            let ws_base = format!("{proj_dir}/.aibox/workspaces");
            let state_dir = format!("{proj_dir}/.aibox/state");
            fs::create_dir_all(&stub_dir).unwrap();
            fs::create_dir_all(format!("{ws_base}/dev")).unwrap();
            fs::create_dir_all(&state_dir).unwrap();
            fs::write(
                format!("{proj_dir}/.aibox/config.json"),
                r#"{"firstRunCompleted":true,"autoSnapshot":true,"autoSnapshotKeep":5}"#,
            )
            .unwrap();
            let incus_log = format!("{work_dir}/incus.log");
            let list_json_file = format!("{work_dir}/list.json");
            let list_json = format!(
                r#"[
  {{
    "name": "dev",
    "type": "container",
    "status": "Stopped",
    "config": {{ "limits.cpu": "8", "user.aibox.autosnap": "true", "user.aibox.wssnap": "false" }},
    "expanded_devices": {{ "workspace": {{ "source": "{ws_base}/dev", "path": "/workspace" }} }},
    "snapshots": ["snap-1"]
  }},
  {{
    "name": "tpl",
    "type": "container",
    "status": "Stopped",
    "config": {{ "limits.cpu": "8", "user.aibox.template": "true" }},
    "expanded_devices": {{}},
    "snapshots": []
  }}
]"#
            );
            fs::write(&list_json_file, list_json).unwrap();
            let stub_path = format!("{stub_dir}/incus");
            let script = format!(
                r#"#!/bin/sh
echo "incus $*" >> "{incus_log}"
if [ "$1" = "list" ]; then
    cat "{list_json_file}"
    exit 0
fi
if [ "$1" = "info" ]; then
    echo "Status: STOPPED"
    exit 0
fi
if [ "$1" = "exec" ]; then
    while [ "$#" -gt 0 ]; do
        if [ "$1" = "--" ]; then
            shift
            break
        fi
        shift
    done
    exec "$@"
fi
if [ "$1" = "query" ]; then
    echo '[{{"name":"snap-1","size":1048576}}]'
    exit 0
fi
exit 0
"#
            );
            fs::write(&stub_path, script).unwrap();
            fs::set_permissions(&stub_path, fs::Permissions::from_mode(0o755)).unwrap();

            let cli_args: Vec<String> = args["args"]
                .as_array()
                .expect("args")
                .iter()
                .map(|v| {
                    v.as_str()
                        .expect("str")
                        .replace("/tmp/aibox-golden-cli/proj", &proj_dir)
                })
                .collect();

            struct CaptureIo {
                stdout: Vec<u8>,
                stderr: Vec<u8>,
            }
            impl cli::CliIo for CaptureIo {
                fn write_stdout(&mut self, data: &[u8]) {
                    self.stdout.extend_from_slice(data);
                }
                fn write_stderr(&mut self, data: &[u8]) {
                    self.stderr.extend_from_slice(data);
                }
                fn read_line(&mut self) -> Option<String> {
                    None
                }
                fn run_forwarded(&mut self, program: &str, args: &[String]) -> i32 {
                    std::process::Command::new(program)
                        .args(args)
                        .status()
                        .map(|s| s.code().unwrap_or(1))
                        .unwrap_or(1)
                }
                fn exec_replace(&mut self, _program: &str, _args: &[String]) -> i32 {
                    0
                }
            }

            let old_path = env::var_os("PATH");
            let new_path = match &old_path {
                Some(p) => format!("{stub_dir}:{}", p.to_string_lossy()),
                None => stub_dir.clone(),
            };
            env::set_var("PATH", new_path);

            let mut io = CaptureIo {
                stdout: Vec::new(),
                stderr: Vec::new(),
            };
            let runner = process::SystemRunner;
            let rc = cli::execute_with(&cli::parse_args(&cli_args), &mut io, &runner, "/tmp");

            if let Some(p) = old_path {
                env::set_var("PATH", p);
            } else {
                env::remove_var("PATH");
            }

            let auto_re = regex::Regex::new(r"auto-\d{8}-\d{6}").unwrap();
            let last_used_re = regex::Regex::new(r"user\.aibox\.lastused=\d+").unwrap();
            let stdout_str = auto_re
                .replace_all(&String::from_utf8_lossy(&io.stdout), "auto-<TS>")
                .replace(work_dir, "/tmp/aibox-golden-cli");
            let stderr_str = auto_re
                .replace_all(&String::from_utf8_lossy(&io.stderr), "auto-<TS>")
                .replace(work_dir, "/tmp/aibox-golden-cli");
            let log_raw = auto_re
                .replace_all(
                    &fs::read_to_string(&incus_log).unwrap_or_default(),
                    "auto-<TS>",
                )
                .replace(work_dir, "/tmp/aibox-golden-cli");
            let log_str = last_used_re
                .replace_all(&log_raw, "user.aibox.lastused=<EPOCH>")
                .to_string();

            let _ = fs::remove_dir_all(work_dir);

            json!({
                "exitCode": rc,
                "stdout": stdout_str,
                "stderr": stderr_str,
                "incusLog": log_str,
            })
        }
        other => panic!("unregistered golden function: {other}"),
    }
}

#[test]
fn replay() {
    let _env = common::GoldenEnvGuard::acquire();
    common::replay_jsonl("cli", dispatch);
}
