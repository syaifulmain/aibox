#include "backend.h"
#include "backend_helpers.h"

#include <QDesktopServices>
#include <QHostAddress>
#include <QHostInfo>
#include <QRegularExpression>
#include <QTcpServer>
#include <QUrl>

#include <algorithm>

namespace {

bool isPrivateOrReservedIpv4(const QHostAddress &addr) {
    if (addr.protocol() != QAbstractSocket::IPv4Protocol)
        return true;
    const quint32 ip = addr.toIPv4Address();
    if ((ip & 0xFF000000u) == 0x00000000u) return true; // 0.0.0.0/8
    if ((ip & 0xFF000000u) == 0x0A000000u) return true; // 10.0.0.0/8
    if ((ip & 0xFFC00000u) == 0x64400000u) return true; // 100.64.0.0/10
    if ((ip & 0xFF000000u) == 0x7F000000u) return true; // 127.0.0.0/8
    if ((ip & 0xFFFF0000u) == 0xA9FE0000u) return true; // 169.254.0.0/16
    if ((ip & 0xFFF00000u) == 0xAC100000u) return true; // 172.16.0.0/12
    if ((ip & 0xFFFF0000u) == 0xC0A80000u) return true; // 192.168.0.0/16
    if (ip >= 0xE0000000u) return true;                 // multicast / reserved
    return false;
}

} // namespace

void Backend::updateNetworkStats(bool restartProbe) {
    const QVariantMap env = selectedEnvironment();
    const QString name = env.value("name").toString();
    if (name != m_netProbeEnv) {
        m_netProbeEnv = name;
        m_netGateway.clear();
        m_netPingMs = -1.0;
        m_netPacketLoss = -1.0;
        restartProbe = true;
    }

    const QString mode = env.value("networkMode").toString();
    const QString ip = env.value("ipv4").toString();
    const bool canProbe = !name.isEmpty() && env.value("running").toBool()
                          && mode != QLatin1String("offline")
                          && !ip.isEmpty() && ip != QStringLiteral("—");

    if (!canProbe) {
        m_netProbeTimer.stop();
        if (m_netProbeProcess && m_netProbeProcess->state() != QProcess::NotRunning)
            m_netProbeProcess->kill();
    } else {
        if (!m_demoMode) {
            if (!m_netProbeTimer.isActive())
                m_netProbeTimer.start();
            const bool probing = m_netProbeProcess && m_netProbeProcess->state() != QProcess::NotRunning;
            if (restartProbe && !probing)
                QTimer::singleShot(150, this, &Backend::refreshNetworkStats);
        }
    }

    QVariantMap probe;
    probe["gateway"] = m_netGateway;
    probe["pingMs"] = m_netPingMs;
    probe["packetLossPercent"] = m_netPacketLoss;
    const QVariantMap stats = networkStatsFromEnvironment(env, probe);
    if (stats != m_networkStats) {
        m_networkStats = stats;
        emit networkStatsChanged();
    }
}

