#include "backend.h"
#include "backend_helpers.h"
#include "rust_bridge.h"

#include "aibox-ffi/src/workspace_diff.rs.h"

#include <QClipboard>
#include <QDateTime>
#include <QDesktopServices>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QImage>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMimeData>
#include <QRegularExpression>
#include <QSaveFile>
#include <QSet>
#include <QStandardPaths>
#include <QThread>
#include <QUrl>

#include <csignal>

namespace {

bool isLocalTcpPortListening(int port) {
    if (port < 1 || port > 65535)
        return false;
    QFile tcp(QStringLiteral("/proc/net/tcp"));
    if (!tcp.open(QIODevice::ReadOnly))
        return false;
    // /proc/net/tcp columns: sl, local_address (HEXADDR:HEXPORT), rem_address, st, ...
    // 127.0.0.1 in little-endian hex is 0100007F and st 0A means LISTEN.
    const QString needle = QStringLiteral("0100007F:%1").arg(port, 4, 16, QLatin1Char('0')).toUpper();
    const QStringList lines = QString::fromLatin1(tcp.readAll()).split(QLatin1Char('\n'), Qt::SkipEmptyParts);
    for (const QString &line : lines) {
        const QStringList fields = line.split(QRegularExpression(QStringLiteral("\\s+")), Qt::SkipEmptyParts);
        if (fields.size() < 4)
            continue;
        if (fields.at(1).compare(needle, Qt::CaseInsensitive) == 0 && fields.at(3) == QLatin1String("0A"))
            return true;
    }
    return false;
}

} // namespace

QString Backend::fileAccessDirPath() const {
    return m_stateDir + QStringLiteral("/file-access");
}

QString Backend::fileAccessStatePath(const QString &name) const {
    return fileAccessDirPath() + QLatin1Char('/') + sanitizeName(name) + QStringLiteral(".json");
}

QString Backend::fileAccessLogPath(const QString &name) const {
    return fileAccessDirPath() + QLatin1Char('/') + sanitizeName(name) + QStringLiteral(".log");
}

QVariantMap Backend::loadFileAccessState(const QString &name) const {
    QFile file(fileAccessStatePath(name));
    if (!file.open(QIODevice::ReadOnly))
        return {};
    const QJsonDocument doc = QJsonDocument::fromJson(file.readAll());
    return doc.isObject() ? doc.object().toVariantMap() : QVariantMap{};
}

bool Backend::fileAccessProcessAlive(qint64 pid, const QString &name) const {
    if (pid <= 0)
        return false;
    if (::kill(static_cast<pid_t>(pid), 0) != 0)
        return false;

    // Guard against PID reuse: only trust processes that still look like the
    // `incus file mount <name> ...` listener we started.
    QFile cmdline(QStringLiteral("/proc/%1/cmdline").arg(pid));
    if (!cmdline.open(QIODevice::ReadOnly))
        return false;
    const QList<QByteArray> args = cmdline.readAll().split('\0');
    bool hasFile = false;
    bool hasMount = false;
    bool hasName = false;
    for (const QByteArray &arg : args) {
        if (arg == "file")
            hasFile = true;
        else if (arg == "mount")
            hasMount = true;
        else if (QString::fromUtf8(arg) == name)
            hasName = true;
        else if (arg.contains("incus file mount") && arg.contains(name.toUtf8())) {
            // The detached `/bin/sh -c 'exec incus file mount ...'` wrapper is
            // still alive for a few milliseconds before exec replaces it.
            hasFile = true;
            hasMount = true;
            hasName = true;
        }
    }
    return hasFile && hasMount && hasName;
}

void Backend::stopFileAccessProcess(const QString &name) {
    if (name.isEmpty())
        return;
    const QVariantMap state = loadFileAccessState(name);
    const qint64 pid = state.value(QStringLiteral("pid")).toLongLong();
    if (fileAccessProcessAlive(pid, name)) {
        ::kill(static_cast<pid_t>(pid), SIGTERM);
        QTimer::singleShot(700, this, [this, pid, name] {
            if (fileAccessProcessAlive(pid, name))
                ::kill(static_cast<pid_t>(pid), SIGKILL);
        });
    }
    QFile::remove(fileAccessStatePath(name));
    QFile::remove(fileAccessLogPath(name));
}

