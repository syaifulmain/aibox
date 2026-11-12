#include "backend.h"

#include <QClipboard>
#include <QDateTime>
#include <QDesktopServices>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QRegularExpression>
#include <QStandardPaths>
#include <QUrl>
#include <csignal>
#include <unistd.h>

namespace {

struct OsEntry {
    const char *id;
    const char *alias;
    const char *name;
    const char *badge;
    const char *description;
};

constexpr OsEntry kOsEntries[] = {
    {"ubuntu-24.04", "ubuntu/24.04", "Ubuntu 24.04 LTS (Noble)", "Recommended",
     "Best compatibility for AI tools, Desktop (Electron/Qt/Flutter), and Android SDK."},
    {"ubuntu-22.04", "ubuntu/22.04", "Ubuntu 22.04 LTS (Jammy)", "LTS",
     "Proven long-term support release for legacy C++, Python, and mobile toolchains."},
    {"debian-12", "debian/12", "Debian 12 (Bookworm)", "Lightweight",
     "Rock-solid stability with low RAM overhead (~60 MB idle). Ideal for backend & databases."},
    {"archlinux", "archlinux", "Arch Linux", "Rolling",
     "Bleeding-edge packages matching Omarchy. Great for native C++, Rust, and Qt6 builds."},
    {"fedora-41", "fedora/41", "Fedora 41", "Modern",
     "Up-to-date workstation & server stack with modern GCC, Podman/Docker, and Wayland."},
    {"alpine-3.21", "alpine/3.21", "Alpine Linux 3.21", "Ultra-Light",
     "Featherweight (~5 MB) musl/BusyBox system. Boots in 0.5s for rapid CLI/microservice tests."},
    {"kali", "kali", "Kali Linux Rolling", "Security",
     "Pre-configured distribution for security auditing and penetration testing projects."},
    {"rockylinux-9", "rockylinux/9", "Rocky Linux 9", "Enterprise",
     "100% bug-for-bug RHEL 9 compatible environment for production server testing."}
};

QString formatBytes(qint64 bytes) {
    if (bytes <= 0)
        return QStringLiteral("0 MB");
    const double mb = static_cast<double>(bytes) / (1024.0 * 1024.0);
    if (mb >= 1024.0)
        return QStringLiteral("%1 GB").arg(mb / 1024.0, 0, 'f', 1);
    return QStringLiteral("%1 MB").arg(mb, 0, 'f', 0);
}

QString findOsAlias(const QString &osId) {
    for (const auto &entry : kOsEntries) {
        if (osId == QLatin1String(entry.id))
            return QString::fromLatin1(entry.alias);
    }
    return QStringLiteral("ubuntu/24.04");
}

} // namespace

Backend::Backend(const QString &workspaceDir, QObject *parent)
    : QObject(parent),
      m_workspaceBase(workspaceDir.isEmpty() ? QDir::homePath() + "/ai-workspaces" : workspaceDir),
      m_stateDir(workspaceDir.isEmpty() ? QDir::homePath() + "/.cache/aibox" : workspaceDir + "/.state"),
      m_demoMode(qEnvironmentVariableIntValue("AIBOX_DEMO_MODE") == 1) {
    m_osCatalog = buildOsCatalog({});

    if (m_demoMode) {
        loadDemoData();
        return;
    }

    QDir().mkpath(m_workspaceBase);
    QDir().mkpath(m_stateDir);

    m_taskWatchTimer.setInterval(250);
    connect(&m_taskWatchTimer, &QTimer::timeout, this, &Backend::checkBackgroundTask);

    m_pollTimer.setInterval(4000);
    connect(&m_pollTimer, &QTimer::timeout, this, [this] {
        if (!m_busy)
            refresh();
    });

    if (!QStandardPaths::findExecutable("incus").isEmpty() &&
        QGuiApplication::platformName() != QLatin1String("offscreen")) {
        attachExistingTask();
        QTimer::singleShot(100, this, &Backend::refresh);
        m_pollTimer.start();
    }
}

Backend::~Backend() {
    // Note: We intentionally do NOT kill detached background tasks when the GUI window closes,
    // so environment creation or OS downloads continue safely in the background and re-attach
    // when the user reopens AIBox.
    if (m_refreshProcess && m_refreshProcess->state() != QProcess::NotRunning) {
        m_refreshProcess->kill();
        m_refreshProcess->waitForFinished(500);
    }
}

void Backend::primaryAction() {
    ++m_count;
    emit countChanged();
    if (!m_environments.isEmpty() && m_selectedIndex >= 0 && m_selectedIndex < m_environments.size()) {
        const auto env = m_environments.at(m_selectedIndex).toMap();
        enterEnvironment(env.value("name").toString());
    }
}

void Backend::setSelectedIndex(int index) {
    if (index == m_selectedIndex)
        return;
    m_selectedIndex = index;
    emit selectedIndexChanged();
}