void Backend::refreshNetworkStats() {
    if (m_demoMode)
        return;

    const QVariantMap env = selectedEnvironment();
    const QString name = env.value("name").toString();
    const QString ip = env.value("ipv4").toString();
    if (name.isEmpty() || !env.value("running").toBool()
        || env.value("networkMode").toString() == QLatin1String("offline")
        || ip.isEmpty() || ip == QStringLiteral("—")) {
        m_netProbeTimer.stop();
        return;
    }
    if (m_netProbeProcess && m_netProbeProcess->state() != QProcess::NotRunning)
        return;

    if (!m_netProbeProcess) {
        m_netProbeProcess = new QProcess(this);
        connect(m_netProbeProcess, &QProcess::finished, this, [this](int, QProcess::ExitStatus) {
            const QString output = QString::fromUtf8(m_netProbeProcess->readAllStandardOutput());
            if (selectedEnvironment().value("name").toString() != m_netProbeEnv)
                return;
            const QVariantMap parsed = parseNetworkProbeOutput(output);
            const QString gateway = parsed.value("gateway").toString();
            if (!gateway.isEmpty())
                m_netGateway = gateway;
            m_netPingMs = parsed.value("pingMs").toDouble();
            m_netPacketLoss = parsed.value("packetLossPercent").toDouble();
            updateNetworkStats();
        });
    }

    m_netProbeEnv = name;
    const QString inner = QStringLiteral(
        "gw=\"\"; "
        "if command -v ip >/dev/null 2>&1; then gw=$(ip -4 route show default 2>/dev/null | awk '{print $3; exit}'); fi; "
        "if [ -z \"$gw\" ] && [ -r /proc/net/route ]; then gw=$(awk 'NR>1 && $2==\"00000000\" {print $3; exit}' /proc/net/route); fi; "
        "echo \"GW=$gw\"; "
        "if command -v ping >/dev/null 2>&1; then ping -n -c 3 -W 1 1.1.1.1 2>/dev/null; fi");
    const QString script = QStringLiteral("incus exec ") + shellQuote(name)
                           + QStringLiteral(" -- sh -c ") + shellQuote(inner);
    m_netProbeProcess->setProperty("envName", name);
    m_netProbeProcess->start(QStringLiteral("/bin/sh"), {QStringLiteral("-c"), script});
    QTimer::singleShot(9000, this, [this, name] {
        if (m_netProbeProcess && m_netProbeProcess->state() != QProcess::NotRunning
            && m_netProbeProcess->property("envName").toString() == name)
            m_netProbeProcess->kill();
    });
}

QVariantMap Backend::parseNetworkProbeOutput(const QString &output) {
    QVariantMap result;
    result["gateway"] = QString();
    result["pingMs"] = -1.0;
    result["packetLossPercent"] = -1.0;

    QString gateway;
    const QStringList lines = output.split(QLatin1Char('\n'));
    for (const QString &rawLine : lines) {
        const QString line = rawLine.trimmed();
        if (line.startsWith(QLatin1String("GW="))) {
            gateway = line.mid(3).trimmed();
            break;
        }
    }

    // /proc/net/route fallback exposes the gateway as little-endian hex.
    static const QRegularExpression hexRe(QStringLiteral("^[0-9A-Fa-f]{8}$"));
    if (hexRe.match(gateway).hasMatch()) {
        bool ok = false;
        const uint value = gateway.toUInt(&ok, 16);
        if (ok) {
            gateway = QStringLiteral("%1.%2.%3.%4")
                          .arg(value & 0xFF)
                          .arg((value >> 8) & 0xFF)
                          .arg((value >> 16) & 0xFF)
                          .arg((value >> 24) & 0xFF);
        }
    }
    result["gateway"] = gateway;

    static const QRegularExpression lossRe(QStringLiteral("(\\d+(?:\\.\\d+)?)%\\s*packet loss"));
    const auto lossMatch = lossRe.match(output);
    if (lossMatch.hasMatch())
        result["packetLossPercent"] = lossMatch.captured(1).toDouble();

    static const QRegularExpression rttRe(QStringLiteral("(?:rtt|round-trip)[^=]*=\\s*[0-9.]+/([0-9.]+)/"));
    const auto rttMatch = rttRe.match(output);
    if (rttMatch.hasMatch())
        result["pingMs"] = rttMatch.captured(1).toDouble();

    return result;
}

QVariantMap Backend::networkStatsFromEnvironment(const QVariantMap &env, const QVariantMap &probe) {
    const QString name = env.value("name").toString();
    const QString networkMode = env.value("networkMode").toString();
    const bool running = env.value("running").toBool();
    QString ip = env.value("ipv4").toString().trimmed();
    if (ip.isEmpty())
        ip = QStringLiteral("—");
    const bool active = !name.isEmpty() && running && networkMode != QLatin1String("offline")
                        && ip != QStringLiteral("—");

    QVariantMap stats;
    stats["active"] = active;
    stats["running"] = running;
    stats["networkMode"] = networkMode.isEmpty() ? QStringLiteral("online") : networkMode;
    stats["ip"] = ip;
    stats["gateway"] = probe.value("gateway").toString().isEmpty() ? QStringLiteral("—")
                                                                   : probe.value("gateway").toString();
    stats["pingMs"] = active ? probe.value("pingMs", -1.0) : -1.0;
    stats["packetLossPercent"] = active ? probe.value("packetLossPercent", -1.0) : -1.0;
    stats["rxRate"] = env.value("netRxRate", qint64(0)).toLongLong();
    stats["txRate"] = env.value("netTxRate", qint64(0)).toLongLong();
    stats["rxTotal"] = env.value("netRxBytes", qint64(0)).toLongLong();
    stats["txTotal"] = env.value("netTxBytes", qint64(0)).toLongLong();
    stats["rxRateText"] = env.value("netRxRateText", formatByteRate(0)).toString();
    stats["txRateText"] = env.value("netTxRateText", formatByteRate(0)).toString();
    stats["rxTotalText"] = env.value("netRxTotalText", formatByteTotal(0)).toString();
    stats["txTotalText"] = env.value("netTxTotalText", formatByteTotal(0)).toString();
    return stats;
}

