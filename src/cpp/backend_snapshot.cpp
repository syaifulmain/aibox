#include "backend.h"
#include "backend_helpers.h"
#include "rust_bridge.h"
#include "aibox-ffi/src/snapshot.rs.h"

#include <QDateTime>
#include <QDir>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonObject>

QVariantMap Backend::parseSnapshotSizesJson(const QByteArray &json) {
    return aibox::ffi::fromRustJson(
               aibox::ffi::parse_snapshot_sizes_json(aibox::ffi::toRustBytes(json)))
        .toMap();
}

QVariantMap Backend::parseDuBytesOutput(const QString &output) {
    return aibox::ffi::fromRustJson(
               aibox::ffi::parse_du_bytes_output_json(aibox::ffi::toRust(output)))
        .toMap();
}

void Backend::applyCachedSnapshotSizes(QVariantList &envs) const {
    for (int i = 0; i < envs.size(); ++i) {
        QVariantMap env = envs.at(i).toMap();
        const QString name = env.value("name").toString();
        bool changed = false;
        const QVariantMap cached = m_snapshotSizesCache.value(name);
        if (!cached.isEmpty()) {
            const QByteArray existingJson =
                QJsonDocument(QJsonObject::fromVariantMap(env.value("snapshotSizes").toMap()))
                    .toJson(QJsonDocument::Compact);
            const QByteArray cachedJson =
                QJsonDocument(QJsonObject::fromVariantMap(cached)).toJson(QJsonDocument::Compact);
            env["snapshotSizes"] = aibox::ffi::fromRustJson(
                aibox::ffi::merge_snapshot_size_map_json(
                    rust::Str(existingJson.constData(), static_cast<std::size_t>(existingJson.size())),
                    rust::Str(cachedJson.constData(), static_cast<std::size_t>(cachedJson.size()))))
                .toMap();
            changed = true;
        }
        const QVariantMap wsCached = m_wsSnapshotSizesCache.value(name);
        if (!wsCached.isEmpty()) {
            const QByteArray existingJson =
                QJsonDocument(QJsonObject::fromVariantMap(env.value("workspaceSnapshotSizes").toMap()))
                    .toJson(QJsonDocument::Compact);
            const QByteArray cachedJson =
                QJsonDocument(QJsonObject::fromVariantMap(wsCached)).toJson(QJsonDocument::Compact);
            env["workspaceSnapshotSizes"] = aibox::ffi::fromRustJson(
                aibox::ffi::merge_snapshot_size_map_json(
                    rust::Str(existingJson.constData(), static_cast<std::size_t>(existingJson.size())),
                    rust::Str(cachedJson.constData(), static_cast<std::size_t>(cachedJson.size()))))
                .toMap();
            changed = true;
        }
        if (changed)
            envs[i] = env;
    }
}

