// Theme watching follows Omacut (MIT, David Heinemeier Hansson).
#include "theme.h"

#include <QDir>
#include <QFileInfo>
#include <QStandardPaths>

#include "aibox-ffi/src/theme.rs.h"
#include "rust_bridge.h"

Theme::Theme(const QString &directory, QObject *parent, const QString &hyprctl)
    : QObject(parent),
      m_directory(directory.isEmpty() ? QDir::homePath() + "/.local/state/omarchy/current" : directory) {
    m_debounce.setSingleShot(true);
    m_debounce.setInterval(80);
    connect(&m_watcher, &QFileSystemWatcher::directoryChanged, this, [this] { m_debounce.start(); });
    connect(&m_watcher, &QFileSystemWatcher::fileChanged, this, [this] { m_debounce.start(); });
    connect(&m_debounce, &QTimer::timeout, this, &Theme::reload);

    // Corners follow Hyprland's effective rounding, which themes and personal
    // overrides can both set, so ask hyprctl rather than reading its config.
    // Without Hyprland the corners stay sharp, like Omarchy's default.
    m_hyprctl = !hyprctl.isEmpty() ? hyprctl
              : directory.isEmpty() && !qEnvironmentVariableIsEmpty("HYPRLAND_INSTANCE_SIGNATURE")
                  ? QStandardPaths::findExecutable("hyprctl") : QString();
    m_roundingTimeout.setSingleShot(true);
    m_roundingTimeout.setInterval(500);
    connect(&m_roundingTimeout, &QTimer::timeout, &m_roundingProcess, &QProcess::kill);
    connect(&m_roundingProcess, &QProcess::errorOccurred, this, [this] { m_roundingTimeout.stop(); });
    connect(&m_roundingProcess, &QProcess::finished, this, [this](int code, QProcess::ExitStatus status) {
        m_roundingTimeout.stop();
        if (code != 0 || status != QProcess::NormalExit)
            return;
        const QByteArray stdoutBytes = m_roundingProcess.readAllStandardOutput();
        const auto parsed = aibox::ffi::theme_parse_rounding(aibox::ffi::toRustBytes(stdoutBytes));
        if (!parsed.valid)
            return;
        if (parsed.value != m_radius) {
            m_radius = parsed.value;
            emit changed();
        }
    });
    m_roundingPoll.setInterval(1000);
    connect(&m_roundingPoll, &QTimer::timeout, this, &Theme::refreshRounding);
    if (!m_hyprctl.isEmpty())
        m_roundingPoll.start();
    reload();
}

Theme::~Theme() {
    if (m_roundingProcess.state() != QProcess::NotRunning) {
        m_roundingProcess.kill();
        m_roundingProcess.waitForFinished(1000);
    }
}

void Theme::refreshRounding() {
    if (m_hyprctl.isEmpty() || m_roundingProcess.state() != QProcess::NotRunning)
        return;
    m_roundingProcess.start(m_hyprctl, {"-j", "getoption", "decoration:rounding"});
    m_roundingTimeout.start();
}

QString Theme::readKey(const QString &path, const QString &key, const QString &fallback) {
    return aibox::ffi::fromRust(aibox::ffi::theme_read_key(
        aibox::ffi::toRust(path), aibox::ffi::toRust(key), aibox::ffi::toRust(fallback)));
}

QString Theme::readAccent(const QString &path) {
    return aibox::ffi::fromRust(aibox::ffi::theme_read_accent(aibox::ffi::toRust(path)));
}

// Black or white, whichever reads on the accent (WCAG relative luminance).
QString Theme::accentForeground() const {
    return aibox::ffi::fromRust(aibox::ffi::theme_accent_foreground(aibox::ffi::toRust(m_accent)));
}

void Theme::reload() {
    refreshRounding();
    const auto paths = m_watcher.files() + m_watcher.directories();
    if (!paths.isEmpty())
        m_watcher.removePaths(paths);

    // A theme switch replaces symlinks and files, so watch the parents too and
    // re-arm after every change. Watching the nearest existing ancestor means
    // installing Omarchy later works without restarting the app.
    QString ancestor = m_directory;
    while (!QFileInfo::exists(ancestor) && ancestor != "/")
        ancestor = QFileInfo(ancestor).absolutePath();
    QStringList candidates{ancestor, QFileInfo(ancestor).absolutePath(), m_directory,
                           m_directory + "/theme", m_directory + "/theme/colors.toml"};
    candidates.removeDuplicates();
    for (const auto &path : candidates)
        if (QFileInfo::exists(path))
            m_watcher.addPath(path);

    const QString tomlPath = m_directory + "/theme/colors.toml";
    const auto colors = aibox::ffi::theme_parse_colors_file(aibox::ffi::toRust(tomlPath));
    const auto accent = aibox::ffi::fromRust(colors.accent);
    const auto bg = aibox::ffi::fromRust(colors.background);
    const auto darkBg = aibox::ffi::fromRust(colors.dark_background);
    const auto lighterBg = aibox::ffi::fromRust(colors.lighter_background);
    const auto fg = aibox::ffi::fromRust(colors.foreground);
    const auto muted = aibox::ffi::fromRust(colors.muted);
    const auto green = aibox::ffi::fromRust(colors.green);
    const auto red = aibox::ffi::fromRust(colors.red);
    const auto yellow = aibox::ffi::fromRust(colors.yellow);

    if (accent != m_accent || bg != m_background || fg != m_foreground || red != m_red || green != m_green || muted != m_muted) {
        m_accent = accent;
        m_background = bg;
        m_darkBackground = darkBg;
        m_lighterBackground = lighterBg;
        m_foreground = fg;
        m_muted = muted;
        m_green = green;
        m_red = red;
        m_yellow = yellow;
        emit changed();
    }
}
