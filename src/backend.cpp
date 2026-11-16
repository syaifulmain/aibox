#include "backend.h"

#include <QClipboard>
#include <QCoreApplication>
#include <QDateTime>
#include <QDesktopServices>
#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QHostAddress>
#include <QHostInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QRegularExpression>
#include <QSaveFile>
#include <QSet>
#include <QStandardPaths>
#include <QStorageInfo>
#include <QTcpServer>
#include <QThread>
#include <QUrl>
#include <QUuid>
#include <algorithm>
#include <cerrno>
#include <cmath>
#include <csignal>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>

namespace {

// Path where the shared host workspace is mounted inside every environment.
const QString kContainerWorkspacePath = QStringLiteral("/home/workspace");

QString formatBytes(qint64 bytes) {
    if (bytes <= 0)
        return QStringLiteral("0 MiB");
    const double mb = static_cast<double>(bytes) / (1024.0 * 1024.0);
    if (mb >= 1024.0)
        return QStringLiteral("%1 GiB").arg(mb / 1024.0, 0, 'f', 1);
    if (mb < 1.0)
        return QStringLiteral("%1 KiB").arg(static_cast<double>(bytes) / 1024.0, 0, 'f', 0);
    return QStringLiteral("%1 MiB").arg(mb, 0, 'f', 0);
}

QString formatByteRate(qint64 bytesPerSecond) {
    if (bytesPerSecond <= 0)
        return QStringLiteral("0 KB/s");
    const double kb = static_cast<double>(bytesPerSecond) / 1024.0;
    if (kb < 1.0)
        return QStringLiteral("%1 B/s").arg(bytesPerSecond);
    if (kb < 1024.0)
        return QStringLiteral("%1 KB/s").arg(kb, 0, 'f', 1);
    const double mb = kb / 1024.0;
    if (mb < 1024.0)
        return QStringLiteral("%1 MB/s").arg(mb, 0, 'f', 2);
    return QStringLiteral("%1 GB/s").arg(mb / 1024.0, 0, 'f', 2);
}

QString formatByteTotal(qint64 bytes) {
    if (bytes <= 0)
        return QStringLiteral("0 B");
    const double kb = static_cast<double>(bytes) / 1024.0;
    if (kb < 1024.0)
        return QStringLiteral("%1 KB").arg(kb, 0, 'f', 1);
    const double mb = kb / 1024.0;
    if (mb < 1024.0)
        return QStringLiteral("%1 MB").arg(mb, 0, 'f', 2);
    const double gb = mb / 1024.0;
    if (gb < 1024.0)
        return QStringLiteral("%1 GB").arg(gb, 0, 'f', 2);
    return QStringLiteral("%1 TB").arg(gb / 1024.0, 0, 'f', 2);
}

qint64 parseSizeBytes(const QString &sizeText) {
    static const QRegularExpression re(
        QStringLiteral("^\\s*(\\d+(?:\\.\\d+)?)\\s*(TiB|TB|GiB|GB|MiB|MB|KiB|KB|B)?\\s*$"),
        QRegularExpression::CaseInsensitiveOption);
    const auto match = re.match(sizeText);
    if (!match.hasMatch())
        return 0;

    bool ok = false;
    const double value = match.captured(1).toDouble(&ok);
    if (!ok || value <= 0.0)
        return 0;

    const QString unit = match.captured(2).toUpper();
    double scale = 1.0;
    if (unit == QLatin1String("KIB") || unit == QLatin1String("KB"))
        scale = 1024.0;
    else if (unit == QLatin1String("MIB") || unit == QLatin1String("MB") || unit.isEmpty())
        scale = 1024.0 * 1024.0;
    else if (unit == QLatin1String("GIB") || unit == QLatin1String("GB"))
        scale = 1024.0 * 1024.0 * 1024.0;
    else if (unit == QLatin1String("TIB") || unit == QLatin1String("TB"))
        scale = 1024.0 * 1024.0 * 1024.0 * 1024.0;

    return static_cast<qint64>(std::llround(value * scale));
}

int parseCpuCount(const QString &cpuLimitText) {
    const QString trimmed = cpuLimitText.trimmed();
    if (trimmed.isEmpty() || trimmed.compare(QLatin1String("All"), Qt::CaseInsensitive) == 0)
        return qMax(1, QThread::idealThreadCount());

    bool ok = false;
    const int direct = trimmed.toInt(&ok);
    if (ok && direct > 0)
        return direct;

    int count = 0;
    for (const QString &part : trimmed.split(',', Qt::SkipEmptyParts)) {
        const QString p = part.trimmed();
        const int dash = p.indexOf('-');
        if (dash > 0) {
            bool ok1 = false, ok2 = false;
            const int start = p.left(dash).toInt(&ok1);
            const int end = p.mid(dash + 1).toInt(&ok2);
            if (ok1 && ok2 && end >= start)
                count += (end - start + 1);
        } else {
            count += 1;
        }
    }
    return qMax(1, count);
}

QString formatPercent(int percent) {
    return QStringLiteral("%1%").arg(qBound(0, percent, 100));
}

QVariantMap poolInfoToVariantMap(const Backend::StoragePoolInfo &pool,
                                 qint64 allocatedBytes,
                                 int unboundedCount) {
    return QVariantMap{
        {"valid", pool.valid},
        {"name", pool.name},
        {"driver", pool.driver},
        {"usedBytes", pool.usedBytes},
        {"totalBytes", pool.totalBytes},
        {"usedText", pool.valid ? formatBytes(pool.usedBytes) : QString()},
        {"totalText", pool.valid ? formatBytes(pool.totalBytes) : QString()},
        {"usedPercent", pool.usedPercent},
        {"allocatedBytes", allocatedBytes},
        {"allocatedText", formatBytes(allocatedBytes)},
        {"unboundedCount", unboundedCount},
        {"sizeText", pool.sizeText},
        {"resizable", pool.resizable},
        {"overcommitted", pool.valid && (allocatedBytes > pool.totalBytes)},
        {"alertLevel", !pool.valid ? 0
                                   : (pool.usedPercent > 95 ? 2
                                      : ((pool.usedPercent > 85 || allocatedBytes > pool.totalBytes) ? 1 : 0))}
    };
}

QVector<qint64> readHostProcStatIdleAndTotal(QVector<qint64> *outTotal) {
    QVector<qint64> idle;
    if (outTotal)
        outTotal->clear();

    QFile statFile(QStringLiteral("/proc/stat"));
    if (!statFile.open(QIODevice::ReadOnly | QIODevice::Text))
        return idle;

    while (!statFile.atEnd()) {
        const QByteArray line = statFile.readLine().trimmed();
        if (!line.startsWith("cpu"))
            continue;
        if (line.length() < 4 || line.at(3) < '0' || line.at(3) > '9')
            continue; // skip aggregate "cpu " line

        const QList<QByteArray> parts = line.split(' ');
        qint64 total = 0;
        qint64 idleVal = 0;
        int col = 0;
        for (int i = 1; i < parts.size(); ++i) {
            if (parts.at(i).isEmpty())
                continue;
            const qint64 v = parts.at(i).toLongLong();
            total += v;
            if (col == 3 || col == 4) // idle + iowait
                idleVal += v;
            ++col;
        }
        idle.append(idleVal);
        if (outTotal)
            outTotal->append(total);
    }
    return idle;
}

QVariantList buildCpuCorePercents(int coreCount, int avgPercent, bool running, const QVector<int> &hostWeights = {}) {
    const int n = qBound(1, coreCount, 64);
    QVariantList list;
    if (!running || avgPercent <= 0) {
        for (int i = 0; i < n; ++i)
            list.append(0);
        return list;
    }

    QVector<double> weights(n, 1.0);
    double weightSum = 0.0;
    for (int i = 0; i < n; ++i) {
        double w = 1.0;
        if (i < hostWeights.size() && hostWeights.at(i) > 0) {
            w = static_cast<double>(hostWeights.at(i)) + 5.0;
        } else {
            // Subtle deterministic spread when host weights are uniform
            w = 1.0 + 0.12 * (((i * 3) % n) - (n - 1) / 2.0);
        }
        weights[i] = qMax(0.2, w);
        weightSum += weights[i];
    }

    const int targetSum = avgPercent * n;
    int currentSum = 0;
    QVector<int> values(n, 0);
    for (int i = 0; i < n; ++i) {
        const int v = qBound(0, static_cast<int>(std::llround((weights[i] / weightSum) * targetSum)), 100);
        values[i] = v;
        currentSum += v;
    }

    // Reconcile rounding difference so average matches avgPercent
    int diff = targetSum - currentSum;
    for (int i = 0; i < n && diff != 0; ++i) {
        if (diff > 0 && values[i] < 100) {
            const int add = qMin(diff, 100 - values[i]);
            values[i] += add;
            diff -= add;
        } else if (diff < 0 && values[i] > 0) {
            const int sub = qMin(-diff, values[i]);
            values[i] -= sub;
            diff += sub;
        }
    }

    for (int v : values)
        list.append(v);
    return list;
}

QStringList validatedSizeOptions(const QStringList &values) {
    static const QRegularExpression sizeRe(QStringLiteral("^\\d+(?:\\.\\d+)?(?:GiB|MiB|TiB|GB|MB|TB)$"),
                                           QRegularExpression::CaseInsensitiveOption);
    QStringList result;
    for (const auto &raw : values) {
        const QString value = raw.trimmed();
        if (value.isEmpty() || !sizeRe.match(value).hasMatch())
            continue;
        if (!result.contains(value, Qt::CaseInsensitive))
            result.append(value);
    }
    return result;
}

QVariantList validatedCpuOptions(const QVariantList &values) {
    QVariantList result;
    QSet<int> seen;
    for (const auto &raw : values) {
        bool ok = false;
        const int cores = raw.toInt(&ok);
        if (!ok || cores < 1 || cores > 1024 || seen.contains(cores))
            continue;
        seen.insert(cores);
        result.append(cores);
    }
    return result;
}

bool isPrivateOrReservedIpv4(const QHostAddress &addr) {
    if (addr.protocol() != QAbstractSocket::IPv4Protocol)
        return true;
    const quint32 ip = addr.toIPv4Address();
    if ((ip & 0xFF000000u) == 0x00000000u) return true; // 0.0.0.0/8
    if ((ip & 0xFF000000u) == 0x0A000000u) return true; // 10.0.0.0/8
    if ((ip & 0xFFC00000u) == 0x64400000u) return true; // 100.64.0.0/10
    if ((ip & 0xFF000000u) == 0x7F000000u) return true; // 127.0.0.0/8
    if ((ip & 0xFFFF0000u) == 0xA9FE0000u) return true; // 169.254.0.0/16
    if ((ip & 0xFFF00000u) == 0xAC100000u) return true; // 172.16.0.0/12
    if ((ip & 0xFFFF0000u) == 0xC0A80000u) return true; // 192.168.0.0/16
    if (ip >= 0xE0000000u) return true;                 // multicast / reserved
    return false;
}

bool isGuiSession() {
    return qobject_cast<QGuiApplication *>(QCoreApplication::instance()) != nullptr &&
           QGuiApplication::platformName() != QLatin1String("offscreen");
}

bool isLocalTcpPortListening(int port) {
    if (port < 1 || port > 65535)
        return false;
    QFile tcp(QStringLiteral("/proc/net/tcp"));
    if (!tcp.open(QIODevice::ReadOnly))
        return false;
    // /proc/net/tcp columns: sl, local_address (HEXADDR:HEXPORT), rem_address, st, ...
    // 127.0.0.1 in little-endian hex is 0100007F and st 0A means LISTEN.
    const QString needle = QStringLiteral("0100007F:%1").arg(port, 4, 16, QLatin1Char('0')).toUpper();
    const QStringList lines = QString::fromLatin1(tcp.readAll()).split(QLatin1Char('\n'), Qt::SkipEmptyParts);
    for (const QString &line : lines) {
        const QStringList fields = line.split(QRegularExpression(QStringLiteral("\\s+")), Qt::SkipEmptyParts);
        if (fields.size() < 4)
            continue;
        if (fields.at(1).compare(needle, Qt::CaseInsensitive) == 0 && fields.at(3) == QLatin1String("0A"))
            return true;
    }
    return false;
}

} // namespace

Backend::Backend(const QString &workspaceDir, QObject *parent)
    : Backend(workspaceDir, PathMode::WorkspaceDir, parent) {}

