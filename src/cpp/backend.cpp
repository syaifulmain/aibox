#include "backend.h"
#include "backend_helpers.h"
#include "aibox-ffi/src/config.rs.h"

#include <QClipboard>
#include <QDateTime>
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

QString Backend::adminFixCommand(const QString &message) const {
    static const QRegularExpression fixRe(
        QStringLiteral("Fix \\(admin\\):\\s*(.+)$"),
        QRegularExpression::CaseInsensitiveOption);
    static const QRegularExpression runRe(
        QStringLiteral("\\brun:\\s*(sudo\\s+.+)$"),
        QRegularExpression::CaseInsensitiveOption);
    const QStringList lines = message.split(QLatin1Char('\n'));
    for (const QString &line : lines) {
        const QRegularExpressionMatch fix = fixRe.match(line);
        if (fix.hasMatch())
            return fix.captured(1).trimmed();
        const QRegularExpressionMatch run = runRe.match(line);
        if (run.hasMatch())
            return run.captured(1).trimmed();
    }
    return {};
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
