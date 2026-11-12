#pragma once

#include <QObject>
#include <QProcess>
#include <QTimer>
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
    Q_PROPERTY(QString workspaceBase READ workspaceBase CONSTANT)

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
    QString m_stateDir;
    QProcess *m_refreshProcess = nullptr;
    QTimer m_pollTimer;
    QTimer m_taskWatchTimer;
    bool m_demoMode = false;
};
