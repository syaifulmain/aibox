#pragma once

#include <QByteArray>
#include <QJsonDocument>
#include <QString>
#include <QStringList>
#include <QVariant>

#include <cstdint>
#include <string>

#include "rust/cxx.h"

namespace aibox::ffi {

inline rust::String toRust(const QString &s) {
    const QByteArray utf8 = s.toUtf8();
    return rust::String(utf8.constData(), static_cast<std::size_t>(utf8.size()));
}

inline QString fromRust(const rust::String &s) {
    return QString::fromUtf8(s.data(), static_cast<qsizetype>(s.size()));
}

inline QString fromRust(rust::Str s) {
    return QString::fromUtf8(s.data(), static_cast<qsizetype>(s.size()));
}

inline rust::Vec<rust::String> toRustVec(const QStringList &list) {
    rust::Vec<rust::String> out;
    out.reserve(static_cast<std::size_t>(list.size()));
    for (const QString &item : list) {
        out.push_back(toRust(item));
    }
    return out;
}

inline QStringList fromRustVec(const rust::Vec<rust::String> &vec) {
    QStringList out;
    out.reserve(static_cast<qsizetype>(vec.size()));
    for (const rust::String &item : vec) {
        out.append(fromRust(item));
    }
    return out;
}

inline QVariant fromRustJson(const rust::String &json) {
    const QByteArray bytes(json.data(), static_cast<qsizetype>(json.size()));
    return QJsonDocument::fromJson(bytes).toVariant();
}

inline QVariant fromRustJson(rust::Str json) {
    const QByteArray bytes(json.data(), static_cast<qsizetype>(json.size()));
    return QJsonDocument::fromJson(bytes).toVariant();
}

inline rust::Slice<const std::uint8_t> toRustBytes(const QByteArray &bytes) {
    return rust::Slice<const std::uint8_t>(
        reinterpret_cast<const std::uint8_t *>(bytes.constData()),
        static_cast<std::size_t>(bytes.size()));
}

inline QByteArray fromRustBytes(const rust::Vec<std::uint8_t> &bytes) {
    return QByteArray(reinterpret_cast<const char *>(bytes.data()),
                      static_cast<qsizetype>(bytes.size()));
}

}  // namespace aibox::ffi
