#include "backend.h"
#include "backend_helpers.h"

#include <QDateTime>

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