QVariantMap Backend::selectedEnvironment() const {
    if (m_selectedIndex >= 0 && m_selectedIndex < m_environments.size())
        return m_environments.at(m_selectedIndex).toMap();
    return {};
}

void Backend::clearError() {
    if (!m_errorMessage.isEmpty()) {
        m_errorMessage.clear();
        emit errorMessageChanged();
    }
}

void Backend::setBusy(bool busy, const QString &status) {
    if (m_busy != busy) {
        m_busy = busy;
        emit busyChanged();
    }
    if (m_statusMessage != status) {
        m_statusMessage = status;
        emit statusMessageChanged();
    }
    if (!busy) {
        m_taskProgress = 0;
        m_taskStage.clear();
        m_taskLog.clear();
        emit taskProgressChanged();
    }
}

void Backend::setError(const QString &error) {
    m_errorMessage = error.trimmed();
    emit errorMessageChanged();
}

QString Backend::sanitizeName(const QString &rawName) {
    QString s = rawName.trimmed().toLower();
    s.replace(QRegularExpression(QStringLiteral("[^a-z0-9-]+")), QStringLiteral("-"));
    s.replace(QRegularExpression(QStringLiteral("-+")), QStringLiteral("-"));
    s.remove(QRegularExpression(QStringLiteral("^-+|-+$")));
    return s;
}

QVariantList Backend::parseIncusListJson(const QByteArray &json, const QString &workspaceBase) {
    QVariantList result;
    const auto doc = QJsonDocument::fromJson(json);
    if (!doc.isArray())
        return result;

    for (const auto &val : doc.array()) {
        const QJsonObject obj = val.toObject();
        const QString name = obj.value("name").toString();
        if (name.isEmpty())
            continue;

        const QString status = obj.value("status").toString().toUpper();
        const bool running = (status == QLatin1String("RUNNING"));
        const QString type = obj.value("type").toString().toUpper();

        const QJsonObject config = obj.value("config").toObject();
        QString osName = config.value("image.description").toString();
        if (osName.isEmpty()) {
            const QString os = config.value("image.os").toString();
            const QString rel = config.value("image.release").toString();
            osName = (!os.isEmpty() ? os + " " + rel : QStringLiteral("Linux Environment")).trimmed();
        }
        const int parenIdx = osName.indexOf('(');
        if (parenIdx > 0)
            osName = osName.left(parenIdx).trimmed();

        const QString cpuLimit = config.value("limits.cpu").toString(QStringLiteral("All"));
        const QString memoryLimit = config.value("limits.memory").toString(QStringLiteral("Dynamic"));

        const QJsonObject expandedDevices = obj.value("expanded_devices").toObject();
        const QJsonObject devices = obj.value("devices").toObject();
        QString diskLimit = devices.value("root").toObject().value("size").toString();
        if (diskLimit.isEmpty())
            diskLimit = expandedDevices.value("root").toObject().value("size").toString(QStringLiteral("Shared"));

        QString preset = QStringLiteral("Standard");
        const QJsonArray profiles = obj.value("profiles").toArray();
        for (const auto &p : profiles) {
            if (p.toString() == QLatin1String("ai-gui"))
                preset = QStringLiteral("Desktop & Mobile (GPU + KVM)");
        }
        if (type == QLatin1String("VIRTUAL-MACHINE"))
            preset = QStringLiteral("Full KVM Virtual Machine");

        QString ipv4 = QStringLiteral("—");
        QString memoryUsage = QStringLiteral("0 MB");
        const QJsonObject state = obj.value("state").toObject();
        if (!state.isEmpty()) {
            const qint64 memBytes = state.value("memory").toObject().value("usage").toVariant().toLongLong();
            if (memBytes > 0)
                memoryUsage = formatBytes(memBytes);

            const QJsonObject net = state.value("network").toObject();
            for (auto it = net.begin(); it != net.end(); ++it) {
                if (it.key() == QLatin1String("lo"))
                    continue;
                const QJsonArray addrs = it.value().toObject().value("addresses").toArray();
                for (const auto &addrVal : addrs) {
                    const QJsonObject addrObj = addrVal.toObject();
                    if (addrObj.value("family").toString() == QLatin1String("inet") &&
                        addrObj.value("scope").toString() == QLatin1String("global")) {
                        ipv4 = addrObj.value("address").toString();
                        break;
                    }
                }
                if (ipv4 != QLatin1String("—"))
                    break;
            }
        }

        QStringList snapshots;
        const QJsonArray snapArr = obj.value("snapshots").toArray();
        for (const auto &snapVal : snapArr) {
            const QString snapName = snapVal.toObject().value("name").toString();
            if (!snapName.isEmpty())
                snapshots.append(snapName);
        }

        QVariantMap item;
        item["name"] = name;
        item["status"] = status;
        item["running"] = running;
        item["type"] = type;
        item["osName"] = osName;
        item["preset"] = preset;
        item["cpuLimit"] = cpuLimit == QLatin1String("All") ? tr("All Cores") : QStringLiteral("%1 Cores").arg(cpuLimit);
        item["memoryLimit"] = memoryLimit;
        item["memoryUsage"] = memoryUsage;
        item["diskLimit"] = diskLimit;
        item["ipv4"] = ipv4;
        item["workspacePath"] = workspaceBase + "/" + name;
        item["snapshotCount"] = snapshots.size();
        item["snapshots"] = snapshots;
        result.append(item);
    }

    return result;
}