QStringList Backend::parseAllowlistEntries(const QString &rawText) {
    static const QRegularExpression separator(QStringLiteral("[,;\\s]+"));
    static const QRegularExpression safeToken(QStringLiteral("^[a-z0-9.:/-]+$"));
    QStringList result;
    for (const QString &part : rawText.split(separator, Qt::SkipEmptyParts)) {
        const QString item = part.trimmed().toLower();
        if (item.isEmpty() || !safeToken.match(item).hasMatch())
            continue;
        if (!result.contains(item))
            result.append(item);
    }
    return result;
}

QStringList Backend::resolveAllowlistCidrs(const QStringList &entries, const HostResolver &resolver,
                                           QStringList *warnings) {
    HostResolver effectiveResolver = resolver;
    if (!effectiveResolver) {
        effectiveResolver = [](const QString &host) {
            QStringList ips;
            const QHostInfo info = QHostInfo::fromName(host);
            for (const QHostAddress &addr : info.addresses()) {
                if (addr.protocol() == QAbstractSocket::IPv4Protocol)
                    ips.append(addr.toString());
            }
            return ips;
        };
    }

    QStringList cidrs;
    auto appendUnique = [&cidrs](const QString &cidr) {
        if (!cidrs.contains(cidr))
            cidrs.append(cidr);
    };

    for (const QString &entry : entries) {
        if (entry.contains('/')) {
            const auto subnet = QHostAddress::parseSubnet(entry);
            if (!subnet.first.isNull() && subnet.first.protocol() == QAbstractSocket::IPv4Protocol && subnet.second >= 0) {
                appendUnique(QStringLiteral("%1/%2").arg(subnet.first.toString()).arg(subnet.second));
            } else if (warnings) {
                warnings->append(tr("Invalid CIDR: %1").arg(entry));
            }
            continue;
        }

        const QHostAddress directAddr(entry);
        if (!directAddr.isNull()) {
            if (directAddr.protocol() == QAbstractSocket::IPv4Protocol) {
                appendUnique(directAddr.toString() + QStringLiteral("/32"));
            } else if (warnings) {
                warnings->append(tr("Only IPv4 addresses are supported: %1").arg(entry));
            }
            continue;
        }

        static const QRegularExpression hostRe(QStringLiteral("^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)*$"));
        if (!hostRe.match(entry).hasMatch()) {
            if (warnings)
                warnings->append(tr("Invalid host entry: %1").arg(entry));
            continue;
        }

        const QStringList resolved = effectiveResolver(entry);
        int addedForHost = 0;
        for (const QString &ipStr : resolved) {
            const QHostAddress addr(ipStr);
            if (addr.isNull() || addr.protocol() != QAbstractSocket::IPv4Protocol)
                continue;
            // Block DNS rebinding to loopback, link-local, host, or LAN unless entered explicitly as IP/CIDR.
            if (isPrivateOrReservedIpv4(addr))
                continue;
            appendUnique(addr.toString() + QStringLiteral("/32"));
            ++addedForHost;
        }
        if (addedForHost == 0 && warnings)
            warnings->append(tr("No public IPv4 addresses resolved for '%1'.").arg(entry));
    }
    return cidrs;
}

