#pragma once

#include <QDateTime>
#include <QElapsedTimer>
#include <QHash>
#include <QObject>
#include <QProcess>
#include <QSet>
#include <QStringList>
#include <QTimer>
#include <QUrl>
#include <QVariantList>
#include <QVariantMap>
#include <functional>

class QImage;

class Backend : public QObject {
    Q_OBJECT
    friend struct GoldenCapture;
    Q_PROPERTY(int count READ count NOTIFY countChanged)
    Q_PROPERTY(QVariantList environments READ environments NOTIFY environmentsChanged)
    Q_PROPERTY(QVariantList osCatalog READ osCatalog NOTIFY osCatalogChanged)
    Q_PROPERTY(int selectedIndex READ selectedIndex WRITE setSelectedIndex NOTIFY selectedIndexChanged)
    Q_PROPERTY(QVariantMap selectedEnvironment READ selectedEnvironment NOTIFY selectedIndexChanged)
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)
    Q_PROPERTY(int taskProgress READ taskProgress NOTIFY taskProgressChanged)
    Q_PROPERTY(QString taskStage READ taskStage NOTIFY taskProgressChanged)
    Q_PROPERTY(QString taskLog READ taskLog NOTIFY taskProgressChanged)
    Q_PROPERTY(QString pendingEnvName READ pendingEnvName NOTIFY busyChanged)
    Q_PROPERTY(QString statusMessage READ statusMessage NOTIFY statusMessageChanged)
    Q_PROPERTY(QString errorMessage READ errorMessage NOTIFY errorMessageChanged)
    Q_PROPERTY(QString downloadingOsId READ downloadingOsId NOTIFY osCatalogChanged)
    Q_PROPERTY(QString systemPath READ systemPath NOTIFY systemPathChanged)
    Q_PROPERTY(QString defaultSystemPath READ defaultSystemPath CONSTANT)
    Q_PROPERTY(QString workspaceBase READ workspaceBase NOTIFY workspaceBaseChanged)
    Q_PROPERTY(QString containerWorkspacePath READ containerWorkspacePath CONSTANT)
    Q_PROPERTY(QString osStoragePath READ osStoragePath NOTIFY osStoragePathChanged)
    Q_PROPERTY(QString defaultWorkspaceBase READ defaultWorkspaceBase CONSTANT)
    Q_PROPERTY(QString defaultOsStoragePath READ defaultOsStoragePath CONSTANT)
    Q_PROPERTY(QString configFilePath READ configFilePath NOTIFY configFilePathChanged)
    Q_PROPERTY(bool firstRun READ firstRun NOTIFY firstRunChanged)
    Q_PROPERTY(QVariantList presets READ presets NOTIFY createOptionsChanged)
    Q_PROPERTY(QStringList diskOptions READ diskOptions NOTIFY createOptionsChanged)
    Q_PROPERTY(QStringList ramOptions READ ramOptions NOTIFY createOptionsChanged)
    Q_PROPERTY(QVariantList cpuOptions READ cpuOptions NOTIFY createOptionsChanged)
    Q_PROPERTY(QString defaultPresetId READ defaultPresetId NOTIFY createOptionsChanged)
    Q_PROPERTY(QString defaultOsId READ defaultOsId NOTIFY osCatalogChanged)
    Q_PROPERTY(QString defaultDisk READ defaultDisk NOTIFY createOptionsChanged)
    Q_PROPERTY(QString defaultRam READ defaultRam NOTIFY createOptionsChanged)
    Q_PROPERTY(int defaultCpu READ defaultCpu NOTIFY createOptionsChanged)
    Q_PROPERTY(bool autoSnapshotEnabled READ autoSnapshotEnabled NOTIFY autoSnapshotSettingsChanged)
    Q_PROPERTY(int autoSnapshotKeep READ autoSnapshotKeep NOTIFY autoSnapshotSettingsChanged)
    Q_PROPERTY(QVariantList templates READ templates NOTIFY environmentsChanged)
    Q_PROPERTY(QStringList processLimitOptions READ processLimitOptions CONSTANT)
    Q_PROPERTY(QStringList ioLimitOptions READ ioLimitOptions CONSTANT)
    Q_PROPERTY(QString defaultProcessLimit READ defaultProcessLimit NOTIFY createOptionsChanged)
    Q_PROPERTY(QString defaultIoLimit READ defaultIoLimit NOTIFY createOptionsChanged)
    Q_PROPERTY(bool watchdogEnabled READ watchdogEnabled NOTIFY watchdogSettingsChanged)
    Q_PROPERTY(int watchdogDiskWarn READ watchdogDiskWarn NOTIFY watchdogSettingsChanged)
    Q_PROPERTY(int watchdogDiskCritical READ watchdogDiskCritical NOTIFY watchdogSettingsChanged)
    Q_PROPERTY(bool watchdogAutoStop READ watchdogAutoStop NOTIFY watchdogSettingsChanged)
    Q_PROPERTY(int idleStopMinutes READ idleStopMinutes NOTIFY idleStopSettingsChanged)
    Q_PROPERTY(QVariantList idleStopOptions READ idleStopOptions CONSTANT)
    Q_PROPERTY(int staleDays READ staleDays NOTIFY staleReportSettingsChanged)
    Q_PROPERTY(QVariantList staleDayOptions READ staleDayOptions CONSTANT)
    Q_PROPERTY(QVariantMap staleReport READ staleReport NOTIFY staleReportChanged)
    Q_PROPERTY(bool recordSessions READ recordSessions NOTIFY recordSessionsSettingsChanged)
    Q_PROPERTY(QVariantList recordedSessions READ recordedSessions NOTIFY recordedSessionsChanged)
    Q_PROPERTY(QVariantList operationHistory READ operationHistory NOTIFY operationHistoryChanged)
    Q_PROPERTY(int recentFailedHistoryCount READ recentFailedHistoryCount NOTIFY operationHistoryChanged)
    Q_PROPERTY(QVariantList egressLogs READ egressLogs NOTIFY egressLogsChanged)
    Q_PROPERTY(QString egressLogsEnvName READ egressLogsEnvName NOTIFY egressLogsChanged)
    Q_PROPERTY(QString backupsDirPath READ backupsDirPath NOTIFY osStoragePathChanged)
    Q_PROPERTY(QVariantList backupArchives READ backupArchives NOTIFY backupArchivesChanged)
    Q_PROPERTY(QStringList diffExcludes READ diffExcludes NOTIFY diffExcludesChanged)
    Q_PROPERTY(bool diffLoading READ diffLoading NOTIFY workspaceDiffChanged)
    Q_PROPERTY(QVariantList diffEntries READ diffEntries NOTIFY workspaceDiffChanged)
    Q_PROPERTY(QVariantMap diffSummary READ diffSummary NOTIFY workspaceDiffChanged)
    Q_PROPERTY(QString selectedDiffText READ selectedDiffText NOTIFY selectedDiffChanged)
    Q_PROPERTY(QString selectedDiffInfo READ selectedDiffInfo NOTIFY selectedDiffChanged)
    Q_PROPERTY(QVariantList secretsList READ secretsList NOTIFY secretsChanged)
    Q_PROPERTY(QString secretsError READ secretsError NOTIFY secretsChanged)
    Q_PROPERTY(QString secretsStorePath READ secretsStorePath CONSTANT)
    Q_PROPERTY(QVariantList environmentSecrets READ environmentSecretsList NOTIFY environmentSecretsChanged)
    Q_PROPERTY(QString environmentSecretsError READ environmentSecretsError NOTIFY environmentSecretsChanged)
    Q_PROPERTY(QString environmentSecretsEnvName READ environmentSecretsEnvName NOTIFY environmentSecretsChanged)
    Q_PROPERTY(QVariantMap poolInfo READ poolInfo NOTIFY poolInfoChanged)