void Backend::applyFileAccessState(QVariantList &environments) {
    QSet<QString> knownFiles;
    for (int i = 0; i < environments.size(); ++i) {
        QVariantMap env = environments.at(i).toMap();
        const QString name = env.value("name").toString();
        if (name.isEmpty())
            continue;
        knownFiles.insert(sanitizeName(name));

        const QVariantMap state = loadFileAccessState(name);
        const qint64 pid = state.value(QStringLiteral("pid")).toLongLong();
        int port = state.value(QStringLiteral("port")).toInt();
        bool active = fileAccessProcessAlive(pid, name);

        if (!active && !state.isEmpty()) {
            QFile::remove(fileAccessStatePath(name));
            QFile::remove(fileAccessLogPath(name));
            port = 0;
        }
        if (active && !env.value("running").toBool()) {
            // The environment stopped, so the listener no longer has a rootfs to serve.
            stopFileAccessProcess(name);
            active = false;
            port = 0;
        }

        env["fileAccessActive"] = active;
        env["fileAccessPort"] = active ? port : 0;
        env["fileAccessUrl"] = active ? fileAccessUrl(port) : QString();
        environments[i] = env;
    }

    // Drop state files whose environment disappeared (deleted outside AIBox).
    QDir dir(fileAccessDirPath());
    const QFileInfoList states = dir.entryInfoList({QStringLiteral("*.json")}, QDir::Files);
    for (const QFileInfo &info : states) {
        const QString base = info.completeBaseName();
        if (knownFiles.contains(base))
            continue;
        QFile stateFile(info.absoluteFilePath());
        QString stateName;
        qint64 pid = 0;
        if (stateFile.open(QIODevice::ReadOnly)) {
            const QJsonDocument doc = QJsonDocument::fromJson(stateFile.readAll());
            if (doc.isObject()) {
                stateName = doc.object().value(QStringLiteral("name")).toString();
                pid = doc.object().value(QStringLiteral("pid")).toVariant().toLongLong();
            }
        }
        if (stateName.isEmpty())
            stateName = base;
        if (fileAccessProcessAlive(pid, stateName))
            ::kill(static_cast<pid_t>(pid), SIGTERM);
        QFile::remove(info.absoluteFilePath());
        QFile::remove(fileAccessLogPath(stateName));
    }
}

QString Backend::fileAccessUrl(int port) {
    if (port < 1 || port > 65535)
        return {};
    return QStringLiteral("sftp://root@127.0.0.1:%1/").arg(port);
}

int Backend::chooseFileAccessPort(const QSet<int> &reservedPorts,
                                  const std::function<bool(int)> &isAvailable,
                                  int startPort, int maxPort) {
    for (int port = startPort; port <= maxPort; ++port) {
        if (reservedPorts.contains(port))
            continue;
        if (isAvailable && !isAvailable(port))
            continue;
        return port;
    }
    return 0;
}

QString Backend::enableFileAccess(const QString &name) {
    auto fail = [this](const QString &message) {
        setError(message);
        return message;
    };

    if (name.isEmpty())
        return fail(tr("Select an environment first."));
    if (m_busy)
        return fail(tr("Wait for the current operation to finish before changing root file access."));

    const QVariantMap env = environmentByName(name);
    if (env.isEmpty())
        return fail(tr("Environment '%1' was not found.").arg(name));
    if (env.value("isTemplate").toBool())
        return fail(tr("Templates cannot expose root files. Clone or unmark as template first."));
    if (!env.value("running").toBool())
        return fail(tr("Start '%1' before enabling root file access.").arg(name));

    const QVariantMap existing = loadFileAccessState(name);
    if (fileAccessProcessAlive(existing.value(QStringLiteral("pid")).toLongLong(), name))
        return {};

    if (!QDir().mkpath(fileAccessDirPath()))
        return fail(tr("Could not create the root file access state directory: %1").arg(fileAccessDirPath()));

    QSet<int> reservedPorts;
    QDir stateDir(fileAccessDirPath());
    const QFileInfoList states = stateDir.entryInfoList({QStringLiteral("*.json")}, QDir::Files);
    for (const QFileInfo &info : states) {
        const QVariantMap state = loadFileAccessState(info.completeBaseName());
        const int port = state.value(QStringLiteral("port")).toInt();
        if (port > 0)
            reservedPorts.insert(port);
    }

    const int port = chooseFileAccessPort(reservedPorts, [](int candidate) {
        return isHostPortAvailable(candidate);
    });
    if (port <= 0)
        return fail(tr("No free local port is available for root file access (tried 2222-2299)."));

    const QString logPath = fileAccessLogPath(name);
    const QString script = QStringLiteral("exec incus file mount %1 --listen %2 --no-auth > %3 2>&1")
                               .arg(shellQuote(name),
                                    shellQuote(QStringLiteral("127.0.0.1:%1").arg(port)),
                                    shellQuote(logPath));
    qint64 pid = 0;
    if (!QProcess::startDetached(QStringLiteral("/bin/sh"), {QStringLiteral("-c"), script},
                                 QDir::homePath(), &pid) ||
        pid <= 0) {
        return fail(tr("Could not start the local SFTP listener."));
    }

    QJsonObject state;
    state[QStringLiteral("name")] = name;
    state[QStringLiteral("pid")] = pid;
    state[QStringLiteral("port")] = port;
    state[QStringLiteral("startedAt")] = QDateTime::currentDateTimeUtc().toString(Qt::ISODate);
    QSaveFile stateFile(fileAccessStatePath(name));
    if (!stateFile.open(QIODevice::WriteOnly)) {
        ::kill(static_cast<pid_t>(pid), SIGTERM);
        return fail(tr("Could not save the root file access state file."));
    }
    stateFile.write(QJsonDocument(state).toJson(QJsonDocument::Compact));
    if (!stateFile.commit()) {
        ::kill(static_cast<pid_t>(pid), SIGTERM);
        return fail(tr("Could not save the root file access state file."));
    }

    // Wait briefly for the listener to bind so the UI never hands out a dead URL.
    bool listening = false;
    QElapsedTimer waitTimer;
    waitTimer.start();
    while (waitTimer.elapsed() < 3000) {
        // Check the process first: if incus failed to bind, it exits and we must
        // not mistake someone else's late listener on that port for ours.
        if (!fileAccessProcessAlive(pid, name))
            break;
        if (isLocalTcpPortListening(port)) {
            listening = true;
            break;
        }
        QThread::msleep(60);
    }
    if (!listening) {
        QString detail;
        QFile log(logPath);
        if (log.open(QIODevice::ReadOnly)) {
            const QStringList lines = QString::fromUtf8(log.readAll())
                                          .split(QRegularExpression(QStringLiteral("[\\r\\n]+")),
                                                 Qt::SkipEmptyParts);
            if (!lines.isEmpty())
                detail = lines.last().trimmed();
        }
        stopFileAccessProcess(name);
        return fail(detail.isEmpty()
                        ? tr("Root file access failed to start on 127.0.0.1:%1.").arg(port)
                        : tr("Root file access failed to start: %1").arg(detail));
    }

    for (int i = 0; i < m_environments.size(); ++i) {
        QVariantMap item = m_environments.at(i).toMap();
        if (item.value("name").toString() != name)
            continue;
        item["fileAccessActive"] = true;
        item["fileAccessPort"] = port;
        item["fileAccessUrl"] = fileAccessUrl(port);
        m_environments[i] = item;
        emit environmentsChanged();
        emit selectedIndexChanged();
        break;
    }

    m_statusMessage = tr("Root file access enabled: %1").arg(fileAccessUrl(port));
    emit statusMessageChanged();
    setError(QString());
    QTimer::singleShot(3000, this, [this] {
        if (!m_busy) {
            m_statusMessage.clear();
            emit statusMessageChanged();
        }
    });

    return {};
}

