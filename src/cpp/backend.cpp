#include "backend.h"
#include "backend_helpers.h"
#include "rust_bridge.h"
#include "aibox-ffi/src/config.rs.h"
#include "aibox-ffi/src/task.rs.h"

#include <QClipboard>
#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSet>
#include <QStandardPaths>
#include <QUrl>

Backend::Backend(const QString &workspaceDir, QObject *parent)
    : Backend(workspaceDir, PathMode::WorkspaceDir, parent) {}

Backend::Backend(const QString &path, PathMode mode, QObject *parent)
    : QObject(parent),
      m_pathMode(mode),
      m_rememberSystemLocation(path.trimmed().isEmpty()),
      m_counterState(aibox::ffi::new_counter_state()),
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

    loadConfig();
    reloadSecrets();
    connect(this, &Backend::environmentsChanged, this, [this] {
        if (m_environmentSecretsEnv.isEmpty())
            return;
        updateEnvironmentSecretAssignments(
            m_environmentSecretsEnv,
            environmentByName(m_environmentSecretsEnv)
                .value(QStringLiteral("secretNames")).toStringList());
    });
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
    if (m_enterPrepareProcess && m_enterPrepareProcess->state() != QProcess::NotRunning) {
        m_enterPrepareProcess->kill();
        m_enterPrepareProcess->waitForFinished(500);
    }
    if (m_enterSecretProcess && m_enterSecretProcess->state() != QProcess::NotRunning) {
        m_enterSecretProcess->kill();
        m_enterSecretProcess->waitForFinished(500);
    }
    if (!m_enterHostSecretFile.isEmpty())
        QFile::remove(m_enterHostSecretFile);
}

QString Backend::defaultSystemPath() {
    return aibox::ffi::fromRust(aibox::ffi::default_system_path());
}

QString Backend::defaultLocationFilePath() {
    return aibox::ffi::fromRust(aibox::ffi::default_location_file_path());
}

QString Backend::configuredSystemPath() {
    return aibox::ffi::fromRust(aibox::ffi::configured_system_path());
}

bool Backend::saveLocationFile(const QString &systemPath) {
    return aibox::ffi::save_location_file(aibox::ffi::toRust(systemPath));
}

QString Backend::resolveProjectAiboxDir(const QString &projectDir, const QString &baseCwd) {
    return aibox::ffi::fromRust(aibox::ffi::resolve_project_aibox_dir(
        aibox::ffi::toRust(projectDir), aibox::ffi::toRust(baseCwd)));
}

QString Backend::defaultWorkspacePath() {
    return aibox::ffi::fromRust(aibox::ffi::default_workspace_path());
}

QString Backend::defaultOsStoragePathValue() {
    return aibox::ffi::fromRust(aibox::ffi::default_os_storage_path());
}

QVariantList Backend::defaultOsEntries() {
    return aibox::ffi::fromRustJson(aibox::ffi::default_os_entries_json()).toList();
}

QVariantList Backend::defaultPresets() {
    return aibox::ffi::fromRustJson(aibox::ffi::default_presets_json()).toList();
}

QStringList Backend::defaultDiskOptions() {
    return aibox::ffi::fromRustVec(aibox::ffi::default_disk_options());
}

QStringList Backend::defaultRamOptions() {
    return aibox::ffi::fromRustVec(aibox::ffi::default_ram_options());
}

QVariantList Backend::defaultCpuOptions() {
    const auto vec = aibox::ffi::default_cpu_options();
    QVariantList out;
    out.reserve(static_cast<qsizetype>(vec.size()));
    for (std::int32_t v : vec)
        out.append(static_cast<int>(v));
    return out;
}