QString Backend::networkControlScript(const QString &name, const QString &mode,
                                      const QString &allowlistRaw, const QStringList &resolvedCidrs,
                                      bool egressLogging) {
    const QString qName = shellQuote(name);
    const QString qAcl = shellQuote(QStringLiteral("aibox-") + name);
    const QString qLogAcl = shellQuote(QStringLiteral("aibox-log-") + name);
    QStringList lines;
    lines << QStringLiteral("set -euo pipefail");

    if (mode == QLatin1String("offline")) {
        lines << QStringLiteral("echo '@@STAGE:40:Disabling network interface...'");
        lines << QStringLiteral("incus config device remove %1 eth0 >/dev/null 2>&1 || true").arg(qName);
        lines << QStringLiteral("incus config device add %1 eth0 none").arg(qName);
        lines << QStringLiteral("incus network acl delete %1 >/dev/null 2>&1 || true").arg(qAcl);
        lines << QStringLiteral("incus network acl delete %1 >/dev/null 2>&1 || true").arg(qLogAcl);
        lines << QStringLiteral("incus config set %1 user.aibox.netmode offline").arg(qName);
        lines << QStringLiteral("incus config unset %1 user.aibox.egresslog >/dev/null 2>&1 || true").arg(qName);
        lines << QStringLiteral("echo '@@STAGE:100:Network disabled.'");
    } else if (mode == QLatin1String("allowlist")) {
        lines << QStringLiteral("echo '@@STAGE:40:Applying network allowlist ACL...'");
        lines << aclApplyShellSnippet(name, allowlistRaw, resolvedCidrs, egressLogging);
        lines << QStringLiteral("echo '@@STAGE:100:Network allowlist active.'");
    } else {
        lines << QStringLiteral("echo '@@STAGE:40:%1'")
                     .arg(egressLogging ? QStringLiteral("Enabling online egress logging ACL...")
                                        : QStringLiteral("Restoring full network access..."));
        lines << QStringLiteral("incus config device remove %1 eth0 >/dev/null 2>&1 || true").arg(qName);
        lines << QStringLiteral("incus network acl delete %1 >/dev/null 2>&1 || true").arg(qAcl);
        lines << QStringLiteral("incus network acl delete %1 >/dev/null 2>&1 || true").arg(qLogAcl);
        lines << QStringLiteral("incus config unset %1 user.aibox.netmode >/dev/null 2>&1 || true").arg(qName);
        lines << QStringLiteral("incus config unset %1 user.aibox.netallow >/dev/null 2>&1 || true").arg(qName);
        if (egressLogging) {
            lines << QStringLiteral("incus network acl create %1").arg(qLogAcl);
            lines << QStringLiteral("incus config device override %1 eth0 security.acls=%2 security.acls.default.egress.action=allow security.acls.default.egress.logged=true")
                         .arg(qName, qLogAcl);
            lines << QStringLiteral("incus config set %1 user.aibox.egresslog true").arg(qName);
        } else {
            lines << QStringLiteral("incus config unset %1 user.aibox.egresslog >/dev/null 2>&1 || true").arg(qName);
        }
        lines << QStringLiteral("echo '@@STAGE:100:%1'")
                     .arg(egressLogging ? QStringLiteral("Egress logging active.")
                                        : QStringLiteral("Network online."));
    }
    return lines.join(QLatin1Char('\n'));
}

QString Backend::buildAllowlistRefreshIfNeeded(const QString &name) const {
    const QVariantMap env = environmentByName(name);
    if (env.value("networkMode").toString() != QLatin1String("allowlist"))
        return {};
    const QString rawAllow = env.value("networkAllowlist").toString();
    const QStringList entries = parseAllowlistEntries(rawAllow);
    const QStringList cidrs = resolveAllowlistCidrs(entries);
    if (entries.isEmpty() || cidrs.isEmpty())
        return {};
    return aclApplyShellSnippet(name, rawAllow, cidrs, env.value("egressRecording").toBool());
}

