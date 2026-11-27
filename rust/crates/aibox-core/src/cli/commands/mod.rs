pub mod completions;
pub mod env;
pub mod snapshot;
pub mod ssh_config;
pub mod svc;

use std::io;
use std::sync::OnceLock;

use chrono::Local;
use regex::Regex;
use serde::Serialize;
use serde_json::json;
use serde_json::ser::{Formatter, Serializer};

use crate::cli::{help_text, CliIo, Command, ExitCode, ParsedArgs};
use crate::engine::{Engine, ResolvedPaths};
use crate::health::checks::run_health_checks;
use crate::history::{record_operation_history, RecordOperationParams};
use crate::incus::list::Environment;
use crate::process::CommandRunner;
use crate::util::paths::workspace_path_for;

struct QtPrettyFormatter {
    current_indent: usize,
}

impl QtPrettyFormatter {
    fn new() -> Self {
        Self { current_indent: 0 }
    }

    fn write_indent<W: ?Sized + io::Write>(&self, writer: &mut W) -> io::Result<()> {
        for _ in 0..self.current_indent {
            writer.write_all(b"    ")?;
        }
        Ok(())
    }
}

impl Formatter for QtPrettyFormatter {
    fn begin_array<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.current_indent += 1;
        writer.write_all(b"[")
    }

    fn end_array<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.current_indent -= 1;
        writer.write_all(b"\n")?;
        self.write_indent(writer)?;
        writer.write_all(b"]")
    }

    fn begin_array_value<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if !first {
            writer.write_all(b",")?;
        }
        writer.write_all(b"\n")?;
        self.write_indent(writer)
    }

    fn begin_object<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.current_indent += 1;
        writer.write_all(b"{")
    }

    fn end_object<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.current_indent -= 1;
        writer.write_all(b"\n")?;
        self.write_indent(writer)?;
        writer.write_all(b"}")
    }

    fn begin_object_key<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if !first {
            writer.write_all(b",")?;
        }
        writer.write_all(b"\n")?;
        self.write_indent(writer)
    }

    fn begin_object_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        writer.write_all(b": ")
    }
}

pub fn to_json_qt_indented<T: Serialize + ?Sized>(value: &T) -> Result<String, serde_json::Error> {
    let mut buf = Vec::new();
    let mut ser = Serializer::with_formatter(&mut buf, QtPrettyFormatter::new());
    value.serialize(&mut ser)?;
    buf.push(b'\n');
    Ok(String::from_utf8(buf).expect("serde_json emits valid UTF-8"))
}

pub(crate) fn workspace_path_for_env(engine: &Engine, env: &Environment) -> String {
    if !env.workspace_path.is_empty() {
        env.workspace_path.clone()
    } else {
        workspace_path_for(&engine.paths.workspace_base, &[], &env.name)
    }
}

pub(crate) fn require_environment(
    engine: &mut Engine,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
    name: &str,
    reject_template: bool,
) -> Result<Environment, i32> {
    if let Err(err) = engine.refresh_sync(runner) {
        io.write_stderr(format!("Error: {err}\n").as_bytes());
        return Err(ExitCode::Failed.as_i32());
    }
    let Some(env) = engine.environments.iter().find(|e| e.name == name).cloned() else {
        io.write_stderr(format!("Error: Environment '{name}' was not found.\n").as_bytes());
        return Err(ExitCode::NotFound.as_i32());
    };
    if reject_template && env.is_template {
        io.write_stderr(format!("Error: Environment '{name}' is a template.\n").as_bytes());
        return Err(ExitCode::NotFound.as_i32());
    }
    Ok(env)
}

