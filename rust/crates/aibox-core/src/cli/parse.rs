use crate::cli::commands::completions::ShellKind;
use crate::cli::{Command, ExitCode, ParsedArgs};
use crate::util::names::sanitize_name;

pub fn parse_command_name(token: &str) -> Command {
    match token {
        "help" => Command::Help,
        "list" => Command::List,
        "check" => Command::Check,
        "enter" => Command::Enter,
        "exec" => Command::Exec,
        "start" => Command::Start,
        "stop" => Command::Stop,
        "snapshot" => Command::Snapshot,
        "snapshots" => Command::Snapshots,
        "restore" => Command::Restore,
        "diff" => Command::Diff,
        "ssh-config" => Command::SshConfig,
        "svc" => Command::Svc,
        "theme" => Command::Theme,
        "completions" => Command::Completions,
        _ => Command::None,
    }
}

pub fn make_usage_error(message: impl Into<String>, target: Command) -> ParsedArgs {
    ParsedArgs {
        valid: false,
        exit_code: ExitCode::Usage.as_i32(),
        error_message: message.into(),
        command: target,
        ..ParsedArgs::default()
    }
}

const MAX_EXEC_TIMEOUT_SECS: u64 = 86_400;

/// Parses a duration such as `90`, `30s`, `5m`, `2h`, or `1h30m` into seconds (`1..=86400`).
pub fn parse_duration_secs(raw: &str) -> Result<u64, String> {
    let trimmed = raw.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return Err("Timeout duration cannot be empty.".to_string());
    }
    if let Ok(plain_secs) = trimmed.parse::<u64>() {
        if plain_secs == 0 || plain_secs > MAX_EXEC_TIMEOUT_SECS {
            return Err(format!(
                "Timeout must be between 1s and {MAX_EXEC_TIMEOUT_SECS}s."
            ));
        }
        return Ok(plain_secs);
    }

    let mut total: u64 = 0;
    let mut num_buf = String::new();
    for ch in trimmed.chars() {
        if ch.is_ascii_digit() {
            num_buf.push(ch);
            continue;
        }
        if num_buf.is_empty() {
            return Err(format!("Invalid timeout duration: '{raw}'."));
        }
        let val = num_buf
            .parse::<u64>()
            .map_err(|_| format!("Invalid timeout duration: '{raw}'."))?;
        num_buf.clear();
        let mult = match ch {
            's' => 1,
            'm' => 60,
            'h' => 3600,
            _ => return Err(format!("Invalid timeout unit '{ch}' in '{raw}'.")),
        };
        total = total.saturating_add(val.saturating_mul(mult));
    }
    if !num_buf.is_empty() || total == 0 || total > MAX_EXEC_TIMEOUT_SECS {
        return Err(format!(
            "Timeout must be between 1s and {MAX_EXEC_TIMEOUT_SECS}s (e.g. 30s, 15m, 1h30m)."
        ));
    }
    Ok(total)
}

pub fn argv_without_program(argv: &[String]) -> Vec<String> {
    if argv.len() <= 1 {
        Vec::new()
    } else {
        argv[1..].to_vec()
    }
}

