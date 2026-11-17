#include "backend.h"
#include "backend_helpers.h"

#include <QDateTime>
#include <QDesktopServices>
#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QRegularExpression>
#include <QSet>
#include <QStandardPaths>
#include <QStorageInfo>
#include <QUrl>

#include <algorithm>

QString Backend::backupsDirPath() const {
    return m_osStoragePath + QStringLiteral("/backups");
}

bool Backend::hasSufficientDiskSpace(qint64 requiredBytes, qint64 availableBytes) {
    if (availableBytes < 0 || requiredBytes < 0)
        return false;
    constexpr qint64 kSafetyMarginBytes = 100LL * 1024LL * 1024LL; // 100 MiB safety margin
    return availableBytes >= (requiredBytes + kSafetyMarginBytes);
}

qint64 Backend::directorySizeBytes(const QString &dirPath) {
    if (dirPath.isEmpty())
        return 0;
    const QFileInfo rootInfo(dirPath);
    if (!rootInfo.exists())
        return 0;
    if (rootInfo.isFile())
        return rootInfo.size();
    qint64 total = 0;
    QDirIterator it(dirPath, QDir::Files | QDir::Hidden | QDir::NoSymLinks, QDirIterator::Subdirectories);
    while (it.hasNext()) {
        it.next();
        total += it.fileInfo().size();
    }
    return total;
}

qint64 Backend::availableDiskBytes(const QString &path) {
    QString probe = path;
    while (!probe.isEmpty() && !QFileInfo::exists(probe)) {
        const QString parent = QFileInfo(probe).absolutePath();
        if (parent == probe)
            break;
        probe = parent;
    }
    const QStorageInfo storage(probe);
    if (!storage.isValid() || !storage.isReady())
        return -1;
    return storage.bytesAvailable();
}

QVariantList Backend::scanBackupArchives(const QString &backupsDir) {
    QVariantList result;
    const QDir dir(backupsDir);
    if (!dir.exists())
        return result;

    const QFileInfoList entries = dir.entryInfoList(QStringList{QStringLiteral("*.tar.gz")}, QDir::Files, QDir::Time);
    QHash<QString, QFileInfo> workspaceMap;
    for (const QFileInfo &fi : entries) {
        if (fi.fileName().endsWith(QStringLiteral("-workspace.tar.gz"))) {
            const QString key = fi.fileName().chopped(QStringLiteral("-workspace.tar.gz").size());
            workspaceMap.insert(key, fi);
        }
    }

    static const QRegularExpression tsSuffixRe(QStringLiteral("-\\d{8}-\\d{6}$"));
    for (const QFileInfo &fi : entries) {
        const QString fn = fi.fileName();
        if (fn.endsWith(QStringLiteral("-workspace.tar.gz")))
            continue;
        QString baseName = fn.chopped(QStringLiteral(".tar.gz").size());
        QString envName = baseName;
        envName.remove(tsSuffixRe);
        const QFileInfo wsInfo = workspaceMap.value(baseName);
        QVariantMap item;
        item["fileName"] = fn;
        item["filePath"] = fi.absoluteFilePath();
        item["envName"] = envName;
        item["sizeBytes"] = fi.size();
        item["sizeDisplay"] = formatBytes(fi.size());
        item["createdAt"] = fi.lastModified().toString(QStringLiteral("yyyy-MM-dd HH:mm"));
        item["hasWorkspaceArchive"] = wsInfo.exists();
        item["workspaceArchivePath"] = wsInfo.exists() ? wsInfo.absoluteFilePath() : QString();
        item["workspaceArchiveSizeDisplay"] = wsInfo.exists() ? formatBytes(wsInfo.size()) : QString();
        result.append(item);
    }
    return result;
}

namespace {
QString buildExportSteps(const QString &name, const QString &backupFile,
                         const QString &workspacePath, const QString &workspaceArchiveFile) {
    const QString qName = QStringLiteral("'%1'").arg(QString(name).replace(QLatin1Char('\''), QStringLiteral("'\\''")));
    const QString qBackup = QStringLiteral("'%1'").arg(QString(backupFile).replace(QLatin1Char('\''), QStringLiteral("'\\''")));
    QString script = QStringLiteral(
        "set -e\n"
        "mkdir -p \"$(dirname %2)\"\n"
        "echo '@@STAGE:15:Exporting instance...'\n"
        "incus export %1 %2 --compression gzip\n"
        "echo '@@STAGE:60:Instance archive written.'")
                         .arg(qName, qBackup);

    if (!workspacePath.trimmed().isEmpty() && !workspaceArchiveFile.trimmed().isEmpty()) {
        const QString qWs = QStringLiteral("'%1'").arg(QString(workspacePath).replace(QLatin1Char('\''), QStringLiteral("'\\''")));
        const QString qWsOut = QStringLiteral("'%1'").arg(QString(workspaceArchiveFile).replace(QLatin1Char('\''), QStringLiteral("'\\''")));
        script += QStringLiteral(
            "\nif [ -d %1 ]; then\n"
            "    echo '@@STAGE:72:Archiving workspace directory...'\n"
            "    tar -czf %2 -C %1 .\n"
            "fi")
                      .arg(qWs, qWsOut);
    }
    return script;
}
} // namespace

QString Backend::exportEnvironmentScript(const QString &name, const QString &backupFile,
                                         const QString &workspacePath,
                                         const QString &workspaceArchiveFile) {
    QString script = buildExportSteps(name, backupFile, workspacePath, workspaceArchiveFile);
    script += QStringLiteral("\necho '@@STAGE:100:Backup export complete.'\n");
    return script;
}

