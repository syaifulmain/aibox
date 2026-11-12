#pragma once

#include <QObject>
#include <QProcess>
#include <QStringList>
#include <QTimer>
#include <QUrl>
#include <QVariantList>
#include <QVariantMap>

class Backend : public QObject {
    Q_OBJECT
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
    Q_PROPERTY(QString workspaceBase READ workspaceBase NOTIFY workspaceBaseChanged)
    Q_PROPERTY(QString osStoragePath READ osStoragePath NOTIFY osStoragePathChanged)
    Q_PROPERTY(QString defaultWorkspaceBase READ defaultWorkspaceBase CONSTANT)
    Q_PROPERTY(QString defaultOsStoragePath READ defaultOsStoragePath CONSTANT)
    Q_PROPERTY(QString configFilePath READ configFilePath CONSTANT)
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

public:
    explicit Backend(const QString &workspaceDir = {}, QObject *parent = nullptr);
    ~Backend() override;

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

    static QString defaultWorkspaceBase() { return defaultWorkspacePath(); }
    static QString defaultOsStoragePath() { return defaultOsStoragePathValue(); }

    Q_INVOKABLE void primaryAction();
    Q_INVOKABLE void refresh();
    Q_INVOKABLE void clearError();
    Q_INVOKABLE void cancelTask();

    Q_INVOKABLE void createEnvironment(const QString &name, const QString &osId, const QString &preset,
                                       const QString &diskGiB, const QString &ramGiB, int cpuCores);
    Q_INVOKABLE void enterEnvironment(const QString &name);
    Q_INVOKABLE void openWorkspaceFolder(const QString &name);
    Q_INVOKABLE void toggleStartStop(const QString &name);
    Q_INVOKABLE void createSnapshot(const QString &name, const QString &snapshotName);
    Q_INVOKABLE void restoreSnapshot(const QString &name, const QString &snapshotName);
    Q_INVOKABLE void deleteSnapshot(const QString &name, const QString &snapshotName);
    Q_INVOKABLE void wipeEnvironment(const QString &name, bool deleteWorkspaceFiles);
    Q_INVOKABLE void runHeadlessCommand(const QString &name, const QString &command);
    Q_INVOKABLE void copyAiCommand(const QString &name);

    Q_INVOKABLE void downloadOsImage(const QString &osId);
    Q_INVOKABLE void removeOsImage(const QString &osId);

    // Configuration menu actions. Methods returning QString return an empty
    // string on success, or a human-readable validation/IO error for the UI.
    Q_INVOKABLE QString completeFirstRun(const QString &workspacePath);
    Q_INVOKABLE QString saveSettings(const QString &workspacePath, const QString &osStoragePath);
    Q_INVOKABLE QString saveOsEntry(const QString &originalId, const QString &id, const QString &alias,
                                    const QString &name, const QString &badge, const QString &description);
    Q_INVOKABLE QString removeOsEntry(const QString &id);
    Q_INVOKABLE QString resetOsCatalog();
    Q_INVOKABLE QString saveCreateOptions(const QString &diskCsv, const QString &ramCsv, const QString &cpuCsv);
    Q_INVOKABLE QString savePreset(const QString &id, const QString &title, const QString &subtitle);
    Q_INVOKABLE QString resetCreateOptions();
    Q_INVOKABLE QUrl pathToUrl(const QString &path) const;
    Q_INVOKABLE QString localPath(const QUrl &url) const;

    static QString sanitizeName(const QString &rawName);
    static QVariantList parseIncusListJson(const QByteArray &json, const QString &workspaceBase);
    static QVariantList buildOsCatalog(const QByteArray &imagesJson, const QString &downloadingId = {});

signals:
    void countChanged();
    void environmentsChanged();
    void osCatalogChanged();
    void selectedIndexChanged();
    void busyChanged();
    void taskProgressChanged();
    void statusMessageChanged();
    void errorMessageChanged();
    void workspaceBaseChanged();
    void osStoragePathChanged();
    void firstRunChanged();
    void createOptionsChanged();
    void environmentCreated(const QString &name);

private:
    void setBusy(bool busy, const QString &status = {});
    void setError(const QString &error);
    void handleProcessOutput(const QByteArray &chunk);
    void runShellTask(const QString &description, const QString &script, const QString &createdEnvName = {});
    void checkBackgroundTask();
    void attachExistingTask();
    qint64 readTaskPid() const;
    void clearTaskFiles();
    void loadDemoData();

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
    QString m_workspaceBase;
    QString m_osStoragePath;
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
    QByteArray m_lastImagesJson;
    QProcess *m_refreshProcess = nullptr;
    QTimer m_pollTimer;
    QTimer m_taskWatchTimer;
    bool m_demoMode = false;
};
