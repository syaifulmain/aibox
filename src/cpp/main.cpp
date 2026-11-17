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

#include <cstring>

#include "backend.h"
#include "cli.h"
#include "rust_bridge.h"
#include "aibox-ffi/src/cli.rs.h"
#include "aibox-ffi/src/meta.rs.h"
#include "aibox-ffi/src/session.rs.h"
#include "theme.h"
#include "translator.h"

int main(int argc, char *argv[]) {
    if (argc >= 3 && std::strcmp(argv[1], "--redact-session") == 0) {
        const QString logPath = QString::fromLocal8Bit(argv[2]);
        const QString sessionsDir = argc >= 4 ? QString::fromLocal8Bit(argv[3]) : QString();
        return aibox::ffi::redact_session_main(
            aibox::ffi::toRust(logPath), aibox::ffi::toRust(sessionsDir));
    }

    if (argc > 1) {
        std::vector<rust::String> cliArgs;
        cliArgs.reserve(static_cast<size_t>(argc - 1));
        for (int i = 1; i < argc; ++i)
            cliArgs.push_back(aibox::ffi::toRust(QString::fromLocal8Bit(argv[i])));
        const rust::Slice<const rust::String> cliSlice(cliArgs.data(), cliArgs.size());
        if (aibox::ffi::cli_is_invocation(cliSlice)) {
            return aibox::ffi::cli_run(cliSlice);
        }
    }

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
    Translator translator;
    app.installTranslator(&translator);

    const QString themeDir = qEnvironmentVariable("AIBOX_THEME_DIR");
    Theme theme(themeDir);
    Backend backend(guiArgs.projectDir, Backend::PathMode::ProjectDir);

    const QString selectIdx = qEnvironmentVariable("AIBOX_SELECT_INDEX");
    if (!selectIdx.isEmpty()) {
        backend.setSelectedIndex(selectIdx.toInt());
    }

    QQmlApplicationEngine engine;
    QObject::connect(&translator, &Translator::changed, &engine, &QQmlEngine::retranslate);
    engine.rootContext()->setContextProperty("translator", &translator);
    engine.rootContext()->setContextProperty("theme", &theme);
    engine.rootContext()->setContextProperty("backend", &backend);
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
