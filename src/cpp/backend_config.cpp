#include "backend.h"
#include "backend_helpers.h"

#include <QDir>
#include <QFileInfo>
#include <QRegularExpression>
#include <QStandardPaths>

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
