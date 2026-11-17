#include "backend.h"
#include "backend_helpers.h"
#include "rust_bridge.h"

#include "aibox-ffi/src/settings.rs.h"

#include <QFileInfo>
#include <QStandardPaths>

namespace {

rust::Str toRustBytes(const QByteArray &bytes) {
    return rust::Str(bytes.constData(), static_cast<std::size_t>(bytes.size()));
}

QByteArray fromRustBytes(const rust::String &s) {
    return QByteArray(s.data(), static_cast<qsizetype>(s.size()));
}

}  // namespace

QString Backend::completeFirstRun(const QString &systemPath) {
    const auto res = aibox::ffi::settings_complete_first_run(
        aibox::ffi::toRust(systemPath),
        aibox::ffi::toRust(m_systemPath),
        aibox::ffi::toRust(m_workspaceBase),
        aibox::ffi::toRust(m_osStoragePath),
        aibox::ffi::toRust(m_stateDir),
        aibox::ffi::toRust(m_configPath),
        m_pathMode == PathMode::ProjectDir,
        m_rememberSystemLocation);
    if (!res.error.empty())
        return aibox::ffi::fromRust(res.error);

    const QString targetRoot = aibox::ffi::fromRust(res.system_path);
    const QString nextWorkspace = aibox::ffi::fromRust(res.workspace_base);
    const QString nextOsStorage = aibox::ffi::fromRust(res.os_storage_path);
    const QString nextStateDir = aibox::ffi::fromRust(res.state_dir);
    const QString nextConfigPath = aibox::ffi::fromRust(res.config_path);

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
    const QByteArray cfgBytes = configToJson();
    const auto res = aibox::ffi::settings_save_settings(
        toRustBytes(cfgBytes),
        aibox::ffi::toRust(workspacePath),
        aibox::ffi::toRust(osStoragePath),
        aibox::ffi::toRust(m_defaultWorkspaceBase),
        aibox::ffi::toRust(m_defaultOsStoragePath));
    if (!res.error.empty())
        return aibox::ffi::fromRust(res.error);

    const QString prevWorkspace = m_workspaceBase;
    const QString prevStorage = m_osStoragePath;
    const bool wasFirstRun = m_firstRun;
    applyConfigJson(fromRustBytes(res.config_json));

    if (!saveConfig())
        return tr("Could not save the configuration file: %1").arg(m_configPath);

    const bool workspaceChanged = m_workspaceBase != prevWorkspace;
    const bool storageChanged = m_osStoragePath != prevStorage;
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
    const QByteArray cfgBytes = configToJson();
    const auto res = aibox::ffi::settings_save_os_entry(
        toRustBytes(cfgBytes),
        aibox::ffi::toRust(originalId),
        aibox::ffi::toRust(id),
        aibox::ffi::toRust(alias),
        aibox::ffi::toRust(name),
        aibox::ffi::toRust(badge),
        aibox::ffi::toRust(description));
    if (!res.error.empty())
        return aibox::ffi::fromRust(res.error);

    applyConfigJson(fromRustBytes(res.config_json));
    if (!saveConfig())
        return tr("Could not save the configuration file: %1").arg(m_configPath);

    rebuildOsCatalog();
    const QString savedName = osEntryById(id.trimmed().toLower()).value(QStringLiteral("name")).toString();
    m_statusMessage = tr("OS entry '%1' saved.").arg(savedName);
    emit statusMessageChanged();
    return {};
}

QString Backend::removeOsEntry(const QString &id) {
    if (m_busy)
        return tr("Wait for the current operation to finish before changing the configuration.");
    const QByteArray cfgBytes = configToJson();
    const auto res = aibox::ffi::settings_remove_os_entry(
        toRustBytes(cfgBytes), aibox::ffi::toRust(id));
    if (!res.error.empty())
        return aibox::ffi::fromRust(res.error);

    applyConfigJson(fromRustBytes(res.config_json));
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
    const QByteArray cfgBytes = configToJson();
    const auto res = aibox::ffi::settings_reset_os_catalog(toRustBytes(cfgBytes));
    if (!res.error.empty())
        return aibox::ffi::fromRust(res.error);

    applyConfigJson(fromRustBytes(res.config_json));
    if (!saveConfig())
        return tr("Could not save the configuration file: %1").arg(m_configPath);
    rebuildOsCatalog();
    return {};
}