Backend::Backend(const QString &path, PathMode mode, QObject *parent)
    : QObject(parent),
      m_pathMode(mode),
      m_rememberSystemLocation(path.trimmed().isEmpty()),
      m_demoMode(qEnvironmentVariableIntValue("AIBOX_DEMO_MODE") == 1) {
    if (mode == PathMode::ProjectDir) {
        const QString aiboxDir = resolveProjectAiboxDir(path);
        m_systemPath = aiboxDir;
        m_workspaceBase = aiboxDir + QStringLiteral("/workspaces");
        m_osStoragePath = aiboxDir + QStringLiteral("/os-images");
        m_stateDir = aiboxDir + QStringLiteral("/state");
        m_configPath = aiboxDir + QStringLiteral("/config.json");
    } else if (path.trimmed().isEmpty()) {
        const QString aiboxDir = configuredSystemPath();
        m_systemPath = aiboxDir;
        m_workspaceBase = aiboxDir + QStringLiteral("/workspaces");
        m_osStoragePath = aiboxDir + QStringLiteral("/os-images");
        m_stateDir = aiboxDir + QStringLiteral("/state");
        m_configPath = aiboxDir + QStringLiteral("/config.json");
    } else {
        const QString cleanPath = QDir::cleanPath(path);
        m_systemPath = cleanPath;
        m_workspaceBase = cleanPath;
        m_osStoragePath = cleanPath + QStringLiteral("/os-images");
        m_stateDir = cleanPath + QStringLiteral("/.state");
        m_configPath = cleanPath + QStringLiteral("/.state/config.json");
    }
    m_defaultWorkspaceBase = m_workspaceBase;
    m_defaultOsStoragePath = m_osStoragePath;

    m_osEntries = defaultOsEntries();
    m_presets = defaultPresets();
    m_diskOptions = defaultDiskOptions();
    m_ramOptions = defaultRamOptions();
    m_cpuOptions = defaultCpuOptions();
    m_defaultPresetId = QStringLiteral("full");
    m_defaultOsId = QStringLiteral("ubuntu-24.04");
    m_defaultDisk = QStringLiteral("35GiB");
    m_defaultRam = QStringLiteral("6GiB");
    m_defaultCpu = 4;
    m_processLimitOptions = QStringList{QStringLiteral("2048"), QStringLiteral("4096"),
                                        QStringLiteral("8192"), QStringLiteral("Unlimited")};
    m_ioLimitOptions = QStringList{QStringLiteral("Unlimited"), QStringLiteral("50MB"),
                                   QStringLiteral("100MB"), QStringLiteral("250MB")};
    m_defaultProcessLimit = QStringLiteral("4096");
    m_defaultIoLimit = QStringLiteral("Unlimited");
    m_diffExcludes = defaultDiffExcludes();
    const QString envSecretsPath = qEnvironmentVariable("AIBOX_SECRETS_PATH").trimmed();
    m_secretsPath = envSecretsPath.isEmpty() ? defaultSecretsStorePath() : QDir::cleanPath(envSecretsPath);

    m_diffTimeoutTimer.setSingleShot(true);
    m_diffTimeoutTimer.setInterval(30000);
    connect(&m_diffTimeoutTimer, &QTimer::timeout, this, [this] {
        if (m_diffProcess && m_diffProcess->state() != QProcess::NotRunning) {
            m_diffProcess->setProperty("timedOut", true);
            m_diffProcess->kill();
        }
    });

    m_netProbeTimer.setInterval(10000);
    connect(&m_netProbeTimer, &QTimer::timeout, this, &Backend::refreshNetworkStats);

    loadConfig();
    reloadSecrets();
    rebuildOsCatalog();
    loadHistorySeenMarker();
    loadOperationHistory();

    if (m_demoMode) {
        m_firstRun = false;
        loadDemoData();
        return;
    }

    if (!m_firstRun) {
        QDir().mkpath(m_workspaceBase);
        QDir().mkpath(m_osStoragePath);
        QDir().mkpath(m_stateDir);
        seedProvisionScripts();
        refreshBackups();
        if (isGuiSession())
            refreshRecordedSessions();
    }

    m_taskWatchTimer.setInterval(250);
    connect(&m_taskWatchTimer, &QTimer::timeout, this, &Backend::checkBackgroundTask);

    m_pollTimer.setInterval(4000);
    connect(&m_pollTimer, &QTimer::timeout, this, [this] {
        if (!m_busy)
            refresh();
    });

    if (!QStandardPaths::findExecutable("incus").isEmpty() && isGuiSession()) {
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
    if (m_netProbeProcess && m_netProbeProcess->state() != QProcess::NotRunning) {
        m_netProbeProcess->kill();
        m_netProbeProcess->waitForFinished(500);
    }
}

QString Backend::defaultSystemPath() {
    return QDir::cleanPath(QDir::homePath() + QStringLiteral("/aibox"));
}

QString Backend::defaultLocationFilePath() {
    const QString envPath = qEnvironmentVariable("AIBOX_LOCATION_FILE").trimmed();
    if (!envPath.isEmpty())
        return QDir::cleanPath(envPath);
    const QString cfg = QStandardPaths::writableLocation(QStandardPaths::ConfigLocation);
    const QString base = cfg.trimmed().isEmpty() ? QDir::homePath() + QStringLiteral("/.config") : cfg.trimmed();
    return QDir::cleanPath(base + QStringLiteral("/aibox/location.json"));
}

QString Backend::configuredSystemPath() {
    QFile file(defaultLocationFilePath());
    if (file.open(QIODevice::ReadOnly)) {
        const QJsonDocument doc = QJsonDocument::fromJson(file.readAll());
        if (doc.isObject()) {
            QString raw = doc.object().value(QStringLiteral("systemPath")).toString().trimmed();
            if (!raw.isEmpty()) {
                if (raw == QLatin1String("~"))
                    raw = QDir::homePath();
                else if (raw.startsWith(QLatin1String("~/")))
                    raw = QDir::homePath() + raw.mid(1);
                if (!QDir::isAbsolutePath(raw))
                    raw = QDir::homePath() + QLatin1Char('/') + raw;
                return QDir::cleanPath(raw);
            }
        }
    }
    return defaultSystemPath();
}

bool Backend::saveLocationFile(const QString &systemPath) {
    const QString locPath = defaultLocationFilePath();
    const QString parentDir = QFileInfo(locPath).absolutePath();
    if (!QDir().mkpath(parentDir))
        return false;
    QJsonObject obj;
    obj[QStringLiteral("version")] = 1;
    obj[QStringLiteral("systemPath")] = QDir::cleanPath(systemPath);
    QSaveFile file(locPath);
    if (!file.open(QIODevice::WriteOnly))
        return false;
    file.write(QJsonDocument(obj).toJson(QJsonDocument::Indented));
    return file.commit();
}

QString Backend::resolveProjectAiboxDir(const QString &projectDir, const QString &baseCwd) {
    QString dir = projectDir.trimmed();
    if (dir.isEmpty()) {
        if (baseCwd.trimmed().isEmpty())
            return configuredSystemPath();
        return QDir::cleanPath(QDir::cleanPath(baseCwd.trimmed()) + QStringLiteral("/aibox"));
    }
    const QString cwd = baseCwd.trimmed().isEmpty() ? QDir::currentPath() : QDir::cleanPath(baseCwd.trimmed());
    if (dir == QLatin1String("~"))
        dir = QDir::homePath();
    else if (dir.startsWith(QLatin1String("~/")))
        dir = QDir::homePath() + dir.mid(1);
    if (!QDir::isAbsolutePath(dir))
        dir = QDir(cwd).filePath(dir);
    dir = QDir::cleanPath(dir);
    const QString baseName = QFileInfo(dir).fileName();
    if (baseName == QLatin1String("aibox") || baseName == QLatin1String(".aibox"))
        return dir;
    const QString legacyDotDir = QDir::cleanPath(dir + QStringLiteral("/.aibox"));
    const QString stdDir = QDir::cleanPath(dir + QStringLiteral("/aibox"));
    if (QDir(legacyDotDir).exists() && !QDir(stdDir).exists())
        return legacyDotDir;
    return stdDir;
}

QString Backend::defaultWorkspacePath() {
    return defaultSystemPath() + QStringLiteral("/workspaces");
}

QString Backend::defaultOsStoragePathValue() {
    return defaultSystemPath() + QStringLiteral("/os-images");
}

QVariantList Backend::defaultOsEntries() {
    auto entry = [](const char *id, const char *alias, const char *name, const char *badge, const char *description) {
        QVariantMap map;
        map["id"] = QString::fromLatin1(id);
        map["alias"] = QString::fromLatin1(alias);
        map["name"] = QString::fromLatin1(name);
        map["badge"] = QString::fromLatin1(badge);
        map["description"] = QString::fromLatin1(description);
        return map;
    };

    return {
        entry("ubuntu-24.04", "ubuntu/24.04", "Ubuntu 24.04 LTS (Noble)", "Recommended",
              "Best compatibility for AI tools, Desktop (Electron/Qt/Flutter), and Android SDK."),
        entry("ubuntu-22.04", "ubuntu/22.04", "Ubuntu 22.04 LTS (Jammy)", "LTS",
              "Proven long-term support release for legacy C++, Python, and mobile toolchains."),
        entry("debian-12", "debian/12", "Debian 12 (Bookworm)", "Lightweight",
              "Rock-solid stability with low RAM overhead (~60 MB idle). Ideal for backend & databases."),
        entry("archlinux", "archlinux", "Arch Linux", "Rolling",
              "Bleeding-edge packages matching Omarchy. Great for native C++, Rust, and Qt6 builds."),
        entry("fedora-41", "fedora/41", "Fedora 41", "Modern",
              "Up-to-date workstation & server stack with modern GCC, Podman/Docker, and Wayland."),
        entry("alpine-3.21", "alpine/3.21", "Alpine Linux 3.21", "Ultra-Light",
              "Featherweight (~5 MB) musl/BusyBox system. Boots in 0.5s for rapid CLI/microservice tests."),
        entry("kali", "kali", "Kali Linux Rolling", "Security",
              "Pre-configured distribution for security auditing and penetration testing projects."),
        entry("rockylinux-9", "rockylinux/9", "Rocky Linux 9", "Enterprise",
              "100% bug-for-bug RHEL 9 compatible environment for production server testing.")
    };
}

QVariantList Backend::defaultPresets() {
    auto preset = [](const char *id, const char *title, const char *sub, const char *flags, bool x11) {
        QVariantMap map;
        map["id"] = QString::fromLatin1(id);
        map["title"] = QString::fromLatin1(title);
        map["sub"] = QString::fromLatin1(sub);
        map["flags"] = QString::fromLatin1(flags);
        map["x11"] = x11;
        return map;
    };

    return {
        preset("full", "Desktop & Mobile", "GPU + KVM + GUI Bridge", "-p default -p ai-gui", true),
        preset("standard", "Standard AI / Web", "Docker + Isolated OS", "-p default -c security.nesting=true", false),
        preset("vm", "Virtual Machine", "Full KVM Kernel (--vm)", "--vm -p default", false)
    };
}

QStringList Backend::defaultDiskOptions() {
    return {QStringLiteral("20GiB"), QStringLiteral("35GiB"), QStringLiteral("60GiB"), QStringLiteral("100GiB")};
}

QStringList Backend::defaultRamOptions() {
    return {QStringLiteral("4GiB"), QStringLiteral("6GiB"), QStringLiteral("8GiB"), QStringLiteral("12GiB")};
}

QVariantList Backend::defaultCpuOptions() {
    return {2, 4, 6, 8};
}

QString Backend::normalizedPath(const QString &raw, const QString &fallback) const {
    QString path = raw.trimmed();
    if (path.isEmpty())
        path = fallback;
    if (path == QLatin1String("~"))
        path = QDir::homePath();
    else if (path.startsWith(QLatin1String("~/")))
        path = QDir::homePath() + path.mid(1);
    if (!QDir::isAbsolutePath(path))
        path = QDir::homePath() + "/" + path;
    return QDir::cleanPath(path);
}

QString Backend::workspacePathFor(const QString &name) const {
    for (const auto &value : m_environments) {
        const QVariantMap env = value.toMap();
        if (env.value("name").toString() == name) {
            const QString path = env.value("workspacePath").toString();
            if (!path.isEmpty())
                return path;
        }
    }
    return m_workspaceBase + "/" + name;
}

QString Backend::cacheBasePath(const QString &osId) const {
    QString safe = osId;
    safe.replace(QRegularExpression(QStringLiteral("[^A-Za-z0-9._-]")), QStringLiteral("_"));
    return m_osStoragePath + "/" + safe;
}

QVariantMap Backend::presetById(const QString &id) const {
    for (const auto &value : m_presets) {
        const QVariantMap preset = value.toMap();
        if (preset.value("id").toString() == id)
            return preset;
    }
    return {};
}

QVariantMap Backend::osEntryById(const QString &id) const {
    for (const auto &value : m_osEntries) {
        const QVariantMap entry = value.toMap();
        if (entry.value("id").toString() == id)
            return entry;
    }
    return {};
}

void Backend::normalizeDefaults() {
    QStringList presetIds;
    for (const auto &value : m_presets)
        presetIds.append(value.toMap().value("id").toString());
    if (!presetIds.contains(m_defaultPresetId))
        m_defaultPresetId = presetIds.value(0, QStringLiteral("standard"));

    if (!m_diskOptions.contains(m_defaultDisk))
        m_defaultDisk = m_diskOptions.value(0, QStringLiteral("35GiB"));
    if (!m_ramOptions.contains(m_defaultRam))
        m_defaultRam = m_ramOptions.value(0, QStringLiteral("6GiB"));

    bool cpuFound = false;
    for (const auto &value : m_cpuOptions) {
        if (value.toInt() == m_defaultCpu) {
            cpuFound = true;
            break;
        }
    }
    if (!cpuFound)
        m_defaultCpu = m_cpuOptions.value(0, 4).toInt();

    QStringList osIds;
    for (const auto &value : m_osEntries)
        osIds.append(value.toMap().value("id").toString());
    if (!osIds.contains(m_defaultOsId))
        m_defaultOsId = osIds.value(0, QStringLiteral("ubuntu-24.04"));
}

void Backend::loadConfig() {
    QFile file(m_configPath);
    if (!file.open(QIODevice::ReadOnly)) {
        m_firstRun = true;
        return;
    }

    QJsonParseError parseError;
    const QJsonDocument doc = QJsonDocument::fromJson(file.readAll(), &parseError);
    if (parseError.error != QJsonParseError::NoError || !doc.isObject()) {
        m_firstRun = true;
        return;
    }

    const QJsonObject obj = doc.object();
    m_firstRun = !obj.value("firstRunCompleted").toBool(false);

    const QString workspace = obj.value("workspaceBase").toString().trimmed();
    if (!workspace.isEmpty())
        m_workspaceBase = normalizedPath(workspace, m_defaultWorkspaceBase);

    const QString osStorage = obj.value("osStoragePath").toString().trimmed();
    if (!osStorage.isEmpty())
        m_osStoragePath = normalizedPath(osStorage, m_defaultOsStoragePath);

    QVariantList entries;
    QSet<QString> seenIds;
    for (const auto &value : obj.value("osCatalog").toArray()) {
        const QVariantMap source = value.toObject().toVariantMap();
        const QString id = source.value("id").toString().trimmed().toLower();
        if (id.isEmpty() || seenIds.contains(id))
            continue;
        seenIds.insert(id);

        QVariantMap entry;
        entry["id"] = id;
        const QString alias = source.value("alias").toString().trimmed();
        entry["alias"] = alias.isEmpty() ? id : alias;
        const QString name = source.value("name").toString().trimmed();
        entry["name"] = name.isEmpty() ? id : name;
        const QString badge = source.value("badge").toString().trimmed();
        entry["badge"] = badge.isEmpty() ? QStringLiteral("Custom") : badge;
        entry["description"] = source.value("description").toString().trimmed();
        entries.append(entry);
    }
    if (!entries.isEmpty())
        m_osEntries = entries;

    const QVariantList defaults = defaultPresets();
    QVariantList presets;
    for (const auto &value : obj.value("presets").toArray()) {
        const QVariantMap source = value.toObject().toVariantMap();
        const QString id = source.value("id").toString().trimmed();
        const QString title = source.value("title").toString().trimmed();
        if (id.isEmpty() || title.isEmpty())
            continue;

        QVariantMap fallback;
        for (const auto &candidate : defaults) {
            if (candidate.toMap().value("id").toString() == id) {
                fallback = candidate.toMap();
                break;
            }
        }

        QVariantMap preset;
        preset["id"] = id;
        preset["title"] = title;
        preset["sub"] = source.value("sub").toString().trimmed();
        preset["flags"] = source.value("flags").toString().trimmed().isEmpty()
                              ? fallback.value("flags", QStringLiteral("-p default")).toString()
                              : source.value("flags").toString().trimmed();
        preset["x11"] = source.contains("x11") ? source.value("x11").toBool()
                                               : fallback.value("x11", false).toBool();
        presets.append(preset);
    }
    if (!presets.isEmpty())
        m_presets = presets;

    auto jsonStrings = [](const QJsonArray &array) {
        QStringList values;
        for (const auto &value : array) {
            if (value.isString())
                values.append(value.toString());
            else if (value.isDouble())
                values.append(QString::number(value.toInt()));
        }
        return values;
    };

    const QStringList disk = validatedSizeOptions(jsonStrings(obj.value("diskOptions").toArray()));
    if (!disk.isEmpty())
        m_diskOptions = disk;
    const QStringList ram = validatedSizeOptions(jsonStrings(obj.value("ramOptions").toArray()));
    if (!ram.isEmpty())
        m_ramOptions = ram;

    QVariantList cpuValues;
    for (const auto &value : obj.value("cpuOptions").toArray()) {
        if (value.isDouble())
            cpuValues.append(value.toInt());
        else if (value.isString())
            cpuValues.append(value.toString().toInt());
    }
    const QVariantList cpus = validatedCpuOptions(cpuValues);
    if (!cpus.isEmpty())
        m_cpuOptions = cpus;

    const QString defaultPreset = obj.value("defaultPresetId").toString().trimmed();
    if (!defaultPreset.isEmpty())
        m_defaultPresetId = defaultPreset;
    const QString defaultOs = obj.value("defaultOsId").toString().trimmed();
    if (!defaultOs.isEmpty())
        m_defaultOsId = defaultOs;
    const QString defaultDiskValue = obj.value("defaultDisk").toString().trimmed();
    if (!defaultDiskValue.isEmpty())
        m_defaultDisk = defaultDiskValue;
    const QString defaultRamValue = obj.value("defaultRam").toString().trimmed();
    if (!defaultRamValue.isEmpty())
        m_defaultRam = defaultRamValue;

    const QJsonValue defaultCpu = obj.value("defaultCpu");
    if (defaultCpu.isDouble()) {
        const int cores = defaultCpu.toInt();
        if (cores > 0 && cores <= 1024)
            m_defaultCpu = cores;
    }

    if (obj.contains("autoSnapshotEnabled"))
        m_autoSnapshotEnabled = obj.value("autoSnapshotEnabled").toBool(true);
    if (obj.contains("autoSnapshotKeep") && obj.value("autoSnapshotKeep").isDouble())
        m_autoSnapshotKeep = qBound(1, obj.value("autoSnapshotKeep").toInt(5), 50);

    if (obj.contains("watchdogEnabled"))
        m_watchdogEnabled = obj.value("watchdogEnabled").toBool(true);
    if (obj.contains("watchdogDiskWarn") && obj.value("watchdogDiskWarn").isDouble())
        m_watchdogDiskWarn = qBound(50, obj.value("watchdogDiskWarn").toInt(90), 98);
    if (obj.contains("watchdogDiskCritical") && obj.value("watchdogDiskCritical").isDouble())
        m_watchdogDiskCritical = qBound(m_watchdogDiskWarn + 1, obj.value("watchdogDiskCritical").toInt(97), 99);
    if (obj.contains("watchdogAutoStop"))
        m_watchdogAutoStop = obj.value("watchdogAutoStop").toBool(false);
    if (obj.contains("idleStopMinutes") && obj.value("idleStopMinutes").isDouble())
        m_idleStopMinutes = qBound(0, obj.value("idleStopMinutes").toInt(0), 1440);
    if (obj.contains("staleDays") && obj.value("staleDays").isDouble())
        m_staleDays = qBound(1, obj.value("staleDays").toInt(30), 3650);
    if (obj.contains("recordSessions"))
        m_recordSessions = obj.value("recordSessions").toBool(false);

    if (obj.contains("diffExcludes") && obj.value("diffExcludes").isArray()) {
        QStringList loaded;
        for (const auto &v : obj.value("diffExcludes").toArray()) {
            const QString pattern = v.toString().trimmed();
            if (!pattern.isEmpty() && !loaded.contains(pattern))
                loaded.append(pattern);
        }
        m_diffExcludes = loaded;
    }

    normalizeDefaults();
}

bool Backend::saveConfig() const {
    const QString directory = QFileInfo(m_configPath).absolutePath();
    if (!QDir().mkpath(directory))
        return false;

    QJsonObject obj;
    obj["version"] = 1;
    obj["firstRunCompleted"] = !m_firstRun;
    obj["workspaceBase"] = m_workspaceBase;
    obj["osStoragePath"] = m_osStoragePath;
    obj["osCatalog"] = QJsonArray::fromVariantList(m_osEntries);
    obj["presets"] = QJsonArray::fromVariantList(m_presets);
    obj["diskOptions"] = QJsonArray::fromStringList(m_diskOptions);
    obj["ramOptions"] = QJsonArray::fromStringList(m_ramOptions);
    obj["cpuOptions"] = QJsonArray::fromVariantList(m_cpuOptions);
    obj["defaultPresetId"] = m_defaultPresetId;
    obj["defaultOsId"] = m_defaultOsId;
    obj["defaultDisk"] = m_defaultDisk;
    obj["defaultRam"] = m_defaultRam;
    obj["defaultCpu"] = m_defaultCpu;
    obj["autoSnapshotEnabled"] = m_autoSnapshotEnabled;
    obj["autoSnapshotKeep"] = m_autoSnapshotKeep;
    obj["watchdogEnabled"] = m_watchdogEnabled;
    obj["watchdogDiskWarn"] = m_watchdogDiskWarn;
    obj["watchdogDiskCritical"] = m_watchdogDiskCritical;
    obj["watchdogAutoStop"] = m_watchdogAutoStop;
    obj["idleStopMinutes"] = m_idleStopMinutes;
    obj["staleDays"] = m_staleDays;
    obj["recordSessions"] = m_recordSessions;
    obj["diffExcludes"] = QJsonArray::fromStringList(m_diffExcludes);

    QSaveFile file(m_configPath);
    if (!file.open(QIODevice::WriteOnly))
        return false;
    file.write(QJsonDocument(obj).toJson(QJsonDocument::Indented));
    return file.commit();
}

void Backend::rebuildOsCatalog() {
    const QVariantList catalog = catalogForEntries(m_osEntries, m_lastImagesJson, m_downloadingOsId, m_osStoragePath);
    normalizeDefaults();
    if (catalog != m_osCatalog) {
        m_osCatalog = catalog;
        emit osCatalogChanged();
    }
}

QString Backend::completeFirstRun(const QString &systemPath) {
    const QString fallback = m_systemPath.isEmpty() ? defaultSystemPath() : m_systemPath;
    const QString targetRoot = normalizedPath(systemPath, fallback);
    if (!QDir().mkpath(targetRoot))
        return tr("Could not create the system directory: %1").arg(targetRoot);
    const QFileInfo rootInfo(targetRoot);
    if (!rootInfo.isDir() || !rootInfo.isWritable())
        return tr("The system directory is not writable: %1").arg(targetRoot);

    const bool useStructuredLayout = (m_pathMode == PathMode::ProjectDir || m_rememberSystemLocation || !systemPath.trimmed().isEmpty());
    const QString nextWorkspace = useStructuredLayout
                                      ? (targetRoot + QStringLiteral("/workspaces"))
                                      : m_workspaceBase;
    const QString nextOsStorage = useStructuredLayout
                                      ? (targetRoot + QStringLiteral("/os-images"))
                                      : m_osStoragePath;
    const QString nextStateDir = useStructuredLayout
                                     ? (targetRoot + QStringLiteral("/state"))
                                     : m_stateDir;
    const QString nextConfigPath = useStructuredLayout
                                       ? (targetRoot + QStringLiteral("/config.json"))
                                       : m_configPath;

    if (!QDir().mkpath(nextWorkspace))
        return tr("Could not create the workspace directory: %1").arg(nextWorkspace);
    if (!QFileInfo(nextWorkspace).isWritable())
        return tr("The workspace directory is not writable: %1").arg(nextWorkspace);
    if (!QDir().mkpath(nextOsStorage))
        return tr("Could not create the OS storage directory: %1").arg(nextOsStorage);
    if (!QFileInfo(nextOsStorage).isWritable())
        return tr("The OS storage directory is not writable: %1").arg(nextOsStorage);
    if (!QDir().mkpath(nextStateDir))
        return tr("Could not create the state directory: %1").arg(nextStateDir);
    if (!QFileInfo(nextStateDir).isWritable())
        return tr("The state directory is not writable: %1").arg(nextStateDir);

    const bool sysChanged = targetRoot != m_systemPath;
    const bool workspaceChanged = nextWorkspace != m_workspaceBase;
    const bool storageChanged = nextOsStorage != m_osStoragePath;
    const bool configChanged = nextConfigPath != m_configPath;

    m_systemPath = targetRoot;
    m_workspaceBase = nextWorkspace;
    m_osStoragePath = nextOsStorage;
    m_stateDir = nextStateDir;
    m_configPath = nextConfigPath;
    m_defaultWorkspaceBase = nextWorkspace;
    m_defaultOsStoragePath = nextOsStorage;

    if (configChanged && QFileInfo::exists(m_configPath))
        loadConfig();

    m_firstRun = false;
    if (!saveConfig())
        return tr("Could not save the configuration file: %1").arg(m_configPath);

    if (m_rememberSystemLocation && !saveLocationFile(m_systemPath))
        return tr("Could not save the system location preference: %1").arg(defaultLocationFilePath());

    seedProvisionScripts();
    refreshBackups();
    rebuildOsCatalog();
    loadHistorySeenMarker();
    loadOperationHistory();
    if (isGuiSession()) {
        refreshRecordedSessions();
        if (!QStandardPaths::findExecutable("incus").isEmpty())
            refresh();
    }

    emit firstRunChanged();
    if (sysChanged)
        emit systemPathChanged();
    if (workspaceChanged)
        emit workspaceBaseChanged();
    if (storageChanged)
        emit osStoragePathChanged();
    if (configChanged)
        emit configFilePathChanged();
    return {};
}

QString Backend::saveSettings(const QString &workspacePath, const QString &osStoragePath) {
    if (m_busy)
        return tr("Wait for the current operation to finish before changing the configuration.");
    const QString workspace = normalizedPath(workspacePath, m_defaultWorkspaceBase.isEmpty() ? defaultWorkspacePath() : m_defaultWorkspaceBase);
    const QString osStorage = normalizedPath(osStoragePath, m_defaultOsStoragePath.isEmpty() ? defaultOsStoragePathValue() : m_defaultOsStoragePath);

    if (!QDir().mkpath(workspace))
        return tr("Could not create the workspace directory: %1").arg(workspace);
    if (!QFileInfo(workspace).isWritable())
        return tr("The workspace directory is not writable: %1").arg(workspace);
    if (!QDir().mkpath(osStorage))
        return tr("Could not create the OS storage directory: %1").arg(osStorage);
    if (!QFileInfo(osStorage).isWritable())
        return tr("The OS storage directory is not writable: %1").arg(osStorage);

    const bool workspaceChanged = workspace != m_workspaceBase;
    const bool storageChanged = osStorage != m_osStoragePath;
    const bool wasFirstRun = m_firstRun;
    m_workspaceBase = workspace;
    m_osStoragePath = osStorage;
    m_firstRun = false;

    if (!saveConfig())
        return tr("Could not save the configuration file: %1").arg(m_configPath);

    if (workspaceChanged)
        emit workspaceBaseChanged();
    if (storageChanged)
        emit osStoragePathChanged();
    if (wasFirstRun)
        emit firstRunChanged();
    if (storageChanged)
        rebuildOsCatalog();

    m_statusMessage = tr("Configuration saved.");
    emit statusMessageChanged();
    return {};
}

QString Backend::saveOsEntry(const QString &originalId, const QString &id, const QString &alias,
                             const QString &name, const QString &badge, const QString &description) {
    if (m_busy)
        return tr("Wait for the current operation to finish before changing the configuration.");
    static const QRegularExpression idRe(QStringLiteral("^[a-z0-9][a-z0-9._-]{0,63}$"));
    static const QRegularExpression aliasRe(QStringLiteral("^[A-Za-z0-9][A-Za-z0-9._/-]{0,127}$"));

    const QString newId = id.trimmed().toLower();
    if (!idRe.match(newId).hasMatch())
        return tr("OS ID must use lowercase letters, numbers, dots, dashes, or underscores.");

    QString newAlias = alias.trimmed();
    if (newAlias.isEmpty())
        newAlias = newId;
    if (!aliasRe.match(newAlias).hasMatch())
        return tr("Image alias must use letters, numbers, dots, slashes, dashes, or underscores.");

    const QString newName = name.trimmed().isEmpty() ? newAlias : name.trimmed();
    const QString newBadge = badge.trimmed().isEmpty() ? tr("Custom") : badge.trimmed();
    const QString newDescription = description.trimmed();

    int foundIndex = -1;
    for (int i = 0; i < m_osEntries.size(); ++i) {
        const QString entryId = m_osEntries.at(i).toMap().value("id").toString();
        if (!originalId.isEmpty() && entryId == originalId)
            foundIndex = i;
        if (entryId == newId && entryId != originalId)
            return tr("An OS entry with the ID '%1' already exists.").arg(newId);
    }
    if (!originalId.isEmpty() && foundIndex < 0)
        return tr("The OS entry '%1' no longer exists.").arg(originalId);

    QVariantMap entry;
    entry["id"] = newId;
    entry["alias"] = newAlias;
    entry["name"] = newName;
    entry["badge"] = newBadge;
    entry["description"] = newDescription;

    if (foundIndex >= 0)
        m_osEntries[foundIndex] = entry;
    else
        m_osEntries.append(entry);

    if (m_defaultOsId == originalId)
        m_defaultOsId = newId;

    if (!saveConfig())
        return tr("Could not save the configuration file: %1").arg(m_configPath);

    rebuildOsCatalog();
    m_statusMessage = tr("OS entry '%1' saved.").arg(newName);
    emit statusMessageChanged();
    return {};
}

QString Backend::removeOsEntry(const QString &id) {
    if (m_busy)
        return tr("Wait for the current operation to finish before changing the configuration.");
    if (m_osEntries.size() <= 1)
        return tr("Keep at least one OS entry in the catalog.");

    const QString target = id.trimmed();
    int foundIndex = -1;
    for (int i = 0; i < m_osEntries.size(); ++i) {
        if (m_osEntries.at(i).toMap().value("id").toString() == target) {
            foundIndex = i;
            break;
        }
    }
    if (foundIndex < 0)
        return tr("The OS entry '%1' was not found.").arg(target);

    m_osEntries.removeAt(foundIndex);
    normalizeDefaults();
    if (!saveConfig())
        return tr("Could not save the configuration file: %1").arg(m_configPath);

    rebuildOsCatalog();
    m_statusMessage = tr("OS entry removed from the catalog.");
    emit statusMessageChanged();
    return {};
}

QString Backend::resetOsCatalog() {
    if (m_busy)
        return tr("Wait for the current operation to finish before changing the configuration.");
    m_osEntries = defaultOsEntries();
    m_defaultOsId = QStringLiteral("ubuntu-24.04");
    normalizeDefaults();
    if (!saveConfig())
        return tr("Could not save the configuration file: %1").arg(m_configPath);
    rebuildOsCatalog();
    return {};
}

QString Backend::saveCreateOptions(const QString &diskCsv, const QString &ramCsv, const QString &cpuCsv) {
    if (m_busy)
        return tr("Wait for the current operation to finish before changing the configuration.");
    const QStringList rawDisk = parseCsv(diskCsv);
    const QStringList disk = validatedSizeOptions(rawDisk);
    if (disk.isEmpty())
        return tr("Add at least one disk quota value, for example 35GiB.");
    if (disk.size() != rawDisk.size())
        return tr("Disk quota values must look like 35GiB, 60GiB, or 512MiB.");

    const QStringList rawRam = parseCsv(ramCsv);
    const QStringList ram = validatedSizeOptions(rawRam);
    if (ram.isEmpty())
        return tr("Add at least one RAM limit value, for example 6GiB.");
    if (ram.size() != rawRam.size())
        return tr("RAM limit values must look like 4GiB, 6GiB, or 512MiB.");

    QVariantList cpuValues;
    for (const auto &raw : parseCsv(cpuCsv)) {
        bool ok = false;
        const int cores = raw.toInt(&ok);
        if (!ok || cores < 1 || cores > 1024)
            return tr("CPU core values must be whole numbers between 1 and 1024.");
        cpuValues.append(cores);
    }
    const QVariantList cpus = validatedCpuOptions(cpuValues);
    if (cpus.isEmpty())
        return tr("Add at least one CPU core value, for example 4.");

    m_diskOptions = disk;
    m_ramOptions = ram;
    m_cpuOptions = cpus;
    normalizeDefaults();

    if (!saveConfig())
        return tr("Could not save the configuration file: %1").arg(m_configPath);

    emit createOptionsChanged();
    return {};
}

QString Backend::savePreset(const QString &id, const QString &title, const QString &subtitle) {
    if (m_busy)
        return tr("Wait for the current operation to finish before changing the configuration.");
    const QString newTitle = title.trimmed();
    if (newTitle.isEmpty())
        return tr("The preset title cannot be empty.");

    for (int i = 0; i < m_presets.size(); ++i) {
        QVariantMap preset = m_presets.at(i).toMap();
        if (preset.value("id").toString() != id)
            continue;
        preset["title"] = newTitle;
        preset["sub"] = subtitle.trimmed();
        m_presets[i] = preset;
        if (!saveConfig())
            return tr("Could not save the configuration file: %1").arg(m_configPath);
        emit createOptionsChanged();
        return {};
    }
    return tr("The preset '%1' no longer exists.").arg(id);
}

QString Backend::resetCreateOptions() {
    if (m_busy)
        return tr("Wait for the current operation to finish before changing the configuration.");
    m_presets = defaultPresets();
    m_diskOptions = defaultDiskOptions();
    m_ramOptions = defaultRamOptions();
    m_cpuOptions = defaultCpuOptions();
    m_defaultPresetId = QStringLiteral("full");
    m_defaultDisk = QStringLiteral("35GiB");
    m_defaultRam = QStringLiteral("6GiB");
    m_defaultCpu = 4;
    normalizeDefaults();

    if (!saveConfig())
        return tr("Could not save the configuration file: %1").arg(m_configPath);
    emit createOptionsChanged();
    return {};
}

QString Backend::saveAutoSnapshotSettings(bool enabled, int keepCount) {
    if (m_busy)
        return tr("Wait for the current operation to finish before changing the configuration.");
    if (keepCount < 1 || keepCount > 50)
        return tr("Auto-snapshot retention must be between 1 and 50.");

    m_autoSnapshotEnabled = enabled;
    m_autoSnapshotKeep = keepCount;
    if (!saveConfig())
        return tr("Could not save the configuration file: %1").arg(m_configPath);

    bool envUpdated = false;
    for (int i = 0; i < m_environments.size(); ++i) {
        QVariantMap env = m_environments.at(i).toMap();
        if (env.value("autoSnapshotOverride").toString().isEmpty()) {
            env["autoSnapshotEnabled"] = enabled;
            m_environments[i] = env;
            envUpdated = true;
        }
    }

    emit autoSnapshotSettingsChanged();
    if (envUpdated) {
        emit environmentsChanged();
        emit selectedIndexChanged();
    }
    return {};
}

QString Backend::saveWatchdogSettings(bool enabled, int diskWarn, int diskCritical, bool autoStop) {
    if (m_busy)
        return tr("Wait for the current operation to finish before changing the configuration.");
    if (diskWarn < 50 || diskWarn > 98)
        return tr("Disk warning threshold must be between 50% and 98%.");
    if (diskCritical <= diskWarn || diskCritical > 99)
        return tr("Disk critical threshold must be higher than the warning threshold (up to 99%).");

    m_watchdogEnabled = enabled;
    m_watchdogDiskWarn = diskWarn;
    m_watchdogDiskCritical = diskCritical;
    m_watchdogAutoStop = autoStop;
    if (!saveConfig())
        return tr("Could not save the configuration file: %1").arg(m_configPath);

    bool envUpdated = false;
    for (int i = 0; i < m_environments.size(); ++i) {
        QVariantMap env = m_environments.at(i).toMap();
        if (env.value("autoStopOverride").toString().isEmpty()) {
            env["autoStopOnCritical"] = autoStop;
            m_environments[i] = env;
            envUpdated = true;
        }
    }

    emit watchdogSettingsChanged();
    if (envUpdated) {
        emit environmentsChanged();
        emit selectedIndexChanged();
    }
    return {};
}

QVariantList Backend::staleDayOptions() const {
    return {7, 14, 30, 60, 90, 180};
}

QString Backend::saveIdleStopSettings(int minutes) {
    if (m_busy)
        return tr("Wait for the current operation to finish before changing the configuration.");
    if (minutes < 0 || minutes > 1440)
        return tr("Idle auto-stop timeout must be between 0 (off) and 1440 minutes.");

    m_idleStopMinutes = minutes;
    if (!saveConfig())
        return tr("Could not save the configuration file: %1").arg(m_configPath);

    emit idleStopSettingsChanged();
    return {};
}

QString Backend::saveStaleSettings(int days) {
    if (m_busy)
        return tr("Wait for the current operation to finish before changing the configuration.");
    if (days < 1 || days > 3650)
        return tr("Stale threshold must be between 1 and 3650 days.");

    m_staleDays = days;
    if (!saveConfig())
        return tr("Could not save the configuration file: %1").arg(m_configPath);

    emit staleReportSettingsChanged();
    if (!m_staleReport.isEmpty())
        refreshStaleReport();
    return {};
}

QString Backend::saveSessionRecordingSettings(bool enabled) {
    if (m_busy)
        return tr("Wait for the current operation to finish before changing the configuration.");

    m_recordSessions = enabled;
    if (!saveConfig())
        return tr("Could not save the configuration file: %1").arg(m_configPath);

    bool envUpdated = false;
    for (int i = 0; i < m_environments.size(); ++i) {
        QVariantMap env = m_environments.at(i).toMap();
        const QString ov = env.value(QStringLiteral("recordOverride")).toString();
        if (ov.isEmpty() || ov == QLatin1String("inherit")) {
            env["recordEnabled"] = enabled;
            m_environments[i] = env;
            envUpdated = true;
        }
    }

    emit recordSessionsSettingsChanged();
    if (envUpdated) {
        emit environmentsChanged();
        emit selectedIndexChanged();
    }
    return {};
}

QString Backend::saveDiffExcludes(const QString &rawExcludes) {
    if (m_busy)
        return tr("Wait for the current operation to finish before changing the configuration.");
    m_diffExcludes = parseDiffExcludes(rawExcludes);
    if (!saveConfig())
        return tr("Could not save the configuration file: %1").arg(m_configPath);
    emit diffExcludesChanged();
    return {};
}

QUrl Backend::pathToUrl(const QString &path) const {
    QString p = normalizedPath(path, QDir::homePath());
    while (!p.isEmpty() && !QFileInfo::exists(p)) {
        const QString parent = QFileInfo(p).dir().path();
        if (parent == p)
            break;
        p = parent;
    }
    if (p.isEmpty() || !QFileInfo::exists(p))
        p = QDir::homePath();
    return QUrl::fromLocalFile(p);
}

QString Backend::localPath(const QUrl &url) const {
    return url.toLocalFile();
}

QStringList Backend::parseCsv(const QString &text) {
    static const QRegularExpression separator(QStringLiteral("[,;\\n]"));
    QStringList values;
    for (const QString &part : text.split(separator, Qt::SkipEmptyParts)) {
        const QString value = part.trimmed();
        if (!value.isEmpty())
            values.append(value);
    }
    values.removeDuplicates();
    return values;
}

QString Backend::shellQuote(const QString &value) {
    QString escaped = value;
    escaped.replace(QLatin1Char('\''), QStringLiteral("'\\''"));
    return QLatin1Char('\'') + escaped + QLatin1Char('\'');
}

QString Backend::shellArrayWords(const QString &words) {
    const QStringList tokens = words.split(QRegularExpression(QStringLiteral("\\s+")), Qt::SkipEmptyParts);
    QStringList quoted;
    quoted.reserve(tokens.size());
    for (const QString &token : tokens)
        quoted.append(shellQuote(token));
    return quoted.join(QLatin1Char(' '));
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
    updateNetworkStats(true);
    if (!m_demoMode)
        refreshSelectedSnapshotSizes();
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

void Backend::updateNetworkStats(bool restartProbe) {
    const QVariantMap env = selectedEnvironment();
    const QString name = env.value("name").toString();
    if (name != m_netProbeEnv) {
        m_netProbeEnv = name;
        m_netGateway.clear();
        m_netPingMs = -1.0;
        m_netPacketLoss = -1.0;
        restartProbe = true;
    }

    const QString mode = env.value("networkMode").toString();
    const QString ip = env.value("ipv4").toString();
    const bool canProbe = !name.isEmpty() && env.value("running").toBool()
                          && mode != QLatin1String("offline")
                          && !ip.isEmpty() && ip != QStringLiteral("—");

    if (!canProbe) {
        m_netProbeTimer.stop();
        if (m_netProbeProcess && m_netProbeProcess->state() != QProcess::NotRunning)
            m_netProbeProcess->kill();
    } else {
        if (!m_demoMode) {
            if (!m_netProbeTimer.isActive())
                m_netProbeTimer.start();
            const bool probing = m_netProbeProcess && m_netProbeProcess->state() != QProcess::NotRunning;
            if (restartProbe && !probing)
                QTimer::singleShot(150, this, &Backend::refreshNetworkStats);
        }
    }

    QVariantMap probe;
    probe["gateway"] = m_netGateway;
    probe["pingMs"] = m_netPingMs;
    probe["packetLossPercent"] = m_netPacketLoss;
    const QVariantMap stats = networkStatsFromEnvironment(env, probe);
    if (stats != m_networkStats) {
        m_networkStats = stats;
        emit networkStatsChanged();
    }
}

void Backend::refreshNetworkStats() {
    if (m_demoMode)
        return;

    const QVariantMap env = selectedEnvironment();
    const QString name = env.value("name").toString();
    const QString ip = env.value("ipv4").toString();
    if (name.isEmpty() || !env.value("running").toBool()
        || env.value("networkMode").toString() == QLatin1String("offline")
        || ip.isEmpty() || ip == QStringLiteral("—")) {
        m_netProbeTimer.stop();
        return;
    }
    if (m_netProbeProcess && m_netProbeProcess->state() != QProcess::NotRunning)
        return;

    if (!m_netProbeProcess) {
        m_netProbeProcess = new QProcess(this);
        connect(m_netProbeProcess, &QProcess::finished, this, [this](int, QProcess::ExitStatus) {
            const QString output = QString::fromUtf8(m_netProbeProcess->readAllStandardOutput());
            if (selectedEnvironment().value("name").toString() != m_netProbeEnv)
                return;
            const QVariantMap parsed = parseNetworkProbeOutput(output);
            const QString gateway = parsed.value("gateway").toString();
            if (!gateway.isEmpty())
                m_netGateway = gateway;
            m_netPingMs = parsed.value("pingMs").toDouble();
            m_netPacketLoss = parsed.value("packetLossPercent").toDouble();
            updateNetworkStats();
        });
    }

    m_netProbeEnv = name;
    const QString inner = QStringLiteral(
        "gw=\"\"; "
        "if command -v ip >/dev/null 2>&1; then gw=$(ip -4 route show default 2>/dev/null | awk '{print $3; exit}'); fi; "
        "if [ -z \"$gw\" ] && [ -r /proc/net/route ]; then gw=$(awk 'NR>1 && $2==\"00000000\" {print $3; exit}' /proc/net/route); fi; "
        "echo \"GW=$gw\"; "
        "if command -v ping >/dev/null 2>&1; then ping -n -c 3 -W 1 1.1.1.1 2>/dev/null; fi");
    const QString script = QStringLiteral("incus exec ") + shellQuote(name)
                           + QStringLiteral(" -- sh -c ") + shellQuote(inner);
    m_netProbeProcess->setProperty("envName", name);
    m_netProbeProcess->start(QStringLiteral("/bin/sh"), {QStringLiteral("-c"), script});
    QTimer::singleShot(9000, this, [this, name] {
        if (m_netProbeProcess && m_netProbeProcess->state() != QProcess::NotRunning
            && m_netProbeProcess->property("envName").toString() == name)
            m_netProbeProcess->kill();
    });
}

QString Backend::sanitizeName(const QString &rawName) {
    QString s = rawName.trimmed().toLower();
    s.replace(QRegularExpression(QStringLiteral("[^a-z0-9-]+")), QStringLiteral("-"));
    s.replace(QRegularExpression(QStringLiteral("-+")), QStringLiteral("-"));
    s.remove(QRegularExpression(QStringLiteral("^-+|-+$")));
    return s;
}

QVariantList Backend::parseIncusListJson(const QByteArray &json, const QString &workspaceBase,
                                         QHash<QString, qint64> *cpuSnapshotNs, qint64 elapsedWallNs,
                                         bool defaultAutoSnapshot, bool defaultAutoStop,
                                         bool defaultRecordSessions,
                                         QHash<QString, QPair<qint64, qint64>> *netCounterSnapshot) {
    QVariantList result;
    const auto doc = QJsonDocument::fromJson(json);
    if (!doc.isArray())
        return result;

    QHash<QString, qint64> nextCpuSnapshot;
    QHash<QString, QPair<qint64, qint64>> nextNetSnapshot;

    // Sample host per-CPU activity weights from /proc/stat to distribute container load across cores
    static QVector<qint64> prevHostIdle;
    static QVector<qint64> prevHostTotal;
    QVector<qint64> currHostTotal;
    const QVector<qint64> currHostIdle = readHostProcStatIdleAndTotal(&currHostTotal);
    QVector<int> hostCoreWeights;
    if (currHostIdle.size() == prevHostIdle.size() && currHostTotal.size() == prevHostTotal.size()) {
        for (int i = 0; i < currHostTotal.size(); ++i) {
            const qint64 dTotal = currHostTotal.at(i) - prevHostTotal.at(i);
            const qint64 dIdle = currHostIdle.at(i) - prevHostIdle.at(i);
            const int pct = (dTotal > 0) ? qBound(0, static_cast<int>(((dTotal - dIdle) * 100) / dTotal), 100) : 0;
            hostCoreWeights.append(pct);
        }
    }
    if (!currHostTotal.isEmpty()) {
        prevHostIdle = currHostIdle;
        prevHostTotal = currHostTotal;
    }

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
        const int cpuCoreCount = parseCpuCount(cpuLimit);
        const QString memoryLimit = config.value("limits.memory").toString(QStringLiteral("Dynamic"));
        const QString processLimit = config.value("limits.processes").toString(QStringLiteral("Unlimited"));

        const QJsonObject expandedDevices = obj.value("expanded_devices").toObject();
        const QJsonObject devices = obj.value("devices").toObject();
        QString diskLimit = devices.value("root").toObject().value("size").toString();
        if (diskLimit.isEmpty())
            diskLimit = expandedDevices.value("root").toObject().value("size").toString(QStringLiteral("Shared"));
        QString diskReadLimit = devices.value("root").toObject().value("limits.read").toString();
        if (diskReadLimit.isEmpty())
            diskReadLimit = expandedDevices.value("root").toObject().value("limits.read").toString(QStringLiteral("Unlimited"));
        QString diskWriteLimit = devices.value("root").toObject().value("limits.write").toString();
        if (diskWriteLimit.isEmpty())
            diskWriteLimit = expandedDevices.value("root").toObject().value("limits.write").toString(QStringLiteral("Unlimited"));

        QString preset = QStringLiteral("Standard");
        const QJsonArray profiles = obj.value("profiles").toArray();
        for (const auto &p : profiles) {
            if (p.toString() == QLatin1String("ai-gui"))
                preset = QStringLiteral("Desktop & Mobile (GPU + KVM)");
        }
        if (type == QLatin1String("VIRTUAL-MACHINE"))
            preset = QStringLiteral("Full KVM Virtual Machine");

        QString ipv4 = QStringLiteral("—");
        QString hostIface = config.value("volatile.eth0.host_name").toString();
        QString memoryUsage = QStringLiteral("0 MB");
        QString diskUsage = QStringLiteral("0 MB");
        qint64 memBytes = 0;
        qint64 memTotalBytes = parseSizeBytes(memoryLimit);
        qint64 diskBytes = 0;
        qint64 diskTotalBytes = parseSizeBytes(diskLimit);
        int cpuPercent = 0;
        qint64 netRxBytes = 0;
        qint64 netTxBytes = 0;

        const QJsonObject state = obj.value("state").toObject();
        if (!state.isEmpty()) {
            const QJsonObject memObj = state.value("memory").toObject();
            memBytes = memObj.value("usage").toVariant().toLongLong();
            if (memBytes > 0)
                memoryUsage = formatBytes(memBytes);
            const qint64 stateMemTotal = memObj.value("total").toVariant().toLongLong();
            if (memTotalBytes <= 0 && stateMemTotal > 0)
                memTotalBytes = stateMemTotal;

            const QJsonObject rootDiskObj = state.value("disk").toObject().value("root").toObject();
            diskBytes = rootDiskObj.value("usage").toVariant().toLongLong();
            if (diskBytes > 0)
                diskUsage = formatBytes(diskBytes);
            const qint64 stateDiskTotal = rootDiskObj.value("total").toVariant().toLongLong();
            if (diskTotalBytes <= 0 && stateDiskTotal > 0)
                diskTotalBytes = stateDiskTotal;

            const QJsonObject cpuObj = state.value("cpu").toObject();
            const qint64 cpuUsageNs = cpuObj.value("usage").toVariant().toLongLong();
            if (running && cpuUsageNs >= 0) {
                nextCpuSnapshot.insert(name, cpuUsageNs);
                if (cpuSnapshotNs && cpuSnapshotNs->contains(name) && elapsedWallNs > 0) {
                    const qint64 deltaNs = cpuUsageNs - cpuSnapshotNs->value(name);
                    if (deltaNs >= 0) {
                        const double capacityNs = static_cast<double>(elapsedWallNs) * qMax(1, cpuCoreCount);
                        cpuPercent = qBound(0, static_cast<int>(std::llround((static_cast<double>(deltaNs) * 100.0) / capacityNs)), 100);
                    }
                } else {
                    const QString startedStr = state.value("started_at").toString();
                    const QDateTime startedAt = QDateTime::fromString(startedStr, Qt::ISODateWithMs);
                    const qint64 uptimeMs = startedAt.isValid() && startedAt.date().year() > 1970
                                                ? startedAt.msecsTo(QDateTime::currentDateTimeUtc())
                                                : 0;
                    if (uptimeMs > 500 && cpuUsageNs > 0) {
                        const double capacityNs = static_cast<double>(uptimeMs) * 1000000.0 * qMax(1, cpuCoreCount);
                        cpuPercent = qBound(0, static_cast<int>(std::llround((static_cast<double>(cpuUsageNs) * 100.0) / capacityNs)), 100);
                    }
                }
            }

            const QJsonObject net = state.value("network").toObject();
            for (auto it = net.begin(); it != net.end(); ++it) {
                if (it.key() == QLatin1String("lo"))
                    continue;
                const QJsonObject ifaceObj = it.value().toObject();
                const QString ifaceHostName = ifaceObj.value("host_name").toString();
                if (!ifaceHostName.isEmpty() && hostIface.isEmpty())
                    hostIface = ifaceHostName;
                const QJsonObject counters = ifaceObj.value("counters").toObject();
                netRxBytes += counters.value("bytes_received").toVariant().toLongLong();
                netTxBytes += counters.value("bytes_sent").toVariant().toLongLong();
                if (ipv4 != QStringLiteral("—"))
                    continue;
                const QJsonArray addrs = ifaceObj.value("addresses").toArray();
                for (const auto &addrVal : addrs) {
                    const QJsonObject addrObj = addrVal.toObject();
                    if (addrObj.value("family").toString() == QLatin1String("inet") &&
                        addrObj.value("scope").toString() == QLatin1String("global")) {
                        ipv4 = addrObj.value("address").toString();
                        break;
                    }
                }
            }
        }

        qint64 netRxRate = 0;
        qint64 netTxRate = 0;
        if (netCounterSnapshot && running) {
            if (netCounterSnapshot->contains(name) && elapsedWallNs > 0) {
                const QPair<qint64, qint64> previous = netCounterSnapshot->value(name);
                if (netRxBytes >= previous.first)
                    netRxRate = static_cast<qint64>(std::llround(static_cast<double>(netRxBytes - previous.first) * 1e9 / static_cast<double>(elapsedWallNs)));
                if (netTxBytes >= previous.second)
                    netTxRate = static_cast<qint64>(std::llround(static_cast<double>(netTxBytes - previous.second) * 1e9 / static_cast<double>(elapsedWallNs)));
            }
            nextNetSnapshot.insert(name, qMakePair(netRxBytes, netTxBytes));
        }

        const int memoryPercent = (memTotalBytes > 0 && memBytes > 0)
                                      ? qBound(0, static_cast<int>(std::llround((static_cast<double>(memBytes) * 100.0) / static_cast<double>(memTotalBytes))), 100)
                                      : 0;
        const int diskPercent = (diskTotalBytes > 0 && diskBytes > 0)
                                    ? qBound(0, static_cast<int>(std::llround((static_cast<double>(diskBytes) * 100.0) / static_cast<double>(diskTotalBytes))), 100)
                                    : 0;

        QStringList snapshots;
        QVariantMap snapshotSizes;
        qint64 snapshotTotalBytes = 0;
        int autoSnapCount = 0;
        const QJsonArray snapArr = obj.value("snapshots").toArray();
        for (const auto &snapVal : snapArr) {
            const QJsonObject snapObj = snapVal.toObject();
            const QString snapName = snapObj.value("name").toString();
            if (!snapName.isEmpty()) {
                snapshots.append(snapName);
                if (isAutoSnapshotName(snapName))
                    ++autoSnapCount;
                const qint64 snapSize = snapObj.value("size").toVariant().toLongLong();
                if (snapSize > 0) {
                    snapshotSizes.insert(snapName, formatBytes(snapSize));
                    snapshotTotalBytes += snapSize;
                }
            }
        }

        const QString autoSnapOverride = config.value("user.aibox.autosnap").toString().trimmed().toLower();
        const bool envAutoSnap = autoSnapOverride.isEmpty()
                                     ? defaultAutoSnapshot
                                     : (autoSnapOverride == QLatin1String("true"));
        const QString autoStopOverride = config.value("user.aibox.autostop").toString().trimmed().toLower();
        const bool envAutoStop = autoStopOverride.isEmpty()
                                     ? defaultAutoStop
                                     : (autoStopOverride == QLatin1String("true"));
        const QString idleStopOverride = config.value("user.aibox.idlestop").toString().trimmed().toLower();
        const bool idleStopIgnorePorts = (config.value("user.aibox.idlestop_ports").toString().trimmed().toLower() == QLatin1String("true"));
        const QString recordOverride = config.value("user.aibox.record").toString().trimmed().toLower();
        const bool envRecordEnabled = effectiveRecordSessions(defaultRecordSessions, recordOverride);
        bool lastUsedOk = false;
        const qint64 parsedLastUsed = config.value("user.aibox.lastused").toString().trimmed().toLongLong(&lastUsedOk);
        const qint64 lastUsedEpoch = (lastUsedOk && parsedLastUsed > 0) ? parsedLastUsed : 0;
        const bool isTemplate = config.value("user.aibox.template").toString().trimmed().toLower() == QLatin1String("true");
        const bool egressRecording = config.value("user.aibox.egresslog").toString().trimmed().toLower() == QLatin1String("true");

        QString netMode = config.value("user.aibox.netmode").toString().trimmed().toLower();
        const QJsonObject eth0Dev = devices.value("eth0").toObject();
        if (netMode != QLatin1String("online") && netMode != QLatin1String("offline") && netMode != QLatin1String("allowlist")) {
            if (eth0Dev.value("type").toString() == QLatin1String("none"))
                netMode = QStringLiteral("offline");
            else if (!eth0Dev.value("security.acls").toString().isEmpty() &&
                     eth0Dev.value("security.acls").toString() != QStringLiteral("aibox-log-") + name)
                netMode = QStringLiteral("allowlist");
            else
                netMode = QStringLiteral("online");
        }
        const QString netAllowlist = config.value("user.aibox.netallow").toString();

        QString workspacePath = expandedDevices.value("workspace").toObject().value("source").toString();
        if (workspacePath.isEmpty())
            workspacePath = devices.value("workspace").toObject().value("source").toString();
        if (workspacePath.isEmpty())
            workspacePath = workspaceBase + "/" + name;

        QString workspaceMountPath = expandedDevices.value("workspace").toObject().value("path").toString();
        if (workspaceMountPath.isEmpty())
            workspaceMountPath = devices.value("workspace").toObject().value("path").toString();
        if (workspaceMountPath.isEmpty())
            workspaceMountPath = kContainerWorkspacePath;

        const QString wsSnapOverride = config.value("user.aibox.wssnap").toString().trimmed().toLower();
        const bool wsOnBtrfs = isBtrfsFilesystem(workspacePath);
        const bool wsIsSubvolume = isBtrfsSubvolume(workspacePath);
        const bool wsSnapEnabled = wsIsSubvolume && (wsSnapOverride != QLatin1String("false"));
        const QString wsSnapDir = workspaceSnapshotDir(workspacePath, name);
        const QStringList wsSnapshots = listWorkspaceSnapshots(workspacePath, name);
        const QStringList orphanWsSnapshots = findOrphanWorkspaceSnapshots(snapshots, wsSnapshots);
        const bool hasPreSubvolBackup = QFileInfo::exists(workspacePath + QStringLiteral(".pre-subvol"));
        const QStringList secretNames = parseEnvSecretNames(config.value("user.aibox.secrets").toString());

        QVariantList portForwards;
        QVariantList extraMounts;
        for (auto devIt = devices.constBegin(); devIt != devices.constEnd(); ++devIt) {
            const QString devName = devIt.key();
            const QJsonObject devObj = devIt.value().toObject();
            const QString devType = devObj.value("type").toString();
            if (devType == QLatin1String("proxy")) {
                const QString listenAddr = devObj.value("listen").toString();
                const QString connectAddr = devObj.value("connect").toString();
                const int hostPort = listenAddr.section(':', -1).toInt();
                const int guestPort = connectAddr.section(':', -1).toInt();
                if (hostPort > 0) {
                    QVariantMap fw;
                    fw["deviceName"] = devName;
                    fw["listen"] = listenAddr;
                    fw["connect"] = connectAddr;
                    fw["hostPort"] = hostPort;
                    fw["guestPort"] = guestPort > 0 ? guestPort : hostPort;
                    portForwards.append(fw);
                }
            } else if (devType == QLatin1String("disk") &&
                       devName != QLatin1String("root") &&
                       devName != QLatin1String("workspace") &&
                       devName != QLatin1String("x11")) {
                const QString src = devObj.value("source").toString();
                const QString mntPath = devObj.value("path").toString();
                const QString roStr = devObj.value("readonly").toString().trimmed().toLower();
                const bool isRo = (roStr == QLatin1String("true") || roStr == QLatin1String("1"));
                QVariantMap mnt;
                mnt["deviceName"] = devName;
                mnt["source"] = src;
                mnt["path"] = mntPath;
                mnt["readonly"] = isRo;
                extraMounts.append(mnt);
            }
        }

        QVariantMap item;
        item["name"] = name;
        item["status"] = status;
        item["running"] = running;
        item["type"] = type;
        item["osName"] = osName;
        item["preset"] = preset;
        item["cpuLimit"] = cpuLimit == QLatin1String("All") ? tr("All Cores") : QStringLiteral("%1 Cores").arg(cpuLimit);
        item["cpuCores"] = cpuCoreCount;
        item["cpuUsage"] = formatPercent(cpuPercent);
        item["cpuPercent"] = cpuPercent;
        item["cpuCorePercents"] = buildCpuCorePercents(cpuCoreCount, cpuPercent, running, hostCoreWeights);
        item["memoryLimit"] = memoryLimit;
        item["memoryUsage"] = memoryUsage;
        item["memoryPercent"] = memoryPercent;
        item["diskLimit"] = diskLimit;
        item["diskQuotaBytes"] = (diskLimit.compare(QLatin1String("Shared"), Qt::CaseInsensitive) == 0)
                                     ? qint64(0)
                                     : parseSizeBytes(diskLimit);
        item["diskUsage"] = diskUsage;
        item["diskUsageBytes"] = diskBytes;
        item["diskPercent"] = diskPercent;
        item["processLimit"] = processLimit;
        item["diskReadLimit"] = diskReadLimit;
        item["diskWriteLimit"] = diskWriteLimit;
        item["ipv4"] = ipv4;
        item["hostInterface"] = hostIface;
        item["netRxBytes"] = netRxBytes;
        item["netTxBytes"] = netTxBytes;
        item["netRxRate"] = netRxRate;
        item["netTxRate"] = netTxRate;
        item["netRxRateText"] = formatByteRate(netRxRate);
        item["netTxRateText"] = formatByteRate(netTxRate);
        item["netRxTotalText"] = formatByteTotal(netRxBytes);
        item["netTxTotalText"] = formatByteTotal(netTxBytes);
        item["workspacePath"] = workspacePath;
        item["workspaceMountPath"] = workspaceMountPath;
        item["workspaceOnBtrfs"] = wsOnBtrfs;
        item["workspaceIsSubvolume"] = wsIsSubvolume;
        item["workspaceSnapshotOverride"] = wsSnapOverride;
        item["workspaceSnapshotEnabled"] = wsSnapEnabled;
        item["workspaceSnapshotDir"] = wsSnapDir;
        item["workspaceSnapshots"] = wsSnapshots;
        item["orphanWorkspaceSnapshots"] = orphanWsSnapshots;
        item["workspaceSnapshotSizes"] = QVariantMap{};
        item["hasPreSubvolBackup"] = hasPreSubvolBackup;
        item["secretNames"] = secretNames;
        item["portForwards"] = portForwards;
        item["portForwardCount"] = portForwards.size();
        item["extraMounts"] = extraMounts;
        item["extraMountCount"] = extraMounts.size();
        item["snapshotCount"] = snapshots.size();
        item["autoSnapshotCount"] = autoSnapCount;
        item["manualSnapshotCount"] = snapshots.size() - autoSnapCount;
        item["snapshots"] = snapshots;
        item["snapshotSizes"] = snapshotSizes;
        item["snapshotTotalBytes"] = snapshotTotalBytes;
        item["snapshotTotalDisplay"] = snapshotTotalBytes > 0 ? formatBytes(snapshotTotalBytes) : QString();
        item["autoSnapshotOverride"] = autoSnapOverride;
        item["autoSnapshotEnabled"] = envAutoSnap;
        item["autoStopOverride"] = autoStopOverride;
        item["autoStopOnCritical"] = envAutoStop;
        item["idleStopOverride"] = idleStopOverride;
        item["idleStopIgnorePorts"] = idleStopIgnorePorts;
        item["recordOverride"] = recordOverride;
        item["recordEnabled"] = envRecordEnabled;
        item["lastUsedEpoch"] = lastUsedEpoch;
        item["diskAlertLevel"] = 0;
        item["isTemplate"] = isTemplate;
        item["networkMode"] = netMode;
        item["networkAllowlist"] = netAllowlist;
        item["networkBlocked"] = (netMode == QLatin1String("offline"));
        item["egressRecording"] = egressRecording;
        result.append(item);
    }

    if (cpuSnapshotNs)
        *cpuSnapshotNs = nextCpuSnapshot;
    if (netCounterSnapshot)
        *netCounterSnapshot = nextNetSnapshot;

    return result;
}

QVariantMap Backend::parseNetworkProbeOutput(const QString &output) {
    QVariantMap result;
    result["gateway"] = QString();
    result["pingMs"] = -1.0;
    result["packetLossPercent"] = -1.0;

    QString gateway;
    const QStringList lines = output.split(QLatin1Char('\n'));
    for (const QString &rawLine : lines) {
        const QString line = rawLine.trimmed();
        if (line.startsWith(QLatin1String("GW="))) {
            gateway = line.mid(3).trimmed();
            break;
        }
    }

    // /proc/net/route fallback exposes the gateway as little-endian hex.
    static const QRegularExpression hexRe(QStringLiteral("^[0-9A-Fa-f]{8}$"));
    if (hexRe.match(gateway).hasMatch()) {
        bool ok = false;
        const uint value = gateway.toUInt(&ok, 16);
        if (ok) {
            gateway = QStringLiteral("%1.%2.%3.%4")
                          .arg(value & 0xFF)
                          .arg((value >> 8) & 0xFF)
                          .arg((value >> 16) & 0xFF)
                          .arg((value >> 24) & 0xFF);
        }
    }
    result["gateway"] = gateway;

    static const QRegularExpression lossRe(QStringLiteral("(\\d+(?:\\.\\d+)?)%\\s*packet loss"));
    const auto lossMatch = lossRe.match(output);
    if (lossMatch.hasMatch())
        result["packetLossPercent"] = lossMatch.captured(1).toDouble();

    static const QRegularExpression rttRe(QStringLiteral("(?:rtt|round-trip)[^=]*=\\s*[0-9.]+/([0-9.]+)/"));
    const auto rttMatch = rttRe.match(output);
    if (rttMatch.hasMatch())
        result["pingMs"] = rttMatch.captured(1).toDouble();

    return result;
}

QVariantMap Backend::networkStatsFromEnvironment(const QVariantMap &env, const QVariantMap &probe) {
    const QString name = env.value("name").toString();
    const QString networkMode = env.value("networkMode").toString();
    const bool running = env.value("running").toBool();
    QString ip = env.value("ipv4").toString().trimmed();
    if (ip.isEmpty())
        ip = QStringLiteral("—");
    const bool active = !name.isEmpty() && running && networkMode != QLatin1String("offline")
                        && ip != QStringLiteral("—");

    QVariantMap stats;
    stats["active"] = active;
    stats["running"] = running;
    stats["networkMode"] = networkMode.isEmpty() ? QStringLiteral("online") : networkMode;
    stats["ip"] = ip;
    stats["gateway"] = probe.value("gateway").toString().isEmpty() ? QStringLiteral("—")
                                                                   : probe.value("gateway").toString();
    stats["pingMs"] = active ? probe.value("pingMs", -1.0) : -1.0;
    stats["packetLossPercent"] = active ? probe.value("packetLossPercent", -1.0) : -1.0;
    stats["rxRate"] = env.value("netRxRate", qint64(0)).toLongLong();
    stats["txRate"] = env.value("netTxRate", qint64(0)).toLongLong();
    stats["rxTotal"] = env.value("netRxBytes", qint64(0)).toLongLong();
    stats["txTotal"] = env.value("netTxBytes", qint64(0)).toLongLong();
    stats["rxRateText"] = env.value("netRxRateText", formatByteRate(0)).toString();
    stats["txRateText"] = env.value("netTxRateText", formatByteRate(0)).toString();
    stats["rxTotalText"] = env.value("netRxTotalText", formatByteTotal(0)).toString();
    stats["txTotalText"] = env.value("netTxTotalText", formatByteTotal(0)).toString();
    return stats;
}

QVariantList Backend::catalogForEntries(const QVariantList &entries, const QByteArray &imagesJson,
                                        const QString &downloadingId, const QString &storagePath) {
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
    for (const auto &value : entries) {
        const QVariantMap entry = value.toMap();
        const QString id = entry.value("id").toString();
        if (id.isEmpty())
            continue;

        const QString localAlias = QStringLiteral("aibox-") + id;
        const bool inIncus = cachedAliases.contains(localAlias);
        qint64 sizeBytes = cachedAliases.value(localAlias, 0);

        bool savedOnDisk = false;
        if (!storagePath.isEmpty()) {
            const QFileInfo metadata(storagePath + "/" + id);
            const QFileInfo rootfs(storagePath + "/" + id + ".root");
            savedOnDisk = metadata.isFile() && metadata.size() > 0 && rootfs.isFile() && rootfs.size() > 0;
            if (!inIncus && savedOnDisk)
                sizeBytes = metadata.size() + rootfs.size();
        }

        const bool downloaded = inIncus || savedOnDisk;
        QString cacheText = QStringLiteral("Not Cached");
        if (inIncus)
            cacheText = QStringLiteral("Cached · %1").arg(formatBytes(sizeBytes));
        else if (savedOnDisk)
            cacheText = QStringLiteral("Saved · %1").arg(formatBytes(sizeBytes));

        QVariantMap item;
        item["id"] = id;
        item["alias"] = entry.value("alias").toString();
        item["name"] = entry.value("name").toString();
        item["badge"] = entry.value("badge").toString();
        item["description"] = entry.value("description").toString();
        item["downloaded"] = downloaded;
        item["downloading"] = (id == downloadingId);
        item["sizeText"] = cacheText;
        list.append(item);
    }
    return list;
}

QVariantList Backend::buildOsCatalog(const QByteArray &imagesJson, const QString &downloadingId) {
    return catalogForEntries(defaultOsEntries(), imagesJson, downloadingId, {});
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
            const QString storageInfoText = QString::fromUtf8(parts.value(2));
            const QString poolSizeText = QString::fromUtf8(parts.value(3)).trimmed();
            m_lastImagesJson = imgJson;
            StoragePoolInfo poolInfo = parseIncusStorageInfo(storageInfoText);
            if (poolInfo.valid) {
                poolInfo.sizeText = poolSizeText;
                poolInfo.resizable = !poolSizeText.isEmpty();
            }
            setPoolStateForTest(poolInfo, listJson);

            const qint64 elapsedWallNs = m_cpuSampleTimer.isValid() ? m_cpuSampleTimer.nsecsElapsed() : 0;
            m_cpuSampleTimer.start();

            QVariantList newEnvs = parseIncusListJson(listJson, m_workspaceBase, &m_lastCpuUsageNs, elapsedWallNs,
                                                      m_autoSnapshotEnabled, m_watchdogAutoStop, m_recordSessions,
                                                      &m_lastNetCounters);
            applyCachedSnapshotSizes(newEnvs);
            applyFileAccessState(newEnvs);
            for (int i = 0; i < newEnvs.size(); ++i) {
                QVariantMap env = newEnvs.at(i).toMap();
                const int pct = env.value("diskPercent").toInt();
                env["diskAlertLevel"] = (pct >= m_watchdogDiskCritical) ? 2 : ((pct >= m_watchdogDiskWarn) ? 1 : 0);
                newEnvs[i] = env;
            }
            if (newEnvs != m_environments) {
                m_environments = newEnvs;
                m_count = m_environments.size();
                if (m_selectedIndex >= m_environments.size())
                    m_selectedIndex = qMax(0, static_cast<int>(m_environments.size()) - 1);
                emit environmentsChanged();
                emit countChanged();
                emit selectedIndexChanged();
                if (!m_staleReport.isEmpty())
                    refreshStaleReport();
            }
            evaluateAllWatchdogs();
            evaluateIdleAutoStops();
            refreshSelectedSnapshotSizes();
            updateNetworkStats();

            const QVariantList newCatalog = catalogForEntries(m_osEntries, imgJson, m_downloadingOsId, m_osStoragePath);
            if (newCatalog != m_osCatalog) {
                m_osCatalog = newCatalog;
                emit osCatalogChanged();
            }
            normalizeDefaults();
        });
    }

    m_refreshProcess->start(QStringLiteral("/bin/sh"),
                            {QStringLiteral("-c"),
                             QStringLiteral("incus list --format=json 2>/dev/null; printf '\\0'; incus image list --format=json 2>/dev/null; printf '\\0'; (incus storage info default --bytes 2>/dev/null || incus storage info default 2>/dev/null); printf '\\0'; (incus storage get default size 2>/dev/null || true)")});
}