QVariantList Backend::buildOsCatalog(const QByteArray &imagesJson, const QString &downloadingId) {
    QHash<QString, qint64> cachedAliases;
    const auto doc = QJsonDocument::fromJson(imagesJson);
    if (doc.isArray()) {
        for (const auto &val : doc.array()) {
            const QJsonObject img = val.toObject();
            const qint64 size = img.value("size").toVariant().toLongLong();
            for (const auto &aliasVal : img.value("aliases").toArray()) {
                const QString aliasName = aliasVal.toObject().value("name").toString();
                if (!aliasName.isEmpty())
                    cachedAliases.insert(aliasName, size);
            }
            const QJsonObject props = img.value("properties").toObject();
            const QString os = props.value("os").toString().toLower();
            const QString rel = props.value("release").toString().toLower();
            if (!os.isEmpty())
                cachedAliases.insert(os + "/" + rel, size);
        }
    }

    QVariantList list;
    for (const auto &entry : kOsEntries) {
        const QString id = QString::fromLatin1(entry.id);
        const QString localAlias = QStringLiteral("aibox-") + id;
        const bool downloaded = cachedAliases.contains(localAlias);
        const qint64 sizeBytes = cachedAliases.value(localAlias, 0);

        QVariantMap item;
        item["id"] = id;
        item["alias"] = QString::fromLatin1(entry.alias);
        item["name"] = QString::fromLatin1(entry.name);
        item["badge"] = QString::fromLatin1(entry.badge);
        item["description"] = QString::fromLatin1(entry.description);
        item["downloaded"] = downloaded;
        item["downloading"] = (id == downloadingId);
        item["sizeText"] = downloaded ? QStringLiteral("Cached · %1").arg(formatBytes(sizeBytes))
                                      : QStringLiteral("Not Cached");
        list.append(item);
    }
    return list;
}

void Backend::refresh() {
    if (m_demoMode)
        return;
    if (m_refreshProcess && m_refreshProcess->state() != QProcess::NotRunning)
        return;

    if (!m_refreshProcess) {
        m_refreshProcess = new QProcess(this);
        connect(m_refreshProcess, &QProcess::finished, this, [this](int code, QProcess::ExitStatus) {
            if (code != 0)
                return;
            const QByteArray out = m_refreshProcess->readAllStandardOutput();
            const QList<QByteArray> parts = out.split('\0');
            const QByteArray listJson = parts.value(0);
            const QByteArray imgJson = parts.value(1);

            const QVariantList newEnvs = parseIncusListJson(listJson, m_workspaceBase);
            if (newEnvs != m_environments) {
                m_environments = newEnvs;
                m_count = m_environments.size();
                if (m_selectedIndex >= m_environments.size())
                    m_selectedIndex = qMax(0, static_cast<int>(m_environments.size()) - 1);
                emit environmentsChanged();
                emit countChanged();
                emit selectedIndexChanged();
            }

            const QVariantList newCatalog = buildOsCatalog(imgJson, m_downloadingOsId);
            if (newCatalog != m_osCatalog) {
                m_osCatalog = newCatalog;
                emit osCatalogChanged();
            }
        });
    }

    m_refreshProcess->start(QStringLiteral("/bin/sh"),
                            {QStringLiteral("-c"),
                             QStringLiteral("incus list --format=json 2>/dev/null; printf '\\0'; incus image list --format=json 2>/dev/null")});
}

qint64 Backend::readTaskPid() const {
    QFile pidFile(m_stateDir + "/task.pid");
    if (!pidFile.open(QIODevice::ReadOnly))
        return 0;
    return pidFile.readAll().trimmed().toLongLong();
}

void Backend::clearTaskFiles() {
    QFile::remove(m_stateDir + "/task.pid");
    QFile::remove(m_stateDir + "/task.meta");
    QFile::remove(m_stateDir + "/task.sh");
}

