#include "cli.h"
#include "backend.h"

#include <QDateTime>
#include <QDir>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QProcess>
#include <QRegularExpression>
#include <QStandardPaths>
#include <cerrno>
#include <cstdio>
#include <cstring>
#include <sys/wait.h>
#include <unistd.h>
#include <vector>

namespace Cli {

namespace {

Command parseCommandName(const QString &token) {
    if (token == QLatin1String("help")) return Command::Help;
    if (token == QLatin1String("list")) return Command::List;
    if (token == QLatin1String("check")) return Command::Check;
    if (token == QLatin1String("enter")) return Command::Enter;
    if (token == QLatin1String("exec")) return Command::Exec;
    if (token == QLatin1String("start")) return Command::Start;
    if (token == QLatin1String("stop")) return Command::Stop;
    if (token == QLatin1String("snapshot")) return Command::Snapshot;
    if (token == QLatin1String("snapshots")) return Command::Snapshots;
    if (token == QLatin1String("restore")) return Command::Restore;
    if (token == QLatin1String("diff")) return Command::Diff;
    return Command::None;
}

ParsedArgs makeUsageError(const QString &message, Command target = Command::None) {
    ParsedArgs err;
    err.valid = false;
    err.exitCode = 2;
    err.errorMessage = message;
    err.command = target;
    return err;
}

QStringList argvWithoutProgram(int argc, char *argv[]) {
    QStringList args;
    for (int i = 1; i < argc; ++i)
        args.append(QString::fromLocal8Bit(argv[i]));
    return args;
}

int runScriptWithTaskLock(Backend &backend, const QString &description, const QString &envName,
                          const QString &script, bool streamProgress = true) {
    if (!backend.acquireCliTaskLock(description, envName)) {
        std::fprintf(stderr, "Error: Another AIBox background task is currently running.\n");
        return 3;
    }

    const QString startedAt = QDateTime::currentDateTime().toString(Qt::ISODate);
    QProcess proc;
    proc.setProcessChannelMode(QProcess::MergedChannels);
    QByteArray fullLog;
    QString remainder;
    static const QRegularExpression stageRe(QStringLiteral(R"(^@@STAGE:(\d+):(.*)$)"));

    QObject::connect(&proc, &QProcess::readyReadStandardOutput, [&]() {
        const QByteArray chunk = proc.readAllStandardOutput();
        fullLog.append(chunk);
        if (!streamProgress)
            return;

        remainder += QString::fromUtf8(chunk);
        int idx = -1;
        while ((idx = remainder.indexOf(QLatin1Char('\n'))) >= 0) {
            const QString line = remainder.left(idx).trimmed();
            remainder.remove(0, idx + 1);
            if (line.isEmpty())
                continue;
            const auto m = stageRe.match(line);
            if (m.hasMatch()) {
                std::fprintf(stdout, "[%s%%] %s\n",
                             m.captured(1).toUtf8().constData(),
                             m.captured(2).trimmed().toUtf8().constData());
            } else {
                std::fprintf(stdout, "%s\n", line.toUtf8().constData());
            }
            std::fflush(stdout);
        }
    });

    proc.start(QStringLiteral("/bin/bash"), {QStringLiteral("-c"), script});
    if (!proc.waitForStarted(5000)) {
        backend.releaseCliTaskLock();
        std::fprintf(stderr, "Error: Failed to execute /bin/bash.\n");
        return 1;
    }
    proc.waitForFinished(-1);

    const QByteArray trailing = proc.readAllStandardOutput();
    if (!trailing.isEmpty()) {
        fullLog.append(trailing);
        remainder += QString::fromUtf8(trailing);
    }
    if (streamProgress && !remainder.trimmed().isEmpty()) {
        const QString line = remainder.trimmed();
        const auto m = stageRe.match(line);
        if (m.hasMatch()) {
            std::fprintf(stdout, "[%s%%] %s\n",
                         m.captured(1).toUtf8().constData(),
                         m.captured(2).trimmed().toUtf8().constData());
        } else {
            std::fprintf(stdout, "%s\n", line.toUtf8().constData());
        }
        std::fflush(stdout);
    }

    const int exitCode = (proc.exitStatus() == QProcess::NormalExit) ? proc.exitCode() : 1;
    backend.recordHistory(description, envName,
                          exitCode == 0 ? QStringLiteral("success") : QStringLiteral("failed"),
                          exitCode, startedAt, fullLog);
    backend.releaseCliTaskLock();
    return exitCode == 0 ? 0 : 1;
}

int runPreStepsForExec(Backend &backend, const QString &envName, const QString &label) {
    if (!backend.acquireCliTaskLock(label, envName)) {
        std::fprintf(stderr, "Error: Another AIBox background task is currently running.\n");
        return 3;
    }
    QProcess preProc;
    preProc.setProcessChannelMode(QProcess::MergedChannels);
    preProc.start(QStringLiteral("/bin/bash"),
                  {QStringLiteral("-c"), backend.prepareEnvironmentScript(envName)});
    if (!preProc.waitForStarted(5000)) {
        backend.releaseCliTaskLock();
        std::fprintf(stderr, "Error: Failed to start pre-step shell.\n");
        return 1;
    }
    preProc.waitForFinished(-1);
    const QByteArray out = preProc.readAllStandardOutput();
    if (out.contains("Warning:")) {
        backend.recordHistory(QObject::tr("Auto-snapshot warning for '%1'").arg(envName), envName,
                              QStringLiteral("warning"), preProc.exitCode(),
                              QDateTime::currentDateTimeUtc().toString(Qt::ISODate), out);
        std::fwrite(out.constData(), 1, out.size(), stderr);
    }
    backend.releaseCliTaskLock();

    if (preProc.exitStatus() != QProcess::NormalExit || preProc.exitCode() != 0) {
        if (!out.isEmpty() && !out.contains("Warning:"))
            std::fwrite(out.constData(), 1, out.size(), stderr);
        std::fprintf(stderr, "Error: Failed to prepare environment '%s'.\n",
                     envName.toUtf8().constData());
        return 1;
    }
    return 0;
}

int runList(Backend &backend) {
    QString err;
    if (!backend.refreshSync(&err)) {
        std::fprintf(stderr, "Error: %s\n", err.toUtf8().constData());
        return 1;
    }
    const auto doc = QJsonDocument::fromVariant(backend.environments());
    const QByteArray out = doc.toJson(QJsonDocument::Indented);
    std::fwrite(out.constData(), 1, out.size(), stdout);
    return 0;
}

int runCheck(const ParsedArgs &parsed) {
    const auto checks = Backend::runHealthChecks(parsed.fix);
    bool allOk = true;
    int warningCount = 0;
    QJsonArray arr;
    for (const auto &c : checks) {
        if (!c.ok)
            allOk = false;
        if (c.warning)
            ++warningCount;
        QJsonObject obj;
        obj["id"] = c.id;
        obj["title"] = c.title;
        obj["ok"] = c.ok;
        obj["warning"] = c.warning;
        obj["repaired"] = c.repaired;
        obj["needsRoot"] = c.needsRoot;
        obj["detail"] = c.detail;
        obj["fixCommand"] = c.fixCommand;
        arr.append(obj);
    }
    if (parsed.jsonOutput) {
        QJsonObject root;
        root["allOk"] = allOk;
        root["warningCount"] = warningCount;
        root["checks"] = arr;
        const QByteArray out = QJsonDocument(root).toJson(QJsonDocument::Indented);
        std::fwrite(out.constData(), 1, out.size(), stdout);
    } else {
        for (const auto &c : checks) {
            const char *tag = !c.ok ? "[FAIL] " : (c.warning ? "[WARN] " : (c.repaired ? "[FIXED]" : "[OK]   "));
            std::fprintf(stdout, "%s %s: %s\n", tag,
                         c.title.toUtf8().constData(),
                         c.detail.toUtf8().constData());
            if (!c.ok && !c.fixCommand.isEmpty()) {
                std::fprintf(stdout, "         %s %s\n",
                             c.needsRoot ? "Run with sudo:" : "Fix:",
                             c.fixCommand.toUtf8().constData());
            }
        }
    }
    return allOk ? 0 : 1;
}

int requireEnvironment(Backend &backend, const QString &name, bool rejectTemplate, QVariantMap *outEnv) {
    QString err;
    if (!backend.refreshSync(&err)) {
        std::fprintf(stderr, "Error: %s\n", err.toUtf8().constData());
        return 1;
    }
    const QVariantMap env = backend.findEnvironment(name);
    if (env.isEmpty()) {
        std::fprintf(stderr, "Error: Environment '%s' was not found.\n", name.toUtf8().constData());
        return 4;
    }
    if (rejectTemplate && env.value("isTemplate").toBool()) {
        std::fprintf(stderr, "Error: Environment '%s' is a template.\n", name.toUtf8().constData());
        return 4;
    }
    if (outEnv)
        *outEnv = env;
    return 0;
}

int runEnter(Backend &backend, const ParsedArgs &parsed) {
    QVariantMap env;
    const int checkRc = requireEnvironment(backend, parsed.envName, true, &env);
    if (checkRc != 0)
        return checkRc;

    const int preRc = runPreStepsForExec(backend, parsed.envName,
                                         QObject::tr("Entering '%1' (CLI)").arg(parsed.envName));
    if (preRc != 0)
        return preRc;

    QString remoteEnvPath;
    QString secretErr;
    if (!backend.pushEnvironmentSecretsToContainer(parsed.envName, &remoteEnvPath, &secretErr)) {
        std::fprintf(stderr, "Error: %s\n", secretErr.toUtf8().constData());
        return 1;
    }

    const QStringList execArgs = backend.enterIncusExecArgs(parsed.envName, remoteEnvPath);
    if (env.value(QStringLiteral("recordEnabled")).toBool() &&
        !QStandardPaths::findExecutable(QStringLiteral("script")).isEmpty()) {
        QString logFile;
        QString timingFile;
        if (backend.prepareRecordedSessionPaths(parsed.envName, &logFile, &timingFile)) {
            QString quotedCmd = QStringLiteral("incus");
            for (const QString &arg : execArgs) {
                QString q = arg;
                q.replace(QLatin1Char('\''), QStringLiteral("'\\''"));
                quotedCmd += QStringLiteral(" '%1'").arg(q);
            }
            const QStringList scriptArgs{
                QStringLiteral("-q"),
                QStringLiteral("-f"),
                QStringLiteral("--log-out"),
                logFile,
                QStringLiteral("--log-timing"),
                timingFile,
                QStringLiteral("-c"),
                quotedCmd
            };
            std::fflush(stdout);
            std::fflush(stderr);
            const pid_t child = ::fork();
            if (child == 0) {
                std::vector<QByteArray> rawArgs;
                rawArgs.push_back(QByteArrayLiteral("script"));
                for (const QString &a : scriptArgs)
                    rawArgs.push_back(a.toUtf8());
                std::vector<char *> cArgv;
                for (QByteArray &buf : rawArgs)
                    cArgv.push_back(buf.data());
                cArgv.push_back(nullptr);
                ::execvp("script", cArgv.data());
                _exit(127);
            }
            int exitCode = 1;
            if (child > 0) {
                int status = 0;
                if (::waitpid(child, &status, 0) > 0 && WIFEXITED(status))
                    exitCode = WEXITSTATUS(status);
            }
            Backend::redactSessionLogFile(logFile, backend.allSecretValues());
            QFile::setPermissions(logFile, QFileDevice::ReadOwner | QFileDevice::WriteOwner);
            Backend::pruneRecordedSessions(backend.sessionsDirPath());
            return exitCode;
        }
    }

    std::vector<QByteArray> rawArgs;
    rawArgs.reserve(execArgs.size() + 1);
    rawArgs.push_back(QByteArrayLiteral("incus"));
    for (const QString &arg : execArgs)
        rawArgs.push_back(arg.toUtf8());

    std::vector<char *> cArgv;
    cArgv.reserve(rawArgs.size() + 1);
    for (QByteArray &buf : rawArgs)
        cArgv.push_back(buf.data());
    cArgv.push_back(nullptr);

    std::fflush(stdout);
    std::fflush(stderr);
    ::execvp("incus", cArgv.data());
    std::fprintf(stderr, "Error: Could not exec incus: %s\n", std::strerror(errno));
    return 1;
}

int runExec(Backend &backend, const ParsedArgs &parsed) {
    QVariantMap env;
    const int checkRc = requireEnvironment(backend, parsed.envName, true, &env);
    if (checkRc != 0)
        return checkRc;

    const int preRc = runPreStepsForExec(backend, parsed.envName,
                                         QObject::tr("Executing command in '%1' (CLI)").arg(parsed.envName));
    if (preRc != 0)
        return preRc;

    QString remoteEnvPath;
    QString secretErr;
    if (!backend.pushEnvironmentSecretsToContainer(parsed.envName, &remoteEnvPath, &secretErr)) {
        std::fprintf(stderr, "Error: %s\n", secretErr.toUtf8().constData());
        return 1;
    }

    const QStringList args = backend.execIncusCommandArgs(parsed.envName, parsed.execCommand, remoteEnvPath);

    QProcess proc;
    proc.setProcessChannelMode(QProcess::ForwardedChannels);
    proc.setInputChannelMode(QProcess::ForwardedInputChannel);
    proc.start(QStringLiteral("incus"), args);
    if (!proc.waitForStarted(5000)) {
        std::fprintf(stderr, "Error: Failed to invoke incus exec.\n");
        return 1;
    }
    proc.waitForFinished(-1);
    if (proc.exitStatus() != QProcess::NormalExit)
        return 1;
    return proc.exitCode();
}

int runStart(Backend &backend, const ParsedArgs &parsed) {
    QVariantMap env;
    const int checkRc = requireEnvironment(backend, parsed.envName, true, &env);
    if (checkRc != 0)
        return checkRc;
    if (backend.isTaskLockActive()) {
        std::fprintf(stderr, "Error: Another AIBox background task is currently running.\n");
        return 3;
    }
    if (env.value("running").toBool()) {
        std::fprintf(stdout, "Environment '%s' is already running.\n",
                     parsed.envName.toUtf8().constData());
        return 0;
    }
    return runScriptWithTaskLock(backend,
                                 QObject::tr("Starting '%1'").arg(parsed.envName),
                                 parsed.envName,
                                 backend.startEnvironmentScript(parsed.envName));
}

int runStop(Backend &backend, const ParsedArgs &parsed) {
    QVariantMap env;
    const int checkRc = requireEnvironment(backend, parsed.envName, true, &env);
    if (checkRc != 0)
        return checkRc;
    if (backend.isTaskLockActive()) {
        std::fprintf(stderr, "Error: Another AIBox background task is currently running.\n");
        return 3;
    }
    if (!env.value("running").toBool()) {
        std::fprintf(stdout, "Environment '%s' is already stopped.\n",
                     parsed.envName.toUtf8().constData());
        return 0;
    }
    return runScriptWithTaskLock(backend,
                                 QObject::tr("Stopping '%1'").arg(parsed.envName),
                                 parsed.envName,
                                 backend.stopEnvironmentScript(parsed.envName));
}

int runSnapshot(Backend &backend, const ParsedArgs &parsed) {
    QVariantMap env;
    const int checkRc = requireEnvironment(backend, parsed.envName, true, &env);
    if (checkRc != 0)
        return checkRc;
    if (backend.isTaskLockActive()) {
        std::fprintf(stderr, "Error: Another AIBox background task is currently running.\n");
        return 3;
    }

    QString snap = parsed.snapshotName;
    if (snap.isEmpty()) {
        snap = QStringLiteral("snap-") + QDateTime::currentDateTime().toString(QStringLiteral("yyyyMMdd-HHmmss"));
    } else {
        snap = Backend::sanitizeName(snap);
        if (snap.isEmpty()) {
            std::fprintf(stderr, "Error: Invalid snapshot name.\n");
            return 2;
        }
    }

    const QString wsPath = env.value("workspacePath").toString().isEmpty()
                               ? backend.workspacePathForEnv(parsed.envName)
                               : env.value("workspacePath").toString();
    const bool includeWs = env.value("workspaceSnapshotEnabled").toBool();
    const QString script = Backend::createSnapshotScript(parsed.envName, snap, wsPath, includeWs);
    return runScriptWithTaskLock(backend,
                                 QObject::tr("Saving snapshot '%1' for '%2'").arg(snap, parsed.envName),
                                 parsed.envName,
                                 script);
}

int runSnapshots(Backend &backend, const ParsedArgs &parsed) {
    QVariantMap env;
    const int checkRc = requireEnvironment(backend, parsed.envName, false, &env);
    if (checkRc != 0)
        return checkRc;

    const QStringList incusSnaps = env.value("snapshots").toStringList();
    const QStringList wsSnaps = env.value("workspaceSnapshots").toStringList();
    const QString wsSnapDir = env.value("workspaceSnapshotDir").toString();

    QVariantMap sysSizes = env.value("snapshotSizes").toMap();
    if (!incusSnaps.isEmpty()) {
        QProcess qProc;
        qProc.start(QStringLiteral("incus"),
                    {QStringLiteral("query"),
                     QStringLiteral("/1.0/instances/%1/snapshots?recursion=1").arg(parsed.envName)});
        if (qProc.waitForFinished(5000) && qProc.exitCode() == 0) {
            const QVariantMap parsedSys = Backend::parseSnapshotSizesJson(qProc.readAllStandardOutput());
            for (auto it = parsedSys.constBegin(); it != parsedSys.constEnd(); ++it)
                sysSizes.insert(it.key(), it.value());
        }
    }

    QVariantMap wsSizes;
    if (!wsSnaps.isEmpty() && !wsSnapDir.isEmpty()) {
        QProcess duProc;
        QStringList args{QStringLiteral("-sb")};
        for (const QString &s : wsSnaps)
            args.append(wsSnapDir + QLatin1Char('/') + s);
        duProc.start(QStringLiteral("du"), args);
        if (duProc.waitForFinished(5000) && duProc.exitCode() == 0)
            wsSizes = Backend::parseDuBytesOutput(QString::fromUtf8(duProc.readAllStandardOutput()));
    }

    QStringList allNames = incusSnaps;
    for (const QString &ws : wsSnaps) {
        if (!allNames.contains(ws))
            allNames.append(ws);
    }

    QVariantList rows;
    for (const QString &snap : allNames) {
        const bool hasSys = incusSnaps.contains(snap);
        const bool hasWs = wsSnaps.contains(snap);
        QVariantMap row;
        row["name"] = snap;
        row["hasSystem"] = hasSys;
        row["hasWorkspace"] = hasWs;
        row["isOrphanWorkspace"] = hasWs && !hasSys;
        row["isAuto"] = Backend::isAutoSnapshotName(snap);
        row["systemSize"] = sysSizes.value(snap).toString();
        row["workspaceSize"] = wsSizes.value(snap).toString();
        rows.append(row);
    }

    if (parsed.jsonOutput) {
        const QByteArray out = QJsonDocument::fromVariant(rows).toJson(QJsonDocument::Indented);
        std::fwrite(out.constData(), 1, out.size(), stdout);
        return 0;
    }

    if (rows.isEmpty()) {
        std::fprintf(stdout, "No snapshots for '%s'.\n", parsed.envName.toUtf8().constData());
        return 0;
    }

    std::fprintf(stdout, "%-28s %-14s %-12s %-12s\n", "NAME", "SCOPE", "SYSTEM_SIZE", "WORKSPACE_SIZE");
    for (const QVariant &v : rows) {
        const QVariantMap r = v.toMap();
        const bool hasSys = r.value("hasSystem").toBool();
        const bool hasWs = r.value("hasWorkspace").toBool();
        const QString scope = (hasSys && hasWs) ? QStringLiteral("both")
                                                : (hasSys ? QStringLiteral("system")
                                                          : QStringLiteral("workspace-only"));
        const QString sysSz = r.value("systemSize").toString().isEmpty() ? QStringLiteral("-") : r.value("systemSize").toString();
        const QString wsSz = r.value("workspaceSize").toString().isEmpty() ? QStringLiteral("-") : r.value("workspaceSize").toString();
        std::fprintf(stdout, "%-28s %-14s %-12s %-12s\n",
                     r.value("name").toString().toUtf8().constData(),
                     scope.toUtf8().constData(),
                     sysSz.toUtf8().constData(),
                     wsSz.toUtf8().constData());
    }
    return 0;
}

int runRestore(Backend &backend, const ParsedArgs &parsed) {
    QVariantMap env;
    const int checkRc = requireEnvironment(backend, parsed.envName, true, &env);
    if (checkRc != 0)
        return checkRc;
    if (backend.isTaskLockActive()) {
        std::fprintf(stderr, "Error: Another AIBox background task is currently running.\n");
        return 3;
    }

    const QStringList incusSnaps = env.value("snapshots").toStringList();
    const QStringList wsSnaps = env.value("workspaceSnapshots").toStringList();
    const bool hasSys = incusSnaps.contains(parsed.snapshotName);
    const bool hasWs = wsSnaps.contains(parsed.snapshotName);

    QString mode = QStringLiteral("both");
    if (parsed.systemOnly)
        mode = QStringLiteral("system");
    else if (parsed.workspaceOnly)
        mode = QStringLiteral("workspace");

    if (mode == QLatin1String("workspace") && !hasWs) {
        std::fprintf(stderr, "Error: Workspace snapshot '%s' was not found.\n",
                     parsed.snapshotName.toUtf8().constData());
        return 1;
    }
    if (mode == QLatin1String("system") && !hasSys) {
        std::fprintf(stderr, "Error: System snapshot '%s' was not found.\n",
                     parsed.snapshotName.toUtf8().constData());
        return 1;
    }
    if (mode == QLatin1String("both")) {
        if (!hasSys && !hasWs) {
            std::fprintf(stderr, "Error: Snapshot '%s' was not found.\n",
                         parsed.snapshotName.toUtf8().constData());
            return 1;
        }
        if (!hasSys && hasWs)
            mode = QStringLiteral("workspace");
    }

    if (!parsed.yes) {
        std::fprintf(stdout, "Restore '%s' to snapshot '%s' (%s)? [y/N] ",
                     parsed.envName.toUtf8().constData(),
                     parsed.snapshotName.toUtf8().constData(),
                     mode.toUtf8().constData());
        std::fflush(stdout);
        char buf[128] = {};
        if (!std::fgets(buf, sizeof(buf), stdin)) {
            std::fprintf(stderr, "Aborted.\n");
            return 1;
        }
        const QString reply = QString::fromUtf8(buf).trimmed().toLower();
        if (reply != QLatin1String("y") && reply != QLatin1String("yes")) {
            std::fprintf(stderr, "Aborted.\n");
            return 1;
        }
    }

    const QString wsPath = env.value("workspacePath").toString().isEmpty()
                               ? backend.workspacePathForEnv(parsed.envName)
                               : env.value("workspacePath").toString();
    const QString script = Backend::restoreSnapshotScript(parsed.envName, parsed.snapshotName,
                                                          wsPath, mode, hasWs);
    return runScriptWithTaskLock(backend,
                                 QObject::tr("Restoring '%1' to snapshot '%2'").arg(parsed.envName, parsed.snapshotName),
                                 parsed.envName,
                                 script);
}

int runDiff(Backend &backend, const ParsedArgs &parsed) {
    QVariantMap env;
    const int checkRc = requireEnvironment(backend, parsed.envName, false, &env);
    if (checkRc != 0)
        return checkRc;

    const QString wsPath = env.value("workspacePath").toString().isEmpty()
                               ? backend.workspacePathForEnv(parsed.envName)
                               : env.value("workspacePath").toString();
    const QStringList wsSnaps = env.value("workspaceSnapshots").toStringList();
    QString snapName = parsed.snapshotName;
    if (snapName.isEmpty()) {
        if (wsSnaps.isEmpty()) {
            std::fprintf(stderr, "Error: No workspace snapshots found for '%s'.\n",
                         parsed.envName.toUtf8().constData());
            return 1;
        }
        snapName = wsSnaps.last();
    }

    const QString snapDir = Backend::workspaceSnapshotDir(wsPath, parsed.envName);
    const QString snapPath = snapDir + QLatin1Char('/') + snapName;
    if (snapDir.isEmpty() || !QFileInfo(snapPath).isDir()) {
        std::fprintf(stderr, "Error: Workspace snapshot '%s' was not found.\n",
                     snapName.toUtf8().constData());
        return 1;
    }

    QProcess proc;
    proc.start(QStringLiteral("rsync"),
               Backend::buildWorkspaceDiffArgs(snapPath, wsPath, backend.diffExcludesList()));
    if (!proc.waitForStarted(5000)) {
        std::fprintf(stderr, "Error: Failed to invoke rsync.\n");
        return 1;
    }
    if (!proc.waitForFinished(30000)) {
        proc.kill();
        proc.waitForFinished(1000);
        std::fprintf(stderr, "Error: Diff scan timed out after 30 seconds.\n");
        return 1;
    }
    if (proc.exitStatus() != QProcess::NormalExit || (proc.exitCode() != 0 && proc.exitCode() != 24)) {
        const QString err = QString::fromUtf8(proc.readAllStandardError()).trimmed();
        std::fprintf(stderr, "Error: %s\n",
                     err.isEmpty() ? "Failed to compare workspace with snapshot." : err.toUtf8().constData());
        return 1;
    }

    QVariantList entries;
    const QVariantMap summary = Backend::processWorkspaceDiffOutput(
        snapName, wsPath, QString::fromUtf8(proc.readAllStandardOutput()), &entries);

    if (parsed.jsonOutput) {
        QVariantMap root;
        root["summary"] = summary;
        root["entries"] = entries;
        const QByteArray out = QJsonDocument::fromVariant(root).toJson(QJsonDocument::Indented);
        std::fwrite(out.constData(), 1, out.size(), stdout);
        return 0;
    }

    std::fprintf(stdout, "Snapshot: %s (+%d ~%d -%d, total %d%s)\n",
                 snapName.toUtf8().constData(),
                 summary.value("addedCount").toInt(),
                 summary.value("modifiedCount").toInt(),
                 summary.value("deletedCount").toInt(),
                 summary.value("totalCount").toInt(),
                 summary.value("truncated").toBool() ? ", truncated at 5000" : "");

    for (const QVariant &v : entries) {
        const QVariantMap r = v.toMap();
        const QString st = r.value("status").toString();
        const char *tag = (st == QLatin1String("added")) ? "A"
                          : (st == QLatin1String("deleted") ? "D" : "M");
        const QString sz = r.value("sizeText").toString().isEmpty() ? QStringLiteral("-") : r.value("sizeText").toString();
        std::fprintf(stdout, "%s  %-10s %s\n", tag,
                     sz.toUtf8().constData(),
                     r.value("path").toString().toUtf8().constData());
    }
    return 0;
}

} // namespace

ParsedArgs parseArgs(const QStringList &args) {
    ParsedArgs parsed;
    QStringList positionals;
    bool parsingExecTail = false;
    bool afterDoubleDash = false;
    bool sawDoubleDash = false;
    bool sawHelp = false;
    bool sawJson = false;
    bool sawFix = false;
    bool sawSystemOnly = false;
    bool sawWorkspaceOnly = false;
    bool sawYes = false;

    for (int i = 0; i < args.size(); ++i) {
        const QString &arg = args.at(i);

        if (parsingExecTail) {
            parsed.execCommand.append(arg);
            continue;
        }

        if (arg == QLatin1String("--") && !afterDoubleDash) {
            sawDoubleDash = true;
            if (parsed.command == Command::Exec) {
                parsingExecTail = true;
            } else {
                afterDoubleDash = true;
            }
            continue;
        }

        if (!afterDoubleDash) {
            if (arg == QLatin1String("--project")) {
                if (i + 1 >= args.size() || args.at(i + 1).trimmed().isEmpty())
                    return makeUsageError(QStringLiteral("--project requires a directory path."));
                parsed.projectDir = args.at(++i);
                continue;
            }
            if (arg.startsWith(QLatin1String("--project="))) {
                const QString val = arg.mid(10);
                if (val.trimmed().isEmpty())
                    return makeUsageError(QStringLiteral("--project requires a directory path."));
                parsed.projectDir = val;
                continue;
            }
            if (arg == QLatin1String("-h") || arg == QLatin1String("--help")) {
                sawHelp = true;
                continue;
            }
            if (arg == QLatin1String("--json")) {
                sawJson = true;
                continue;
            }
            if (arg == QLatin1String("--fix")) {
                sawFix = true;
                continue;
            }
            if (arg == QLatin1String("--system-only")) {
                sawSystemOnly = true;
                continue;
            }
            if (arg == QLatin1String("--workspace-only")) {
                sawWorkspaceOnly = true;
                continue;
            }
            if (arg == QLatin1String("--yes") || arg == QLatin1String("-y")) {
                sawYes = true;
                continue;
            }
            if (arg.startsWith(QLatin1Char('-'))) {
                return makeUsageError(QStringLiteral("Unknown option: %1").arg(arg), parsed.command);
            }
        }

        if (parsed.command == Command::None) {
            const Command cmd = parseCommandName(arg);
            if (cmd == Command::None)
                return makeUsageError(QStringLiteral("Unknown command: %1").arg(arg));
            parsed.command = cmd;
        } else if (parsed.command == Command::Help) {
            if (parsed.helpTarget != Command::None)
                return makeUsageError(QStringLiteral("Unexpected argument: %1").arg(arg), Command::Help);
            const Command target = parseCommandName(arg);
            if (target == Command::None)
                return makeUsageError(QStringLiteral("Unknown help topic: %1").arg(arg), Command::Help);
            parsed.helpTarget = target;
        } else {
            positionals.append(arg);
        }
    }

    if (sawHelp) {
        parsed.helpTarget = (parsed.command == Command::Help) ? parsed.helpTarget : parsed.command;
        parsed.command = Command::Help;
        return parsed;
    }

    if (parsed.command == Command::Help) {
        if (sawJson || sawFix || sawSystemOnly || sawWorkspaceOnly || sawYes)
            return makeUsageError(QStringLiteral("Invalid option for help."), Command::Help);
        return parsed;
    }

    if (parsed.command == Command::None) {
        if (sawJson || sawFix || sawSystemOnly || sawWorkspaceOnly || sawYes || sawDoubleDash)
            return makeUsageError(QStringLiteral("Missing subcommand."));
        return parsed;
    }

    if (sawJson && parsed.command != Command::List && parsed.command != Command::Check &&
        parsed.command != Command::Snapshots && parsed.command != Command::Diff) {
        return makeUsageError(QStringLiteral("--json is not supported for this command."), parsed.command);
    }
    if (sawFix && parsed.command != Command::Check)
        return makeUsageError(QStringLiteral("--fix is only supported for 'check'."), parsed.command);
    if ((sawSystemOnly || sawWorkspaceOnly || sawYes) && parsed.command != Command::Restore)
        return makeUsageError(QStringLiteral("Restore flags require the 'restore' command."), parsed.command);
    if (sawSystemOnly && sawWorkspaceOnly)
        return makeUsageError(QStringLiteral("Cannot combine --system-only and --workspace-only."), Command::Restore);

    parsed.jsonOutput = sawJson;
    parsed.fix = sawFix;
    parsed.systemOnly = sawSystemOnly;
    parsed.workspaceOnly = sawWorkspaceOnly;
    parsed.yes = sawYes;

    switch (parsed.command) {
    case Command::List:
    case Command::Check:
        if (!positionals.isEmpty())
            return makeUsageError(QStringLiteral("Unexpected argument: %1").arg(positionals.first()), parsed.command);
        break;
    case Command::Enter:
    case Command::Start:
    case Command::Stop:
    case Command::Snapshots:
        if (positionals.size() != 1)
            return makeUsageError(QStringLiteral("Expected exactly <env>."), parsed.command);
        parsed.envName = positionals.at(0);
        break;
    case Command::Exec:
        if (positionals.isEmpty())
            return makeUsageError(QStringLiteral("exec requires <env> and '-- <cmd...>'."), Command::Exec);
        if (positionals.size() > 1 || !sawDoubleDash)
            return makeUsageError(QStringLiteral("exec requires '--' before the command: aibox exec <env> -- <cmd...>"), Command::Exec);
        if (parsed.execCommand.isEmpty())
            return makeUsageError(QStringLiteral("Missing command after '--'."), Command::Exec);
        parsed.envName = positionals.at(0);
        break;
    case Command::Snapshot:
        if (positionals.isEmpty() || positionals.size() > 2)
            return makeUsageError(QStringLiteral("Expected <env> [name]."), Command::Snapshot);
        parsed.envName = positionals.at(0);
        if (positionals.size() == 2) {
            parsed.snapshotName = Backend::sanitizeName(positionals.at(1));
            if (parsed.snapshotName.isEmpty())
                return makeUsageError(QStringLiteral("Invalid snapshot name: %1").arg(positionals.at(1)), Command::Snapshot);
        }
        break;
    case Command::Restore:
        if (positionals.size() != 2)
            return makeUsageError(QStringLiteral("Expected <env> <snap>."), Command::Restore);
        parsed.envName = positionals.at(0);
        parsed.snapshotName = positionals.at(1);
        break;
    case Command::Diff:
        if (positionals.isEmpty() || positionals.size() > 2)
            return makeUsageError(QStringLiteral("Expected <env> [snap]."), Command::Diff);
        parsed.envName = positionals.at(0);
        if (positionals.size() == 2)
            parsed.snapshotName = positionals.at(1);
        break;
    case Command::None:
    case Command::Help:
        break;
    }

    return parsed;
}

bool isCliInvocation(int argc, char *argv[]) {
    if (argc <= 1)
        return false;
    const ParsedArgs parsed = parseArgs(argvWithoutProgram(argc, argv));
    if (!parsed.valid)
        return true;
    return parsed.command != Command::None;
}

QString helpText(Command cmd) {
    switch (cmd) {
    case Command::List:
        return QStringLiteral(
            "Usage: aibox [--project <dir>] list [--json]\n\n"
            "List Incus environments managed by AIBox in JSON format.\n");
    case Command::Check:
        return QStringLiteral(
            "Usage: aibox [--project <dir>] check [--fix] [--json]\n\n"
            "Run host diagnostics for Incus, bridge, KVM, GPU, and Btrfs snapshots.\n");
    case Command::Enter:
        return QStringLiteral(
            "Usage: aibox [--project <dir>] enter <env>\n\n"
            "Run pre-steps (auto-snapshot, allowlist refresh, start if stopped) and attach an interactive shell in /home/workspace.\n");
    case Command::Exec:
        return QStringLiteral(
            "Usage: aibox [--project <dir>] exec <env> -- <cmd...>\n\n"
            "Run pre-steps and execute a non-interactive command in /home/workspace, returning its exit code.\n");
    case Command::Start:
        return QStringLiteral(
            "Usage: aibox [--project <dir>] start <env>\n\n"
            "Start <env> with AIBox auto-snapshot and network allowlist refresh rules.\n");
    case Command::Stop:
        return QStringLiteral(
            "Usage: aibox [--project <dir>] stop <env>\n\n"
            "Stop <env>.\n");
    case Command::Snapshot:
        return QStringLiteral(
            "Usage: aibox [--project <dir>] snapshot <env> [name]\n\n"
            "Create a paired OS container and /home/workspace Btrfs snapshot.\n");
    case Command::Snapshots:
        return QStringLiteral(
            "Usage: aibox [--project <dir>] snapshots <env> [--json]\n\n"
            "List snapshots with OS/workspace availability and sizes.\n");
    case Command::Restore:
        return QStringLiteral(
            "Usage: aibox [--project <dir>] restore <env> <snap> [--system-only|--workspace-only] [--yes]\n\n"
            "Restore an environment snapshot (with pre-restore workspace safety snapshot).\n");
    case Command::Diff:
        return QStringLiteral(
            "Usage: aibox [--project <dir>] diff <env> [snap] [--json]\n\n"
            "Show /home/workspace file changes compared against a snapshot (default: latest).\n");
    case Command::Help:
    case Command::None:
        break;
    }

    return QStringLiteral(
        "Usage: aibox [--project <dir>] <command> [options]\n\n"
        "Commands:\n"
        "  list [--json]                                       List environments\n"
        "  check [--fix] [--json]                              Check/repair host setup\n"
        "  enter <env>                                         Open interactive shell in /home/workspace\n"
        "  exec <env> -- <cmd...>                              Run command in /home/workspace\n"
        "  start <env>                                         Start environment\n"
        "  stop <env>                                          Stop environment\n"
        "  snapshot <env> [name]                               Create paired OS + /home/workspace snapshot\n"
        "  snapshots <env> [--json]                            List snapshots and sizes\n"
        "  restore <env> <snap> [--system-only|--workspace-only] [--yes]\n"
        "                                                      Restore snapshot\n"
        "  diff <env> [snap] [--json]                          Review /home/workspace changes\n"
        "  help [command]                                      Show help\n\n"
        "Global options:\n"
        "  --project <dir>                                     Select project directory containing aibox/ (default: ~/aibox)\n"
        "  -h, --help                                          Show help\n\n"
        "Exit codes:\n"
        "  0  OK (or <cmd> exit code for exec)\n"
        "  1  Operation failed\n"
        "  2  Usage error\n"
        "  3  Another AIBox background task is running\n"
        "  4  Environment not found or is a template\n");
}

int execute(const ParsedArgs &parsed) {
    if (!parsed.valid) {
        std::fprintf(stderr, "Error: %s\n\n", parsed.errorMessage.toUtf8().constData());
        const QString h = helpText(parsed.command);
        std::fwrite(h.toUtf8().constData(), 1, h.toUtf8().size(), stderr);
        return parsed.exitCode;
    }

    if (parsed.command == Command::Help) {
        const QString h = helpText(parsed.helpTarget);
        std::fwrite(h.toUtf8().constData(), 1, h.toUtf8().size(), stdout);
        return 0;
    }

    if (parsed.command == Command::Check)
        return runCheck(parsed);

    Backend backend(parsed.projectDir, Backend::PathMode::ProjectDir);

    switch (parsed.command) {
    case Command::List:
        return runList(backend);
    case Command::Enter:
        return runEnter(backend, parsed);
    case Command::Exec:
        return runExec(backend, parsed);
    case Command::Start:
        return runStart(backend, parsed);
    case Command::Stop:
        return runStop(backend, parsed);
    case Command::Snapshot:
        return runSnapshot(backend, parsed);
    case Command::Snapshots:
        return runSnapshots(backend, parsed);
    case Command::Restore:
        return runRestore(backend, parsed);
    case Command::Diff:
        return runDiff(backend, parsed);
    case Command::Check:
    case Command::Help:
    case Command::None:
        break;
    }
    return 2;
}

int run(int argc, char *argv[]) {
    return execute(parseArgs(argvWithoutProgram(argc, argv)));
}

} // namespace Cli
