#pragma once

#include "backend.h"
#include "rust_bridge.h"
#include "aibox-ffi/src/util.rs.h"

#include <QCoreApplication>
#include <QFile>
#include <QGuiApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QProcess>
#include <QRegularExpression>
#include <QStandardPaths>
#include <QVariantMap>

// Shared free helpers for the Backend translation units. `inline` keeps these
// definitions safe when the header is included from more than one module.
namespace {

inline const QString kContainerWorkspacePath = QStringLiteral("/home/workspace");

inline QString formatBytes(qint64 bytes) {
    return aibox::ffi::fromRust(aibox::ffi::format_bytes(bytes));
}

inline qint64 parseSizeBytes(const QString &sizeText) {
    return aibox::ffi::parse_size_bytes(aibox::ffi::toRust(sizeText));
}

inline QString formatByteRate(qint64 bytesPerSecond) {
    return aibox::ffi::fromRust(aibox::ffi::format_byte_rate(bytesPerSecond));
}

inline QString formatByteTotal(qint64 bytes) {
    return aibox::ffi::fromRust(aibox::ffi::format_byte_total(bytes));
}

inline QStringList validatedSizeOptions(const QStringList &values) {
    return aibox::ffi::fromRustVec(
        aibox::ffi::validated_size_options(aibox::ffi::toRustVec(values)));
}

inline QVariantList validatedCpuOptions(const QVariantList &values) {
    const QByteArray json =
        QJsonDocument(QJsonArray::fromVariantList(values)).toJson(QJsonDocument::Compact);
    const rust::Vec<std::int32_t> cores = aibox::ffi::validated_cpu_options_json(
        rust::Str(json.constData(), static_cast<std::size_t>(json.size())));
    QVariantList result;
    result.reserve(static_cast<qsizetype>(cores.size()));
    for (const std::int32_t c : cores)
        result.append(static_cast<int>(c));
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