void Backend::handleProcessOutput(const QByteArray &chunk) {
    QString text = QString::fromUtf8(chunk);
    static const QRegularExpression ansiRe(QStringLiteral(R"(\x1B\[[0-9;?]*[a-zA-Z])"));
    text.remove(ansiRe);

    const QStringList lines = text.split(QRegularExpression(QStringLiteral("[\r\n]+")), Qt::SkipEmptyParts);
    static const QRegularExpression stageRe(QStringLiteral(R"(^@@STAGE:(\d+):(.*)$)"));
    static const QRegularExpression exitRe(QStringLiteral(R"(^@@EXIT:(\d+)$)"));
    static const QRegularExpression pctRe(QStringLiteral(R"((\d{1,3})%)"));

    for (const QString &rawLine : lines) {
        const QString line = rawLine.trimmed();
        if (line.isEmpty())
            continue;

        const auto exitMatch = exitRe.match(line);
        if (exitMatch.hasMatch()) {
            m_hasExitCode = true;
            m_lastExitCode = exitMatch.captured(1).toInt();
            continue;
        }

        const auto stageMatch = stageRe.match(line);
        if (stageMatch.hasMatch()) {
            m_taskProgress = qBound(0, stageMatch.captured(1).toInt(), 100);
            m_taskStage = stageMatch.captured(2).trimmed();
            emit taskProgressChanged();
            continue;
        }

        const auto pctMatch = pctRe.match(line);
        if (pctMatch.hasMatch()) {
            const int subPct = qBound(0, pctMatch.captured(1).toInt(), 100);
            if (line.contains(QLatin1String("Retrieving image"), Qt::CaseInsensitive) ||
                line.contains(QLatin1String("Copying"), Qt::CaseInsensitive)) {
                if (!m_pendingEnvName.isEmpty())
                    m_taskProgress = 10 + (subPct * 55) / 100;
                else
                    m_taskProgress = subPct;
            } else if (line.contains(QLatin1String("Unpacking"), Qt::CaseInsensitive)) {
                if (!m_pendingEnvName.isEmpty())
                    m_taskProgress = 65 + (subPct * 15) / 100;
                else
                    m_taskProgress = qMax(m_taskProgress, subPct);
            }
        }

        m_taskLog = line;
        emit taskProgressChanged();
    }
}

void Backend::attachExistingTask() {
    QFile metaFile(m_stateDir + "/task.meta");
    if (!metaFile.open(QIODevice::ReadOnly))
        return;

    const QJsonObject meta = QJsonDocument::fromJson(metaFile.readAll()).object();
    const QString desc = meta.value("description").toString();
    const QString envName = meta.value("envName").toString();
    const QString dlOsId = meta.value("downloadingOsId").toString();

    m_pendingEnvName = envName;
    m_downloadingOsId = dlOsId;
    m_logOffset = 0;
    m_hasExitCode = false;
    m_lastExitCode = -1;
    m_taskProgress = 5;
    m_taskStage = desc;
    m_taskLog = tr("Resuming background task monitor...");

    const qint64 pid = readTaskPid();
    const bool stillRunning = (pid > 0 && ::kill(static_cast<pid_t>(pid), 0) == 0);

    // Read any log output written so far
    QFile logFile(m_stateDir + "/task.log");
    if (logFile.open(QIODevice::ReadOnly)) {
        const QByteArray data = logFile.readAll();
        m_logOffset = data.size();
        handleProcessOutput(data);
    }

    if (stillRunning && !m_hasExitCode) {
        setBusy(true, desc);
        emit taskProgressChanged();
        m_taskWatchTimer.start();
    } else {
        // Task already finished while the GUI was closed!
        clearTaskFiles();
        m_downloadingOsId.clear();
        m_pendingEnvName.clear();
        setBusy(false, {});
        if (m_hasExitCode && m_lastExitCode == 0) {
            m_statusMessage = !envName.isEmpty()
                                  ? tr("Environment '%1' finished creating in the background.").arg(envName)
                                  : tr("Background operation completed successfully.");
            emit statusMessageChanged();
        } else if (m_hasExitCode && m_lastExitCode != 0) {
            setError(tr("Previous background operation failed: %1").arg(m_taskLog));
        }
    }
}

void Backend::checkBackgroundTask() {
    QFile logFile(m_stateDir + "/task.log");
    if (logFile.open(QIODevice::ReadOnly)) {
        if (logFile.size() > m_logOffset) {
            logFile.seek(m_logOffset);
            const QByteArray newBytes = logFile.readAll();
            m_logOffset = logFile.pos();
            handleProcessOutput(newBytes);
        }
    }

    const qint64 pid = readTaskPid();
    const bool alive = (pid > 0 && ::kill(static_cast<pid_t>(pid), 0) == 0);

    if (!alive || m_hasExitCode) {
        m_taskWatchTimer.stop();
        const QString finishedEnv = m_pendingEnvName;
        const QString lastLog = m_taskLog;
        const int exitCode = m_hasExitCode ? m_lastExitCode : 1;

        clearTaskFiles();
        m_downloadingOsId.clear();
        m_pendingEnvName.clear();
        setBusy(false, {});
        refresh();

        if (exitCode == 0) {
            if (!finishedEnv.isEmpty()) {
                m_statusMessage = tr("Environment '%1' is ready.").arg(finishedEnv);
                emit statusMessageChanged();
                emit environmentCreated(finishedEnv);
            }
        } else {
            setError(lastLog.isEmpty() ? tr("Operation failed (exit code %1).").arg(exitCode) : lastLog);
        }
    }
}

