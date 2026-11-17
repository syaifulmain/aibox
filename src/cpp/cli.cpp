#include "cli.h"

#include "aibox-ffi/src/cli.rs.h"
#include "rust_bridge.h"

namespace Cli {

namespace {

std::vector<rust::String> argvWithoutProgramRust(int argc, char *argv[]) {
    std::vector<rust::String> args;
    if (argc > 1) {
        args.reserve(static_cast<size_t>(argc - 1));
        for (int i = 1; i < argc; ++i)
            args.push_back(aibox::ffi::toRust(QString::fromLocal8Bit(argv[i])));
    }
    return args;
}

Command i32ToCommand(std::int32_t v) {
    switch (v) {
    case 1: return Command::Help;
    case 2: return Command::List;
    case 3: return Command::Check;
    case 4: return Command::Enter;
    case 5: return Command::Exec;
    case 6: return Command::Start;
    case 7: return Command::Stop;
    case 8: return Command::Snapshot;
    case 9: return Command::Snapshots;
    case 10: return Command::Restore;
    case 11: return Command::Diff;
    default: return Command::None;
    }
}

std::int32_t commandToI32(Command c) {
    switch (c) {
    case Command::None: return 0;
    case Command::Help: return 1;
    case Command::List: return 2;
    case Command::Check: return 3;
    case Command::Enter: return 4;
    case Command::Exec: return 5;
    case Command::Start: return 6;
    case Command::Stop: return 7;
    case Command::Snapshot: return 8;
    case Command::Snapshots: return 9;
    case Command::Restore: return 10;
    case Command::Diff: return 11;
    }
    return 0;
}

ParsedArgs fromFfi(const aibox::ffi::CliParsedArgsFfi &f) {
    ParsedArgs p;
    p.valid = f.valid;
    p.exitCode = f.exit_code;
    p.errorMessage = aibox::ffi::fromRust(f.error_message);
    p.command = i32ToCommand(f.command);
    p.helpTarget = i32ToCommand(f.help_target);
    p.projectDir = aibox::ffi::fromRust(f.project_dir);
    p.envName = aibox::ffi::fromRust(f.env_name);
    p.snapshotName = aibox::ffi::fromRust(f.snapshot_name);
    p.execCommand = aibox::ffi::fromRustVec(f.exec_command);
    p.jsonOutput = f.json_output;
    p.fix = f.fix;
    p.systemOnly = f.system_only;
    p.workspaceOnly = f.workspace_only;
    p.yes = f.yes;
    return p;
}

aibox::ffi::CliParsedArgsFfi toFfi(const ParsedArgs &p) {
    aibox::ffi::CliParsedArgsFfi f;
    f.valid = p.valid;
    f.exit_code = p.exitCode;
    f.error_message = aibox::ffi::toRust(p.errorMessage);
    f.command = commandToI32(p.command);
    f.help_target = commandToI32(p.helpTarget);
    f.project_dir = aibox::ffi::toRust(p.projectDir);
    f.env_name = aibox::ffi::toRust(p.envName);
    f.snapshot_name = aibox::ffi::toRust(p.snapshotName);
    f.exec_command = aibox::ffi::toRustVec(p.execCommand);
    f.json_output = p.jsonOutput;
    f.fix = p.fix;
    f.system_only = p.systemOnly;
    f.workspace_only = p.workspaceOnly;
    f.yes = p.yes;
    return f;
}

} // namespace

ParsedArgs parseArgs(const QStringList &args) {
    const auto r = aibox::ffi::toRustVec(args);
    return fromFfi(aibox::ffi::cli_parse_args(
        rust::Slice<const rust::String>(r.data(), r.size())));
}

bool isCliInvocation(int argc, char *argv[]) {
    const auto r = argvWithoutProgramRust(argc, argv);
    return aibox::ffi::cli_is_invocation(
        rust::Slice<const rust::String>(r.data(), r.size()));
}

QString helpText(Command cmd) {
    return aibox::ffi::fromRust(aibox::ffi::cli_help_text(commandToI32(cmd)));
}

int execute(const ParsedArgs &parsed) {
    return aibox::ffi::cli_execute(toFfi(parsed));
}

int run(int argc, char *argv[]) {
    const auto r = argvWithoutProgramRust(argc, argv);
    return aibox::ffi::cli_run(rust::Slice<const rust::String>(r.data(), r.size()));
}

} // namespace Cli
