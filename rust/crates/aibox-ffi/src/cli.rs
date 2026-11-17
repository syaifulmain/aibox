use aibox_core::cli::{
    execute, help_text, is_cli_invocation, parse_args, run, Command, ParsedArgs,
};

#[cxx::bridge(namespace = "aibox::ffi")]
mod ffi {
    struct CliParsedArgsFfi {
        valid: bool,
        exit_code: i32,
        error_message: String,
        command: i32,
        help_target: i32,
        project_dir: String,
        env_name: String,
        snapshot_name: String,
        exec_command: Vec<String>,
        json_output: bool,
        fix: bool,
        system_only: bool,
        workspace_only: bool,
        yes: bool,
    }

    extern "Rust" {
        fn cli_is_invocation(args_without_program: &[String]) -> bool;
        fn cli_parse_args(args_without_program: &[String]) -> CliParsedArgsFfi;
        fn cli_help_text(cmd: i32) -> String;
        fn cli_execute(parsed: &CliParsedArgsFfi) -> i32;
        fn cli_run(args_without_program: &[String]) -> i32;
    }
}

fn cmd_to_i32(cmd: Command) -> i32 {
    match cmd {
        Command::None => 0,
        Command::Help => 1,
        Command::List => 2,
        Command::Check => 3,
        Command::Enter => 4,
        Command::Exec => 5,
        Command::Start => 6,
        Command::Stop => 7,
        Command::Snapshot => 8,
        Command::Snapshots => 9,
        Command::Restore => 10,
        Command::Diff => 11,
    }
}

fn i32_to_cmd(val: i32) -> Command {
    match val {
        1 => Command::Help,
        2 => Command::List,
        3 => Command::Check,
        4 => Command::Enter,
        5 => Command::Exec,
        6 => Command::Start,
        7 => Command::Stop,
        8 => Command::Snapshot,
        9 => Command::Snapshots,
        10 => Command::Restore,
        11 => Command::Diff,
        _ => Command::None,
    }
}

fn parsed_to_ffi(p: ParsedArgs) -> ffi::CliParsedArgsFfi {
    ffi::CliParsedArgsFfi {
        valid: p.valid,
        exit_code: p.exit_code,
        error_message: p.error_message,
        command: cmd_to_i32(p.command),
        help_target: cmd_to_i32(p.help_target),
        project_dir: p.project_dir,
        env_name: p.env_name,
        snapshot_name: p.snapshot_name,
        exec_command: p.exec_command,
        json_output: p.json_output,
        fix: p.fix,
        system_only: p.system_only,
        workspace_only: p.workspace_only,
        yes: p.yes,
    }
}

fn ffi_to_parsed(p: &ffi::CliParsedArgsFfi) -> ParsedArgs {
    ParsedArgs {
        valid: p.valid,
        exit_code: p.exit_code,
        error_message: p.error_message.clone(),
        command: i32_to_cmd(p.command),
        help_target: i32_to_cmd(p.help_target),
        project_dir: p.project_dir.clone(),
        env_name: p.env_name.clone(),
        snapshot_name: p.snapshot_name.clone(),
        exec_command: p.exec_command.clone(),
        json_output: p.json_output,
        fix: p.fix,
        system_only: p.system_only,
        workspace_only: p.workspace_only,
        yes: p.yes,
    }
}

fn cli_is_invocation(args_without_program: &[String]) -> bool {
    is_cli_invocation(args_without_program)
}

fn cli_parse_args(args_without_program: &[String]) -> ffi::CliParsedArgsFfi {
    parsed_to_ffi(parse_args(args_without_program))
}

fn cli_help_text(cmd: i32) -> String {
    help_text(i32_to_cmd(cmd)).to_string()
}

fn cli_execute(parsed: &ffi::CliParsedArgsFfi) -> i32 {
    execute(&ffi_to_parsed(parsed))
}

fn cli_run(args_without_program: &[String]) -> i32 {
    run(args_without_program)
}
