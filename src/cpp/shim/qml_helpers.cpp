#include "qml_helpers.h"

#include <QFile>
#include <QGuiApplication>
#include <QLocale>
#include <QQmlContext>

void aibox_set_context_property(QQmlApplicationEngine *engine, const QString &name, QObject *value) {
    if (engine)
        engine->rootContext()->setContextProperty(name, value);
}

void aibox_apply_locale(const QString &localeName) {
    const QLocale locale(localeName);
    QLocale::setDefault(locale);
    QGuiApplication::setLayoutDirection(locale.textDirection());
}

QByteArray aibox_read_qt_resource(const QString &path) {
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly))
        return {};
    return file.readAll();
}

bool aibox_is_gui_session() {
    return qobject_cast<QGuiApplication *>(QCoreApplication::instance()) != nullptr &&
           QGuiApplication::platformName() != QLatin1String("offscreen");
}
