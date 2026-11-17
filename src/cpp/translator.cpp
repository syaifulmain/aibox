#include "translator.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QLocale>

#include "rust_bridge.h"

Translator::Translator(const QString &languageFile, const QString &catalogDirectory, QObject *parent)
    : QTranslator(parent),
      m_languageFile(languageFile.isEmpty() ? QDir::homePath() + "/.config/omarchy/language" : languageFile),
      m_catalogDirectory(catalogDirectory),
      m_catalog(aibox::ffi::i18n_catalog_new()) {
    m_debounce.setSingleShot(true);
    m_debounce.setInterval(80);
    connect(&m_watcher, &QFileSystemWatcher::directoryChanged, this, [this] { m_debounce.start(); });
    connect(&m_watcher, &QFileSystemWatcher::fileChanged, this, [this] { m_debounce.start(); });
    connect(&m_debounce, &QTimer::timeout, this, &Translator::reload);
    reload();
}

// One registry code on the first line; anything else, or no file, is English.
QString Translator::readLanguage(const QString &path) {
    return aibox::ffi::fromRust(aibox::ffi::i18n_read_language(aibox::ffi::toRust(path)));
}

// CLDR cardinal plural categories for whole counts, for Omarchy's languages
// (the same rules as Omarchy's shell/Commons/Plural.js).
QString Translator::pluralCategory(const QString &language, long long count) {
    return aibox::ffi::fromRust(
        aibox::ffi::i18n_plural_category(aibox::ffi::toRust(language), static_cast<std::int64_t>(count)));
}

// The locale Omarchy formats this language with, from its registry, so the
// app's numbers and dates match the desktop's (Arabic there uses Western
// digits); without Omarchy, Qt's own locale for the code.
QLocale Translator::formatLocale(const QString &language) {
    const QString omarchyPath = qEnvironmentVariable("OMARCHY_PATH");
    const QString name = aibox::ffi::fromRust(
        aibox::ffi::i18n_format_locale_name(aibox::ffi::toRust(language), aibox::ffi::toRust(omarchyPath)));
    return QLocale(name);
}

bool Translator::isEmpty() const {
    return aibox::ffi::i18n_catalog_is_empty(*m_catalog);
}

QString Translator::translate(const char *, const char *sourceText, const char *, int n) const {
    // A null string tells Qt there is no translation, so it shows the English.
    const auto res = aibox::ffi::i18n_catalog_lookup(
        *m_catalog, sourceText ? rust::Str(sourceText) : rust::Str(), aibox::ffi::toRust(m_language), n);
    return res.found ? aibox::ffi::fromRust(res.text) : QString();
}

void Translator::reload() {
    const auto paths = m_watcher.files() + m_watcher.directories();
    if (!paths.isEmpty())
        m_watcher.removePaths(paths);

    // omarchy-language replaces the file, which drops it from the watch, and a
    // first pick creates it; watch its directory too and re-arm every time.
    const QString directory = QFileInfo(m_languageFile).absolutePath();
    if (QFileInfo::exists(directory))
        m_watcher.addPath(directory);
    if (QFileInfo::exists(m_languageFile))
        m_watcher.addPath(m_languageFile);

    // Catalogs are compiled in, so only a different language needs a reload.
    const QString language = readLanguage(m_languageFile);
    if (language == m_language)
        return;

    // English first (it carries the plural forms of counted strings), then
    // the base language and the regional file over it (pt, then pt-BR), so a
    // missing string falls back toward English one step at a time.
    auto catalog = aibox::ffi::i18n_catalog_new();
    const QStringList codes = aibox::ffi::fromRustVec(aibox::ffi::i18n_catalog_codes(aibox::ffi::toRust(language)));
    for (const auto &code : codes) {
        QFile file(m_catalogDirectory + "/" + code + ".json");
        if (!file.open(QIODevice::ReadOnly))
            continue;
        const QByteArray bytes = file.readAll();
        aibox::ffi::i18n_catalog_merge_json(*catalog, aibox::ffi::toRustBytes(bytes));
    }

    m_language = language;
    m_catalog = std::move(catalog);
    // Dates, numbers, and Qt's own day and month names follow the language
    // too, and right-to-left languages mirror the window.
    const QLocale locale = formatLocale(language);
    QLocale::setDefault(locale);
    QGuiApplication::setLayoutDirection(locale.textDirection());
    emit changed();
}