QString Backend::exportAndWipeEnvironmentScript(const QString &name, const QString &backupFile,
                                                const QString &workspacePath,
                                                const QString &workspaceArchiveFile,
                                                bool deleteWorkspaceFiles,
                                                const QString &sessionsEnvDir) {
    const QString qBackup = shellQuote(backupFile);
    QString script = buildExportSteps(name, backupFile, workspacePath, workspaceArchiveFile);
    script += QStringLiteral(
        "\necho '@@STAGE:78:Verifying exported archive before wipe...'\n"
        "if [ ! -s %1 ]; then\n"
        "    echo 'Export verification failed: instance backup archive is missing or empty; aborting wipe.' >&2\n"
        "    exit 1\n"
        "fi\n").arg(qBackup);
    if (!workspacePath.trimmed().isEmpty() && !workspaceArchiveFile.trimmed().isEmpty()) {
        const QString qWs = shellQuote(workspacePath);
        const QString qWsOut = shellQuote(workspaceArchiveFile);
        script += QStringLiteral(
            "if [ -d %1 ] && [ ! -s %2 ]; then\n"
            "    echo 'Export verification failed: workspace archive is missing or empty; aborting wipe.' >&2\n"
            "    exit 1\n"
            "fi\n").arg(qWs, qWsOut);
    }
    script += wipeEnvironmentScript(name, workspacePath, deleteWorkspaceFiles, sessionsEnvDir);
    return script;
}

QString Backend::importEnvironmentScript(const QString &backupFile, const QString &targetName,
                                         const QString &workspacePath,
                                         const QString &workspaceArchiveFile) {
    const QString qBackup = shellQuote(backupFile);
    const QString qName = shellQuote(targetName);
    const QString qWs = shellQuote(workspacePath);
    QString script = QStringLiteral(
        "set -e\n"
        "echo '@@STAGE:15:Preparing target workspace...'\n"
        "%4\n"
        "if command -v setfacl >/dev/null 2>&1; then\n"
        "    ACL_USER=\"${SUDO_USER:-${USER:-$(id -un)}}\"\n"
        "    setfacl -R -m \"u:${ACL_USER}:rwx\" -m \"m::rwx\" -d -m \"u:${ACL_USER}:rwx\" -d -m \"m::rwx\" %3 2>/dev/null || true\n"
        "fi\n"
        "echo '@@STAGE:35:Importing archive as %2...'\n"
        "incus import %1 %2\n"
        "incus config unset %2 user.aibox.template >/dev/null 2>&1 || true\n"
        "incus config set %2 user.aibox.workspace %3 >/dev/null 2>&1 || true\n"
        "incus config device set %2 workspace source=%3 path=%5 >/dev/null 2>&1 || "
        "incus config device add %2 workspace disk source=%3 path=%5 >/dev/null 2>&1 || true")
                         .arg(qBackup, qName, qWs, workspaceCreateSnippet(workspacePath),
                              shellQuote(kContainerWorkspacePath));

    if (!workspaceArchiveFile.trimmed().isEmpty()) {
        const QString qWsArc = shellQuote(workspaceArchiveFile);
        script += QStringLiteral(
            "\nif [ -f %1 ]; then\n"
            "    echo '@@STAGE:82:Extracting workspace archive...'\n"
            "    tar -xzf %1 -C %2\n"
            "fi")
                      .arg(qWsArc, qWs);
    }

    script += QStringLiteral("\necho '@@STAGE:100:Environment import complete.'\n");
    return script;
}

void Backend::refreshBackups() {
    if (m_demoMode)
        return;
    m_backupArchives = scanBackupArchives(backupsDirPath());
    emit backupArchivesChanged();
}

QVariantMap Backend::checkExportSpace(const QString &name, bool includeWorkspace) const {
    const QVariantMap env = environmentByName(name);
    qint64 instanceBytes = env.value(QStringLiteral("diskUsageBytes")).toLongLong();
    if (instanceBytes <= 0)
        instanceBytes = parseByteSizeToBytes(env.value(QStringLiteral("diskUsage")).toString());
    if (instanceBytes <= 0)
        instanceBytes = 512LL * 1024LL * 1024LL; // conservative 512 MiB fallback when disk usage unknown

    qint64 workspaceBytes = 0;
    if (includeWorkspace) {
        QString wsPath = env.value(QStringLiteral("workspacePath")).toString().trimmed();
        if (wsPath.startsWith(QLatin1String("~/")))
            wsPath = QDir::homePath() + wsPath.mid(1);
        workspaceBytes = directorySizeBytes(wsPath);
    }

    const qint64 estimatedBytes = instanceBytes + workspaceBytes;
    const qint64 availableBytes = m_demoMode ? (40LL * 1024LL * 1024LL * 1024LL) : availableDiskBytes(backupsDirPath());
    const bool ok = hasSufficientDiskSpace(estimatedBytes, availableBytes);

    QVariantMap out;
    out["ok"] = ok;
    out["estimatedBytes"] = estimatedBytes;
    out["estimatedDisplay"] = formatBytes(estimatedBytes);
    out["availableBytes"] = availableBytes;
    out["availableDisplay"] = availableBytes >= 0 ? formatBytes(availableBytes) : tr("Unknown");
    if (!ok) {
        out["message"] = tr("Insufficient disk space in %1 (needs %2 + 100 MB margin, only %3 free).")
                             .arg(backupsDirPath(), formatBytes(estimatedBytes),
                                  availableBytes >= 0 ? formatBytes(availableBytes) : tr("0 MB"));
    } else {
        out["message"] = QString();
    }
    return out;
}