pub fn parse_args(args: &[String]) -> ParsedArgs {
    let mut parsed = ParsedArgs::default();
    let mut positionals: Vec<String> = Vec::new();
    let mut parsing_exec_tail = false;
    let mut after_double_dash = false;
    let mut saw_double_dash = false;
    let mut saw_help = false;
    let mut saw_json = false;
    let mut saw_fix = false;
    let mut saw_system_only = false;
    let mut saw_workspace_only = false;
    let mut saw_yes = false;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        if parsing_exec_tail {
            parsed.exec_command.push(arg.clone());
            i += 1;
            continue;
        }

        if arg == "--" && !after_double_dash {
            saw_double_dash = true;
            if parsed.command == Command::Exec {
                parsing_exec_tail = true;
            } else {
                after_double_dash = true;
            }
            i += 1;
            continue;
        }

        if !after_double_dash {
            if arg == "--project" {
                if i + 1 >= args.len() || args[i + 1].trim().is_empty() {
                    return make_usage_error("--project requires a directory path.", Command::None);
                }
                i += 1;
                parsed.project_dir = args[i].clone();
                i += 1;
                continue;
            }
            if let Some(val) = arg.strip_prefix("--project=") {
                if val.trim().is_empty() {
                    return make_usage_error("--project requires a directory path.", Command::None);
                }
                parsed.project_dir = val.to_string();
                i += 1;
                continue;
            }
            if arg == "--output" {
                if i + 1 >= args.len() || args[i + 1].trim().is_empty() {
                    return make_usage_error("--output requires a file path.", Command::Theme);
                }
                i += 1;
                parsed.output_path = args[i].clone();
                i += 1;
                continue;
            }
            if let Some(val) = arg.strip_prefix("--output=") {
                if val.trim().is_empty() {
                    return make_usage_error("--output requires a file path.", Command::Theme);
                }
                parsed.output_path = val.to_string();
                i += 1;
                continue;
            }
            if arg == "--theme-dir" {
                if i + 1 >= args.len() || args[i + 1].trim().is_empty() {
                    return make_usage_error(
                        "--theme-dir requires a directory path.",
                        Command::Theme,
                    );
                }
                i += 1;
                parsed.theme_path = args[i].clone();
                i += 1;
                continue;
            }
            if let Some(val) = arg.strip_prefix("--theme-dir=") {
                if val.trim().is_empty() {
                    return make_usage_error(
                        "--theme-dir requires a directory path.",
                        Command::Theme,
                    );
                }
                parsed.theme_path = val.to_string();
                i += 1;
                continue;
            }
            if arg == "--timeout" {
                if i + 1 >= args.len() || args[i + 1].trim().is_empty() {
                    return make_usage_error(
                        "--timeout requires a duration (e.g. 30m).",
                        Command::Exec,
                    );
                }
                i += 1;
                match parse_duration_secs(&args[i]) {
                    Ok(secs) => parsed.timeout_secs = Some(secs),
                    Err(msg) => return make_usage_error(msg, Command::Exec),
                }
                i += 1;
                continue;
            }
            if let Some(val) = arg.strip_prefix("--timeout=") {
                match parse_duration_secs(val) {
                    Ok(secs) => parsed.timeout_secs = Some(secs),
                    Err(msg) => return make_usage_error(msg, Command::Exec),
                }
                i += 1;
                continue;
            }
            if arg == "--lines" {
                if i + 1 >= args.len() {
                    return make_usage_error("--lines requires a positive integer.", Command::Svc);
                }
                i += 1;
                match args[i].trim().parse::<usize>() {
                    Ok(n) if n > 0 => parsed.log_lines = n,
                    _ => {
                        return make_usage_error(
                            "--lines requires a positive integer.",
                            Command::Svc,
                        );
                    }
                }
                i += 1;
                continue;
            }
            if let Some(val) = arg.strip_prefix("--lines=") {
                match val.trim().parse::<usize>() {
                    Ok(n) if n > 0 => parsed.log_lines = n,
                    _ => {
                        return make_usage_error(
                            "--lines requires a positive integer.",
                            Command::Svc,
                        );
                    }
                }
                i += 1;
                continue;
            }
            if arg == "-h" || arg == "--help" {
                saw_help = true;
                i += 1;
                continue;
            }
            if arg == "--json" {
                saw_json = true;
                i += 1;
                continue;
            }
            if arg == "--fix" {
                saw_fix = true;
                i += 1;
                continue;
            }
            if arg == "--system-only" {
                saw_system_only = true;
                i += 1;
                continue;
            }
            if arg == "--workspace-only" {
                saw_workspace_only = true;
                i += 1;
                continue;
            }
            if arg == "--yes" || arg == "-y" {
                saw_yes = true;
                i += 1;
                continue;
            }
            if arg.starts_with('-') {
                return make_usage_error(format!("Unknown option: {arg}"), parsed.command);
            }
        }

        if parsed.command == Command::None {
            let cmd = parse_command_name(arg);
            if cmd == Command::None {
                return make_usage_error(format!("Unknown command: {arg}"), Command::None);
            }
            parsed.command = cmd;
        } else if parsed.command == Command::Help {
            if parsed.help_target != Command::None {
                return make_usage_error(format!("Unexpected argument: {arg}"), Command::Help);
            }
            let target = parse_command_name(arg);
            if target == Command::None {
                return make_usage_error(format!("Unknown help topic: {arg}"), Command::Help);
            }
            parsed.help_target = target;
        } else {
            positionals.push(arg.clone());
        }

        i += 1;
    }

    if saw_help {
        parsed.help_target = if parsed.command == Command::Help {
            parsed.help_target
        } else {
            parsed.command
        };
        parsed.command = Command::Help;
        return parsed;
    }

    if parsed.command == Command::Help {
        if saw_json || saw_fix || saw_system_only || saw_workspace_only || saw_yes {
            return make_usage_error("Invalid option for help.", Command::Help);
        }
        return parsed;
    }

    if parsed.command == Command::None {
        if saw_json
            || saw_fix
            || saw_system_only
            || saw_workspace_only
            || saw_yes
            || saw_double_dash
        {
            return make_usage_error("Missing subcommand.", Command::None);
        }
        return parsed;
    }

    if saw_json
        && !matches!(
            parsed.command,
            Command::List
                | Command::Check
                | Command::Snapshots
                | Command::Diff
                | Command::Svc
                | Command::Theme
        )
    {
        return make_usage_error("--json is not supported for this command.", parsed.command);
    }
    if saw_fix && parsed.command != Command::Check {
        return make_usage_error("--fix is only supported for 'check'.", parsed.command);
    }
    if (saw_system_only || saw_workspace_only || saw_yes) && parsed.command != Command::Restore {
        return make_usage_error(
            "Restore flags require the 'restore' command.",
            parsed.command,
        );
    }
    if saw_system_only && saw_workspace_only {
        return make_usage_error(
            "Cannot combine --system-only and --workspace-only.",
            Command::Restore,
        );
    }

    if (!parsed.output_path.is_empty() || !parsed.theme_path.is_empty())
        && parsed.command != Command::Theme
    {
        return make_usage_error(
            "--output and --theme-dir are only supported for 'theme'.",
            parsed.command,
        );
    }
    if parsed.timeout_secs.is_some() && parsed.command != Command::Exec {
        return make_usage_error("--timeout is only supported for 'exec'.", parsed.command);
    }
    if args
        .iter()
        .any(|a| a == "--lines" || a.starts_with("--lines="))
        && parsed.command != Command::Svc
    {
        return make_usage_error("--lines is only supported for 'svc'.", parsed.command);
    }

    parsed.json_output = saw_json;
    parsed.fix = saw_fix;
    parsed.system_only = saw_system_only;
    parsed.workspace_only = saw_workspace_only;
    parsed.yes = saw_yes;

    match parsed.command {
        Command::List | Command::Check => {
            if let Some(first) = positionals.first() {
                return make_usage_error(format!("Unexpected argument: {first}"), parsed.command);
            }
        }
        Command::Enter | Command::Start | Command::Stop | Command::Snapshots => {
            if positionals.len() != 1 {
                return make_usage_error("Expected exactly <env>.", parsed.command);
            }
            parsed.env_name = positionals[0].clone();
        }
        Command::Exec => {
            if positionals.is_empty() {
                return make_usage_error("exec requires <env> and '-- <cmd...>'.", Command::Exec);
            }
            if positionals.len() > 1 || !saw_double_dash {
                return make_usage_error(
                    "exec requires '--' before the command: aibox exec <env> -- <cmd...>",
                    Command::Exec,
                );
            }
            if parsed.exec_command.is_empty() {
                return make_usage_error("Missing command after '--'.", Command::Exec);
            }
            parsed.env_name = positionals[0].clone();
        }
        Command::Snapshot => {
            if positionals.is_empty() || positionals.len() > 2 {
                return make_usage_error("Expected <env> [name].", Command::Snapshot);
            }
            parsed.env_name = positionals[0].clone();
            if positionals.len() == 2 {
                parsed.snapshot_name = sanitize_name(&positionals[1]);
                if parsed.snapshot_name.is_empty() {
                    return make_usage_error(
                        format!("Invalid snapshot name: {}", positionals[1]),
                        Command::Snapshot,
                    );
                }
            }
        }
        Command::Restore => {
            if positionals.len() != 2 {
                return make_usage_error("Expected <env> <snap>.", Command::Restore);
            }
            parsed.env_name = positionals[0].clone();
            parsed.snapshot_name = positionals[1].clone();
        }
        Command::Diff => {
            if positionals.is_empty() || positionals.len() > 2 {
                return make_usage_error("Expected <env> [snap].", Command::Diff);
            }
            parsed.env_name = positionals[0].clone();
            if positionals.len() == 2 {
                parsed.snapshot_name = positionals[1].clone();
            }
        }
        Command::SshConfig => {
            if positionals.len() > 1 {
                return make_usage_error("Expected [env].", Command::SshConfig);
            }
            if let Some(env) = positionals.first() {
                parsed.env_name = env.clone();
            }
        }
        Command::Svc => {
            if positionals.len() < 2 || positionals.len() > 3 {
                return make_usage_error(
                    "Expected <env> <list|logs|start|stop|restart|enable|disable> [unit].",
                    Command::Svc,
                );
            }
            parsed.env_name = positionals[0].clone();
            let action = positionals[1].trim().to_ascii_lowercase();
            if !matches!(
                action.as_str(),
                "list" | "logs" | "start" | "stop" | "restart" | "enable" | "disable"
            ) {
                return make_usage_error(
                    format!("Unknown svc action: {}", positionals[1]),
                    Command::Svc,
                );
            }
            if action == "list" {
                if positionals.len() != 2 {
                    return make_usage_error("'svc <env> list' takes no unit.", Command::Svc);
                }
            } else {
                if positionals.len() != 3 {
                    return make_usage_error(
                        format!("'svc <env> {action}' requires <unit>."),
                        Command::Svc,
                    );
                }
                if saw_json {
                    return make_usage_error(
                        "--json is only supported for 'svc <env> list'.",
                        Command::Svc,
                    );
                }
                parsed.svc_unit = positionals[2].clone();
            }
            parsed.svc_action = action;
        }
        Command::Theme => match positionals.first().map(String::as_str) {
            Some("dump") => {
                if positionals.len() > 2 || !parsed.output_path.is_empty() {
                    return make_usage_error("Expected 'dump [path] [--json]'.", Command::Theme);
                }
                parsed.theme_subcommand = "dump".to_string();
                if positionals.len() == 2 {
                    parsed.theme_path = positionals[1].clone();
                }
            }
            Some("render-icon") => {
                if positionals.len() != 1 || saw_json {
                    return make_usage_error(
                        "Expected 'render-icon [--output PATH] [--theme-dir DIR]'.",
                        Command::Theme,
                    );
                }
                parsed.theme_subcommand = "render-icon".to_string();
            }
            _ => {
                return make_usage_error(
                    "Expected 'dump [path]' or 'render-icon [--output PATH] [--theme-dir DIR]'.",
                    Command::Theme,
                );
            }
        },
        Command::Completions => {
            if positionals.len() != 1 || ShellKind::parse(&positionals[0]).is_none() {
                return make_usage_error("Expected <bash|zsh|fish>.", Command::Completions);
            }
            parsed.shell = positionals[0].clone();
        }
        Command::None | Command::Help => {}
    }

    parsed
}

pub fn is_cli_invocation(args_without_program: &[String]) -> bool {
    if args_without_program.is_empty() {
        return false;
    }
    let parsed = parse_args(args_without_program);
    if !parsed.valid {
        return true;
    }
    parsed.command != Command::None
}
