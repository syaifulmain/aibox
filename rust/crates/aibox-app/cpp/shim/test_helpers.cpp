#include "test_helpers.h"

#include <QByteArray>
#include <QCoreApplication>
#include <QDir>
#include <QGuiApplication>
#include <QImage>
#include <QMutex>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QString>
#include <QStringList>
#include <QUrl>

#include <algorithm>
#include <cstring>
#include <memory>

#include "aibox-app/src/qt_backend.cxxqt.h"
#include "aibox-app/src/qt_theme.cxxqt.h"
#include "aibox-app/src/qt_translator.cxxqt.h"
#include "qml_helpers.h"
#include "translator_shim.h"

namespace {

std::unique_ptr<QGuiApplication> g_test_app;
int g_argc = 1;
char g_prog_name[] = "aibox_test";
char *g_argv[] = {g_prog_name, nullptr};

QMutex g_msg_mutex;
QStringList g_collected_warnings;

void test_message_handler(QtMsgType type, const QMessageLogContext &ctx, const QString &msg) {
    if (type == QtWarningMsg || type == QtCriticalMsg || type == QtFatalMsg) {
        const QString loc = ctx.file
            ? QStringLiteral("%1:%2: ").arg(QString::fromUtf8(ctx.file)).arg(ctx.line)
            : QString();
        QMutexLocker locker(&g_msg_mutex);
        g_collected_warnings.append(loc + msg);
    }
}

int copy_to_buf(const QString &text, char *buf, int buf_len) {
    const QByteArray utf8 = text.toUtf8();
    if (buf && buf_len > 0) {
        const int n = std::min(static_cast<int>(utf8.size()), buf_len - 1);
        if (n > 0)
            std::memcpy(buf, utf8.constData(), static_cast<size_t>(n));
        buf[n] = '\0';
    }
    return static_cast<int>(utf8.size());
}

} // namespace