void Backend::disableFileAccess(const QString &name) {
    if (name.isEmpty())
        return;
    stopFileAccessProcess(name);
    for (int i = 0; i < m_environments.size(); ++i) {
        QVariantMap env = m_environments.at(i).toMap();
        if (env.value("name").toString() != name)
            continue;
        env["fileAccessActive"] = false;
        env["fileAccessPort"] = 0;
        env["fileAccessUrl"] = QString();
        m_environments[i] = env;
        emit environmentsChanged();
        emit selectedIndexChanged();
        break;
    }
    m_statusMessage = tr("Root file access disabled for '%1'.").arg(name);
    emit statusMessageChanged();
    QTimer::singleShot(3000, this, [this] {
        if (!m_busy) {
            m_statusMessage.clear();
            emit statusMessageChanged();
        }
    });
}

void Backend::openFileAccess(const QString &name) {
    if (name.isEmpty())
        return;
    const QVariantMap env = environmentByName(name);
    const QString url = env.value("fileAccessUrl").toString();
    if (url.isEmpty())
        return;

    // Prefer a registered sftp:// scheme handler (GVFS/Nautilus, KIO, etc.).
    QProcess gio;
    QProcessEnvironment gioEnv = QProcessEnvironment::systemEnvironment();
    gioEnv.insert(QStringLiteral("LC_ALL"), QStringLiteral("C"));
    gio.setProcessEnvironment(gioEnv);
    gio.start(QStringLiteral("gio"),
              {QStringLiteral("mime"), QStringLiteral("x-scheme-handler/sftp")});
    const bool hasHandler = gio.waitForFinished(1500) && gio.exitCode() == 0 &&
                            !QString::fromUtf8(gio.readAllStandardOutput())
                                 .contains(QLatin1String("No default applications"));
    if (hasHandler && QDesktopServices::openUrl(QUrl(url)))
        return;

    // Nautilus and most file managers accept an sftp:// URI directly even when
    // the desktop environment has no x-scheme-handler registered for sftp.
    for (const QString &fileManager : {QStringLiteral("nautilus"), QStringLiteral("dolphin"),
                                       QStringLiteral("thunar"), QStringLiteral("nemo"),
                                       QStringLiteral("pcmanfm"), QStringLiteral("caja")}) {
        if (QStandardPaths::findExecutable(fileManager).isEmpty())
            continue;
        QStringList args;
        if (fileManager == QLatin1String("nautilus"))
            args << QStringLiteral("-w");
        args << url;
        if (QProcess::startDetached(fileManager, args))
            return;
    }

    copyText(url, tr("SFTP URL"));
    setError(tr("No file manager accepted the SFTP URL; it has been copied to the clipboard."));
}