void Backend::refreshSelectedSnapshotSizes() {
    if (m_demoMode || m_environments.isEmpty())
        return;
    const QVariantMap env = selectedEnvironment();
    const QString name = env.value("name").toString();
    if (name.isEmpty())
        return;

    const QStringList snaps = env.value("snapshots").toStringList();
    if (!snaps.isEmpty() &&
        !(m_snapshotSizeKeyCache.value(name) == snaps && m_snapshotSizesCache.contains(name)) &&
        !(m_snapshotSizeProcess && m_snapshotSizeProcess->state() != QProcess::NotRunning)) {
        if (!m_snapshotSizeProcess) {
            m_snapshotSizeProcess = new QProcess(this);
            connect(m_snapshotSizeProcess, &QProcess::finished, this, [this](int code, QProcess::ExitStatus) {
                const QString targetName = m_snapshotSizeProcess->property("envName").toString();
                const QStringList targetSnaps = m_snapshotSizeProcess->property("snaps").toStringList();
                if (code != 0 || targetName.isEmpty())
                    return;
                const QVariantMap parsed = parseSnapshotSizesJson(m_snapshotSizeProcess->readAllStandardOutput());
                m_snapshotSizesCache.insert(targetName, parsed);
                m_snapshotSizeKeyCache.insert(targetName, targetSnaps);
                QVariantList updated = m_environments;
                applyCachedSnapshotSizes(updated);
                if (updated != m_environments) {
                    m_environments = updated;
                    emit environmentsChanged();
                }
            });
        }
        m_snapshotSizeProcess->setProperty("envName", name);
        m_snapshotSizeProcess->setProperty("snaps", snaps);
        m_snapshotSizeProcess->start(QStringLiteral("incus"),
                                     {QStringLiteral("query"),
                                      QStringLiteral("/1.0/instances/%1/snapshots?recursion=1").arg(name)});
    }

    const QStringList wsSnaps = env.value("workspaceSnapshots").toStringList();
    const QString wsSnapDir = env.value("workspaceSnapshotDir").toString();
    if (!wsSnaps.isEmpty() && !wsSnapDir.isEmpty() &&
        !(m_wsSnapshotSizeKeyCache.value(name) == wsSnaps && m_wsSnapshotSizesCache.contains(name)) &&
        !(m_wsSnapshotSizeProcess && m_wsSnapshotSizeProcess->state() != QProcess::NotRunning)) {
        if (!m_wsSnapshotSizeProcess) {
            m_wsSnapshotSizeProcess = new QProcess(this);
            connect(m_wsSnapshotSizeProcess, &QProcess::finished, this, [this](int code, QProcess::ExitStatus) {
                const QString targetName = m_wsSnapshotSizeProcess->property("envName").toString();
                const QStringList targetSnaps = m_wsSnapshotSizeProcess->property("wsSnaps").toStringList();
                if (code != 0 || targetName.isEmpty())
                    return;
                const QVariantMap parsed = parseDuBytesOutput(
                    QString::fromUtf8(m_wsSnapshotSizeProcess->readAllStandardOutput()));
                m_wsSnapshotSizesCache.insert(targetName, parsed);
                m_wsSnapshotSizeKeyCache.insert(targetName, targetSnaps);
                QVariantList updated = m_environments;
                applyCachedSnapshotSizes(updated);
                if (updated != m_environments) {
                    m_environments = updated;
                    emit environmentsChanged();
                }
            });
        }
        QStringList args{QStringLiteral("-sb")};
        for (const QString &s : wsSnaps)
            args.append(wsSnapDir + QLatin1Char('/') + s);
        m_wsSnapshotSizeProcess->setProperty("envName", name);
        m_wsSnapshotSizeProcess->setProperty("wsSnaps", wsSnaps);
        m_wsSnapshotSizeProcess->start(QStringLiteral("du"), args);
    }
}

bool Backend::isAutoSnapshotName(const QString &snapshotName) {
    return aibox::ffi::is_auto_snapshot_name(aibox::ffi::toRust(snapshotName));
}

QDateTime Backend::parseAutoSnapshotTimestamp(const QString &snapshotName) {
    const QString iso = aibox::ffi::fromRust(
        aibox::ffi::parse_auto_snapshot_timestamp_iso(aibox::ffi::toRust(snapshotName)));
    if (iso.isEmpty())
        return {};
    return QDateTime::fromString(iso, Qt::ISODate);
}

bool Backend::shouldCreateAutoSnapshot(const QStringList &snapshots, const QDateTime &now, int minIntervalSeconds) {
    const rust::Vec<rust::String> vec = aibox::ffi::toRustVec(snapshots);
    return aibox::ffi::should_create_auto_snapshot(
        rust::Slice<const rust::String>(vec.data(), vec.size()),
        now.toSecsSinceEpoch(),
        minIntervalSeconds);
}

QStringList Backend::autoSnapshotsToPrune(const QStringList &snapshots, int keepCount) {
    const rust::Vec<rust::String> vec = aibox::ffi::toRustVec(snapshots);
    return aibox::ffi::fromRustVec(aibox::ffi::auto_snapshots_to_prune(
        rust::Slice<const rust::String>(vec.data(), vec.size()),
        keepCount));
}

QString Backend::workspaceSnapshotDir(const QString &workspacePath, const QString &envName) {
    return aibox::ffi::fromRust(aibox::ffi::workspace_snapshot_dir(
        aibox::ffi::toRust(workspacePath),
        aibox::ffi::toRust(envName)));
}

bool Backend::isBtrfsFilesystem(const QString &path) {
    return aibox::ffi::is_btrfs_filesystem(aibox::ffi::toRust(path));
}