void Backend::cancelTask() {
    if (!m_busy)
        return;

    m_taskWatchTimer.stop();
    const qint64 pid = readTaskPid();
    if (pid > 0) {
        ::kill(-static_cast<pid_t>(pid), SIGTERM);
        ::kill(-static_cast<pid_t>(pid), SIGKILL);
    }

    const QString cleanupName = m_pendingEnvName;
    clearTaskFiles();
    m_downloadingOsId.clear();
    m_pendingEnvName.clear();
    setBusy(false, tr("Operation cancelled."));

    if (!cleanupName.isEmpty()) {
        const QString wsPath = m_workspaceBase + "/" + cleanupName;
        QProcess::startDetached(QStringLiteral("/bin/bash"),
                                {QStringLiteral("-c"),
                                 QStringLiteral("incus delete '%1' --force >/dev/null 2>&1 || true; rmdir '%2' 2>/dev/null || true")
                                     .arg(cleanupName, wsPath)});
    }

    QTimer::singleShot(800, this, &Backend::refresh);
    QTimer::singleShot(3000, this, [this] {
        if (!m_busy) {
            m_statusMessage.clear();
            emit statusMessageChanged();
        }
    });
}

void Backend::runShellTask(const QString &description, const QString &script, const QString &createdEnvName) {
    if (m_busy)
        return;

    clearError();
    m_pendingEnvName = createdEnvName;
    m_logOffset = 0;
    m_hasExitCode = false;
    m_lastExitCode = -1;
    m_taskProgress = 5;
    m_taskStage = description;
    m_taskLog = tr("Initializing...");
    setBusy(true, description);
    emit taskProgressChanged();

    // Persist task metadata so reopening AIBox re-attaches seamlessly
    QJsonObject meta;
    meta["description"] = description;
    meta["envName"] = createdEnvName;
    meta["downloadingOsId"] = m_downloadingOsId;
    meta["startedAt"] = QDateTime::currentDateTime().toString(Qt::ISODate);

    QFile metaFile(m_stateDir + "/task.meta");
    if (metaFile.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        metaFile.write(QJsonDocument(meta).toJson(QJsonDocument::Compact));
        metaFile.close();
    }

    QFile logFile(m_stateDir + "/task.log");
    if (logFile.open(QIODevice::WriteOnly | QIODevice::Truncate))
        logFile.close();

    const QString scriptPath = m_stateDir + "/task.sh";
    QFile scriptFile(scriptPath);
    if (scriptFile.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        const QString wrapped = QStringLiteral(R"sh(#!/usr/bin/env bash
(
%1
)
CODE=$?
echo "@@EXIT:$CODE"
if command -v notify-send >/dev/null 2>&1; then
    if [ "$CODE" -eq 0 ]; then
        notify-send -a "AIBox" "AIBox" "%2 — Completed" 2>/dev/null || true
    else
        notify-send -u critical -a "AIBox" "AIBox" "%2 — Failed" 2>/dev/null || true
    fi
fi
exit $CODE
)sh").arg(script, description);
        scriptFile.write(wrapped.toUtf8());
        scriptFile.close();
        scriptFile.setPermissions(QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner);
    }

    const QString launchCmd = QStringLiteral(
        "setsid '%1' > '%2/task.log' 2>&1 < /dev/null & echo $! > '%2/task.pid'")
                                  .arg(scriptPath, m_stateDir);
    QProcess::execute(QStringLiteral("/bin/sh"), {QStringLiteral("-c"), launchCmd});

    m_taskWatchTimer.start();
}

