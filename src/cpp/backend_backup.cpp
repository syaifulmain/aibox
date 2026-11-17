#include "backend.h"
#include "backend_helpers.h"
#include "rust_bridge.h"
#include "aibox-ffi/src/backup.rs.h"

#include <QDateTime>
#include <QDesktopServices>
#include <QDir>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QSet>
#include <QStandardPaths>
#include <QUrl>

using aibox::ffi::fromRust;
using aibox::ffi::fromRustJson;
using aibox::ffi::fromRustVec;
using aibox::ffi::toRust;

namespace {
rust::String variantListToJson(const QVariantList &list) {
    const QByteArray bytes = QJsonDocument(QJsonArray::fromVariantList(list)).toJson(QJsonDocument::Compact);
    return rust::String(bytes.constData(), static_cast<std::size_t>(bytes.size()));
}
} // namespace

QString Backend::backupsDirPath() const {
    return fromRust(aibox::ffi::backup_backups_dir_path(toRust(m_osStoragePath)));
}

bool Backend::hasSufficientDiskSpace(qint64 requiredBytes, qint64 availableBytes) {
    return aibox::ffi::backup_has_sufficient_disk_space(requiredBytes, availableBytes);
}

qint64 Backend::directorySizeBytes(const QString &dirPath) {
    return aibox::ffi::backup_directory_size_bytes(toRust(dirPath));
}

qint64 Backend::availableDiskBytes(const QString &path) {
    return aibox::ffi::backup_available_disk_bytes(toRust(path));
}

QVariantList Backend::scanBackupArchives(const QString &backupsDir) {
    return fromRustJson(aibox::ffi::backup_scan_archives_json(toRust(backupsDir))).toList();
}

QString Backend::exportEnvironmentScript(const QString &name, const QString &backupFile,
                                         const QString &workspacePath,
                                         const QString &workspaceArchiveFile) {
    return fromRust(aibox::ffi::backup_export_environment_script(
        toRust(name), toRust(backupFile), toRust(workspacePath), toRust(workspaceArchiveFile)));
}

QString Backend::exportAndWipeEnvironmentScript(const QString &name, const QString &backupFile,
                                                const QString &workspacePath,
                                                const QString &workspaceArchiveFile,
                                                bool deleteWorkspaceFiles,
                                                const QString &sessionsEnvDir,
                                                const QString &environmentSecretsPath) {
    return fromRust(aibox::ffi::backup_export_and_wipe_environment_script(
        toRust(name), toRust(backupFile), toRust(workspacePath), toRust(workspaceArchiveFile),
        deleteWorkspaceFiles, toRust(sessionsEnvDir), toRust(environmentSecretsPath)));
}

QString Backend::importEnvironmentScript(const QString &backupFile, const QString &targetName,
                                         const QString &workspacePath,
                                         const QString &workspaceArchiveFile) {
    return fromRust(aibox::ffi::backup_import_environment_script(
        toRust(backupFile), toRust(targetName), toRust(workspacePath), toRust(workspaceArchiveFile)));
}

void Backend::refreshBackups() {
    if (m_demoMode)
        return;
    m_backupArchives = scanBackupArchives(backupsDirPath());
    emit backupArchivesChanged();
}

QVariantMap Backend::checkExportSpace(const QString &name, bool includeWorkspace) const {
    const QVariantMap env = environmentByName(name);
    return fromRustJson(aibox::ffi::backup_check_export_space_json(
                            env.value(QStringLiteral("diskUsageBytes")).toLongLong(),
                            toRust(env.value(QStringLiteral("diskUsage")).toString()),
                            toRust(env.value(QStringLiteral("workspacePath")).toString()),
                            includeWorkspace, m_demoMode, toRust(backupsDirPath())))
        .toMap();
}

QString Backend::exportEnvironment(const QString &name, bool includeWorkspace) {
    rust::String cleanRust;
    const QString err = fromRust(aibox::ffi::backup_validate_export_request(
        m_busy, toRust(name), !environmentByName(sanitizeName(name)).isEmpty(), cleanRust));
    if (!err.isEmpty())
        return err;
    const QString clean = fromRust(cleanRust);

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
    rust::String cleanRust;
    const QString err = fromRust(aibox::ffi::backup_validate_export_request(
        m_busy, toRust(name), !environmentByName(sanitizeName(name)).isEmpty(), cleanRust));
    if (!err.isEmpty())
        return err;
    const QString clean = fromRust(cleanRust);

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
                                                deleteWorkspaceFiles, envSessionsDir,
                                                environmentSecretsStorePath(clean)),
                 {}, clean);
    return {};
}