bool Backend::isBtrfsSubvolume(const QString &path) {
    return aibox::ffi::is_btrfs_subvolume(aibox::ffi::toRust(path));
}

QStringList Backend::listWorkspaceSnapshots(const QString &workspacePath, const QString &envName) {
    return aibox::ffi::fromRustVec(aibox::ffi::list_workspace_snapshots(
        aibox::ffi::toRust(workspacePath),
        aibox::ffi::toRust(envName)));
}

QStringList Backend::findOrphanWorkspaceSnapshots(const QStringList &incusSnapshots,
                                                  const QStringList &wsSnapshots) {
    const rust::Vec<rust::String> incusVec = aibox::ffi::toRustVec(incusSnapshots);
    const rust::Vec<rust::String> wsVec = aibox::ffi::toRustVec(wsSnapshots);
    return aibox::ffi::fromRustVec(aibox::ffi::find_orphan_workspace_snapshots(
        rust::Slice<const rust::String>(incusVec.data(), incusVec.size()),
        rust::Slice<const rust::String>(wsVec.data(), wsVec.size())));
}

QString Backend::workspaceCreateSnippet(const QString &workspacePath) {
    return aibox::ffi::fromRust(
        aibox::ffi::workspace_create_snippet(aibox::ffi::toRust(workspacePath)));
}

QString Backend::workspaceMountMigrationScript(const QString &envName) {
    return aibox::ffi::fromRust(
        aibox::ffi::workspace_mount_migration_script(aibox::ffi::toRust(envName)));
}

// Reads the live workspace mount path inside an environment so legacy
// environments that still mount at /workspace keep working until migrated.
QString Backend::containerWorkspacePathFor(const QString &name) const {
    return aibox::ffi::fromRust(
        aibox::ffi::container_workspace_path_for(aibox::ffi::toRust(name), m_demoMode));
}

QString Backend::workspaceSnapshotSnippet(const QString &workspacePath, const QString &snapDir,
                                          const QString &snapName) {
    return aibox::ffi::fromRust(aibox::ffi::workspace_snapshot_snippet(
        aibox::ffi::toRust(workspacePath),
        aibox::ffi::toRust(snapDir),
        aibox::ffi::toRust(snapName)));
}

QString Backend::workspaceDeleteSnapshotSnippet(const QString &snapPath, bool warnOnly) {
    return aibox::ffi::fromRust(
        aibox::ffi::workspace_delete_snapshot_snippet(aibox::ffi::toRust(snapPath), warnOnly));
}

QString Backend::workspaceRestoreSnippet(const QString &workspacePath, const QString &snapDir,
                                         const QString &snapName, qint64 safetyEpoch) {
    return aibox::ffi::fromRust(aibox::ffi::workspace_restore_snippet(
        aibox::ffi::toRust(workspacePath),
        aibox::ffi::toRust(snapDir),
        aibox::ffi::toRust(snapName),
        safetyEpoch));
}

QString Backend::convertWorkspaceToSubvolumeScript(const QString &envName, const QString &workspacePath) {
    return aibox::ffi::fromRust(aibox::ffi::convert_workspace_to_subvolume_script(
        aibox::ffi::toRust(envName),
        aibox::ffi::toRust(workspacePath)));
}

QString Backend::autoSnapshotShellSnippet(const QString &name, const QString &newSnapName,
                                          const QStringList &pruneNames,
                                          const QString &workspacePath,
                                          bool includeWorkspace) {
    const rust::Vec<rust::String> pruneVec = aibox::ffi::toRustVec(pruneNames);
    return aibox::ffi::fromRust(aibox::ffi::auto_snapshot_shell_snippet(
        aibox::ffi::toRust(name),
        aibox::ffi::toRust(newSnapName),
        rust::Slice<const rust::String>(pruneVec.data(), pruneVec.size()),
        aibox::ffi::toRust(workspacePath),
        includeWorkspace));
}

