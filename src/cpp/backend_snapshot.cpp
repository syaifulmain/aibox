#include "backend.h"
#include "backend_helpers.h"

#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QStorageInfo>

#include <algorithm>
#include <sys/stat.h>

QVariantMap Backend::parseSnapshotSizesJson(const QByteArray &json) {
    QVariantMap sizes;
    if (json.trimmed().isEmpty())
        return sizes;
    QJsonParseError err;
    const QJsonDocument doc = QJsonDocument::fromJson(json, &err);
    if (err.error != QJsonParseError::NoError || !doc.isArray())
        return sizes;

    const QJsonArray arr = doc.array();
    for (const QJsonValue &val : arr) {
        const QJsonObject obj = val.toObject();
        const QString snapName = obj.value("name").toString();
        const qint64 snapBytes = obj.value("size").toInteger(-1);
        if (!snapName.isEmpty() && snapBytes > 0)
            sizes.insert(snapName, formatBytes(snapBytes));
    }
    return sizes;
}

QVariantMap Backend::parseDuBytesOutput(const QString &output) {
    QVariantMap sizes;
    const QStringList lines = output.split(QLatin1Char('\n'), Qt::SkipEmptyParts);
    for (const QString &line : lines) {
        const int tabIdx = line.indexOf(QLatin1Char('\t'));
        if (tabIdx <= 0)
            continue;
        bool ok = false;
        const qint64 bytes = line.left(tabIdx).trimmed().toLongLong(&ok);
        const QString path = line.mid(tabIdx + 1).trimmed();
        const QString name = QFileInfo(path).fileName();
        if (ok && bytes >= 0 && !name.isEmpty())
            sizes.insert(name, formatBytes(bytes));
    }
    return sizes;
}

