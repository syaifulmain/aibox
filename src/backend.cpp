#include "backend.h"

#include <QClipboard>
#include <QDateTime>
#include <QDesktopServices>
#include <QDir>
#include <QFileInfo>
#include <QGuiApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QRegularExpression>
#include <QStandardPaths>
#include <QUrl>

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
      m_demoMode(qEnvironmentVariableIntValue("AIBOX_DEMO_MODE") == 1) {
    m_osCatalog = buildOsCatalog({});

    if (m_demoMode) {
        loadDemoData();
        return;
    }

    QDir().mkpath(m_workspaceBase);

    m_pollTimer.setInterval(4000);
    connect(&m_pollTimer, &QTimer::timeout, this, [this] {
        if (!m_busy)
            refresh();
    });

    // Only start background polling when running inside a live GUI session with incus installed.
    if (!QStandardPaths::findExecutable("incus").isEmpty() &&
        QGuiApplication::platformName() != QLatin1String("offscreen")) {
        QTimer::singleShot(100, this, &Backend::refresh);
        m_pollTimer.start();
    }
}

Backend::~Backend() {
    if (m_taskProcess && m_taskProcess->state() != QProcess::NotRunning) {
        m_taskProcess->kill();
        m_taskProcess->waitForFinished(1000);
    }
    if (m_refreshProcess && m_refreshProcess->state() != QProcess::NotRunning) {
        m_refreshProcess->kill();
        m_refreshProcess->waitForFinished(1000);
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
        // Shorten verbose cloud descriptions
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

        // Determine preset label
        QString preset = QStringLiteral("Standard");
        const QJsonArray profiles = obj.value("profiles").toArray();
        for (const auto &p : profiles) {
            if (p.toString() == QLatin1String("ai-gui"))
                preset = QStringLiteral("Desktop & Mobile (GPU + KVM)");
        }
        if (type == QLatin1String("VIRTUAL-MACHINE"))
            preset = QStringLiteral("Full KVM Virtual Machine");

        // Extract IPv4 and live memory usage from state
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

        // Extract snapshots
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
        item["sizeText"] = downloaded ? QStringLiteral("Ready (%1)").arg(formatBytes(sizeBytes))
                                      : QStringLiteral("Tap to Download");
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

void Backend::runShellTask(const QString &description, const QString &script, const QString &createdEnvName) {
    if (m_busy)
        return;

    clearError();
    setBusy(true, description);

    if (!m_taskProcess) {
        m_taskProcess = new QProcess(this);
    } else {
        m_taskProcess->disconnect();
    }

    connect(m_taskProcess, &QProcess::finished, this,
            [this, createdEnvName](int code, QProcess::ExitStatus status) {
                const QString stdErr = QString::fromUtf8(m_taskProcess->readAllStandardError()).trimmed();
                m_downloadingOsId.clear();
                setBusy(false, {});
                refresh();
                if (status != QProcess::NormalExit || code != 0) {
                    setError(stdErr.isEmpty() ? tr("Operation failed (exit code %1).").arg(code) : stdErr);
                } else if (!createdEnvName.isEmpty()) {
                    emit environmentCreated(createdEnvName);
                }
            });

    m_taskProcess->start(QStringLiteral("/bin/bash"), {QStringLiteral("-c"), script});
}

void Backend::createEnvironment(const QString &rawName, const QString &osId, const QString &preset,
                                const QString &diskGiB, const QString &ramGiB, int cpuCores) {
    const QString name = sanitizeName(rawName);
    if (name.isEmpty()) {
        setError(tr("Please enter a valid environment name (letters, numbers, hyphens)."));
        return;
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

mkdir -p "$WORKSPACE"
if command -v setfacl >/dev/null 2>&1; then
    setfacl -R -m "u:${USER}:rwx" -d -m "u:${USER}:rwx" "$WORKSPACE" 2>/dev/null || true
fi

if incus image info "$LOCAL_ALIAS" >/dev/null 2>&1; then
    SOURCE_IMAGE="$LOCAL_ALIAS"
else
    incus image copy "$REMOTE_IMAGE" local: --alias "$LOCAL_ALIAS" --auto-update
    SOURCE_IMAGE="$LOCAL_ALIAS"
fi

incus launch "$SOURCE_IMAGE" "$NAME" %5 \
    -c "limits.cpu=%6" \
    -c "limits.memory=%7" \
    -d "root,size=%8"

sleep 2

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

for _ in 1 2 3 4 5 6; do
    if incus list "$NAME" -f csv -c 4 | grep -q "[0-9]"; then
        break
    fi
    sleep 1
done
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

    runShellTask(tr("Creating environment '%1' (%2, %3 disk, %4 RAM)...").arg(name, osId, diskGiB, ramGiB),
                 script, name);
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
    if (name.isEmpty())
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
        runShellTask(tr("Stopping '%1'...").arg(name),
                     QStringLiteral("incus stop '%1'").arg(name));
    } else {
        runShellTask(tr("Starting '%1'...").arg(name),
                     QStringLiteral("incus start '%1' && sleep 2").arg(name));
    }
}

void Backend::createSnapshot(const QString &name, const QString &snapshotName) {
    if (name.isEmpty())
        return;
    QString snap = sanitizeName(snapshotName);
    if (snap.isEmpty())
        snap = QStringLiteral("snap-") + QDateTime::currentDateTime().toString(QStringLiteral("yyyyMMdd-HHmmss"));

    runShellTask(tr("Saving snapshot '%1' for '%2'...").arg(snap, name),
                 QStringLiteral("incus snapshot create '%1' '%2'").arg(name, snap));
}

void Backend::restoreSnapshot(const QString &name, const QString &snapshotName) {
    if (name.isEmpty() || snapshotName.isEmpty())
        return;
    runShellTask(tr("Restoring '%1' to snapshot '%2'...").arg(name, snapshotName),
                 QStringLiteral("incus snapshot restore '%1' '%2'").arg(name, snapshotName));
}

void Backend::deleteSnapshot(const QString &name, const QString &snapshotName) {
    if (name.isEmpty() || snapshotName.isEmpty())
        return;
    runShellTask(tr("Deleting snapshot '%1'...").arg(snapshotName),
                 QStringLiteral("incus snapshot delete '%1' '%2'").arg(name, snapshotName));
}

void Backend::wipeEnvironment(const QString &name, bool deleteWorkspaceFiles) {
    if (name.isEmpty())
        return;
    const QString hostWorkspace = m_workspaceBase + "/" + name;
    QString script = QStringLiteral("incus delete '%1' --force").arg(name);
    if (deleteWorkspaceFiles)
        script += QStringLiteral(" && rm -rf '%1'").arg(hostWorkspace);

    runShellTask(tr("Wiping environment '%1'...").arg(name), script);
}

void Backend::runHeadlessCommand(const QString &name, const QString &command) {
    if (name.isEmpty() || command.trimmed().isEmpty())
        return;
    QString escapedCmd = command;
    escapedCmd.replace('\'', QStringLiteral("'\\''"));

    const QString script = QStringLiteral(R"sh(
set -euo pipefail
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
incus exec '%1' --cwd /workspace -- xvfb-run -a -s "-screen 0 1920x1080x24" sh -lc '%2'
)sh").arg(name, escapedCmd);

    runShellTask(tr("Running headless GUI test in '%1'...").arg(name), script);
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

    const QString script = QStringLiteral("incus image copy 'images:%1' local: --alias '%2' --auto-update")
                               .arg(alias, localAlias);
    runShellTask(tr("Downloading OS image '%1' to local cache...").arg(osId), script);
}

void Backend::removeOsImage(const QString &osId) {
    if (osId.isEmpty() || m_busy)
        return;
    const QString localAlias = QStringLiteral("aibox-") + osId;
    runShellTask(tr("Removing cached OS image '%1'...").arg(osId),
                 QStringLiteral("incus image delete '%1'").arg(localAlias));
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