QString Backend::exportEnvironment(const QString &name, bool includeWorkspace) {
    if (m_busy)
        return tr("Another operation is already running.");
    const QString clean = sanitizeName(name);
    if (clean.isEmpty() || environmentByName(clean).isEmpty())
        return tr("Environment '%1' was not found.").arg(name);

    const QVariantMap space = checkExportSpace(clean, includeWorkspace);
    if (!space.value(QStringLiteral("ok")).toBool())
        return space.value(QStringLiteral("message")).toString();

    if (m_demoMode) {
        const QString stamp = QDateTime::currentDateTime().toString(QStringLiteral("yyyyMMdd-HHmmss"));
        const QString fn = QStringLiteral("%1-%2.tar.gz").arg(clean, stamp);
        QVariantMap item;
        item["fileName"] = fn;
        item["filePath"] = backupsDirPath() + QLatin1Char('/') + fn;
        item["envName"] = clean;
        item["sizeBytes"] = space.value(QStringLiteral("estimatedBytes")).toLongLong();
        item["sizeDisplay"] = space.value(QStringLiteral("estimatedDisplay")).toString();
        item["createdAt"] = QDateTime::currentDateTime().toString(QStringLiteral("yyyy-MM-dd HH:mm"));
        item["hasWorkspaceArchive"] = includeWorkspace;
        item["workspaceArchivePath"] = includeWorkspace ? (backupsDirPath() + QStringLiteral("/%1-%2-workspace.tar.gz").arg(clean, stamp)) : QString();
        item["workspaceArchiveSizeDisplay"] = includeWorkspace ? QStringLiteral("42 MB") : QString();
        m_backupArchives.prepend(item);
        emit backupArchivesChanged();
        m_statusMessage = tr("[Demo] Exported backup '%1'.").arg(fn);
        emit statusMessageChanged();
        return {};
    }

    QDir().mkpath(backupsDirPath());
    const QString stamp = QDateTime::currentDateTime().toString(QStringLiteral("yyyyMMdd-HHmmss"));
    const QString backupFile = backupsDirPath() + QStringLiteral("/%1-%2.tar.gz").arg(clean, stamp);

    QString wsPath;
    QString wsArchiveFile;
    if (includeWorkspace) {
        wsPath = environmentByName(clean).value(QStringLiteral("workspacePath")).toString().trimmed();
        if (wsPath.startsWith(QLatin1String("~/")))
            wsPath = QDir::homePath() + wsPath.mid(1);
        if (!wsPath.isEmpty() && QFileInfo::exists(wsPath))
            wsArchiveFile = backupsDirPath() + QStringLiteral("/%1-%2-workspace.tar.gz").arg(clean, stamp);
    }

    runShellTask(tr("Exporting backup of '%1'").arg(clean),
                 exportEnvironmentScript(clean, backupFile, wsPath, wsArchiveFile),
                 {}, clean);
    return {};
}

QString Backend::exportAndWipeEnvironment(const QString &name, bool includeWorkspace, bool deleteWorkspaceFiles) {
    if (m_busy)
        return tr("Another operation is already running.");
    const QString clean = sanitizeName(name);
    if (clean.isEmpty() || environmentByName(clean).isEmpty())
        return tr("Environment '%1' was not found.").arg(name);

    const QVariantMap space = checkExportSpace(clean, includeWorkspace);
    if (!space.value(QStringLiteral("ok")).toBool())
        return space.value(QStringLiteral("message")).toString();

    if (m_demoMode) {
        const QString stamp = QDateTime::currentDateTime().toString(QStringLiteral("yyyyMMdd-HHmmss"));
        const QString fn = QStringLiteral("%1-%2.tar.gz").arg(clean, stamp);
        QVariantMap item;
        item["fileName"] = fn;
        item["filePath"] = backupsDirPath() + QLatin1Char('/') + fn;
        item["envName"] = clean;
        item["sizeBytes"] = space.value(QStringLiteral("estimatedBytes")).toLongLong();
        item["sizeDisplay"] = space.value(QStringLiteral("estimatedDisplay")).toString();
        item["createdAt"] = QDateTime::currentDateTime().toString(QStringLiteral("yyyy-MM-dd HH:mm"));
        item["hasWorkspaceArchive"] = includeWorkspace;
        item["workspaceArchivePath"] = includeWorkspace ? (backupsDirPath() + QStringLiteral("/%1-%2-workspace.tar.gz").arg(clean, stamp)) : QString();
        item["workspaceArchiveSizeDisplay"] = includeWorkspace ? QStringLiteral("42 MB") : QString();
        m_backupArchives.prepend(item);
        emit backupArchivesChanged();
        for (int i = 0; i < m_environments.size(); ++i) {
            if (m_environments.at(i).toMap().value(QStringLiteral("name")).toString() == clean) {
                m_environments.removeAt(i);
                break;
            }
        }
        m_count = m_environments.size();
        if (m_selectedIndex >= m_count)
            m_selectedIndex = qMax(0, m_count - 1);
        emit environmentsChanged();
        emit countChanged();
        emit selectedIndexChanged();
        refreshStaleReport();
        m_statusMessage = tr("[Demo] Exported backup '%1' and wiped '%2'.").arg(fn, clean);
        emit statusMessageChanged();
        return {};
    }

    QDir().mkpath(backupsDirPath());
    const QString stamp = QDateTime::currentDateTime().toString(QStringLiteral("yyyyMMdd-HHmmss"));
    const QString backupFile = backupsDirPath() + QStringLiteral("/%1-%2.tar.gz").arg(clean, stamp);

    QString wsPath = environmentByName(clean).value(QStringLiteral("workspacePath")).toString().trimmed();
    if (wsPath.isEmpty())
        wsPath = workspacePathFor(clean);
    if (wsPath.startsWith(QLatin1String("~/")))
        wsPath = QDir::homePath() + wsPath.mid(1);

    QString wsArchiveFile;
    if (includeWorkspace && !wsPath.isEmpty() && QFileInfo::exists(wsPath))
        wsArchiveFile = backupsDirPath() + QStringLiteral("/%1-%2-workspace.tar.gz").arg(clean, stamp);

    const QString envSessionsDir = sessionsDirPath() + QLatin1Char('/') + clean;
    stopFileAccessProcess(clean);
    runShellTask(tr("Exporting & wiping '%1'").arg(clean),
                 exportAndWipeEnvironmentScript(clean, backupFile, wsPath, wsArchiveFile,
                                                deleteWorkspaceFiles, envSessionsDir),
                 {}, clean);
    return {};
}