pub(crate) fn run_script_with_task_lock(
    engine: &Engine,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
    description: &str,
    env_name: &str,
    script: &str,
    stream_progress: bool,
) -> i32 {
    if !engine.acquire_cli_task_lock(description, env_name) {
        io.write_stderr(b"Error: Another AIBox background task is currently running.\n");
        return ExitCode::Busy.as_i32();
    }

    let started_at = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let out = runner.run_with_timeout("/bin/bash", &["-c", script], None);

    let mut combined = Vec::new();
    combined.extend_from_slice(out.std_out.as_bytes());
    if !out.std_err.is_empty() {
        if !combined.is_empty() && !combined.ends_with(b"\n") {
            combined.push(b'\n');
        }
        combined.extend_from_slice(out.std_err.as_bytes());
    }

    if stream_progress {
        static STAGE_RE: OnceLock<Regex> = OnceLock::new();
        let stage_re = STAGE_RE
            .get_or_init(|| Regex::new(r"^@@STAGE:(\d+):(.*)$").expect("valid stage regex"));
        let text = String::from_utf8_lossy(&combined);
        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(caps) = stage_re.captures(line) {
                let pct = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let msg = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
                io.write_stdout(format!("[{pct}%] {msg}\n").as_bytes());
            } else {
                io.write_stdout(format!("{line}\n").as_bytes());
            }
        }
    }

    let status = if out.exit_code == 0 {
        "success"
    } else {
        "failed"
    };
    let secret_vals = engine.secrets.all_secret_values();
    let _ = record_operation_history(&RecordOperationParams {
        state_dir: &engine.paths.state_dir,
        description,
        env_name,
        status,
        exit_code: out.exit_code,
        started_at_iso: &started_at,
        log_bytes: &combined,
        secret_values: &secret_vals,
        reason: "",
        max_entries: 50,
    });
    engine.release_cli_task_lock();
    if out.exit_code == 0 {
        ExitCode::Ok.as_i32()
    } else {
        ExitCode::Failed.as_i32()
    }
}

fn run_check(
    parsed: &ParsedArgs,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
    cwd: &str,
) -> i32 {
    let ws_base = if !parsed.project_dir.is_empty() {
        let paths = ResolvedPaths::resolve(Some(parsed.project_dir.as_str()), true, cwd);
        let cfg = crate::config::model::load(
            &paths.config_file_path,
            &paths.workspace_base,
            &paths.os_storage_path,
        );
        cfg.workspace_base
    } else {
        let paths = ResolvedPaths::resolve(None, false, cwd);
        if std::path::Path::new(&paths.config_file_path).exists() {
            let cfg = crate::config::model::load(
                &paths.config_file_path,
                &paths.workspace_base,
                &paths.os_storage_path,
            );
            cfg.workspace_base
        } else {
            String::new()
        }
    };
    let checks = run_health_checks(parsed.fix, runner, "", "", &ws_base);
    let mut all_ok = true;
    let mut warning_count = 0i32;
    let mut arr = Vec::with_capacity(checks.len());

    for c in &checks {
        if !c.ok {
            all_ok = false;
        }
        if c.warning {
            warning_count += 1;
        }
        arr.push(json!({
            "id": c.id,
            "title": c.title,
            "ok": c.ok,
            "warning": c.warning,
            "repaired": c.repaired,
            "needsRoot": c.needs_root,
            "detail": c.detail,
            "fixCommand": c.fix_command,
        }));
    }

    if parsed.json_output {
        let root = json!({
            "allOk": all_ok,
            "warningCount": warning_count,
            "checks": arr,
        });
        let json_text = to_json_qt_indented(&root).unwrap_or_else(|_| "{\n}\n".to_string());
        io.write_stdout(json_text.as_bytes());
    } else {
        for c in &checks {
            let tag = if !c.ok {
                "[FAIL] "
            } else if c.warning {
                "[WARN] "
            } else if c.repaired {
                "[FIXED]"
            } else {
                "[OK]   "
            };
            io.write_stdout(format!("{tag} {}: {}\n", c.title, c.detail).as_bytes());
            if !c.ok && !c.fix_command.is_empty() {
                let prefix = if c.needs_root {
                    "Run with sudo:"
                } else {
                    "Fix:"
                };
                io.write_stdout(format!("         {prefix} {}\n", c.fix_command).as_bytes());
            }
        }
    }
    if all_ok {
        ExitCode::Ok.as_i32()
    } else {
        ExitCode::Failed.as_i32()
    }
}

