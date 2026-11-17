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