void Backend::createEnvironment(const QString &rawName, const QString &osId, const QString &preset,
                                const QString &diskGiB, const QString &ramGiB, int cpuCores) {
    if (m_busy) {
        setError(tr("Another operation is currently in progress. Please wait or cancel it first."));
        return;
    }

    const QString name = sanitizeName(rawName);
    if (name.isEmpty()) {
        setError(tr("Please enter a valid environment name (letters, numbers, hyphens)."));
        return;
    }

    for (const auto &v : m_environments) {
        if (v.toMap().value("name").toString() == name) {
            setError(tr("Environment '%1' already exists.").arg(name));
            return;
        }
    }

    const QString osAlias = findOsAlias(osId);
    const QString localAlias = QStringLiteral("aibox-") + osId;
    const QString hostWorkspace = m_workspaceBase + "/" + name;

    QString profileFlags = QStringLiteral("-p default -p ai-gui");
    if (preset == QLatin1String("standard"))
        profileFlags = QStringLiteral("-p default -c security.nesting=true");
    else if (preset == QLatin1String("vm"))
        profileFlags = QStringLiteral("--vm -p default");

    const QString script = QStringLiteral(R"sh(
set -euo pipefail
NAME=%1
LOCAL_ALIAS=%2
REMOTE_IMAGE="images:%3"
WORKSPACE=%4

echo "@@STAGE:8:Step 1/4 — Preparing shared workspace directory..."
mkdir -p "$WORKSPACE"
if command -v setfacl >/dev/null 2>&1; then
    setfacl -R -m "u:${USER}:rwx" -d -m "u:${USER}:rwx" "$WORKSPACE" 2>/dev/null || true
fi

if incus image info "$LOCAL_ALIAS" >/dev/null 2>&1; then
    echo "@@STAGE:60:Step 2/4 — Using cached OS image ($LOCAL_ALIAS)..."
    SOURCE_IMAGE="$LOCAL_ALIAS"
else
    echo "@@STAGE:12:Step 2/4 — Downloading OS image ($REMOTE_IMAGE)..."
    script -q -e -c "incus image copy \"$REMOTE_IMAGE\" local: --alias \"$LOCAL_ALIAS\" --auto-update" /dev/null
    SOURCE_IMAGE="$LOCAL_ALIAS"
fi

echo "@@STAGE:72:Step 3/4 — Launching isolated OS container & applying quotas..."
script -q -e -c "incus launch \"$SOURCE_IMAGE\" \"$NAME\" %5 -c limits.cpu=%6 -c limits.memory=%7 -d root,size=%8" /dev/null

echo "@@STAGE:86:Step 4/4 — Mounting /workspace & waiting for network IP..."
sleep 1

if ! incus config device add "$NAME" workspace disk source="$WORKSPACE" path=/workspace shift=true >/dev/null 2>&1; then
    incus config device add "$NAME" workspace disk source="$WORKSPACE" path=/workspace >/dev/null
fi

if [[ "%9" == "full" ]] && [[ -d /tmp/.X11-unix ]]; then
    if command -v xhost >/dev/null 2>&1 && [[ -n "${DISPLAY:-}" ]]; then
        xhost +local: >/dev/null 2>&1 || true
    fi
    incus config device add "$NAME" x11 disk source=/tmp/.X11-unix path=/tmp/.X11-unix >/dev/null 2>&1 || true
fi

incus exec "$NAME" -- sh -c '
    mkdir -p /workspace && chmod 777 /workspace || true
    if [ -f /etc/dhcpcd.conf ] && ! grep -q "^noarp" /etc/dhcpcd.conf; then
        echo "noarp" >> /etc/dhcpcd.conf
        pkill -HUP dhcpcd 2>/dev/null || udhcpc -i eth0 -n -q 2>/dev/null || true
    fi
' >/dev/null 2>&1 || true

for _ in 1 2 3 4 5; do
    if incus list "$NAME" -f csv -c 4 | grep -q "[0-9]"; then
        break
    fi
    sleep 1
done
echo "@@STAGE:100:Environment ready!"
)sh")
                               .arg(QStringLiteral("'") + name + QStringLiteral("'"),
                                    QStringLiteral("'") + localAlias + QStringLiteral("'"),
                                    osAlias,
                                    QStringLiteral("'") + hostWorkspace + QStringLiteral("'"),
                                    profileFlags,
                                    QString::number(qMax(1, cpuCores)),
                                    ramGiB,
                                    diskGiB,
                                    preset);

    runShellTask(tr("Creating environment '%1'").arg(name), script, name);
}

void Backend::enterEnvironment(const QString &name) {
    if (name.isEmpty())
        return;

    const QString script = QStringLiteral(R"sh(
if command -v xhost >/dev/null 2>&1 && [[ -n "${DISPLAY:-}" ]]; then
    xhost +local: >/dev/null 2>&1 || true
fi
STATUS=$(incus info '%1' | awk '/^Status:/ {print $2}')
if [[ "$STATUS" != "RUNNING" ]]; then
    incus start '%1'
    sleep 1
fi
)sh").arg(name);

    QProcess::execute(QStringLiteral("/bin/bash"), {QStringLiteral("-c"), script});

    const QString enterCmd = QStringLiteral(
        "incus exec '%1' --cwd /workspace --env DISPLAY=\"${DISPLAY:-:0}\" --env TERM=\"${TERM:-xterm-256color}\" -- bash -l || "
        "incus exec '%1' --cwd /workspace -- sh").arg(name);

    if (!QStandardPaths::findExecutable(QStringLiteral("xdg-terminal-exec")).isEmpty()) {
        QProcess::startDetached(QStringLiteral("xdg-terminal-exec"),
                                {QStringLiteral("sh"), QStringLiteral("-c"), enterCmd});
    } else if (!QStandardPaths::findExecutable(QStringLiteral("foot")).isEmpty()) {
        QProcess::startDetached(QStringLiteral("foot"),
                                {QStringLiteral("sh"), QStringLiteral("-c"), enterCmd});
    }
    refresh();
}