void Backend::applyCachedSnapshotSizes(QVariantList &envs) const {
    for (int i = 0; i < envs.size(); ++i) {
        QVariantMap env = envs.at(i).toMap();
        const QString name = env.value("name").toString();
        bool changed = false;
        const QVariantMap cached = m_snapshotSizesCache.value(name);
        if (!cached.isEmpty()) {
            QVariantMap merged = env.value("snapshotSizes").toMap();
            for (auto it = cached.constBegin(); it != cached.constEnd(); ++it)
                merged.insert(it.key(), it.value());
            env["snapshotSizes"] = merged;
            changed = true;
        }
        const QVariantMap wsCached = m_wsSnapshotSizesCache.value(name);
        if (!wsCached.isEmpty()) {
            QVariantMap merged = env.value("workspaceSnapshotSizes").toMap();
            for (auto it = wsCached.constBegin(); it != wsCached.constEnd(); ++it)
                merged.insert(it.key(), it.value());
            env["workspaceSnapshotSizes"] = merged;
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
    return snapshotName.startsWith(QLatin1String("auto-"));
}

QDateTime Backend::parseAutoSnapshotTimestamp(const QString &snapshotName) {
    if (!isAutoSnapshotName(snapshotName))
        return {};
    const QString ts = snapshotName.mid(5);
    return QDateTime::fromString(ts, QStringLiteral("yyyyMMdd-HHmmss"));
}

bool Backend::shouldCreateAutoSnapshot(const QStringList &snapshots, const QDateTime &now, int minIntervalSeconds) {
    QDateTime newest;
    for (const QString &snap : snapshots) {
        const QDateTime dt = parseAutoSnapshotTimestamp(snap);
        if (dt.isValid() && (!newest.isValid() || dt > newest))
            newest = dt;
    }
    if (!newest.isValid())
        return true;
    return newest.secsTo(now) >= qMax(0, minIntervalSeconds);
}

QStringList Backend::autoSnapshotsToPrune(const QStringList &snapshots, int keepCount) {
    struct Entry {
        QString name;
        qint64 epoch = 0;
    };
    QVector<Entry> autos;
    for (const QString &snap : snapshots) {
        if (!isAutoSnapshotName(snap))
            continue;
        const QDateTime dt = parseAutoSnapshotTimestamp(snap);
        autos.append({snap, dt.isValid() ? dt.toSecsSinceEpoch() : 0});
    }
    const int keep = qMax(0, keepCount);
    if (autos.size() <= keep)
        return {};

    std::sort(autos.begin(), autos.end(), [](const Entry &a, const Entry &b) {
        if (a.epoch != b.epoch)
            return a.epoch < b.epoch;
        return a.name < b.name;
    });

    QStringList toDelete;
    const int dropCount = autos.size() - keep;
    for (int i = 0; i < dropCount; ++i)
        toDelete.append(autos.at(i).name);
    return toDelete;
}

QString Backend::workspaceSnapshotDir(const QString &workspacePath, const QString &envName) {
    const QString cleanWs = QDir::cleanPath(workspacePath.trimmed());
    const QString cleanEnv = envName.trimmed();
    if (cleanWs.isEmpty() || cleanEnv.isEmpty())
        return {};
    const QString parent = QFileInfo(cleanWs).dir().path();
    return QDir::cleanPath(parent + QStringLiteral("/.snapshots/") + cleanEnv);
}

bool Backend::isBtrfsFilesystem(const QString &path) {
    QString cur = QDir::cleanPath(path.trimmed());
    if (cur.isEmpty())
        return false;
    for (int i = 0; i < 16 && !cur.isEmpty(); ++i) {
        if (QFileInfo::exists(cur))
            return QStorageInfo(cur).fileSystemType() == QByteArrayLiteral("btrfs");
        const QString parent = QFileInfo(cur).dir().path();
        if (parent == cur)
            break;
        cur = parent;
    }
    return false;
}

bool Backend::isBtrfsSubvolume(const QString &path) {
    const QString clean = QDir::cleanPath(path.trimmed());
    if (clean.isEmpty() || !QFileInfo::exists(clean) || !isBtrfsFilesystem(clean))
        return false;
    struct stat st {};
    if (::lstat(QFile::encodeName(clean).constData(), &st) != 0)
        return false;
    return S_ISDIR(st.st_mode) && st.st_ino == 256;
}

QStringList Backend::listWorkspaceSnapshots(const QString &workspacePath, const QString &envName) {
    const QString snapDir = workspaceSnapshotDir(workspacePath, envName);
    if (snapDir.isEmpty())
        return {};
    const QDir dir(snapDir);
    if (!dir.exists())
        return {};
    return dir.entryList(QDir::Dirs | QDir::NoDotAndDotDot, QDir::Name);
}

QStringList Backend::findOrphanWorkspaceSnapshots(const QStringList &incusSnapshots,
                                                  const QStringList &wsSnapshots) {
    QStringList orphans;
    for (const QString &wsSnap : wsSnapshots) {
        if (!incusSnapshots.contains(wsSnap))
            orphans.append(wsSnap);
    }
    return orphans;
}

QString Backend::workspaceCreateSnippet(const QString &workspacePath) {
    const QString qWs = shellQuote(QDir::cleanPath(workspacePath.trimmed()));
    return QStringLiteral(
        "mkdir -p \"$(dirname %1)\"\n"
        "if [ ! -e %1 ]; then\n"
        "    if command -v btrfs >/dev/null 2>&1 && "
        "[[ \"$(stat -f -c %%T \"$(dirname %1)\" 2>/dev/null || true)\" == \"btrfs\" ]]; then\n"
        "        btrfs subvolume create %1 >/dev/null\n"
        "    else\n"
        "        mkdir -p %1\n"
        "    fi\n"
        "fi")
        .arg(qWs);
}

QString Backend::workspaceMountMigrationScript(const QString &envName) {
    const QString qName = shellQuote(envName);
    const QString qPath = shellQuote(kContainerWorkspacePath);
    return QStringLiteral(
        "WS_MOUNT=$(incus config device get %1 workspace path 2>/dev/null || true)\n"
        "if [ -n \"$WS_MOUNT\" ] && [ \"$WS_MOUNT\" != %2 ]; then\n"
        "    if incus config device set %1 workspace path=%2 >/dev/null 2>&1; then\n"
        "        echo \"Workspace mount moved: $WS_MOUNT -> %2\"\n"
        "    fi\n"
        "fi")
        .arg(qName, qPath);
}

// Reads the live workspace mount path inside an environment so legacy
// environments that still mount at /workspace keep working until migrated.
QString Backend::containerWorkspacePathFor(const QString &name) const {
    if (m_demoMode || name.trimmed().isEmpty())
        return containerWorkspacePath();
    QProcess proc;
    proc.start(QStringLiteral("incus"),
               {QStringLiteral("config"), QStringLiteral("device"), QStringLiteral("get"), name,
                QStringLiteral("workspace"), QStringLiteral("path")});
    if (!proc.waitForStarted(3000))
        return containerWorkspacePath();
    if (!proc.waitForFinished(5000)) {
        proc.kill();
        proc.waitForFinished(1000);
        return containerWorkspacePath();
    }
    const QString out = QString::fromUtf8(proc.readAllStandardOutput()).trimmed();
    if (proc.exitCode() == 0 && out.startsWith(QLatin1Char('/')))
        return out;
    return containerWorkspacePath();
}

QString Backend::workspaceSnapshotSnippet(const QString &workspacePath, const QString &snapDir,
                                          const QString &snapName) {
    const QString cleanWs = QDir::cleanPath(workspacePath.trimmed());
    const QString cleanDir = QDir::cleanPath(snapDir.trimmed());
    const QString cleanSnap = snapName.trimmed();
    if (cleanWs.isEmpty() || cleanDir.isEmpty() || cleanSnap.isEmpty())
        return {};
    const QString qWs = shellQuote(cleanWs);
    const QString qDir = shellQuote(cleanDir);
    const QString qDst = shellQuote(cleanDir + QLatin1Char('/') + cleanSnap);
    return QStringLiteral(
        "if command -v btrfs >/dev/null 2>&1 && [ -d %1 ] && "
        "[[ \"$(stat -f -c %%T %1 2>/dev/null || true)\" == \"btrfs\" ]] && "
        "[[ \"$(stat -c %%i %1 2>/dev/null || true)\" == \"256\" ]]; then\n"
        "    mkdir -p %2\n"
        "    if [ ! -e %3 ]; then\n"
        "        btrfs subvolume snapshot -r %1 %3 >/dev/null\n"
        "    fi\n"
        "fi")
        .arg(qWs, qDir, qDst);
}

QString Backend::workspaceDeleteSnapshotSnippet(const QString &snapPath, bool warnOnly) {
    const QString cleanSnap = QDir::cleanPath(snapPath.trimmed());
    if (cleanSnap.isEmpty())
        return {};
    const QString qSnap = shellQuote(cleanSnap);
    return QStringLiteral(
        "if [ -e %1 ]; then\n"
        "    if ! btrfs subvolume delete %1 >/dev/null 2>&1; then\n"
        "        if command -v btrfs >/dev/null 2>&1 && "
        "[[ \"$(stat -f -c %%T %1 2>/dev/null || true)\" == \"btrfs\" ]] && "
        "[[ \"$(stat -c %%i %1 2>/dev/null || true)\" == \"256\" ]]; then\n"
        "            btrfs property set -ts %1 ro false >/dev/null 2>&1 || true\n"
        "        fi\n"
        "        rm -rf %1 >/dev/null 2>&1 || true\n"
        "    fi\n"
        "    if [ -e %1 ] && command -v sudo >/dev/null 2>&1 && sudo -n true >/dev/null 2>&1; then\n"
        "        sudo -n rm -rf %1 >/dev/null 2>&1 || true\n"
        "    fi\n"
        "    if [ -e %1 ]; then\n"
        "        echo \"Warning: could not delete subvolume or root-owned files; run: sudo rm -rf %1\" >&2\n"
        "%2"
        "    fi\n"
        "fi")
        .arg(qSnap, warnOnly ? QString() : QStringLiteral("        exit 1\n"));
}

QString Backend::workspaceRestoreSnippet(const QString &workspacePath, const QString &snapDir,
                                         const QString &snapName, qint64 safetyEpoch) {
    const QString cleanWs = QDir::cleanPath(workspacePath.trimmed());
    const QString cleanDir = QDir::cleanPath(snapDir.trimmed());
    const QString cleanSnap = snapName.trimmed();
    if (cleanWs.isEmpty() || cleanDir.isEmpty() || cleanSnap.isEmpty())
        return {};
    const QString qWs = shellQuote(cleanWs);
    const QString qDir = shellQuote(cleanDir);
    const QString qSnap = shellQuote(cleanDir + QLatin1Char('/') + cleanSnap);
    const QString qTmp = shellQuote(cleanWs + QStringLiteral(".restore-tmp"));
    const QString qOld = shellQuote(cleanWs + QStringLiteral(".restore-old"));
    const QString epochExpr = safetyEpoch > 0 ? QString::number(safetyEpoch) : QStringLiteral("$(date +%s)");
    const QString safetyPathExpr = QStringLiteral("%1\"/pre-restore-%2\"").arg(shellQuote(cleanDir), epochExpr);

    return QStringLiteral(
        "if [ ! -d %1 ]; then\n"
        "    echo \"Workspace snapshot %1 does not exist.\" >&2\n"
        "    exit 1\n"
        "fi\n"
        "if ! command -v btrfs >/dev/null 2>&1; then\n"
        "    echo \"btrfs tool is required for workspace snapshot restore.\" >&2\n"
        "    exit 1\n"
        "fi\n"
        "mkdir -p %2\n"
        "if [ -d %3 ] && [[ \"$(stat -f -c %%T %3 2>/dev/null || true)\" == \"btrfs\" ]] && "
        "[[ \"$(stat -c %%i %3 2>/dev/null || true)\" == \"256\" ]]; then\n"
        "    SAFETY_SNAP=%4\n"
        "    [ -e \"$SAFETY_SNAP\" ] || btrfs subvolume snapshot -r %3 \"$SAFETY_SNAP\" >/dev/null\n"
        "fi\n"
        "if [ -e %5 ]; then\n"
        "    %6\n"
        "fi\n"
        "btrfs subvolume snapshot %1 %5 >/dev/null\n"
        "if [ -e %3 ]; then\n"
        "    mv %3 %7\n"
        "fi\n"
        "if ! mv %5 %3; then\n"
        "    [ -e %7 ] && mv %7 %3 || true\n"
        "    exit 1\n"
        "fi\n"
        "%8")
        .arg(qSnap,
             qDir,
             qWs,
             safetyPathExpr,
             qTmp,
             workspaceDeleteSnapshotSnippet(cleanWs + QStringLiteral(".restore-tmp")),
             qOld,
             workspaceDeleteSnapshotSnippet(cleanWs + QStringLiteral(".restore-old")));
}

QString Backend::convertWorkspaceToSubvolumeScript(const QString &envName, const QString &workspacePath) {
    const QString cleanWs = QDir::cleanPath(workspacePath.trimmed());
    const QString qName = shellQuote(envName);
    const QString qWs = shellQuote(cleanWs);
    const QString qTmp = shellQuote(cleanWs + QStringLiteral(".subvol-tmp"));
    const QString qPre = shellQuote(cleanWs + QStringLiteral(".pre-subvol"));
    return QStringLiteral(
        "set -euo pipefail\n"
        "echo '@@STAGE:10:Checking container & Btrfs status...'\n"
        "STATUS=$(incus info %1 2>/dev/null | awk '/^Status:/ {print $2}')\n"
        "if [[ \"$STATUS\" == \"RUNNING\" ]]; then\n"
        "    echo \"Stop '%1' before converting its workspace to a Btrfs subvolume.\" >&2\n"
        "    exit 1\n"
        "fi\n"
        "if ! command -v btrfs >/dev/null 2>&1; then\n"
        "    echo \"btrfs command not found.\" >&2\n"
        "    exit 1\n"
        "fi\n"
        "mkdir -p \"$(dirname %2)\"\n"
        "if [[ \"$(stat -f -c %%T \"$(dirname %2)\" 2>/dev/null || true)\" != \"btrfs\" ]]; then\n"
        "    echo \"Workspace filesystem is not Btrfs.\" >&2\n"
        "    exit 1\n"
        "fi\n"
        "if [ -d %2 ] && [[ \"$(stat -c %%i %2 2>/dev/null || true)\" == \"256\" ]]; then\n"
        "    echo '@@STAGE:100:Workspace is already a Btrfs subvolume.'\n"
        "    exit 0\n"
        "fi\n"
        "if [ -e %4 ]; then\n"
        "    echo \"Backup directory %4 already exists. Remove or rename it first.\" >&2\n"
        "    exit 1\n"
        "fi\n"
        "echo '@@STAGE:30:Creating Btrfs subvolume...'\n"
        "if [ -e %3 ]; then rm -rf %3; fi\n"
        "btrfs subvolume create %3 >/dev/null\n"
        "if [ -d %2 ]; then\n"
        "    echo '@@STAGE:55:Copying workspace files with reflinks...'\n"
        "    cp -a --reflink=always %2/. %3/ 2>/dev/null || cp -a %2/. %3/\n"
        "fi\n"
        "if command -v setfacl >/dev/null 2>&1; then\n"
        "    ACL_USER=\"${SUDO_USER:-${USER:-$(id -un)}}\"\n"
        "    setfacl -R -m \"u:${ACL_USER}:rwx\" -m \"m::rwx\" -d -m \"u:${ACL_USER}:rwx\" -d -m \"m::rwx\" %3 2>/dev/null || true\n"
        "fi\n"
        "echo '@@STAGE:85:Swapping workspace into place...'\n"
        "if [ -d %2 ]; then\n"
        "    mv %2 %4\n"
        "fi\n"
        "if ! mv %3 %2; then\n"
        "    [ -e %4 ] && mv %4 %2 || true\n"
        "    exit 1\n"
        "fi\n"
        "echo '@@STAGE:100:Workspace converted to Btrfs subvolume.'")
        .arg(qName, qWs, qTmp, qPre);
}

QString Backend::autoSnapshotShellSnippet(const QString &name, const QString &newSnapName,
                                          const QStringList &pruneNames,
                                          const QString &workspacePath,
                                          bool includeWorkspace) {
    if (name.isEmpty() || newSnapName.isEmpty())
        return {};
    QStringList lines;
    lines << QStringLiteral("incus snapshot create %1 %2 >/dev/null 2>&1 || true")
                 .arg(shellQuote(name), shellQuote(newSnapName));
    const QString snapDir = includeWorkspace ? workspaceSnapshotDir(workspacePath, name) : QString();
    if (includeWorkspace && !snapDir.isEmpty()) {
        const QString wsSnap = workspaceSnapshotSnippet(workspacePath, snapDir, newSnapName);
        if (!wsSnap.isEmpty()) {
            lines << QStringLiteral("(\n%1\n) || echo %2 >&2")
                         .arg(wsSnap, shellQuote(QStringLiteral("Warning: workspace auto-snapshot failed for %1/%2").arg(name, newSnapName)));
        }
    }
    for (const QString &oldSnap : pruneNames) {
        lines << QStringLiteral("incus snapshot delete %1 %2 >/dev/null 2>&1 || true")
                     .arg(shellQuote(name), shellQuote(oldSnap));
        if (includeWorkspace && !snapDir.isEmpty()) {
            lines << workspaceDeleteSnapshotSnippet(snapDir + QLatin1Char('/') + oldSnap);
        }
    }
    return lines.join(QLatin1Char('\n'));
}

QString Backend::buildAutoSnapshotIfNeeded(const QString &name) {
    const QVariantMap env = environmentByName(name);
    if (env.isEmpty() || env.value("isTemplate").toBool())
        return {};
    if (!env.value("autoSnapshotEnabled").toBool())
        return {};

    const QDateTime now = QDateTime::currentDateTime();
    const QDateTime lastAttempt = m_lastAutoSnapAttempt.value(name);
    if (lastAttempt.isValid() && lastAttempt.secsTo(now) < 300)
        return {};

    const QStringList snapshots = env.value("snapshots").toStringList();
    if (!shouldCreateAutoSnapshot(snapshots, now, 300))
        return {};

    QString newSnap = QStringLiteral("auto-") + now.toString(QStringLiteral("yyyyMMdd-HHmmss"));
    if (snapshots.contains(newSnap))
        return {};

    QStringList allSnapshots = snapshots;
    allSnapshots.append(newSnap);
    const QStringList pruneNames = autoSnapshotsToPrune(allSnapshots, m_autoSnapshotKeep);
    m_lastAutoSnapAttempt.insert(name, now);
    const QString wsPath = env.value("workspacePath").toString();
    const bool includeWs = env.value("workspaceSnapshotEnabled").toBool();
    return autoSnapshotShellSnippet(name, newSnap, pruneNames, wsPath, includeWs);
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
    const QString qName = shellQuote(name);
    const QString qSnap = shellQuote(snapName);
    QString script = QStringLiteral(
        "set -euo pipefail\n"
        "echo '@@STAGE:40:Creating OS container snapshot...'\n"
        "incus snapshot create %1 %2\n").arg(qName, qSnap);
    const QString snapDir = includeWorkspace ? workspaceSnapshotDir(workspacePath, name) : QString();
    if (includeWorkspace && !snapDir.isEmpty()) {
        const QString wsSnap = workspaceSnapshotSnippet(workspacePath, snapDir, snapName);
        if (!wsSnap.isEmpty()) {
            script += QStringLiteral(
                "echo '@@STAGE:75:Creating workspace Btrfs snapshot...'\n"
                "(\n%1\n) || echo %2 >&2\n")
                          .arg(wsSnap, shellQuote(QStringLiteral("Warning: workspace snapshot failed for %1/%2").arg(name, snapName)));
        }
    }
    script += QStringLiteral("echo '@@STAGE:100:Saved.'\n");
    return script;
}

QString Backend::restoreSnapshotScript(const QString &name, const QString &snapName,
                                       const QString &workspacePath, const QString &mode,
                                       bool includeWorkspace, qint64 safetyEpoch) {
    const QString normalized = mode.trimmed().toLower();
    const bool doSystem = (normalized != QLatin1String("workspace"));
    const bool doWorkspace = (normalized == QLatin1String("workspace")) ||
                             (normalized != QLatin1String("system") && includeWorkspace);
    const QString qName = shellQuote(name);
    const QString qSnap = shellQuote(snapName);

    QString script = QStringLiteral("set -euo pipefail\n");
    if (doWorkspace) {
        const QString snapDir = workspaceSnapshotDir(workspacePath, name);
        if (snapDir.isEmpty()) {
            script += QStringLiteral("echo \"Workspace path is unavailable.\" >&2\nexit 1\n");
            return script;
        }
        script += QStringLiteral(
            "echo '@@STAGE:15:Checking environment state...'\n"
            "STATUS=$(incus info %1 2>/dev/null | awk '/^Status:/ {print $2}')\n"
            "if [[ \"$STATUS\" == \"RUNNING\" ]]; then\n"
            "    echo '@@STAGE:30:Stopping environment before workspace swap...'\n"
            "    incus stop %1\n"
            "fi\n").arg(qName);
        if (doSystem) {
            script += QStringLiteral(
                "echo '@@STAGE:55:Restoring OS snapshot...'\n"
                "incus snapshot restore %1 %2\n").arg(qName, qSnap);
        }
        script += QStringLiteral("echo '@@STAGE:78:Restoring workspace snapshot (with pre-restore safety snapshot)...'\n");
        script += workspaceRestoreSnippet(workspacePath, snapDir, snapName, safetyEpoch) + QLatin1Char('\n');
    } else if (doSystem) {
        script += QStringLiteral(
            "echo '@@STAGE:50:Restoring OS snapshot...'\n"
            "incus snapshot restore %1 %2\n").arg(qName, qSnap);
    }
    script += QStringLiteral("echo '@@STAGE:100:Restored.'\n");
    return script;
}

QString Backend::deleteSnapshotScript(const QString &name, const QString &snapName,
                                      const QString &workspacePath, bool hasIncusSnap,
                                      bool hasWorkspaceSnap) {
    QString script = QStringLiteral("set -euo pipefail\n");
    if (hasIncusSnap) {
        script += QStringLiteral(
            "echo '@@STAGE:40:Deleting OS snapshot...'\n"
            "incus snapshot delete %1 %2\n").arg(shellQuote(name), shellQuote(snapName));
    }
    if (hasWorkspaceSnap) {
        const QString snapDir = workspaceSnapshotDir(workspacePath, name);
        if (!snapDir.isEmpty()) {
            script += QStringLiteral("echo '@@STAGE:75:Deleting workspace snapshot...'\n");
            script += workspaceDeleteSnapshotSnippet(snapDir + QLatin1Char('/') + snapName, false) + QLatin1Char('\n');
        }
    }
    script += QStringLiteral("echo '@@STAGE:100:Deleted.'\n");
    return script;
}

QString Backend::wipeEnvironmentScript(const QString &name, const QString &workspacePath,
                                       bool deleteWorkspaceFiles,
                                       const QString &sessionsEnvDir) {
    QString script = QStringLiteral(
        "echo '@@STAGE:40:Wiping OS container...'; "
        "incus delete %1 --force; "
        "incus network acl delete %2 >/dev/null 2>&1 || true; "
        "incus network acl delete %3 >/dev/null 2>&1 || true")
                         .arg(shellQuote(name),
                              shellQuote(QStringLiteral("aibox-") + name),
                              shellQuote(QStringLiteral("aibox-log-") + name));
    if (deleteWorkspaceFiles && !workspacePath.trimmed().isEmpty()) {
        const QString cleanWs = QDir::cleanPath(workspacePath.trimmed());
        const QString snapDir = workspaceSnapshotDir(cleanWs, name);
        script += QStringLiteral("\necho '@@STAGE:75:Removing workspace and workspace snapshots...'\n");
        if (!snapDir.isEmpty()) {
            const QString qSnapDir = shellQuote(snapDir);
            script += QStringLiteral(
                "if [ -d %1 ]; then\n"
                "    for S in %1/*; do\n"
                "        [ -e \"$S\" ] || continue\n"
                "        if ! btrfs subvolume delete \"$S\" >/dev/null 2>&1; then\n"
                "            btrfs property set -ts \"$S\" ro false >/dev/null 2>&1 || true\n"
                "            rm -rf \"$S\" >/dev/null 2>&1 || true\n"
                "        fi\n"
                "    done\n"
                "    rmdir %1 2>/dev/null || rm -rf %1 2>/dev/null || true\n"
                "fi\n").arg(qSnapDir);
        }
        script += workspaceDeleteSnapshotSnippet(cleanWs, true) + QLatin1Char('\n');
        script += workspaceDeleteSnapshotSnippet(cleanWs + QStringLiteral(".pre-subvol"), true) + QLatin1Char('\n');
    }
    if (!sessionsEnvDir.trimmed().isEmpty()) {
        script += QStringLiteral("\nrm -rf %1 >/dev/null 2>&1 || true\n")
                      .arg(shellQuote(QDir::cleanPath(sessionsEnvDir.trimmed())));
    }
    script += QStringLiteral("\necho '@@STAGE:100:Wiped.'");
    return script;
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
                 wipeEnvironmentScript(name, hostWorkspace, deleteWorkspaceFiles, envSessionsDir), {}, name);
}
