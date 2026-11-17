#pragma once

#include "backend.h"
#include "rust_bridge.h"
#include "aibox-ffi/src/network.rs.h"
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
    const rust::Vec<rust::String> cidrsVec = aibox::ffi::toRustVec(resolvedCidrs);
    return aibox::ffi::fromRust(aibox::ffi::acl_apply_shell_snippet(
        aibox::ffi::toRust(name),
        aibox::ffi::toRust(allowlistRaw),
        rust::Slice<const rust::String>(cidrsVec.data(), cidrsVec.size()),
        egressLogging));
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
