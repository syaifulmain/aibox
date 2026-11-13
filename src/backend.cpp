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
#include <QSaveFile>
#include <QSet>
#include <QStandardPaths>
#include <QUrl>
#include <csignal>
#include <unistd.h>

namespace {

QString formatBytes(qint64 bytes) {
    if (bytes <= 0)
        return QStringLiteral("0 MB");
    const double mb = static_cast<double>(bytes) / (1024.0 * 1024.0);
    if (mb >= 1024.0)
        return QStringLiteral("%1 GB").arg(mb / 1024.0, 0, 'f', 1);
    return QStringLiteral("%1 MB").arg(mb, 0, 'f', 0);
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

} // namespace

Backend::Backend(const QString &workspaceDir, QObject *parent)
    : QObject(parent),
      m_workspaceBase(workspaceDir.isEmpty() ? defaultWorkspacePath() : QDir::cleanPath(workspaceDir)),
      m_osStoragePath(workspaceDir.isEmpty() ? defaultOsStoragePathValue() : workspaceDir + "/os-images"),
      m_stateDir(workspaceDir.isEmpty() ? QDir::currentPath() + "/.aibox/state" : workspaceDir + "/.state"),
      m_configPath(workspaceDir.isEmpty()
                       ? QDir::currentPath() + "/.aibox/config.json"
                       : workspaceDir + "/.state/config.json"),
      m_demoMode(qEnvironmentVariableIntValue("AIBOX_DEMO_MODE") == 1) {
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

    loadConfig();
    rebuildOsCatalog();

    if (m_demoMode) {
        loadDemoData();
        return;
    }

    QDir().mkpath(m_workspaceBase);
    QDir().mkpath(m_osStoragePath);
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

QString Backend::defaultWorkspacePath() {
    return QDir::currentPath() + "/.aibox/workspaces";
}

QString Backend::defaultOsStoragePathValue() {
    return QDir::currentPath() + "/.aibox/os-images";
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
        m_workspaceBase = normalizedPath(workspace, defaultWorkspacePath());

    const QString osStorage = obj.value("osStoragePath").toString().trimmed();
    if (!osStorage.isEmpty())
        m_osStoragePath = normalizedPath(osStorage, defaultOsStoragePathValue());

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

QString Backend::completeFirstRun(const QString &workspacePath) {
    const QString workspace = normalizedPath(workspacePath, m_workspaceBase);
    if (!QDir().mkpath(workspace))
        return tr("Could not create the workspace directory: %1").arg(workspace);
    const QFileInfo info(workspace);
    if (!info.isDir() || !info.isWritable())
        return tr("The workspace directory is not writable: %1").arg(workspace);

    const bool workspaceChanged = workspace != m_workspaceBase;
    m_workspaceBase = workspace;
    m_firstRun = false;
    if (!saveConfig())
        return tr("Could not save the configuration file: %1").arg(m_configPath);

    emit firstRunChanged();
    if (workspaceChanged)
        emit workspaceBaseChanged();
    return {};
}

QString Backend::saveSettings(const QString &workspacePath, const QString &osStoragePath) {
    if (m_busy)
        return tr("Wait for the current operation to finish before changing the configuration.");
    const QString workspace = normalizedPath(workspacePath, defaultWorkspacePath());
    const QString osStorage = normalizedPath(osStoragePath, defaultOsStoragePathValue());

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

QUrl Backend::pathToUrl(const QString &path) const {
    return QUrl::fromLocalFile(path);
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

        QString workspacePath = expandedDevices.value("workspace").toObject().value("source").toString();
        if (workspacePath.isEmpty())
            workspacePath = devices.value("workspace").toObject().value("source").toString();
        if (workspacePath.isEmpty())
            workspacePath = workspaceBase + "/" + name;

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
        item["workspacePath"] = workspacePath;
        item["snapshotCount"] = snapshots.size();
        item["snapshots"] = snapshots;
        result.append(item);
    }

    return result;
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
            m_lastImagesJson = imgJson;

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

void Backend::createEnvironment(const QString &rawName, const QString &osId, const QString &presetId,
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

    const QString script = createEnvironmentScript(name,
                                                   localAlias,
                                                   QStringLiteral("images:") + osAlias,
                                                   hostWorkspace,
                                                   cacheBase,
                                                   preset.value("flags").toString(),
                                                   ramGiB,
                                                   diskGiB,
                                                   cpuCores,
                                                   preset.value("x11").toBool());

    runShellTask(tr("Creating environment '%1'").arg(name), script, name);
}

QString Backend::createEnvironmentScript(const QString &name, const QString &localAlias, const QString &remoteImage,
                                         const QString &workspace, const QString &cacheBase,
                                         const QString &profileFlags, const QString &ramGiB, const QString &diskGiB,
                                         int cpuCores, bool x11) {
    const QString quotaArgs = QStringLiteral("-c limits.cpu=%1 -c limits.memory=%2 -d root,size=%3")
                                  .arg(QString::number(qMax(1, cpuCores)), ramGiB.trimmed(), diskGiB.trimmed());

    return QStringLiteral(R"sh(
set -euo pipefail
NAME=%1
LOCAL_ALIAS=%2
REMOTE_IMAGE=%3
WORKSPACE=%4
CACHE=%5
CACHE_ROOT=%6
PROFILE_ARGS=(%7)
QUOTA_ARGS=(%8)
X11=%9

echo "@@STAGE:8:Step 1/4 — Preparing shared workspace directory..."
mkdir -p "$WORKSPACE"
if command -v setfacl >/dev/null 2>&1; then
    ACL_USER="${SUDO_USER:-${USER:-$(id -un)}}"
    setfacl -R -m "u:${ACL_USER}:rwx" -d -m "u:${ACL_USER}:rwx" "$WORKSPACE" 2>/dev/null || true
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

echo "@@STAGE:86:Step 4/4 — Mounting /workspace & waiting for network IP..."
sleep 1

if ! incus config device add "$NAME" workspace disk source="$WORKSPACE" path=/workspace shift=true >/dev/null 2>&1; then
    incus config device add "$NAME" workspace disk source="$WORKSPACE" path=/workspace >/dev/null
fi

if [[ "$X11" == "1" ]] && [[ -d /tmp/.X11-unix ]]; then
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
                               .arg(shellQuote(name),
                                    shellQuote(localAlias),
                                    shellQuote(remoteImage),
                                    shellQuote(workspace),
                                    shellQuote(cacheBase),
                                    shellQuote(cacheBase + ".root"),
                                    shellArrayWords(profileFlags),
                                    shellArrayWords(quotaArgs),
                                    x11 ? QStringLiteral("1") : QStringLiteral("0"));
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
    const QString path = workspacePathFor(name);
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
    const QString hostWorkspace = workspacePathFor(name);
    QString script = QStringLiteral("echo '@@STAGE:50:Wiping OS container...'; incus delete '%1' --force").arg(name);
    if (deleteWorkspaceFiles)
        script += QStringLiteral(" && rm -rf %1").arg(shellQuote(hostWorkspace));
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