QString Backend::importEnvironment(const QString &backupFile, const QString &newName,
                                   const QString &workspaceArchiveFile) {
    if (m_busy)
        return tr("Another operation is already running.");
    const QString targetName = sanitizeName(newName);
    if (targetName.isEmpty())
        return tr("New environment name is invalid.");
    if (!environmentByName(targetName).isEmpty())
        return tr("Environment '%1' already exists.").arg(targetName);

    const QString cleanBackup = backupFile.trimmed();
    if (cleanBackup.isEmpty())
        return tr("Backup archive path is required.");
    if (!m_demoMode && !QFileInfo::exists(cleanBackup))
        return tr("Backup archive not found: %1").arg(cleanBackup);

    const QString cleanWsArchive = workspaceArchiveFile.trimmed();
    const QString wsPath = workspacePathFor(targetName);

    if (m_demoMode) {
        QVariantMap env;
        env["name"] = targetName;
        env["status"] = QStringLiteral("STOPPED");
        env["running"] = false;
        env["type"] = QStringLiteral("CONTAINER");
        env["osName"] = QStringLiteral("Restored Backup");
        env["preset"] = QStringLiteral("Imported");
        env["cpuLimit"] = QStringLiteral("4 Cores");
        env["cpuCores"] = 4;
        env["cpuUsage"] = QStringLiteral("0%");
        env["cpuPercent"] = 0;
        env["memoryLimit"] = QStringLiteral("8GiB");
        env["memoryUsage"] = QStringLiteral("0 MB");
        env["memoryPercent"] = 0;
        env["diskLimit"] = QStringLiteral("40GiB");
        env["diskUsage"] = QStringLiteral("2.1 GB");
        env["diskPercent"] = 5;
        env["ipv4"] = QStringLiteral("—");
        env["workspacePath"] = wsPath;
        env["snapshotCount"] = 0;
        env["snapshots"] = QStringList{};
        env["isTemplate"] = false;
        env["networkMode"] = QStringLiteral("online");
        m_environments.append(env);
        m_count = m_environments.size();
        emit countChanged();
        emit environmentsChanged();
        m_statusMessage = tr("[Demo] Imported '%1' from backup.").arg(targetName);
        emit statusMessageChanged();
        return {};
    }

    runShellTask(tr("Importing backup as '%1'").arg(targetName),
                 importEnvironmentScript(cleanBackup, targetName, wsPath, cleanWsArchive),
                 targetName, targetName);
    return {};
}

void Backend::deleteBackup(const QString &backupFile, const QString &workspaceArchiveFile) {
    if (m_demoMode) {
        QVariantList updated;
        for (const auto &v : m_backupArchives) {
            if (v.toMap().value(QStringLiteral("filePath")).toString() != backupFile)
                updated.append(v);
        }
        m_backupArchives = updated;
        emit backupArchivesChanged();
        m_statusMessage = tr("[Demo] Backup deleted.");
        emit statusMessageChanged();
        return;
    }
    if (!backupFile.trimmed().isEmpty())
        QFile::remove(backupFile.trimmed());
    if (!workspaceArchiveFile.trimmed().isEmpty())
        QFile::remove(workspaceArchiveFile.trimmed());
    refreshBackups();
    m_statusMessage = tr("Backup archive deleted.");
    emit statusMessageChanged();
}

QSet<QString> Backend::parseActiveIncusExecEnvs(const QString &pgrepOutput) {
    static const QSet<QString> kValueFlags = {
        QStringLiteral("--cwd"),
        QStringLiteral("--env"),
        QStringLiteral("--user"),
        QStringLiteral("--group"),
        QStringLiteral("--mode"),
        QStringLiteral("--project"),
        QStringLiteral("--target"),
        QStringLiteral("-e"),
        QStringLiteral("-u"),
        QStringLiteral("-g")
    };
    QSet<QString> envs;
    for (const QString &rawLine : pgrepOutput.split(QLatin1Char('\n'), Qt::SkipEmptyParts)) {
        const QString line = rawLine.trimmed();
        const int execIdx = line.indexOf(QStringLiteral("incus exec "));
        if (execIdx < 0)
            continue;
        const QString tail = line.mid(execIdx + 11).trimmed();
        const QStringList tokens = tail.split(QRegularExpression(QStringLiteral("\\s+")), Qt::SkipEmptyParts);
        for (int i = 0; i < tokens.size(); ++i) {
            const QString &tok = tokens.at(i);
            if (tok == QLatin1String("--"))
                break;
            if (tok.startsWith(QLatin1Char('-'))) {
                if (!tok.contains(QLatin1Char('=')) && kValueFlags.contains(tok))
                    ++i; // skip flag value token (e.g., --cwd /workspace)
                continue;
            }
            envs.insert(tok);
            break;
        }
    }
    return envs;
}

Backend::StalenessResult Backend::classifyEnvironmentStaleness(qint64 lastUsedEpoch, bool isTemplate,
                                                                 int staleDays, qint64 nowEpoch) {
    StalenessResult res;
    if (isTemplate) {
        res.classification = StalenessClass::Template;
        res.daysUnused = -1;
        res.statusKey = QStringLiteral("template");
        return res;
    }
    if (lastUsedEpoch <= 0) {
        res.classification = StalenessClass::Unknown;
        res.daysUnused = -1;
        res.statusKey = QStringLiteral("unknown");
        return res;
    }
    const int thresholdDays = qBound(1, staleDays, 3650);
    const qint64 refNow = (nowEpoch > 0) ? nowEpoch : QDateTime::currentSecsSinceEpoch();
    const qint64 elapsed = qMax<qint64>(0, refNow - lastUsedEpoch);
    res.daysUnused = elapsed / 86400LL;
    if (elapsed >= qint64(thresholdDays) * 86400LL) {
        res.classification = StalenessClass::Stale;
        res.statusKey = QStringLiteral("stale");
    } else {
        res.classification = StalenessClass::Fresh;
        res.statusKey = QStringLiteral("fresh");
    }
    return res;
}

