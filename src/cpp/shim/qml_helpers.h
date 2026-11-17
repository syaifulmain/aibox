#pragma once

#include <QByteArray>
#include <QObject>
#include <QQmlApplicationEngine>
#include <QString>

void aibox_set_context_property(QQmlApplicationEngine *engine, const QString &name, QObject *value);
void aibox_apply_locale(const QString &localeName);
QByteArray aibox_read_qt_resource(const QString &path);