QString Backend::importEnvironment(const QString &backupFile, const QString &newName,
                                   const QString &workspaceArchiveFile) {
    const auto val = aibox::ffi::backup_validate_import_request(
        m_busy, toRust(newName), !environmentByName(sanitizeName(newName)).isEmpty(),
        toRust(backupFile), toRust(workspaceArchiveFile), m_demoMode);
    if (!val.error.empty())
        return fromRust(val.error);

    const QString targetName = fromRust(val.target_name);
    const QString cleanBackup = fromRust(val.clean_backup);
    const QString cleanWsArchive = fromRust(val.clean_ws_archive);
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
    aibox::ffi::backup_delete_files(toRust(backupFile), toRust(workspaceArchiveFile));
    refreshBackups();
    m_statusMessage = tr("Backup archive deleted.");
    emit statusMessageChanged();
}

QSet<QString> Backend::parseActiveIncusExecEnvs(const QString &pgrepOutput) {
    const QStringList list = fromRustVec(
        aibox::ffi::health_parse_active_incus_exec_envs(toRust(pgrepOutput)));
    return QSet<QString>(list.begin(), list.end());
}

Backend::StalenessResult Backend::classifyEnvironmentStaleness(qint64 lastUsedEpoch, bool isTemplate,
                                                                 int staleDays, qint64 nowEpoch) {
    const auto r = aibox::ffi::backup_classify_environment_staleness(
        lastUsedEpoch, isTemplate, staleDays, nowEpoch);
    StalenessResult res;
    res.classification = static_cast<StalenessClass>(r.classification);
    res.daysUnused = r.days_unused;
    res.statusKey = fromRust(r.status_key);
    return res;
}

QVariantList Backend::detectOrphans(const QVariantList &environments, const QString &workspaceBase,
                                    const QString &backupsDir) {
    return fromRustJson(aibox::ffi::backup_detect_orphans_json(
                            variantListToJson(environments), toRust(workspaceBase), toRust(backupsDir)))
        .toList();
}

QVariantMap Backend::buildStaleReport(const QVariantList &environments, const QString &workspaceBase,
                                      const QString &backupsDir, int staleDays, qint64 nowEpoch) {
    return fromRustJson(aibox::ffi::backup_build_stale_report_json(
                            variantListToJson(environments), toRust(workspaceBase),
                            toRust(backupsDir), staleDays, nowEpoch))
        .toMap();
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
    return fromRust(aibox::ffi::backup_delete_orphan_script(toRust(kind), toRust(targetPath)));
}

QString Backend::adminDeleteScriptText(const QString &kind, const QString &targetPath) {
    return fromRust(aibox::ffi::backup_admin_delete_script_text(toRust(kind), toRust(targetPath)));
}

QString Backend::validateOrphanPath(const QString &kind, const QString &cleanPath) const {
    return fromRust(aibox::ffi::backup_validate_orphan_path(
        toRust(kind), toRust(cleanPath), toRust(m_workspaceBase), toRust(backupsDirPath())));
}

QString Backend::deleteOrphanAsAdmin(const QString &kind, const QString &path) {
    if (m_demoMode) {
        const auto val = aibox::ffi::backup_validate_orphan_request(
            toRust(kind), toRust(path), toRust(m_workspaceBase), toRust(backupsDirPath()));
        if (!val.error.empty())
            return fromRust(val.error);
        return deleteOrphan(kind, fromRust(val.clean_path));
    }

    const bool hasSudo = !QStandardPaths::findExecutable(QStringLiteral("sudo")).isEmpty();
    const auto prep = aibox::ffi::backup_prepare_delete_orphan_as_admin(
        toRust(kind), toRust(path), toRust(m_workspaceBase), toRust(backupsDirPath()),
        toRust(m_stateDir), hasSudo);
    if (!prep.error.empty())
        return fromRust(prep.error);

    launchCommandInTerminal(fromRust(prep.command));
    m_statusMessage = tr("Admin delete opened in terminal for '%1'.").arg(fromRust(prep.file_name));
    emit statusMessageChanged();
    return {};
}

QString Backend::deleteOrphan(const QString &kind, const QString &path) {
    const auto val = aibox::ffi::backup_validate_orphan_request(
        toRust(kind), toRust(path), toRust(m_workspaceBase), toRust(backupsDirPath()));
    if (!val.error.empty())
        return fromRust(val.error);
    const QString clean = fromRust(val.clean_path);
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
