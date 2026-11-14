#pragma once

#include <QString>
#include <QStringList>

namespace Cli {

enum class Command {
    None,
    Help,
    List,
    Check,
    Enter,
    Exec,
    Start,
    Stop,
    Snapshot,
    Snapshots,
    Restore,
    Diff
};

struct ParsedArgs {
    bool valid = true;
    int exitCode = 0;
    QString errorMessage;

    Command command = Command::None;
    Command helpTarget = Command::None;
    QString projectDir;
    QString envName;
    QString snapshotName;
    QStringList execCommand;
    bool jsonOutput = false;
    bool fix = false;
    bool systemOnly = false;
    bool workspaceOnly = false;
    bool yes = false;
};

// Returns true if argv represents a CLI invocation rather than launching the GUI.
bool isCliInvocation(int argc, char *argv[]);

// Parses CLI arguments (excluding argv[0]).
ParsedArgs parseArgs(const QStringList &args);

// Returns CLI usage text for general help or a specific subcommand.
QString helpText(Command cmd = Command::None);

// Dispatches a CLI invocation and returns the process exit code.
int run(int argc, char *argv[]);
int execute(const ParsedArgs &parsed);

} // namespace Cli