QString Backend::buildAutoSnapshotIfNeeded(const QString &name) {
    const QVariantMap env = environmentByName(name);
    if (env.isEmpty())
        return {};

    const QDateTime now = QDateTime::currentDateTime();
    const QDateTime lastAttempt = m_lastAutoSnapAttempt.value(name);
    const rust::Vec<rust::String> snapsVec = aibox::ffi::toRustVec(env.value("snapshots").toStringList());
    const QString snippet = aibox::ffi::fromRust(aibox::ffi::decide_auto_snapshot(
        aibox::ffi::toRust(name),
        env.value("isTemplate").toBool(),
        env.value("autoSnapshotEnabled").toBool(),
        rust::Slice<const rust::String>(snapsVec.data(), snapsVec.size()),
        aibox::ffi::toRust(env.value("workspacePath").toString()),
        env.value("workspaceSnapshotEnabled").toBool(),
        m_autoSnapshotKeep,
        lastAttempt.isValid(),
        lastAttempt.isValid() ? lastAttempt.toSecsSinceEpoch() : 0,
        now.toSecsSinceEpoch(),
        aibox::ffi::toRust(now.toString(QStringLiteral("yyyyMMdd-HHmmss")))));
    if (!snippet.isEmpty())
        m_lastAutoSnapAttempt.insert(name, now);
    return snippet;
}

void Backend::setEnvironmentAutoSnapshot(const QString &name, bool enabled) {
    if (name.isEmpty() || m_busy)
        return;
    const QString script = QStringLiteral(
        "set -euo pipefail\necho '@@STAGE:50:Updating auto-snapshot setting...'\n"
        "incus config set %1 user.aibox.autosnap %2\n"
        "echo '@@STAGE:100:Auto-snapshot updated.'")
                               .arg(shellQuote(name), enabled ? QStringLiteral("true") : QStringLiteral("false"));
    runShellTask(tr("Updating auto-snapshot for '%1'").arg(name), script);
}

QString Backend::createSnapshotScript(const QString &name, const QString &snapName,
                                      const QString &workspacePath, bool includeWorkspace) {
    return aibox::ffi::fromRust(aibox::ffi::create_snapshot_script(
        aibox::ffi::toRust(name),
        aibox::ffi::toRust(snapName),
        aibox::ffi::toRust(workspacePath),
        includeWorkspace));
}

QString Backend::restoreSnapshotScript(const QString &name, const QString &snapName,
                                       const QString &workspacePath, const QString &mode,
                                       bool includeWorkspace, qint64 safetyEpoch) {
    return aibox::ffi::fromRust(aibox::ffi::restore_snapshot_script(
        aibox::ffi::toRust(name),
        aibox::ffi::toRust(snapName),
        aibox::ffi::toRust(workspacePath),
        aibox::ffi::toRust(mode),
        includeWorkspace,
        safetyEpoch));
}

QString Backend::deleteSnapshotScript(const QString &name, const QString &snapName,
                                      const QString &workspacePath, bool hasIncusSnap,
                                      bool hasWorkspaceSnap) {
    return aibox::ffi::fromRust(aibox::ffi::delete_snapshot_script(
        aibox::ffi::toRust(name),
        aibox::ffi::toRust(snapName),
        aibox::ffi::toRust(workspacePath),
        hasIncusSnap,
        hasWorkspaceSnap));
}

QString Backend::wipeEnvironmentScript(const QString &name, const QString &workspacePath,
                                       bool deleteWorkspaceFiles,
                                       const QString &sessionsEnvDir,
                                       const QString &environmentSecretsPath) {
    return aibox::ffi::fromRust(aibox::ffi::wipe_environment_script(
        aibox::ffi::toRust(name),
        aibox::ffi::toRust(workspacePath),
        deleteWorkspaceFiles,
        aibox::ffi::toRust(sessionsEnvDir),
        aibox::ffi::toRust(environmentSecretsPath)));
}

void Backend::setEnvironmentWorkspaceSnapshot(const QString &name, bool enabled) {
    if (name.isEmpty() || m_busy)
        return;
    const QString script = QStringLiteral(
        "set -euo pipefail\necho '@@STAGE:50:Updating workspace snapshot setting...'\n"
        "incus config set %1 user.aibox.wssnap %2\n"
        "echo '@@STAGE:100:Workspace snapshot setting updated.'")
                               .arg(shellQuote(name), enabled ? QStringLiteral("true") : QStringLiteral("false"));
    runShellTask(tr("Updating workspace snapshots for '%1'").arg(name), script, {}, name);
}