void Backend::setNetworkMode(const QString &name, const QString &mode, const QString &allowlist) {
    if (name.isEmpty() || m_busy)
        return;
    const QString normalizedMode = mode.trimmed().toLower();
    if (normalizedMode != QLatin1String("online") &&
        normalizedMode != QLatin1String("offline") &&
        normalizedMode != QLatin1String("allowlist")) {
        setError(tr("Unsupported network mode: %1").arg(mode));
        return;
    }

    QStringList cidrs;
    QString normalizedAllowlist;
    if (normalizedMode == QLatin1String("allowlist")) {
        const QStringList entries = parseAllowlistEntries(allowlist);
        if (entries.isEmpty()) {
            setError(tr("Enter at least one domain, IP, or CIDR for allowlist mode."));
            return;
        }
        QStringList warnings;
        cidrs = resolveAllowlistCidrs(entries, {}, &warnings);
        if (cidrs.isEmpty()) {
            setError(warnings.isEmpty()
                         ? tr("Could not resolve any public IPv4 destinations from the allowlist.")
                         : warnings.join(QStringLiteral(" ")));
            return;
        }
        normalizedAllowlist = entries.join(QStringLiteral(", "));
    }

    const QVariantMap env = environmentByName(name);
    const bool keepLogging = (normalizedMode != QLatin1String("offline")) && env.value("egressRecording").toBool();
    const QString script = networkControlScript(name, normalizedMode, normalizedAllowlist, cidrs, keepLogging);
    runShellTask(tr("Updating network mode for '%1'").arg(name), script, {}, name);
}

void Backend::refreshNetworkAllowlist(const QString &name) {
    const QVariantMap env = environmentByName(name);
    if (env.isEmpty())
        return;
    setNetworkMode(name, QStringLiteral("allowlist"), env.value("networkAllowlist").toString());
}

void Backend::setEgressRecording(const QString &name, bool enabled) {
    if (name.isEmpty() || m_busy)
        return;
    const QVariantMap env = environmentByName(name);
    if (env.isEmpty())
        return;

    const QString mode = env.value("networkMode").toString();
    if (enabled && mode == QLatin1String("offline")) {
        setError(tr("Disable Offline mode first to record egress traffic."));
        return;
    }

    QString allowRaw;
    QStringList cidrs;
    QString targetMode = mode;
    if (mode == QLatin1String("allowlist")) {
        allowRaw = env.value("networkAllowlist").toString();
        const QStringList entries = parseAllowlistEntries(allowRaw);
        cidrs = resolveAllowlistCidrs(entries);
        if (entries.isEmpty() || cidrs.isEmpty()) {
            setError(tr("Allowlist entries could not be resolved. Update the allowlist first."));
            return;
        }
    } else {
        targetMode = QStringLiteral("online");
    }

    const QString script = networkControlScript(name, targetMode, allowRaw, cidrs, enabled);
    runShellTask(enabled ? tr("Enabling egress recording for '%1'").arg(name)
                         : tr("Disabling egress recording for '%1'").arg(name),
                 script, {}, name);
}