fn resolve_theme_dir(override_path: &str) -> std::path::PathBuf {
    if !override_path.is_empty() {
        return std::path::PathBuf::from(override_path);
    }
    if let Ok(env_dir) = std::env::var("AIBOX_THEME_DIR") {
        if !env_dir.trim().is_empty() {
            return std::path::PathBuf::from(env_dir);
        }
    }
    let state_dir = crate::util::paths::home_dir().join(".local/state/omarchy/current");
    if state_dir.exists() {
        state_dir
    } else {
        crate::util::paths::home_dir().join(".config/omarchy/current")
    }
}

fn run_theme(parsed: &ParsedArgs, io: &mut dyn CliIo) -> i32 {
    if parsed.theme_subcommand == "render-icon" {
        return run_theme_render_icon(parsed, io);
    }
    run_theme_dump(parsed, io)
}

fn run_theme_dump(parsed: &ParsedArgs, io: &mut dyn CliIo) -> i32 {
    let theme_dir = resolve_theme_dir(&parsed.theme_path);
    let palette = crate::theme::load_palette(&theme_dir);
    if parsed.json_output {
        let out = serde_json::to_string_pretty(&palette).unwrap_or_else(|_| "{}".to_string());
        io.write_stdout(format!("{out}\n").as_bytes());
    } else {
        io.write_stdout(palette.format_dump().as_bytes());
    }
    ExitCode::Ok.as_i32()
}

fn run_theme_render_icon(parsed: &ParsedArgs, io: &mut dyn CliIo) -> i32 {
    let theme_dir = resolve_theme_dir(&parsed.theme_path);
    let palette = crate::theme::load_palette(&theme_dir);
    let output_path = if parsed.output_path.is_empty() {
        crate::theme::icon::default_icon_output_path()
    } else {
        std::path::PathBuf::from(&parsed.output_path)
    };

    if let Err(err) = crate::theme::icon::write_icon(&palette, &output_path) {
        io.write_stderr(format!("Error: {err}\n").as_bytes());
        return ExitCode::Failed.as_i32();
    }
    ExitCode::Ok.as_i32()
}

pub fn execute_with(
    parsed: &ParsedArgs,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
    cwd: &str,
) -> i32 {
    if !parsed.valid {
        io.write_stderr(format!("Error: {}\n\n", parsed.error_message).as_bytes());
        io.write_stderr(help_text(parsed.command).as_bytes());
        return parsed.exit_code;
    }

    match parsed.command {
        Command::Help => {
            io.write_stdout(help_text(parsed.help_target).as_bytes());
            return ExitCode::Ok.as_i32();
        }
        Command::Check => return run_check(parsed, io, runner, cwd),
        Command::Theme => return run_theme(parsed, io),
        Command::Completions => return completions::run_completions(parsed, io),
        _ => {}
    }

    let path_override = if parsed.project_dir.is_empty() {
        None
    } else {
        Some(parsed.project_dir.as_str())
    };
    let mut engine = Engine::new(path_override, true, cwd);

    match parsed.command {
        Command::List => env::run_list(&mut engine, io, runner),
        Command::Enter => env::run_enter(&mut engine, parsed, io, runner),
        Command::Exec => env::run_exec(&mut engine, parsed, io, runner),
        Command::Start => env::run_start(&mut engine, parsed, io, runner),
        Command::Stop => env::run_stop(&mut engine, parsed, io, runner),
        Command::Snapshot => snapshot::run_snapshot(&mut engine, parsed, io, runner),
        Command::Snapshots => snapshot::run_snapshots(&mut engine, parsed, io, runner),
        Command::Restore => snapshot::run_restore(&mut engine, parsed, io, runner),
        Command::Diff => snapshot::run_diff(&mut engine, parsed, io, runner),
        Command::SshConfig => ssh_config::run_ssh_config(&mut engine, parsed, io, runner),
        Command::Svc => svc::run_svc(&mut engine, parsed, io, runner),
        Command::Check | Command::Help | Command::None | Command::Theme | Command::Completions => {
            ExitCode::Usage.as_i32()
        }
    }
}
