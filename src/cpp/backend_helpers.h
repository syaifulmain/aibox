#pragma once

#include "backend.h"

#include <QCoreApplication>
#include <QFile>
#include <QGuiApplication>
#include <QProcess>
#include <QRegularExpression>
#include <QStandardPaths>
#include <QVariantMap>

// Shared free helpers for the Backend translation units. `inline` keeps these
// definitions safe when the header is included from more than one module.
namespace {

inline const QString kContainerWorkspacePath = QStringLiteral("/home/workspace");

inline QString formatBytes(qint64 bytes) {
    if (bytes <= 0)
        return QStringLiteral("0 MiB");
    const double mb = static_cast<double>(bytes) / (1024.0 * 1024.0);
    if (mb >= 1024.0)
        return QStringLiteral("%1 GiB").arg(mb / 1024.0, 0, 'f', 1);
    if (mb < 1.0)
        return QStringLiteral("%1 KiB").arg(static_cast<double>(bytes) / 1024.0, 0, 'f', 0);
    return QStringLiteral("%1 MiB").arg(mb, 0, 'f', 0);
}

inline qint64 parseSizeBytes(const QString &sizeText) {
    static const QRegularExpression re(
        QStringLiteral("^\\s*(\\d+(?:\\.\\d+)?)\\s*(TiB|TB|GiB|GB|MiB|MB|KiB|KB|B)?\\s*$"),
        QRegularExpression::CaseInsensitiveOption);
    const auto match = re.match(sizeText);
    if (!match.hasMatch())
        return 0;

    bool ok = false;
    const double value = match.captured(1).toDouble(&ok);
    if (!ok || value <= 0.0)
        return 0;

    const QString unit = match.captured(2).toUpper();
    double scale = 1.0;
    if (unit == QLatin1String("KIB") || unit == QLatin1String("KB"))
        scale = 1024.0;
    else if (unit == QLatin1String("MIB") || unit == QLatin1String("MB") || unit.isEmpty())
        scale = 1024.0 * 1024.0;
    else if (unit == QLatin1String("GIB") || unit == QLatin1String("GB"))
        scale = 1024.0 * 1024.0 * 1024.0;
    else if (unit == QLatin1String("TIB") || unit == QLatin1String("TB"))
        scale = 1024.0 * 1024.0 * 1024.0 * 1024.0;

    return static_cast<qint64>(std::llround(value * scale));
}

inline QString formatByteRate(qint64 bytesPerSecond) {
    if (bytesPerSecond <= 0)
        return QStringLiteral("0 KB/s");
    const double kb = static_cast<double>(bytesPerSecond) / 1024.0;
    if (kb < 1.0)
        return QStringLiteral("%1 B/s").arg(bytesPerSecond);
    if (kb < 1024.0)
        return QStringLiteral("%1 KB/s").arg(kb, 0, 'f', 1);
    const double mb = kb / 1024.0;
    if (mb < 1024.0)
        return QStringLiteral("%1 MB/s").arg(mb, 0, 'f', 2);
    return QStringLiteral("%1 GB/s").arg(mb / 1024.0, 0, 'f', 2);
}

inline QString formatByteTotal(qint64 bytes) {
    if (bytes <= 0)
        return QStringLiteral("0 B");
    const double kb = static_cast<double>(bytes) / 1024.0;
    if (kb < 1024.0)
        return QStringLiteral("%1 KB").arg(kb, 0, 'f', 1);
    const double mb = kb / 1024.0;
    if (mb < 1024.0)
        return QStringLiteral("%1 MB").arg(mb, 0, 'f', 2);
    const double gb = mb / 1024.0;
    if (gb < 1024.0)
        return QStringLiteral("%1 GB").arg(gb, 0, 'f', 2);
    return QStringLiteral("%1 TB").arg(gb / 1024.0, 0, 'f', 2);
}

inline QStringList validatedSizeOptions(const QStringList &values) {
    static const QRegularExpression sizeRe(QStringLiteral("^\\d+(?:\\.\\d+)?(?:GiB|MiB|TiB|GB|MB|TB)$"),
                                           QRegularExpression::CaseInsensitiveOption);
    QStringList result;
    for (const auto &raw : values) {
        const QString value = raw.trimmed();
        if (value.isEmpty() || !sizeRe.match(value).hasMatch())
            continue;
        if (!result.contains(value, Qt::CaseInsensitive))
            result.append(value);
    }
    return result;
}

inline QVariantList validatedCpuOptions(const QVariantList &values) {
    QVariantList result;
    QSet<int> seen;
    for (const auto &raw : values) {
        bool ok = false;
        const int cores = raw.toInt(&ok);
        if (!ok || cores < 1 || cores > 1024 || seen.contains(cores))
            continue;
        seen.insert(cores);
        result.append(cores);
    }
    return result;
}

inline QVariantMap poolInfoToVariantMap(const Backend::StoragePoolInfo &pool,
                                 qint64 allocatedBytes,
                                 int unboundedCount) {
    return QVariantMap{
        {"valid", pool.valid},
        {"name", pool.name},
        {"driver", pool.driver},
        {"usedBytes", pool.usedBytes},
        {"totalBytes", pool.totalBytes},
        {"usedText", pool.valid ? formatBytes(pool.usedBytes) : QString()},
        {"totalText", pool.valid ? formatBytes(pool.totalBytes) : QString()},
        {"usedPercent", pool.usedPercent},
        {"allocatedBytes", allocatedBytes},
        {"allocatedText", formatBytes(allocatedBytes)},
        {"unboundedCount", unboundedCount},
        {"sizeText", pool.sizeText},
        {"resizable", pool.resizable},
        {"overcommitted", pool.valid && (allocatedBytes > pool.totalBytes)},
        {"alertLevel", !pool.valid ? 0
                                   : (pool.usedPercent > 95 ? 2
                                      : ((pool.usedPercent > 85 || allocatedBytes > pool.totalBytes) ? 1 : 0))}
    };
}

inline bool isGuiSession() {
    return qobject_cast<QGuiApplication *>(QCoreApplication::instance()) != nullptr &&
           QGuiApplication::platformName() != QLatin1String("offscreen");
}

inline QString aclApplyShellSnippet(const QString &name, const QString &allowlistRaw,
                                    const QStringList &resolvedCidrs, bool egressLogging = false) {
    const QString qName = QStringLiteral("'") + QString(name).replace('\'', QStringLiteral("'\\''")) + QStringLiteral("'");
    const QString aclName = QStringLiteral("aibox-") + name;
    const QString qAcl = QStringLiteral("'") + QString(aclName).replace('\'', QStringLiteral("'\\''")) + QStringLiteral("'");
    const QString qLogAcl = QStringLiteral("'") + QString(QStringLiteral("aibox-log-") + name).replace('\'', QStringLiteral("'\\''")) + QStringLiteral("'");
    const QString qAllow = QStringLiteral("'") + QString(allowlistRaw).replace('\'', QStringLiteral("'\\''")) + QStringLiteral("'");

    QStringList lines;
    lines << QStringLiteral("incus config device remove %1 eth0 >/dev/null 2>&1 || true").arg(qName);
    lines << QStringLiteral("incus network acl delete %1 >/dev/null 2>&1 || true").arg(qAcl);
    lines << QStringLiteral("incus network acl delete %1 >/dev/null 2>&1 || true").arg(qLogAcl);
    lines << QStringLiteral("incus network acl create %1").arg(qAcl);
    for (const QString &cidr : resolvedCidrs) {
        const QString qCidr = QStringLiteral("'") + QString(cidr).replace('\'', QStringLiteral("'\\''")) + QStringLiteral("'");
        if (egressLogging)
            lines << QStringLiteral("incus network acl rule add %1 egress action=allow destination=%2 state=logged").arg(qAcl, qCidr);
        else
            lines << QStringLiteral("incus network acl rule add %1 egress action=allow destination=%2").arg(qAcl, qCidr);
    }
    if (egressLogging) {
        lines << QStringLiteral("incus config device override %1 eth0 security.acls=%2 security.acls.default.egress.action=drop security.acls.default.egress.logged=true")
                     .arg(qName, qAcl);
        lines << QStringLiteral("incus config set %1 user.aibox.egresslog true").arg(qName);
    } else {
        lines << QStringLiteral("incus config device override %1 eth0 security.acls=%2 security.acls.default.egress.action=drop")
                     .arg(qName, qAcl);
        lines << QStringLiteral("incus config unset %1 user.aibox.egresslog >/dev/null 2>&1 || true").arg(qName);
    }
    lines << QStringLiteral("incus config set %1 user.aibox.netmode allowlist").arg(qName);
    lines << QStringLiteral("incus config set %1 user.aibox.netallow %2").arg(qName, qAllow);
    return lines.join(QLatin1Char('\n'));
}

inline void launchCommandInTerminal(const QString &shellCmd) {
    if (!QStandardPaths::findExecutable(QStringLiteral("xdg-terminal-exec")).isEmpty()) {
        QProcess::startDetached(QStringLiteral("xdg-terminal-exec"),
                                {QStringLiteral("sh"), QStringLiteral("-c"), shellCmd});
    } else if (!QStandardPaths::findExecutable(QStringLiteral("foot")).isEmpty()) {
        QProcess::startDetached(QStringLiteral("foot"),
                                {QStringLiteral("sh"), QStringLiteral("-c"), shellCmd});
    }
}

} // namespace