QVariantList Backend::detectOrphans(const QVariantList &environments, const QString &workspaceBase,
                                    const QString &backupsDir) {
    QSet<QString> knownEnvNames;
    QSet<QString> knownWorkspacePaths;
    for (const QVariant &v : environments) {
        const QVariantMap env = v.toMap();
        const QString name = env.value(QStringLiteral("name")).toString();
        if (!name.isEmpty())
            knownEnvNames.insert(name);
        QString ws = env.value(QStringLiteral("workspacePath")).toString().trimmed();
        if (ws.startsWith(QLatin1String("~/")))
            ws = QDir::homePath() + ws.mid(1);
        if (!ws.isEmpty())
            knownWorkspacePaths.insert(QDir::cleanPath(QFileInfo(ws).absoluteFilePath()));
    }

    QVariantList orphans;
    const QString cleanWsBase = workspaceBase.trimmed().isEmpty() ? QString() : QDir::cleanPath(workspaceBase.trimmed());
    if (!cleanWsBase.isEmpty() && QDir(cleanWsBase).exists()) {
        const QDir wsDir(cleanWsBase);
        const QFileInfoList dirs = wsDir.entryInfoList(QDir::Dirs | QDir::NoDotAndDotDot | QDir::NoSymLinks, QDir::Name);
        for (const QFileInfo &fi : dirs) {
            const QString dirName = fi.fileName();
            if (dirName.startsWith(QLatin1Char('.')))
                continue;
            QString envCandidate = dirName;
            if (envCandidate.endsWith(QLatin1String(".pre-subvol")))
                envCandidate.chop(11);
            const QString absPath = QDir::cleanPath(fi.absoluteFilePath());
            if (!knownEnvNames.contains(dirName) &&
                !knownEnvNames.contains(envCandidate) &&
                !knownWorkspacePaths.contains(absPath)) {
                const qint64 bytes = directorySizeBytes(absPath);
                QVariantMap o;
                o["kind"] = QStringLiteral("workspace");
                o["kindLabel"] = tr("Orphan workspace");
                o["name"] = dirName;
                o["path"] = absPath;
                o["openPath"] = absPath;
                o["sizeBytes"] = bytes;
                o["sizeDisplay"] = formatBytes(bytes);
                orphans.append(o);
            }
        }

        const QDir snapRoot(cleanWsBase + QStringLiteral("/.snapshots"));
        if (snapRoot.exists()) {
            const QFileInfoList snapDirs = snapRoot.entryInfoList(QDir::Dirs | QDir::NoDotAndDotDot | QDir::NoSymLinks, QDir::Name);
            for (const QFileInfo &fi : snapDirs) {
                const QString envName = fi.fileName();
                if (knownEnvNames.contains(envName))
                    continue;
                const QString absPath = QDir::cleanPath(fi.absoluteFilePath());
                const qint64 bytes = directorySizeBytes(absPath);
                QVariantMap o;
                o["kind"] = QStringLiteral("workspace-snapshot");
                o["kindLabel"] = tr("Orphan ws snapshots");
                o["name"] = envName;
                o["path"] = absPath;
                o["openPath"] = absPath;
                o["sizeBytes"] = bytes;
                o["sizeDisplay"] = formatBytes(bytes);
                orphans.append(o);
            }
        }
    }

    const QString cleanBackupsDir = backupsDir.trimmed().isEmpty() ? QString() : QDir::cleanPath(backupsDir.trimmed());
    if (!cleanBackupsDir.isEmpty() && QDir(cleanBackupsDir).exists()) {
        const QDir bDir(cleanBackupsDir);
        const QFileInfoList files = bDir.entryInfoList(QStringList{QStringLiteral("*.tar.gz")}, QDir::Files, QDir::Name);
        QSet<QString> instanceBases;
        for (const QFileInfo &fi : files) {
            const QString fn = fi.fileName();
            if (!fn.endsWith(QStringLiteral("-workspace.tar.gz")))
                instanceBases.insert(fn.chopped(7));
        }
        for (const QFileInfo &fi : files) {
            const QString fn = fi.fileName();
            if (!fn.endsWith(QStringLiteral("-workspace.tar.gz")))
                continue;
            const QString base = fn.chopped(17);
            if (!instanceBases.contains(base)) {
                const QString absPath = QDir::cleanPath(fi.absoluteFilePath());
                const qint64 bytes = fi.size();
                QVariantMap o;
                o["kind"] = QStringLiteral("backup-workspace");
                o["kindLabel"] = tr("Orphan backup workspace");
                o["name"] = fn;
                o["path"] = absPath;
                o["openPath"] = cleanBackupsDir;
                o["sizeBytes"] = bytes;
                o["sizeDisplay"] = formatBytes(bytes);
                orphans.append(o);
            }
        }
    }

    return orphans;
}