QString Backend::storePastedImage(const QImage &image, const QString &workspacePath,
                                  QString *errorOut) {
    if (image.isNull()) {
        if (errorOut)
            *errorOut = tr("The clipboard does not contain an image. Copy a screenshot or an image file first.");
        return {};
    }

    const QString pasteDir = QDir(workspacePath).filePath(QStringLiteral(".aibox-paste"));
    if (!QDir().mkpath(pasteDir)) {
        if (errorOut)
            *errorOut = tr("Could not create the paste folder '%1'.").arg(pasteDir);
        return {};
    }

    const QString stamp = QDateTime::currentDateTime().toString(QStringLiteral("yyyyMMdd-HHmmss"));
    QString fileName = QStringLiteral("paste-%1.png").arg(stamp);
    for (int suffix = 2; QFileInfo::exists(QDir(pasteDir).filePath(fileName)); ++suffix)
        fileName = QStringLiteral("paste-%1-%2.png").arg(stamp).arg(suffix);
    const QString hostPath = QDir(pasteDir).filePath(fileName);
    if (!image.save(hostPath, "PNG")) {
        if (errorOut)
            *errorOut = tr("Could not save the clipboard image to '%1'.").arg(hostPath);
        return {};
    }

    // Keep the paste folder small: drop images older than a week.
    const QDateTime cutoff = QDateTime::currentDateTime().addDays(-7);
    const QFileInfoList stale = QDir(pasteDir).entryInfoList(
        {QStringLiteral("paste-*.png")}, QDir::Files, QDir::Time | QDir::Reversed);
    for (const QFileInfo &entry : stale) {
        if (entry.lastModified() < cutoff)
            QFile::remove(entry.absoluteFilePath());
    }

    return hostPath;
}

QString Backend::pasteImageToWorkspace(const QString &name) {
    const QString envName = name.trimmed();
    if (envName.isEmpty()) {
        setError(tr("Select an environment before pasting an image."));
        return {};
    }
    if (environmentByName(envName).isEmpty()) {
        setError(tr("The environment '%1' is not available.").arg(envName));
        return {};
    }

    auto *cb = QGuiApplication::clipboard();
    if (!cb) {
        setError(tr("The clipboard is not available in this session."));
        return {};
    }

    QImage image = cb->image();
    if (image.isNull() && cb->mimeData()) {
        const QList<QUrl> urls = cb->mimeData()->urls();
        for (const QUrl &url : urls) {
            if (!url.isLocalFile())
                continue;
            QImage loaded(url.toLocalFile());
            if (!loaded.isNull()) {
                image = loaded;
                break;
            }
        }
    }
    if (image.isNull()) {
        setError(tr("The clipboard does not contain an image. Copy a screenshot or an image file first."));
        return {};
    }

    QString saveError;
    const QString hostPath = storePastedImage(image, workspacePathFor(envName), &saveError);
    if (hostPath.isEmpty()) {
        setError(saveError);
        return {};
    }

    QString containerRoot = containerWorkspacePathFor(envName);
    while (containerRoot.endsWith(QLatin1Char('/')))
        containerRoot.chop(1);
    const QString containerPath = containerRoot + QStringLiteral("/.aibox-paste/")
                                  + QFileInfo(hostPath).fileName();

    cb->setText(containerPath);
    m_statusMessage = tr("Saved clipboard image as '%1'. Path is on the clipboard; paste it in the terminal.")
                          .arg(containerPath);
    emit statusMessageChanged();
    QTimer::singleShot(6000, this, [this] {
        if (!m_busy) {
            m_statusMessage.clear();
            emit statusMessageChanged();
        }
    });
    return containerPath;
}