qint64 Backend::readTaskPid() const {
    QFile pidFile(m_stateDir + "/task.pid");
    if (!pidFile.open(QIODevice::ReadOnly))
        return 0;
    return pidFile.readAll().trimmed().toLongLong();
}

bool Backend::isTaskLockActive(qint64 *outPid) const {
    if (m_busy) {
        if (outPid)
            *outPid = readTaskPid();
        return true;
    }
    const qint64 pid = readTaskPid();
    if (pid > 0 && ::kill(static_cast<pid_t>(pid), 0) == 0) {
        if (outPid)
            *outPid = pid;
        return true;
    }
    if (outPid)
        *outPid = 0;
    return false;
}

bool Backend::acquireCliTaskLock(const QString &description, const QString &envName) {
    if (isTaskLockActive())
        return false;
    QDir().mkpath(m_stateDir);

    QJsonObject meta;
    meta["description"] = description;
    meta["envName"] = envName;
    meta["targetEnvName"] = envName;
    meta["startedAt"] = QDateTime::currentDateTime().toString(Qt::ISODate);
    QFile metaFile(m_stateDir + QStringLiteral("/task.meta"));
    if (metaFile.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        metaFile.write(QJsonDocument(meta).toJson(QJsonDocument::Compact));
        metaFile.close();
    }

    QFile pidFile(m_stateDir + QStringLiteral("/task.pid"));
    if (!pidFile.open(QIODevice::WriteOnly | QIODevice::Truncate))
        return false;
    pidFile.write(QByteArray::number(static_cast<qint64>(::getpid())) + '\n');
    pidFile.close();
    return true;
}

void Backend::releaseCliTaskLock() {
    clearTaskFiles();
}

bool Backend::refreshSync(QString *errorOut) {
    QProcess proc;
    proc.start(QStringLiteral("incus"), {QStringLiteral("list"), QStringLiteral("--format=json")});
    if (!proc.waitForFinished(10000) || proc.exitStatus() != QProcess::NormalExit || proc.exitCode() != 0) {
        if (errorOut) {
            const QString err = QString::fromUtf8(proc.readAllStandardError()).trimmed();
            *errorOut = err.isEmpty() ? QStringLiteral("Incus daemon is not reachable.") : err;
        }
        return false;
    }
    const QByteArray rawJson = proc.readAllStandardOutput();
    m_environments = parseIncusListJson(rawJson, m_workspaceBase, nullptr, 0,
                                          m_autoSnapshotEnabled, m_watchdogAutoStop, m_recordSessions);
    applyFileAccessState(m_environments);
    m_count = m_environments.size();
    return true;
}

void Backend::clearTaskFiles() {
    QFile::remove(m_stateDir + "/task.pid");
    QFile::remove(m_stateDir + "/task.meta");
    QFile::remove(m_stateDir + "/task.sh");
}