QVariantMap Backend::buildStaleReport(const QVariantList &environments, const QString &workspaceBase,
                                      const QString &backupsDir, int staleDays, qint64 nowEpoch) {
    const int thresholdDays = qBound(1, staleDays, 3650);
    const qint64 refNow = (nowEpoch > 0) ? nowEpoch : QDateTime::currentSecsSinceEpoch();

    QVariantList staleEntries;
    QVariantList unknownEntries;
    int freshCount = 0;
    int templateCount = 0;
    qint64 reclaimableBytes = 0;

    for (const QVariant &v : environments) {
        const QVariantMap env = v.toMap();
        const bool isTpl = env.value(QStringLiteral("isTemplate")).toBool();
        const qint64 lastUsed = env.value(QStringLiteral("lastUsedEpoch")).toLongLong();
        const StalenessResult cls = classifyEnvironmentStaleness(lastUsed, isTpl, thresholdDays, refNow);

        if (cls.classification == StalenessClass::Template) {
            ++templateCount;
            continue;
        }
        if (cls.classification == StalenessClass::Fresh) {
            ++freshCount;
            continue;
        }

        qint64 diskBytes = env.value(QStringLiteral("diskUsageBytes")).toLongLong();
        if (diskBytes <= 0)
            diskBytes = parseSizeBytes(env.value(QStringLiteral("diskUsage")).toString());

        qint64 snapTotalBytes = env.value(QStringLiteral("snapshotTotalBytes")).toLongLong();
        if (snapTotalBytes <= 0) {
            const QVariantMap snapSizes = env.value(QStringLiteral("snapshotSizes")).toMap();
            for (auto it = snapSizes.constBegin(); it != snapSizes.constEnd(); ++it)
                snapTotalBytes += qMax<qint64>(0, parseSizeBytes(it.value().toString()));
        }

        QString wsPath = env.value(QStringLiteral("workspacePath")).toString().trimmed();
        if (wsPath.startsWith(QLatin1String("~/")))
            wsPath = QDir::homePath() + wsPath.mid(1);
        const qint64 wsBytes = wsPath.isEmpty() ? 0 : directorySizeBytes(wsPath);
        const qint64 totalBytes = qMax<qint64>(0, diskBytes) + qMax<qint64>(0, snapTotalBytes) + qMax<qint64>(0, wsBytes);
        reclaimableBytes += totalBytes;

        QVariantMap item;
        item["name"] = env.value(QStringLiteral("name")).toString();
        item["status"] = cls.statusKey;
        item["running"] = env.value(QStringLiteral("running")).toBool();
        item["type"] = env.value(QStringLiteral("type")).toString();
        item["osName"] = env.value(QStringLiteral("osName")).toString();
        item["lastUsedEpoch"] = lastUsed;
        item["daysUnused"] = cls.daysUnused;
        item["lastUsedDisplay"] = (cls.classification == StalenessClass::Stale)
                                      ? tr("%1d ago (%2)")
                                            .arg(cls.daysUnused)
                                            .arg(QDateTime::fromSecsSinceEpoch(lastUsed).toString(QStringLiteral("yyyy-MM-dd")))
                                      : tr("Unknown (never recorded)");
        item["diskUsageBytes"] = diskBytes;
        item["diskUsageDisplay"] = diskBytes > 0 ? formatBytes(diskBytes) : env.value(QStringLiteral("diskUsage")).toString();
        item["snapshotCount"] = env.value(QStringLiteral("snapshotCount")).toInt();
        item["snapshotTotalBytes"] = snapTotalBytes;
        item["snapshotTotalDisplay"] = snapTotalBytes > 0 ? formatBytes(snapTotalBytes) : QStringLiteral("—");
        item["workspacePath"] = env.value(QStringLiteral("workspacePath")).toString();
        item["workspaceSizeBytes"] = wsBytes;
        item["workspaceSizeDisplay"] = formatBytes(wsBytes);
        item["totalFootprintBytes"] = totalBytes;
        item["totalFootprintDisplay"] = formatBytes(totalBytes);

        if (cls.classification == StalenessClass::Stale)
            staleEntries.append(item);
        else
            unknownEntries.append(item);
    }

    std::sort(staleEntries.begin(), staleEntries.end(), [](const QVariant &a, const QVariant &b) {
        return a.toMap().value(QStringLiteral("daysUnused")).toLongLong() >
               b.toMap().value(QStringLiteral("daysUnused")).toLongLong();
    });

    QVariantList entries = staleEntries;
    entries.append(unknownEntries);

    const QVariantList orphans = detectOrphans(environments, workspaceBase, backupsDir);
    for (const QVariant &ov : orphans)
        reclaimableBytes += qMax<qint64>(0, ov.toMap().value(QStringLiteral("sizeBytes")).toLongLong());

    QVariantMap report;
    report["staleDays"] = thresholdDays;
    report["generatedAt"] = QDateTime::currentDateTime().toString(QStringLiteral("yyyy-MM-dd HH:mm:ss"));
    report["entries"] = entries;
    report["orphans"] = orphans;
    report["staleCount"] = staleEntries.size();
    report["unknownCount"] = unknownEntries.size();
    report["freshCount"] = freshCount;
    report["templateCount"] = templateCount;
    report["orphanCount"] = orphans.size();
    report["totalReclaimableBytes"] = reclaimableBytes;
    report["totalReclaimableDisplay"] = formatBytes(reclaimableBytes);
    return report;
}