QString Backend::normalizedPath(const QString &raw, const QString &fallback) const {
    return aibox::ffi::fromRust(
        aibox::ffi::normalized_path(aibox::ffi::toRust(raw), aibox::ffi::toRust(fallback)));
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
    return aibox::ffi::fromRust(
        aibox::ffi::cache_base_path(aibox::ffi::toRust(m_osStoragePath), aibox::ffi::toRust(osId)));
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

    const QString compact = QString::fromUtf8(QJsonDocument(obj).toJson(QJsonDocument::Compact));
    const QString normalized = aibox::ffi::fromRust(aibox::ffi::config_normalize_json(aibox::ffi::toRust(compact)));
    const QJsonObject out = QJsonDocument::fromJson(normalized.toUtf8()).object();
    if (!out.isEmpty()) {
        m_defaultPresetId = out.value("defaultPresetId").toString();
        m_defaultOsId = out.value("defaultOsId").toString();
        m_defaultDisk = out.value("defaultDisk").toString();
        m_defaultRam = out.value("defaultRam").toString();
        m_defaultCpu = out.value("defaultCpu").toInt();
    }
}

void Backend::loadConfig() {
    const rust::String raw = aibox::ffi::config_load(
        aibox::ffi::toRust(m_configPath),
        aibox::ffi::toRust(m_defaultWorkspaceBase),
        aibox::ffi::toRust(m_defaultOsStoragePath));
    applyConfigJson(QByteArray(raw.data(), static_cast<qsizetype>(raw.size())));
}

void Backend::applyConfigJson(const QByteArray &raw) {
    const QJsonDocument doc = QJsonDocument::fromJson(raw);
    const QJsonObject obj = doc.object();
    if (obj.isEmpty()) {
        m_firstRun = true;
        return;
    }

    m_firstRun = !obj.value("firstRunCompleted").toBool(false);
    m_workspaceBase = obj.value("workspaceBase").toString();
    m_osStoragePath = obj.value("osStoragePath").toString();

    QVariantList osEntries;
    for (const auto &v : obj.value("osCatalog").toArray())
        osEntries.append(v.toObject().toVariantMap());
    m_osEntries = osEntries;

    QVariantList presets;
    for (const auto &v : obj.value("presets").toArray())
        presets.append(v.toObject().toVariantMap());
    m_presets = presets;

    QStringList diskOptions;
    for (const auto &v : obj.value("diskOptions").toArray())
        diskOptions.append(v.toString());
    m_diskOptions = diskOptions;

    QStringList ramOptions;
    for (const auto &v : obj.value("ramOptions").toArray())
        ramOptions.append(v.toString());
    m_ramOptions = ramOptions;

    QVariantList cpuOptions;
    for (const auto &v : obj.value("cpuOptions").toArray())
        cpuOptions.append(v.toInt());
    m_cpuOptions = cpuOptions;

    m_defaultPresetId = obj.value("defaultPresetId").toString();
    m_defaultOsId = obj.value("defaultOsId").toString();
    m_defaultDisk = obj.value("defaultDisk").toString();
    m_defaultRam = obj.value("defaultRam").toString();
    m_defaultCpu = obj.value("defaultCpu").toInt();

    m_autoSnapshotEnabled = obj.value("autoSnapshotEnabled").toBool(true);
    m_autoSnapshotKeep = obj.value("autoSnapshotKeep").toInt(5);
    m_watchdogEnabled = obj.value("watchdogEnabled").toBool(true);
    m_watchdogDiskWarn = obj.value("watchdogDiskWarn").toInt(90);
    m_watchdogDiskCritical = obj.value("watchdogDiskCritical").toInt(97);
    m_watchdogAutoStop = obj.value("watchdogAutoStop").toBool(false);
    m_idleStopMinutes = obj.value("idleStopMinutes").toInt(0);
    m_staleDays = obj.value("staleDays").toInt(30);
    m_recordSessions = obj.value("recordSessions").toBool(false);

    QStringList diffExcludes;
    for (const auto &v : obj.value("diffExcludes").toArray())
        diffExcludes.append(v.toString());
    m_diffExcludes = diffExcludes;
}

QByteArray Backend::configToJson() const {
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

    return QJsonDocument(obj).toJson(QJsonDocument::Compact);
}

bool Backend::saveConfig() const {
    const QByteArray compact = configToJson();
    const rust::String err = aibox::ffi::config_save(
        aibox::ffi::toRust(m_configPath),
        rust::Str(compact.constData(), static_cast<std::size_t>(compact.size())));
    return err.empty();
}

