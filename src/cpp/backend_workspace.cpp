#include "backend.h"
#include "backend_helpers.h"
#include "rust_bridge.h"

#include "port_predicate.h"
#include "aibox-ffi/src/workspace_diff.rs.h"
#include "aibox-ffi/src/workspace_workflow.rs.h"

#include <QClipboard>
#include <QDateTime>
#include <QDesktopServices>
#include <QDir>
#include <QFileInfo>
#include <QGuiApplication>
#include <QImage>
#include <QMimeData>
#include <QStandardPaths>
#include <QThread>
#include <QUrl>

#include <csignal>

QString Backend::fileAccessDirPath() const {
    return aibox::ffi::fromRust(aibox::ffi::file_access_dir_path(aibox::ffi::toRust(m_stateDir)));
}

QString Backend::fileAccessStatePath(const QString &name) const {
    return aibox::ffi::fromRust(aibox::ffi::file_access_state_path(
        aibox::ffi::toRust(m_stateDir), aibox::ffi::toRust(name)));
}

QString Backend::fileAccessLogPath(const QString &name) const {
    return aibox::ffi::fromRust(aibox::ffi::file_access_log_path(
        aibox::ffi::toRust(m_stateDir), aibox::ffi::toRust(name)));
}

QVariantMap Backend::loadFileAccessState(const QString &name) const {
    const auto state = aibox::ffi::load_file_access_state(
        aibox::ffi::toRust(m_stateDir), aibox::ffi::toRust(name));
    if (!state.found)
        return {};
    QVariantMap map;
    if (!state.name.empty())
        map[QStringLiteral("name")] = aibox::ffi::fromRust(state.name);
    if (state.pid != 0)
        map[QStringLiteral("pid")] = static_cast<qint64>(state.pid);
    if (state.port != 0)
        map[QStringLiteral("port")] = state.port;
    if (!state.started_at.empty())
        map[QStringLiteral("startedAt")] = aibox::ffi::fromRust(state.started_at);
    return map;
}

bool Backend::fileAccessProcessAlive(qint64 pid, const QString &name) const {
    return aibox::ffi::file_access_process_alive(pid, aibox::ffi::toRust(name));
}

void Backend::stopFileAccessProcess(const QString &name) {
    const qint64 pid = aibox::ffi::stop_file_access_process(
        aibox::ffi::toRust(m_stateDir), aibox::ffi::toRust(name));
    if (pid > 0) {
        QTimer::singleShot(700, this, [stateDir = m_stateDir, pid, name] {
            aibox::ffi::kill_file_access_if_alive(pid, aibox::ffi::toRust(name), SIGKILL);
        });
    }
}

void Backend::applyFileAccessState(QVariantList &environments) {
    rust::Vec<aibox::ffi::FileAccessEnvInputFfi> inputs;
    inputs.reserve(static_cast<std::size_t>(environments.size()));
    for (const QVariant &item : environments) {
        const QVariantMap env = item.toMap();
        inputs.push_back(aibox::ffi::FileAccessEnvInputFfi{
            aibox::ffi::toRust(env.value("name").toString()),
            env.value("running").toBool(),
        });
    }
    const auto res = aibox::ffi::apply_file_access_state(
        aibox::ffi::toRust(m_stateDir), std::move(inputs));

    for (int i = 0; i < environments.size() && i < static_cast<int>(res.envs.size()); ++i) {
        QVariantMap env = environments.at(i).toMap();
        if (env.value("name").toString().isEmpty())
            continue;
        const auto &out = res.envs[static_cast<std::size_t>(i)];
        env["fileAccessActive"] = out.active;
        env["fileAccessPort"] = out.port;
        env["fileAccessUrl"] = aibox::ffi::fromRust(out.url);
        environments[i] = env;
    }
    for (const auto &k : res.pending_kills) {
        const qint64 pid = k.pid;
        const QString name = aibox::ffi::fromRust(k.name);
        QTimer::singleShot(700, this, [pid, name] {
            aibox::ffi::kill_file_access_if_alive(pid, aibox::ffi::toRust(name), SIGKILL);
        });
    }
}

