#include "test_helpers.h"

#include <QByteArray>
#include <QCoreApplication>
#include <QGuiApplication>
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

int aibox_test_qml_smoke(char *err_buf, int err_buf_len) {
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
    aibox_test_shutdown_gui_app();
    return rc;
}

} // extern "C"