void Backend::openWorkspaceFolder(const QString &name) {
    if (name.isEmpty())
        return;
    const QString path = m_workspaceBase + "/" + name;
    QDir().mkpath(path);
    QDesktopServices::openUrl(QUrl::fromLocalFile(path));
}

void Backend::toggleStartStop(const QString &name) {
    if (name.isEmpty() || m_busy)
        return;
    bool isRunning = false;
    for (const auto &v : m_environments) {
        const auto map = v.toMap();
        if (map.value("name").toString() == name) {
            isRunning = map.value("running").toBool();
            break;
        }
    }

    if (isRunning) {
        runShellTask(tr("Stopping '%1'").arg(name),
                     QStringLiteral("echo '@@STAGE:40:Stopping container...'; incus stop '%1'; echo '@@STAGE:100:Stopped.'").arg(name));
    } else {
        runShellTask(tr("Starting '%1'").arg(name),
                     QStringLiteral("echo '@@STAGE:40:Starting container...'; incus start '%1' && sleep 1; echo '@@STAGE:100:Started.'").arg(name));
    }
}

void Backend::createSnapshot(const QString &name, const QString &snapshotName) {
    if (name.isEmpty() || m_busy)
        return;
    QString snap = sanitizeName(snapshotName);
    if (snap.isEmpty())
        snap = QStringLiteral("snap-") + QDateTime::currentDateTime().toString(QStringLiteral("yyyyMMdd-HHmmss"));

    runShellTask(tr("Saving snapshot '%1' for '%2'").arg(snap, name),
                 QStringLiteral("echo '@@STAGE:50:Creating Btrfs snapshot...'; incus snapshot create '%1' '%2'; echo '@@STAGE:100:Saved.'").arg(name, snap));
}

void Backend::restoreSnapshot(const QString &name, const QString &snapshotName) {
    if (name.isEmpty() || snapshotName.isEmpty() || m_busy)
        return;
    runShellTask(tr("Restoring '%1' to snapshot '%2'").arg(name, snapshotName),
                 QStringLiteral("echo '@@STAGE:50:Restoring snapshot...'; incus snapshot restore '%1' '%2'; echo '@@STAGE:100:Restored.'").arg(name, snapshotName));
}

void Backend::deleteSnapshot(const QString &name, const QString &snapshotName) {
    if (name.isEmpty() || snapshotName.isEmpty() || m_busy)
        return;
    runShellTask(tr("Deleting snapshot '%1'").arg(snapshotName),
                 QStringLiteral("echo '@@STAGE:50:Deleting snapshot...'; incus snapshot delete '%1' '%2'; echo '@@STAGE:100:Deleted.'").arg(name, snapshotName));
}

void Backend::wipeEnvironment(const QString &name, bool deleteWorkspaceFiles) {
    if (name.isEmpty() || m_busy)
        return;
    const QString hostWorkspace = m_workspaceBase + "/" + name;
    QString script = QStringLiteral("echo '@@STAGE:50:Wiping OS container...'; incus delete '%1' --force").arg(name);
    if (deleteWorkspaceFiles)
        script += QStringLiteral(" && rm -rf '%1'").arg(hostWorkspace);
    script += QStringLiteral("; echo '@@STAGE:100:Wiped.'");

    runShellTask(tr("Wiping environment '%1'").arg(name), script);
}

void Backend::runHeadlessCommand(const QString &name, const QString &command) {
    if (name.isEmpty() || command.trimmed().isEmpty() || m_busy)
        return;
    QString escapedCmd = command;
    escapedCmd.replace('\'', QStringLiteral("'\\''"));

    const QString script = QStringLiteral(R"sh(
set -euo pipefail
echo "@@STAGE:20:Checking virtual display server (Xvfb)..."
STATUS=$(incus info '%1' | awk '/^Status:/ {print $2}')
if [[ "$STATUS" != "RUNNING" ]]; then
    incus start '%1'
    sleep 2
fi
incus exec '%1' -- sh -c '
    if ! command -v xvfb-run >/dev/null 2>&1; then
        if command -v apt-get >/dev/null 2>&1; then
            apt-get update && apt-get install -y xvfb xauth
        elif command -v pacman >/dev/null 2>&1; then
            pacman -Sy --noconfirm xorg-server-xvfb xorg-xauth
        elif command -v dnf >/dev/null 2>&1; then
            dnf install -y xorg-x11-server-Xvfb xauth
        elif command -v apk >/dev/null 2>&1; then
            apk add --no-cache xvfb-run xvfb xauth
        fi
    fi
'
echo "@@STAGE:60:Running command inside 1920x1080 virtual screen..."
incus exec '%1' --cwd /workspace -- xvfb-run -a -s "-screen 0 1920x1080x24" sh -lc '%2'
echo "@@STAGE:100:Completed."
)sh").arg(name, escapedCmd);

    runShellTask(tr("Running headless GUI test in '%1'").arg(name), script);
}