void Backend::handleProcessOutput(const QByteArray &chunk) {
    QString text = redactSecrets(QString::fromUtf8(chunk), allSecretValues());
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

QString Backend::historyDirPath() const {
    return m_stateDir + QStringLiteral("/history");
}

QString Backend::historySeenMarkerPath() const {
    return m_stateDir + QStringLiteral("/history-seen");
}

void Backend::loadHistorySeenMarker() {
    QFile f(historySeenMarkerPath());
    if (!f.open(QIODevice::ReadOnly)) {
        m_historySeenId.clear();
        return;
    }
    m_historySeenId = QString::fromUtf8(f.readAll()).trimmed();
}

QByteArray Backend::truncateLogTail(const QByteArray &data, qint64 maxBytes) {
    if (maxBytes <= 0 || data.size() <= maxBytes)
        return data;
    QByteArray tail = data.right(maxBytes);
    const int newlineIdx = tail.indexOf('\n');
    if (newlineIdx > 0 && newlineIdx < tail.size() - 1)
        tail = tail.mid(newlineIdx + 1);
    return QByteArray("... [truncated] ...\n") + tail;
}

int Backend::recentFailedHistoryCount() const {
    int count = 0;
    for (const auto &v : m_operationHistory) {
        const QVariantMap entry = v.toMap();
        if (entry.value("status").toString() != QLatin1String("failed"))
            continue;
        if (!m_historySeenId.isEmpty()
            && entry.value("id").toString() <= m_historySeenId)
            continue;
        ++count;
    }
    return count;
}

void Backend::loadOperationHistory() {
    QVariantList loaded;
    const QDir dir(historyDirPath());
    if (dir.exists()) {
        const QFileInfoList files = dir.entryInfoList({QStringLiteral("*.json")}, QDir::Files, QDir::Name | QDir::Reversed);
        for (const QFileInfo &fi : files) {
            QFile f(fi.absoluteFilePath());
            if (!f.open(QIODevice::ReadOnly))
                continue;
            const QJsonObject obj = QJsonDocument::fromJson(f.readAll()).object();
            if (obj.value("id").toString().isEmpty())
                continue;
            loaded.append(obj.toVariantMap());
        }
    }
    if (loaded != m_operationHistory) {
        m_operationHistory = loaded;
        emit operationHistoryChanged();
    }
}

void Backend::recordHistory(const QString &description, const QString &envName, const QString &status,
                            int exitCode, const QString &startedAtIso, const QByteArray &logBytes) {
    recordOperationHistory(description, envName, status, exitCode, startedAtIso, logBytes);
}

void Backend::recordOperationHistory(const QString &description, const QString &envName, const QString &status,
                                     int exitCode, const QString &startedAtIso, const QByteArray &logBytes,
                                     const QString &reason) {
    if (description.isEmpty())
        return;
    const QString dirPath = historyDirPath();
    if (!QDir().mkpath(dirPath))
        return;

    const QDateTime now = QDateTime::currentDateTime();
    const QDateTime started = QDateTime::fromString(startedAtIso, Qt::ISODate);
    const qint64 durationMs = started.isValid() ? qMax<qint64>(0, started.msecsTo(now)) : 0;

    static int seq = 0;
    const QString id = QStringLiteral("%1-%2").arg(now.toString(QStringLiteral("yyyyMMdd-HHmmss-zzz"))).arg(++seq, 3, 10, QLatin1Char('0'));
    const QString logName = id + QStringLiteral(".log");

    const QStringList secretVals = allSecretValues();
    const QString safeDescription = redactSecrets(description, secretVals);
    const QByteArray safeLogBytes = redactSecretsBytes(logBytes, secretVals);

    QFile outLog(dirPath + QLatin1Char('/') + logName);
    if (outLog.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        outLog.write(truncateLogTail(safeLogBytes));
        outLog.close();
    }

    QJsonObject entry;
    entry["id"] = id;
    entry["timestamp"] = now.toString(QStringLiteral("yyyy-MM-dd HH:mm:ss"));
    entry["startedAt"] = startedAtIso;
    entry["durationMs"] = durationMs;
    entry["description"] = safeDescription;
    entry["envName"] = envName;
    entry["status"] = status;
    entry["exitCode"] = exitCode;
    entry["logFile"] = logName;
    if (!reason.isEmpty())
        entry["reason"] = reason;

    QFile outMeta(dirPath + QLatin1Char('/') + id + QStringLiteral(".json"));
    if (outMeta.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        outMeta.write(QJsonDocument(entry).toJson(QJsonDocument::Compact));
        outMeta.close();
    }

    // Prune oldest beyond m_historyMaxEntries
    const QDir dir(dirPath);
    const QFileInfoList allJson = dir.entryInfoList({QStringLiteral("*.json")}, QDir::Files, QDir::Name);
    if (allJson.size() > m_historyMaxEntries) {
        const int removeCount = allJson.size() - m_historyMaxEntries;
        for (int i = 0; i < removeCount; ++i) {
            const QString base = allJson.at(i).completeBaseName();
            QFile::remove(dirPath + QLatin1Char('/') + base + QStringLiteral(".json"));
            QFile::remove(dirPath + QLatin1Char('/') + base + QStringLiteral(".log"));
        }
    }

    loadOperationHistory();
}

QString Backend::readOperationLog(const QString &id) const {
    const QString safeId = QFileInfo(id).fileName();
    if (safeId.isEmpty())
        return {};
    if (m_demoMode) {
        return QStringLiteral("[demo] Task %1 completed cleanly.\n@@STAGE:100:Done.").arg(safeId);
    }
    QFile f(historyDirPath() + QLatin1Char('/') + safeId + QStringLiteral(".log"));
    if (!f.open(QIODevice::ReadOnly))
        return tr("Log file is not available.");
    return QString::fromUtf8(f.readAll());
}

void Backend::clearOperationHistory() {
    const QDir dir(historyDirPath());
    if (dir.exists()) {
        for (const QFileInfo &fi : dir.entryInfoList(QDir::Files))
            QFile::remove(fi.absoluteFilePath());
    }
    QFile::remove(historySeenMarkerPath());
    m_historySeenId.clear();
    if (!m_operationHistory.isEmpty()) {
        m_operationHistory.clear();
        emit operationHistoryChanged();
    }
}

void Backend::acknowledgeOperationHistory() {
    if (m_operationHistory.isEmpty())
        return;
    const QString newestId = m_operationHistory.first().toMap().value("id").toString();
    if (newestId.isEmpty() || newestId == m_historySeenId)
        return;
    QFile f(historySeenMarkerPath());
    if (f.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        f.write(newestId.toUtf8());
        f.close();
    } else {
        return;
    }
    m_historySeenId = newestId;
    emit operationHistoryChanged();
}

void Backend::attachExistingTask() {
    QFile metaFile(m_stateDir + "/task.meta");
    if (!metaFile.open(QIODevice::ReadOnly))
        return;

    const QJsonObject meta = QJsonDocument::fromJson(metaFile.readAll()).object();
    const QString desc = meta.value("description").toString();
    const QString envName = meta.value("envName").toString();
    const QString targetEnv = meta.value("targetEnvName").toString();
    const QString startedAt = meta.value("startedAt").toString();
    const QString dlOsId = meta.value("downloadingOsId").toString();
    const QString taskReason = meta.value("reason").toString();

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
    QByteArray rawLog;
    QFile logFile(m_stateDir + "/task.log");
    if (logFile.open(QIODevice::ReadOnly)) {
        rawLog = logFile.readAll();
        m_logOffset = rawLog.size();
        handleProcessOutput(rawLog);
    }

    if (stillRunning && !m_hasExitCode) {
        setBusy(true, desc);
        emit taskProgressChanged();
        m_taskWatchTimer.start();
    } else {
        // Task already finished while the GUI was closed!
        const int exitCode = m_hasExitCode ? m_lastExitCode : 1;
        const QStringList secretVals = allSecretValues();
        if (!secretVals.isEmpty() && !rawLog.isEmpty()) {
            const QByteArray scrubbed = redactSecretsBytes(rawLog, secretVals);
            if (scrubbed != rawLog) {
                QFile out(m_stateDir + "/task.log");
                if (out.open(QIODevice::WriteOnly | QIODevice::Truncate))
                    out.write(scrubbed);
            }
        }
        recordOperationHistory(desc, !targetEnv.isEmpty() ? targetEnv : envName,
                               exitCode == 0 ? QStringLiteral("success") : QStringLiteral("failed"),
                               exitCode, startedAt, rawLog, taskReason);
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

        QString desc = m_taskStage;
        QString targetEnv = finishedEnv;
        QString startedAt;
        QString taskReason;
        QFile metaFile(m_stateDir + "/task.meta");
        if (metaFile.open(QIODevice::ReadOnly)) {
            const QJsonObject meta = QJsonDocument::fromJson(metaFile.readAll()).object();
            if (!meta.value("description").toString().isEmpty())
                desc = meta.value("description").toString();
            if (!meta.value("targetEnvName").toString().isEmpty())
                targetEnv = meta.value("targetEnvName").toString();
            startedAt = meta.value("startedAt").toString();
            taskReason = meta.value("reason").toString();
        }
        QByteArray fullLog;
        if (logFile.isOpen()) {
            logFile.seek(0);
            fullLog = logFile.readAll();
            logFile.close();
            const QStringList secretVals = allSecretValues();
            if (!secretVals.isEmpty() && !fullLog.isEmpty()) {
                const QByteArray scrubbed = redactSecretsBytes(fullLog, secretVals);
                if (scrubbed != fullLog) {
                    QFile out(m_stateDir + "/task.log");
                    if (out.open(QIODevice::WriteOnly | QIODevice::Truncate))
                        out.write(scrubbed);
                }
            }
        }
        recordOperationHistory(desc, targetEnv,
                               exitCode == 0 ? QStringLiteral("success") : QStringLiteral("failed"),
                               exitCode, startedAt, fullLog, taskReason);

        clearTaskFiles();
        m_downloadingOsId.clear();
        m_pendingEnvName.clear();
        setBusy(false, {});
        refresh();
        refreshBackups();
        if (!m_staleReport.isEmpty())
            refreshStaleReport();

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

    QString desc = m_taskStage;
    QString targetEnv = m_pendingEnvName;
    QString startedAt;
    QString taskReason;
    QFile metaFile(m_stateDir + "/task.meta");
    if (metaFile.open(QIODevice::ReadOnly)) {
        const QJsonObject meta = QJsonDocument::fromJson(metaFile.readAll()).object();
        if (!meta.value("description").toString().isEmpty())
            desc = meta.value("description").toString();
        if (!meta.value("targetEnvName").toString().isEmpty())
            targetEnv = meta.value("targetEnvName").toString();
        startedAt = meta.value("startedAt").toString();
        taskReason = meta.value("reason").toString();
    }
    QByteArray fullLog;
    QFile logFile(m_stateDir + "/task.log");
    if (logFile.open(QIODevice::ReadOnly)) {
        fullLog = logFile.readAll();
        logFile.close();
        const QStringList secretVals = allSecretValues();
        if (!secretVals.isEmpty() && !fullLog.isEmpty()) {
            const QByteArray scrubbed = redactSecretsBytes(fullLog, secretVals);
            if (scrubbed != fullLog) {
                QFile out(m_stateDir + "/task.log");
                if (out.open(QIODevice::WriteOnly | QIODevice::Truncate))
                    out.write(scrubbed);
            }
        }
    }
    recordOperationHistory(desc, targetEnv, QStringLiteral("cancelled"), 130, startedAt, fullLog, taskReason);

    const QString cleanupName = m_pendingEnvName;
    clearTaskFiles();
    m_downloadingOsId.clear();
    m_pendingEnvName.clear();
    setBusy(false, tr("Operation cancelled."));

    if (!cleanupName.isEmpty()) {
        const QString wsPath = workspacePathFor(cleanupName);
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

void Backend::runShellTask(const QString &description, const QString &script, const QString &createdEnvName,
                           const QString &targetEnvName, const QString &reason) {
    if (m_busy)
        return;
    if (isTaskLockActive()) {
        setError(tr("Another AIBox task is currently running."));
        return;
    }

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
    meta["targetEnvName"] = !targetEnvName.isEmpty() ? targetEnvName : createdEnvName;
    meta["downloadingOsId"] = m_downloadingOsId;
    meta["startedAt"] = QDateTime::currentDateTime().toString(Qt::ISODate);
    if (!reason.isEmpty())
        meta["reason"] = reason;

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

Backend::WatchdogDecision Backend::evaluateWatchdog(int percent, int previousLevel, int warnThreshold,
                                                    int criticalThreshold, bool autoStopEnabled) {
    WatchdogDecision decision;
    const int warn = qBound(50, warnThreshold, 98);
    const int crit = qBound(warn + 1, criticalThreshold, 100);
    const int prev = qBound(0, previousLevel, 2);

    if (percent >= crit) {
        decision.newLevel = 2;
    } else if (percent >= warn) {
        // Keep level 2 until dropping 5 points below critical (hysteresis)
        if (prev == 2 && percent >= crit - 5)
            decision.newLevel = 2;
        else
            decision.newLevel = 1;
    } else {
        // Keep level 1/2 if within 5-point hysteresis band below warn threshold
        if (prev >= 1 && percent >= warn - 5)
            decision.newLevel = 1;
        else
            decision.newLevel = 0;
    }

    decision.shouldNotify = (decision.newLevel > prev);
    decision.shouldAutoStop = (decision.newLevel == 2 && autoStopEnabled);
    return decision;
}

void Backend::evaluateAllWatchdogs() {
    if (!m_watchdogEnabled || m_demoMode)
        return;

    const bool canNotify = isGuiSession() &&
                           !QStandardPaths::findExecutable(QStringLiteral("notify-send")).isEmpty();

    QString autoStopTarget;
    for (int i = 0; i < m_environments.size(); ++i) {
        QVariantMap env = m_environments.at(i).toMap();
        const QString name = env.value("name").toString();
        const bool running = env.value("running").toBool();
        const int diskPct = env.value("diskPercent").toInt();
        const bool envAutoStop = env.value("autoStopOnCritical").toBool();

        const int prevDisk = m_watchdogDiskLevel.value(name, 0);
        const WatchdogDecision d = evaluateWatchdog(diskPct, prevDisk, m_watchdogDiskWarn, m_watchdogDiskCritical,
                                                    running && envAutoStop);
        m_watchdogDiskLevel.insert(name, d.newLevel);

        if (env.value("diskAlertLevel").toInt() != d.newLevel) {
            env["diskAlertLevel"] = d.newLevel;
            m_environments[i] = env;
        }

        if (d.shouldNotify && canNotify) {
            const bool isCrit = (d.newLevel == 2);
            QString msg = isCrit
                              ? tr("Disk critical (%1%) in '%2' (%3 of %4).")
                                    .arg(diskPct)
                                    .arg(name, env.value("diskUsage").toString(), env.value("diskLimit").toString())
                              : tr("Disk warning (%1%) in '%2' (%3 of %4).")
                                    .arg(diskPct)
                                    .arg(name, env.value("diskUsage").toString(), env.value("diskLimit").toString());
            if (d.shouldAutoStop)
                msg += QStringLiteral(" ") + tr("Auto-stopping container to protect host.");
            QProcess::startDetached(QStringLiteral("notify-send"),
                                    {QStringLiteral("-u"), isCrit ? QStringLiteral("critical") : QStringLiteral("normal"),
                                     QStringLiteral("-a"), QStringLiteral("AIBox"), QStringLiteral("AIBox Watchdog"), msg});
        }

        if (d.shouldAutoStop && autoStopTarget.isEmpty() && !m_busy)
            autoStopTarget = name;

        // Sustained CPU / RAM streak alerts (warn after 3 consecutive high polls when running)
        if (running) {
            const int cpuPct = env.value("cpuPercent").toInt();
            const int cpuStreak = (cpuPct >= m_watchdogCpuWarn) ? (m_watchdogCpuHighStreak.value(name, 0) + 1) : 0;
            m_watchdogCpuHighStreak.insert(name, cpuStreak);
            const int prevCpuLvl = m_watchdogCpuLevel.value(name, 0);
            if (cpuStreak >= 3 && prevCpuLvl == 0) {
                m_watchdogCpuLevel.insert(name, 1);
                if (canNotify) {
                    QProcess::startDetached(QStringLiteral("notify-send"),
                                            {QStringLiteral("-a"), QStringLiteral("AIBox"),
                                             QStringLiteral("AIBox Watchdog"),
                                             tr("Sustained high CPU (%1%) in '%2'.").arg(cpuPct).arg(name)});
                }
            } else if (cpuPct < m_watchdogCpuWarn - 10) {
                m_watchdogCpuLevel.insert(name, 0);
            }

            const int ramPct = env.value("memoryPercent").toInt();
            const int ramStreak = (ramPct >= m_watchdogRamWarn) ? (m_watchdogRamHighStreak.value(name, 0) + 1) : 0;
            m_watchdogRamHighStreak.insert(name, ramStreak);
            const int prevRamLvl = m_watchdogRamLevel.value(name, 0);
            if (ramStreak >= 3 && prevRamLvl == 0) {
                m_watchdogRamLevel.insert(name, 1);
                if (canNotify) {
                    QProcess::startDetached(QStringLiteral("notify-send"),
                                            {QStringLiteral("-a"), QStringLiteral("AIBox"),
                                             QStringLiteral("AIBox Watchdog"),
                                             tr("High memory pressure (%1%) in '%2'.").arg(ramPct).arg(name)});
                }
            } else if (ramPct < m_watchdogRamWarn - 10) {
                m_watchdogRamLevel.insert(name, 0);
            }
        } else {
            m_watchdogCpuHighStreak.insert(name, 0);
            m_watchdogCpuLevel.insert(name, 0);
            m_watchdogRamHighStreak.insert(name, 0);
            m_watchdogRamLevel.insert(name, 0);
        }
    }

    if (!autoStopTarget.isEmpty()) {
        runShellTask(tr("Watchdog auto-stopping '%1' (disk >= %2%)").arg(autoStopTarget).arg(m_watchdogDiskCritical),
                     QStringLiteral("echo '@@STAGE:50:Stopping container on critical disk quota...'; "
                                    "incus stop %1 --timeout=15 || incus stop %1 --force; "
                                    "echo '@@STAGE:100:Stopped by watchdog.'")
                         .arg(shellQuote(autoStopTarget)),
                     {}, autoStopTarget);
    }
}

void Backend::evaluateIdleAutoStops() {
    if (m_demoMode)
        return;

    bool anyTracking = false;
    for (const auto &v : m_environments) {
        const QVariantMap env = v.toMap();
        const int eff = effectiveIdleStopMinutes(m_idleStopMinutes,
                                                 parseIdleStopOverride(env.value("idleStopOverride").toString()));
        if (env.value("running").toBool() && !env.value("isTemplate").toBool() && eff > 0) {
            anyTracking = true;
            break;
        }
    }
    if (!anyTracking) {
        m_idleSamples.clear();
        m_idleWarned.clear();
        return;
    }

    QSet<QString> activeExecEnvs;
    const QString user = qEnvironmentVariable("USER").trimmed();
    QProcess pgrepProc;
    QStringList pgrepArgs;
    if (!user.isEmpty())
        pgrepArgs << QStringLiteral("-u") << user;
    pgrepArgs << QStringLiteral("-af") << QStringLiteral("incus exec");
    pgrepProc.start(QStringLiteral("pgrep"), pgrepArgs);
    if (pgrepProc.waitForFinished(800) && pgrepProc.exitCode() == 0)
        activeExecEnvs = parseActiveIncusExecEnvs(QString::fromUtf8(pgrepProc.readAllStandardOutput()));

    const qint64 nowMs = QDateTime::currentMSecsSinceEpoch();
    const qint64 cutoffMs = nowMs - 25LL * 60LL * 60LL * 1000LL;
    const bool canNotify = isGuiSession() &&
                           !QStandardPaths::findExecutable(QStringLiteral("notify-send")).isEmpty();

    QString activeTaskEnv;
    if (m_busy) {
        QFile metaFile(m_stateDir + QStringLiteral("/task.meta"));
        if (metaFile.open(QIODevice::ReadOnly))
            activeTaskEnv = QJsonDocument::fromJson(metaFile.readAll()).object().value(QStringLiteral("targetEnvName")).toString();
    }

    QString idleStopTarget;
    for (const auto &v : m_environments) {
        const QVariantMap env = v.toMap();
        const QString name = env.value("name").toString();
        const bool running = env.value("running").toBool();
        const bool isTemplate = env.value("isTemplate").toBool();
        const int portCount = env.value("portForwardCount").toInt();
        const bool ignorePorts = env.value("idleStopIgnorePorts").toBool();
        const int thresholdMin = effectiveIdleStopMinutes(
            m_idleStopMinutes,
            parseIdleStopOverride(env.value("idleStopOverride").toString()));
        const bool busyOnThisEnv = m_busy && (activeTaskEnv == name || m_pendingEnvName == name);
        const bool hasExec = activeExecEnvs.contains(name);

        if (!running || isTemplate || thresholdMin <= 0 || busyOnThisEnv || hasExec ||
            (portCount > 0 && !ignorePorts)) {
            m_idleSamples.remove(name);
            m_idleWarned.remove(name);
            continue;
        }

        auto &samples = m_idleSamples[name];
        samples.append(IdleCpuSample{nowMs, env.value("cpuPercent").toInt()});
        while (!samples.isEmpty() && samples.first().timestampMs < cutoffMs)
            samples.removeFirst();

        IdleDecisionInput in;
        in.running = running;
        in.isTemplate = isTemplate;
        in.busyOnThisEnv = busyOnThisEnv;
        in.hasActiveExecSession = hasExec;
        in.portForwardCount = portCount;
        in.ignorePortForwards = ignorePorts;
        in.thresholdMinutes = thresholdMin;
        in.nowMs = nowMs;
        in.warningAlreadySent = m_idleWarned.contains(name);
        in.samples = samples;

        const IdleDecisionResult res = evaluateIdleDecision(in);
        if (res.shouldWarnOneMin) {
            m_idleWarned.insert(name);
            if (canNotify) {
                QProcess::startDetached(QStringLiteral("notify-send"),
                                        {QStringLiteral("-a"), QStringLiteral("AIBox"),
                                         QStringLiteral("AIBox Idle Auto-Stop"),
                                         tr("'%1' has been idle and will stop in 1 minute.").arg(name)});
            }
        }
        if (res.shouldStopNow && idleStopTarget.isEmpty() && !m_busy)
            idleStopTarget = name;
    }

    if (!idleStopTarget.isEmpty()) {
        m_idleSamples.remove(idleStopTarget);
        m_idleWarned.remove(idleStopTarget);
        runShellTask(tr("Idle auto-stopping '%1' (reason: idle)").arg(idleStopTarget),
                     stopEnvironmentScript(idleStopTarget, true),
                     {}, idleStopTarget, QStringLiteral("idle"));
    }
}

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

void Backend::setEnvironmentAutoStop(const QString &name, bool enabled) {
    if (name.isEmpty() || m_busy)
        return;
    runShellTask(enabled ? tr("Enabling critical disk auto-stop for '%1'").arg(name)
                         : tr("Disabling critical disk auto-stop for '%1'").arg(name),
                 QStringLiteral("echo '@@STAGE:50:Updating watchdog auto-stop policy...'; "
                                "incus config set %1 user.aibox.autostop %2; "
                                "echo '@@STAGE:100:Updated.'")
                     .arg(shellQuote(name), enabled ? QStringLiteral("true") : QStringLiteral("false")),
                 {}, name);
}

void Backend::setEnvironmentIdleStop(const QString &name, const QString &value, bool allowWithPortForwards) {
    if (name.isEmpty() || m_busy)
        return;
    const QString norm = value.trimmed().toLower();
    const IdleStopOverride parsed = parseIdleStopOverride(norm);
    const QString stored = (parsed.mode == IdleStopOverride::Off)
                               ? QStringLiteral("off")
                               : (parsed.mode == IdleStopOverride::Minutes
                                      ? QString::number(parsed.minutes)
                                      : QStringLiteral("inherit"));
    m_idleSamples.remove(name);
    m_idleWarned.remove(name);
    runShellTask(tr("Updating idle auto-stop policy for '%1'").arg(name),
                 QStringLiteral("echo '@@STAGE:50:Updating idle auto-stop policy...'; "
                                "incus config set %1 user.aibox.idlestop %2 user.aibox.idlestop_ports %3; "
                                "echo '@@STAGE:100:Updated.'")
                     .arg(shellQuote(name), shellQuote(stored),
                          allowWithPortForwards ? QStringLiteral("true") : QStringLiteral("false")),
                 {}, name);
}

bool Backend::isValidProcessLimit(const QString &value) {
    const QString trimmed = value.trimmed();
    if (trimmed.isEmpty() || trimmed.compare(QLatin1String("unlimited"), Qt::CaseInsensitive) == 0)
        return true;
    static const QRegularExpression re(QStringLiteral("^[1-9][0-9]*$"));
    if (!re.match(trimmed).hasMatch())
        return false;
    bool ok = false;
    const int count = trimmed.toInt(&ok);
    return ok && count >= 10 && count <= 1000000;
}

bool Backend::isValidIoLimit(const QString &value) {
    const QString trimmed = value.trimmed();
    if (trimmed.isEmpty() || trimmed.compare(QLatin1String("unlimited"), Qt::CaseInsensitive) == 0)
        return true;
    static const QRegularExpression re(QStringLiteral("^[1-9][0-9]*(?:B|kB|KB|MB|GB|KiB|MiB|GiB|iops)$"));
    return re.match(trimmed).hasMatch();
}

void Backend::createEnvironment(const QString &rawName, const QString &osId, const QString &presetId,
                                const QString &diskGiB, const QString &ramGiB, int cpuCores,
                                const QString &processLimit, const QString &ioLimit,
                                const QStringList &provisionBundleIds) {
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

    static const QRegularExpression sizeRe(QStringLiteral("^\\d+(?:\\.\\d+)?(?:GiB|MiB|TiB|GB|MB|TB)$"),
                                           QRegularExpression::CaseInsensitiveOption);
    if (!sizeRe.match(diskGiB.trimmed()).hasMatch()) {
        setError(tr("The disk quota is invalid. Use a value such as 35GiB."));
        return;
    }
    if (!sizeRe.match(ramGiB.trimmed()).hasMatch()) {
        setError(tr("The RAM limit is invalid. Use a value such as 6GiB."));
        return;
    }

    const QString cleanProc = processLimit.trimmed().compare(QLatin1String("unlimited"), Qt::CaseInsensitive) == 0
                                  ? QString()
                                  : processLimit.trimmed();
    const QString cleanIo = ioLimit.trimmed().compare(QLatin1String("unlimited"), Qt::CaseInsensitive) == 0
                                ? QString()
                                : ioLimit.trimmed();
    if (!isValidProcessLimit(cleanProc)) {
        setError(tr("The process limit is invalid. Use an integer >= 10 (e.g. 512) or leave unlimited."));
        return;
    }
    if (!isValidIoLimit(cleanIo)) {
        setError(tr("The disk I/O limit is invalid. Use a rate such as 200MB or 2500iops."));
        return;
    }

    QVariantMap osEntry = osEntryById(osId);
    if (osEntry.isEmpty())
        osEntry = m_osEntries.value(0).toMap();
    if (osEntry.isEmpty()) {
        setError(tr("There is no OS in the catalog. Add one in Configuration first."));
        return;
    }

    QVariantMap preset = presetById(presetId);
    if (preset.isEmpty())
        preset = presetById(m_defaultPresetId);
    if (preset.isEmpty())
        preset = m_presets.value(0).toMap();
    if (preset.isEmpty()) {
        setError(tr("There is no capability preset. Add one in Configuration first."));
        return;
    }

    const QString entryId = osEntry.value("id").toString();
    const QString osAlias = osEntry.value("alias").toString();
    const QString localAlias = QStringLiteral("aibox-") + entryId;
    const QString hostWorkspace = m_workspaceBase + "/" + name;
    const QString cacheBase = cacheBasePath(entryId);
    const QString flags = preset.value("flags").toString();
    const bool isVm = flags.contains(QLatin1String("--vm"));
    const QString provisionSnippet = buildProvisionShellSnippet(provisionBundleIds, entryId, osAlias, provisionDirPath());

    const QString script = createEnvironmentScript(name,
                                                   localAlias,
                                                   QStringLiteral("images:") + osAlias,
                                                   hostWorkspace,
                                                   cacheBase,
                                                   flags,
                                                   ramGiB,
                                                   diskGiB,
                                                   cpuCores,
                                                   preset.value("x11").toBool(),
                                                   cleanProc,
                                                   cleanIo,
                                                   isVm,
                                                   provisionSnippet);

    runShellTask(tr("Creating environment '%1'").arg(name), script, name, name);
}

QString Backend::createEnvironmentScript(const QString &name, const QString &localAlias, const QString &remoteImage,
                                         const QString &workspace, const QString &cacheBase,
                                         const QString &profileFlags, const QString &ramGiB, const QString &diskGiB,
                                         int cpuCores, bool x11, const QString &processLimit, const QString &ioLimit,
                                         bool isVm, const QString &provisionSnippet) {
    QString rootOpts = QStringLiteral("size=%1").arg(diskGiB.trimmed());
    const QString cleanIo = ioLimit.trimmed();
    if (!cleanIo.isEmpty() && cleanIo.compare(QLatin1String("unlimited"), Qt::CaseInsensitive) != 0)
        rootOpts += QStringLiteral(",limits.read=%1,limits.write=%1").arg(cleanIo);

    QString quotaArgs = QStringLiteral("-c limits.cpu=%1 -c limits.memory=%2 -d root,%3")
                            .arg(QString::number(qMax(1, cpuCores)), ramGiB.trimmed(), rootOpts);
    const QString cleanProc = processLimit.trimmed();
    if (!isVm && !cleanProc.isEmpty() && cleanProc.compare(QLatin1String("unlimited"), Qt::CaseInsensitive) != 0)
        quotaArgs += QStringLiteral(" -c limits.processes=%1").arg(cleanProc);

    return QStringLiteral(R"sh(
set -euo pipefail
NAME=%1
LOCAL_ALIAS=%2
REMOTE_IMAGE=%3
WORKSPACE=%4
WS_PATH=%12
CACHE=%5
CACHE_ROOT=%6
PROFILE_ARGS=(%7)
QUOTA_ARGS=(%8)
X11=%9

echo "@@STAGE:8:Step 1/4 — Preparing shared workspace directory..."
%11
if command -v setfacl >/dev/null 2>&1; then
    ACL_USER="${SUDO_USER:-${USER:-$(id -un)}}"
    setfacl -R -m "u:${ACL_USER}:rwx" -m "m::rwx" -d -m "u:${ACL_USER}:rwx" -d -m "m::rwx" "$WORKSPACE" 2>/dev/null || true
fi

if incus image info "$LOCAL_ALIAS" >/dev/null 2>&1; then
    echo "@@STAGE:60:Step 2/4 — Using cached OS image ($LOCAL_ALIAS)..."
    SOURCE_IMAGE="$LOCAL_ALIAS"
elif [ -s "$CACHE" ] && [ -s "$CACHE_ROOT" ]; then
    echo "@@STAGE:30:Step 2/4 — Importing OS image from OS storage..."
    incus image import "$CACHE" "$CACHE_ROOT" --alias "$LOCAL_ALIAS"
    SOURCE_IMAGE="$LOCAL_ALIAS"
else
    echo "@@STAGE:12:Step 2/4 — Downloading OS image ($REMOTE_IMAGE)..."
    script -q -e -c "incus image copy \"$REMOTE_IMAGE\" local: --alias \"$LOCAL_ALIAS\" --auto-update" /dev/null
    SOURCE_IMAGE="$LOCAL_ALIAS"
fi

echo "@@STAGE:72:Step 3/4 — Launching isolated OS container & applying quotas..."
script -q -e -c "incus launch \"$SOURCE_IMAGE\" \"$NAME\" ${PROFILE_ARGS[*]} ${QUOTA_ARGS[*]}" /dev/null

echo "@@STAGE:86:Step 4/4 — Mounting $WS_PATH & waiting for network IP..."
sleep 1

if ! incus config device add "$NAME" workspace disk source="$WORKSPACE" path="$WS_PATH" shift=true >/dev/null 2>&1; then
    incus config device add "$NAME" workspace disk source="$WORKSPACE" path="$WS_PATH" >/dev/null
fi

if [[ "$X11" == "1" ]] && [[ -d /tmp/.X11-unix ]]; then
    if command -v xhost >/dev/null 2>&1 && [[ -n "${DISPLAY:-}" ]]; then
        xhost +local: >/dev/null 2>&1 || true
    fi
    incus config device add "$NAME" x11 disk source=/tmp/.X11-unix path=/tmp/.X11-unix >/dev/null 2>&1 || true
fi

incus exec "$NAME" -- sh -c '
    mkdir -p "$1" && chmod 777 "$1" || true
    if [ -f /etc/dhcpcd.conf ] && ! grep -q "^noarp" /etc/dhcpcd.conf; then
        echo "noarp" >> /etc/dhcpcd.conf
        pkill -HUP dhcpcd 2>/dev/null || udhcpc -i eth0 -n -q 2>/dev/null || true
    fi
' aibox "$WS_PATH" >/dev/null 2>&1 || true

for _ in 1 2 3 4 5; do
    if incus list "$NAME" -f csv -c 4 | grep -q "[0-9]"; then
        break
    fi
    sleep 1
done
%10
echo "@@STAGE:100:Environment ready!"
)sh")
                               .arg(shellQuote(name),
                                    shellQuote(localAlias),
                                    shellQuote(remoteImage),
                                    shellQuote(workspace),
                                    shellQuote(cacheBase),
                                    shellQuote(cacheBase + ".root"),
                                    shellArrayWords(profileFlags),
                                    shellArrayWords(quotaArgs),
                                    x11 ? QStringLiteral("1") : QStringLiteral("0"),
                                    provisionSnippet,
                                    workspaceCreateSnippet(workspace))
                                .arg(shellQuote(kContainerWorkspacePath));
}

QVariantList Backend::templates() const {
    QVariantList result;
    for (const auto &v : m_environments) {
        const QVariantMap env = v.toMap();
        if (env.value("isTemplate").toBool())
            result.append(env);
    }
    return result;
}

QVariantMap Backend::environmentByName(const QString &name) const {
    for (const auto &v : m_environments) {
        const QVariantMap env = v.toMap();
        if (env.value("name").toString() == name)
            return env;
    }
    return {};
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

QStringList Backend::parseAllowlistEntries(const QString &rawText) {
    static const QRegularExpression separator(QStringLiteral("[,;\\s]+"));
    static const QRegularExpression safeToken(QStringLiteral("^[a-z0-9.:/-]+$"));
    QStringList result;
    for (const QString &part : rawText.split(separator, Qt::SkipEmptyParts)) {
        const QString item = part.trimmed().toLower();
        if (item.isEmpty() || !safeToken.match(item).hasMatch())
            continue;
        if (!result.contains(item))
            result.append(item);
    }
    return result;
}

QStringList Backend::resolveAllowlistCidrs(const QStringList &entries, const HostResolver &resolver,
                                           QStringList *warnings) {
    HostResolver effectiveResolver = resolver;
    if (!effectiveResolver) {
        effectiveResolver = [](const QString &host) {
            QStringList ips;
            const QHostInfo info = QHostInfo::fromName(host);
            for (const QHostAddress &addr : info.addresses()) {
                if (addr.protocol() == QAbstractSocket::IPv4Protocol)
                    ips.append(addr.toString());
            }
            return ips;
        };
    }

    QStringList cidrs;
    auto appendUnique = [&cidrs](const QString &cidr) {
        if (!cidrs.contains(cidr))
            cidrs.append(cidr);
    };

    for (const QString &entry : entries) {
        if (entry.contains('/')) {
            const auto subnet = QHostAddress::parseSubnet(entry);
            if (!subnet.first.isNull() && subnet.first.protocol() == QAbstractSocket::IPv4Protocol && subnet.second >= 0) {
                appendUnique(QStringLiteral("%1/%2").arg(subnet.first.toString()).arg(subnet.second));
            } else if (warnings) {
                warnings->append(tr("Invalid CIDR: %1").arg(entry));
            }
            continue;
        }

        const QHostAddress directAddr(entry);
        if (!directAddr.isNull()) {
            if (directAddr.protocol() == QAbstractSocket::IPv4Protocol) {
                appendUnique(directAddr.toString() + QStringLiteral("/32"));
            } else if (warnings) {
                warnings->append(tr("Only IPv4 addresses are supported: %1").arg(entry));
            }
            continue;
        }

        static const QRegularExpression hostRe(QStringLiteral("^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)*$"));
        if (!hostRe.match(entry).hasMatch()) {
            if (warnings)
                warnings->append(tr("Invalid host entry: %1").arg(entry));
            continue;
        }

        const QStringList resolved = effectiveResolver(entry);
        int addedForHost = 0;
        for (const QString &ipStr : resolved) {
            const QHostAddress addr(ipStr);
            if (addr.isNull() || addr.protocol() != QAbstractSocket::IPv4Protocol)
                continue;
            // Block DNS rebinding to loopback, link-local, host, or LAN unless entered explicitly as IP/CIDR.
            if (isPrivateOrReservedIpv4(addr))
                continue;
            appendUnique(addr.toString() + QStringLiteral("/32"));
            ++addedForHost;
        }
        if (addedForHost == 0 && warnings)
            warnings->append(tr("No public IPv4 addresses resolved for '%1'.").arg(entry));
    }
    return cidrs;
}

static QString aclApplyShellSnippet(const QString &name, const QString &allowlistRaw,
                                    const QStringList &resolvedCidrs, bool egressLogging = false) {
    const QString qName = QStringLiteral("'") + QString(name).replace('\'', QStringLiteral("'\\''")) + QStringLiteral("'");
    const QString aclName = QStringLiteral("aibox-") + name;
    const QString qAcl = QStringLiteral("'") + QString(aclName).replace('\'', QStringLiteral("'\\''")) + QStringLiteral("'");
    const QString qLogAcl = QStringLiteral("'") + QString(QStringLiteral("aibox-log-") + name).replace('\'', QStringLiteral("'\\''")) + QStringLiteral("'");
    const QString qAllow = QStringLiteral("'") + QString(allowlistRaw).replace('\'', QStringLiteral("'\\''")) + QStringLiteral("'");

    QStringList lines;
    lines << QStringLiteral("incus config device remove %1 eth0 >/dev/null 2>&1 || true").arg(qName);
    lines << QStringLiteral("incus network acl delete %1 >/dev/null 2>&1 || true").arg(qAcl);
    lines << QStringLiteral("incus network acl delete %1 >/dev/null 2>&1 || true").arg(qLogAcl);
    lines << QStringLiteral("incus network acl create %1").arg(qAcl);
    for (const QString &cidr : resolvedCidrs) {
        const QString qCidr = QStringLiteral("'") + QString(cidr).replace('\'', QStringLiteral("'\\''")) + QStringLiteral("'");
        if (egressLogging)
            lines << QStringLiteral("incus network acl rule add %1 egress action=allow destination=%2 state=logged").arg(qAcl, qCidr);
        else
            lines << QStringLiteral("incus network acl rule add %1 egress action=allow destination=%2").arg(qAcl, qCidr);
    }
    if (egressLogging) {
        lines << QStringLiteral("incus config device override %1 eth0 security.acls=%2 security.acls.default.egress.action=drop security.acls.default.egress.logged=true")
                     .arg(qName, qAcl);
        lines << QStringLiteral("incus config set %1 user.aibox.egresslog true").arg(qName);
    } else {
        lines << QStringLiteral("incus config device override %1 eth0 security.acls=%2 security.acls.default.egress.action=drop")
                     .arg(qName, qAcl);
        lines << QStringLiteral("incus config unset %1 user.aibox.egresslog >/dev/null 2>&1 || true").arg(qName);
    }
    lines << QStringLiteral("incus config set %1 user.aibox.netmode allowlist").arg(qName);
    lines << QStringLiteral("incus config set %1 user.aibox.netallow %2").arg(qName, qAllow);
    return lines.join(QLatin1Char('\n'));
}

QString Backend::networkControlScript(const QString &name, const QString &mode,
                                      const QString &allowlistRaw, const QStringList &resolvedCidrs,
                                      bool egressLogging) {
    const QString qName = shellQuote(name);
    const QString qAcl = shellQuote(QStringLiteral("aibox-") + name);
    const QString qLogAcl = shellQuote(QStringLiteral("aibox-log-") + name);
    QStringList lines;
    lines << QStringLiteral("set -euo pipefail");

    if (mode == QLatin1String("offline")) {
        lines << QStringLiteral("echo '@@STAGE:40:Disabling network interface...'");
        lines << QStringLiteral("incus config device remove %1 eth0 >/dev/null 2>&1 || true").arg(qName);
        lines << QStringLiteral("incus config device add %1 eth0 none").arg(qName);
        lines << QStringLiteral("incus network acl delete %1 >/dev/null 2>&1 || true").arg(qAcl);
        lines << QStringLiteral("incus network acl delete %1 >/dev/null 2>&1 || true").arg(qLogAcl);
        lines << QStringLiteral("incus config set %1 user.aibox.netmode offline").arg(qName);
        lines << QStringLiteral("incus config unset %1 user.aibox.egresslog >/dev/null 2>&1 || true").arg(qName);
        lines << QStringLiteral("echo '@@STAGE:100:Network disabled.'");
    } else if (mode == QLatin1String("allowlist")) {
        lines << QStringLiteral("echo '@@STAGE:40:Applying network allowlist ACL...'");
        lines << aclApplyShellSnippet(name, allowlistRaw, resolvedCidrs, egressLogging);
        lines << QStringLiteral("echo '@@STAGE:100:Network allowlist active.'");
    } else {
        lines << QStringLiteral("echo '@@STAGE:40:%1'")
                     .arg(egressLogging ? QStringLiteral("Enabling online egress logging ACL...")
                                        : QStringLiteral("Restoring full network access..."));
        lines << QStringLiteral("incus config device remove %1 eth0 >/dev/null 2>&1 || true").arg(qName);
        lines << QStringLiteral("incus network acl delete %1 >/dev/null 2>&1 || true").arg(qAcl);
        lines << QStringLiteral("incus network acl delete %1 >/dev/null 2>&1 || true").arg(qLogAcl);
        lines << QStringLiteral("incus config unset %1 user.aibox.netmode >/dev/null 2>&1 || true").arg(qName);
        lines << QStringLiteral("incus config unset %1 user.aibox.netallow >/dev/null 2>&1 || true").arg(qName);
        if (egressLogging) {
            lines << QStringLiteral("incus network acl create %1").arg(qLogAcl);
            lines << QStringLiteral("incus config device override %1 eth0 security.acls=%2 security.acls.default.egress.action=allow security.acls.default.egress.logged=true")
                         .arg(qName, qLogAcl);
            lines << QStringLiteral("incus config set %1 user.aibox.egresslog true").arg(qName);
        } else {
            lines << QStringLiteral("incus config unset %1 user.aibox.egresslog >/dev/null 2>&1 || true").arg(qName);
        }
        lines << QStringLiteral("echo '@@STAGE:100:%1'")
                     .arg(egressLogging ? QStringLiteral("Egress logging active.")
                                        : QStringLiteral("Network online."));
    }
    return lines.join(QLatin1Char('\n'));
}

QString Backend::buildAllowlistRefreshIfNeeded(const QString &name) const {
    const QVariantMap env = environmentByName(name);
    if (env.value("networkMode").toString() != QLatin1String("allowlist"))
        return {};
    const QString rawAllow = env.value("networkAllowlist").toString();
    const QStringList entries = parseAllowlistEntries(rawAllow);
    const QStringList cidrs = resolveAllowlistCidrs(entries);
    if (entries.isEmpty() || cidrs.isEmpty())
        return {};
    return aclApplyShellSnippet(name, rawAllow, cidrs, env.value("egressRecording").toBool());
}

QString Backend::cloneEnvironmentScript(const QString &sourceName, const QString &newName,
                                        const QString &workspace, const QString &diskGiB,
                                        const QString &ramGiB, int cpuCores, bool startAfter,
                                        const QString &netMode, const QString &netAllowlist,
                                        const QStringList &resolvedCidrs, const QString &processLimit,
                                        const QString &ioLimit, bool isVm) {
    QString quotaLines;
    if (cpuCores > 0)
        quotaLines += QStringLiteral("incus config set %1 limits.cpu %2\n").arg(shellQuote(newName), shellQuote(QString::number(cpuCores)));
    if (!ramGiB.trimmed().isEmpty())
        quotaLines += QStringLiteral("incus config set %1 limits.memory %2\n").arg(shellQuote(newName), shellQuote(ramGiB.trimmed()));
    const QString cleanProc = processLimit.trimmed();
    if (!isVm && !cleanProc.isEmpty() && cleanProc.compare(QLatin1String("unlimited"), Qt::CaseInsensitive) != 0)
        quotaLines += QStringLiteral("incus config set %1 limits.processes %2\n").arg(shellQuote(newName), shellQuote(cleanProc));
    if (!diskGiB.trimmed().isEmpty())
        quotaLines += QStringLiteral("incus config device set %1 root size=%2 >/dev/null 2>&1 || incus config device override %1 root size=%2\n")
                          .arg(shellQuote(newName), shellQuote(diskGiB.trimmed()));
    const QString cleanIo = ioLimit.trimmed();
    if (!cleanIo.isEmpty() && cleanIo.compare(QLatin1String("unlimited"), Qt::CaseInsensitive) != 0) {
        quotaLines += QStringLiteral("incus config device set %1 root limits.read=%2 limits.write=%2 >/dev/null 2>&1 || incus config device override %1 root limits.read=%2 limits.write=%2\n")
                          .arg(shellQuote(newName), shellQuote(cleanIo));
    }

    QString netSnippet;
    if (netMode == QLatin1String("allowlist")) {
        netSnippet = aclApplyShellSnippet(newName, netAllowlist, resolvedCidrs, false);
    } else if (netMode == QLatin1String("offline")) {
        netSnippet = QStringLiteral("incus config device remove %1 eth0 >/dev/null 2>&1 || true\nincus config device add %1 eth0 none >/dev/null 2>&1 || true")
                         .arg(shellQuote(newName));
    } else {
        netSnippet = QStringLiteral("incus config device remove %1 eth0 >/dev/null 2>&1 || true").arg(shellQuote(newName));
    }
    netSnippet += QStringLiteral("\nincus config unset %1 user.aibox.egresslog >/dev/null 2>&1 || true").arg(shellQuote(newName));

    QString startSnippet;
    if (startAfter) {
        startSnippet = QStringLiteral(R"sh(
echo "@@STAGE:85:Starting cloned environment..."
incus start "$DST"
for _ in 1 2 3 4 5; do
    if incus list "$DST" -f csv -c 4 | grep -q "[0-9]"; then
        break
    fi
    sleep 1
done
)sh");
    }

    return QStringLiteral(R"sh(
set -euo pipefail
SRC=%1
DST=%2
WORKSPACE=%3
WS_PATH=%8

echo "@@STAGE:12:Step 1/3 — Preparing fresh workspace directory..."
%7
if command -v setfacl >/dev/null 2>&1; then
    ACL_USER="${SUDO_USER:-${USER:-$(id -un)}}"
    setfacl -R -m "u:${ACL_USER}:rwx" -m "m::rwx" -d -m "u:${ACL_USER}:rwx" -d -m "m::rwx" "$WORKSPACE" 2>/dev/null || true
fi

echo "@@STAGE:35:Step 2/3 — Cloning environment with Btrfs copy-on-write..."
script -q -e -c "incus copy \"$SRC\" \"$DST\" --stateless" /dev/null

echo "@@STAGE:70:Step 3/3 — Configuring isolated workspace & limits..."
incus config unset "$DST" user.aibox.template >/dev/null 2>&1 || true
incus config device remove "$DST" workspace >/dev/null 2>&1 || true
if ! incus config device add "$DST" workspace disk source="$WORKSPACE" path="$WS_PATH" shift=true >/dev/null 2>&1; then
    incus config device add "$DST" workspace disk source="$WORKSPACE" path="$WS_PATH" >/dev/null
fi
%4
%5
%6
echo "@@STAGE:100:Clone ready!"
)sh")
        .arg(shellQuote(sourceName),
             shellQuote(newName),
             shellQuote(workspace),
             quotaLines.trimmed(),
             netSnippet,
             startSnippet,
             workspaceCreateSnippet(workspace),
             shellQuote(kContainerWorkspacePath));
}

QString Backend::prepareEnvironmentScript(const QString &name) {
    const QString autoSnap = buildAutoSnapshotIfNeeded(name);
    const QString allowRefresh = buildAllowlistRefreshIfNeeded(name);

    return QStringLiteral(R"sh(
if command -v xhost >/dev/null 2>&1 && [[ -n "${DISPLAY:-}" ]]; then
    xhost +local: >/dev/null 2>&1 || true
fi
%2
%3
%4
STATUS=$(incus info %1 | awk '/^Status:/ {print $2}')
if [[ "$STATUS" != "RUNNING" ]]; then
    incus start %1
    sleep 1
fi
incus config set %1 user.aibox.lastused "$(date +%s)" >/dev/null 2>&1 || true
)sh").arg(shellQuote(name), autoSnap, allowRefresh, workspaceMountMigrationScript(name));
}

QStringList Backend::enterIncusExecArgs(const QString &name, const QString &remoteEnvPath) const {
    const QString displayVal = qEnvironmentVariable("DISPLAY").trimmed().isEmpty()
                                   ? QStringLiteral(":0")
                                   : qEnvironmentVariable("DISPLAY");
    const QString termVal = qEnvironmentVariable("TERM").trimmed().isEmpty()
                                ? QStringLiteral("xterm-256color")
                                : qEnvironmentVariable("TERM");
    const QString shellCmd = remoteEnvPath.isEmpty()
                                 ? QStringLiteral("if command -v bash >/dev/null 2>&1; then exec bash -l; else exec sh; fi")
                                 : QStringLiteral("if [ -f %1 ]; then set -a; . %1; set +a; rm -f %1; fi; if command -v bash >/dev/null 2>&1; then exec bash -l; else exec sh; fi")
                                       .arg(shellQuote(remoteEnvPath));
    return {
        QStringLiteral("exec"),
        name,
        QStringLiteral("--cwd"),
        containerWorkspacePathFor(name),
        QStringLiteral("--env"),
        QStringLiteral("DISPLAY=") + displayVal,
        QStringLiteral("--env"),
        QStringLiteral("TERM=") + termVal,
        QStringLiteral("--"),
        QStringLiteral("sh"),
        QStringLiteral("-c"),
        shellCmd
    };
}

QStringList Backend::execIncusCommandArgs(const QString &name, const QStringList &cmd,
                                          const QString &remoteEnvPath) const {
    QStringList args{
        QStringLiteral("exec"),
        name,
        QStringLiteral("--cwd"),
        containerWorkspacePathFor(name),
        QStringLiteral("--")
    };
    if (remoteEnvPath.isEmpty()) {
        args.append(cmd);
    } else {
        args << QStringLiteral("sh")
             << QStringLiteral("-c")
             << QStringLiteral("AIBOX_ENV=\"$1\"; shift; if [ -f \"$AIBOX_ENV\" ]; then set -a; . \"$AIBOX_ENV\"; set +a; rm -f \"$AIBOX_ENV\"; fi; exec \"$@\"")
             << QStringLiteral("aibox-exec")
             << remoteEnvPath;
        args.append(cmd);
    }
    return args;
}

QString Backend::startEnvironmentScript(const QString &name) {
    const QString autoSnap = buildAutoSnapshotIfNeeded(name);
    const QString allowRefresh = buildAllowlistRefreshIfNeeded(name);
    QString script = QStringLiteral("set -euo pipefail\necho '@@STAGE:20:Preparing environment...'\n");
    if (!autoSnap.isEmpty())
        script += autoSnap + QLatin1Char('\n');
    if (!allowRefresh.isEmpty())
        script += allowRefresh + QLatin1Char('\n');
    script += QStringLiteral("echo '@@STAGE:55:Starting container...'\n%2\nincus start %1 && sleep 1\n"
                             "incus config set %1 user.aibox.lastused \"$(date +%s)\" >/dev/null 2>&1 || true\n"
                             "echo '@@STAGE:100:Started.'")
                  .arg(shellQuote(name), workspaceMountMigrationScript(name));
    return script;
}

QString Backend::stopEnvironmentScript(const QString &name, bool includeAutoSnapshot) {
    QString script = QStringLiteral("set -euo pipefail\n");
    if (includeAutoSnapshot) {
        const QString autoSnap = buildAutoSnapshotIfNeeded(name);
        if (!autoSnap.isEmpty())
            script += autoSnap + QLatin1Char('\n');
    }
    script += QStringLiteral("echo '@@STAGE:40:Stopping container...'\nincus stop %1\necho '@@STAGE:100:Stopped.'")
                  .arg(shellQuote(name));
    return script;
}

namespace {
void launchCommandInTerminal(const QString &shellCmd) {
    if (!QStandardPaths::findExecutable(QStringLiteral("xdg-terminal-exec")).isEmpty()) {
        QProcess::startDetached(QStringLiteral("xdg-terminal-exec"),
                                {QStringLiteral("sh"), QStringLiteral("-c"), shellCmd});
    } else if (!QStandardPaths::findExecutable(QStringLiteral("foot")).isEmpty()) {
        QProcess::startDetached(QStringLiteral("foot"),
                                {QStringLiteral("sh"), QStringLiteral("-c"), shellCmd});
    }
}
} // namespace

void Backend::enterEnvironment(const QString &name) {
    if (name.isEmpty())
        return;
    const QVariantMap env = environmentByName(name);
    if (env.value("isTemplate").toBool()) {
        setError(tr("Templates cannot be entered directly. Create a new environment from this template first."));
        return;
    }

    const QString script = prepareEnvironmentScript(name);

    QProcess preProc;
    preProc.setProcessChannelMode(QProcess::MergedChannels);
    preProc.start(QStringLiteral("/bin/bash"), {QStringLiteral("-c"), script});
    preProc.waitForFinished(60000);
    const QByteArray preOut = preProc.readAllStandardOutput();
    if (preOut.contains("Warning:")) {
        recordOperationHistory(tr("Auto-snapshot warning for '%1'").arg(name), name,
                               QStringLiteral("warning"), preProc.exitCode(),
                               QDateTime::currentDateTimeUtc().toString(Qt::ISODate), preOut);
    }

    QString remoteEnvPath;
    QString secretErr;
    if (!pushEnvironmentSecretsToContainer(name, &remoteEnvPath, &secretErr)) {
        setError(secretErr);
        refresh();
        return;
    }

    QString enterCmd;
    const QString wsMountArg = QStringLiteral(
        "\"$(incus config device get %1 workspace path 2>/dev/null || echo %2)\"")
        .arg(shellQuote(name), shellQuote(kContainerWorkspacePath));
    if (remoteEnvPath.isEmpty()) {
        enterCmd = QStringLiteral(
            "incus exec %1 --cwd %2 --env DISPLAY=\"${DISPLAY:-:0}\" --env TERM=\"${TERM:-xterm-256color}\" -- bash -l || "
            "incus exec %1 --cwd %2 -- sh").arg(shellQuote(name), wsMountArg);
    } else {
        enterCmd = QStringLiteral(
            "incus exec %1 --cwd %3 --env DISPLAY=\"${DISPLAY:-:0}\" --env TERM=\"${TERM:-xterm-256color}\" -- "
            "sh -c 'F=\"$1\"; if [ -f \"$F\" ]; then set -a; . \"$F\"; set +a; rm -f \"$F\"; fi; exec bash -l' aibox-enter %2 || "
            "incus exec %1 --cwd %3 -- "
            "sh -c 'F=\"$1\"; if [ -f \"$F\" ]; then set -a; . \"$F\"; set +a; rm -f \"$F\"; fi; exec sh' aibox-enter %2")
                       .arg(shellQuote(name), shellQuote(remoteEnvPath), wsMountArg);
    }

    if (env.value(QStringLiteral("recordEnabled")).toBool()) {
        QString logFile;
        QString timingFile;
        if (prepareRecordedSessionPaths(name, &logFile, &timingFile)) {
            const bool hasScript = !QStandardPaths::findExecutable(QStringLiteral("script")).isEmpty();
            enterCmd = buildSessionWrapCommand(enterCmd, logFile, timingFile,
                                               QCoreApplication::applicationFilePath(),
                                               sessionsDirPath(), hasScript);
        }
    }

    launchCommandInTerminal(enterCmd);
    refresh();
}

void Backend::openWorkspaceFolder(const QString &name) {
    if (name.isEmpty())
        return;
    const QString path = workspacePathFor(name);
    QDir().mkpath(path);
    QDesktopServices::openUrl(QUrl::fromLocalFile(path));
}

QString Backend::fileAccessDirPath() const {
    return m_stateDir + QStringLiteral("/file-access");
}

QString Backend::fileAccessStatePath(const QString &name) const {
    return fileAccessDirPath() + QLatin1Char('/') + sanitizeName(name) + QStringLiteral(".json");
}

QString Backend::fileAccessLogPath(const QString &name) const {
    return fileAccessDirPath() + QLatin1Char('/') + sanitizeName(name) + QStringLiteral(".log");
}

QVariantMap Backend::loadFileAccessState(const QString &name) const {
    QFile file(fileAccessStatePath(name));
    if (!file.open(QIODevice::ReadOnly))
        return {};
    const QJsonDocument doc = QJsonDocument::fromJson(file.readAll());
    return doc.isObject() ? doc.object().toVariantMap() : QVariantMap{};
}

bool Backend::fileAccessProcessAlive(qint64 pid, const QString &name) const {
    if (pid <= 0)
        return false;
    if (::kill(static_cast<pid_t>(pid), 0) != 0)
        return false;

    // Guard against PID reuse: only trust processes that still look like the
    // `incus file mount <name> ...` listener we started.
    QFile cmdline(QStringLiteral("/proc/%1/cmdline").arg(pid));
    if (!cmdline.open(QIODevice::ReadOnly))
        return false;
    const QList<QByteArray> args = cmdline.readAll().split('\0');
    bool hasFile = false;
    bool hasMount = false;
    bool hasName = false;
    for (const QByteArray &arg : args) {
        if (arg == "file")
            hasFile = true;
        else if (arg == "mount")
            hasMount = true;
        else if (QString::fromUtf8(arg) == name)
            hasName = true;
        else if (arg.contains("incus file mount") && arg.contains(name.toUtf8())) {
            // The detached `/bin/sh -c 'exec incus file mount ...'` wrapper is
            // still alive for a few milliseconds before exec replaces it.
            hasFile = true;
            hasMount = true;
            hasName = true;
        }
    }
    return hasFile && hasMount && hasName;
}

void Backend::stopFileAccessProcess(const QString &name) {
    if (name.isEmpty())
        return;
    const QVariantMap state = loadFileAccessState(name);
    const qint64 pid = state.value(QStringLiteral("pid")).toLongLong();
    if (fileAccessProcessAlive(pid, name)) {
        ::kill(static_cast<pid_t>(pid), SIGTERM);
        QTimer::singleShot(700, this, [this, pid, name] {
            if (fileAccessProcessAlive(pid, name))
                ::kill(static_cast<pid_t>(pid), SIGKILL);
        });
    }
    QFile::remove(fileAccessStatePath(name));
    QFile::remove(fileAccessLogPath(name));
}

void Backend::applyFileAccessState(QVariantList &environments) {
    QSet<QString> knownFiles;
    for (int i = 0; i < environments.size(); ++i) {
        QVariantMap env = environments.at(i).toMap();
        const QString name = env.value("name").toString();
        if (name.isEmpty())
            continue;
        knownFiles.insert(sanitizeName(name));

        const QVariantMap state = loadFileAccessState(name);
        const qint64 pid = state.value(QStringLiteral("pid")).toLongLong();
        int port = state.value(QStringLiteral("port")).toInt();
        bool active = fileAccessProcessAlive(pid, name);

        if (!active && !state.isEmpty()) {
            QFile::remove(fileAccessStatePath(name));
            QFile::remove(fileAccessLogPath(name));
            port = 0;
        }
        if (active && !env.value("running").toBool()) {
            // The environment stopped, so the listener no longer has a rootfs to serve.
            stopFileAccessProcess(name);
            active = false;
            port = 0;
        }

        env["fileAccessActive"] = active;
        env["fileAccessPort"] = active ? port : 0;
        env["fileAccessUrl"] = active ? fileAccessUrl(port) : QString();
        environments[i] = env;
    }

    // Drop state files whose environment disappeared (deleted outside AIBox).
    QDir dir(fileAccessDirPath());
    const QFileInfoList states = dir.entryInfoList({QStringLiteral("*.json")}, QDir::Files);
    for (const QFileInfo &info : states) {
        const QString base = info.completeBaseName();
        if (knownFiles.contains(base))
            continue;
        QFile stateFile(info.absoluteFilePath());
        QString stateName;
        qint64 pid = 0;
        if (stateFile.open(QIODevice::ReadOnly)) {
            const QJsonDocument doc = QJsonDocument::fromJson(stateFile.readAll());
            if (doc.isObject()) {
                stateName = doc.object().value(QStringLiteral("name")).toString();
                pid = doc.object().value(QStringLiteral("pid")).toVariant().toLongLong();
            }
        }
        if (stateName.isEmpty())
            stateName = base;
        if (fileAccessProcessAlive(pid, stateName))
            ::kill(static_cast<pid_t>(pid), SIGTERM);
        QFile::remove(info.absoluteFilePath());
        QFile::remove(fileAccessLogPath(stateName));
    }
}

QString Backend::fileAccessUrl(int port) {
    if (port < 1 || port > 65535)
        return {};
    return QStringLiteral("sftp://root@127.0.0.1:%1/").arg(port);
}

int Backend::chooseFileAccessPort(const QSet<int> &reservedPorts,
                                  const std::function<bool(int)> &isAvailable,
                                  int startPort, int maxPort) {
    for (int port = startPort; port <= maxPort; ++port) {
        if (reservedPorts.contains(port))
            continue;
        if (isAvailable && !isAvailable(port))
            continue;
        return port;
    }
    return 0;
}

QString Backend::enableFileAccess(const QString &name) {
    auto fail = [this](const QString &message) {
        setError(message);
        return message;
    };

    if (name.isEmpty())
        return fail(tr("Select an environment first."));
    if (m_busy)
        return fail(tr("Wait for the current operation to finish before changing root file access."));

    const QVariantMap env = environmentByName(name);
    if (env.isEmpty())
        return fail(tr("Environment '%1' was not found.").arg(name));
    if (env.value("isTemplate").toBool())
        return fail(tr("Templates cannot expose root files. Clone or unmark as template first."));
    if (!env.value("running").toBool())
        return fail(tr("Start '%1' before enabling root file access.").arg(name));

    const QVariantMap existing = loadFileAccessState(name);
    if (fileAccessProcessAlive(existing.value(QStringLiteral("pid")).toLongLong(), name))
        return {};

    if (!QDir().mkpath(fileAccessDirPath()))
        return fail(tr("Could not create the root file access state directory: %1").arg(fileAccessDirPath()));

    QSet<int> reservedPorts;
    QDir stateDir(fileAccessDirPath());
    const QFileInfoList states = stateDir.entryInfoList({QStringLiteral("*.json")}, QDir::Files);
    for (const QFileInfo &info : states) {
        const QVariantMap state = loadFileAccessState(info.completeBaseName());
        const int port = state.value(QStringLiteral("port")).toInt();
        if (port > 0)
            reservedPorts.insert(port);
    }

    const int port = chooseFileAccessPort(reservedPorts, [](int candidate) {
        return isHostPortAvailable(candidate);
    });
    if (port <= 0)
        return fail(tr("No free local port is available for root file access (tried 2222-2299)."));

    const QString logPath = fileAccessLogPath(name);
    const QString script = QStringLiteral("exec incus file mount %1 --listen %2 --no-auth > %3 2>&1")
                               .arg(shellQuote(name),
                                    shellQuote(QStringLiteral("127.0.0.1:%1").arg(port)),
                                    shellQuote(logPath));
    qint64 pid = 0;
    if (!QProcess::startDetached(QStringLiteral("/bin/sh"), {QStringLiteral("-c"), script},
                                 QDir::homePath(), &pid) ||
        pid <= 0) {
        return fail(tr("Could not start the local SFTP listener."));
    }

    QJsonObject state;
    state[QStringLiteral("name")] = name;
    state[QStringLiteral("pid")] = pid;
    state[QStringLiteral("port")] = port;
    state[QStringLiteral("startedAt")] = QDateTime::currentDateTimeUtc().toString(Qt::ISODate);
    QSaveFile stateFile(fileAccessStatePath(name));
    if (!stateFile.open(QIODevice::WriteOnly)) {
        ::kill(static_cast<pid_t>(pid), SIGTERM);
        return fail(tr("Could not save the root file access state file."));
    }
    stateFile.write(QJsonDocument(state).toJson(QJsonDocument::Compact));
    if (!stateFile.commit()) {
        ::kill(static_cast<pid_t>(pid), SIGTERM);
        return fail(tr("Could not save the root file access state file."));
    }

    // Wait briefly for the listener to bind so the UI never hands out a dead URL.
    bool listening = false;
    QElapsedTimer waitTimer;
    waitTimer.start();
    while (waitTimer.elapsed() < 3000) {
        // Check the process first: if incus failed to bind, it exits and we must
        // not mistake someone else's late listener on that port for ours.
        if (!fileAccessProcessAlive(pid, name))
            break;
        if (isLocalTcpPortListening(port)) {
            listening = true;
            break;
        }
        QThread::msleep(60);
    }
    if (!listening) {
        QString detail;
        QFile log(logPath);
        if (log.open(QIODevice::ReadOnly)) {
            const QStringList lines = QString::fromUtf8(log.readAll())
                                          .split(QRegularExpression(QStringLiteral("[\\r\\n]+")),
                                                 Qt::SkipEmptyParts);
            if (!lines.isEmpty())
                detail = lines.last().trimmed();
        }
        stopFileAccessProcess(name);
        return fail(detail.isEmpty()
                        ? tr("Root file access failed to start on 127.0.0.1:%1.").arg(port)
                        : tr("Root file access failed to start: %1").arg(detail));
    }

    for (int i = 0; i < m_environments.size(); ++i) {
        QVariantMap item = m_environments.at(i).toMap();
        if (item.value("name").toString() != name)
            continue;
        item["fileAccessActive"] = true;
        item["fileAccessPort"] = port;
        item["fileAccessUrl"] = fileAccessUrl(port);
        m_environments[i] = item;
        emit environmentsChanged();
        emit selectedIndexChanged();
        break;
    }

    m_statusMessage = tr("Root file access enabled: %1").arg(fileAccessUrl(port));
    emit statusMessageChanged();
    setError(QString());
    QTimer::singleShot(3000, this, [this] {
        if (!m_busy) {
            m_statusMessage.clear();
            emit statusMessageChanged();
        }
    });

    return {};
}

void Backend::disableFileAccess(const QString &name) {
    if (name.isEmpty())
        return;
    stopFileAccessProcess(name);
    for (int i = 0; i < m_environments.size(); ++i) {
        QVariantMap env = m_environments.at(i).toMap();
        if (env.value("name").toString() != name)
            continue;
        env["fileAccessActive"] = false;
        env["fileAccessPort"] = 0;
        env["fileAccessUrl"] = QString();
        m_environments[i] = env;
        emit environmentsChanged();
        emit selectedIndexChanged();
        break;
    }
    m_statusMessage = tr("Root file access disabled for '%1'.").arg(name);
    emit statusMessageChanged();
    QTimer::singleShot(3000, this, [this] {
        if (!m_busy) {
            m_statusMessage.clear();
            emit statusMessageChanged();
        }
    });
}

void Backend::openFileAccess(const QString &name) {
    if (name.isEmpty())
        return;
    const QVariantMap env = environmentByName(name);
    const QString url = env.value("fileAccessUrl").toString();
    if (url.isEmpty())
        return;

    // Prefer a registered sftp:// scheme handler (GVFS/Nautilus, KIO, etc.).
    QProcess gio;
    QProcessEnvironment gioEnv = QProcessEnvironment::systemEnvironment();
    gioEnv.insert(QStringLiteral("LC_ALL"), QStringLiteral("C"));
    gio.setProcessEnvironment(gioEnv);
    gio.start(QStringLiteral("gio"),
              {QStringLiteral("mime"), QStringLiteral("x-scheme-handler/sftp")});
    const bool hasHandler = gio.waitForFinished(1500) && gio.exitCode() == 0 &&
                            !QString::fromUtf8(gio.readAllStandardOutput())
                                 .contains(QLatin1String("No default applications"));
    if (hasHandler && QDesktopServices::openUrl(QUrl(url)))
        return;

    // Nautilus and most file managers accept an sftp:// URI directly even when
    // the desktop environment has no x-scheme-handler registered for sftp.
    for (const QString &fileManager : {QStringLiteral("nautilus"), QStringLiteral("dolphin"),
                                       QStringLiteral("thunar"), QStringLiteral("nemo"),
                                       QStringLiteral("pcmanfm"), QStringLiteral("caja")}) {
        if (QStandardPaths::findExecutable(fileManager).isEmpty())
            continue;
        QStringList args;
        if (fileManager == QLatin1String("nautilus"))
            args << QStringLiteral("-w");
        args << url;
        if (QProcess::startDetached(fileManager, args))
            return;
    }

    copyText(url, tr("SFTP URL"));
    setError(tr("No file manager accepted the SFTP URL; it has been copied to the clipboard."));
}

void Backend::toggleStartStop(const QString &name) {
    if (name.isEmpty() || m_busy)
        return;
    const QVariantMap env = environmentByName(name);
    if (env.value("isTemplate").toBool()) {
        setError(tr("Templates cannot be started directly. Clone or unmark as template first."));
        return;
    }
    const bool isRunning = env.value("running").toBool();

    if (isRunning) {
        stopFileAccessProcess(name);
        runShellTask(tr("Stopping '%1'").arg(name), stopEnvironmentScript(name), {}, name);
    } else {
        runShellTask(tr("Starting '%1'").arg(name), startEnvironmentScript(name), {}, name);
    }
}

void Backend::cloneEnvironment(const QString &sourceName, const QString &rawNewName,
                               const QString &diskGiB, const QString &ramGiB,
                               int cpuCores, bool startAfter,
                               const QString &processLimit, const QString &ioLimit) {
    if (m_busy) {
        setError(tr("Another operation is currently in progress. Please wait or cancel it first."));
        return;
    }
    const QVariantMap source = environmentByName(sourceName);
    if (source.isEmpty()) {
        setError(tr("Source environment '%1' was not found.").arg(sourceName));
        return;
    }
    const QString newName = sanitizeName(rawNewName);
    if (newName.isEmpty()) {
        setError(tr("Please enter a valid environment name (letters, numbers, hyphens)."));
        return;
    }
    if (!environmentByName(newName).isEmpty()) {
        setError(tr("Environment '%1' already exists.").arg(newName));
        return;
    }

    static const QRegularExpression sizeRe(QStringLiteral("^\\d+(?:\\.\\d+)?(?:GiB|MiB|TiB|GB|MB|TB)$"),
                                           QRegularExpression::CaseInsensitiveOption);
    if (!diskGiB.trimmed().isEmpty() && !sizeRe.match(diskGiB.trimmed()).hasMatch()) {
        setError(tr("The disk quota is invalid. Use a value such as 35GiB."));
        return;
    }
    if (!ramGiB.trimmed().isEmpty() && !sizeRe.match(ramGiB.trimmed()).hasMatch()) {
        setError(tr("The RAM limit is invalid. Use a value such as 6GiB."));
        return;
    }

    const QString cleanProc = processLimit.trimmed().compare(QLatin1String("unlimited"), Qt::CaseInsensitive) == 0
                                  ? QString()
                                  : processLimit.trimmed();
    const QString cleanIo = ioLimit.trimmed().compare(QLatin1String("unlimited"), Qt::CaseInsensitive) == 0
                                ? QString()
                                : ioLimit.trimmed();
    if (!isValidProcessLimit(cleanProc)) {
        setError(tr("The process limit is invalid. Use an integer >= 10 (e.g. 512) or leave unlimited."));
        return;
    }
    if (!isValidIoLimit(cleanIo)) {
        setError(tr("The disk I/O limit is invalid. Use a rate such as 200MB or 2500iops."));
        return;
    }

    const QString netMode = source.value("networkMode").toString();
    const QString netAllow = source.value("networkAllowlist").toString();
    QStringList resolvedCidrs;
    if (netMode == QLatin1String("allowlist"))
        resolvedCidrs = resolveAllowlistCidrs(parseAllowlistEntries(netAllow));

    const bool isVm = source.value("type").toString().toUpper() == QLatin1String("VM");
    const QString hostWorkspace = m_workspaceBase + "/" + newName;
    const QString script = cloneEnvironmentScript(sourceName, newName, hostWorkspace,
                                                  diskGiB, ramGiB, cpuCores, startAfter,
                                                  netMode, netAllow, resolvedCidrs,
                                                  cleanProc, cleanIo, isVm);
    runShellTask(tr("Cloning '%1' to '%2'").arg(sourceName, newName), script, newName, newName);
}

void Backend::setEnvironmentTemplate(const QString &name, bool isTemplate) {
    if (name.isEmpty() || m_busy)
        return;
    const QVariantMap env = environmentByName(name);
    if (env.isEmpty())
        return;

    if (isTemplate) {
        QString script = QStringLiteral("set -euo pipefail\necho '@@STAGE:30:Preparing template...'\n");
        if (env.value("running").toBool())
            script += QStringLiteral("incus stop %1\n").arg(shellQuote(name));
        script += QStringLiteral("incus config set %1 user.aibox.template true\necho '@@STAGE:100:Marked as template.'").arg(shellQuote(name));
        runShellTask(tr("Marking '%1' as template").arg(name), script);
    } else {
        const QString script = QStringLiteral(
            "set -euo pipefail\necho '@@STAGE:50:Unmarking template...'\n"
            "incus config unset %1 user.aibox.template >/dev/null 2>&1 || true\n"
            "echo '@@STAGE:100:Template unmarked.'").arg(shellQuote(name));
        runShellTask(tr("Unmarking '%1' as template").arg(name), script);
    }
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

void Backend::setNetworkMode(const QString &name, const QString &mode, const QString &allowlist) {
    if (name.isEmpty() || m_busy)
        return;
    const QString normalizedMode = mode.trimmed().toLower();
    if (normalizedMode != QLatin1String("online") &&
        normalizedMode != QLatin1String("offline") &&
        normalizedMode != QLatin1String("allowlist")) {
        setError(tr("Unsupported network mode: %1").arg(mode));
        return;
    }

    QStringList cidrs;
    QString normalizedAllowlist;
    if (normalizedMode == QLatin1String("allowlist")) {
        const QStringList entries = parseAllowlistEntries(allowlist);
        if (entries.isEmpty()) {
            setError(tr("Enter at least one domain, IP, or CIDR for allowlist mode."));
            return;
        }
        QStringList warnings;
        cidrs = resolveAllowlistCidrs(entries, {}, &warnings);
        if (cidrs.isEmpty()) {
            setError(warnings.isEmpty()
                         ? tr("Could not resolve any public IPv4 destinations from the allowlist.")
                         : warnings.join(QStringLiteral(" ")));
            return;
        }
        normalizedAllowlist = entries.join(QStringLiteral(", "));
    }

    const QVariantMap env = environmentByName(name);
    const bool keepLogging = (normalizedMode != QLatin1String("offline")) && env.value("egressRecording").toBool();
    const QString script = networkControlScript(name, normalizedMode, normalizedAllowlist, cidrs, keepLogging);
    runShellTask(tr("Updating network mode for '%1'").arg(name), script, {}, name);
}

void Backend::refreshNetworkAllowlist(const QString &name) {
    const QVariantMap env = environmentByName(name);
    if (env.isEmpty())
        return;
    setNetworkMode(name, QStringLiteral("allowlist"), env.value("networkAllowlist").toString());
}

void Backend::setEgressRecording(const QString &name, bool enabled) {
    if (name.isEmpty() || m_busy)
        return;
    const QVariantMap env = environmentByName(name);
    if (env.isEmpty())
        return;

    const QString mode = env.value("networkMode").toString();
    if (enabled && mode == QLatin1String("offline")) {
        setError(tr("Disable Offline mode first to record egress traffic."));
        return;
    }

    QString allowRaw;
    QStringList cidrs;
    QString targetMode = mode;
    if (mode == QLatin1String("allowlist")) {
        allowRaw = env.value("networkAllowlist").toString();
        const QStringList entries = parseAllowlistEntries(allowRaw);
        cidrs = resolveAllowlistCidrs(entries);
        if (entries.isEmpty() || cidrs.isEmpty()) {
            setError(tr("Allowlist entries could not be resolved. Update the allowlist first."));
            return;
        }
    } else {
        targetMode = QStringLiteral("online");
    }

    const QString script = networkControlScript(name, targetMode, allowRaw, cidrs, enabled);
    runShellTask(enabled ? tr("Enabling egress recording for '%1'").arg(name)
                         : tr("Disabling egress recording for '%1'").arg(name),
                 script, {}, name);
}

QVariantList Backend::parseKernelEgressLogs(const QString &journalOutput, const QString &containerIp,
                                            const QString &hostIface, int maxEntries) {
    const QString cleanIp = (containerIp.trimmed() == QLatin1String("—")) ? QString() : containerIp.trimmed();
    const QString cleanIface = hostIface.trimmed();
    if (cleanIp.isEmpty() && cleanIface.isEmpty())
        return {};

    static const QRegularExpression isoTsRe(QStringLiteral(R"(^(\d{4}-\d{2}-\d{2}T(\d{2}:\d{2}:\d{2}))\S*)"));
    static const QRegularExpression sysTsRe(QStringLiteral(R"(^[A-Z][a-z]{2}\s+\d+\s+(\d{2}:\d{2}:\d{2}))"));
    static const QRegularExpression inRe(QStringLiteral(R"(\bIN=(\S+))"));
    static const QRegularExpression srcRe(QStringLiteral(R"(\bSRC=(\S+))"));
    static const QRegularExpression dstRe(QStringLiteral(R"(\bDST=(\S+))"));
    static const QRegularExpression protoRe(QStringLiteral(R"(\bPROTO=(\S+))"));
    static const QRegularExpression dptRe(QStringLiteral(R"(\bDPT=(\d+))"));

    struct AggEntry {
        QString dstIp;
        QString proto;
        QString port;
        QString lastSeen;
        int count = 0;
        qint64 seq = 0;
    };
    QHash<QString, AggEntry> agg;
    qint64 seqCounter = 0;

    const QStringList lines = journalOutput.split(QLatin1Char('\n'), Qt::SkipEmptyParts);
    for (const QString &line : lines) {
        if (!line.contains(QLatin1String("eth0-egress")))
            continue;

        const QString inIf = inRe.match(line).captured(1);
        const QString srcIp = srcRe.match(line).captured(1);
        const bool matchIface = !cleanIface.isEmpty() && !inIf.isEmpty() && (inIf == cleanIface);
        const bool matchIp = !cleanIp.isEmpty() && !srcIp.isEmpty() && (srcIp == cleanIp);
        if (!matchIface && !matchIp)
            continue;

        const QString dstIp = dstRe.match(line).captured(1);
        if (dstIp.isEmpty())
            continue;
        const QString proto = protoRe.match(line).captured(1);
        const QString port = dptRe.match(line).captured(1);

        QString seen;
        const auto isoMatch = isoTsRe.match(line);
        if (isoMatch.hasMatch()) {
            seen = isoMatch.captured(2);
        } else {
            const auto sysMatch = sysTsRe.match(line);
            if (sysMatch.hasMatch())
                seen = sysMatch.captured(1);
        }

        const QString key = dstIp + QLatin1Char('|') + proto + QLatin1Char('|') + port;
        auto it = agg.find(key);
        ++seqCounter;
        if (it == agg.end()) {
            agg.insert(key, AggEntry{dstIp, proto.isEmpty() ? QStringLiteral("IP") : proto, port, seen, 1, seqCounter});
        } else {
            it->count += 1;
            if (!seen.isEmpty())
                it->lastSeen = seen;
            it->seq = seqCounter;
        }
    }

    QList<AggEntry> list = agg.values();
    std::sort(list.begin(), list.end(), [](const AggEntry &a, const AggEntry &b) {
        return a.seq > b.seq;
    });

    const int limit = qMax(1, maxEntries);
    QVariantList result;
    result.reserve(qMin(limit, static_cast<int>(list.size())));
    for (int i = 0; i < list.size() && i < limit; ++i) {
        const AggEntry &e = list.at(i);
        QVariantMap row;
        row["dstIp"] = e.dstIp;
        row["proto"] = e.proto;
        row["port"] = e.port.isEmpty() ? QStringLiteral("—") : e.port;
        row["count"] = e.count;
        row["lastSeen"] = e.lastSeen.isEmpty() ? QStringLiteral("—") : e.lastSeen;
        result.append(row);
    }
    return result;
}

void Backend::refreshEgressLog(const QString &name) {
    if (name.isEmpty())
        return;
    const QVariantMap env = environmentByName(name);
    if (env.isEmpty())
        return;

    m_egressLogsEnvName = name;
    if (m_demoMode) {
        m_egressLogs = {
            QVariantMap{{"dstIp", QStringLiteral("140.82.121.4")}, {"proto", QStringLiteral("TCP")}, {"port", QStringLiteral("443")}, {"count", 18}, {"lastSeen", QStringLiteral("13:18:42")}},
            QVariantMap{{"dstIp", QStringLiteral("160.79.104.10")}, {"proto", QStringLiteral("TCP")}, {"port", QStringLiteral("443")}, {"count", 9}, {"lastSeen", QStringLiteral("13:17:15")}},
            QVariantMap{{"dstIp", QStringLiteral("104.16.25.34")}, {"proto", QStringLiteral("TCP")}, {"port", QStringLiteral("443")}, {"count", 4}, {"lastSeen", QStringLiteral("13:15:09")}}
        };
        emit egressLogsChanged();
        return;
    }

    const QString containerIp = env.value("ipv4").toString();
    const QString hostIface = env.value("hostInterface").toString();

    auto *proc = new QProcess(this);
    connect(proc, &QProcess::finished, this, [this, proc, name, containerIp, hostIface](int, QProcess::ExitStatus) {
        const QString out = QString::fromUtf8(proc->readAllStandardOutput());
        proc->deleteLater();
        if (m_egressLogsEnvName != name)
            return;
        m_egressLogs = parseKernelEgressLogs(out, containerIp, hostIface, 100);
        emit egressLogsChanged();
    });
    proc->start(QStringLiteral("journalctl"),
                {QStringLiteral("-k"), QStringLiteral("-n"), QStringLiteral("4000"),
                 QStringLiteral("--no-pager"), QStringLiteral("--output=short-iso")});
}

void Backend::addEgressDestinationToAllowlist(const QString &name, const QString &destinationIp) {
    if (name.isEmpty() || m_busy)
        return;
    const QString ip = destinationIp.trimmed();
    if (ip.isEmpty())
        return;

    const QVariantMap env = environmentByName(name);
    QStringList entries = parseAllowlistEntries(env.value("networkAllowlist").toString());
    if (!entries.contains(ip) && !entries.contains(ip + QStringLiteral("/32")))
        entries.append(ip);
    setNetworkMode(name, QStringLiteral("allowlist"), entries.join(QStringLiteral(", ")));
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

void Backend::runHeadlessCommand(const QString &name, const QString &command) {
    if (name.isEmpty() || command.trimmed().isEmpty() || m_busy)
        return;

    QString secretErr;
    const QList<QPair<QString, QString>> envSecrets = resolveEnvironmentSecrets(name, &secretErr);
    if (!secretErr.isEmpty()) {
        setError(secretErr);
        return;
    }

    QString hostEnvPath;
    QString remoteEnvPath;
    if (!envSecrets.isEmpty()) {
        hostEnvPath = createHostTempSecretFile(buildSecretEnvFileContent(envSecrets), &secretErr);
        if (hostEnvPath.isEmpty()) {
            setError(secretErr);
            return;
        }
        remoteEnvPath = QStringLiteral("/dev/shm/aibox-%1.env")
                            .arg(QUuid::createUuid().toString(QUuid::Id128));
    }

    QString pushSnippet;
    QString execLine;
    const QString qName = shellQuote(name);
    const QString qCmd = shellQuote(command);
    if (!hostEnvPath.isEmpty()) {
        const QString qHost = shellQuote(hostEnvPath);
        const QString qRemote = shellQuote(remoteEnvPath);
        pushSnippet = QStringLiteral(
            "HOST_ENV=%1\n"
            "REMOTE_ENV=%2\n"
            "trap '[ -n \"${HOST_ENV:-}\" ] && rm -f \"$HOST_ENV\"' EXIT\n"
            "incus file push --mode 0400 \"$HOST_ENV\" %3\"$REMOTE_ENV\"\n"
            "rm -f \"$HOST_ENV\"\n"
            "HOST_ENV=\n").arg(qHost, qRemote, qName);
        execLine = QStringLiteral(
            "incus exec %1 --cwd \"$WS_PATH\" -- sh -c "
            "'F=\"$1\"; C=\"$2\"; if [ -f \"$F\" ]; then set -a; . \"$F\"; set +a; rm -f \"$F\"; fi; "
            "exec xvfb-run -a -s \"-screen 0 1920x1080x24\" sh -lc \"$C\"' "
            "aibox-headless \"$REMOTE_ENV\" %2").arg(qName, qCmd);
    } else {
        execLine = QStringLiteral(
            "incus exec %1 --cwd \"$WS_PATH\" -- xvfb-run -a -s \"-screen 0 1920x1080x24\" sh -lc %2")
                       .arg(qName, qCmd);
    }

    const QString script = QStringLiteral(R"sh(
set -euo pipefail
echo "@@STAGE:20:Checking virtual display server (Xvfb)..."
STATUS=$(incus info %1 | awk '/^Status:/ {print $2}')
if [[ "$STATUS" != "RUNNING" ]]; then
    incus start %1
    sleep 2
fi
incus config set %1 user.aibox.lastused "$(date +%s)" >/dev/null 2>&1 || true
%4
WS_PATH=$(incus config device get %1 workspace path 2>/dev/null || echo /home/workspace)
%2
incus exec %1 -- sh -c '
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
%3
echo "@@STAGE:100:Completed."
)sh").arg(qName, pushSnippet, execLine, workspaceMountMigrationScript(name));

    runShellTask(tr("Running headless GUI test in '%1'").arg(name), script, {}, name);
}

QString Backend::aiExecCommandText(const QString &name) const {
    if (name.isEmpty())
        return {};
    const QVariantMap env = environmentByName(name);
    if (!env.value("secretNames").toStringList().isEmpty())
        return QStringLiteral("aibox exec %1 -- bash").arg(name);
    const QString wsMount = env.value(QStringLiteral("workspaceMountPath")).toString().trimmed();
    return QStringLiteral("incus exec %1 --cwd %2 -- bash")
        .arg(name, wsMount.isEmpty() ? containerWorkspacePath() : wsMount);
}

void Backend::copyAiCommand(const QString &name) {
    if (name.isEmpty())
        return;
    if (auto *cb = QGuiApplication::clipboard()) {
        cb->setText(aiExecCommandText(name));
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

bool Backend::copyText(const QString &text, const QString &label) {
    if (text.isEmpty() || text == QStringLiteral("—"))
        return false;
    auto *cb = QGuiApplication::clipboard();
    if (!cb)
        return false;
    cb->setText(text);
    m_statusMessage = label.isEmpty()
                          ? tr("Copied '%1' to clipboard.").arg(text)
                          : tr("Copied %1 '%2' to clipboard.").arg(label, text);
    emit statusMessageChanged();
    QTimer::singleShot(3000, this, [this] {
        if (!m_busy) {
            m_statusMessage.clear();
            emit statusMessageChanged();
        }
    });
    return true;
}

void Backend::downloadOsImage(const QString &osId) {
    if (osId.isEmpty() || m_busy)
        return;
    const QVariantMap entry = osEntryById(osId);
    if (entry.isEmpty()) {
        setError(tr("The OS entry '%1' is not in the catalog.").arg(osId));
        return;
    }

    QDir().mkpath(m_osStoragePath);
    const QString alias = entry.value("alias").toString();
    const QString localAlias = QStringLiteral("aibox-") + osId;
    const QString cacheBase = cacheBasePath(osId);
    m_downloadingOsId = osId;
    rebuildOsCatalog();

    const QString script = QStringLiteral(R"sh(
set -euo pipefail
LOCAL_ALIAS=%1
REMOTE_IMAGE=%2
CACHE=%3
CACHE_ROOT=%4

mkdir -p "$(dirname "$CACHE")"
if incus image info "$LOCAL_ALIAS" >/dev/null 2>&1; then
    echo "@@STAGE:45:OS image is already in the local Incus store..."
elif [ -s "$CACHE" ] && [ -s "$CACHE_ROOT" ]; then
    echo "@@STAGE:35:Importing OS image from OS storage..."
    incus image import "$CACHE" "$CACHE_ROOT" --alias "$LOCAL_ALIAS"
else
    echo "@@STAGE:5:Downloading %5 from the official image server..."
    script -q -e -c "incus image copy \"$REMOTE_IMAGE\" local: --alias \"$LOCAL_ALIAS\" --auto-update" /dev/null
fi

if [ ! -s "$CACHE" ] || [ ! -s "$CACHE_ROOT" ]; then
    echo "@@STAGE:85:Saving OS image to OS storage..."
    rm -f "$CACHE" "$CACHE_ROOT"
    incus image export "$LOCAL_ALIAS" "$CACHE" >/dev/null
fi
echo "@@STAGE:100:Download complete!"
)sh")
                           .arg(shellQuote(localAlias),
                                shellQuote(QStringLiteral("images:") + alias),
                                shellQuote(cacheBase),
                                shellQuote(cacheBase + ".root"),
                                osId);
    runShellTask(tr("Downloading OS image '%1'").arg(osId), script);
}

void Backend::removeOsImage(const QString &osId) {
    if (osId.isEmpty() || m_busy)
        return;
    const QString localAlias = QStringLiteral("aibox-") + osId;
    const QString cacheBase = cacheBasePath(osId);
    runShellTask(tr("Removing cached OS image '%1'").arg(osId),
                 QStringLiteral("set -euo pipefail\n"
                                "echo '@@STAGE:50:Removing image cache...'\n"
                                "if incus image info %1 >/dev/null 2>&1; then incus image delete %1; fi\n"
                                "rm -f %2 %3\n"
                                "echo '@@STAGE:100:Removed.'")
                     .arg(shellQuote(localAlias), shellQuote(cacheBase), shellQuote(cacheBase + ".root")));
}

QString Backend::proxyDeviceName(int hostPort) {
    return QStringLiteral("fwd-%1").arg(hostPort);
}

bool Backend::isHostPortAvailable(int hostPort) {
    if (hostPort < 1024 || hostPort > 65535)
        return false;
    QTcpServer server;
    if (!server.listen(QHostAddress::LocalHost, static_cast<quint16>(hostPort)))
        return false;
    server.close();
    return true;
}

QString Backend::addPortForwardScript(const QString &name, int hostPort, int guestPort) {
    const QString dev = proxyDeviceName(hostPort);
    return QStringLiteral("set -euo pipefail\n"
                          "echo '@@STAGE:45:Adding localhost port forward (%2 -> %3)...'\n"
                          "incus config device add %1 %4 proxy listen=%5 connect=%6\n"
                          "echo '@@STAGE:100:Port forward active.'")
        .arg(shellQuote(name),
             QString::number(hostPort),
             QString::number(guestPort),
             shellQuote(dev),
             shellQuote(QStringLiteral("tcp:127.0.0.1:%1").arg(hostPort)),
             shellQuote(QStringLiteral("tcp:127.0.0.1:%1").arg(guestPort)));
}

QString Backend::addPortForward(const QString &name, int hostPort, int guestPort) {
    if (name.isEmpty())
        return tr("Select an environment first.");
    if (m_busy)
        return tr("Another operation is currently in progress.");
    if (hostPort < 1024 || hostPort > 65535)
        return tr("Host port must be between 1024 and 65535 (127.0.0.1 bind).");
    if (guestPort < 1 || guestPort > 65535)
        return tr("Container port must be between 1 and 65535.");

    const QVariantMap env = environmentByName(name);
    if (env.isEmpty())
        return tr("Environment '%1' was not found.").arg(name);

    for (const auto &v : env.value("portForwards").toList()) {
        if (v.toMap().value("hostPort").toInt() == hostPort)
            return tr("Host port %1 is already forwarded on '%2'.").arg(hostPort).arg(name);
    }

    if (!isHostPortAvailable(hostPort))
        return tr("Host port 127.0.0.1:%1 is already in use on this machine.").arg(hostPort);

    runShellTask(tr("Forwarding 127.0.0.1:%1 -> %2:%3").arg(hostPort).arg(name).arg(guestPort),
                 addPortForwardScript(name, hostPort, guestPort), {}, name);
    return {};
}

void Backend::removePortForward(const QString &name, const QString &deviceName) {
    if (name.isEmpty() || deviceName.isEmpty() || m_busy)
        return;
    runShellTask(tr("Removing port forward '%1' on '%2'").arg(deviceName, name),
                 QStringLiteral("set -euo pipefail\n"
                                "echo '@@STAGE:50:Removing port forward...'\n"
                                "incus config device remove %1 %2\n"
                                "echo '@@STAGE:100:Port forward removed.'")
                     .arg(shellQuote(name), shellQuote(deviceName)),
                 {}, name);
}

void Backend::openForwardedPort(int hostPort) {
    if (hostPort < 1 || hostPort > 65535)
        return;
    if (isGuiSession())
        QDesktopServices::openUrl(QUrl(QStringLiteral("http://127.0.0.1:%1").arg(hostPort)));
}

QString Backend::validateExtraMountPath(const QString &rawPath, const QStringList &forbiddenConfigPaths) {
    QString trimmed = rawPath.trimmed();
    if (trimmed.isEmpty())
        return tr("Choose a host directory to mount.");
    if (trimmed == QLatin1String("~"))
        trimmed = QDir::homePath();
    else if (trimmed.startsWith(QLatin1String("~/")))
        trimmed = QDir::homePath() + trimmed.mid(1);

    const QFileInfo rawInfo(trimmed);
    if (!rawInfo.exists() || !rawInfo.isDir())
        return tr("Host path '%1' does not exist or is not a directory.").arg(trimmed);

    QString canonical = rawInfo.canonicalFilePath();
    if (canonical.isEmpty())
        canonical = QDir::cleanPath(rawInfo.absoluteFilePath());

    const QString homeClean = QDir::cleanPath(QDir::homePath());
    static const QSet<QString> exactBlocked = {
        QStringLiteral("/"), QStringLiteral("/bin"), QStringLiteral("/boot"),
        QStringLiteral("/dev"), QStringLiteral("/etc"), QStringLiteral("/home"),
        QStringLiteral("/lib"), QStringLiteral("/lib64"), QStringLiteral("/proc"),
        QStringLiteral("/root"), QStringLiteral("/run"), QStringLiteral("/sbin"),
        QStringLiteral("/sys"), QStringLiteral("/usr"), QStringLiteral("/var")
    };
    if (exactBlocked.contains(canonical) || canonical == homeClean) {
        return tr("Refusing to mount system root or home directory '%1'. Pick a specific project/data subfolder.").arg(canonical);
    }

    static const QStringList prefixBlocked = {
        QStringLiteral("/etc/"), QStringLiteral("/boot/"), QStringLiteral("/dev/"),
        QStringLiteral("/proc/"), QStringLiteral("/sys/"), QStringLiteral("/root/"),
        QStringLiteral("/run/"), QStringLiteral("/bin/"), QStringLiteral("/sbin/"),
        QStringLiteral("/lib/"), QStringLiteral("/lib64/"), QStringLiteral("/usr/")
    };
    for (const QString &prefix : prefixBlocked) {
        if (canonical.startsWith(prefix))
            return tr("Refusing to mount system path '%1'.").arg(canonical);
    }

    static const QSet<QString> sensitiveComponents = {
        QStringLiteral(".ssh"), QStringLiteral(".gnupg"), QStringLiteral(".aws"),
        QStringLiteral(".kube"), QStringLiteral(".docker"), QStringLiteral(".aibox"),
        QStringLiteral(".password-store")
    };
    const QStringList parts = canonical.split(QLatin1Char('/'), Qt::SkipEmptyParts);
    for (const QString &part : parts) {
        if (sensitiveComponents.contains(part))
            return tr("Refusing to mount sensitive directory component '%1'.").arg(part);
    }

    for (const QString &cfg : forbiddenConfigPaths) {
        const QString cleanCfg = QDir::cleanPath(cfg);
        if (!cleanCfg.isEmpty() &&
            (canonical == cleanCfg || canonical.startsWith(cleanCfg + QLatin1Char('/')))) {
            return tr("Refusing to mount AIBox configuration directory '%1'.").arg(canonical);
        }
    }
    return {};
}

QString Backend::validateExtraMountCandidate(const QString &rawPath) const {
    return validateExtraMountPath(rawPath, {QFileInfo(m_configPath).absolutePath(), m_stateDir});
}

QString Backend::extraMountDeviceName(const QString &rawName) {
    const QString clean = sanitizeName(rawName);
    return clean.isEmpty() ? QString() : QStringLiteral("mnt-") + clean;
}

QString Backend::addExtraMountScript(const QString &name, const QString &deviceName,
                                     const QString &hostPath, const QString &containerPath, bool readOnly) {
    const QString roFlag = readOnly ? QStringLiteral("true") : QStringLiteral("false");
    return QStringLiteral("set -euo pipefail\n"
                          "echo '@@STAGE:45:Adding extra mount (%4)...'\n"
                          "if ! incus config device add %1 %2 disk source=%3 path=%4 readonly=%5 shift=true >/dev/null 2>&1; then\n"
                          "    incus config device add %1 %2 disk source=%3 path=%4 readonly=%5\n"
                          "fi\n"
                          "echo '@@STAGE:100:Mount attached.'")
        .arg(shellQuote(name), shellQuote(deviceName), shellQuote(hostPath), shellQuote(containerPath), roFlag);
}

QString Backend::addExtraMount(const QString &name, const QString &hostPath, const QString &mountName,
                               bool readOnly, bool confirmReadWrite) {
    if (name.isEmpty())
        return tr("Select an environment first.");
    if (m_busy)
        return tr("Another operation is currently in progress.");
    if (!readOnly && !confirmReadWrite)
        return tr("Read-write extra mounts require explicit second confirmation.");

    const QString cleanMount = sanitizeName(mountName.isEmpty() ? QFileInfo(hostPath.trimmed()).fileName() : mountName);
    if (cleanMount.isEmpty() || cleanMount == QLatin1String("workspace") ||
        cleanMount == QLatin1String("root") || cleanMount == QLatin1String("x11")) {
        return tr("Choose a valid mount name (cannot be 'workspace', 'root', or 'x11').");
    }

    QString expanded = hostPath.trimmed();
    if (expanded == QLatin1String("~"))
        expanded = QDir::homePath();
    else if (expanded.startsWith(QLatin1String("~/")))
        expanded = QDir::homePath() + expanded.mid(1);

    const QString pathErr = validateExtraMountCandidate(expanded);
    if (!pathErr.isEmpty())
        return pathErr;

    const QString resolvedSource = QFileInfo(expanded).canonicalFilePath();
    const QString devName = extraMountDeviceName(cleanMount);
    const QString containerPath = QStringLiteral("/mnt/") + cleanMount;

    const QVariantMap env = environmentByName(name);
    if (env.isEmpty())
        return tr("Environment '%1' was not found.").arg(name);
    for (const auto &v : env.value("extraMounts").toList()) {
        const QVariantMap m = v.toMap();
        if (m.value("deviceName").toString() == devName || m.value("path").toString() == containerPath)
            return tr("Mount '%1' already exists on '%2'.").arg(containerPath, name);
    }

    runShellTask(tr("Mounting %1 at %2 (%3)").arg(resolvedSource, containerPath, readOnly ? tr("RO") : tr("RW")),
                 addExtraMountScript(name, devName, resolvedSource, containerPath, readOnly),
                 {}, name);
    return {};
}

void Backend::removeExtraMount(const QString &name, const QString &deviceName) {
    if (name.isEmpty() || deviceName.isEmpty() || m_busy)
        return;
    runShellTask(tr("Removing mount '%1' on '%2'").arg(deviceName, name),
                 QStringLiteral("set -euo pipefail\n"
                                "echo '@@STAGE:50:Detaching extra mount...'\n"
                                "incus config device remove %1 %2\n"
                                "echo '@@STAGE:100:Mount removed.'")
                     .arg(shellQuote(name), shellQuote(deviceName)),
                 {}, name);
}

qint64 Backend::parseByteSizeToBytes(const QString &text) {
    return parseSizeBytes(text);
}

QString Backend::updateResourcesScript(const QString &name, int cpuCores, const QString &ramGiB,
                                       const QString &diskGiB, const QString &processLimit,
                                       const QString &ioLimit, bool isVm) {
    const QString qName = shellQuote(name);
    QStringList lines;
    lines << QStringLiteral("set -euo pipefail");
    lines << QStringLiteral("echo '@@STAGE:25:Updating CPU and memory quotas...'");
    if (cpuCores > 0)
        lines << QStringLiteral("incus config set %1 limits.cpu %2").arg(qName, shellQuote(QString::number(cpuCores)));
    if (!ramGiB.trimmed().isEmpty())
        lines << QStringLiteral("incus config set %1 limits.memory %2").arg(qName, shellQuote(ramGiB.trimmed()));

    if (!isVm) {
        const QString cleanProc = processLimit.trimmed();
        if (cleanProc.isEmpty() || cleanProc.compare(QLatin1String("unlimited"), Qt::CaseInsensitive) == 0)
            lines << QStringLiteral("incus config unset %1 limits.processes >/dev/null 2>&1 || true").arg(qName);
        else
            lines << QStringLiteral("incus config set %1 limits.processes %2").arg(qName, shellQuote(cleanProc));
    }

    lines << QStringLiteral("echo '@@STAGE:65:Updating root disk quota and I/O limits...'");
    if (!diskGiB.trimmed().isEmpty()) {
        lines << QStringLiteral("incus config device set %1 root size=%2 >/dev/null 2>&1 || incus config device override %1 root size=%2")
                     .arg(qName, shellQuote(diskGiB.trimmed()));
    }

    const QString cleanIo = ioLimit.trimmed();
    if (cleanIo.isEmpty() || cleanIo.compare(QLatin1String("unlimited"), Qt::CaseInsensitive) == 0) {
        lines << QStringLiteral("incus config device unset %1 root limits.read >/dev/null 2>&1 || true").arg(qName);
        lines << QStringLiteral("incus config device unset %1 root limits.write >/dev/null 2>&1 || true").arg(qName);
    } else {
        lines << QStringLiteral("incus config device set %1 root limits.read=%2 limits.write=%2 >/dev/null 2>&1 || incus config device override %1 root limits.read=%2 limits.write=%2")
                     .arg(qName, shellQuote(cleanIo));
    }

    lines << QStringLiteral("echo '@@STAGE:100:Quotas updated.'");
    return lines.join(QLatin1Char('\n'));
}

QString Backend::updateEnvironmentResources(const QString &name, int cpuCores, const QString &ramGiB,
                                            const QString &diskGiB, const QString &processLimit,
                                            const QString &ioLimit) {
    if (name.isEmpty())
        return tr("Select an environment first.");
    if (m_busy)
        return tr("Another operation is currently in progress.");
    if (cpuCores < 1 || cpuCores > 1024)
        return tr("CPU cores must be between 1 and 1024.");

    static const QRegularExpression sizeRe(QStringLiteral("^\\d+(?:\\.\\d+)?(?:GiB|MiB|TiB|GB|MB|TB)$"),
                                           QRegularExpression::CaseInsensitiveOption);
    if (!sizeRe.match(ramGiB.trimmed()).hasMatch())
        return tr("The RAM limit is invalid. Use a value such as 8GiB.");
    if (!sizeRe.match(diskGiB.trimmed()).hasMatch())
        return tr("The disk quota is invalid. Use a value such as 50GiB.");

    const QString cleanProc = processLimit.trimmed().compare(QLatin1String("unlimited"), Qt::CaseInsensitive) == 0
                                  ? QString()
                                  : processLimit.trimmed();
    const QString cleanIo = ioLimit.trimmed().compare(QLatin1String("unlimited"), Qt::CaseInsensitive) == 0
                                ? QString()
                                : ioLimit.trimmed();
    if (!isValidProcessLimit(cleanProc))
        return tr("The process limit is invalid. Use an integer >= 10 (e.g. 4096) or Unlimited.");
    if (!isValidIoLimit(cleanIo))
        return tr("The disk I/O limit is invalid. Use a rate such as 200MB, 2500iops, or Unlimited.");

    const QVariantMap env = environmentByName(name);
    if (env.isEmpty())
        return tr("Environment '%1' was not found.").arg(name);

    const qint64 usedBytes = env.value("diskUsageBytes").toLongLong();
    const qint64 targetBytes = parseByteSizeToBytes(diskGiB.trimmed());
    if (usedBytes > 0 && targetBytes > 0 && targetBytes < usedBytes) {
        return tr("Cannot shrink disk quota (%1) below current usage (%2).")
            .arg(diskGiB.trimmed(), env.value("diskUsage").toString());
    }

    const QString envType = env.value("type").toString().toUpper();
    const bool isVm = (envType == QLatin1String("VIRTUAL-MACHINE") || envType == QLatin1String("VM"));
    runShellTask(tr("Updating resource quotas for '%1'").arg(name),
                 updateResourcesScript(name, cpuCores, ramGiB, diskGiB, cleanProc, cleanIo, isVm),
                 {}, name);
    return {};
}

QString Backend::provisionDirPath() const {
    return QFileInfo(m_configPath).absolutePath() + QStringLiteral("/provision");
}

QVariantList Backend::defaultProvisionBundles() {
    return {
        QVariantMap{{"id", QStringLiteral("devtools")}, {"title", QStringLiteral("Dev Essentials")}, {"desc", QStringLiteral("git, curl, jq, build tools")}},
        QVariantMap{{"id", QStringLiteral("node")}, {"title", QStringLiteral("Node.js LTS")}, {"desc", QStringLiteral("nodejs & npm")}},
        QVariantMap{{"id", QStringLiteral("python")}, {"title", QStringLiteral("Python 3")}, {"desc", QStringLiteral("python3, pip, venv")}},
        QVariantMap{{"id", QStringLiteral("rust")}, {"title", QStringLiteral("Rust")}, {"desc", QStringLiteral("rustc & cargo")}}
    };
}

QVariantList Backend::provisionBundles() const {
    return defaultProvisionBundles();
}

QString Backend::distroFamilyForOsId(const QString &osId, const QString &osAlias) {
    const QString combined = (osId + QLatin1Char(' ') + osAlias).toLower();
    if (combined.contains(QLatin1String("ubuntu")) || combined.contains(QLatin1String("debian")))
        return QStringLiteral("apt");
    if (combined.contains(QLatin1String("arch")))
        return QStringLiteral("pacman");
    return {};
}

QString Backend::provisionScriptForBundle(const QString &bundleId, const QString &distroFamily,
                                          const QString &customProvisionDir) {
    const QString cleanId = sanitizeName(bundleId);
    if (cleanId.isEmpty() || (distroFamily != QLatin1String("apt") && distroFamily != QLatin1String("pacman")))
        return {};

    if (!customProvisionDir.isEmpty()) {
        QFile custom(customProvisionDir + QLatin1Char('/') + cleanId + QLatin1Char('.') + distroFamily + QStringLiteral(".sh"));
        if (custom.open(QIODevice::ReadOnly)) {
            const QString content = QString::fromUtf8(custom.readAll()).trimmed();
            if (!content.isEmpty())
                return content;
        }
    }

    if (distroFamily == QLatin1String("apt")) {
        if (cleanId == QLatin1String("devtools"))
            return QStringLiteral("export DEBIAN_FRONTEND=noninteractive\napt-get update && apt-get install -y --no-install-recommends git curl jq ca-certificates build-essential");
        if (cleanId == QLatin1String("node"))
            return QStringLiteral("export DEBIAN_FRONTEND=noninteractive\napt-get update && apt-get install -y --no-install-recommends nodejs npm ca-certificates");
        if (cleanId == QLatin1String("python"))
            return QStringLiteral("export DEBIAN_FRONTEND=noninteractive\napt-get update && apt-get install -y --no-install-recommends python3 python3-pip python3-venv");
        if (cleanId == QLatin1String("rust"))
            return QStringLiteral("export DEBIAN_FRONTEND=noninteractive\napt-get update && apt-get install -y --no-install-recommends rustc cargo");
    } else if (distroFamily == QLatin1String("pacman")) {
        if (cleanId == QLatin1String("devtools"))
            return QStringLiteral("pacman -Sy --noconfirm --needed git curl jq base-devel");
        if (cleanId == QLatin1String("node"))
            return QStringLiteral("pacman -Sy --noconfirm --needed nodejs npm");
        if (cleanId == QLatin1String("python"))
            return QStringLiteral("pacman -Sy --noconfirm --needed python python-pip");
        if (cleanId == QLatin1String("rust"))
            return QStringLiteral("pacman -Sy --noconfirm --needed rust");
    }
    return {};
}

QString Backend::buildProvisionShellSnippet(const QStringList &bundleIds, const QString &osId,
                                            const QString &osAlias, const QString &customProvisionDir) {
    const QString family = distroFamilyForOsId(osId, osAlias);
    if (bundleIds.isEmpty() || family.isEmpty())
        return {};

    QStringList blocks;
    int idx = 0;
    for (const QString &bundleId : bundleIds) {
        const QString cleanId = sanitizeName(bundleId);
        const QString scriptBody = provisionScriptForBundle(cleanId, family, customProvisionDir);
        if (scriptBody.isEmpty())
            continue;
        ++idx;
        const int pct = qMin(98, 88 + idx * 2);
        const QString delim = QStringLiteral("__AIBOX_PROV_%1_%2__").arg(cleanId).arg(idx);
        blocks << QStringLiteral(
                      "echo '@@STAGE:%1:Provisioning bundle (%2)...'\n"
                      "if ! incus exec \"$NAME\" -- sh -s <<'%3'\n"
                      "set -e\n"
                      "%4\n"
                      "%3\n"
                      "then\n"
                      "    echo '[provision] WARN: Bundle %2 failed, continuing...'\n"
                      "fi")
                      .arg(QString::number(pct), cleanId, delim, scriptBody);
    }
    return blocks.join(QLatin1Char('\n'));
}

void Backend::seedProvisionScripts() const {
    if (m_demoMode)
        return;
    const QString dir = provisionDirPath();
    if (!QDir().mkpath(dir))
        return;
    const QStringList families = {QStringLiteral("apt"), QStringLiteral("pacman")};
    for (const auto &v : defaultProvisionBundles()) {
        const QString id = v.toMap().value("id").toString();
        for (const QString &fam : families) {
            const QString target = dir + QLatin1Char('/') + id + QLatin1Char('.') + fam + QStringLiteral(".sh");
            if (QFileInfo::exists(target))
                continue;
            const QString body = provisionScriptForBundle(id, fam);
            if (!body.isEmpty()) {
                QFile f(target);
                if (f.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
                    f.write(QStringLiteral("#!/bin/sh\n").toUtf8() + body.toUtf8() + QByteArray("\n"));
                }
            }
        }
    }
}

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

QList<Backend::HealthCheckResult> Backend::runHealthChecks(bool fix, const CommandRunner &customRunner,
                                                           const QString &customUser,
                                                           const QString &ufwRulesContent,
                                                           const QString &workspaceBase) {
    CommandRunner run = customRunner;
    if (!run) {
        run = [](const QString &program, const QStringList &args) -> CommandOutput {
            CommandOutput out;
            if (program == QLatin1String("__socket_rw__") && !args.isEmpty()) {
                const QString sock = args.first();
                if (!QFileInfo::exists(sock)) {
                    out.exitCode = 1;
                    out.stdErr = QStringLiteral("Socket file not found.");
                    return out;
                }
                const bool rw = (::access(sock.toLocal8Bit().constData(), R_OK | W_OK) == 0);
                out.exitCode = rw ? 0 : 1;
                out.stdErr = rw ? QString() : QStringLiteral("No read/write access on socket.");
                return out;
            }
            QProcess proc;
            proc.start(program, args);
            if (!proc.waitForFinished(8000)) {
                proc.kill();
                out.exitCode = 124;
                out.stdErr = QStringLiteral("Command timed out.");
                return out;
            }
            out.exitCode = proc.exitCode();
            out.stdOut = QString::fromUtf8(proc.readAllStandardOutput());
            out.stdErr = QString::fromUtf8(proc.readAllStandardError());
            return out;
        };
    }

    QString user = customUser.trimmed();
    if (user.isEmpty())
        user = qEnvironmentVariable("USER").trimmed();
    if (user.isEmpty())
        user = QStringLiteral("user");

    QList<HealthCheckResult> results;

    // 1. User group memberships
    {
        HealthCheckResult r;
        r.id = QStringLiteral("groups");
        r.title = QStringLiteral("User group membership");
        const CommandOutput out = run(QStringLiteral("id"), {QStringLiteral("-nG"), user});
        const QStringList groups = out.stdOut.split(QRegularExpression(QStringLiteral("\\s+")), Qt::SkipEmptyParts);
        const QStringList required = {QStringLiteral("incus-admin"), QStringLiteral("kvm"),
                                      QStringLiteral("render"), QStringLiteral("video")};
        QStringList missing;
        for (const QString &req : required) {
            if (!groups.contains(req))
                missing << req;
        }
        if (out.exitCode == 0 && missing.isEmpty()) {
            r.ok = true;
            r.detail = QStringLiteral("User '%1' belongs to incus-admin, kvm, render, video.").arg(user);
        } else {
            r.ok = false;
            r.needsRoot = true;
            r.detail = missing.isEmpty()
                           ? QStringLiteral("Could not inspect groups for '%1'.").arg(user)
                           : QStringLiteral("Missing group(s): %1").arg(missing.join(QStringLiteral(", ")));
            r.fixCommand = QStringLiteral("sudo usermod -aG incus-admin,kvm,render,video %1").arg(user);
        }
        results.append(r);
    }

    // 2. Incus UNIX socket ACL
    {
        HealthCheckResult r;
        r.id = QStringLiteral("socket_acl");
        r.title = QStringLiteral("Incus socket ACL");
        const CommandOutput out = run(QStringLiteral("__socket_rw__"), {QStringLiteral("/var/lib/incus/unix.socket")});
        if (out.exitCode == 0) {
            r.ok = true;
            r.detail = QStringLiteral("Read/write access confirmed on /var/lib/incus/unix.socket.");
        } else {
            r.ok = false;
            r.needsRoot = true;
            r.detail = out.stdErr.trimmed().isEmpty()
                           ? QStringLiteral("Missing R/W permission on /var/lib/incus/unix.socket.")
                           : out.stdErr.trimmed();
            r.fixCommand = QStringLiteral("sudo setfacl -m u:%1:rw /var/lib/incus/unix.socket").arg(user);
        }
        results.append(r);
    }

    // 3. Incus daemon reachability
    bool daemonReachable = false;
    QByteArray cachedListJson;
    {
        HealthCheckResult r;
        r.id = QStringLiteral("incus_daemon");
        r.title = QStringLiteral("Incus daemon reachable");
        const CommandOutput out = run(QStringLiteral("incus"), {QStringLiteral("list"), QStringLiteral("--format=json")});
        if (out.exitCode == 0) {
            daemonReachable = true;
            cachedListJson = out.stdOut.toUtf8();
            r.ok = true;
            r.detail = QStringLiteral("Incus daemon responded cleanly.");
        } else {
            r.ok = false;
            r.needsRoot = true;
            r.detail = out.stdErr.trimmed().isEmpty()
                           ? QStringLiteral("Incus daemon is not reachable.")
                           : out.stdErr.trimmed();
            r.fixCommand = QStringLiteral("sudo systemctl enable --now incus.socket incus.service");
        }
        results.append(r);
    }

    // 4. Bridge network incusbr0
    {
        HealthCheckResult r;
        r.id = QStringLiteral("bridge_incusbr0");
        r.title = QStringLiteral("Bridge network incusbr0");
        auto checkBridge = [&]() -> bool {
            const CommandOutput out = run(QStringLiteral("incus"), {QStringLiteral("network"), QStringLiteral("show"), QStringLiteral("incusbr0")});
            return out.exitCode == 0;
        };
        if (checkBridge()) {
            r.ok = true;
            r.detail = QStringLiteral("Network bridge incusbr0 is configured.");
        } else {
            if (fix && daemonReachable) {
                const CommandOutput rep = run(QStringLiteral("incus"),
                                              {QStringLiteral("network"), QStringLiteral("create"), QStringLiteral("incusbr0"),
                                               QStringLiteral("ipv4.address=auto"), QStringLiteral("ipv4.nat=true"),
                                               QStringLiteral("ipv6.address=none")});
                if (rep.exitCode == 0 && checkBridge()) {
                    r.ok = true;
                    r.repaired = true;
                    r.detail = QStringLiteral("Created bridge incusbr0 automatically.");
                }
            }
            if (!r.ok) {
                r.needsRoot = !daemonReachable;
                r.detail = QStringLiteral("Managed bridge incusbr0 is missing.");
                r.fixCommand = QStringLiteral("incus network create incusbr0 ipv4.address=auto ipv4.nat=true ipv6.address=none");
            }
        }
        results.append(r);
    }

    // 5. ai-gui profile
    {
        HealthCheckResult r;
        r.id = QStringLiteral("ai_gui_profile");
        r.title = QStringLiteral("ai-gui profile (nesting, gpu, kvm)");
        auto inspectProfile = [&]() -> std::pair<bool, QString> {
            const CommandOutput out = run(QStringLiteral("incus"), {QStringLiteral("profile"), QStringLiteral("show"), QStringLiteral("ai-gui")});
            if (out.exitCode != 0)
                return {false, QStringLiteral("Profile ai-gui does not exist.")};
            const QString txt = out.stdOut;
            QStringList missing;
            if (!txt.contains(QStringLiteral("security.nesting")))
                missing << QStringLiteral("security.nesting=true");
            if (!txt.contains(QStringLiteral("gpu:")))
                missing << QStringLiteral("gpu");
            if (!txt.contains(QStringLiteral("kvm:")) && !txt.contains(QStringLiteral("/dev/kvm")))
                missing << QStringLiteral("kvm");
            if (!missing.isEmpty())
                return {false, QStringLiteral("Profile ai-gui missing: %1").arg(missing.join(QStringLiteral(", ")))};
            return {true, QStringLiteral("Profile ai-gui configured with nesting, gpu, and kvm.")};
        };

        auto [okNow, detailNow] = inspectProfile();
        if (okNow) {
            r.ok = true;
            r.detail = detailNow;
        } else {
            if (fix && daemonReachable) {
                const CommandOutput showBefore = run(QStringLiteral("incus"), {QStringLiteral("profile"), QStringLiteral("show"), QStringLiteral("ai-gui")});
                if (showBefore.exitCode != 0)
                    run(QStringLiteral("incus"), {QStringLiteral("profile"), QStringLiteral("create"), QStringLiteral("ai-gui")});
                run(QStringLiteral("incus"), {QStringLiteral("profile"), QStringLiteral("set"), QStringLiteral("ai-gui"), QStringLiteral("security.nesting=true")});
                const CommandOutput showMid = run(QStringLiteral("incus"), {QStringLiteral("profile"), QStringLiteral("show"), QStringLiteral("ai-gui")});
                if (!showMid.stdOut.contains(QStringLiteral("gpu:")))
                    run(QStringLiteral("incus"), {QStringLiteral("profile"), QStringLiteral("device"), QStringLiteral("add"), QStringLiteral("ai-gui"), QStringLiteral("gpu"), QStringLiteral("gpu")});
                if (!showMid.stdOut.contains(QStringLiteral("kvm:")) && !showMid.stdOut.contains(QStringLiteral("/dev/kvm")))
                    run(QStringLiteral("incus"), {QStringLiteral("profile"), QStringLiteral("device"), QStringLiteral("add"), QStringLiteral("ai-gui"), QStringLiteral("kvm"), QStringLiteral("unix-char"), QStringLiteral("path=/dev/kvm")});

                auto [okAfter, detailAfter] = inspectProfile();
                if (okAfter) {
                    r.ok = true;
                    r.repaired = true;
                    r.detail = QStringLiteral("Repaired ai-gui profile automatically.");
                } else {
                    detailNow = detailAfter;
                }
            }
            if (!r.ok) {
                r.needsRoot = !daemonReachable;
                r.detail = detailNow;
                r.fixCommand = QStringLiteral("incus profile create ai-gui; incus profile set ai-gui security.nesting=true; incus profile device add ai-gui gpu gpu; incus profile device add ai-gui kvm unix-char path=/dev/kvm");
            }
        }
        results.append(r);
    }

    // 6. UFW firewall rules for incusbr0
    {
        HealthCheckResult r;
        r.id = QStringLiteral("ufw_rules");
        r.title = QStringLiteral("UFW firewall rules for incusbr0");
        const CommandOutput ufwState = run(QStringLiteral("systemctl"), {QStringLiteral("is-active"), QStringLiteral("ufw")});
        if (ufwState.stdOut.trimmed() != QStringLiteral("active")) {
            r.ok = true;
            r.detail = QStringLiteral("UFW firewall is inactive.");
        } else {
            QString rules = ufwRulesContent;
            bool readOk = !rules.isEmpty();
            if (!readOk) {
                QFile rf(QStringLiteral("/etc/ufw/user.rules"));
                if (rf.open(QIODevice::ReadOnly)) {
                    rules = QString::fromUtf8(rf.readAll());
                    readOk = true;
                }
            }
            const bool hasIn = rules.contains(QRegularExpression(QStringLiteral("ufw-user-input\\s+-i\\s+incusbr0\\s+-j\\s+ACCEPT")));
            const bool hasFwdIn = rules.contains(QRegularExpression(QStringLiteral("ufw-user-forward\\s+-i\\s+incusbr0\\s+-j\\s+ACCEPT")));
            const bool hasFwdOut = rules.contains(QRegularExpression(QStringLiteral("ufw-user-forward\\s+-o\\s+incusbr0\\s+-j\\s+ACCEPT")));
            if (readOk && hasIn && hasFwdIn && hasFwdOut) {
                r.ok = true;
                r.detail = QStringLiteral("UFW allows input & routed traffic on incusbr0.");
            } else {
                r.ok = false;
                r.needsRoot = true;
                r.detail = readOk
                               ? QStringLiteral("UFW is missing allow/route rules for incusbr0.")
                               : QStringLiteral("Could not inspect /etc/ufw/user.rules.");
                r.fixCommand = QStringLiteral("sudo ufw allow in on incusbr0 && sudo ufw route allow in on incusbr0 && sudo ufw route allow out on incusbr0 && sudo ufw reload");
            }
        }
        results.append(r);
    }

    // 7. Workspace Btrfs user_subvol_rm_allowed mount option (S0/S1)
    {
        HealthCheckResult r;
        r.id = QStringLiteral("workspace_subvol_rm");
        r.title = QStringLiteral("Workspace Btrfs subvolume deletion");
        QString probePath = QDir::cleanPath(workspaceBase.trimmed().isEmpty() ? defaultWorkspacePath() : workspaceBase.trimmed());
        for (int i = 0; i < 16 && !probePath.isEmpty() && !QFileInfo::exists(probePath); ++i) {
            const QString parent = QFileInfo(probePath).dir().path();
            if (parent == probePath)
                break;
            probePath = parent;
        }
        const CommandOutput out = run(QStringLiteral("findmnt"),
                                      {QStringLiteral("-rn"), QStringLiteral("-o"),
                                       QStringLiteral("TARGET,FSTYPE,OPTIONS"),
                                       QStringLiteral("-T"), probePath});
        if (out.exitCode != 0 || out.stdOut.trimmed().isEmpty()) {
            r.ok = true;
            r.detail = QStringLiteral("Could not query mount options for %1.").arg(probePath);
        } else {
            const QStringList cols = out.stdOut.trimmed().split(QLatin1Char(' '), Qt::SkipEmptyParts);
            const QString mountTarget = cols.value(0);
            const QString fsType = cols.value(1).toLower();
            const QStringList opts = cols.value(2).split(QLatin1Char(','), Qt::SkipEmptyParts);
            if (fsType != QLatin1String("btrfs")) {
                r.ok = true;
                r.detail = QStringLiteral("Workspace mount %1 uses %2 (user_subvol_rm_allowed not applicable).")
                               .arg(mountTarget, fsType);
            } else if (opts.contains(QStringLiteral("user_subvol_rm_allowed"))) {
                r.ok = true;
                r.detail = QStringLiteral("Btrfs mount %1 enables user_subvol_rm_allowed.").arg(mountTarget);
            } else {
                r.ok = false;
                r.needsRoot = true;
                r.detail = QStringLiteral("Btrfs mount %1 lacks user_subvol_rm_allowed; btrfs subvolume delete needs sudo (add option to /etc/fstab).").arg(mountTarget);
                r.fixCommand = QStringLiteral("sudo mount -o remount,user_subvol_rm_allowed %1").arg(mountTarget);
            }
        }
        results.append(r);
    }

    // 8. Incus storage pool capacity & overcommit check (H2)
    {
        StoragePoolInfo poolInfo;
        if (daemonReachable) {
            CommandOutput stOut = run(QStringLiteral("incus"),
                                      {QStringLiteral("storage"), QStringLiteral("info"),
                                       QStringLiteral("default"), QStringLiteral("--bytes")});
            if (stOut.exitCode != 0 || stOut.stdOut.trimmed().isEmpty()) {
                stOut = run(QStringLiteral("incus"),
                            {QStringLiteral("storage"), QStringLiteral("info"), QStringLiteral("default")});
            }
            if (stOut.exitCode == 0)
                poolInfo = parseIncusStorageInfo(stOut.stdOut);
        }
        int unboundedCount = 0;
        const qint64 allocatedBytes = sumRootDiskQuotasBytes(cachedListJson, {}, poolInfo.name, &unboundedCount);
        results.append(evaluatePoolCapacityHealth(poolInfo, allocatedBytes, unboundedCount));
    }

    return results;
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
        e1["cpuCores"] = 4;
        e1["cpuUsage"] = QStringLiteral("18%");
        e1["cpuPercent"] = 18;
        e1["cpuCorePercents"] = QVariantList{24, 19, 16, 13};
        e1["memoryLimit"] = QStringLiteral("8GiB");
        e1["memoryUsage"] = QStringLiteral("1.4 GB");
        e1["memoryPercent"] = 18;
        e1["diskLimit"] = QStringLiteral("60GiB");
        e1["diskUsage"] = QStringLiteral("8.2 GB");
        e1["diskUsageBytes"] = qint64(8804682956LL);
        e1["diskPercent"] = 14;
        e1["processLimit"] = QStringLiteral("512");
        e1["diskReadLimit"] = QStringLiteral("200MB");
        e1["diskWriteLimit"] = QStringLiteral("200MB");
        e1["ipv4"] = QStringLiteral("10.143.44.18");
        e1["netRxBytes"] = qint64(39012732109LL);
        e1["netTxBytes"] = qint64(7365862195LL);
        e1["netRxRate"] = qint64(153293);
        e1["netTxRate"] = qint64(14234);
        e1["netRxRateText"] = QStringLiteral("149.7 KB/s");
        e1["netTxRateText"] = QStringLiteral("13.9 KB/s");
        e1["netRxTotalText"] = QStringLiteral("36.33 GB");
        e1["netTxTotalText"] = QStringLiteral("6.86 GB");
        e1["hostInterface"] = QStringLiteral("veth-demo1");
        e1["workspacePath"] = QStringLiteral("~/ai-workspaces/flutter-pos");
        e1["workspaceOnBtrfs"] = true;
        e1["workspaceIsSubvolume"] = true;
        e1["workspaceSnapshotOverride"] = QString();
        e1["workspaceSnapshotEnabled"] = true;
        e1["workspaceSnapshotDir"] = QStringLiteral("~/ai-workspaces/.snapshots/flutter-pos");
        e1["workspaceSnapshots"] = QStringList{QStringLiteral("clean-sdk"), QStringLiteral("before-ai-refactor"), QStringLiteral("auto-20261113-120000")};
        e1["orphanWorkspaceSnapshots"] = QStringList{};
        e1["workspaceSnapshotSizes"] = QVariantMap{
            {QStringLiteral("clean-sdk"), QStringLiteral("145 MB")},
            {QStringLiteral("before-ai-refactor"), QStringLiteral("152 MB")},
            {QStringLiteral("auto-20261113-120000"), QStringLiteral("152 MB")}
        };
        e1["hasPreSubvolBackup"] = false;
        e1["snapshotCount"] = 3;
        e1["autoSnapshotCount"] = 1;
        e1["manualSnapshotCount"] = 2;
        e1["snapshots"] = QStringList{QStringLiteral("clean-sdk"), QStringLiteral("before-ai-refactor"), QStringLiteral("auto-20261113-120000")};
        e1["snapshotSizes"] = QVariantMap{
            {QStringLiteral("clean-sdk"), QStringLiteral("420 MB")},
            {QStringLiteral("before-ai-refactor"), QStringLiteral("1.1 GB")},
            {QStringLiteral("auto-20261113-120000"), QStringLiteral("185 MB")}
        };
        e1["autoSnapshotOverride"] = QString();
        e1["autoSnapshotEnabled"] = true;
        e1["autoStopOverride"] = QString();
        e1["autoStopOnCritical"] = false;
        e1["idleStopOverride"] = QString();
        e1["idleStopIgnorePorts"] = false;
        e1["diskAlertLevel"] = 0;
        e1["isTemplate"] = false;
        e1["networkMode"] = QStringLiteral("online");
        e1["networkAllowlist"] = QString();
        e1["networkBlocked"] = false;
        e1["egressRecording"] = true;
        e1["portForwards"] = QVariantList{
            QVariantMap{{"deviceName", QStringLiteral("fwd-3000")}, {"hostPort", 3000}, {"guestPort", 3000}},
            QVariantMap{{"deviceName", QStringLiteral("fwd-8080")}, {"hostPort", 8080}, {"guestPort", 8080}}
        };
        e1["portForwardCount"] = 2;
        e1["extraMounts"] = QVariantList{
            QVariantMap{{"deviceName", QStringLiteral("mnt-datasets")}, {"source", QStringLiteral("/home/user/datasets")}, {"path", QStringLiteral("/mnt/datasets")}, {"readonly", true}}
        };
        e1["extraMountCount"] = 1;
        e1["secretNames"] = QStringList{QStringLiteral("ANTHROPIC_API_KEY")};
        e1["snapshotTotalBytes"] = qint64(1787822080LL);
        e1["snapshotTotalDisplay"] = QStringLiteral("1.66 GB");
        e1["lastUsedEpoch"] = QDateTime::currentSecsSinceEpoch() - (2LL * 86400LL);
        e1["recordOverride"] = QString();
        e1["recordEnabled"] = m_recordSessions;
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
        e2["cpuCores"] = 6;
        e2["cpuUsage"] = QStringLiteral("9%");
        e2["cpuPercent"] = 9;
        e2["cpuCorePercents"] = QVariantList{14, 11, 9, 8, 7, 5};
        e2["memoryLimit"] = QStringLiteral("6GiB");
        e2["memoryUsage"] = QStringLiteral("620 MB");
        e2["memoryPercent"] = 10;
        e2["diskLimit"] = QStringLiteral("35GiB");
        e2["diskUsage"] = QStringLiteral("32.2 GB");
        e2["diskPercent"] = 92;
        e2["processLimit"] = QStringLiteral("1024");
        e2["diskReadLimit"] = QString();
        e2["diskWriteLimit"] = QString();
        e2["ipv4"] = QStringLiteral("10.143.44.42");
        e2["netRxBytes"] = qint64(2587711488LL);
        e2["netTxBytes"] = qint64(934281216LL);
        e2["netRxRate"] = qint64(24883);
        e2["netTxRate"] = qint64(3174);
        e2["netRxRateText"] = QStringLiteral("24.3 KB/s");
        e2["netTxRateText"] = QStringLiteral("3.1 KB/s");
        e2["netRxTotalText"] = QStringLiteral("2.41 GB");
        e2["netTxTotalText"] = QStringLiteral("891.0 MB");
        e2["hostInterface"] = QStringLiteral("veth-demo2");
        e2["workspacePath"] = QStringLiteral("~/ai-workspaces/desktop-studio");
        e2["snapshotCount"] = 1;
        e2["autoSnapshotCount"] = 0;
        e2["manualSnapshotCount"] = 1;
        e2["snapshots"] = QStringList{QStringLiteral("initial-setup")};
        e2["snapshotSizes"] = QVariantMap{{QStringLiteral("initial-setup"), QStringLiteral("640 MB")}};
        e2["autoSnapshotOverride"] = QString();
        e2["autoSnapshotEnabled"] = true;
        e2["autoStopOverride"] = QStringLiteral("true");
        e2["autoStopOnCritical"] = true;
        e2["diskAlertLevel"] = 1;
        e2["isTemplate"] = false;
        e2["networkMode"] = QStringLiteral("allowlist");
        e2["networkAllowlist"] = QStringLiteral("github.com, api.anthropic.com");
        e2["networkBlocked"] = false;
        e2["egressRecording"] = true;
        e2["snapshotTotalBytes"] = qint64(671088640LL);
        e2["snapshotTotalDisplay"] = QStringLiteral("640 MB");
        e2["lastUsedEpoch"] = QDateTime::currentSecsSinceEpoch() - (45LL * 86400LL);
        e2["recordOverride"] = QStringLiteral("true");
        e2["recordEnabled"] = true;
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
        e3["cpuCores"] = 2;
        e3["cpuUsage"] = QStringLiteral("0%");
        e3["cpuPercent"] = 0;
        e3["cpuCorePercents"] = QVariantList{0, 0};
        e3["memoryLimit"] = QStringLiteral("4GiB");
        e3["memoryUsage"] = QStringLiteral("0 MB");
        e3["memoryPercent"] = 0;
        e3["diskLimit"] = QStringLiteral("20GiB");
        e3["diskUsage"] = QStringLiteral("1.1 GB");
        e3["diskPercent"] = 6;
        e3["processLimit"] = QStringLiteral("512");
        e3["diskReadLimit"] = QString();
        e3["diskWriteLimit"] = QString();
        e3["ipv4"] = QStringLiteral("—");
        e3["hostInterface"] = QString();
        e3["workspacePath"] = QStringLiteral("~/ai-workspaces/saas-backend");
        e3["snapshotCount"] = 0;
        e3["autoSnapshotCount"] = 0;
        e3["manualSnapshotCount"] = 0;
        e3["snapshots"] = QStringList{};
        e3["snapshotSizes"] = QVariantMap{};
        e3["autoSnapshotOverride"] = QStringLiteral("false");
        e3["autoSnapshotEnabled"] = false;
        e3["autoStopOverride"] = QString();
        e3["autoStopOnCritical"] = false;
        e3["diskAlertLevel"] = 0;
        e3["isTemplate"] = true;
        e3["networkMode"] = QStringLiteral("offline");
        e3["networkAllowlist"] = QString();
        e3["networkBlocked"] = true;
        e3["egressRecording"] = false;
        e3["recordOverride"] = QStringLiteral("false");
        e3["recordEnabled"] = false;
        demoEnvs.append(e3);
    }
    m_environments = demoEnvs;
    m_count = m_environments.size();
    m_netProbeEnv = QStringLiteral("flutter-pos");
    m_netGateway = QStringLiteral("10.143.44.1");
    m_netPingMs = 30.0;
    m_netPacketLoss = 0.0;
    updateNetworkStats();

    m_operationHistory = {
        QVariantMap{
            {"id", QStringLiteral("demo-1")},
            {"description", tr("Updating network mode for '%1'").arg(QStringLiteral("desktop-studio"))},
            {"envName", QStringLiteral("desktop-studio")},
            {"status", QStringLiteral("success")},
            {"exitCode", 0},
            {"timestamp", QStringLiteral("2026-11-13 13:10:02")},
            {"startedAt", QStringLiteral("2026-11-13T13:10:00")},
            {"durationMs", 2000},
            {"logFile", QString()}
        },
        QVariantMap{
            {"id", QStringLiteral("demo-2")},
            {"description", tr("Saving snapshot '%1' for '%2'").arg(QStringLiteral("before-ai-refactor"), QStringLiteral("flutter-pos"))},
            {"envName", QStringLiteral("flutter-pos")},
            {"status", QStringLiteral("success")},
            {"exitCode", 0},
            {"timestamp", QStringLiteral("2026-11-13 12:45:13")},
            {"startedAt", QStringLiteral("2026-11-13T12:45:12")},
            {"durationMs", 1000},
            {"logFile", QString()}
        }
    };

    if (m_secrets.isEmpty()) {
        m_secrets = {
            {QStringLiteral("ANTHROPIC_API_KEY"), QStringLiteral("sk-ant-demo-0000")},
            {QStringLiteral("GITHUB_TOKEN"), QStringLiteral("ghp_demo_0000")}
        };
    }

    m_backupArchives = {
        QVariantMap{
            {"fileName", QStringLiteral("flutter-pos-20261112-194000.tar.gz")},
            {"filePath", backupsDirPath() + QStringLiteral("/flutter-pos-20261112-194000.tar.gz")},
            {"envName", QStringLiteral("flutter-pos")},
            {"sizeBytes", qint64(2415919104LL)},
            {"sizeDisplay", QStringLiteral("2.2 GB")},
            {"createdAt", QStringLiteral("2026-11-12 19:40")},
            {"hasWorkspaceArchive", true},
            {"workspaceArchivePath", backupsDirPath() + QStringLiteral("/flutter-pos-20261112-194000-workspace.tar.gz")},
            {"workspaceArchiveSizeDisplay", QStringLiteral("148 MB")}
        }
    };

    m_lastPoolInfo = StoragePoolInfo{
        true,
        QStringLiteral("default"),
        QStringLiteral("btrfs"),
        qint64(19756849562LL),
        qint64(85899345920LL),
        23,
        QStringLiteral("80GiB"),
        true
    };
    m_lastPoolAllocatedBytes = qint64(80) * 1024LL * 1024LL * 1024LL;
    m_lastPoolUnboundedCount = 0;
    m_poolInfo = poolInfoToVariantMap(m_lastPoolInfo, m_lastPoolAllocatedBytes, 0);
    m_recordedSessions = {
        QVariantMap{
            {"id", QStringLiteral("desktop-studio/1794400000")},
            {"envName", QStringLiteral("desktop-studio")},
            {"epoch", qint64(1794400000LL)},
            {"timestamp", QDateTime::fromSecsSinceEpoch(1794400000LL).toString(QStringLiteral("yyyy-MM-dd HH:mm:ss"))},
            {"logPath", QStringLiteral("demo://desktop-studio/1794400000.log")},
            {"timingPath", QStringLiteral("demo://desktop-studio/1794400000.timing")},
            {"hasTiming", true},
            {"sizeBytes", qint64(4280)},
            {"sizeDisplay", QStringLiteral("4 KB")}
        }
    };
    refreshStaleReport();
}

QStringList Backend::defaultDiffExcludes() {
    return {
        QStringLiteral(".git/"),
        QStringLiteral("node_modules/"),
        QStringLiteral("target/"),
        QStringLiteral(".venv/"),
        QStringLiteral("__pycache__/"),
        QStringLiteral("build/")
    };
}

QStringList Backend::parseDiffExcludes(const QString &rawText) {
    static const QRegularExpression sep(QStringLiteral("[,;\\r\\n]+"));
    QStringList out;
    for (const QString &part : rawText.split(sep, Qt::SkipEmptyParts)) {
        const QString clean = part.trimmed();
        if (!clean.isEmpty() && !out.contains(clean))
            out.append(clean);
    }
    return out;
}

QStringList Backend::buildRsyncExcludeArgs(const QStringList &excludes) {
    QStringList args;
    for (const QString &raw : excludes) {
        const QString clean = raw.trimmed();
        if (!clean.isEmpty())
            args.append(QStringLiteral("--exclude=") + clean);
    }
    return args;
}

bool Backend::isSafeRelativePath(const QString &relativePath) {
    if (relativePath.isEmpty() || relativePath.startsWith(QLatin1Char('/')) ||
        relativePath.contains(QLatin1Char('\0')))
        return false;
    const QStringList parts = relativePath.split(QLatin1Char('/'), Qt::SkipEmptyParts);
    if (parts.isEmpty())
        return false;
    for (const QString &seg : parts) {
        if (seg == QLatin1String(".") || seg == QLatin1String(".."))
            return false;
    }
    return true;
}

bool Backend::isLikelyBinaryData(const QByteArray &sample) {
    return sample.left(8192).contains('\0');
}

QString Backend::capDiffLines(const QString &diffOutput, int maxLines, bool *outTruncated) {
    const int limit = qMax(1, maxLines);
    const QStringList lines = diffOutput.split(QLatin1Char('\n'));
    if (lines.size() <= limit) {
        if (outTruncated)
            *outTruncated = false;
        return diffOutput;
    }
    if (outTruncated)
        *outTruncated = true;
    return lines.mid(0, limit).join(QLatin1Char('\n'));
}

QVariantMap Backend::parseRsyncItemize(const QString &output, int maxEntries) {
    static const QRegularExpression lineRe(
        QStringLiteral("^(\\*deleting\\s+|[<>ch.*][fdLDS][^\\s]{9})\\s+(\\d+)\\s+(.+)$"));
    const int cap = qMax(1, maxEntries);

    QVariantList entries;
    int addedCount = 0;
    int modifiedCount = 0;
    int deletedCount = 0;
    bool truncated = false;

    const QStringList lines = output.split(QLatin1Char('\n'), Qt::SkipEmptyParts);
    for (const QString &rawLine : lines) {
        const QString line = rawLine.endsWith(QLatin1Char('\r')) ? rawLine.chopped(1) : rawLine;
        const auto m = lineRe.match(line);
        if (!m.hasMatch())
            continue;

        const QString code = m.captured(1);
        const qint64 sizeBytes = m.captured(2).toLongLong();
        QString relPath = m.captured(3);

        if (relPath == QLatin1String("./") || relPath == QLatin1String("."))
            continue;
        if (code.startsWith(QLatin1String(".d")))
            continue;

        QString status;
        QString entryType = QStringLiteral("file");
        if (code.startsWith(QLatin1String("*deleting"))) {
            status = QStringLiteral("added");
            if (relPath.endsWith(QLatin1Char('/'))) {
                entryType = QStringLiteral("dir");
                relPath.chop(1);
            }
        } else {
            const QChar typeChar = code.at(1);
            if (typeChar == QLatin1Char('d')) {
                entryType = QStringLiteral("dir");
                if (relPath.endsWith(QLatin1Char('/')))
                    relPath.chop(1);
            } else if (typeChar == QLatin1Char('L')) {
                entryType = QStringLiteral("symlink");
            }
            if (code.mid(2, 9) == QLatin1String("+++++++++"))
                status = QStringLiteral("deleted");
            else
                status = QStringLiteral("modified");
        }

        if (relPath.isEmpty())
            continue;

        if (entries.size() >= cap) {
            truncated = true;
            break;
        }

        if (status == QLatin1String("added"))
            ++addedCount;
        else if (status == QLatin1String("deleted"))
            ++deletedCount;
        else
            ++modifiedCount;

        QVariantMap row;
        row["path"] = relPath;
        row["status"] = status;
        row["type"] = entryType;
        row["sizeBytes"] = sizeBytes;
        row["sizeText"] = (entryType == QLatin1String("dir")) ? QStringLiteral("dir") : formatBytes(sizeBytes);
        entries.append(row);
    }

    QVariantMap out;
    out["entries"] = entries;
    out["truncated"] = truncated;
    out["addedCount"] = addedCount;
    out["modifiedCount"] = modifiedCount;
    out["deletedCount"] = deletedCount;
    out["totalCount"] = entries.size();
    return out;
}

QStringList Backend::buildWorkspaceDiffArgs(const QString &snapPath, const QString &wsPath,
                                             const QStringList &excludes) {
    QStringList args{
        QStringLiteral("-a"),
        QStringLiteral("-n"),
        QStringLiteral("--delete"),
        QStringLiteral("--itemize-changes"),
        QStringLiteral("--out-format=%i %l %n")
    };
    args.append(buildRsyncExcludeArgs(excludes));
    args.append(QDir::cleanPath(snapPath) + QLatin1Char('/'));
    args.append(QDir::cleanPath(wsPath) + QLatin1Char('/'));
    return args;
}

QVariantMap Backend::processWorkspaceDiffOutput(const QString &snapshotName, const QString &wsPath,
                                                const QString &rsyncStdout, QVariantList *entriesOut) {
    const QVariantMap parsed = parseRsyncItemize(rsyncStdout, 5000);
    QVariantList entries = parsed.value("entries").toList();
    for (int i = 0; i < entries.size(); ++i) {
        QVariantMap row = entries.at(i).toMap();
        if (row.value("status").toString() == QLatin1String("added") &&
            row.value("type").toString() != QLatin1String("dir")) {
            const QFileInfo fi(wsPath + QLatin1Char('/') + row.value("path").toString());
            if (fi.isSymLink()) {
                row["type"] = QStringLiteral("symlink");
            } else if (fi.exists()) {
                row["sizeBytes"] = fi.size();
                row["sizeText"] = formatBytes(fi.size());
            }
            entries[i] = row;
        }
    }
    if (entriesOut)
        *entriesOut = entries;
    return QVariantMap{
        {"snapshotName", snapshotName},
        {"addedCount", parsed.value("addedCount").toInt()},
        {"modifiedCount", parsed.value("modifiedCount").toInt()},
        {"deletedCount", parsed.value("deletedCount").toInt()},
        {"totalCount", parsed.value("totalCount").toInt()},
        {"truncated", parsed.value("truncated").toBool()},
        {"error", QString()}
    };
}

void Backend::loadWorkspaceDiff(const QString &name, const QString &snapshotName) {
    if (name.isEmpty() || snapshotName.isEmpty())
        return;

    m_selectedDiffText.clear();
    m_selectedDiffInfo.clear();
    emit selectedDiffChanged();

    if (m_demoMode) {
        m_diffLoading = false;
        m_diffEntries = {
            QVariantMap{{"path", QStringLiteral("lib/main.dart")}, {"status", QStringLiteral("modified")}, {"type", QStringLiteral("file")}, {"sizeBytes", 4120}, {"sizeText", QStringLiteral("4 KB")}},
            QVariantMap{{"path", QStringLiteral("lib/services/auth.dart")}, {"status", QStringLiteral("added")}, {"type", QStringLiteral("file")}, {"sizeBytes", 1840}, {"sizeText", QStringLiteral("2 KB")}},
            QVariantMap{{"path", QStringLiteral("lib/legacy_helper.dart")}, {"status", QStringLiteral("deleted")}, {"type", QStringLiteral("file")}, {"sizeBytes", 920}, {"sizeText", QStringLiteral("1 KB")}}
        };
        m_diffSummary = QVariantMap{
            {"snapshotName", snapshotName},
            {"addedCount", 1},
            {"modifiedCount", 1},
            {"deletedCount", 1},
            {"totalCount", 3},
            {"truncated", false},
            {"error", QString()}
        };
        emit workspaceDiffChanged();
        return;
    }

    const QVariantMap env = environmentByName(name);
    const QString wsPath = env.value("workspacePath").toString().isEmpty()
                               ? workspacePathFor(name)
                               : env.value("workspacePath").toString();
    const QString snapDir = workspaceSnapshotDir(wsPath, name);
    const QString snapPath = snapDir + QLatin1Char('/') + snapshotName;

    if (snapDir.isEmpty() || !QFileInfo(snapPath).isDir()) {
        m_diffLoading = false;
        m_diffEntries.clear();
        m_diffSummary = QVariantMap{
            {"snapshotName", snapshotName},
            {"addedCount", 0},
            {"modifiedCount", 0},
            {"deletedCount", 0},
            {"totalCount", 0},
            {"truncated", false},
            {"error", tr("Workspace snapshot '%1' was not found.").arg(snapshotName)}
        };
        emit workspaceDiffChanged();
        return;
    }

    if (m_diffProcess && m_diffProcess->state() != QProcess::NotRunning) {
        m_diffTimeoutTimer.stop();
        m_diffProcess->disconnect(this);
        m_diffProcess->kill();
        m_diffProcess->waitForFinished(1000);
    }
    if (!m_diffProcess)
        m_diffProcess = new QProcess(this);
    else
        m_diffProcess->disconnect(this);

    m_diffLoading = true;
    m_diffEntries.clear();
    m_diffSummary = QVariantMap{
        {"snapshotName", snapshotName},
        {"addedCount", 0},
        {"modifiedCount", 0},
        {"deletedCount", 0},
        {"totalCount", 0},
        {"truncated", false},
        {"error", QString()}
    };
    emit workspaceDiffChanged();

    m_diffProcess->setProperty("timedOut", false);
    connect(m_diffProcess, &QProcess::finished, this, [this, snapshotName, wsPath](int code, QProcess::ExitStatus) {
        m_diffTimeoutTimer.stop();
        m_diffLoading = false;
        const bool timedOut = m_diffProcess->property("timedOut").toBool();
        if (timedOut) {
            m_diffEntries.clear();
            m_diffSummary = QVariantMap{
                {"snapshotName", snapshotName},
                {"addedCount", 0},
                {"modifiedCount", 0},
                {"deletedCount", 0},
                {"totalCount", 0},
                {"truncated", false},
                {"error", tr("Diff scan timed out after 30 seconds.")}
            };
            emit workspaceDiffChanged();
            return;
        }
        if (code != 0 && code != 24) {
            const QString errTxt = QString::fromUtf8(m_diffProcess->readAllStandardError()).trimmed();
            m_diffEntries.clear();
            m_diffSummary = QVariantMap{
                {"snapshotName", snapshotName},
                {"addedCount", 0},
                {"modifiedCount", 0},
                {"deletedCount", 0},
                {"totalCount", 0},
                {"truncated", false},
                {"error", errTxt.isEmpty() ? tr("Failed to compare workspace with snapshot.") : errTxt}
            };
            emit workspaceDiffChanged();
            return;
        }

        const QString out = QString::fromUtf8(m_diffProcess->readAllStandardOutput());
        QVariantList entries;
        const QVariantMap summary = processWorkspaceDiffOutput(snapshotName, wsPath, out, &entries);
        m_diffEntries = entries;
        m_diffSummary = summary;
        emit workspaceDiffChanged();
    });

    const QStringList args = buildWorkspaceDiffArgs(snapPath, wsPath, m_diffExcludes);

    m_diffTimeoutTimer.start();
    m_diffProcess->start(QStringLiteral("rsync"), args);
}

void Backend::loadFileDiff(const QString &name, const QString &snapshotName,
                           const QString &relativePath, const QString &status) {
    if (name.isEmpty() || snapshotName.isEmpty() || !isSafeRelativePath(relativePath)) {
        m_selectedDiffText.clear();
        m_selectedDiffInfo = tr("Invalid file path.");
        emit selectedDiffChanged();
        return;
    }

    if (m_demoMode) {
        if (status == QLatin1String("added")) {
            m_selectedDiffText = QStringLiteral("--- snapshot\n+++ current\n@@ -0,0 +1,4 @@\n+class AuthService {\n+  Future<bool> signIn() async => true;\n+}\n");
        } else if (status == QLatin1String("deleted")) {
            m_selectedDiffText = QStringLiteral("--- snapshot\n+++ current\n@@ -1,3 +0,0 @@\n-String legacyHelper() {\n-  return 'deprecated';\n-}\n");
        } else {
            m_selectedDiffText = QStringLiteral("--- snapshot\n+++ current\n@@ -10,4 +10,5 @@\n void main() {\n-  runApp(const OldApp());\n+  // Updated by AI agent\n+  runApp(const NewApp());\n }\n");
        }
        m_selectedDiffInfo = relativePath;
        emit selectedDiffChanged();
        return;
    }

    const QVariantMap env = environmentByName(name);
    const QString wsPath = env.value("workspacePath").toString().isEmpty()
                               ? workspacePathFor(name)
                               : env.value("workspacePath").toString();
    const QString snapFile = workspaceSnapshotDir(wsPath, name) + QLatin1Char('/') + snapshotName + QLatin1Char('/') + relativePath;
    const QString wsFile = QDir::cleanPath(wsPath) + QLatin1Char('/') + relativePath;

    const bool checkSnap = (status != QLatin1String("added"));
    const bool checkWs = (status != QLatin1String("deleted"));
    const QFileInfo snapInfo(snapFile);
    const QFileInfo wsInfo(wsFile);

    if ((checkSnap && snapInfo.isSymLink()) || (checkWs && wsInfo.isSymLink())) {
        m_selectedDiffText = QStringLiteral("Symlink target:\n  snapshot: %1\n  current:  %2")
                                 .arg(checkSnap && snapInfo.isSymLink() ? snapInfo.symLinkTarget() : QStringLiteral("—"),
                                      checkWs && wsInfo.isSymLink() ? wsInfo.symLinkTarget() : QStringLiteral("—"));
        m_selectedDiffInfo = relativePath;
        emit selectedDiffChanged();
        return;
    }
    if ((checkSnap && snapInfo.isDir()) || (checkWs && wsInfo.isDir())) {
        m_selectedDiffText.clear();
        m_selectedDiffInfo = tr("Directory entry — select a file to view its diff.");
        emit selectedDiffChanged();
        return;
    }

    constexpr qint64 kMaxPreviewBytes = 512 * 1024;
    if ((checkSnap && snapInfo.size() > kMaxPreviewBytes) ||
        (checkWs && wsInfo.size() > kMaxPreviewBytes)) {
        const qint64 maxSz = qMax(checkSnap ? snapInfo.size() : 0, checkWs ? wsInfo.size() : 0);
        m_selectedDiffText.clear();
        m_selectedDiffInfo = tr("File exceeds 512 KB preview limit (%1).").arg(formatBytes(maxSz));
        emit selectedDiffChanged();
        return;
    }

    auto readSample = [](const QString &path) -> QByteArray {
        QFile f(path);
        if (!f.open(QIODevice::ReadOnly))
            return {};
        return f.read(8192);
    };
    if ((checkSnap && isLikelyBinaryData(readSample(snapFile))) ||
        (checkWs && isLikelyBinaryData(readSample(wsFile)))) {
        m_selectedDiffText.clear();
        m_selectedDiffInfo = tr("Binary file — text diff unavailable.");
        emit selectedDiffChanged();
        return;
    }

    if (m_fileDiffProcess && m_fileDiffProcess->state() != QProcess::NotRunning) {
        m_fileDiffProcess->disconnect(this);
        m_fileDiffProcess->kill();
        m_fileDiffProcess->waitForFinished(500);
    }
    if (!m_fileDiffProcess)
        m_fileDiffProcess = new QProcess(this);
    else
        m_fileDiffProcess->disconnect(this);

    const QString leftArg = checkSnap ? snapFile : QStringLiteral("/dev/null");
    const QString rightArg = checkWs ? wsFile : QStringLiteral("/dev/null");

    connect(m_fileDiffProcess, &QProcess::finished, this, [this, relativePath](int code, QProcess::ExitStatus) {
        if (code != 0 && code != 1) {
            m_selectedDiffText.clear();
            m_selectedDiffInfo = tr("Could not generate diff for %1.").arg(relativePath);
            emit selectedDiffChanged();
            return;
        }
        bool truncated = false;
        const QString raw = QString::fromUtf8(m_fileDiffProcess->readAllStandardOutput());
        m_selectedDiffText = capDiffLines(raw, 2000, &truncated);
        m_selectedDiffInfo = truncated
                                 ? tr("%1 (diff capped at 2000 lines)").arg(relativePath)
                                 : relativePath;
        emit selectedDiffChanged();
    });

    m_fileDiffProcess->start(QStringLiteral("diff"),
                             {QStringLiteral("-u"),
                              QStringLiteral("--label"), QStringLiteral("snapshot"),
                              QStringLiteral("--label"), QStringLiteral("current"),
                              leftArg, rightArg});
}

QString Backend::restoreSingleFile(const QString &name, const QString &snapshotName,
                                   const QString &relativePath) {
    if (name.isEmpty() || snapshotName.isEmpty() || !isSafeRelativePath(relativePath))
        return tr("Invalid file restore request.");

    if (m_demoMode) {
        recordOperationHistory(tr("Restored file '%1' from snapshot '%2'").arg(relativePath, snapshotName),
                               name, QStringLiteral("success"), 0,
                               QDateTime::currentDateTimeUtc().toString(Qt::ISODate),
                               QByteArrayLiteral("Single-file restore (demo)\n"));
        m_statusMessage = tr("[Demo] Restored '%1' from '%2'.").arg(relativePath, snapshotName);
        emit statusMessageChanged();
        return {};
    }

    const QVariantMap env = environmentByName(name);
    const QString wsPath = env.value("workspacePath").toString().isEmpty()
                               ? workspacePathFor(name)
                               : env.value("workspacePath").toString();
    const QString snapFile = workspaceSnapshotDir(wsPath, name) + QLatin1Char('/') + snapshotName + QLatin1Char('/') + relativePath;
    const QString wsFile = QDir::cleanPath(wsPath) + QLatin1Char('/') + relativePath;

    const QFileInfo srcInfo(snapFile);
    if (!srcInfo.exists() && !srcInfo.isSymLink())
        return tr("File '%1' does not exist in snapshot '%2'.").arg(relativePath, snapshotName);
    if (srcInfo.isDir() && !srcInfo.isSymLink())
        return tr("Cannot single-restore a directory; use Snapshot Restore instead.");

    const QString parentDir = QFileInfo(wsFile).absolutePath();
    if (!QDir().mkpath(parentDir))
        return tr("Could not create parent directory: %1").arg(parentDir);

    if (QFileInfo::exists(wsFile) || QFileInfo(wsFile).isSymLink()) {
        if (!QFile::remove(wsFile))
            return tr("Could not overwrite workspace file: %1").arg(wsFile);
    }

    if (srcInfo.isSymLink()) {
        if (!QFile::link(srcInfo.symLinkTarget(), wsFile))
            return tr("Could not restore symlink: %1").arg(relativePath);
    } else {
        if (!QFile::copy(snapFile, wsFile))
            return tr("Could not copy file from snapshot: %1").arg(relativePath);
        QFile::setPermissions(wsFile, srcInfo.permissions());
    }

    const QString logMsg = QStringLiteral("Restored %1 -> %2\n").arg(snapFile, wsFile);
    recordOperationHistory(tr("Restored file '%1' from snapshot '%2'").arg(relativePath, snapshotName),
                           name, QStringLiteral("success"), 0,
                           QDateTime::currentDateTimeUtc().toString(Qt::ISODate),
                           logMsg.toUtf8());
    m_statusMessage = tr("Restored '%1' from snapshot '%2'.").arg(relativePath, snapshotName);
    emit statusMessageChanged();
    loadWorkspaceDiff(name, snapshotName);
    return {};
}

// --- S3: Secrets injection helpers ---

QString Backend::defaultSecretsStorePath() {
    const QString cfg = QStandardPaths::writableLocation(QStandardPaths::ConfigLocation);
    const QString base = cfg.trimmed().isEmpty() ? QDir::homePath() + QStringLiteral("/.config") : cfg.trimmed();
    return QDir::cleanPath(base + QStringLiteral("/aibox/secrets.json"));
}

bool Backend::isValidSecretName(const QString &name) {
    static const QRegularExpression nameRe(QStringLiteral("^[A-Z_][A-Z0-9_]*$"));
    return nameRe.match(name).hasMatch();
}

QStringList Backend::parseEnvSecretNames(const QString &rawCsv) {
    QStringList names;
    for (const QString &part : rawCsv.split(QLatin1Char(','), Qt::SkipEmptyParts)) {
        const QString clean = part.trimmed();
        if (isValidSecretName(clean) && !names.contains(clean))
            names.append(clean);
    }
    return names;
}

QList<QPair<QString, QString>> Backend::loadSecretsStore(const QString &filePath, QString *errorOut) {
    if (errorOut)
        errorOut->clear();
    if (filePath.trimmed().isEmpty())
        return {};

    struct stat st {};
    const QByteArray encoded = QFile::encodeName(filePath);
    if (::stat(encoded.constData(), &st) != 0) {
        if (errno == ENOENT)
            return {};
        if (errorOut)
            *errorOut = tr("Could not inspect secrets file permissions: %1").arg(filePath);
        return {};
    }

    if ((st.st_mode & (S_IRWXG | S_IRWXO)) != 0) {
        if (errorOut) {
            *errorOut = tr("Refusing to read %1 with insecure permissions (%2). Run: chmod 600 %1")
                            .arg(filePath, QString::number(static_cast<int>(st.st_mode & 0777), 8));
        }
        return {};
    }

    QFile file(filePath);
    if (!file.open(QIODevice::ReadOnly)) {
        if (errorOut)
            *errorOut = tr("Could not open secrets file: %1").arg(filePath);
        return {};
    }

    QJsonParseError err;
    const QJsonDocument doc = QJsonDocument::fromJson(file.readAll(), &err);
    if (err.error != QJsonParseError::NoError) {
        if (errorOut)
            *errorOut = tr("Invalid JSON in secrets file: %1").arg(err.errorString());
        return {};
    }

    QJsonArray arr;
    if (doc.isObject())
        arr = doc.object().value(QStringLiteral("secrets")).toArray();
    else if (doc.isArray())
        arr = doc.array();

    QList<QPair<QString, QString>> result;
    QSet<QString> seen;
    for (const QJsonValue &val : arr) {
        if (!val.isObject())
            continue;
        const QJsonObject obj = val.toObject();
        const QString name = obj.value(QStringLiteral("name")).toString();
        const QString secretVal = obj.value(QStringLiteral("value")).toString();
        if (!isValidSecretName(name) || seen.contains(name))
            continue;
        seen.insert(name);
        result.append({name, secretVal});
    }
    return result;
}

QString Backend::saveSecretsStore(const QString &filePath, const QList<QPair<QString, QString>> &secrets) {
    const QString cleanPath = QDir::cleanPath(filePath.trimmed());
    if (cleanPath.isEmpty())
        return tr("Invalid secrets store path.");

    QJsonArray arr;
    QSet<QString> seen;
    for (const auto &pair : secrets) {
        if (!isValidSecretName(pair.first))
            return tr("Invalid secret name '%1'. Use ^[A-Z_][A-Z0-9_]*$.").arg(pair.first);
        if (seen.contains(pair.first))
            continue;
        seen.insert(pair.first);
        QJsonObject obj;
        obj[QStringLiteral("name")] = pair.first;
        obj[QStringLiteral("value")] = pair.second;
        arr.append(obj);
    }

    const QString dirPath = QFileInfo(cleanPath).absolutePath();
    if (!QDir().mkpath(dirPath))
        return tr("Could not create secrets directory: %1").arg(dirPath);
    if (::chmod(QFile::encodeName(dirPath).constData(), 0700) != 0)
        return tr("Could not set 0700 permissions on directory: %1").arg(dirPath);

    QJsonObject root;
    root[QStringLiteral("secrets")] = arr;
    const QByteArray payload = QJsonDocument(root).toJson(QJsonDocument::Indented);

    const QString tmpPath = cleanPath + QStringLiteral(".tmp.%1")
                                            .arg(QUuid::createUuid().toString(QUuid::Id128));
    const QByteArray tmpEncoded = QFile::encodeName(tmpPath);
    const int fd = ::open(tmpEncoded.constData(), O_WRONLY | O_CREAT | O_EXCL, S_IRUSR | S_IWUSR);
    if (fd < 0)
        return tr("Could not create temporary secrets file: %1").arg(tmpPath);
    ::fchmod(fd, S_IRUSR | S_IWUSR);

    qint64 totalWritten = 0;
    while (totalWritten < payload.size()) {
        const ssize_t n = ::write(fd, payload.constData() + totalWritten,
                                  static_cast<size_t>(payload.size() - totalWritten));
        if (n < 0) {
            if (errno == EINTR)
                continue;
            ::close(fd);
            QFile::remove(tmpPath);
            return tr("Could not write secrets file: %1").arg(cleanPath);
        }
        totalWritten += n;
    }
    ::close(fd);

    const QByteArray dstEncoded = QFile::encodeName(cleanPath);
    if (::rename(tmpEncoded.constData(), dstEncoded.constData()) != 0) {
        QFile::remove(tmpPath);
        return tr("Could not replace secrets file: %1").arg(cleanPath);
    }
    if (::chmod(dstEncoded.constData(), 0600) != 0)
        return tr("Could not set 0600 permissions on secrets file: %1").arg(cleanPath);

    return {};
}

QString Backend::buildSecretEnvFileContent(const QList<QPair<QString, QString>> &secrets) {
    QString out;
    for (const auto &pair : secrets) {
        if (!isValidSecretName(pair.first))
            continue;
        out += pair.first + QLatin1Char('=') + shellQuote(pair.second) + QLatin1Char('\n');
    }
    return out;
}

QString Backend::redactSecrets(const QString &text, const QStringList &values) {
    if (text.isEmpty() || values.isEmpty())
        return text;

    QStringList sorted;
    sorted.reserve(values.size());
    for (const QString &val : values) {
        if (!val.isEmpty() && !sorted.contains(val))
            sorted.append(val);
    }
    std::sort(sorted.begin(), sorted.end(), [](const QString &a, const QString &b) {
        return a.size() > b.size();
    });

    QString result = text;
    for (const QString &secretVal : sorted)
        result.replace(secretVal, QStringLiteral("[REDACTED]"));
    return result;
}

QByteArray Backend::redactSecretsBytes(const QByteArray &data, const QStringList &values) {
    if (data.isEmpty() || values.isEmpty())
        return data;
    return redactSecrets(QString::fromUtf8(data), values).toUtf8();
}

QString Backend::createHostTempSecretFile(const QString &content, QString *errorOut) {
    if (errorOut)
        errorOut->clear();

    QString runtimeDir = qEnvironmentVariable("XDG_RUNTIME_DIR").trimmed();
    if (runtimeDir.isEmpty() || !QFileInfo(runtimeDir).isDir() || !QFileInfo(runtimeDir).isWritable())
        runtimeDir = QDir::tempPath();

    const QString path = QDir::cleanPath(
        runtimeDir + QStringLiteral("/aibox-%1.env").arg(QUuid::createUuid().toString(QUuid::Id128)));
    const QByteArray encoded = QFile::encodeName(path);
    const int fd = ::open(encoded.constData(), O_WRONLY | O_CREAT | O_EXCL, S_IRUSR | S_IWUSR);
    if (fd < 0) {
        if (errorOut)
            *errorOut = tr("Could not create temporary runtime secret file.");
        return {};
    }
    ::fchmod(fd, S_IRUSR | S_IWUSR);

    const QByteArray bytes = content.toUtf8();
    qint64 written = 0;
    while (written < bytes.size()) {
        const ssize_t n = ::write(fd, bytes.constData() + written, static_cast<size_t>(bytes.size() - written));
        if (n < 0) {
            if (errno == EINTR)
                continue;
            ::close(fd);
            QFile::remove(path);
            if (errorOut)
                *errorOut = tr("Could not write temporary runtime secret file.");
            return {};
        }
        written += n;
    }
    ::close(fd);
    return path;
}

void Backend::setSecretsStorePathForTest(const QString &path) {
    m_secretsPath = path.isEmpty() ? defaultSecretsStorePath() : QDir::cleanPath(path);
    reloadSecrets();
}

void Backend::reloadSecrets() {
    QString err;
    const auto loaded = loadSecretsStore(m_secretsPath, &err);
    m_secrets = loaded;
    m_secretsError = err;
    emit secretsChanged();
}

QVariantList Backend::secretsList() const {
    QVariantList list;
    for (const auto &pair : m_secrets) {
        const QString masked = pair.second.isEmpty() ? QStringLiteral("(empty)") : QStringLiteral("••••••••");
        list.append(QVariantMap{
            {QStringLiteral("name"), pair.first},
            {QStringLiteral("value"), pair.second},
            {QStringLiteral("maskedValue"), masked}
        });
    }
    return list;
}

QStringList Backend::allSecretValues() const {
    QStringList values;
    for (const auto &pair : m_secrets) {
        if (!pair.second.isEmpty())
            values.append(pair.second);
    }
    return values;
}

QString Backend::saveSecret(const QString &name, const QString &value) {
    const QString cleanName = name.trimmed();
    if (!isValidSecretName(cleanName))
        return tr("Secret name must match ^[A-Z_][A-Z0-9_]*$.");

    if (m_demoMode) {
        bool updated = false;
        for (int i = 0; i < m_secrets.size(); ++i) {
            if (m_secrets.at(i).first == cleanName) {
                m_secrets[i].second = value;
                updated = true;
                break;
            }
        }
        if (!updated)
            m_secrets.append({cleanName, value});
        m_secretsError.clear();
        emit secretsChanged();
        return {};
    }

    QString loadErr;
    QList<QPair<QString, QString>> next = loadSecretsStore(m_secretsPath, &loadErr);
    if (!loadErr.isEmpty()) {
        // Keep current in-memory list when recovering permissions on save.
        next = m_secrets;
    }

    bool updated = false;
    for (int i = 0; i < next.size(); ++i) {
        if (next.at(i).first == cleanName) {
            next[i].second = value;
            updated = true;
            break;
        }
    }
    if (!updated)
        next.append({cleanName, value});

    const QString saveErr = saveSecretsStore(m_secretsPath, next);
    if (!saveErr.isEmpty())
        return saveErr;

    m_secrets = next;
    m_secretsError.clear();
    emit secretsChanged();
    return {};
}

QString Backend::deleteSecret(const QString &name) {
    const QString cleanName = name.trimmed();
    QList<QPair<QString, QString>> next;
    for (const auto &pair : m_secrets) {
        if (pair.first != cleanName)
            next.append(pair);
    }

    if (m_demoMode) {
        m_secrets = next;
        m_secretsError.clear();
        emit secretsChanged();
        return {};
    }

    const QString saveErr = saveSecretsStore(m_secretsPath, next);
    if (!saveErr.isEmpty())
        return saveErr;

    m_secrets = next;
    m_secretsError.clear();
    emit secretsChanged();
    return {};
}

void Backend::setEnvironmentSecrets(const QString &name, const QStringList &secretNames) {    if (name.isEmpty())
        return;

    const QStringList cleanNames = parseEnvSecretNames(secretNames.join(QLatin1Char(',')));
    if (m_demoMode) {
        for (int i = 0; i < m_environments.size(); ++i) {
            QVariantMap env = m_environments.at(i).toMap();
            if (env.value("name").toString() == name) {
                env["secretNames"] = cleanNames;
                m_environments[i] = env;
                emit environmentsChanged();
                break;
            }
        }
        return;
    }

    if (m_busy)
        return;

    const QString qName = shellQuote(name);
    QString script;
    if (cleanNames.isEmpty()) {
        script = QStringLiteral(
            "set -euo pipefail\n"
            "echo '@@STAGE:50:Clearing environment secret bindings...'\n"
            "incus config unset %1 user.aibox.secrets >/dev/null 2>&1 || true\n"
            "echo '@@STAGE:100:Secrets updated.'").arg(qName);
    } else {
        script = QStringLiteral(
            "set -euo pipefail\n"
            "echo '@@STAGE:50:Updating environment secret bindings...'\n"
            "incus config set %1 user.aibox.secrets %2\n"
            "echo '@@STAGE:100:Secrets updated.'").arg(qName, shellQuote(cleanNames.join(QLatin1Char(','))));
    }
    runShellTask(tr("Updating secrets for '%1'").arg(name), script, {}, name);
}

QList<QPair<QString, QString>> Backend::resolveEnvironmentSecrets(const QString &envName, QString *errorOut) const {
    if (errorOut)
        errorOut->clear();
    const QVariantMap env = environmentByName(envName);
    const QStringList assigned = env.value("secretNames").toStringList();
    if (assigned.isEmpty())
        return {};

    QString loadErr;
    QList<QPair<QString, QString>> store;
    if (m_demoMode) {
        store = m_secrets;
    } else {
        store = loadSecretsStore(m_secretsPath, &loadErr);
        if (!loadErr.isEmpty()) {
            if (errorOut)
                *errorOut = loadErr;
            return {};
        }
    }

    QList<QPair<QString, QString>> resolved;
    for (const QString &reqName : assigned) {
        for (const auto &pair : store) {
            if (pair.first == reqName) {
                resolved.append(pair);
                break;
            }
        }
    }
    return resolved;
}

bool Backend::pushEnvironmentSecretsToContainer(const QString &envName, QString *outRemoteEnvPath,
                                                QString *errorOut) const {
    if (outRemoteEnvPath)
        outRemoteEnvPath->clear();
    if (errorOut)
        errorOut->clear();

    QString resolveErr;
    const QList<QPair<QString, QString>> pairs = resolveEnvironmentSecrets(envName, &resolveErr);
    if (!resolveErr.isEmpty()) {
        if (errorOut)
            *errorOut = resolveErr;
        return false;
    }
    if (pairs.isEmpty())
        return true;

    QString tmpErr;
    const QString hostFile = createHostTempSecretFile(buildSecretEnvFileContent(pairs), &tmpErr);
    if (hostFile.isEmpty()) {
        if (errorOut)
            *errorOut = tmpErr;
        return false;
    }

    const QString remotePath = QStringLiteral("/dev/shm/aibox-%1.env")
                                   .arg(QUuid::createUuid().toString(QUuid::Id128));
    QProcess pushProc;
    pushProc.start(QStringLiteral("incus"),
                   {QStringLiteral("file"), QStringLiteral("push"),
                    QStringLiteral("--mode"), QStringLiteral("0400"),
                    hostFile, envName + remotePath});
    const bool finished = pushProc.waitForFinished(15000);
    QFile::remove(hostFile);

    if (!finished || pushProc.exitStatus() != QProcess::NormalExit || pushProc.exitCode() != 0) {
        if (errorOut) {
            const QString errTxt = QString::fromUtf8(pushProc.readAllStandardError()).trimmed();
            *errorOut = errTxt.isEmpty() ? tr("Failed to inject secrets into container.") : errTxt;
        }
        return false;
    }

    if (outRemoteEnvPath)
        *outRemoteEnvPath = remotePath;
    return true;
}

static qint64 parseStorageMetricBytes(const QString &rawVal) {
    QString clean = rawVal.trimmed();
    if ((clean.startsWith(QLatin1Char('"')) && clean.endsWith(QLatin1Char('"'))) ||
        (clean.startsWith(QLatin1Char('\'')) && clean.endsWith(QLatin1Char('\'')))) {
        clean = clean.mid(1, clean.size() - 2).trimmed();
    }
    if (clean.isEmpty())
        return 0;
    bool intOk = false;
    const qint64 exactBytes = clean.toLongLong(&intOk);
    if (intOk)
        return qMax<qint64>(0, exactBytes);
    return parseSizeBytes(clean);
}

Backend::StoragePoolInfo Backend::parseIncusStorageInfo(const QString &output) {
    StoragePoolInfo info;
    const QString trimmed = output.trimmed();
    if (trimmed.isEmpty())
        return info;

    if (trimmed.startsWith(QLatin1Char('{'))) {
        const QJsonDocument doc = QJsonDocument::fromJson(trimmed.toUtf8());
        if (doc.isObject()) {
            QJsonObject obj = doc.object();
            if (obj.contains(QStringLiteral("info")) && obj.value(QStringLiteral("info")).isObject())
                obj = obj.value(QStringLiteral("info")).toObject();
            if (!obj.value(QStringLiteral("name")).toString().isEmpty())
                info.name = obj.value(QStringLiteral("name")).toString();
            info.driver = obj.value(QStringLiteral("driver")).toString();
            const auto usedVal = obj.contains(QStringLiteral("space used"))
                                     ? obj.value(QStringLiteral("space used"))
                                     : obj.value(QStringLiteral("used"));
            const auto totalVal = obj.contains(QStringLiteral("total space"))
                                      ? obj.value(QStringLiteral("total space"))
                                      : obj.value(QStringLiteral("total"));
            info.usedBytes = usedVal.isString() ? parseStorageMetricBytes(usedVal.toString())
                                                : qMax<qint64>(0, usedVal.toVariant().toLongLong());
            info.totalBytes = totalVal.isString() ? parseStorageMetricBytes(totalVal.toString())
                                                  : qMax<qint64>(0, totalVal.toVariant().toLongLong());
            if (info.totalBytes > 0) {
                info.usedPercent = qBound(0,
                                          static_cast<int>(std::llround((static_cast<double>(info.usedBytes) * 100.0) /
                                                                        static_cast<double>(info.totalBytes))),
                                          100);
                info.valid = true;
            }
            return info;
        }
    }

    bool inUsedBy = false;
    for (const QString &rawLine : trimmed.split(QLatin1Char('\n'))) {
        if (!rawLine.startsWith(QLatin1Char(' ')) && !rawLine.startsWith(QLatin1Char('\t'))) {
            const QString topKey = rawLine.trimmed();
            if (topKey.startsWith(QLatin1String("used by:"))) {
                inUsedBy = true;
                continue;
            }
            if (topKey.startsWith(QLatin1String("info:"))) {
                inUsedBy = false;
                continue;
            }
        }
        if (inUsedBy)
            continue;
        const int colon = rawLine.indexOf(QLatin1Char(':'));
        if (colon <= 0)
            continue;
        const QString key = rawLine.left(colon).trimmed().toLower();
        const QString val = rawLine.mid(colon + 1).trimmed();
        if (key == QLatin1String("name") && !val.isEmpty())
            info.name = val;
        else if (key == QLatin1String("driver"))
            info.driver = val;
        else if (key == QLatin1String("space used"))
            info.usedBytes = parseStorageMetricBytes(val);
        else if (key == QLatin1String("total space"))
            info.totalBytes = parseStorageMetricBytes(val);
    }

    if (info.totalBytes > 0) {
        info.usedPercent = qBound(0,
                                  static_cast<int>(std::llround((static_cast<double>(info.usedBytes) * 100.0) /
                                                                static_cast<double>(info.totalBytes))),
                                  100);
        info.valid = true;
    }
    return info;
}

qint64 Backend::sumRootDiskQuotasBytes(const QByteArray &incusListJson,
                                       const QString &excludeEnvName,
                                       const QString &poolName,
                                       int *unboundedCountOut) {
    if (unboundedCountOut)
        *unboundedCountOut = 0;

    const QJsonDocument doc = QJsonDocument::fromJson(incusListJson);
    if (!doc.isArray())
        return 0;

    qint64 total = 0;
    for (const auto &val : doc.array()) {
        const QJsonObject obj = val.toObject();
        const QString name = obj.value(QStringLiteral("name")).toString();
        if (name.isEmpty() || (!excludeEnvName.isEmpty() && name == excludeEnvName))
            continue;

        const QJsonObject expanded = obj.value(QStringLiteral("expanded_devices")).toObject();
        const QJsonObject direct = obj.value(QStringLiteral("devices")).toObject();
        QJsonObject rootDev = expanded.value(QStringLiteral("root")).toObject();
        if (rootDev.isEmpty())
            rootDev = direct.value(QStringLiteral("root")).toObject();

        const QString rootPool = rootDev.value(QStringLiteral("pool")).toString();
        if (!poolName.isEmpty() && !rootPool.isEmpty() && rootPool != poolName)
            continue;

        QString sizeStr = rootDev.value(QStringLiteral("size")).toString().trimmed();
        if (sizeStr.isEmpty())
            sizeStr = direct.value(QStringLiteral("root")).toObject().value(QStringLiteral("size")).toString().trimmed();

        const qint64 quota = parseSizeBytes(sizeStr);
        if (quota > 0) {
            total += quota;
        } else if (unboundedCountOut) {
            ++(*unboundedCountOut);
        }
    }
    return total;
}

QString Backend::evaluatePoolOvercommitWarning(const StoragePoolInfo &pool,
                                             qint64 existingQuotasBytes,
                                             qint64 requestedBytes,
                                             int unboundedCount) {
    if (!pool.valid || pool.totalBytes <= 0)
        return {};

    const qint64 req = qMax<qint64>(0, requestedBytes);
    const qint64 projected = existingQuotasBytes + req;
    const bool overcommit = (projected > pool.totalBytes);
    const bool highPool = (pool.usedPercent > 85);
    if (!overcommit && !highPool)
        return {};

    QStringList parts;
    if (highPool) {
        parts << QObject::tr("Pool '%1' is %2% full (%3 / %4).")
                     .arg(pool.name)
                     .arg(pool.usedPercent)
                     .arg(formatBytes(pool.usedBytes), formatBytes(pool.totalBytes));
    }
    if (overcommit) {
        parts << QObject::tr("Total allocated quotas (%1) will exceed pool capacity (%2).")
                     .arg(formatBytes(projected), formatBytes(pool.totalBytes));
    }
    if (unboundedCount > 0) {
        parts << QObject::tr("%1 environment(s) have no disk quota.")
                     .arg(unboundedCount);
    }
    return parts.join(QLatin1Char(' '));
}

Backend::HealthCheckResult Backend::evaluatePoolCapacityHealth(const StoragePoolInfo &pool,
                                                               qint64 allocatedQuotasBytes,
                                                               int unboundedCount) {
    HealthCheckResult r;
    r.id = QStringLiteral("pool_capacity");
    r.title = QStringLiteral("Storage pool '%1' capacity").arg(pool.name);
    if (!pool.valid || pool.totalBytes <= 0) {
        r.ok = true;
        r.warning = false;
        r.detail = QStringLiteral("Could not query storage pool '%1' capacity.").arg(pool.name);
        return r;
    }

    QString nums = QStringLiteral("%1 / %2 (%3%), %4 allocated across quotas")
                       .arg(formatBytes(pool.usedBytes), formatBytes(pool.totalBytes))
                       .arg(pool.usedPercent)
                       .arg(formatBytes(allocatedQuotasBytes));
    if (unboundedCount > 0)
        nums += QStringLiteral(", %1 without disk quota").arg(unboundedCount);
    if (allocatedQuotasBytes > pool.totalBytes)
        nums += QStringLiteral(" [OVERCOMMITTED]");

    if (pool.usedPercent > 95) {
        r.ok = false;
        r.warning = false;
        r.needsRoot = false;
        r.detail = QStringLiteral("Critical pool usage (>95%): %1.").arg(nums);
        r.fixCommand = QStringLiteral("Wipe unused environments or delete old snapshots on pool '%1'.").arg(pool.name);
    } else if (pool.usedPercent > 85) {
        r.ok = true;
        r.warning = true;
        r.needsRoot = false;
        r.detail = QStringLiteral("High pool usage warning (>85%): %1.").arg(nums);
    } else {
        r.ok = true;
        r.warning = false;
        r.detail = QStringLiteral("Healthy pool usage: %1.").arg(nums);
    }
    return r;
}

QString Backend::poolWarningForCreate(const QString &diskSpec) const {
    if (!m_lastPoolInfo.valid)
        return {};
    const qint64 reqBytes = parseSizeBytes(diskSpec);
    return evaluatePoolOvercommitWarning(m_lastPoolInfo, m_lastPoolAllocatedBytes, reqBytes, m_lastPoolUnboundedCount);
}

QString Backend::poolWarningForQuotaEdit(const QString &envName, const QString &diskSpec) const {
    if (!m_lastPoolInfo.valid)
        return {};
    const qint64 newBytes = parseSizeBytes(diskSpec);
    if (!m_lastListJson.isEmpty()) {
        int unbounded = 0;
        const qint64 otherAllocated = sumRootDiskQuotasBytes(m_lastListJson, envName, m_lastPoolInfo.name, &unbounded);
        return evaluatePoolOvercommitWarning(m_lastPoolInfo, otherAllocated, newBytes, unbounded);
    }
    qint64 otherAllocated = 0;
    int unbounded = 0;
    for (const auto &v : m_environments) {
        const QVariantMap env = v.toMap();
        if (env.value(QStringLiteral("name")).toString() == envName)
            continue;
        const qint64 q = env.value(QStringLiteral("diskQuotaBytes")).toLongLong();
        if (q > 0)
            otherAllocated += q;
        else
            ++unbounded;
    }
    return evaluatePoolOvercommitWarning(m_lastPoolInfo, otherAllocated, newBytes, unbounded);
}

QString Backend::resizeStoragePoolScript(const QString &poolName, const QString &sizeSpec) {
    const QString qPool = shellQuote(poolName);
    const QString qSize = shellQuote(sizeSpec.trimmed());
    return QStringLiteral("set -euo pipefail\n"
                          "echo '@@STAGE:20:Growing storage pool %1 to %2...'\n"
                          "incus storage set %1 size=%2\n"
                          "echo '@@STAGE:80:Verifying new pool capacity...'\n"
                          "incus storage info %1 --bytes 2>/dev/null || incus storage info %1\n"
                          "echo '@@STAGE:100:Storage pool grown to %2.'")
        .arg(qPool, qSize);
}

bool Backend::parsePoolSizeSpec(const QString &spec, qint64 *outBytes) {
    if (outBytes)
        *outBytes = 0;
    static const QRegularExpression re(
        QStringLiteral("^\\s*(\\d+(?:[\\.,]\\d+)?)\\s*(TiB|TB|GiB|GB|MiB|MB)?\\s*$"),
        QRegularExpression::CaseInsensitiveOption);
    const QRegularExpressionMatch match = re.match(spec);
    if (!match.hasMatch())
        return false;

    QString number = match.captured(1);
    number.replace(QLatin1Char(','), QLatin1Char('.'));
    bool ok = false;
    const double value = number.toDouble(&ok);
    if (!ok || value <= 0.0)
        return false;

    // Incus treats bare storage sizes as GiB, so "60" means 60GiB here too.
    const QString unit = match.captured(2).toUpper();
    double scale = 1024.0 * 1024.0 * 1024.0;
    if (unit == QLatin1String("TIB") || unit == QLatin1String("TB"))
        scale = 1024.0 * 1024.0 * 1024.0 * 1024.0;
    else if (unit == QLatin1String("MIB") || unit == QLatin1String("MB"))
        scale = 1024.0 * 1024.0;

    const double bytes = value * scale;
    if (bytes < 1.0 || bytes >= 9.0e18)
        return false;
    if (outBytes)
        *outBytes = static_cast<qint64>(std::llround(bytes));
    return true;
}

QString Backend::validateStoragePoolResize(const QString &sizeSpec) const {
    if (m_busy)
        return tr("Wait for the current AIBox task to finish before growing the storage pool.");
    if (isTaskLockActive())
        return tr("Another AIBox task is currently running.");
    if (!m_lastPoolInfo.valid || m_lastPoolInfo.totalBytes <= 0)
        return tr("The Incus storage pool is not available yet. Refresh and try again.");
    if (!m_lastPoolInfo.resizable)
        return tr("Pool '%1' is not loop-file backed, so its capacity follows the host filesystem and cannot be grown from AIBox.")
            .arg(m_lastPoolInfo.name);

    qint64 targetBytes = 0;
    if (!parsePoolSizeSpec(sizeSpec, &targetBytes))
        return tr("Enter a pool size such as 60GiB (plain numbers are treated as GiB).");

    if (targetBytes <= m_lastPoolInfo.totalBytes)
        return tr("The new size (%1) must be larger than the current pool size (%2); Incus cannot shrink a pool.")
            .arg(formatBytes(targetBytes), formatBytes(m_lastPoolInfo.totalBytes));

    const qint64 delta = targetBytes - m_lastPoolInfo.totalBytes;
    const qint64 available = availableDiskBytes(QStringLiteral("/var/lib/incus"));
    if (available > 0 && delta > available) {
        return tr("Growing the pool needs %1 more space, but only %2 is free on the host filesystem.")
            .arg(formatBytes(delta), formatBytes(available));
    }
    return {};
}

QString Backend::resizeStoragePool(const QString &sizeSpec) {
    const QString error = validateStoragePoolResize(sizeSpec);
    if (!error.isEmpty())
        return error;

    const QString cleanSize = sizeSpec.trimmed();
    runShellTask(tr("Growing storage pool '%1' to %2").arg(m_lastPoolInfo.name, cleanSize),
                 resizeStoragePoolScript(m_lastPoolInfo.name, cleanSize));
    return {};
}

void Backend::setPoolStateForTest(const StoragePoolInfo &pool, const QByteArray &incusListJson) {
    m_lastPoolInfo = pool;
    m_lastListJson = incusListJson;
    m_lastPoolUnboundedCount = 0;
    m_lastPoolAllocatedBytes = sumRootDiskQuotasBytes(incusListJson, {}, pool.name, &m_lastPoolUnboundedCount);
    const QVariantMap nextMap = poolInfoToVariantMap(pool, m_lastPoolAllocatedBytes, m_lastPoolUnboundedCount);
    if (nextMap != m_poolInfo) {
        m_poolInfo = nextMap;
        emit poolInfoChanged();
    }
}

Backend::IdleStopOverride Backend::parseIdleStopOverride(const QString &raw) {
    IdleStopOverride ov;
    const QString norm = raw.trimmed().toLower();
    if (norm.isEmpty() || norm == QLatin1String("inherit") || norm == QLatin1String("default")) {
        ov.mode = IdleStopOverride::Inherit;
        ov.minutes = 0;
        return ov;
    }
    if (norm == QLatin1String("off") || norm == QLatin1String("false") || norm == QLatin1String("0")) {
        ov.mode = IdleStopOverride::Off;
        ov.minutes = 0;
        return ov;
    }
    bool ok = false;
    const int mins = norm.toInt(&ok);
    if (ok && mins > 0 && mins <= 1440) {
        ov.mode = IdleStopOverride::Minutes;
        ov.minutes = mins;
        return ov;
    }
    ov.mode = IdleStopOverride::Inherit;
    ov.minutes = 0;
    return ov;
}

int Backend::effectiveIdleStopMinutes(int globalMinutes, const IdleStopOverride &ov) {
    if (ov.mode == IdleStopOverride::Off)
        return 0;
    if (ov.mode == IdleStopOverride::Minutes)
        return qBound(1, ov.minutes, 1440);
    return (globalMinutes > 0) ? qBound(1, globalMinutes, 1440) : 0;
}

Backend::IdleDecisionResult Backend::evaluateIdleDecision(const IdleDecisionInput &in) {
    IdleDecisionResult res;
    if (!in.running) {
        res.blockedReason = QStringLiteral("stopped");
        return res;
    }
    if (in.isTemplate) {
        res.blockedReason = QStringLiteral("template");
        return res;
    }
    if (in.thresholdMinutes <= 0) {
        res.blockedReason = QStringLiteral("disabled");
        return res;
    }
    if (in.busyOnThisEnv) {
        res.blockedReason = QStringLiteral("busy");
        return res;
    }
    if (in.hasActiveExecSession) {
        res.blockedReason = QStringLiteral("exec-session");
        return res;
    }
    if (in.portForwardCount > 0 && !in.ignorePortForwards) {
        res.blockedReason = QStringLiteral("port-forwards");
        return res;
    }

    res.eligible = true;
    if (in.samples.isEmpty())
        return res;

    // CPU spikes (>= 3%) reset the idle timer; inspect contiguous calm samples since last spike
    int startIdx = 0;
    for (int i = in.samples.size() - 1; i >= 0; --i) {
        if (in.samples.at(i).cpuPercent >= 3) {
            startIdx = i + 1;
            break;
        }
    }
    if (startIdx >= in.samples.size()) {
        res.blockedReason = QStringLiteral("cpu-spike");
        return res;
    }

    double sumCpu = 0.0;
    const int calmCount = in.samples.size() - startIdx;
    for (int i = startIdx; i < in.samples.size(); ++i)
        sumCpu += static_cast<double>(qMax(0, in.samples.at(i).cpuPercent));
    res.averageCpuPercent = sumCpu / static_cast<double>(calmCount);

    const qint64 firstCalmMs = in.samples.at(startIdx).timestampMs;
    const qint64 refNowMs = (in.nowMs > 0) ? in.nowMs : in.samples.last().timestampMs;
    res.idleDurationMs = qMax<qint64>(0, refNowMs - firstCalmMs);

    if (res.averageCpuPercent >= 3.0) {
        res.idleDurationMs = 0;
        res.blockedReason = QStringLiteral("cpu-high");
        return res;
    }

    const qint64 windowMs = qint64(in.thresholdMinutes) * 60LL * 1000LL;
    const qint64 warnLeadMs = (in.thresholdMinutes >= 2) ? 60000LL : (windowMs / 2);
    if (res.idleDurationMs >= windowMs) {
        res.shouldStopNow = true;
    } else if (!in.warningAlreadySent && res.idleDurationMs >= (windowMs - warnLeadMs)) {
        res.shouldWarnOneMin = true;
    }
    return res;
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

QString Backend::sessionsDirPath() const {
    return m_stateDir + QStringLiteral("/sessions");
}

bool Backend::prepareRecordedSessionPaths(const QString &envName, QString *outLogFile, QString *outTimingFile) const {
    const QString cleanEnv = sanitizeName(envName);
    if (cleanEnv.isEmpty())
        return false;
    const QString envDir = sessionsDirPath() + QLatin1Char('/') + cleanEnv;
    if (!QDir().mkpath(envDir))
        return false;
    QFile::setPermissions(sessionsDirPath(),
                          QFileDevice::ReadOwner | QFileDevice::WriteOwner | QFileDevice::ExeOwner);
    QFile::setPermissions(envDir,
                          QFileDevice::ReadOwner | QFileDevice::WriteOwner | QFileDevice::ExeOwner);

    qint64 epoch = QDateTime::currentSecsSinceEpoch();
    QString base = QString::number(epoch);
    while (QFileInfo::exists(envDir + QLatin1Char('/') + base + QStringLiteral(".log"))) {
        ++epoch;
        base = QString::number(epoch);
    }
    if (outLogFile)
        *outLogFile = envDir + QLatin1Char('/') + base + QStringLiteral(".log");
    if (outTimingFile)
        *outTimingFile = envDir + QLatin1Char('/') + base + QStringLiteral(".timing");
    return true;
}

bool Backend::effectiveRecordSessions(bool globalEnabled, const QString &envOverride) {
    const QString ov = envOverride.trimmed().toLower();
    if (ov == QLatin1String("true") || ov == QLatin1String("1"))
        return true;
    if (ov == QLatin1String("false") || ov == QLatin1String("0"))
        return false;
    return globalEnabled;
}

void Backend::setEnvironmentRecordSessions(const QString &name, const QString &mode) {
    if (name.isEmpty() || m_busy)
        return;
    const QString norm = mode.trimmed().toLower();
    const QString stored = (norm == QLatin1String("true"))
                               ? QStringLiteral("true")
                               : (norm == QLatin1String("false") ? QStringLiteral("false") : QStringLiteral("inherit"));
    runShellTask(tr("Updating session recording for '%1'").arg(name),
                 QStringLiteral("echo '@@STAGE:50:Updating session recording policy...'; "
                                "incus config set %1 user.aibox.record %2; "
                                "echo '@@STAGE:100:Updated.'")
                     .arg(shellQuote(name), shellQuote(stored)),
                 {}, name);
}

QString Backend::buildSessionWrapCommand(const QString &innerCommand, const QString &logFile,
                                         const QString &timingFile,
                                         const QString &redactBinPath,
                                         const QString &sessionsDir,
                                         bool scriptAvailable) {
    if (!scriptAvailable || innerCommand.trimmed().isEmpty() ||
        logFile.trimmed().isEmpty() || timingFile.trimmed().isEmpty()) {
        return innerCommand;
    }

    const QString logDir = QFileInfo(logFile).absolutePath();
    const QString qLogDir = shellQuote(logDir);
    const QString qLog = shellQuote(logFile);
    const QString qTiming = shellQuote(timingFile);
    const QString qInner = shellQuote(innerCommand);

    QString postCmd;
    if (!redactBinPath.trimmed().isEmpty()) {
        postCmd = QStringLiteral("    %1 --redact-session %2%3 >/dev/null 2>&1 || true\n")
                      .arg(shellQuote(redactBinPath),
                           qLog,
                           sessionsDir.trimmed().isEmpty()
                               ? QString()
                               : QStringLiteral(" ") + shellQuote(sessionsDir));
    }

    return QStringLiteral(
        "if command -v script >/dev/null 2>&1; then\n"
        "    mkdir -p %1 && chmod 700 %1 2>/dev/null || true\n"
        "    touch %2 %3 && chmod 600 %2 %3 2>/dev/null || true\n"
        "    script -q -f --log-out %2 --log-timing %3 -c %4\n"
        "    _aibox_rc=$?\n"
        "%5"
        "    exit $_aibox_rc\n"
        "else\n"
        "    %6\n"
        "fi")
        .arg(qLogDir, qLog, qTiming, qInner, postCmd, innerCommand);
}

QString Backend::stripAnsiSequences(const QString &rawText) {
    QString out = rawText;
    out.replace(QStringLiteral("\r\n"), QStringLiteral("\n"));
    out.remove(QLatin1Char('\r'));
    // OSC sequences: ESC ] ... (BEL or ST)
    static const QRegularExpression oscRe(QStringLiteral("\\x1b\\][^\\x07\\x1b]*(?:\\x07|\\x1b\\\\)"));
    out.remove(oscRe);
    // CSI sequences: ESC [ params intermediate final
    static const QRegularExpression csiRe(QStringLiteral("\\x1b\\[[0-9;?]*[ -/]*[@-~]"));
    out.remove(csiRe);
    // Character-set / 2-byte ESC sequences
    static const QRegularExpression esc2Re(QStringLiteral("\\x1b[()#%*+\\-/][0-9A-Za-z]?"));
    out.remove(esc2Re);
    static const QRegularExpression escShortRe(QStringLiteral("\\x1b[@-Z\\\\-_]"));
    out.remove(escShortRe);
    out.remove(QLatin1Char('\x07'));
    return out;
}

QString Backend::formatRecordedSessionText(const QByteArray &rawBytes, int maxBytes) {
    if (rawBytes.isEmpty())
        return {};
    const int cap = qMax(1024, maxBytes);
    bool truncated = false;
    QByteArray slice = rawBytes;
    const int rawBudget = cap * 2;
    if (slice.size() > rawBudget) {
        slice = slice.right(rawBudget);
        truncated = true;
    }
    QString clean = stripAnsiSequences(QString::fromUtf8(slice));
    QByteArray utf8 = clean.toUtf8();
    if (utf8.size() > cap) {
        truncated = true;
        utf8 = utf8.right(cap);
        const int nl = utf8.indexOf('\n');
        if (nl >= 0 && nl < 256)
            utf8 = utf8.mid(nl + 1);
        clean = QString::fromUtf8(utf8);
    }
    if (truncated)
        clean.prepend(QStringLiteral("[... truncated to last 200 KB ...]\n"));
    return clean;
}

bool Backend::redactSessionLogFile(const QString &logFilePath, const QStringList &secretValues) {
    if (logFilePath.trimmed().isEmpty() || secretValues.isEmpty())
        return false;
    QFile in(logFilePath);
    if (!in.exists() || !in.open(QIODevice::ReadOnly))
        return false;
    const QByteArray raw = in.readAll();
    in.close();
    const QByteArray scrubbed = redactSecretsBytes(raw, secretValues);
    if (scrubbed == raw)
        return false;
    QSaveFile out(logFilePath);
    if (!out.open(QIODevice::WriteOnly))
        return false;
    out.write(scrubbed);
    if (!out.commit())
        return false;
    QFile::setPermissions(logFilePath, QFileDevice::ReadOwner | QFileDevice::WriteOwner);
    return true;
}

namespace {
struct SessionFilePair {
    QString logPath;
    QString timingPath;
    qint64 mtimeMs = 0;
    qint64 totalBytes = 0;
};

void removeSessionPairFiles(const SessionFilePair &pair) {
    if (!pair.logPath.isEmpty())
        QFile::remove(pair.logPath);
    if (!pair.timingPath.isEmpty())
        QFile::remove(pair.timingPath);
}
} // namespace

int Backend::pruneRecordedSessions(const QString &sessionsDir, int maxPerEnv, qint64 maxTotalBytes) {
    const QDir root(sessionsDir);
    if (!root.exists())
        return 0;

    int deleted = 0;
    QList<SessionFilePair> surviving;
    const QFileInfoList envDirs = root.entryInfoList(QDir::Dirs | QDir::NoDotAndDotDot, QDir::Name);
    for (const QFileInfo &envDirInfo : envDirs) {
        const QDir envDir(envDirInfo.absoluteFilePath());
        const QFileInfoList logs = envDir.entryInfoList({QStringLiteral("*.log")}, QDir::Files, QDir::Name);
        QList<SessionFilePair> pairs;
        pairs.reserve(logs.size());
        for (const QFileInfo &logFi : logs) {
            const QString base = logFi.completeBaseName();
            const QString timingPath = envDir.absoluteFilePath(base + QStringLiteral(".timing"));
            const QFileInfo timingFi(timingPath);
            SessionFilePair pair;
            pair.logPath = logFi.absoluteFilePath();
            pair.timingPath = timingFi.exists() ? timingPath : QString();
            pair.mtimeMs = logFi.lastModified().toMSecsSinceEpoch();
            pair.totalBytes = logFi.size() + (timingFi.exists() ? timingFi.size() : 0);
            pairs.append(pair);
        }

        // Newest first for per-env cap
        std::sort(pairs.begin(), pairs.end(), [](const SessionFilePair &a, const SessionFilePair &b) {
            if (a.mtimeMs != b.mtimeMs)
                return a.mtimeMs > b.mtimeMs;
            return a.logPath > b.logPath;
        });

        for (int i = 0; i < pairs.size(); ++i) {
            if (maxPerEnv >= 0 && i >= maxPerEnv) {
                removeSessionPairFiles(pairs.at(i));
                ++deleted;
            } else {
                surviving.append(pairs.at(i));
            }
        }
    }

    if (maxTotalBytes > 0) {
        qint64 totalBytes = 0;
        for (const auto &p : surviving)
            totalBytes += p.totalBytes;

        if (totalBytes > maxTotalBytes) {
            // Oldest first for global size cap
            std::sort(surviving.begin(), surviving.end(), [](const SessionFilePair &a, const SessionFilePair &b) {
                if (a.mtimeMs != b.mtimeMs)
                    return a.mtimeMs < b.mtimeMs;
                return a.logPath < b.logPath;
            });
            for (const auto &p : surviving) {
                if (totalBytes <= maxTotalBytes)
                    break;
                removeSessionPairFiles(p);
                totalBytes -= p.totalBytes;
                ++deleted;
            }
        }
    }

    return deleted;
}

QVariantList Backend::scanRecordedSessions(const QString &sessionsDir) {
    QVariantList result;
    const QDir root(sessionsDir);
    if (!root.exists())
        return result;

    const QFileInfoList envDirs = root.entryInfoList(QDir::Dirs | QDir::NoDotAndDotDot, QDir::Name);
    for (const QFileInfo &envDirInfo : envDirs) {
        const QString envName = envDirInfo.fileName();
        const QDir envDir(envDirInfo.absoluteFilePath());
        const QFileInfoList logs = envDir.entryInfoList({QStringLiteral("*.log")}, QDir::Files);
        for (const QFileInfo &logFi : logs) {
            const QString base = logFi.completeBaseName();
            const QString timingPath = envDir.absoluteFilePath(base + QStringLiteral(".timing"));
            const QFileInfo timingFi(timingPath);
            const bool hasTiming = timingFi.exists() && timingFi.size() > 0;
            bool ok = false;
            const qint64 epoch = base.toLongLong(&ok);
            const QDateTime dt = (ok && epoch > 0)
                                     ? QDateTime::fromSecsSinceEpoch(epoch)
                                     : logFi.lastModified();
            const qint64 totalBytes = logFi.size() + (timingFi.exists() ? timingFi.size() : 0);

            QVariantMap item;
            item["id"] = envName + QLatin1Char('/') + base;
            item["envName"] = envName;
            item["epoch"] = dt.toSecsSinceEpoch();
            item["timestamp"] = dt.toString(QStringLiteral("yyyy-MM-dd HH:mm:ss"));
            item["logPath"] = logFi.absoluteFilePath();
            item["timingPath"] = hasTiming ? timingPath : QString();
            item["hasTiming"] = hasTiming;
            item["sizeBytes"] = totalBytes;
            item["sizeDisplay"] = formatBytes(totalBytes);
            result.append(item);
        }
    }

    std::sort(result.begin(), result.end(), [](const QVariant &a, const QVariant &b) {
        return a.toMap().value(QStringLiteral("epoch")).toLongLong() >
               b.toMap().value(QStringLiteral("epoch")).toLongLong();
    });
    return result;
}

void Backend::refreshRecordedSessions() {
    if (m_demoMode) {
        emit recordedSessionsChanged();
        return;
    }

    const QString sDir = sessionsDirPath();
    const QStringList secretVals = allSecretValues();
    if (!secretVals.isEmpty() && QDir(sDir).exists()) {
        const QVariantList preScan = scanRecordedSessions(sDir);
        for (const QVariant &v : preScan)
            redactSessionLogFile(v.toMap().value(QStringLiteral("logPath")).toString(), secretVals);
    }

    pruneRecordedSessions(sDir);
    const QVariantList scanned = scanRecordedSessions(sDir);
    if (scanned != m_recordedSessions) {
        m_recordedSessions = scanned;
        emit recordedSessionsChanged();
    }
}

QString Backend::readRecordedSessionLog(const QString &logPath) const {
    if (logPath.trimmed().isEmpty())
        return {};
    if (m_demoMode && logPath.startsWith(QLatin1String("demo://"))) {
        return formatRecordedSessionText(QByteArrayLiteral(
            "Script started on 2026-11-13 17:10:00+07:00 [COMMAND=\"incus exec desktop-studio --cwd /home/workspace -- bash -l\"]\n"
            "\x1b[32muser@desktop-studio:/home/workspace$\x1b[0m ls -la\n"
            "total 12\n"
            "drwxrwx--- 1 root user 120 Nov 13 17:09 .\n"
            "\x1b[32muser@desktop-studio:/home/workspace$\x1b[0m echo $ANTHROPIC_API_KEY\n"
            "[REDACTED]\n"
            "\x1b[32muser@desktop-studio:/home/workspace$\x1b[0m exit\n"
            "Script done on 2026-11-13 17:12:14+07:00 [COMMAND_EXIT_CODE=\"0\"]\n"));
    }

    QFile f(logPath);
    if (!f.open(QIODevice::ReadOnly))
        return tr("Session log is not available.");
    return formatRecordedSessionText(f.readAll());
}

void Backend::replayRecordedSession(const QString &logPath, const QString &timingPath) {
    if (m_demoMode) {
        m_statusMessage = tr("[Demo] Replaying recorded session in terminal with scriptreplay.");
        emit statusMessageChanged();
        return;
    }
    if (logPath.trimmed().isEmpty() || timingPath.trimmed().isEmpty() ||
        !QFileInfo::exists(logPath) || !QFileInfo::exists(timingPath)) {
        setError(tr("Recorded session or timing file is missing."));
        return;
    }
    if (QStandardPaths::findExecutable(QStringLiteral("scriptreplay")).isEmpty()) {
        setError(tr("scriptreplay (util-linux) is not installed on the host."));
        return;
    }
    const QString replayCmd = QStringLiteral(
        "scriptreplay -t %1 -O %2; printf '\\n[Replay complete — press Enter to close]'; read -r _")
                                  .arg(shellQuote(timingPath), shellQuote(logPath));
    launchCommandInTerminal(replayCmd);
}

QString Backend::deleteRecordedSession(const QString &logPath, const QString &timingPath) {
    if (m_demoMode) {
        QVariantList next;
        for (const QVariant &v : m_recordedSessions) {
            if (v.toMap().value(QStringLiteral("logPath")).toString() != logPath)
                next.append(v);
        }
        m_recordedSessions = next;
        emit recordedSessionsChanged();
        return {};
    }

    const QString cleanSessionsRoot = QDir::cleanPath(sessionsDirPath());
    const QString cleanLog = QDir::cleanPath(logPath.trimmed());
    if (cleanLog.isEmpty() || !cleanLog.startsWith(cleanSessionsRoot + QLatin1Char('/')))
        return tr("Refusing to delete session outside %1.").arg(cleanSessionsRoot);

    QFile::remove(cleanLog);
    const QString cleanTiming = QDir::cleanPath(timingPath.trimmed());
    if (!cleanTiming.isEmpty() && cleanTiming.startsWith(cleanSessionsRoot + QLatin1Char('/')))
        QFile::remove(cleanTiming);

    refreshRecordedSessions();
    return {};
}