QString Backend::validateExtraMountPath(const QString &rawPath, const QStringList &forbiddenConfigPaths) {
    QString trimmed = rawPath.trimmed();
    if (trimmed.isEmpty())
        return tr("Choose a host directory to mount.");
    if (trimmed == QLatin1String("~"))
        trimmed = QDir::homePath();
    else if (trimmed.startsWith(QLatin1String("~/")))
        trimmed = QDir::homePath() + trimmed.mid(1);

    const QFileInfo rawInfo(trimmed);
    if (!rawInfo.exists() || !rawInfo.isDir())
        return tr("Host path '%1' does not exist or is not a directory.").arg(trimmed);

    QString canonical = rawInfo.canonicalFilePath();
    if (canonical.isEmpty())
        canonical = QDir::cleanPath(rawInfo.absoluteFilePath());

    const QString homeClean = QDir::cleanPath(QDir::homePath());
    static const QSet<QString> exactBlocked = {
        QStringLiteral("/"), QStringLiteral("/bin"), QStringLiteral("/boot"),
        QStringLiteral("/dev"), QStringLiteral("/etc"), QStringLiteral("/home"),
        QStringLiteral("/lib"), QStringLiteral("/lib64"), QStringLiteral("/proc"),
        QStringLiteral("/root"), QStringLiteral("/run"), QStringLiteral("/sbin"),
        QStringLiteral("/sys"), QStringLiteral("/usr"), QStringLiteral("/var")
    };
    if (exactBlocked.contains(canonical) || canonical == homeClean) {
        return tr("Refusing to mount system root or home directory '%1'. Pick a specific project/data subfolder.").arg(canonical);
    }

    static const QStringList prefixBlocked = {
        QStringLiteral("/etc/"), QStringLiteral("/boot/"), QStringLiteral("/dev/"),
        QStringLiteral("/proc/"), QStringLiteral("/sys/"), QStringLiteral("/root/"),
        QStringLiteral("/run/"), QStringLiteral("/bin/"), QStringLiteral("/sbin/"),
        QStringLiteral("/lib/"), QStringLiteral("/lib64/"), QStringLiteral("/usr/")
    };
    for (const QString &prefix : prefixBlocked) {
        if (canonical.startsWith(prefix))
            return tr("Refusing to mount system path '%1'.").arg(canonical);
    }

    static const QSet<QString> sensitiveComponents = {
        QStringLiteral(".ssh"), QStringLiteral(".gnupg"), QStringLiteral(".aws"),
        QStringLiteral(".kube"), QStringLiteral(".docker"), QStringLiteral(".aibox"),
        QStringLiteral(".password-store")
    };
    const QStringList parts = canonical.split(QLatin1Char('/'), Qt::SkipEmptyParts);
    for (const QString &part : parts) {
        if (sensitiveComponents.contains(part))
            return tr("Refusing to mount sensitive directory component '%1'.").arg(part);
    }

    for (const QString &cfg : forbiddenConfigPaths) {
        const QString cleanCfg = QDir::cleanPath(cfg);
        if (!cleanCfg.isEmpty() &&
            (canonical == cleanCfg || canonical.startsWith(cleanCfg + QLatin1Char('/')))) {
            return tr("Refusing to mount AIBox configuration directory '%1'.").arg(canonical);
        }
    }
    return {};
}

QString Backend::validateExtraMountCandidate(const QString &rawPath) const {
    return validateExtraMountPath(rawPath, {QFileInfo(m_configPath).absolutePath(), m_stateDir});
}

QString Backend::extraMountDeviceName(const QString &rawName) {
    const QString clean = sanitizeName(rawName);
    return clean.isEmpty() ? QString() : QStringLiteral("mnt-") + clean;
}

QString Backend::addExtraMountScript(const QString &name, const QString &deviceName,
                                     const QString &hostPath, const QString &containerPath, bool readOnly) {
    const QString roFlag = readOnly ? QStringLiteral("true") : QStringLiteral("false");
    return QStringLiteral("set -euo pipefail\n"
                          "echo '@@STAGE:45:Adding extra mount (%4)...'\n"
                          "if ! incus config device add %1 %2 disk source=%3 path=%4 readonly=%5 shift=true >/dev/null 2>&1; then\n"
                          "    incus config device add %1 %2 disk source=%3 path=%4 readonly=%5\n"
                          "fi\n"
                          "echo '@@STAGE:100:Mount attached.'")
        .arg(shellQuote(name), shellQuote(deviceName), shellQuote(hostPath), shellQuote(containerPath), roFlag);
}

QString Backend::addExtraMount(const QString &name, const QString &hostPath, const QString &mountName,
                               bool readOnly, bool confirmReadWrite) {
    if (name.isEmpty())
        return tr("Select an environment first.");
    if (m_busy)
        return tr("Another operation is currently in progress.");
    if (!readOnly && !confirmReadWrite)
        return tr("Read-write extra mounts require explicit second confirmation.");

    const QString cleanMount = sanitizeName(mountName.isEmpty() ? QFileInfo(hostPath.trimmed()).fileName() : mountName);
    if (cleanMount.isEmpty() || cleanMount == QLatin1String("workspace") ||
        cleanMount == QLatin1String("root") || cleanMount == QLatin1String("x11")) {
        return tr("Choose a valid mount name (cannot be 'workspace', 'root', or 'x11').");
    }

    QString expanded = hostPath.trimmed();
    if (expanded == QLatin1String("~"))
        expanded = QDir::homePath();
    else if (expanded.startsWith(QLatin1String("~/")))
        expanded = QDir::homePath() + expanded.mid(1);

    const QString pathErr = validateExtraMountCandidate(expanded);
    if (!pathErr.isEmpty())
        return pathErr;

    const QString resolvedSource = QFileInfo(expanded).canonicalFilePath();
    const QString devName = extraMountDeviceName(cleanMount);
    const QString containerPath = kContainerWorkspacePath + QLatin1Char('/') + cleanMount;

    const QVariantMap env = environmentByName(name);
    if (env.isEmpty())
        return tr("Environment '%1' was not found.").arg(name);
    for (const auto &v : env.value("extraMounts").toList()) {
        const QVariantMap m = v.toMap();
        if (m.value("deviceName").toString() == devName || m.value("path").toString() == containerPath)
            return tr("Mount '%1' already exists on '%2'.").arg(containerPath, name);
    }

    runShellTask(tr("Mounting %1 at %2 (%3)").arg(resolvedSource, containerPath, readOnly ? tr("RO") : tr("RW")),
                 addExtraMountScript(name, devName, resolvedSource, containerPath, readOnly),
                 {}, name);
    return {};
}