QString Backend::fileAccessUrl(int port) {
    return aibox::ffi::fromRust(aibox::ffi::file_access_url(port));
}

int Backend::chooseFileAccessPort(const QSet<int> &reservedPorts,
                                  const std::function<bool(int)> &isAvailable,
                                  int startPort, int maxPort) {
    rust::Vec<std::int32_t> reserved;
    reserved.reserve(static_cast<std::size_t>(reservedPorts.size()));
    for (int port : reservedPorts)
        reserved.push_back(port);
    const bool hasPredicate = static_cast<bool>(isAvailable);
    const aibox::ffi::PortPredicate predicate{hasPredicate ? &isAvailable : nullptr};
    return aibox::ffi::choose_file_access_port(
        rust::Slice<const std::int32_t>(reserved.data(), reserved.size()),
        hasPredicate, predicate, startPort, maxPort);
}

QString Backend::enableFileAccess(const QString &name) {
    auto fail = [this](const QString &message) {
        setError(message);
        return message;
    };

    const QVariantMap env = name.isEmpty() ? QVariantMap{} : environmentByName(name);
    const QString preErr = aibox::ffi::fromRust(aibox::ffi::validate_enable_file_access(
        aibox::ffi::toRust(name), m_busy, !env.isEmpty(),
        env.value("isTemplate").toBool(), env.value("running").toBool()));
    if (!preErr.isEmpty())
        return fail(preErr);

    const QVariantMap existing = loadFileAccessState(name);
    if (fileAccessProcessAlive(existing.value(QStringLiteral("pid")).toLongLong(), name))
        return {};

    if (!QDir().mkpath(fileAccessDirPath()))
        return fail(tr("Could not create the root file access state directory: %1").arg(fileAccessDirPath()));

    QSet<int> reservedPorts;
    for (int port : aibox::ffi::collect_reserved_file_access_ports(aibox::ffi::toRust(m_stateDir)))
        reservedPorts.insert(port);

    const int port = chooseFileAccessPort(reservedPorts, [](int candidate) {
        return isHostPortAvailable(candidate);
    });
    if (port <= 0)
        return fail(tr("No free local port is available for root file access (tried 2222-2299)."));

    const QString logPath = fileAccessLogPath(name);
    const QString script = aibox::ffi::fromRust(aibox::ffi::build_file_access_mount_script(
        aibox::ffi::toRust(name), port, aibox::ffi::toRust(logPath)));
    qint64 pid = 0;
    if (!QProcess::startDetached(QStringLiteral("/bin/sh"), {QStringLiteral("-c"), script},
                                 QDir::homePath(), &pid) ||
        pid <= 0) {
        return fail(tr("Could not start the local SFTP listener."));
    }

    const QString startedAt = QDateTime::currentDateTimeUtc().toString(Qt::ISODate);
    const QString saveErr = aibox::ffi::fromRust(aibox::ffi::write_file_access_state(
        aibox::ffi::toRust(m_stateDir), aibox::ffi::toRust(name),
        pid, port, aibox::ffi::toRust(startedAt)));
    if (!saveErr.isEmpty()) {
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
        if (aibox::ffi::is_local_tcp_port_listening(port)) {
            listening = true;
            break;
        }
        QThread::msleep(60);
    }
    if (!listening) {
        const QString detail = aibox::ffi::fromRust(
            aibox::ffi::read_file_access_failure_detail(aibox::ffi::toRust(logPath)));
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

    const auto pathRes = aibox::ffi::prepare_pasted_image_path(aibox::ffi::toRust(workspacePath));
    if (!pathRes.error.empty()) {
        if (errorOut)
            *errorOut = aibox::ffi::fromRust(pathRes.error);
        return {};
    }
    const QString hostPath = aibox::ffi::fromRust(pathRes.value);
    if (!image.save(hostPath, "PNG")) {
        if (errorOut)
            *errorOut = tr("Could not save the clipboard image to '%1'.").arg(hostPath);
        return {};
    }

    aibox::ffi::prune_stale_pasted_images(aibox::ffi::toRust(workspacePath), 7);
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

    const QString containerPath = aibox::ffi::fromRust(aibox::ffi::pasted_image_container_path(
        aibox::ffi::toRust(containerWorkspacePathFor(envName)), aibox::ffi::toRust(hostPath)));

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
    const rust::Vec<rust::String> rForbidden = aibox::ffi::toRustVec(forbiddenConfigPaths);
    return aibox::ffi::fromRust(aibox::ffi::validate_extra_mount_path(
        aibox::ffi::toRust(rawPath),
        rust::Slice<const rust::String>(rForbidden.data(), rForbidden.size())));
}

QString Backend::validateExtraMountCandidate(const QString &rawPath) const {
    return validateExtraMountPath(rawPath, {QFileInfo(m_configPath).absolutePath(), m_stateDir});
}

QString Backend::extraMountDeviceName(const QString &rawName) {
    return aibox::ffi::fromRust(aibox::ffi::extra_mount_device_name(aibox::ffi::toRust(rawName)));
}

QString Backend::addExtraMountScript(const QString &name, const QString &deviceName,
                                     const QString &hostPath, const QString &containerPath, bool readOnly) {
    return aibox::ffi::fromRust(aibox::ffi::add_extra_mount_script(
        aibox::ffi::toRust(name), aibox::ffi::toRust(deviceName),
        aibox::ffi::toRust(hostPath), aibox::ffi::toRust(containerPath), readOnly));
}

QString Backend::addExtraMount(const QString &name, const QString &hostPath, const QString &mountName,
                               bool readOnly, bool confirmReadWrite) {
    const QVariantMap env = name.isEmpty() ? QVariantMap{} : environmentByName(name);
    const rust::Vec<rust::String> rForbidden = aibox::ffi::toRustVec(
        {QFileInfo(m_configPath).absolutePath(), m_stateDir});
    rust::Vec<aibox::ffi::ExtraMountPairFfi> existingMounts;
    const QVariantList mountList = env.value("extraMounts").toList();
    existingMounts.reserve(static_cast<std::size_t>(mountList.size()));
    for (const auto &v : mountList) {
        const QVariantMap m = v.toMap();
        existingMounts.push_back(aibox::ffi::ExtraMountPairFfi{
            aibox::ffi::toRust(m.value("deviceName").toString()),
            aibox::ffi::toRust(m.value("path").toString()),
        });
    }

    const auto prepared = aibox::ffi::prepare_add_extra_mount(
        aibox::ffi::toRust(name), m_busy, aibox::ffi::toRust(hostPath),
        aibox::ffi::toRust(mountName), readOnly, confirmReadWrite,
        rust::Slice<const rust::String>(rForbidden.data(), rForbidden.size()),
        !env.isEmpty(),
        std::move(existingMounts));
    if (!prepared.error.empty())
        return aibox::ffi::fromRust(prepared.error);

    runShellTask(tr("Mounting %1 at %2 (%3)").arg(
                     aibox::ffi::fromRust(prepared.resolved_source),
                     aibox::ffi::fromRust(prepared.container_path),
                     readOnly ? tr("RO") : tr("RW")),
                 aibox::ffi::fromRust(prepared.script),
                 {}, name);
    return {};
}

void Backend::removeExtraMount(const QString &name, const QString &deviceName) {
    if (name.isEmpty() || deviceName.isEmpty() || m_busy)
        return;
    runShellTask(tr("Removing mount '%1' on '%2'").arg(deviceName, name),
                 aibox::ffi::fromRust(aibox::ffi::remove_extra_mount_script(
                     aibox::ffi::toRust(name), aibox::ffi::toRust(deviceName))),
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