void Backend::copyAiCommand(const QString &name) {
    if (name.isEmpty())
        return;
    if (auto *cb = QGuiApplication::clipboard()) {
        cb->setText(QStringLiteral("incus exec %1 --cwd /workspace -- bash").arg(name));
        m_statusMessage = tr("Copied AI exec command for '%1' to clipboard.").arg(name);
        emit statusMessageChanged();
        QTimer::singleShot(3000, this, [this] {
            if (!m_busy) {
                m_statusMessage.clear();
                emit statusMessageChanged();
            }
        });
    }
}

void Backend::downloadOsImage(const QString &osId) {
    if (osId.isEmpty() || m_busy)
        return;
    const QString alias = findOsAlias(osId);
    const QString localAlias = QStringLiteral("aibox-") + osId;
    m_downloadingOsId = osId;
    m_osCatalog = buildOsCatalog({}, m_downloadingOsId);
    emit osCatalogChanged();

    const QString script = QStringLiteral(
        "set -euo pipefail\n"
        "echo '@@STAGE:5:Downloading %1 from official image server...'\n"
        "script -q -e -c \"incus image copy 'images:%1' local: --alias '%2' --auto-update\" /dev/null\n"
        "echo '@@STAGE:100:Download complete!'\n").arg(alias, localAlias);
    runShellTask(tr("Downloading OS image '%1'").arg(osId), script);
}

void Backend::removeOsImage(const QString &osId) {
    if (osId.isEmpty() || m_busy)
        return;
    const QString localAlias = QStringLiteral("aibox-") + osId;
    runShellTask(tr("Removing cached OS image '%1'").arg(osId),
                 QStringLiteral("echo '@@STAGE:50:Removing image cache...'; incus image delete '%1'; echo '@@STAGE:100:Removed.'").arg(localAlias));
}

void Backend::loadDemoData() {
    QVariantList demoEnvs;
    {
        QVariantMap e1;
        e1["name"] = QStringLiteral("flutter-pos");
        e1["status"] = QStringLiteral("RUNNING");
        e1["running"] = true;
        e1["type"] = QStringLiteral("CONTAINER");
        e1["osName"] = QStringLiteral("Ubuntu 24.04 LTS");
        e1["preset"] = QStringLiteral("Desktop & Mobile (GPU + KVM)");
        e1["cpuLimit"] = QStringLiteral("4 Cores");
        e1["memoryLimit"] = QStringLiteral("8GiB");
        e1["memoryUsage"] = QStringLiteral("1.4 GB");
        e1["diskLimit"] = QStringLiteral("60GiB");
        e1["ipv4"] = QStringLiteral("10.143.44.18");
        e1["workspacePath"] = QStringLiteral("~/ai-workspaces/flutter-pos");
        e1["snapshotCount"] = 2;
        e1["snapshots"] = QStringList{QStringLiteral("clean-sdk"), QStringLiteral("before-ai-refactor")};
        demoEnvs.append(e1);
    }
    {
        QVariantMap e2;
        e2["name"] = QStringLiteral("desktop-studio");
        e2["status"] = QStringLiteral("RUNNING");
        e2["running"] = true;
        e2["type"] = QStringLiteral("CONTAINER");
        e2["osName"] = QStringLiteral("Arch Linux");
        e2["preset"] = QStringLiteral("Desktop & Mobile (GPU + KVM)");
        e2["cpuLimit"] = QStringLiteral("6 Cores");
        e2["memoryLimit"] = QStringLiteral("6GiB");
        e2["memoryUsage"] = QStringLiteral("620 MB");
        e2["diskLimit"] = QStringLiteral("35GiB");
        e2["ipv4"] = QStringLiteral("10.143.44.42");
        e2["workspacePath"] = QStringLiteral("~/ai-workspaces/desktop-studio");
        e2["snapshotCount"] = 1;
        e2["snapshots"] = QStringList{QStringLiteral("initial-setup")};
        demoEnvs.append(e2);
    }
    {
        QVariantMap e3;
        e3["name"] = QStringLiteral("saas-backend");
        e3["status"] = QStringLiteral("STOPPED");
        e3["running"] = false;
        e3["type"] = QStringLiteral("CONTAINER");
        e3["osName"] = QStringLiteral("Debian 12");
        e3["preset"] = QStringLiteral("Standard");
        e3["cpuLimit"] = QStringLiteral("2 Cores");
        e3["memoryLimit"] = QStringLiteral("4GiB");
        e3["memoryUsage"] = QStringLiteral("0 MB");
        e3["diskLimit"] = QStringLiteral("20GiB");
        e3["ipv4"] = QStringLiteral("—");
        e3["workspacePath"] = QStringLiteral("~/ai-workspaces/saas-backend");
        e3["snapshotCount"] = 0;
        e3["snapshots"] = QStringList{};
        demoEnvs.append(e3);
    }
    m_environments = demoEnvs;
    m_count = m_environments.size();
}