void Backend::refreshStaleReport() {
    QVariantMap report = buildStaleReport(m_environments, m_workspaceBase, backupsDirPath(), m_staleDays);
    if (m_demoMode) {
        QVariantList entries = report.value(QStringLiteral("entries")).toList();
        for (int i = 0; i < entries.size(); ++i) {
            QVariantMap item = entries.at(i).toMap();
            if (item.value(QStringLiteral("workspaceSizeBytes")).toLongLong() <= 0) {
                const qint64 demoWs = qint64(256) * 1024LL * 1024LL;
                item["workspaceSizeBytes"] = demoWs;
                item["workspaceSizeDisplay"] = formatBytes(demoWs);
                const qint64 total = item.value(QStringLiteral("diskUsageBytes")).toLongLong() +
                                     item.value(QStringLiteral("snapshotTotalBytes")).toLongLong() + demoWs;
                item["totalFootprintBytes"] = total;
                item["totalFootprintDisplay"] = formatBytes(total);
                entries[i] = item;
            }
        }
        report["entries"] = entries;
        if (report.value(QStringLiteral("orphans")).toList().isEmpty()) {
            QVariantList demoOrphans;
            demoOrphans.append(QVariantMap{
                {"kind", QStringLiteral("workspace")},
                {"kindLabel", tr("Orphan workspace")},
                {"name", QStringLiteral("legacy-agent-poc")},
                {"path", m_workspaceBase + QStringLiteral("/legacy-agent-poc")},
                {"openPath", m_workspaceBase + QStringLiteral("/legacy-agent-poc")},
                {"sizeBytes", qint64(482344960LL)},
                {"sizeDisplay", formatBytes(482344960LL)}
            });
            demoOrphans.append(QVariantMap{
                {"kind", QStringLiteral("backup-workspace")},
                {"kindLabel", tr("Orphan backup workspace")},
                {"name", QStringLiteral("old-bot-20260918-112233-workspace.tar.gz")},
                {"path", backupsDirPath() + QStringLiteral("/old-bot-20260918-112233-workspace.tar.gz")},
                {"openPath", backupsDirPath()},
                {"sizeBytes", qint64(134217728LL)},
                {"sizeDisplay", formatBytes(134217728LL)}
            });
            report["orphans"] = demoOrphans;
            report["orphanCount"] = demoOrphans.size();
        }
    }
    m_staleReport = report;
    emit staleReportChanged();
}

void Backend::openOrphanFolder(const QString &path) {
    const QString clean = QDir::cleanPath(path.trimmed());
    if (clean.isEmpty())
        return;
    const QFileInfo fi(clean);
    const QString target = fi.isDir() ? fi.absoluteFilePath() : fi.absolutePath();
    QDesktopServices::openUrl(QUrl::fromLocalFile(target));
}

QString Backend::deleteOrphanScript(const QString &kind, const QString &targetPath) {
    const QString cleanTarget = QDir::cleanPath(targetPath.trimmed());
    const QString qTarget = shellQuote(cleanTarget);
    if (kind == QLatin1String("backup-workspace")) {
        return QStringLiteral(
            "set -euo pipefail\n"
            "echo '@@STAGE:40:Removing orphan backup archive...'\n"
            "rm -f %1\n"
            "echo '@@STAGE:100:Orphan removed.'").arg(qTarget);
    }
    if (kind == QLatin1String("workspace-snapshot")) {
        return QStringLiteral(
            "set -euo pipefail\n"
            "echo '@@STAGE:35:Removing orphan workspace snapshots...'\n"
            "if [ -d %1 ]; then\n"
            "    for S in %1/*; do\n"
            "        [ -e \"$S\" ] || continue\n"
            "        if ! btrfs subvolume delete \"$S\" >/dev/null 2>&1; then\n"
            "            btrfs property set -ts \"$S\" ro false >/dev/null 2>&1 || true\n"
            "            rm -rf \"$S\" >/dev/null 2>&1 || true\n"
            "        fi\n"
            "    done\n"
            "    rmdir %1 >/dev/null 2>&1 || rm -rf %1 >/dev/null 2>&1 || true\n"
            "    if [ -e %1 ] && command -v sudo >/dev/null 2>&1 && sudo -n true >/dev/null 2>&1; then\n"
            "        echo '@@STAGE:70:Retrying removal with admin rights...'\n"
            "        sudo -n rm -rf %1 >/dev/null 2>&1 || true\n"
            "    fi\n"
            "fi\n"
            "if [ -e %1 ]; then\n"
            "    echo 'Could not completely remove orphan workspace snapshots (root-owned files or Btrfs subvolumes).' >&2\n"
            "    echo \"Fix (admin): sudo rm -rf %1\" >&2\n"
            "    exit 1\n"
            "fi\n"
            "echo '@@STAGE:100:Orphan workspace snapshots removed.'").arg(qTarget);
    }
    return QStringLiteral(
        "set -euo pipefail\n"
        "echo '@@STAGE:35:Removing orphan workspace folder...'\n"
        "%1\n"
        "rm -rf %2 >/dev/null 2>&1 || true\n"
        "if [ -e %2 ] && command -v sudo >/dev/null 2>&1 && sudo -n true >/dev/null 2>&1; then\n"
        "    echo '@@STAGE:70:Retrying removal with admin rights...'\n"
        "    sudo -n rm -rf %2 >/dev/null 2>&1 || true\n"
        "fi\n"
        "if [ -e %2 ]; then\n"
        "    echo 'Could not completely remove orphan workspace folder (root-owned files or Btrfs subvolumes).' >&2\n"
        "    echo \"Fix (admin): sudo rm -rf %2\" >&2\n"
        "    exit 1\n"
        "fi\n"
        "echo '@@STAGE:100:Orphan workspace removed.'")
        .arg(workspaceDeleteSnapshotSnippet(cleanTarget, true), qTarget);
}

QString Backend::adminDeleteScriptText(const QString &kind, const QString &targetPath) {
    const QString cleanTarget = QDir::cleanPath(targetPath.trimmed());
    const QString qTarget = shellQuote(cleanTarget);
    if (kind == QLatin1String("backup-workspace")) {
        return QStringLiteral("#!/bin/sh\nset -u\nrm -f %1\n").arg(qTarget);
    }
    return QStringLiteral(
        "#!/bin/sh\n"
        "set -u\n"
        "TARGET=%1\n"
        "rm -rf \"$TARGET\" 2>/dev/null || true\n"
        "if [ -e \"$TARGET\" ]; then\n"
        "    find \"$TARGET\" -depth -type d -exec sh -c 'btrfs subvolume show \"$1\" >/dev/null 2>&1 && "
        "btrfs subvolume delete \"$1\" >/dev/null 2>&1' sh {} \\; 2>/dev/null || true\n"
        "    rm -rf \"$TARGET\" 2>/dev/null || true\n"
        "fi\n"
        "if [ -e \"$TARGET\" ]; then\n"
        "    echo \"Failed to remove: $TARGET\" >&2\n"
        "    exit 1\n"
        "fi\n"
        "echo \"Removed: $TARGET\"\n")
        .arg(qTarget);
}

