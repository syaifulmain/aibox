use crate::cli::{Command, ParsedArgs};
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
        "theme" => Command::Theme,
        _ => Command::None,
    }
}

pub fn make_usage_error(message: impl Into<String>, target: Command) -> ParsedArgs {
    ParsedArgs {
        valid: false,
        exit_code: 2,
        error_message: message.into(),
        command: target,
        ..ParsedArgs::default()
    }
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
            Command::List | Command::Check | Command::Snapshots | Command::Diff | Command::Theme
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
