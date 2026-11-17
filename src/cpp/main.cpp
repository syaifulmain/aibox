// aibox — Isolated Linux OS environment manager for AI, desktop, and mobile projects.

#include <QFile>
#include <QGuiApplication>
#include <QIcon>
#include <QImage>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QTimer>
#include <QUrl>

#include "backend.h"
#include "cli.h"
#include "rust_bridge.h"
#include "aibox-app/src/qt_theme.cxxqt.h"
#include "aibox-app/src/qt_translator.cxxqt.h"
#include "aibox-ffi/src/meta.rs.h"
#include "shim/qml_helpers.h"
#include "shim/translator_shim.h"

extern "C" int aibox_gui_main(int argc, char *argv[]) {

    QGuiApplication app(argc, argv);
    app.setApplicationName("aibox");
    app.setApplicationVersion(aibox::ffi::fromRust(aibox::ffi::core_version()));

    QStringList rawArgs;
    for (int i = 1; i < argc; ++i)
        rawArgs.append(QString::fromLocal8Bit(argv[i]));
    const Cli::ParsedArgs guiArgs = Cli::parseArgs(rawArgs);

    // Matches the window to aibox.desktop, so the compositor (Wayland app_id)
    // and the launcher pick up the installed icon.
    app.setDesktopFileName("aibox");
    app.setWindowIcon(QIcon::fromTheme("aibox"));

    QQuickStyle::setStyle("Material");

    // Installed before anything is translated, and every qsTr() binding is
    // re-evaluated when the Omarchy language changes.
    TranslatorState translatorState;
    TranslatorShim translator(&translatorState);
    app.installTranslator(&translator);

    Theme theme;
    const QString themeDir = qEnvironmentVariable("AIBOX_THEME_DIR");
    if (!themeDir.isEmpty())
        theme.configure(themeDir, QString());
    Backend backend(guiArgs.projectDir, Backend::PathMode::ProjectDir);

    const QString selectIdx = qEnvironmentVariable("AIBOX_SELECT_INDEX");
    if (!selectIdx.isEmpty()) {
        backend.setSelectedIndex(selectIdx.toInt());
    }

    QQmlApplicationEngine engine;
    QObject::connect(&translatorState, &TranslatorState::changed, &engine, &QQmlEngine::retranslate);
    aibox_set_context_property(&engine, QStringLiteral("translator"), &translatorState);
    aibox_set_context_property(&engine, QStringLiteral("theme"), &theme);
    aibox_set_context_property(&engine, QStringLiteral("backend"), &backend);
    engine.load(QUrl("qrc:/Main.qml"));
    if (engine.rootObjects().isEmpty())
        return 1;

    const QString grabPath = qEnvironmentVariable("AIBOX_GRAB_PATH");
    if (!grabPath.isEmpty()) {
        auto *win = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
        if (win) {
            if (qEnvironmentVariableIntValue("AIBOX_PREVIEW_CREATE") == 1)
                win->setProperty("createVisible", true);
            if (qEnvironmentVariableIntValue("AIBOX_OPEN_SETTINGS") == 1) {
                win->setProperty("settingsSection", qEnvironmentVariableIntValue("AIBOX_SETTINGS_SECTION"));
                win->setProperty("settingsVisible", true);
            }
            if (qEnvironmentVariableIntValue("AIBOX_OPEN_OS_EDITOR") == 1)
                win->setProperty("osEditorVisible", true);
            QTimer::singleShot(350, win, [win, grabPath]() {
                const QImage img = win->grabWindow();
                img.save(grabPath);
                QGuiApplication::quit();
            });
        }
    }

    return app.exec();
}