public:
    enum class PathMode {
        WorkspaceDir,
        ProjectDir
    };

    explicit Backend(const QString &workspaceDir = {}, QObject *parent = nullptr);
    explicit Backend(const QString &path, PathMode mode, QObject *parent = nullptr);
    ~Backend() override;

    static QString resolveProjectAiboxDir(const QString &projectDir = {}, const QString &baseCwd = {});
    static QString defaultSystemPath();
    static QString defaultLocationFilePath();
    static QString configuredSystemPath();
    static bool saveLocationFile(const QString &systemPath);
    bool refreshSync(QString *errorOut = nullptr);
    bool isTaskLockActive(qint64 *outPid = nullptr) const;
    bool acquireCliTaskLock(const QString &description, const QString &envName);
    void releaseCliTaskLock();
    void recordHistory(const QString &description, const QString &envName, const QString &status,
                       int exitCode, const QString &startedAtIso, const QByteArray &logBytes);
    QVariantMap findEnvironment(const QString &name) const { return environmentByName(name); }
    QString workspacePathForEnv(const QString &name) const { return workspacePathFor(name); }
    QString containerWorkspacePath() const { return QStringLiteral("/home/workspace"); }
    QString containerWorkspacePathFor(const QString &name) const;
    QString stateDir() const { return m_stateDir; }
    QStringList diffExcludesList() const { return m_diffExcludes; }
    QString prepareEnvironmentScript(const QString &name);
    QString startEnvironmentScript(const QString &name);
    QString stopEnvironmentScript(const QString &name, bool includeAutoSnapshot = false);
    QStringList enterIncusExecArgs(const QString &name, const QString &remoteEnvPath = {}) const;
    QStringList execIncusCommandArgs(const QString &name, const QStringList &cmd,
                                     const QString &remoteEnvPath = {}) const;
    bool pushEnvironmentSecretsToContainer(const QString &envName, QString *outRemoteEnvPath,
                                           QString *errorOut = nullptr) const;
    QString aiExecCommandText(const QString &name) const;
    QStringList allSecretValues() const;
    QList<QPair<QString, QString>> resolveEnvironmentSecrets(const QString &envName,
                                                             QString *errorOut = nullptr) const;
    void setSecretsStorePathForTest(const QString &path);
    Q_INVOKABLE QString environmentSecretsStorePath(const QString &envName) const;

    int count() const { return m_count; }
    QVariantList environments() const { return m_environments; }
    QVariantList osCatalog() const { return m_osCatalog; }
    int selectedIndex() const { return m_selectedIndex; }
    void setSelectedIndex(int index);
    QVariantMap selectedEnvironment() const;
    bool busy() const { return m_busy; }
    int taskProgress() const { return m_taskProgress; }
    QString taskStage() const { return m_taskStage; }
    QString taskLog() const { return m_taskLog; }
    QString pendingEnvName() const { return m_pendingEnvName; }
    QString statusMessage() const { return m_statusMessage; }
    QString errorMessage() const { return m_errorMessage; }
    QString downloadingOsId() const { return m_downloadingOsId; }
    QString systemPath() const { return m_systemPath; }
    QString workspaceBase() const { return m_workspaceBase; }
    QString osStoragePath() const { return m_osStoragePath; }
    QString configFilePath() const { return m_configPath; }
    bool firstRun() const { return m_firstRun; }
    QVariantList presets() const { return m_presets; }
    QStringList diskOptions() const { return m_diskOptions; }
    QStringList ramOptions() const { return m_ramOptions; }
    QVariantList cpuOptions() const { return m_cpuOptions; }
    QString defaultPresetId() const { return m_defaultPresetId; }
    QString defaultOsId() const { return m_defaultOsId; }
    QString defaultDisk() const { return m_defaultDisk; }
    QString defaultRam() const { return m_defaultRam; }
    int defaultCpu() const { return m_defaultCpu; }
    bool autoSnapshotEnabled() const { return m_autoSnapshotEnabled; }
    int autoSnapshotKeep() const { return m_autoSnapshotKeep; }
    QVariantList templates() const;
    QStringList processLimitOptions() const { return m_processLimitOptions; }
    QStringList ioLimitOptions() const { return m_ioLimitOptions; }
    QString defaultProcessLimit() const { return m_defaultProcessLimit; }
    QString defaultIoLimit() const { return m_defaultIoLimit; }
    bool watchdogEnabled() const { return m_watchdogEnabled; }
    int watchdogDiskWarn() const { return m_watchdogDiskWarn; }
    int watchdogDiskCritical() const { return m_watchdogDiskCritical; }
    bool watchdogAutoStop() const { return m_watchdogAutoStop; }
    QVariantList operationHistory() const { return m_operationHistory; }
    int recentFailedHistoryCount() const;
    QVariantList egressLogs() const { return m_egressLogs; }
    QString egressLogsEnvName() const { return m_egressLogsEnvName; }
    QString backupsDirPath() const;
    QVariantList backupArchives() const { return m_backupArchives; }
    QStringList diffExcludes() const { return m_diffExcludes; }
    bool diffLoading() const { return m_diffLoading; }
    QVariantList diffEntries() const { return m_diffEntries; }
    QVariantMap diffSummary() const { return m_diffSummary; }
    QString selectedDiffText() const { return m_selectedDiffText; }
    QString selectedDiffInfo() const { return m_selectedDiffInfo; }
    QVariantList secretsList() const;
    QString secretsError() const { return m_secretsError; }
    QString secretsStorePath() const { return m_secretsPath; }
    QVariantList environmentSecretsList() const { return m_environmentSecrets; }
    QString environmentSecretsError() const { return m_environmentSecretsError; }
    QString environmentSecretsEnvName() const { return m_environmentSecretsEnv; }
    int idleStopMinutes() const { return m_idleStopMinutes; }
    QVariantList idleStopOptions() const { return {0, 15, 30, 60, 120}; }

    static QString defaultWorkspaceBase() { return defaultWorkspacePath(); }
    static QString defaultOsStoragePath() { return defaultOsStoragePathValue(); }

    Q_INVOKABLE void primaryAction();
    Q_INVOKABLE void refresh();
    Q_INVOKABLE void clearError();
    Q_INVOKABLE void cancelTask();

    Q_INVOKABLE void createEnvironment(const QString &name, const QString &osId, const QString &preset,
                                       const QString &diskGiB, const QString &ramGiB, int cpuCores,
                                       const QString &processLimit = {}, const QString &ioLimit = {},
                                       const QString &workspaceParent = {});
    Q_INVOKABLE void enterEnvironment(const QString &name);
    Q_INVOKABLE void openWorkspaceFolder(const QString &name);
    Q_INVOKABLE QString enableFileAccess(const QString &name);
    Q_INVOKABLE void disableFileAccess(const QString &name);
    Q_INVOKABLE void openFileAccess(const QString &name);
    Q_INVOKABLE void toggleStartStop(const QString &name);
    Q_INVOKABLE void createSnapshot(const QString &name, const QString &snapshotName);
    Q_INVOKABLE void restoreSnapshot(const QString &name, const QString &snapshotName,
                                     const QString &mode = QStringLiteral("both"));
    Q_INVOKABLE void deleteSnapshot(const QString &name, const QString &snapshotName);
    Q_INVOKABLE void setEnvironmentWorkspaceSnapshot(const QString &name, bool enabled);
    Q_INVOKABLE void convertWorkspaceToSubvolume(const QString &name);
    Q_INVOKABLE void deletePreSubvolBackup(const QString &name);
    Q_INVOKABLE void loadWorkspaceDiff(const QString &name, const QString &snapshotName);
    Q_INVOKABLE void loadFileDiff(const QString &name, const QString &snapshotName,
                                  const QString &relativePath, const QString &status);
    Q_INVOKABLE QString restoreSingleFile(const QString &name, const QString &snapshotName,
                                          const QString &relativePath);
    Q_INVOKABLE QString saveDiffExcludes(const QString &rawExcludes);
    Q_INVOKABLE void wipeEnvironment(const QString &name, bool deleteWorkspaceFiles);
    Q_INVOKABLE void cloneEnvironment(const QString &sourceName, const QString &newName,
                                      const QString &diskGiB = {}, const QString &ramGiB = {},
                                      int cpuCores = 0, bool startAfter = false,
                                      const QString &processLimit = {}, const QString &ioLimit = {},
                                      const QString &workspaceParent = {});
    Q_INVOKABLE void setEnvironmentTemplate(const QString &name, bool isTemplate);
    Q_INVOKABLE void setEnvironmentAutoSnapshot(const QString &name, bool enabled);
    Q_INVOKABLE void setEnvironmentAutoStop(const QString &name, bool enabled);
    Q_INVOKABLE void setEnvironmentIdleStop(const QString &name, const QString &value,
                                            bool allowWithPortForwards = false);
    Q_INVOKABLE void setNetworkMode(const QString &name, const QString &mode, const QString &allowlist = {});
    Q_INVOKABLE void refreshNetworkAllowlist(const QString &name);
    Q_INVOKABLE void setEgressRecording(const QString &name, bool enabled);
    Q_INVOKABLE void refreshEgressLog(const QString &name);
    Q_INVOKABLE void addEgressDestinationToAllowlist(const QString &name, const QString &destinationIp);
    Q_INVOKABLE QString addPortForward(const QString &name, int hostPort, int guestPort);
    Q_INVOKABLE void removePortForward(const QString &name, const QString &deviceName);
    Q_INVOKABLE void openForwardedPort(int hostPort);
    Q_INVOKABLE QString addExtraMount(const QString &name, const QString &hostPath, const QString &mountName,
                                      bool readOnly, bool confirmReadWrite = false);
    Q_INVOKABLE void removeExtraMount(const QString &name, const QString &deviceName);
    Q_INVOKABLE QString validateExtraMountCandidate(const QString &rawPath) const;
    Q_INVOKABLE QString updateEnvironmentResources(const QString &name, int cpuCores, const QString &ramGiB,
                                                   const QString &diskGiB, const QString &processLimit,
                                                   const QString &ioLimit);
    Q_INVOKABLE void refreshBackups();
    Q_INVOKABLE QVariantMap checkExportSpace(const QString &name, bool includeWorkspace) const;
    Q_INVOKABLE QString exportEnvironment(const QString &name, bool includeWorkspace);
    Q_INVOKABLE QString importEnvironment(const QString &backupFile, const QString &newName,
                                          const QString &workspaceArchiveFile = {});
    Q_INVOKABLE void deleteBackup(const QString &backupFile, const QString &workspaceArchiveFile = {});
    Q_INVOKABLE QString readOperationLog(const QString &id) const;
    Q_INVOKABLE void clearOperationHistory();
    Q_INVOKABLE void acknowledgeOperationHistory();
    Q_INVOKABLE void copyAiCommand(const QString &name);
    Q_INVOKABLE QString pasteImageToWorkspace(const QString &name);
    Q_INVOKABLE bool copyText(const QString &text, const QString &label = QString());
    Q_INVOKABLE QString adminFixCommand(const QString &message) const;
    Q_INVOKABLE QString saveSecret(const QString &name, const QString &value);
    Q_INVOKABLE QString deleteSecret(const QString &name);
    Q_INVOKABLE bool copySecret(const QString &name);
    Q_INVOKABLE void reloadSecrets();
    Q_INVOKABLE void setEnvironmentSecrets(const QString &name, const QStringList &secretNames);
    Q_INVOKABLE void reloadEnvironmentSecrets(const QString &envName);
    Q_INVOKABLE QString saveEnvironmentSecret(const QString &envName, const QString &name,
                                              const QString &value);
    Q_INVOKABLE QString deleteEnvironmentSecret(const QString &envName, const QString &name);
    Q_INVOKABLE bool copyEnvironmentSecret(const QString &envName, const QString &name);
    Q_INVOKABLE QString setEnvironmentLocalSecretEnabled(const QString &envName, const QString &name,
                                                         bool enabled);
    Q_INVOKABLE QString setGlobalSecretInstanceVisible(const QString &name, bool visible);
    Q_INVOKABLE QString poolWarningForCreate(const QString &diskSpec) const;
    Q_INVOKABLE QString poolWarningForQuotaEdit(const QString &envName, const QString &diskSpec) const;
    Q_INVOKABLE QString validateStoragePoolResize(const QString &sizeSpec) const;
    Q_INVOKABLE QString resizeStoragePool(const QString &sizeSpec);
    Q_INVOKABLE QString saveStaleSettings(int days);
    Q_INVOKABLE void refreshStaleReport();
    Q_INVOKABLE QString exportAndWipeEnvironment(const QString &name, bool includeWorkspace = true,
                                                 bool deleteWorkspaceFiles = true);
    Q_INVOKABLE void openOrphanFolder(const QString &path);
    Q_INVOKABLE QString deleteOrphan(const QString &kind, const QString &path);
    Q_INVOKABLE QString deleteOrphanAsAdmin(const QString &kind, const QString &path);
    Q_INVOKABLE QString saveSessionRecordingSettings(bool enabled);
    Q_INVOKABLE void setEnvironmentRecordSessions(const QString &name, const QString &mode);
    Q_INVOKABLE void refreshRecordedSessions();
    Q_INVOKABLE QString readRecordedSessionLog(const QString &logPath) const;
    Q_INVOKABLE void replayRecordedSession(const QString &logPath, const QString &timingPath);
    Q_INVOKABLE QString deleteRecordedSession(const QString &logPath, const QString &timingPath);

    Q_INVOKABLE void downloadOsImage(const QString &osId);
    Q_INVOKABLE void removeOsImage(const QString &osId);

    // Configuration menu actions. Methods returning QString return an empty
    // string on success, or a human-readable validation/IO error for the UI.
    Q_INVOKABLE QString completeFirstRun(const QString &systemPath);
    Q_INVOKABLE QString saveSettings(const QString &workspacePath, const QString &osStoragePath);
    Q_INVOKABLE QString saveOsEntry(const QString &originalId, const QString &id, const QString &alias,
                                    const QString &name, const QString &badge, const QString &description);
    Q_INVOKABLE QString removeOsEntry(const QString &id);
    Q_INVOKABLE QString resetOsCatalog();
    Q_INVOKABLE QString saveCreateOptions(const QString &diskCsv, const QString &ramCsv, const QString &cpuCsv);
    Q_INVOKABLE QString savePreset(const QString &id, const QString &title, const QString &subtitle);
    Q_INVOKABLE QString resetCreateOptions();
    Q_INVOKABLE QString saveAutoSnapshotSettings(bool enabled, int keepCount);
    Q_INVOKABLE QString saveWatchdogSettings(bool enabled, int diskWarn, int diskCritical, bool autoStop);
    Q_INVOKABLE QString saveIdleStopSettings(int minutes);
    Q_INVOKABLE QUrl pathToUrl(const QString &path) const;
    Q_INVOKABLE QString localPath(const QUrl &url) const;

    static QString sanitizeName(const QString &rawName);
    // Resolves an optional custom shared-folder parent into a full workspace path.
    static QString resolveWorkspacePath(const QString &rawParent, const QString &envName,
                                        const QString &defaultBase, QString *error = nullptr);
    static QVariantList parseIncusListJson(const QByteArray &json, const QString &workspaceBase,
                                           QHash<QString, qint64> *cpuSnapshotNs = nullptr,
                                           qint64 elapsedWallNs = 0,
                                           bool defaultAutoSnapshot = true,
                                           bool defaultAutoStop = false,
                                           bool defaultRecordSessions = false,
                                           QHash<QString, QPair<qint64, qint64>> *netCounterSnapshot = nullptr);
    static QVariantList buildOsCatalog(const QByteArray &imagesJson, const QString &downloadingId = {});
    // Shell bootstrap used by createEnvironment(); exposed for the test suite.
    static QString createEnvironmentScript(const QString &name, const QString &localAlias, const QString &remoteImage,
                                           const QString &workspace, const QString &cacheBase,
                                           const QString &profileFlags, const QString &ramGiB,
                                           const QString &diskGiB, int cpuCores, bool x11,
                                           const QString &processLimit = {}, const QString &ioLimit = {},
                                           bool isVm = false);
    static QString cloneEnvironmentScript(const QString &sourceName, const QString &newName,
                                          const QString &workspace, const QString &diskGiB,
                                          const QString &ramGiB, int cpuCores, bool startAfter,
                                          const QString &netMode = {}, const QString &netAllowlist = {},
                                          const QStringList &resolvedCidrs = {},
                                          const QString &processLimit = {}, const QString &ioLimit = {},
                                          bool isVm = false);

    // Auto-snapshot & retention helpers (F1)
    static bool isAutoSnapshotName(const QString &snapshotName);
    static QDateTime parseAutoSnapshotTimestamp(const QString &snapshotName);
    static bool shouldCreateAutoSnapshot(const QStringList &snapshots, const QDateTime &now,
                                         int minIntervalSeconds = 300);
    static QStringList autoSnapshotsToPrune(const QStringList &snapshots, int keepCount);
    static QString autoSnapshotShellSnippet(const QString &name, const QString &newSnapName,
                                            const QStringList &pruneNames,
                                            const QString &workspacePath = {},
                                            bool includeWorkspace = false);

    // Workspace Btrfs snapshot helpers (S1)
    static QString storePastedImage(const QImage &image, const QString &workspacePath,
                                    QString *errorOut = nullptr);
    static QString workspaceSnapshotDir(const QString &workspacePath, const QString &envName);
    static bool isBtrfsFilesystem(const QString &path);
    static bool isBtrfsSubvolume(const QString &path);
    static QStringList listWorkspaceSnapshots(const QString &workspacePath, const QString &envName);
    static QStringList findOrphanWorkspaceSnapshots(const QStringList &incusSnapshots,
                                                    const QStringList &wsSnapshots);
    static QVariantMap parseDuBytesOutput(const QString &output);
    static QString workspaceCreateSnippet(const QString &workspacePath);
    static QString workspaceMountMigrationScript(const QString &envName);
    static QString workspaceSnapshotSnippet(const QString &workspacePath, const QString &snapDir,
                                            const QString &snapName);
    static QString workspaceDeleteSnapshotSnippet(const QString &snapPath, bool warnOnly = true);
    static QString workspaceRestoreSnippet(const QString &workspacePath, const QString &snapDir,
                                           const QString &snapName, qint64 safetyEpoch = 0);
    static QString convertWorkspaceToSubvolumeScript(const QString &envName, const QString &workspacePath);
    static QString createSnapshotScript(const QString &name, const QString &snapName,
                                        const QString &workspacePath, bool includeWorkspace);
    static QString restoreSnapshotScript(const QString &name, const QString &snapName,
                                         const QString &workspacePath, const QString &mode,
                                         bool includeWorkspace, qint64 safetyEpoch = 0);
    static QString deleteSnapshotScript(const QString &name, const QString &snapName,
                                        const QString &workspacePath, bool hasIncusSnap,
                                        bool hasWorkspaceSnap);
    static QString wipeEnvironmentScript(const QString &name, const QString &workspacePath,
                                         bool deleteWorkspaceFiles,
                                         const QString &sessionsEnvDir = {},
                                         const QString &environmentSecretsPath = {});

    // Workspace change review helpers (S2)
    static QStringList defaultDiffExcludes();
    static QStringList parseDiffExcludes(const QString &rawText);
    static QStringList buildRsyncExcludeArgs(const QStringList &excludes);
    static QStringList buildWorkspaceDiffArgs(const QString &snapPath, const QString &wsPath,
                                              const QStringList &excludes);
    static QVariantMap processWorkspaceDiffOutput(const QString &snapshotName, const QString &wsPath,
                                                  const QString &rsyncStdout, QVariantList *entriesOut);
    static QVariantMap parseRsyncItemize(const QString &output, int maxEntries = 5000);
    static bool isSafeRelativePath(const QString &relativePath);
    static bool isLikelyBinaryData(const QByteArray &sample);
    static QString capDiffLines(const QString &diffOutput, int maxLines = 2000, bool *outTruncated = nullptr);

    // Secrets injection helpers (S3)
    static QString defaultSecretsStorePath();
    static bool isValidSecretName(const QString &name);
    static QStringList parseEnvSecretNames(const QString &rawCsv);
    static QList<QPair<QString, QString>> loadSecretsStore(const QString &filePath, QString *errorOut = nullptr);
    static QStringList loadSecretNameList(const QString &filePath, const QString &key);
    static QString saveSecretsStore(const QString &filePath, const QList<QPair<QString, QString>> &secrets,
                                    const QStringList &disabledNames = {},
                                    const QStringList &hiddenInInstances = {});
    static QString buildSecretEnvFileContent(const QList<QPair<QString, QString>> &secrets);
    static QString redactSecrets(const QString &text, const QStringList &values);
    static QByteArray redactSecretsBytes(const QByteArray &data, const QStringList &values);
    static QString createHostTempSecretFile(const QString &content, QString *errorOut = nullptr);
    static bool isSafeEnvironmentFileName(const QString &name);
    QString instanceSecretsDirPath() const;
    bool copySecretValue(const QString &name, const QString &value);
    void updateEnvironmentSecretAssignments(const QString &envName, const QStringList &names);
    void clearGlobalSecretFromEnvironments(const QString &name);
    void refreshInstanceSecretValuesCache();

    // Network ACL & egress helpers (F3 & G3)
    using HostResolver = std::function<QStringList(const QString &host)>;
    static QStringList parseAllowlistEntries(const QString &rawText);
    static QStringList resolveAllowlistCidrs(const QStringList &entries, const HostResolver &resolver = {},
                                             QStringList *warnings = nullptr);
    static QString networkControlScript(const QString &name, const QString &mode,
                                        const QString &allowlistRaw, const QStringList &resolvedCidrs,
                                        bool egressLogging = false);
    static QVariantList parseKernelEgressLogs(const QString &journalOutput, const QString &containerIp,
                                              const QString &hostIface, int maxEntries = 100);

    // Watchdog & snapshot size helpers (G1 & G2)
    struct WatchdogDecision {
        int newLevel = 0;
        bool shouldNotify = false;
        bool shouldAutoStop = false;
    };
    static WatchdogDecision evaluateWatchdog(int percent, int previousLevel, int warnThreshold,
                                             int criticalThreshold, bool autoStopEnabled);
    static QVariantMap parseSnapshotSizesJson(const QByteArray &json);
    static bool isValidProcessLimit(const QString &value);
    static bool isValidIoLimit(const QString &value);

    // Idle auto-stop helpers (H1)
    struct IdleStopOverride {
        enum Mode {
            Inherit,
            Off,
            Minutes
        };
        Mode mode = Inherit;
        int minutes = 0;
    };
    struct IdleCpuSample {
        qint64 timestampMs = 0;
        int cpuPercent = 0;
    };
    struct IdleDecisionInput {
        bool running = false;
        bool isTemplate = false;
        bool busyOnThisEnv = false;
        bool hasActiveExecSession = false;
        int portForwardCount = 0;
        bool ignorePortForwards = false;
        int thresholdMinutes = 0;
        qint64 nowMs = 0;
        bool warningAlreadySent = false;
        QList<IdleCpuSample> samples;
    };
    struct IdleDecisionResult {
        bool eligible = false;
        qint64 idleDurationMs = 0;
        double averageCpuPercent = 0.0;
        bool shouldWarnOneMin = false;
        bool shouldStopNow = false;
        QString blockedReason;
    };
    static IdleStopOverride parseIdleStopOverride(const QString &raw);
    static int effectiveIdleStopMinutes(int globalMinutes, const IdleStopOverride &ov);
    static IdleDecisionResult evaluateIdleDecision(const IdleDecisionInput &input);
    static QSet<QString> parseActiveIncusExecEnvs(const QString &pgrepOutput);

    // Operation history helpers (G4)
    static QByteArray truncateLogTail(const QByteArray &data, qint64 maxBytes = 204800);

    // Daily workflow helpers (W1 - W4)
    static QString proxyDeviceName(int hostPort);
    static bool isHostPortAvailable(int hostPort);
    static QString fileAccessUrl(int port);
    static int chooseFileAccessPort(const QSet<int> &reservedPorts,
                                    const std::function<bool(int)> &isAvailable,
                                    int startPort = 2222, int maxPort = 2299);
    static QString addPortForwardScript(const QString &name, int hostPort, int guestPort);
    static QString validateExtraMountPath(const QString &rawPath, const QStringList &forbiddenConfigPaths = {});
    static QString extraMountDeviceName(const QString &rawName);
    static QString addExtraMountScript(const QString &name, const QString &deviceName,
                                       const QString &hostPath, const QString &containerPath, bool readOnly);
    static qint64 parseByteSizeToBytes(const QString &text);
    static QString updateResourcesScript(const QString &name, int cpuCores, const QString &ramGiB,
                                         const QString &diskGiB, const QString &processLimit,
                                         const QString &ioLimit, bool isVm);

    // Resilience: Export / Import & Health Check Helpers (R1, R2)
    static bool hasSufficientDiskSpace(qint64 requiredBytes, qint64 availableBytes);
    static qint64 directorySizeBytes(const QString &dirPath);
    static qint64 availableDiskBytes(const QString &path);
    static QVariantList scanBackupArchives(const QString &backupsDir);
    static QString exportEnvironmentScript(const QString &name, const QString &backupFile,
                                           const QString &workspacePath = {},
                                           const QString &workspaceArchiveFile = {});
    static QString importEnvironmentScript(const QString &backupFile, const QString &targetName,
                                           const QString &workspacePath,
                                           const QString &workspaceArchiveFile = {});

    struct StoragePoolInfo {
        bool valid = false;
        QString name = QStringLiteral("default");
        QString driver;
        qint64 usedBytes = 0;
        qint64 totalBytes = 0;
        int usedPercent = 0;
        // Loop-file backed pools expose a growable "size" (e.g. "30GiB").
        // Directory/subvolume backed pools leave this empty and follow the host filesystem.
        QString sizeText;
        bool resizable = false;
    };
    static StoragePoolInfo parseIncusStorageInfo(const QString &output);
    static qint64 sumRootDiskQuotasBytes(const QByteArray &incusListJson,
                                         const QString &excludeEnvName = {},
                                         const QString &poolName = QStringLiteral("default"),
                                         int *unboundedCountOut = nullptr);
    static QString evaluatePoolOvercommitWarning(const StoragePoolInfo &pool,
                                                 qint64 existingQuotasBytes,
                                                 qint64 requestedBytes,
                                                 int unboundedCount = 0);
    // Grow-only pool resize helpers. Plain numbers in `spec` mean GiB.
    static bool parsePoolSizeSpec(const QString &spec, qint64 *outBytes);
    static QString resizeStoragePoolScript(const QString &poolName, const QString &sizeSpec);

    struct CommandOutput {
        int exitCode = -1;
        QString stdOut;
        QString stdErr;
    };
    using CommandRunner = std::function<CommandOutput(const QString &program, const QStringList &args)>;
    struct HealthCheckResult {
        QString id;
        QString title;
        bool ok = false;
        bool warning = false;
        bool repaired = false;
        bool needsRoot = false;
        QString detail;
        QString fixCommand;
    };
    static HealthCheckResult evaluatePoolCapacityHealth(const StoragePoolInfo &pool,
                                                        qint64 allocatedQuotasBytes = 0,
                                                        int unboundedCount = 0);
    static QList<HealthCheckResult> runHealthChecks(bool fix = false, const CommandRunner &runner = {},
                                                    const QString &targetUser = {},
                                                    const QString &ufwRulesContent = {},
                                                    const QString &workspaceBase = {});

    int staleDays() const { return m_staleDays; }
    QVariantList staleDayOptions() const;
    QVariantMap staleReport() const { return m_staleReport; }

    enum class StalenessClass {
        Fresh,
        Stale,
        Unknown,
        Template
    };
    struct StalenessResult {
        StalenessClass classification = StalenessClass::Unknown;
        qint64 daysUnused = -1;
        QString statusKey = QStringLiteral("unknown");
    };
    static StalenessResult classifyEnvironmentStaleness(qint64 lastUsedEpoch, bool isTemplate,
                                                        int staleDays, qint64 nowEpoch = 0);
    static QVariantList detectOrphans(const QVariantList &environments, const QString &workspaceBase,
                                      const QString &backupsDir);
    static QVariantMap buildStaleReport(const QVariantList &environments, const QString &workspaceBase,
                                        const QString &backupsDir, int staleDays, qint64 nowEpoch = 0);
    static QString exportAndWipeEnvironmentScript(const QString &name, const QString &backupFile,
                                                  const QString &workspacePath,
                                                  const QString &workspaceArchiveFile,
                                                  bool deleteWorkspaceFiles,
                                                  const QString &sessionsEnvDir = {},
                                                  const QString &environmentSecretsPath = {});
    static QString deleteOrphanScript(const QString &kind, const QString &targetPath);
    QString validateOrphanPath(const QString &kind, const QString &cleanPath) const;
    static QString adminDeleteScriptText(const QString &kind, const QString &targetPath);

    bool recordSessions() const { return m_recordSessions; }
    QVariantList recordedSessions() const { return m_recordedSessions; }
    QString sessionsDirPath() const;
    bool prepareRecordedSessionPaths(const QString &envName, QString *outLogFile, QString *outTimingFile) const;
    static bool effectiveRecordSessions(bool globalEnabled, const QString &envOverride);
    static QString buildSessionWrapCommand(const QString &innerCommand, const QString &logFile,
                                           const QString &timingFile,
                                           const QString &redactBinPath = {},
                                           const QString &sessionsDir = {},
                                           bool scriptAvailable = true);
    static QString stripAnsiSequences(const QString &rawText);
    static QString formatRecordedSessionText(const QByteArray &rawBytes, int maxBytes = 200 * 1024);
    static bool redactSessionLogFile(const QString &logFilePath, const QStringList &secretValues);
    static int pruneRecordedSessions(const QString &sessionsDir, int maxPerEnv = 20,
                                     qint64 maxTotalBytes = 200LL * 1024LL * 1024LL);
    static QVariantList scanRecordedSessions(const QString &sessionsDir);

    QVariantMap poolInfo() const { return m_poolInfo; }
    void setPoolStateForTest(const StoragePoolInfo &pool, const QByteArray &incusListJson);

