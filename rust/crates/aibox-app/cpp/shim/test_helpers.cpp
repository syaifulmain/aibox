#include "test_helpers.h"

#include <QByteArray>
#include <QCoreApplication>
#include <QDir>
#include <QGuiApplication>
#include <QImage>
#include <QJsonArray>
#include <QJsonDocument>
#include <QKeyEvent>
#include <QMouseEvent>
#include <QMutex>
#include <QQmlApplicationEngine>
#include <QQuickItem>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QString>
#include <QStringList>
#include <QUrl>

#include <algorithm>
#include <cmath>
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

bool verify_control_heights(QQuickItem *item, const QString &ctx, QString &err_out) {
    if (!item || !item->isVisible())
        return true;

    const char *cls = item->metaObject()->className();
    if (cls && (std::strncmp(cls, "ActionButton_QMLTYPE", 20) == 0
                || std::strncmp(cls, "FlatInput_QMLTYPE", 17) == 0
                || std::strncmp(cls, "ToggleChip_QMLTYPE", 18) == 0)) {
        if (item->width() > 0 && item->height() > 0) {
            const bool compact = item->property("compact").toBool();
            const double expected = compact ? 28.0 : 34.0;
            if (std::abs(item->height() - expected) > 0.5) {
                err_out = QStringLiteral("Control height mismatch in %1 (%2, label='%3'): got %4, expected %5")
                              .arg(ctx, QString::fromLatin1(cls), item->property("label").toString())
                              .arg(item->height())
                              .arg(expected);
                return false;
            }
        }
    }

    const auto children = item->childItems();
    for (QQuickItem *child : children) {
        if (!verify_control_heights(child, ctx, err_out))
            return false;
    }
    return true;
}