void Backend::rebuildOsCatalog() {
    const QVariantList catalog = catalogForEntries(m_osEntries, m_lastImagesJson, m_downloadingOsId, m_osStoragePath);
    normalizeDefaults();
    if (catalog != m_osCatalog) {
        m_osCatalog = catalog;
        emit osCatalogChanged();
    }
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
    return aibox::ffi::fromRustVec(aibox::ffi::parse_csv(aibox::ffi::toRust(text)));
}

QString Backend::shellQuote(const QString &value) {
    return aibox::ffi::fromRust(aibox::ffi::shell_quote(aibox::ffi::toRust(value)));
}

QString Backend::shellArrayWords(const QString &words) {
    return aibox::ffi::fromRust(aibox::ffi::shell_array_words(aibox::ffi::toRust(words)));
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

QString Backend::sanitizeName(const QString &rawName) {
    return aibox::ffi::fromRust(aibox::ffi::sanitize_name(aibox::ffi::toRust(rawName)));
}

QString Backend::resolveWorkspacePath(const QString &rawParent, const QString &envName,
                                      const QString &defaultBase, QString *error) {
    const auto res = aibox::ffi::resolve_workspace_path(
        aibox::ffi::toRust(rawParent), aibox::ffi::toRust(envName), aibox::ffi::toRust(defaultBase));
    if (error)
        *error = aibox::ffi::fromRust(res.error);
    return aibox::ffi::fromRust(res.path);
}

QVariantList Backend::catalogForEntries(const QVariantList &entries, const QByteArray &imagesJson,
                                        const QString &downloadingId, const QString &storagePath) {
    std::vector<aibox::ffi::OsEntryFullFfi> rustEntries;
    rustEntries.reserve(entries.size());
    for (const auto &value : entries) {
        const QVariantMap entry = value.toMap();
        rustEntries.push_back({
            aibox::ffi::toRust(entry.value("id").toString()),
            aibox::ffi::toRust(entry.value("alias").toString()),
            aibox::ffi::toRust(entry.value("name").toString()),
            aibox::ffi::toRust(entry.value("badge").toString()),
            aibox::ffi::toRust(entry.value("description").toString()),
        });
    }
    const rust::String outJson = aibox::ffi::catalog_for_entries_json(
        rust::Slice<const aibox::ffi::OsEntryFullFfi>(rustEntries.data(), rustEntries.size()),
        rust::Slice<const uint8_t>(reinterpret_cast<const uint8_t *>(imagesJson.constData()),
                                   static_cast<size_t>(imagesJson.size())),
        aibox::ffi::toRust(downloadingId),
        aibox::ffi::toRust(storagePath));
    return aibox::ffi::fromRustJson(outJson).toList();
}

QVariantList Backend::buildOsCatalog(const QByteArray &imagesJson, const QString &downloadingId) {
    const rust::String outJson = aibox::ffi::build_os_catalog_json(
        rust::Slice<const uint8_t>(reinterpret_cast<const uint8_t *>(imagesJson.constData()),
                                   static_cast<size_t>(imagesJson.size())),
        aibox::ffi::toRust(downloadingId));
    return aibox::ffi::fromRustJson(outJson).toList();
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

            const rust::String envsJson = aibox::ffi::parse_incus_list_with_state(
                *m_counterState,
                rust::Slice<const uint8_t>(reinterpret_cast<const uint8_t *>(listJson.constData()),
                                           static_cast<size_t>(listJson.size())),
                aibox::ffi::toRust(m_workspaceBase),
                elapsedWallNs,
                m_autoSnapshotEnabled,
                m_watchdogAutoStop,
                m_recordSessions,
                true);
            QVariantList newEnvs = aibox::ffi::fromRustJson(envsJson).toList();
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
    return static_cast<qint64>(aibox::ffi::read_task_pid(aibox::ffi::toRust(m_stateDir)));
}

bool Backend::isTaskLockActive(qint64 *outPid) const {
    std::int64_t pid = 0;
    const bool active = aibox::ffi::is_task_lock_active(aibox::ffi::toRust(m_stateDir), m_busy, pid);
    if (outPid)
        *outPid = static_cast<qint64>(pid);
    return active;
}

bool Backend::acquireCliTaskLock(const QString &description, const QString &envName) {
    return aibox::ffi::acquire_cli_task_lock(
        aibox::ffi::toRust(m_stateDir), m_busy, aibox::ffi::toRust(description), aibox::ffi::toRust(envName));
}

void Backend::releaseCliTaskLock() {
    aibox::ffi::release_cli_task_lock(aibox::ffi::toRust(m_stateDir));
}

bool Backend::refreshSync(QString *errorOut) {
    const auto res = aibox::ffi::engine_refresh_sync(
        aibox::ffi::toRust(m_workspaceBase), aibox::ffi::toRust(m_stateDir),
        m_autoSnapshotEnabled, m_watchdogAutoStop, m_recordSessions);
    if (!res.ok) {
        if (errorOut)
            *errorOut = aibox::ffi::fromRust(res.error);
        return false;
    }
    m_environments = aibox::ffi::fromRustJson(res.environments_json).toList();
    applyFileAccessState(m_environments);
    m_count = m_environments.size();
    return true;
}

void Backend::clearTaskFiles() {
    aibox::ffi::clear_task_files(aibox::ffi::toRust(m_stateDir));
}

void Backend::handleProcessOutput(const QByteArray &chunk) {
    aibox::ffi::TaskProgressFfi state{
        m_taskProgress,
        aibox::ffi::toRust(m_taskStage),
        aibox::ffi::toRust(m_taskLog),
        m_hasExitCode,
        m_lastExitCode,
        aibox::ffi::toRust(m_pendingEnvName),
    };
    const auto secretVec = aibox::ffi::toRustVec(allSecretValues());
    const rust::Slice<const rust::String> secretSlice(secretVec.data(), secretVec.size());
    const rust::Slice<const std::uint8_t> bytes = aibox::ffi::toRustBytes(chunk);
    const bool changed = aibox::ffi::parse_process_output_chunk(state, bytes, secretSlice);

    m_taskProgress = state.task_progress;
    m_taskStage = aibox::ffi::fromRust(state.task_stage);
    m_taskLog = aibox::ffi::fromRust(state.task_log);
    m_hasExitCode = state.has_exit_code;
    m_lastExitCode = state.last_exit_code;
    if (changed)
        emit taskProgressChanged();
}

void Backend::attachExistingTask() {
    const auto secretVec = aibox::ffi::toRustVec(allSecretValues());
    const rust::Slice<const rust::String> secretSlice(secretVec.data(), secretVec.size());
    const auto res = aibox::ffi::evaluate_attach_existing_task(
        aibox::ffi::toRust(m_stateDir), secretSlice, 50);
    if (res.kind == 0)
        return;

    if (res.kind == 1) {
        m_pendingEnvName = aibox::ffi::fromRust(res.env_name);
        m_downloadingOsId = aibox::ffi::fromRust(res.downloading_os_id);
        m_logOffset = static_cast<qint64>(res.log_offset);
        m_hasExitCode = res.progress.has_exit_code;
        m_lastExitCode = res.progress.last_exit_code;
        m_taskProgress = res.progress.task_progress;
        m_taskStage = aibox::ffi::fromRust(res.progress.task_stage);
        m_taskLog = res.progress.task_log.empty()
                        ? tr("Resuming background task monitor...")
                        : aibox::ffi::fromRust(res.progress.task_log);
        setBusy(true, aibox::ffi::fromRust(res.description));
        emit taskProgressChanged();
        m_taskWatchTimer.start();
    } else {
        if (!res.history_json.empty()) {
            m_operationHistory = aibox::ffi::fromRustJson(res.history_json).toList();
            emit operationHistoryChanged();
        }
        m_downloadingOsId.clear();
        m_pendingEnvName.clear();
        m_logOffset = 0;
        m_hasExitCode = res.has_exit_code;
        m_lastExitCode = res.exit_code;
        m_taskLog = aibox::ffi::fromRust(res.progress.task_log);
        setBusy(false, {});
        const QString envName = aibox::ffi::fromRust(res.env_name);
        if (res.has_exit_code && res.exit_code == 0) {
            m_statusMessage = !envName.isEmpty()
                                  ? tr("Environment '%1' finished creating in the background.").arg(envName)
                                  : tr("Background operation completed successfully.");
            emit statusMessageChanged();
        } else if (res.has_exit_code && res.exit_code != 0) {
            setError(tr("Previous background operation failed: %1").arg(m_taskLog));
        }
    }
}

void Backend::checkBackgroundTask() {
    aibox::ffi::TaskProgressFfi state{
        m_taskProgress,
        aibox::ffi::toRust(m_taskStage),
        aibox::ffi::toRust(m_taskLog),
        m_hasExitCode,
        m_lastExitCode,
        aibox::ffi::toRust(m_pendingEnvName),
    };
    const auto secretVec = aibox::ffi::toRustVec(allSecretValues());
    const rust::Slice<const rust::String> secretSlice(secretVec.data(), secretVec.size());
    const auto res = aibox::ffi::evaluate_check_background_task(
        aibox::ffi::toRust(m_stateDir), static_cast<std::int64_t>(m_logOffset),
        state, secretSlice, 50);

    m_logOffset = static_cast<qint64>(res.log_offset);
    m_taskProgress = res.progress.task_progress;
    m_taskStage = aibox::ffi::fromRust(res.progress.task_stage);
    m_taskLog = aibox::ffi::fromRust(res.progress.task_log);
    m_hasExitCode = res.progress.has_exit_code;
    m_lastExitCode = res.progress.last_exit_code;
    if (res.progress_changed)
        emit taskProgressChanged();

    if (!res.finished)
        return;

    m_taskWatchTimer.stop();
    const QString finishedEnv = aibox::ffi::fromRust(res.finished_env);
    const QString lastLog = aibox::ffi::fromRust(res.last_log);
    if (!res.history_json.empty()) {
        m_operationHistory = aibox::ffi::fromRustJson(res.history_json).toList();
        emit operationHistoryChanged();
    }

    m_downloadingOsId.clear();
    m_pendingEnvName.clear();
    setBusy(false, {});
    refresh();
    refreshBackups();
    if (!m_staleReport.isEmpty())
        refreshStaleReport();

    if (res.exit_code == 0) {
        if (!finishedEnv.isEmpty()) {
            m_statusMessage = tr("Environment '%1' is ready.").arg(finishedEnv);
            emit statusMessageChanged();
            emit environmentCreated(finishedEnv);
        }
    } else {
        setError(lastLog.isEmpty() ? tr("Operation failed (exit code %1).").arg(res.exit_code) : lastLog);
    }
}

void Backend::cancelTask() {
    if (!m_busy)
        return;

    m_taskWatchTimer.stop();
    const auto secretVec = aibox::ffi::toRustVec(allSecretValues());
    const rust::Slice<const rust::String> secretSlice(secretVec.data(), secretVec.size());
    const auto histJson = aibox::ffi::cancel_running_task(
        aibox::ffi::toRust(m_stateDir), aibox::ffi::toRust(m_taskStage),
        aibox::ffi::toRust(m_pendingEnvName), secretSlice, 50);
    if (!histJson.empty()) {
        m_operationHistory = aibox::ffi::fromRustJson(histJson).toList();
        emit operationHistoryChanged();
    }

    const QString cleanupName = m_pendingEnvName;
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

    const QString launchCmd = aibox::ffi::fromRust(aibox::ffi::prepare_shell_task(
        aibox::ffi::toRust(m_stateDir), aibox::ffi::toRust(description),
        aibox::ffi::toRust(script), aibox::ffi::toRust(createdEnvName),
        aibox::ffi::toRust(targetEnvName), aibox::ffi::toRust(m_downloadingOsId),
        aibox::ffi::toRust(reason)));
    if (!launchCmd.isEmpty())
        QProcess::execute(QStringLiteral("/bin/sh"), {QStringLiteral("-c"), launchCmd});

    m_taskWatchTimer.start();
}

QString Backend::adminFixCommand(const QString &message) const {
    return aibox::ffi::fromRust(aibox::ffi::admin_fix_command(aibox::ffi::toRust(message)));
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
