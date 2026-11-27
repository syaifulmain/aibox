use std::fs;
use std::path::Path;

use crate::cli::commands::{require_environment, run_script_with_task_lock};
use crate::cli::{CliIo, ExitCode, ParsedArgs};
use crate::config::manifest::{
    manifest_from_environment, parse_manifest_toml, reconcile_manifest_script, render_manifest_toml,
};
use crate::engine::Engine;
use crate::process::CommandRunner;
use crate::provision::create::create_environment_script;
use crate::util::paths::{cache_base_path, workspace_path_for};

pub fn run_export_config(
    engine: &mut Engine,
    parsed: &ParsedArgs,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
) -> i32 {
    let env = match require_environment(engine, io, runner, &parsed.env_name, false) {
        Ok(e) => e,
        Err(rc) => return rc,
    };
    let manifest = manifest_from_environment(&env);
    let toml = render_manifest_toml(&manifest);

    if parsed.output_path.is_empty() {
        io.write_stdout(toml.as_bytes());
        return ExitCode::Ok.as_i32();
    }

    if let Err(err) = fs::write(&parsed.output_path, toml.as_bytes()) {
        io.write_stderr(
            format!("Error: Failed to write '{}': {err}\n", parsed.output_path).as_bytes(),
        );
        return ExitCode::Failed.as_i32();
    }
    io.write_stdout(format!("Wrote {}\n", parsed.output_path).as_bytes());
    ExitCode::Ok.as_i32()
}

pub fn run_up(
    engine: &mut Engine,
    parsed: &ParsedArgs,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
    cwd: &str,
) -> i32 {
    let manifest_path = if parsed.manifest_path.is_empty() {
        Path::new(cwd).join("aibox.toml")
    } else {
        Path::new(&parsed.manifest_path).to_path_buf()
    };

    let raw = match fs::read_to_string(&manifest_path) {
        Ok(s) => s,
        Err(err) => {
            io.write_stderr(
                format!(
                    "Error: Failed to read '{}': {err}\n",
                    manifest_path.display()
                )
                .as_bytes(),
            );
            return ExitCode::Failed.as_i32();
        }
    };

    let manifest = match parse_manifest_toml(&raw) {
        Ok(m) => m,
        Err(err) => {
            io.write_stderr(format!("Error: {err}\n").as_bytes());
            return ExitCode::Usage.as_i32();
        }
    };

    if let Err(err) = engine.refresh_sync(runner) {
        io.write_stderr(format!("Error: {err}\n").as_bytes());
        return ExitCode::Failed.as_i32();
    }

    if let Some(existing) = engine
        .environments
        .iter()
        .find(|e| e.name == manifest.name)
        .cloned()
    {
        let existing_ports: Vec<i32> = existing.port_forwards.iter().map(|p| p.host_port).collect();
        let script = reconcile_manifest_script(&manifest, &existing_ports);
        return run_script_with_task_lock(
            engine,
            io,
            runner,
            &format!("Reconciling '{}' from manifest", manifest.name),
            &manifest.name,
            &script,
            true,
        );
    }

    let os_entry = engine
        .config
        .os_catalog
        .iter()
        .find(|o| o.id == manifest.os || o.name.eq_ignore_ascii_case(&manifest.os));
    let local_alias = os_entry
        .map(|o| format!("aibox-{}", o.id))
        .unwrap_or_else(|| format!("aibox-{}", manifest.os));
    let remote_image = os_entry
        .map(|o| format!("images:{}", o.alias))
        .unwrap_or_else(|| format!("images:{}", manifest.os));
    let ws = workspace_path_for(&engine.paths.workspace_base, &[], &manifest.name);
    let cache_base = cache_base_path(&engine.paths.os_storage_path, &manifest.os);
    let is_vm = manifest.preset.eq_ignore_ascii_case("vm");
    let profile_flags = if is_vm {
        "--vm"
    } else if manifest.preset.eq_ignore_ascii_case("gui") {
        "-p default -p ai-gui"
    } else {
        "-p default"
    };

    let create_script = create_environment_script(
        &manifest.name,
        &local_alias,
        &remote_image,
        &ws,
        &cache_base,
        profile_flags,
        &manifest.memory,
        &manifest.disk,
        manifest.cpu,
        manifest.desktop,
        "",
        "",
        &manifest.swap,
        is_vm,
    );
    let reconcile = reconcile_manifest_script(&manifest, &[]);
    let full_script = format!("{create_script}\n{reconcile}");

    run_script_with_task_lock(
        engine,
        io,
        runner,
        &format!("Provisioning '{}' from manifest", manifest.name),
        &manifest.name,
        &full_script,
        true,
    )
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

    struct ManifestRunner {
        list_json: String,
        bash_scripts: RefCell<Vec<String>>,
    }

    impl CommandRunner for ManifestRunner {
        fn run(&self, program: &str, args: &[&str]) -> CommandOutput {
            if program == "incus" && args.first() == Some(&"list") {
                return CommandOutput {
                    exit_code: 0,
                    std_out: self.list_json.clone(),
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
    fn export_config_and_up_round_trip() {
        let dir = std::env::temp_dir().join("aibox_manifest_cli_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("aibox/state")).expect("mkdir");
        let proj = dir.to_string_lossy().to_string();
        let toml_path = dir.join("aibox.toml");

        let mut engine = Engine::new(Some(&proj), true, "/tmp");
        let runner = ManifestRunner {
            list_json: r#"[{"name":"web","type":"container","status":"Running","config":{"boot.autostart":"true","limits.cpu":"2","limits.memory":"4GiB"}}]"#.to_string(),
            bash_scripts: RefCell::new(Vec::new()),
        };

        let mut io_exp = DummyIo::new();
        let parsed_exp = parse_args(&[
            "export-config".into(),
            "web".into(),
            "--output".into(),
            toml_path.to_string_lossy().to_string(),
        ]);
        assert!(parsed_exp.valid);
        assert_eq!(
            run_export_config(&mut engine, &parsed_exp, &mut io_exp, &runner),
            0
        );
        let written = fs::read_to_string(&toml_path).expect("read exported toml");
        assert!(written.contains("name = \"web\""));
        assert!(written.contains("autostart = true"));

        let mut io_up = DummyIo::new();
        let parsed_up = parse_args(&["up".into(), toml_path.to_string_lossy().to_string()]);
        assert!(parsed_up.valid);
        assert_eq!(
            run_up(&mut engine, &parsed_up, &mut io_up, &runner, &proj),
            0
        );
        assert!(runner.bash_scripts.borrow()[0].contains("boot.autostart=true"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