void Backend::removeExtraMount(const QString &name, const QString &deviceName) {
    if (name.isEmpty() || deviceName.isEmpty() || m_busy)
        return;
    runShellTask(tr("Removing mount '%1' on '%2'").arg(deviceName, name),
                 QStringLiteral("set -euo pipefail\n"
                                "echo '@@STAGE:50:Detaching extra mount...'\n"
                                "incus config device remove %1 %2\n"
                                "echo '@@STAGE:100:Mount removed.'")
                     .arg(shellQuote(name), shellQuote(deviceName)),
                 {}, name);
}

QStringList Backend::defaultDiffExcludes() {
    return aibox::ffi::fromRustVec(aibox::ffi::default_diff_excludes());
}

QStringList Backend::parseDiffExcludes(const QString &rawText) {
    return aibox::ffi::fromRustVec(aibox::ffi::parse_diff_excludes(aibox::ffi::toRust(rawText)));
}

QStringList Backend::buildRsyncExcludeArgs(const QStringList &excludes) {
    const rust::Vec<rust::String> rExcludes = aibox::ffi::toRustVec(excludes);
    return aibox::ffi::fromRustVec(aibox::ffi::build_rsync_exclude_args(
        rust::Slice<const rust::String>(rExcludes.data(), rExcludes.size())));
}

bool Backend::isSafeRelativePath(const QString &relativePath) {
    return aibox::ffi::is_safe_relative_path(aibox::ffi::toRust(relativePath));
}

bool Backend::isLikelyBinaryData(const QByteArray &sample) {
    return aibox::ffi::is_likely_binary_data(aibox::ffi::toRustBytes(sample));
}

QString Backend::capDiffLines(const QString &diffOutput, int maxLines, bool *outTruncated) {
    const auto res = aibox::ffi::cap_diff_lines(aibox::ffi::toRust(diffOutput), maxLines);
    if (outTruncated)
        *outTruncated = res.truncated;
    return aibox::ffi::fromRust(res.text);
}

QVariantMap Backend::parseRsyncItemize(const QString &output, int maxEntries) {
    return aibox::ffi::fromRustJson(
               aibox::ffi::parse_rsync_itemize_json(aibox::ffi::toRust(output), maxEntries))
        .toMap();
}

QStringList Backend::buildWorkspaceDiffArgs(const QString &snapPath, const QString &wsPath,
                                             const QStringList &excludes) {
    const rust::Vec<rust::String> rExcludes = aibox::ffi::toRustVec(excludes);
    return aibox::ffi::fromRustVec(aibox::ffi::build_workspace_diff_args(
        aibox::ffi::toRust(snapPath), aibox::ffi::toRust(wsPath),
        rust::Slice<const rust::String>(rExcludes.data(), rExcludes.size())));
}

QVariantMap Backend::processWorkspaceDiffOutput(const QString &snapshotName, const QString &wsPath,
                                                const QString &rsyncStdout, QVariantList *entriesOut) {
    const QVariantMap res = aibox::ffi::fromRustJson(
                                aibox::ffi::process_workspace_diff_output_json(
                                    aibox::ffi::toRust(snapshotName),
                                    aibox::ffi::toRust(wsPath),
                                    aibox::ffi::toRust(rsyncStdout)))
                                .toMap();
    if (entriesOut)
        *entriesOut = res.value(QStringLiteral("entries")).toList();
    return res.value(QStringLiteral("summary")).toMap();
}