QString Backend::validateOrphanPath(const QString &kind, const QString &cleanPath) const {
    const QFileInfo fi(cleanPath);
    const QString parentDir = QDir::cleanPath(fi.dir().absolutePath());
    const QString cleanWsBase = QDir::cleanPath(m_workspaceBase);
    const QString cleanSnapRoot = QDir::cleanPath(m_workspaceBase + QStringLiteral("/.snapshots"));
    const QString cleanBackups = QDir::cleanPath(backupsDirPath());

    if (kind == QLatin1String("workspace") &&
        (parentDir != cleanWsBase || fi.fileName() == QLatin1String(".snapshots") ||
         fi.fileName().startsWith(QLatin1Char('.')))) {
        return tr("Refusing to delete path outside workspace directory: %1").arg(cleanPath);
    }
    if (kind == QLatin1String("workspace-snapshot") && parentDir != cleanSnapRoot) {
        return tr("Refusing to delete path outside snapshot root: %1").arg(cleanPath);
    }
    if (kind == QLatin1String("backup-workspace") &&
        (parentDir != cleanBackups || !fi.fileName().endsWith(QStringLiteral("-workspace.tar.gz")))) {
        return tr("Refusing to delete non-orphan archive outside backups directory: %1").arg(cleanPath);
    }
    return {};
}

QString Backend::deleteOrphanAsAdmin(const QString &kind, const QString &path) {
    const QString clean = QDir::cleanPath(path.trimmed());
    if (clean.isEmpty())
        return tr("Orphan path is empty.");

    if (kind != QLatin1String("workspace") &&
        kind != QLatin1String("workspace-snapshot") &&
        kind != QLatin1String("backup-workspace")) {
        return tr("Unknown orphan kind '%1'.").arg(kind);
    }
    const QString pathErr = validateOrphanPath(kind, clean);
    if (!pathErr.isEmpty())
        return pathErr;
    if (m_demoMode)
        return deleteOrphan(kind, clean);
    if (!QFileInfo::exists(clean))
        return tr("Orphan path '%1' no longer exists.").arg(clean);
    if (QStandardPaths::findExecutable(QStringLiteral("sudo")).isEmpty())
        return tr("sudo is not available on this system.");

    const QString scriptPath = m_stateDir + QStringLiteral("/admin-delete.sh");
    QDir().mkpath(m_stateDir);
    QFile f(scriptPath);
    if (!f.open(QIODevice::WriteOnly | QIODevice::Truncate))
        return tr("Could not write admin delete script to %1.").arg(scriptPath);
    f.write(adminDeleteScriptText(kind, clean).toUtf8());
    f.close();
    QFile::setPermissions(scriptPath, QFileDevice::ReadOwner | QFileDevice::WriteOwner
                                           | QFileDevice::ExeOwner);

    const QString cmd = QStringLiteral(
        "%1; RC=$?; echo; if [ $RC -eq 0 ]; then echo 'AIBox: orphan removed.'; "
        "else echo 'AIBox: orphan removal failed.'; fi; "
        "echo 'Refresh the stale report to update this list.'; "
        "echo 'Press Enter to close...'; read -r _")
        .arg(QStringLiteral("sudo -- sh %1").arg(shellQuote(scriptPath)));
    launchCommandInTerminal(cmd);
    m_statusMessage = tr("Admin delete opened in terminal for '%1'.").arg(QFileInfo(clean).fileName());
    emit statusMessageChanged();
    return {};
}

QString Backend::deleteOrphan(const QString &kind, const QString &path) {
    const QString clean = QDir::cleanPath(path.trimmed());
    if (clean.isEmpty())
        return tr("Orphan path is empty.");

    if (kind != QLatin1String("workspace") &&
        kind != QLatin1String("workspace-snapshot") &&
        kind != QLatin1String("backup-workspace")) {
        return tr("Unknown orphan kind '%1'.").arg(kind);
    }
    const QString pathErr = validateOrphanPath(kind, clean);
    if (!pathErr.isEmpty())
        return pathErr;

    const QFileInfo fi(clean);

    if (m_demoMode) {
        QVariantList orphans = m_staleReport.value(QStringLiteral("orphans")).toList();
        for (int i = 0; i < orphans.size(); ++i) {
            if (orphans.at(i).toMap().value(QStringLiteral("path")).toString() == clean) {
                orphans.removeAt(i);
                break;
            }
        }
        m_staleReport["orphans"] = orphans;
        m_staleReport["orphanCount"] = orphans.size();
        emit staleReportChanged();
        m_statusMessage = tr("[Demo] Removed orphan '%1'.").arg(fi.fileName());
        emit statusMessageChanged();
        return {};
    }

    if (!fi.exists())
        return tr("Orphan path '%1' no longer exists.").arg(clean);

    if (kind == QLatin1String("backup-workspace")) {
        if (!QFile::remove(clean))
            return tr("Could not delete orphan archive '%1'.").arg(clean);
        refreshBackups();
        refreshStaleReport();
        m_statusMessage = tr("Removed orphan archive '%1'.").arg(fi.fileName());
        emit statusMessageChanged();
        return {};
    }

    if (m_busy)
        return tr("Another operation is currently running.");

    runShellTask(tr("Removing orphan '%1'").arg(fi.fileName()),
                 deleteOrphanScript(kind, clean));
    return {};
}