extern "C" {

void aibox_test_init_gui_app(void) {
    if (QCoreApplication::instance() != nullptr)
        return;
    if (qEnvironmentVariableIsEmpty("QT_QPA_PLATFORM"))
        qputenv("QT_QPA_PLATFORM", "offscreen");
    g_test_app = std::make_unique<QGuiApplication>(g_argc, g_argv);
    g_test_app->setApplicationName(QStringLiteral("aibox"));
    QQuickStyle::setStyle(QStringLiteral("Material"));
}

void aibox_test_shutdown_gui_app(void) {
    g_test_app.reset();
}

void aibox_test_process_events(int max_ms) {
    QCoreApplication::processEvents(QEventLoop::AllEvents, max_ms);
}

void *aibox_test_backend_new(void) {
    return new Backend();
}

void aibox_test_backend_free(void *ptr) {
    delete static_cast<Backend *>(ptr);
}

void aibox_test_backend_configure(void *ptr, const char *path, bool project_mode) {
    auto *b = static_cast<Backend *>(ptr);
    b->configure(QString::fromUtf8(path), project_mode);
}

int aibox_test_backend_count(void *ptr) {
    auto *b = static_cast<Backend *>(ptr);
    return b->getCount();
}

int aibox_test_backend_primary_action_with_spy(void *ptr) {
    auto *b = static_cast<Backend *>(ptr);
    int fired = 0;
    QObject::connect(b, &Backend::countChanged, b, [&fired]() { ++fired; });
    b->primaryAction();
    return fired;
}

int aibox_test_backend_enable_file_access(void *ptr, const char *name, char *buf, int buf_len) {
    auto *b = static_cast<Backend *>(ptr);
    const QString err = b->enableFileAccess(QString::fromUtf8(name));
    return copy_to_buf(err, buf, buf_len);
}

void aibox_test_backend_disable_file_access(void *ptr, const char *name) {
    auto *b = static_cast<Backend *>(ptr);
    b->disableFileAccess(QString::fromUtf8(name));
}

void aibox_test_backend_open_file_access(void *ptr, const char *name) {
    auto *b = static_cast<Backend *>(ptr);
    b->openFileAccess(QString::fromUtf8(name));
}

bool aibox_test_backend_busy(void *ptr) {
    auto *b = static_cast<Backend *>(ptr);
    return b->getBusy();
}

bool aibox_test_backend_selected_env_empty(void *ptr) {
    auto *b = static_cast<Backend *>(ptr);
    return b->selected_environment().isEmpty();
}

int aibox_test_backend_env_count(void *ptr) {
    auto *b = static_cast<Backend *>(ptr);
    return static_cast<int>(b->getEnvironments().size());
}

void aibox_test_backend_enter_environment(void *ptr, const char *name) {
    auto *b = static_cast<Backend *>(ptr);
    b->enterEnvironment(QString::fromUtf8(name));
}

void *aibox_test_theme_new(void) {
    return new Theme();
}

void aibox_test_theme_free(void *ptr) {
    delete static_cast<Theme *>(ptr);
}

void aibox_test_theme_configure(void *ptr, const char *dir) {
    auto *t = static_cast<Theme *>(ptr);
    t->configure(QString::fromUtf8(dir ? dir : ""), QString());
}

bool aibox_test_theme_is_dark(void *ptr) {
    auto *t = static_cast<Theme *>(ptr);
    return t->getIsDark();
}

int aibox_test_theme_string_prop(void *ptr, const char *prop_name, char *buf, int buf_len) {
    auto *t = static_cast<Theme *>(ptr);
    const QVariant val = t->property(prop_name);
    return copy_to_buf(val.toString(), buf, buf_len);
}

struct ThemeChangedSpy {
    int count = 0;
    QMetaObject::Connection conn;
};

void *aibox_test_theme_spy_new(void *ptr) {
    auto *t = static_cast<Theme *>(ptr);
    auto *spy = new ThemeChangedSpy();
    spy->conn = QObject::connect(t, &Theme::changed, t, [spy]() { ++spy->count; });
    return spy;
}

int aibox_test_theme_spy_count(void *spy) {
    return static_cast<ThemeChangedSpy *>(spy)->count;
}

void aibox_test_theme_spy_free(void *spy) {
    auto *s = static_cast<ThemeChangedSpy *>(spy);
    QObject::disconnect(s->conn);
    delete s;
}

int aibox_test_qml_smoke(char *err_buf, int err_buf_len) {
    return aibox_test_qml_smoke_with_theme(nullptr, err_buf, err_buf_len);
}

int aibox_test_qml_smoke_with_theme(const char *theme_dir, char *err_buf, int err_buf_len) {
    aibox_test_init_gui_app();

    {
        QMutexLocker locker(&g_msg_mutex);
        g_collected_warnings.clear();
    }
    qInstallMessageHandler(test_message_handler);

    const QByteArray prev_demo = qgetenv("AIBOX_DEMO_MODE");
    qputenv("AIBOX_DEMO_MODE", "1");

    int rc = 0;
    {
        TranslatorState translatorState;
        TranslatorShim translator(&translatorState);
        QCoreApplication::installTranslator(&translator);

        Theme theme;
        if (theme_dir && theme_dir[0] != '\0')
            theme.configure(QString::fromUtf8(theme_dir), QString());
        Backend backend;
        backend.configure(QString(), true);

        QQmlApplicationEngine engine;
        QObject::connect(&translatorState, &TranslatorState::changed, &engine, &QQmlEngine::retranslate);
        aibox_set_context_property(&engine, QStringLiteral("translator"), &translatorState);
        aibox_set_context_property(&engine, QStringLiteral("theme"), &theme);
        aibox_set_context_property(&engine, QStringLiteral("backend"), &backend);
        engine.load(QUrl(QStringLiteral("qrc:/Main.qml")));

        if (engine.rootObjects().isEmpty()) {
            copy_to_buf(QStringLiteral("Failed to load qrc:/Main.qml"), err_buf, err_buf_len);
            rc = 1;
        } else {
            QCoreApplication::processEvents(QEventLoop::AllEvents, 50);
            auto *win = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
            if (!win) {
                copy_to_buf(QStringLiteral("Root object is not a QQuickWindow"), err_buf, err_buf_len);
                rc = 1;
            } else {
                win->setProperty("settingsVisible", true);
                for (int section = 0; section <= 4; ++section) {
                    win->setProperty("settingsSection", section);
                    QCoreApplication::processEvents(QEventLoop::AllEvents, 20);
                }
                win->setProperty("settingsVisible", false);
                QCoreApplication::processEvents(QEventLoop::AllEvents, 10);

                const char *dialogs[] = {
                    "helpVisible",
                    "toolsVisible",
                    "envActionsVisible",
                    "createVisible",
                    "cloneVisible",
                    "wipeVisible",
                    "osEditorVisible",
                    "staleReportVisible",
                    "historyVisible",
                    "egressVisible",
                    "portsVisible",
                    "mountsVisible",
                    "editQuotasVisible",
                    "backupsVisible",
                    "restoreDialogVisible",
                    "changeReviewVisible",
                    "secretsVisible",
                };
                for (const char *prop : dialogs) {
                    win->setProperty(prop, true);
                    QCoreApplication::processEvents(QEventLoop::AllEvents, 20);
                    win->setProperty(prop, false);
                    QCoreApplication::processEvents(QEventLoop::AllEvents, 10);
                }
            }
        }

        QCoreApplication::removeTranslator(&translator);
    }

    if (prev_demo.isNull())
        qunsetenv("AIBOX_DEMO_MODE");
    else
        qputenv("AIBOX_DEMO_MODE", prev_demo);

    qInstallMessageHandler(nullptr);

    if (rc == 0) {
        QMutexLocker locker(&g_msg_mutex);
        if (!g_collected_warnings.isEmpty()) {
            copy_to_buf(g_collected_warnings.join(QStringLiteral("\n")), err_buf, err_buf_len);
            rc = static_cast<int>(g_collected_warnings.size());
        }
    }
    return rc;
}

int aibox_test_render_theme_matrix(const char *theme_dir, const char *out_dir, char *err_buf, int err_buf_len) {
    aibox_test_init_gui_app();

    const QString outPath = QString::fromUtf8(out_dir ? out_dir : "");
    if (outPath.isEmpty() || !QDir().mkpath(outPath)) {
        copy_to_buf(QStringLiteral("Failed to create output directory"), err_buf, err_buf_len);
        return 1;
    }

    const QByteArray prev_demo = qgetenv("AIBOX_DEMO_MODE");
    qputenv("AIBOX_DEMO_MODE", "1");

    int rc = 0;
    {
        TranslatorState translatorState;
        TranslatorShim translator(&translatorState);
        QCoreApplication::installTranslator(&translator);

        Theme theme;
        if (theme_dir && theme_dir[0] != '\0')
            theme.configure(QString::fromUtf8(theme_dir), QString());
        Backend backend;
        backend.configure(QString(), true);

        auto grab_window = [&](QQuickWindow *win, const QString &filename) -> bool {
            win->show();
            QCoreApplication::processEvents(QEventLoop::AllEvents, 120);
            win->requestUpdate();
            QCoreApplication::processEvents(QEventLoop::AllEvents, 120);
            const QImage img = win->grabWindow();
            if (img.isNull()) {
                copy_to_buf(QStringLiteral("grabWindow() returned null for ") + filename, err_buf, err_buf_len);
                return false;
            }
            const QString target = outPath + QLatin1Char('/') + filename;
            if (!img.save(target, "PNG")) {
                copy_to_buf(QStringLiteral("Failed to save ") + target, err_buf, err_buf_len);
                return false;
            }
            return true;
        };

        // 1. Main screen, Settings, CreateDialog, WipeDialog
        {
            QQmlApplicationEngine engine;
            QObject::connect(&translatorState, &TranslatorState::changed, &engine, &QQmlEngine::retranslate);
            aibox_set_context_property(&engine, QStringLiteral("translator"), &translatorState);
            aibox_set_context_property(&engine, QStringLiteral("theme"), &theme);
            aibox_set_context_property(&engine, QStringLiteral("backend"), &backend);
            engine.load(QUrl(QStringLiteral("qrc:/Main.qml")));
            if (engine.rootObjects().isEmpty()) {
                copy_to_buf(QStringLiteral("Failed to load qrc:/Main.qml"), err_buf, err_buf_len);
                rc = 1;
            } else if (auto *win = qobject_cast<QQuickWindow *>(engine.rootObjects().first())) {
                if (!grab_window(win, QStringLiteral("main.png")))
                    rc = 1;
                if (rc == 0) {
                    win->setProperty("settingsVisible", true);
                    if (!grab_window(win, QStringLiteral("settings.png")))
                        rc = 1;
                    win->setProperty("settingsVisible", false);
                }
                if (rc == 0) {
                    win->setProperty("createVisible", true);
                    if (!grab_window(win, QStringLiteral("create.png")))
                        rc = 1;
                    win->setProperty("createVisible", false);
                }
                if (rc == 0) {
                    win->setProperty("wipeVisible", true);
                    if (!grab_window(win, QStringLiteral("wipe.png")))
                        rc = 1;
                    win->setProperty("wipeVisible", false);
                }
            } else {
                copy_to_buf(QStringLiteral("Main root object is not a QQuickWindow"), err_buf, err_buf_len);
                rc = 1;
            }
        }

        // 2. ComponentGallery
        if (rc == 0) {
            QQmlApplicationEngine galleryEngine;
            QObject::connect(&translatorState, &TranslatorState::changed, &galleryEngine, &QQmlEngine::retranslate);
            aibox_set_context_property(&galleryEngine, QStringLiteral("translator"), &translatorState);
            aibox_set_context_property(&galleryEngine, QStringLiteral("theme"), &theme);
            aibox_set_context_property(&galleryEngine, QStringLiteral("backend"), &backend);
            galleryEngine.load(QUrl(QStringLiteral("qrc:/dev/ComponentGallery.qml")));
            if (galleryEngine.rootObjects().isEmpty()) {
                copy_to_buf(QStringLiteral("Failed to load qrc:/dev/ComponentGallery.qml"), err_buf, err_buf_len);
                rc = 1;
            } else if (auto *win = qobject_cast<QQuickWindow *>(galleryEngine.rootObjects().first())) {
                if (!grab_window(win, QStringLiteral("gallery.png")))
                    rc = 1;
            } else {
                copy_to_buf(QStringLiteral("Gallery root object is not a QQuickWindow"), err_buf, err_buf_len);
                rc = 1;
            }
        }

        QCoreApplication::removeTranslator(&translator);
    }

    if (prev_demo.isNull())
        qunsetenv("AIBOX_DEMO_MODE");
    else
        qputenv("AIBOX_DEMO_MODE", prev_demo);

    return rc;
}

} // extern "C"