void Backend::loadWorkspaceDiff(const QString &name, const QString &snapshotName) {
    if (name.isEmpty() || snapshotName.isEmpty())
        return;

    m_selectedDiffText.clear();
    m_selectedDiffInfo.clear();
    emit selectedDiffChanged();

    if (m_demoMode) {
        m_diffLoading = false;
        m_diffEntries = {
            QVariantMap{{"path", QStringLiteral("lib/main.dart")}, {"status", QStringLiteral("modified")}, {"type", QStringLiteral("file")}, {"sizeBytes", 4120}, {"sizeText", QStringLiteral("4 KB")}},
            QVariantMap{{"path", QStringLiteral("lib/services/auth.dart")}, {"status", QStringLiteral("added")}, {"type", QStringLiteral("file")}, {"sizeBytes", 1840}, {"sizeText", QStringLiteral("2 KB")}},
            QVariantMap{{"path", QStringLiteral("lib/legacy_helper.dart")}, {"status", QStringLiteral("deleted")}, {"type", QStringLiteral("file")}, {"sizeBytes", 920}, {"sizeText", QStringLiteral("1 KB")}}
        };
        m_diffSummary = QVariantMap{
            {"snapshotName", snapshotName},
            {"addedCount", 1},
            {"modifiedCount", 1},
            {"deletedCount", 1},
            {"totalCount", 3},
            {"truncated", false},
            {"error", QString()}
        };
        emit workspaceDiffChanged();
        return;
    }

    const QVariantMap env = environmentByName(name);
    const QString wsPath = env.value("workspacePath").toString().isEmpty()
                               ? workspacePathFor(name)
                               : env.value("workspacePath").toString();
    const QString snapDir = workspaceSnapshotDir(wsPath, name);
    const QString snapPath = snapDir + QLatin1Char('/') + snapshotName;

    if (snapDir.isEmpty() || !QFileInfo(snapPath).isDir()) {
        m_diffLoading = false;
        m_diffEntries.clear();
        m_diffSummary = QVariantMap{
            {"snapshotName", snapshotName},
            {"addedCount", 0},
            {"modifiedCount", 0},
            {"deletedCount", 0},
            {"totalCount", 0},
            {"truncated", false},
            {"error", tr("Workspace snapshot '%1' was not found.").arg(snapshotName)}
        };
        emit workspaceDiffChanged();
        return;
    }

    if (m_diffProcess && m_diffProcess->state() != QProcess::NotRunning) {
        m_diffTimeoutTimer.stop();
        m_diffProcess->disconnect(this);
        m_diffProcess->kill();
        m_diffProcess->waitForFinished(1000);
    }
    if (!m_diffProcess)
        m_diffProcess = new QProcess(this);
    else
        m_diffProcess->disconnect(this);

    m_diffLoading = true;
    m_diffEntries.clear();
    m_diffSummary = QVariantMap{
        {"snapshotName", snapshotName},
        {"addedCount", 0},
        {"modifiedCount", 0},
        {"deletedCount", 0},
        {"totalCount", 0},
        {"truncated", false},
        {"error", QString()}
    };
    emit workspaceDiffChanged();

    m_diffProcess->setProperty("timedOut", false);
    connect(m_diffProcess, &QProcess::finished, this, [this, snapshotName, wsPath](int code, QProcess::ExitStatus) {
        m_diffTimeoutTimer.stop();
        m_diffLoading = false;
        const bool timedOut = m_diffProcess->property("timedOut").toBool();
        if (timedOut) {
            m_diffEntries.clear();
            m_diffSummary = QVariantMap{
                {"snapshotName", snapshotName},
                {"addedCount", 0},
                {"modifiedCount", 0},
                {"deletedCount", 0},
                {"totalCount", 0},
                {"truncated", false},
                {"error", tr("Diff scan timed out after 30 seconds.")}
            };
            emit workspaceDiffChanged();
            return;
        }
        if (code != 0 && code != 24) {
            const QString errTxt = QString::fromUtf8(m_diffProcess->readAllStandardError()).trimmed();
            m_diffEntries.clear();
            m_diffSummary = QVariantMap{
                {"snapshotName", snapshotName},
                {"addedCount", 0},
                {"modifiedCount", 0},
                {"deletedCount", 0},
                {"totalCount", 0},
                {"truncated", false},
                {"error", errTxt.isEmpty() ? tr("Failed to compare workspace with snapshot.") : errTxt}
            };
            emit workspaceDiffChanged();
            return;
        }

        const QString out = QString::fromUtf8(m_diffProcess->readAllStandardOutput());
        QVariantList entries;
        const QVariantMap summary = processWorkspaceDiffOutput(snapshotName, wsPath, out, &entries);
        m_diffEntries = entries;
        m_diffSummary = summary;
        emit workspaceDiffChanged();
    });

    const QStringList args = buildWorkspaceDiffArgs(snapPath, wsPath, m_diffExcludes);

    m_diffTimeoutTimer.start();
    m_diffProcess->start(QStringLiteral("rsync"), args);
}