signals:
    void countChanged();
    void environmentsChanged();
    void osCatalogChanged();
    void selectedIndexChanged();
    void busyChanged();
    void taskProgressChanged();
    void statusMessageChanged();
    void errorMessageChanged();
    void systemPathChanged();
    void workspaceBaseChanged();
    void osStoragePathChanged();
    void configFilePathChanged();
    void firstRunChanged();
    void createOptionsChanged();
    void autoSnapshotSettingsChanged();
    void watchdogSettingsChanged();
    void idleStopSettingsChanged();
    void staleReportSettingsChanged();
    void staleReportChanged();
    void recordSessionsSettingsChanged();
    void recordedSessionsChanged();
    void operationHistoryChanged();
    void egressLogsChanged();
    void backupArchivesChanged();
    void diffExcludesChanged();
    void workspaceDiffChanged();
    void selectedDiffChanged();
    void secretsChanged();
    void environmentSecretsChanged();
    void poolInfoChanged();
    void environmentCreated(const QString &name);

private:
    void setBusy(bool busy, const QString &status = {});
    void setError(const QString &error);
    void handleProcessOutput(const QByteArray &chunk);
    void runShellTask(const QString &description, const QString &script, const QString &createdEnvName = {},
                      const QString &targetEnvName = {}, const QString &reason = {});
    void checkBackgroundTask();
    void attachExistingTask();
    qint64 readTaskPid() const;
    void clearTaskFiles();
    void loadDemoData();

    QString historyDirPath() const;
    QString historySeenMarkerPath() const;
    void loadHistorySeenMarker();
    void loadOperationHistory();
    void recordOperationHistory(const QString &description, const QString &envName, const QString &status,
                                int exitCode, const QString &startedAtIso, const QByteArray &logBytes,
                                const QString &reason = {});
    void evaluateAllWatchdogs();
    void evaluateIdleAutoStops();
    void applyCachedSnapshotSizes(QVariantList &envs) const;
    void refreshSelectedSnapshotSizes();

    void loadConfig();
    bool saveConfig() const;
    void rebuildOsCatalog();
    void normalizeDefaults();
    QString normalizedPath(const QString &raw, const QString &fallback) const;
    QString workspacePathFor(const QString &name) const;
    QString cacheBasePath(const QString &osId) const;
    QVariantMap presetById(const QString &id) const;
    QVariantMap osEntryById(const QString &id) const;

    static QString defaultWorkspacePath();
    static QString defaultOsStoragePathValue();
    static QVariantList defaultOsEntries();
    static QVariantList defaultPresets();
    static QStringList defaultDiskOptions();
    static QStringList defaultRamOptions();
    static QVariantList defaultCpuOptions();
    static QVariantList catalogForEntries(const QVariantList &entries, const QByteArray &imagesJson,
                                          const QString &downloadingId, const QString &storagePath);
    static QStringList parseCsv(const QString &text);
    static QString shellQuote(const QString &value);
    static QString shellArrayWords(const QString &words);

    QVariantMap environmentByName(const QString &name) const;
    QString buildAutoSnapshotIfNeeded(const QString &name);
    QString buildAllowlistRefreshIfNeeded(const QString &name) const;
    QString buildPrepareScript(const QString &name, const QString &allowRefresh);
    void beginEnterPrepare(const QString &name);
    void resolveEnterAllowlistHosts(const QStringList &entries,
                                    const std::function<void(const QStringList &)> &done);
    void startEnterPrepareProcess();
    void handleEnterPrepared(int exitCode);
    void pushEnterSecrets();
    void handleEnterSecretsPushed(int exitCode, QProcess::ExitStatus status);
    QString buildEnterCommand(const QString &name, const QString &remoteEnvPath) const;
    void launchEnterTerminal(const QString &remoteEnvPath);
    void finishEnter(const QString &errorMessage);
    QString fileAccessDirPath() const;
    QString fileAccessStatePath(const QString &name) const;
    QString fileAccessLogPath(const QString &name) const;
    QVariantMap loadFileAccessState(const QString &name) const;
    bool fileAccessProcessAlive(qint64 pid, const QString &name) const;
    void stopFileAccessProcess(const QString &name);
    void applyFileAccessState(QVariantList &environments);

    int m_count = 0;
    QVariantList m_environments;
    QVariantList m_osCatalog;
    int m_selectedIndex = 0;
    bool m_busy = false;
    int m_taskProgress = 0;
    QString m_taskStage;
    QString m_taskLog;
    qint64 m_logOffset = 0;
    int m_lastExitCode = -1;
    bool m_hasExitCode = false;
    QString m_pendingEnvName;
    QString m_statusMessage;
    QString m_errorMessage;
    QString m_downloadingOsId;
    PathMode m_pathMode = PathMode::ProjectDir;
    bool m_rememberSystemLocation = false;
    QString m_systemPath;
    QString m_workspaceBase;
    QString m_osStoragePath;
    QString m_defaultWorkspaceBase;
    QString m_defaultOsStoragePath;
    QString m_stateDir;
    QString m_configPath;
    bool m_firstRun = false;
    QVariantList m_osEntries;
    QVariantList m_presets;
    QStringList m_diskOptions;
    QStringList m_ramOptions;
    QVariantList m_cpuOptions;
    QString m_defaultPresetId;
    QString m_defaultOsId;
    QString m_defaultDisk;
    QString m_defaultRam;
    int m_defaultCpu = 4;
    bool m_autoSnapshotEnabled = true;
    int m_autoSnapshotKeep = 5;
    QStringList m_processLimitOptions;
    QStringList m_ioLimitOptions;
    QString m_defaultProcessLimit;
    QString m_defaultIoLimit;
    bool m_watchdogEnabled = true;
    int m_watchdogDiskWarn = 90;
    int m_watchdogDiskCritical = 97;
    int m_watchdogCpuWarn = 95;
    int m_watchdogRamWarn = 92;
    bool m_watchdogAutoStop = false;
    int m_idleStopMinutes = 0;
    int m_staleDays = 30;
    QVariantMap m_staleReport;
    bool m_recordSessions = false;
    QVariantList m_recordedSessions;
    QHash<QString, QList<IdleCpuSample>> m_idleSamples;
    QSet<QString> m_idleWarned;
    QHash<QString, int> m_watchdogDiskLevel;
    QHash<QString, int> m_watchdogCpuLevel;
    QHash<QString, int> m_watchdogCpuHighStreak;
    QHash<QString, int> m_watchdogRamLevel;
    QHash<QString, int> m_watchdogRamHighStreak;
    QHash<QString, QVariantMap> m_snapshotSizesCache;
    QHash<QString, QStringList> m_snapshotSizeKeyCache;
    QProcess *m_snapshotSizeProcess = nullptr;
    QHash<QString, QVariantMap> m_wsSnapshotSizesCache;
    QHash<QString, QStringList> m_wsSnapshotSizeKeyCache;
    QProcess *m_wsSnapshotSizeProcess = nullptr;
    QVariantList m_operationHistory;
    QString m_historySeenId;
    int m_historyMaxEntries = 50;
    QVariantList m_egressLogs;
    QString m_egressLogsEnvName;
    QHash<QString, QDateTime> m_lastAutoSnapAttempt;
    QByteArray m_lastImagesJson;
    QHash<QString, qint64> m_lastCpuUsageNs;
    QElapsedTimer m_cpuSampleTimer;
    QProcess *m_refreshProcess = nullptr;
    QTimer m_pollTimer;
    QTimer m_taskWatchTimer;
    QVariantList m_backupArchives;
    QStringList m_diffExcludes;
    bool m_diffLoading = false;
    QVariantList m_diffEntries;
    QVariantMap m_diffSummary;
    QString m_selectedDiffText;
    QString m_selectedDiffInfo;
    QProcess *m_diffProcess = nullptr;
    QTimer m_diffTimeoutTimer;
    QProcess *m_fileDiffProcess = nullptr;
    QString m_secretsPath;
    QList<QPair<QString, QString>> m_secrets;
    QString m_secretsError;
    QStringList m_hiddenGlobalSecrets;
    QVariantList m_environmentSecrets;
    QString m_environmentSecretsError;
    QString m_environmentSecretsEnv;
    QStringList m_instanceSecretValues;
    QVariantMap m_poolInfo;
    StoragePoolInfo m_lastPoolInfo;
    qint64 m_lastPoolAllocatedBytes = 0;
    int m_lastPoolUnboundedCount = 0;
    QByteArray m_lastListJson;
    bool m_demoMode = false;
    QHash<QString, QPair<qint64, qint64>> m_lastNetCounters;
    QProcess *m_enterPrepareProcess = nullptr;
    QProcess *m_enterSecretProcess = nullptr;
    QString m_enterEnvName;
    QString m_enterAllowlistSnippet;
    QString m_enterHostSecretFile;
    QString m_enterRemoteEnvPath;
};
