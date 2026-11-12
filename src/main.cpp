// aibox — Isolated Linux OS environment manager for AI, desktop, and mobile projects.

#include <QCommandLineParser>
#include <QDir>
#include <QGuiApplication>
#include <QIcon>
#include <QImage>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QTimer>
#include <QUrl>
#include <cstdio>

#include "backend.h"
#include "theme.h"
#include "translator.h"

int main(int argc, char *argv[]) {
    // Support headless CLI inspection (`aibox check` / `aibox list --json`) without a display.
    if (argc > 1) {
        const QString sub = QString::fromLocal8Bit(argv[1]);
        if (sub == QLatin1String("check") || sub == QLatin1String("list")) {
            qputenv("QT_QPA_PLATFORM", "offscreen");
        }
    }

    QGuiApplication app(argc, argv);
    app.setApplicationName("aibox");
    app.setApplicationVersion("0.1.0");

    if (argc > 1) {
        const QString sub = QString::fromLocal8Bit(argv[1]);
        if (sub == QLatin1String("check") || sub == QLatin1String("list")) {
            QProcess proc;
            proc.start(QStringLiteral("incus"), {QStringLiteral("list"), QStringLiteral("--format=json")});
            proc.waitForFinished(5000);
            if (proc.exitCode() != 0) {
                std::fprintf(stderr, "Error: Incus daemon is not reachable.\n");
                return 1;
            }
            const auto envs = Backend::parseIncusListJson(proc.readAllStandardOutput(),
                                                          QDir::homePath() + "/ai-workspaces");
            const auto doc = QJsonDocument::fromVariant(envs);
            std::fwrite(doc.toJson(QJsonDocument::Indented).constData(), 1,
                        doc.toJson(QJsonDocument::Indented).size(), stdout);
            return 0;
        }
    }

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
    Backend backend;

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
            QTimer::singleShot(350, win, [win, grabPath]() {
                const QImage img = win->grabWindow();
                img.save(grabPath);
                QGuiApplication::quit();
            });
        }
    }

    return app.exec();
}