void Backend::loadFileDiff(const QString &name, const QString &snapshotName,
                           const QString &relativePath, const QString &status) {
    if (name.isEmpty() || snapshotName.isEmpty() || !isSafeRelativePath(relativePath)) {
        m_selectedDiffText.clear();
        m_selectedDiffInfo = tr("Invalid file path.");
        emit selectedDiffChanged();
        return;
    }

    if (m_demoMode) {
        if (status == QLatin1String("added")) {
            m_selectedDiffText = QStringLiteral("--- snapshot\n+++ current\n@@ -0,0 +1,4 @@\n+class AuthService {\n+  Future<bool> signIn() async => true;\n+}\n");
        } else if (status == QLatin1String("deleted")) {
            m_selectedDiffText = QStringLiteral("--- snapshot\n+++ current\n@@ -1,3 +0,0 @@\n-String legacyHelper() {\n-  return 'deprecated';\n-}\n");
        } else {
            m_selectedDiffText = QStringLiteral("--- snapshot\n+++ current\n@@ -10,4 +10,5 @@\n void main() {\n-  runApp(const OldApp());\n+  // Updated by AI agent\n+  runApp(const NewApp());\n }\n");
        }
        m_selectedDiffInfo = relativePath;
        emit selectedDiffChanged();
        return;
    }

    const QVariantMap env = environmentByName(name);
    const QString wsPath = env.value("workspacePath").toString().isEmpty()
                               ? workspacePathFor(name)
                               : env.value("workspacePath").toString();

    const auto prep = aibox::ffi::prepare_file_diff_ffi(
        aibox::ffi::toRust(name), aibox::ffi::toRust(snapshotName),
        aibox::ffi::toRust(wsPath), aibox::ffi::toRust(relativePath),
        aibox::ffi::toRust(status));
    if (!prep.run_diff) {
        m_selectedDiffText = aibox::ffi::fromRust(prep.text);
        m_selectedDiffInfo = aibox::ffi::fromRust(prep.info);
        emit selectedDiffChanged();
        return;
    }

    if (m_fileDiffProcess && m_fileDiffProcess->state() != QProcess::NotRunning) {
        m_fileDiffProcess->disconnect(this);
        m_fileDiffProcess->kill();
        m_fileDiffProcess->waitForFinished(500);
    }
    if (!m_fileDiffProcess)
        m_fileDiffProcess = new QProcess(this);
    else
        m_fileDiffProcess->disconnect(this);

    connect(m_fileDiffProcess, &QProcess::finished, this, [this, relativePath](int code, QProcess::ExitStatus) {
        const QString raw = QString::fromUtf8(m_fileDiffProcess->readAllStandardOutput());
        const auto shaped = aibox::ffi::shape_file_diff_output_ffi(
            code, aibox::ffi::toRust(raw), aibox::ffi::toRust(relativePath));
        m_selectedDiffText = aibox::ffi::fromRust(shaped.text);
        m_selectedDiffInfo = aibox::ffi::fromRust(shaped.info);
        emit selectedDiffChanged();
    });

    m_fileDiffProcess->start(QStringLiteral("diff"), aibox::ffi::fromRustVec(prep.args));
}

QString Backend::restoreSingleFile(const QString &name, const QString &snapshotName,
                                   const QString &relativePath) {
    if (name.isEmpty() || snapshotName.isEmpty() || !isSafeRelativePath(relativePath))
        return tr("Invalid file restore request.");

    if (m_demoMode) {
        recordOperationHistory(tr("Restored file '%1' from snapshot '%2'").arg(relativePath, snapshotName),
                               name, QStringLiteral("success"), 0,
                               QDateTime::currentDateTimeUtc().toString(Qt::ISODate),
                               QByteArrayLiteral("Single-file restore (demo)\n"));
        m_statusMessage = tr("[Demo] Restored '%1' from '%2'.").arg(relativePath, snapshotName);
        emit statusMessageChanged();
        return {};
    }

    const QVariantMap env = environmentByName(name);
    const QString wsPath = env.value("workspacePath").toString().isEmpty()
                               ? workspacePathFor(name)
                               : env.value("workspacePath").toString();

    const auto res = aibox::ffi::restore_single_file_ffi(
        aibox::ffi::toRust(name), aibox::ffi::toRust(snapshotName),
        aibox::ffi::toRust(wsPath), aibox::ffi::toRust(relativePath));
    if (!res.ok)
        return aibox::ffi::fromRust(res.message);

    const QString logMsg = aibox::ffi::fromRust(res.message);
    recordOperationHistory(tr("Restored file '%1' from snapshot '%2'").arg(relativePath, snapshotName),
                           name, QStringLiteral("success"), 0,
                           QDateTime::currentDateTimeUtc().toString(Qt::ISODate),
                           logMsg.toUtf8());
    m_statusMessage = tr("Restored '%1' from snapshot '%2'.").arg(relativePath, snapshotName);
    emit statusMessageChanged();
    loadWorkspaceDiff(name, snapshotName);
    return {};
}