void Backend::convertWorkspaceToSubvolume(const QString &name) {
    if (name.isEmpty() || m_busy)
        return;
    const QVariantMap env = environmentByName(name);
    if (env.isEmpty())
        return;
    if (env.value("running").toBool()) {
        setError(tr("Stop '%1' before converting its workspace to a Btrfs subvolume.").arg(name));
        return;
    }
    const QString wsPath = workspacePathFor(name);
    runShellTask(tr("Converting workspace of '%1' to Btrfs subvolume").arg(name),
                 convertWorkspaceToSubvolumeScript(name, wsPath), {}, name);
}

void Backend::deletePreSubvolBackup(const QString &name) {
    if (name.isEmpty() || m_busy)
        return;
    const QString prePath = QDir::cleanPath(workspacePathFor(name)) + QStringLiteral(".pre-subvol");
    if (!QFileInfo::exists(prePath))
        return;
    const QString script = QStringLiteral(
        "set -euo pipefail\n"
        "echo '@@STAGE:40:Removing pre-subvolume workspace backup...'\n"
        "rm -rf %1\n"
        "echo '@@STAGE:100:Backup removed.'").arg(shellQuote(prePath));
    runShellTask(tr("Removing pre-subvolume backup for '%1'").arg(name), script, {}, name);
}

void Backend::createSnapshot(const QString &name, const QString &snapshotName) {
    if (name.isEmpty() || m_busy)
        return;
    QString snap = sanitizeName(snapshotName);
    if (snap.isEmpty())
        snap = QStringLiteral("snap-") + QDateTime::currentDateTime().toString(QStringLiteral("yyyyMMdd-HHmmss"));

    const QVariantMap env = environmentByName(name);
    const QString wsPath = env.value("workspacePath").toString().isEmpty()
                               ? workspacePathFor(name)
                               : env.value("workspacePath").toString();
    const bool includeWs = env.value("workspaceSnapshotEnabled").toBool();
    runShellTask(tr("Saving snapshot '%1' for '%2'").arg(snap, name),
                 createSnapshotScript(name, snap, wsPath, includeWs), {}, name);
}

void Backend::restoreSnapshot(const QString &name, const QString &snapshotName, const QString &mode) {
    if (name.isEmpty() || snapshotName.isEmpty() || m_busy)
        return;
    const QVariantMap env = environmentByName(name);
    const QString wsPath = env.value("workspacePath").toString().isEmpty()
                               ? workspacePathFor(name)
                               : env.value("workspacePath").toString();
    const bool hasWsSnap = env.value("workspaceSnapshots").toStringList().contains(snapshotName);
    if (mode.trimmed().toLower() == QLatin1String("workspace") && !hasWsSnap) {
        setError(tr("Workspace snapshot '%1' was not found.").arg(snapshotName));
        return;
    }
    runShellTask(tr("Restoring '%1' to snapshot '%2'").arg(name, snapshotName),
                 restoreSnapshotScript(name, snapshotName, wsPath, mode, hasWsSnap), {}, name);
}

void Backend::deleteSnapshot(const QString &name, const QString &snapshotName) {
    if (name.isEmpty() || snapshotName.isEmpty() || m_busy)
        return;
    const QVariantMap env = environmentByName(name);
    const QString wsPath = env.value("workspacePath").toString().isEmpty()
                               ? workspacePathFor(name)
                               : env.value("workspacePath").toString();
    const bool hasIncus = env.value("snapshots").toStringList().contains(snapshotName);
    const bool hasWs = env.value("workspaceSnapshots").toStringList().contains(snapshotName);
    runShellTask(tr("Deleting snapshot '%1'").arg(snapshotName),
                 deleteSnapshotScript(name, snapshotName, wsPath, hasIncus || !hasWs, hasWs), {}, name);
}

void Backend::wipeEnvironment(const QString &name, bool deleteWorkspaceFiles) {
    if (name.isEmpty() || m_busy)
        return;
    const QString hostWorkspace = workspacePathFor(name);
    const QString envSessionsDir = sessionsDirPath() + QLatin1Char('/') + sanitizeName(name);
    stopFileAccessProcess(name);
    runShellTask(tr("Wiping environment '%1'").arg(name),
                 wipeEnvironmentScript(name, hostWorkspace, deleteWorkspaceFiles, envSessionsDir,
                                       environmentSecretsStorePath(name)),
                 {}, name);
}
