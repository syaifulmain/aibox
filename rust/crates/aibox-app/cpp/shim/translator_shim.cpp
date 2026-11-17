#include "translator_shim.h"

#include "aibox-app/src/qt_translator.cxxqt.h"

TranslatorShim::TranslatorShim(const TranslatorState *state, QObject *parent)
    : QTranslator(parent), m_state(state) {}

bool TranslatorShim::isEmpty() const {
    return !m_state || translator_state_is_empty(*m_state);
}

QString TranslatorShim::translate(const char *, const char *sourceText, const char *, int n) const {
    if (!m_state || !sourceText)
        return {};
    const auto res = translator_state_translate(*m_state, rust::Str(sourceText), n);
    return res.found
        ? QString::fromUtf8(res.text.data(), static_cast<qsizetype>(res.text.size()))
        : QString();
}
