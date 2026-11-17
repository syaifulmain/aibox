#include "backend.h"
#include "backend_helpers.h"

#include <QClipboard>
#include <QCoreApplication>
#include <QDateTime>
#include <QDesktopServices>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QHostAddress>
#include <QHostInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QRegularExpression>
#include <QStandardPaths>
#include <QThread>
#include <QUrl>
#include <QUuid>

#include <cmath>

namespace {

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

QString formatPercent(int percent) {
    return QStringLiteral("%1%").arg(qBound(0, percent, 100));
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

} // namespace

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

void Backend::createEnvironment(const QString &rawName, const QString &osId, const QString &presetId,
                                const QString &diskGiB, const QString &ramGiB, int cpuCores,
                                const QString &processLimit, const QString &ioLimit,
                                const QStringList &provisionBundleIds, const QString &workspaceParent) {
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
    QString workspaceError;
    const QString hostWorkspace = resolveWorkspacePath(workspaceParent, name, m_workspaceBase, &workspaceError);
    if (hostWorkspace.isEmpty()) {
        setError(workspaceError);
        return;
    }
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
    return buildPrepareScript(name, buildAllowlistRefreshIfNeeded(name));
}

QString Backend::buildPrepareScript(const QString &name, const QString &allowRefresh) {
    const QString autoSnap = buildAutoSnapshotIfNeeded(name);

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

void Backend::enterEnvironment(const QString &name) {
    if (name.isEmpty())
        return;
    const QVariantMap env = environmentByName(name);
    if (env.value("isTemplate").toBool()) {
        setError(tr("Templates cannot be entered directly. Create a new environment from this template first."));
        return;
    }
    if (m_enterPrepareProcess && m_enterPrepareProcess->state() != QProcess::NotRunning)
        return;
    if (m_enterSecretProcess && m_enterSecretProcess->state() != QProcess::NotRunning)
        return;

    m_enterEnvName = name;
    m_enterAllowlistSnippet.clear();
    m_enterHostSecretFile.clear();
    m_enterRemoteEnvPath.clear();
    if (m_statusMessage != tr("Preparing environment '%1'...").arg(name)) {
        m_statusMessage = tr("Preparing environment '%1'...").arg(name);
        emit statusMessageChanged();
    }
    beginEnterPrepare(name);
}

void Backend::beginEnterPrepare(const QString &name) {
    const QVariantMap env = environmentByName(name);
    const QString rawAllow = env.value(QStringLiteral("networkAllowlist")).toString();
    const QStringList entries = parseAllowlistEntries(rawAllow);
    if (env.value(QStringLiteral("networkMode")).toString() != QLatin1String("allowlist") || entries.isEmpty()) {
        startEnterPrepareProcess();
        return;
    }

    // Hostname lookups must not block the GUI thread, so resolve them first.
    resolveEnterAllowlistHosts(entries, [this, rawAllow](const QStringList &cidrs) {
        if (m_enterEnvName.isEmpty())
            return;
        const QVariantMap current = environmentByName(m_enterEnvName);
        if (!cidrs.isEmpty()) {
            m_enterAllowlistSnippet = aclApplyShellSnippet(m_enterEnvName, rawAllow, cidrs,
                                                           current.value("egressRecording").toBool());
        }
        startEnterPrepareProcess();
    });
}

void Backend::resolveEnterAllowlistHosts(const QStringList &entries,
                                         const std::function<void(const QStringList &)> &done) {
    static const QRegularExpression hostRe(QStringLiteral("^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)*$"));
    QStringList hostnames;
    for (const QString &entry : entries) {
        if (entry.contains(QLatin1Char('/')) || !QHostAddress(entry).isNull())
            continue;
        if (hostRe.match(entry).hasMatch() && !hostnames.contains(entry))
            hostnames.append(entry);
    }

    auto results = std::make_shared<QHash<QString, QStringList>>();
    auto finish = [entries, results, done]() {
        done(resolveAllowlistCidrs(entries, [results](const QString &host) {
            return results->value(host);
        }));
    };
    if (hostnames.isEmpty()) {
        finish();
        return;
    }

    auto pending = std::make_shared<int>(hostnames.size());
    for (const QString &host : hostnames) {
        QHostInfo::lookupHost(host, this,
                              [host, pending, results, finish](const QHostInfo &info) {
            QStringList ips;
            for (const QHostAddress &addr : info.addresses()) {
                if (addr.protocol() == QAbstractSocket::IPv4Protocol)
                    ips.append(addr.toString());
            }
            results->insert(host, ips);
            if (--(*pending) == 0)
                finish();
        });
    }
}

void Backend::startEnterPrepareProcess() {
    if (m_enterEnvName.isEmpty())
        return;
    if (!m_enterPrepareProcess) {
        m_enterPrepareProcess = new QProcess(this);
        m_enterPrepareProcess->setProcessChannelMode(QProcess::MergedChannels);
        connect(m_enterPrepareProcess, QOverload<int, QProcess::ExitStatus>::of(&QProcess::finished),
                this, [this](int exitCode, QProcess::ExitStatus) { handleEnterPrepared(exitCode); });
        connect(m_enterPrepareProcess, &QProcess::errorOccurred, this, [this](QProcess::ProcessError error) {
            if (error == QProcess::FailedToStart)
                finishEnter(tr("Could not start environment preparation."));
        });
    }
    if (m_enterPrepareProcess->state() != QProcess::NotRunning)
        return;
    m_enterPrepareProcess->start(QStringLiteral("/bin/bash"),
                                 {QStringLiteral("-c"), buildPrepareScript(m_enterEnvName, m_enterAllowlistSnippet)});
}

void Backend::handleEnterPrepared(int exitCode) {
    if (m_enterEnvName.isEmpty())
        return;
    const QByteArray preOut = m_enterPrepareProcess->readAllStandardOutput();
    if (preOut.contains("Warning:")) {
        recordOperationHistory(tr("Auto-snapshot warning for '%1'").arg(m_enterEnvName), m_enterEnvName,
                               QStringLiteral("warning"), exitCode,
                               QDateTime::currentDateTimeUtc().toString(Qt::ISODate), preOut);
    }
    pushEnterSecrets();
}

void Backend::pushEnterSecrets() {
    if (m_enterEnvName.isEmpty())
        return;
    QString secretErr;
    const QList<QPair<QString, QString>> pairs = resolveEnvironmentSecrets(m_enterEnvName, &secretErr);
    if (!secretErr.isEmpty()) {
        finishEnter(secretErr);
        return;
    }
    if (pairs.isEmpty()) {
        launchEnterTerminal(QString());
        return;
    }

    QString tmpErr;
    m_enterHostSecretFile = createHostTempSecretFile(buildSecretEnvFileContent(pairs), &tmpErr);
    if (m_enterHostSecretFile.isEmpty()) {
        finishEnter(tmpErr);
        return;
    }
    m_enterRemoteEnvPath = QStringLiteral("/dev/shm/aibox-%1.env")
                               .arg(QUuid::createUuid().toString(QUuid::Id128));

    if (!m_enterSecretProcess) {
        m_enterSecretProcess = new QProcess(this);
        connect(m_enterSecretProcess, QOverload<int, QProcess::ExitStatus>::of(&QProcess::finished),
                this, [this](int exitCode, QProcess::ExitStatus status) {
            handleEnterSecretsPushed(exitCode, status);
        });
    }
    m_enterSecretProcess->start(QStringLiteral("incus"),
                                {QStringLiteral("file"), QStringLiteral("push"),
                                 QStringLiteral("--mode"), QStringLiteral("0400"),
                                 m_enterHostSecretFile, m_enterEnvName + m_enterRemoteEnvPath});
}

void Backend::handleEnterSecretsPushed(int exitCode, QProcess::ExitStatus status) {
    QString errText;
    if (exitCode != 0 || status != QProcess::NormalExit) {
        errText = QString::fromUtf8(m_enterSecretProcess->readAllStandardError()).trimmed();
        if (errText.isEmpty())
            errText = tr("Failed to inject secrets into container.");
    }
    if (!m_enterHostSecretFile.isEmpty())
        QFile::remove(m_enterHostSecretFile);
    m_enterHostSecretFile.clear();
    if (!errText.isEmpty()) {
        finishEnter(errText);
        return;
    }
    launchEnterTerminal(m_enterRemoteEnvPath);
}

QString Backend::buildEnterCommand(const QString &name, const QString &remoteEnvPath) const {
    const QString wsMountArg = QStringLiteral(
        "\"$(incus config device get %1 workspace path 2>/dev/null || echo %2)\"")
        .arg(shellQuote(name), shellQuote(kContainerWorkspacePath));
    if (remoteEnvPath.isEmpty()) {
        return QStringLiteral(
            "incus exec %1 --cwd %2 --env DISPLAY=\"${DISPLAY:-:0}\" --env TERM=\"${TERM:-xterm-256color}\" -- bash -l || "
            "incus exec %1 --cwd %2 -- sh").arg(shellQuote(name), wsMountArg);
    }
    return QStringLiteral(
        "incus exec %1 --cwd %3 --env DISPLAY=\"${DISPLAY:-:0}\" --env TERM=\"${TERM:-xterm-256color}\" -- "
        "sh -c 'F=\"$1\"; if [ -f \"$F\" ]; then set -a; . \"$F\"; set +a; rm -f \"$F\"; fi; exec bash -l' aibox-enter %2 || "
        "incus exec %1 --cwd %3 -- "
        "sh -c 'F=\"$1\"; if [ -f \"$F\" ]; then set -a; . \"$F\"; set +a; rm -f \"$F\"; fi; exec sh' aibox-enter %2")
                   .arg(shellQuote(name), shellQuote(remoteEnvPath), wsMountArg);
}

void Backend::launchEnterTerminal(const QString &remoteEnvPath) {
    const QString name = m_enterEnvName;
    if (name.isEmpty())
        return;

    QString enterCmd = buildEnterCommand(name, remoteEnvPath);
    const QVariantMap env = environmentByName(name);
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

    m_enterEnvName.clear();
    m_enterAllowlistSnippet.clear();
    m_enterRemoteEnvPath.clear();
    m_statusMessage.clear();
    emit statusMessageChanged();
    launchCommandInTerminal(enterCmd);
    refresh();
}

void Backend::finishEnter(const QString &errorMessage) {
    if (!m_enterHostSecretFile.isEmpty())
        QFile::remove(m_enterHostSecretFile);
    m_enterHostSecretFile.clear();
    m_enterRemoteEnvPath.clear();
    m_enterEnvName.clear();
    m_enterAllowlistSnippet.clear();
    m_statusMessage.clear();
    emit statusMessageChanged();
    if (!errorMessage.isEmpty())
        setError(errorMessage);
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
                               const QString &processLimit, const QString &ioLimit,
                               const QString &workspaceParent) {
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
    QString workspaceError;
    const QString hostWorkspace = resolveWorkspacePath(workspaceParent, newName, m_workspaceBase, &workspaceError);
    if (hostWorkspace.isEmpty()) {
        setError(workspaceError);
        return;
    }
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
