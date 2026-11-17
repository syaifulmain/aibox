#pragma once

#include <QTranslator>

class TranslatorState;

// Virtual QTranslator::translate() override cannot be expressed directly in
// cxx-qt; this shim forwards lookups to the Rust TranslatorState catalog.
class TranslatorShim : public QTranslator {
public:
    explicit TranslatorShim(const TranslatorState *state, QObject *parent = nullptr);
    bool isEmpty() const override;
    QString translate(const char *context, const char *sourceText,
                      const char *disambiguation = nullptr, int n = -1) const override;

private:
    const TranslatorState *m_state;
};