QVariantList Backend::parseKernelEgressLogs(const QString &journalOutput, const QString &containerIp,
                                            const QString &hostIface, int maxEntries) {
    const QString cleanIp = (containerIp.trimmed() == QLatin1String("—")) ? QString() : containerIp.trimmed();
    const QString cleanIface = hostIface.trimmed();
    if (cleanIp.isEmpty() && cleanIface.isEmpty())
        return {};

    static const QRegularExpression isoTsRe(QStringLiteral(R"(^(\d{4}-\d{2}-\d{2}T(\d{2}:\d{2}:\d{2}))\S*)"));
    static const QRegularExpression sysTsRe(QStringLiteral(R"(^[A-Z][a-z]{2}\s+\d+\s+(\d{2}:\d{2}:\d{2}))"));
    static const QRegularExpression inRe(QStringLiteral(R"(\bIN=(\S+))"));
    static const QRegularExpression srcRe(QStringLiteral(R"(\bSRC=(\S+))"));
    static const QRegularExpression dstRe(QStringLiteral(R"(\bDST=(\S+))"));
    static const QRegularExpression protoRe(QStringLiteral(R"(\bPROTO=(\S+))"));
    static const QRegularExpression dptRe(QStringLiteral(R"(\bDPT=(\d+))"));

    struct AggEntry {
        QString dstIp;
        QString proto;
        QString port;
        QString lastSeen;
        int count = 0;
        qint64 seq = 0;
    };
    QHash<QString, AggEntry> agg;
    qint64 seqCounter = 0;

    const QStringList lines = journalOutput.split(QLatin1Char('\n'), Qt::SkipEmptyParts);
    for (const QString &line : lines) {
        if (!line.contains(QLatin1String("eth0-egress")))
            continue;

        const QString inIf = inRe.match(line).captured(1);
        const QString srcIp = srcRe.match(line).captured(1);
        const bool matchIface = !cleanIface.isEmpty() && !inIf.isEmpty() && (inIf == cleanIface);
        const bool matchIp = !cleanIp.isEmpty() && !srcIp.isEmpty() && (srcIp == cleanIp);
        if (!matchIface && !matchIp)
            continue;

        const QString dstIp = dstRe.match(line).captured(1);
        if (dstIp.isEmpty())
            continue;
        const QString proto = protoRe.match(line).captured(1);
        const QString port = dptRe.match(line).captured(1);

        QString seen;
        const auto isoMatch = isoTsRe.match(line);
        if (isoMatch.hasMatch()) {
            seen = isoMatch.captured(2);
        } else {
            const auto sysMatch = sysTsRe.match(line);
            if (sysMatch.hasMatch())
                seen = sysMatch.captured(1);
        }

        const QString key = dstIp + QLatin1Char('|') + proto + QLatin1Char('|') + port;
        auto it = agg.find(key);
        ++seqCounter;
        if (it == agg.end()) {
            agg.insert(key, AggEntry{dstIp, proto.isEmpty() ? QStringLiteral("IP") : proto, port, seen, 1, seqCounter});
        } else {
            it->count += 1;
            if (!seen.isEmpty())
                it->lastSeen = seen;
            it->seq = seqCounter;
        }
    }

    QList<AggEntry> list = agg.values();
    std::sort(list.begin(), list.end(), [](const AggEntry &a, const AggEntry &b) {
        return a.seq > b.seq;
    });

    const int limit = qMax(1, maxEntries);
    QVariantList result;
    result.reserve(qMin(limit, static_cast<int>(list.size())));
    for (int i = 0; i < list.size() && i < limit; ++i) {
        const AggEntry &e = list.at(i);
        QVariantMap row;
        row["dstIp"] = e.dstIp;
        row["proto"] = e.proto;
        row["port"] = e.port.isEmpty() ? QStringLiteral("—") : e.port;
        row["count"] = e.count;
        row["lastSeen"] = e.lastSeen.isEmpty() ? QStringLiteral("—") : e.lastSeen;
        result.append(row);
    }
    return result;
}

void Backend::refreshEgressLog(const QString &name) {
    if (name.isEmpty())
        return;
    const QVariantMap env = environmentByName(name);
    if (env.isEmpty())
        return;

    m_egressLogsEnvName = name;
    if (m_demoMode) {
        m_egressLogs = {
            QVariantMap{{"dstIp", QStringLiteral("140.82.121.4")}, {"proto", QStringLiteral("TCP")}, {"port", QStringLiteral("443")}, {"count", 18}, {"lastSeen", QStringLiteral("13:18:42")}},
            QVariantMap{{"dstIp", QStringLiteral("160.79.104.10")}, {"proto", QStringLiteral("TCP")}, {"port", QStringLiteral("443")}, {"count", 9}, {"lastSeen", QStringLiteral("13:17:15")}},
            QVariantMap{{"dstIp", QStringLiteral("104.16.25.34")}, {"proto", QStringLiteral("TCP")}, {"port", QStringLiteral("443")}, {"count", 4}, {"lastSeen", QStringLiteral("13:15:09")}}
        };
        emit egressLogsChanged();
        return;
    }

    const QString containerIp = env.value("ipv4").toString();
    const QString hostIface = env.value("hostInterface").toString();

    auto *proc = new QProcess(this);
    connect(proc, &QProcess::finished, this, [this, proc, name, containerIp, hostIface](int, QProcess::ExitStatus) {
        const QString out = QString::fromUtf8(proc->readAllStandardOutput());
        proc->deleteLater();
        if (m_egressLogsEnvName != name)
            return;
        m_egressLogs = parseKernelEgressLogs(out, containerIp, hostIface, 100);
        emit egressLogsChanged();
    });
    proc->start(QStringLiteral("journalctl"),
                {QStringLiteral("-k"), QStringLiteral("-n"), QStringLiteral("4000"),
                 QStringLiteral("--no-pager"), QStringLiteral("--output=short-iso")});
}