QQuickItem *find_visible_close_button(QQuickItem *item) {
    if (!item || !item->isVisible())
        return nullptr;
    if (item->objectName() == QStringLiteral("modalCloseButton") && item->width() > 0 && item->height() > 0)
        return item;
    const auto children = item->childItems();
    for (QQuickItem *child : children) {
        if (QQuickItem *found = find_visible_close_button(child))
            return found;
    }
    return nullptr;
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

int aibox_test_backend_environments_json(void *ptr, char *buf, int buf_len) {
    auto *b = static_cast<Backend *>(ptr);
    const QJsonArray arr = QJsonArray::fromVariantList(b->getEnvironments());
    const QString json = QString::fromUtf8(QJsonDocument(arr).toJson(QJsonDocument::Compact));
    return copy_to_buf(json, buf, buf_len);
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
                win->show();
                win->requestActivate();
                QCoreApplication::processEvents(QEventLoop::AllEvents, 20);

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

                // Verify interactive control heights on the main inspector view.
                QString heightErr;
                if (!verify_control_heights(win->contentItem(), QStringLiteral("inspector"), heightErr)) {
                    copy_to_buf(heightErr, err_buf, err_buf_len);
                    rc = 1;
                }

                // Verify Close button, Esc, and scrim click close every modal dialog,
                // and verify all interactive control heights inside each modal.
                const char *modals[] = {
                    "helpVisible",
                    "toolsVisible",
                    "envActionsVisible",
                    "createVisible",
                    "cloneVisible",
                    "wipeVisible",
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
                for (const char *prop : modals) {
                    if (rc != 0)
                        break;

                    win->setProperty(prop, true);
                    QCoreApplication::processEvents(QEventLoop::AllEvents, 20);

                    if (!verify_control_heights(win->contentItem(), QString::fromLatin1(prop), heightErr)) {
                        copy_to_buf(heightErr, err_buf, err_buf_len);
                        rc = 1;
                        win->setProperty(prop, false);
                        break;
                    }

                    QQuickItem *closeBtn = find_visible_close_button(win->contentItem());
                    if (!closeBtn || !closeBtn->isEnabled()) {
                        copy_to_buf(QStringLiteral("Missing visible enabled Close button in modal: %1").arg(QString::fromLatin1(prop)), err_buf, err_buf_len);
                        rc = 1;
                        win->setProperty(prop, false);
                        break;
                    }
                    QMetaObject::invokeMethod(closeBtn, "clicked");
                    QCoreApplication::processEvents(QEventLoop::AllEvents, 20);
                    if (win->property(prop).toBool()) {
                        copy_to_buf(QStringLiteral("Close button did not close modal: %1").arg(QString::fromLatin1(prop)), err_buf, err_buf_len);
                        rc = 1;
                        win->setProperty(prop, false);
                        break;
                    }

                    win->setProperty(prop, true);
                    QCoreApplication::processEvents(QEventLoop::AllEvents, 20);
                    if (QGuiApplication::focusWindow() != win) {
                        win->requestActivate();
                        QCoreApplication::processEvents(QEventLoop::AllEvents, 50);
                    }
                    QKeyEvent escPress(QEvent::KeyPress, Qt::Key_Escape, Qt::NoModifier);
                    QKeyEvent escRelease(QEvent::KeyRelease, Qt::Key_Escape, Qt::NoModifier);
                    QCoreApplication::sendEvent(win, &escPress);
                    QCoreApplication::sendEvent(win, &escRelease);
                    QCoreApplication::processEvents(QEventLoop::AllEvents, 20);
                    if (win->property(prop).toBool()) {
                        copy_to_buf(
                            QStringLiteral("Escape did not close modal: %1 (active=%2, firstRun=%3)")
                                .arg(QString::fromLatin1(prop))
                                .arg(win->isActive())
                                .arg(backend.getFirstRun()),
                            err_buf,
                            err_buf_len);
                        rc = 1;
                        win->setProperty(prop, false);
                        break;
                    }

                    win->setProperty(prop, true);
                    QCoreApplication::processEvents(QEventLoop::AllEvents, 20);
                    const QPointF scrimPt(2.0, 2.0);
                    QMouseEvent scrimPress(QEvent::MouseButtonPress, scrimPt, scrimPt, Qt::LeftButton, Qt::LeftButton, Qt::NoModifier);
                    QMouseEvent scrimRelease(QEvent::MouseButtonRelease, scrimPt, scrimPt, Qt::LeftButton, Qt::NoButton, Qt::NoModifier);
                    QCoreApplication::sendEvent(win, &scrimPress);
                    QCoreApplication::sendEvent(win, &scrimRelease);
                    QCoreApplication::processEvents(QEventLoop::AllEvents, 20);
                    if (win->property(prop).toBool()) {
                        copy_to_buf(QStringLiteral("Scrim click did not close modal: %1").arg(QString::fromLatin1(prop)), err_buf, err_buf_len);
                        rc = 1;
                        win->setProperty(prop, false);
                        break;
                    }
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

int aibox_test_gallery_theme_matrix(const char *theme_dirs_newline_sep, char *err_buf, int err_buf_len) {
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
        engine.load(QUrl(QStringLiteral("qrc:/dev/ComponentGallery.qml")));

        if (engine.rootObjects().isEmpty()) {
            copy_to_buf(QStringLiteral("Failed to load qrc:/dev/ComponentGallery.qml"), err_buf, err_buf_len);
            rc = 1;
        } else {
            const QStringList dirs = QString::fromUtf8(theme_dirs_newline_sep ? theme_dirs_newline_sep : "")
                                         .split(QLatin1Char('\n'), Qt::SkipEmptyParts);
            const QString modes[] = {QStringLiteral("light"), QStringLiteral("dark")};
            const bool contrasts[] = {false, true};

            for (const QString &dir : dirs) {
                theme.configure(dir, QString());
                for (const QString &mode : modes) {
                    for (bool hc : contrasts) {
                        theme.setPreferences(mode, hc, true);
                        QCoreApplication::processEvents(QEventLoop::AllEvents, 5);
                    }
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

} // extern "C"