QString Backend::saveCreateOptions(const QString &diskCsv, const QString &ramCsv, const QString &cpuCsv) {
    if (m_busy)
        return tr("Wait for the current operation to finish before changing the configuration.");
    const QByteArray cfgBytes = configToJson();
    const auto res = aibox::ffi::settings_save_create_options(
        toRustBytes(cfgBytes),
        aibox::ffi::toRust(diskCsv),
        aibox::ffi::toRust(ramCsv),
        aibox::ffi::toRust(cpuCsv));
    if (!res.error.empty())
        return aibox::ffi::fromRust(res.error);

    applyConfigJson(fromRustBytes(res.config_json));
    if (!saveConfig())
        return tr("Could not save the configuration file: %1").arg(m_configPath);

    emit createOptionsChanged();
    return {};
}

QString Backend::savePreset(const QString &id, const QString &title, const QString &subtitle) {
    if (m_busy)
        return tr("Wait for the current operation to finish before changing the configuration.");
    const QByteArray cfgBytes = configToJson();
    const auto res = aibox::ffi::settings_save_preset(
        toRustBytes(cfgBytes),
        aibox::ffi::toRust(id),
        aibox::ffi::toRust(title),
        aibox::ffi::toRust(subtitle));
    if (!res.error.empty())
        return aibox::ffi::fromRust(res.error);

    applyConfigJson(fromRustBytes(res.config_json));
    if (!saveConfig())
        return tr("Could not save the configuration file: %1").arg(m_configPath);

    emit createOptionsChanged();
    return {};
}

QString Backend::resetCreateOptions() {
    if (m_busy)
        return tr("Wait for the current operation to finish before changing the configuration.");
    const QByteArray cfgBytes = configToJson();
    const auto res = aibox::ffi::settings_reset_create_options(toRustBytes(cfgBytes));
    if (!res.error.empty())
        return aibox::ffi::fromRust(res.error);

    applyConfigJson(fromRustBytes(res.config_json));
    if (!saveConfig())
        return tr("Could not save the configuration file: %1").arg(m_configPath);
    emit createOptionsChanged();
    return {};
}

QString Backend::saveAutoSnapshotSettings(bool enabled, int keepCount) {
    if (m_busy)
        return tr("Wait for the current operation to finish before changing the configuration.");
    const QByteArray cfgBytes = configToJson();
    const auto res = aibox::ffi::settings_save_auto_snapshot(toRustBytes(cfgBytes), enabled, keepCount);
    if (!res.error.empty())
        return aibox::ffi::fromRust(res.error);

    applyConfigJson(fromRustBytes(res.config_json));
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
    const QByteArray cfgBytes = configToJson();
    const auto res = aibox::ffi::settings_save_watchdog(
        toRustBytes(cfgBytes), enabled, diskWarn, diskCritical, autoStop);
    if (!res.error.empty())
        return aibox::ffi::fromRust(res.error);

    applyConfigJson(fromRustBytes(res.config_json));
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
    QVariantList out;
    for (int day : aibox::ffi::settings_stale_day_options())
        out.append(day);
    return out;
}

QString Backend::saveIdleStopSettings(int minutes) {
    if (m_busy)
        return tr("Wait for the current operation to finish before changing the configuration.");
    const QByteArray cfgBytes = configToJson();
    const auto res = aibox::ffi::settings_save_idle_stop(toRustBytes(cfgBytes), minutes);
    if (!res.error.empty())
        return aibox::ffi::fromRust(res.error);

    applyConfigJson(fromRustBytes(res.config_json));
    if (!saveConfig())
        return tr("Could not save the configuration file: %1").arg(m_configPath);

    emit idleStopSettingsChanged();
    return {};
}

QString Backend::saveStaleSettings(int days) {
    if (m_busy)
        return tr("Wait for the current operation to finish before changing the configuration.");
    const QByteArray cfgBytes = configToJson();
    const auto res = aibox::ffi::settings_save_stale(toRustBytes(cfgBytes), days);
    if (!res.error.empty())
        return aibox::ffi::fromRust(res.error);

    applyConfigJson(fromRustBytes(res.config_json));
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
    const QByteArray cfgBytes = configToJson();
    const auto res = aibox::ffi::settings_save_session_recording(toRustBytes(cfgBytes), enabled);
    if (!res.error.empty())
        return aibox::ffi::fromRust(res.error);

    applyConfigJson(fromRustBytes(res.config_json));
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
    const QByteArray cfgBytes = configToJson();
    const auto res = aibox::ffi::settings_save_diff_excludes(
        toRustBytes(cfgBytes), aibox::ffi::toRust(rawExcludes));
    if (!res.error.empty())
        return aibox::ffi::fromRust(res.error);

    applyConfigJson(fromRustBytes(res.config_json));
    if (!saveConfig())
        return tr("Could not save the configuration file: %1").arg(m_configPath);
    emit diffExcludesChanged();
    return {};
}