void Backend::addEgressDestinationToAllowlist(const QString &name, const QString &destinationIp) {
    if (name.isEmpty() || m_busy)
        return;
    const QString ip = destinationIp.trimmed();
    if (ip.isEmpty())
        return;

    const QVariantMap env = environmentByName(name);
    QStringList entries = parseAllowlistEntries(env.value("networkAllowlist").toString());
    if (!entries.contains(ip) && !entries.contains(ip + QStringLiteral("/32")))
        entries.append(ip);
    setNetworkMode(name, QStringLiteral("allowlist"), entries.join(QStringLiteral(", ")));
}

QString Backend::proxyDeviceName(int hostPort) {
    return QStringLiteral("fwd-%1").arg(hostPort);
}

bool Backend::isHostPortAvailable(int hostPort) {
    if (hostPort < 1024 || hostPort > 65535)
        return false;
    QTcpServer server;
    if (!server.listen(QHostAddress::LocalHost, static_cast<quint16>(hostPort)))
        return false;
    server.close();
    return true;
}

QString Backend::addPortForwardScript(const QString &name, int hostPort, int guestPort) {
    const QString dev = proxyDeviceName(hostPort);
    return QStringLiteral("set -euo pipefail\n"
                          "echo '@@STAGE:45:Adding localhost port forward (%2 -> %3)...'\n"
                          "incus config device add %1 %4 proxy listen=%5 connect=%6\n"
                          "echo '@@STAGE:100:Port forward active.'")
        .arg(shellQuote(name),
             QString::number(hostPort),
             QString::number(guestPort),
             shellQuote(dev),
             shellQuote(QStringLiteral("tcp:127.0.0.1:%1").arg(hostPort)),
             shellQuote(QStringLiteral("tcp:127.0.0.1:%1").arg(guestPort)));
}

QString Backend::addPortForward(const QString &name, int hostPort, int guestPort) {
    if (name.isEmpty())
        return tr("Select an environment first.");
    if (m_busy)
        return tr("Another operation is currently in progress.");
    if (hostPort < 1024 || hostPort > 65535)
        return tr("Host port must be between 1024 and 65535 (127.0.0.1 bind).");
    if (guestPort < 1 || guestPort > 65535)
        return tr("Container port must be between 1 and 65535.");

    const QVariantMap env = environmentByName(name);
    if (env.isEmpty())
        return tr("Environment '%1' was not found.").arg(name);

    for (const auto &v : env.value("portForwards").toList()) {
        if (v.toMap().value("hostPort").toInt() == hostPort)
            return tr("Host port %1 is already forwarded on '%2'.").arg(hostPort).arg(name);
    }

    if (!isHostPortAvailable(hostPort))
        return tr("Host port 127.0.0.1:%1 is already in use on this machine.").arg(hostPort);

    runShellTask(tr("Forwarding 127.0.0.1:%1 -> %2:%3").arg(hostPort).arg(name).arg(guestPort),
                 addPortForwardScript(name, hostPort, guestPort), {}, name);
    return {};
}

void Backend::removePortForward(const QString &name, const QString &deviceName) {
    if (name.isEmpty() || deviceName.isEmpty() || m_busy)
        return;
    runShellTask(tr("Removing port forward '%1' on '%2'").arg(deviceName, name),
                 QStringLiteral("set -euo pipefail\n"
                                "echo '@@STAGE:50:Removing port forward...'\n"
                                "incus config device remove %1 %2\n"
                                "echo '@@STAGE:100:Port forward removed.'")
                     .arg(shellQuote(name), shellQuote(deviceName)),
                 {}, name);
}

void Backend::openForwardedPort(int hostPort) {
    if (hostPort < 1 || hostPort > 65535)
        return;
    if (isGuiSession())
        QDesktopServices::openUrl(QUrl(QStringLiteral("http://127.0.0.1:%1").arg(hostPort)));
}
