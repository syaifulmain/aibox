#include "backend.h"
#include "backend_helpers.h"
#include "rust_bridge.h"

#include "aibox-ffi/src/incus.rs.h"
#include "aibox-ffi/src/provision.rs.h"

#include <QClipboard>
#include <QCoreApplication>
#include <QDateTime>
#include <QDesktopServices>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QHostAddress>
#include <QHostInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QRegularExpression>
#include <QStandardPaths>
#include <QThread>
#include <QUrl>
#include <QUuid>

#include <cmath>

using aibox::ffi::fromRust;
using aibox::ffi::fromRustVec;
using aibox::ffi::toRust;
using aibox::ffi::toRustVec;

QVariantList Backend::parseIncusListJson(const QByteArray &json, const QString &workspaceBase,
                                         QHash<QString, qint64> *cpuSnapshotNs, qint64 elapsedWallNs,
                                         bool defaultAutoSnapshot, bool defaultAutoStop,
                                         bool defaultRecordSessions,
                                         QHash<QString, QPair<qint64, qint64>> *netCounterSnapshot) {
    std::vector<aibox::ffi::CpuSnapshotEntryFfi> cpuIn;
    if (cpuSnapshotNs) {
        cpuIn.reserve(cpuSnapshotNs->size());
        for (auto it = cpuSnapshotNs->constBegin(); it != cpuSnapshotNs->constEnd(); ++it)
            cpuIn.push_back({toRust(it.key()), it.value()});
    }

    std::vector<aibox::ffi::NetCounterEntryFfi> netIn;
    if (netCounterSnapshot) {
        netIn.reserve(netCounterSnapshot->size());
        for (auto it = netCounterSnapshot->constBegin(); it != netCounterSnapshot->constEnd(); ++it)
            netIn.push_back({toRust(it.key()), it.value().first, it.value().second});
    }

    const bool track = (cpuSnapshotNs != nullptr) || (netCounterSnapshot != nullptr);
    const auto res = aibox::ffi::parse_incus_list_stateless(
        rust::Slice<const uint8_t>(reinterpret_cast<const uint8_t *>(json.constData()),
                                   static_cast<size_t>(json.size())),
        toRust(workspaceBase),
        rust::Slice<const aibox::ffi::CpuSnapshotEntryFfi>(cpuIn.data(), cpuIn.size()),
        rust::Slice<const aibox::ffi::NetCounterEntryFfi>(netIn.data(), netIn.size()),
        elapsedWallNs,
        defaultAutoSnapshot,
        defaultAutoStop,
        defaultRecordSessions,
        track);

    if (cpuSnapshotNs) {
        cpuSnapshotNs->clear();
        for (const auto &entry : res.cpu_snapshots)
            cpuSnapshotNs->insert(fromRust(entry.name), entry.usage_ns);
    }
    if (netCounterSnapshot) {
        netCounterSnapshot->clear();
        for (const auto &entry : res.net_counters)
            netCounterSnapshot->insert(fromRust(entry.name), qMakePair(entry.rx_bytes, entry.tx_bytes));
    }

    return aibox::ffi::fromRustJson(res.envs_json).toList();
}

void Backend::createEnvironment(const QString &rawName, const QString &osId, const QString &presetId,
                                const QString &diskGiB, const QString &ramGiB, int cpuCores,
                                const QString &processLimit, const QString &ioLimit,
                                const QString &workspaceParent) {
    QStringList existingNames;
    existingNames.reserve(m_environments.size());
    for (const auto &v : m_environments)
        existingNames << v.toMap().value("name").toString();

    std::vector<aibox::ffi::OsEntryInputFfi> osEntries;
    osEntries.reserve(m_osEntries.size());
    for (const auto &v : m_osEntries) {
        const QVariantMap m = v.toMap();
        osEntries.push_back({toRust(m.value("id").toString()), toRust(m.value("alias").toString())});
    }

    std::vector<aibox::ffi::PresetInputFfi> presets;
    presets.reserve(m_presets.size());
    for (const auto &v : m_presets) {
        const QVariantMap m = v.toMap();
        presets.push_back({toRust(m.value("id").toString()), toRust(m.value("flags").toString()),
                           m.value("x11").toBool()});
    }

    const auto existingRust = toRustVec(existingNames);
    const auto plan = aibox::ffi::prepare_create_environment(
        m_busy, toRust(rawName), rust::Slice<const rust::String>(existingRust.data(), existingRust.size()),
        toRust(osId), rust::Slice<const aibox::ffi::OsEntryInputFfi>(osEntries.data(), osEntries.size()),
        toRust(presetId), toRust(m_defaultPresetId),
        rust::Slice<const aibox::ffi::PresetInputFfi>(presets.data(), presets.size()),
        toRust(diskGiB), toRust(ramGiB), cpuCores, toRust(processLimit), toRust(ioLimit),
        toRust(workspaceParent), toRust(m_workspaceBase), toRust(m_osStoragePath));
    if (!plan.ok) {
        setError(fromRust(plan.error));
        return;
    }

    const QString name = fromRust(plan.name);
    runShellTask(fromRust(plan.title), fromRust(plan.script), name, name);
}

QString Backend::createEnvironmentScript(const QString &name, const QString &localAlias, const QString &remoteImage,
                                         const QString &workspace, const QString &cacheBase,
                                         const QString &profileFlags, const QString &ramGiB, const QString &diskGiB,
                                         int cpuCores, bool x11, const QString &processLimit, const QString &ioLimit,
                                         bool isVm) {
    return fromRust(aibox::ffi::create_environment_script(
        toRust(name), toRust(localAlias), toRust(remoteImage), toRust(workspace), toRust(cacheBase),
        toRust(profileFlags), toRust(ramGiB), toRust(diskGiB), cpuCores, x11,
        toRust(processLimit), toRust(ioLimit), isVm));
}

QVariantList Backend::templates() const {
    QVariantList result;
    for (const auto &v : m_environments) {
        const QVariantMap env = v.toMap();
        if (env.value("isTemplate").toBool())
            result.append(env);
    }
    return result;
}

QVariantMap Backend::environmentByName(const QString &name) const {
    for (const auto &v : m_environments) {
        const QVariantMap env = v.toMap();
        if (env.value("name").toString() == name)
            return env;
    }
    return {};
}

QString Backend::cloneEnvironmentScript(const QString &sourceName, const QString &newName,
                                        const QString &workspace, const QString &diskGiB,
                                        const QString &ramGiB, int cpuCores, bool startAfter,
                                        const QString &netMode, const QString &netAllowlist,
                                        const QStringList &resolvedCidrs, const QString &processLimit,
                                        const QString &ioLimit, bool isVm) {
    const auto cidrsRust = toRustVec(resolvedCidrs);
    return fromRust(aibox::ffi::clone_environment_script(
        toRust(sourceName), toRust(newName), toRust(workspace), toRust(diskGiB),
        toRust(ramGiB), cpuCores, startAfter, toRust(netMode), toRust(netAllowlist),
        rust::Slice<const rust::String>(cidrsRust.data(), cidrsRust.size()),
        toRust(processLimit), toRust(ioLimit), isVm));
}

QString Backend::prepareEnvironmentScript(const QString &name) {
    return buildPrepareScript(name, buildAllowlistRefreshIfNeeded(name));
}

QString Backend::buildPrepareScript(const QString &name, const QString &allowRefresh) {
    const QString autoSnap = buildAutoSnapshotIfNeeded(name);
    return fromRust(aibox::ffi::build_prepare_script(toRust(name), toRust(autoSnap), toRust(allowRefresh)));
}

QStringList Backend::enterIncusExecArgs(const QString &name, const QString &remoteEnvPath) const {
    return fromRustVec(aibox::ffi::enter_incus_exec_args(
        toRust(name), toRust(remoteEnvPath), toRust(containerWorkspacePathFor(name)),
        toRust(qEnvironmentVariable("DISPLAY")), toRust(qEnvironmentVariable("TERM"))));
}

QStringList Backend::execIncusCommandArgs(const QString &name, const QStringList &cmd,
                                          const QString &remoteEnvPath) const {
    const auto cmdRust = toRustVec(cmd);
    return fromRustVec(aibox::ffi::exec_incus_command_args(
        toRust(name), rust::Slice<const rust::String>(cmdRust.data(), cmdRust.size()),
        toRust(remoteEnvPath), toRust(containerWorkspacePathFor(name))));
}

QString Backend::startEnvironmentScript(const QString &name) {
    const QString autoSnap = buildAutoSnapshotIfNeeded(name);
    const QString allowRefresh = buildAllowlistRefreshIfNeeded(name);
    return fromRust(aibox::ffi::start_environment_script(
        toRust(name), toRust(autoSnap), toRust(allowRefresh)));
}

QString Backend::stopEnvironmentScript(const QString &name, bool includeAutoSnapshot) {
    const QString autoSnap = includeAutoSnapshot ? buildAutoSnapshotIfNeeded(name) : QString();
    return fromRust(aibox::ffi::stop_environment_script(
        toRust(name), includeAutoSnapshot, toRust(autoSnap)));
}

void Backend::enterEnvironment(const QString &name) {
    if (name.isEmpty())
        return;
    const QVariantMap env = environmentByName(name);
    if (env.value("isTemplate").toBool()) {
        setError(tr("Templates cannot be entered directly. Create a new environment from this template first."));
        return;
    }
    if (m_enterPrepareProcess && m_enterPrepareProcess->state() != QProcess::NotRunning)
        return;
    if (m_enterSecretProcess && m_enterSecretProcess->state() != QProcess::NotRunning)
        return;

    m_enterEnvName = name;
    m_enterAllowlistSnippet.clear();
    m_enterHostSecretFile.clear();
    m_enterRemoteEnvPath.clear();
    if (m_statusMessage != tr("Preparing environment '%1'...").arg(name)) {
        m_statusMessage = tr("Preparing environment '%1'...").arg(name);
        emit statusMessageChanged();
    }
    beginEnterPrepare(name);
}

void Backend::beginEnterPrepare(const QString &name) {
    const QVariantMap env = environmentByName(name);
    const QString rawAllow = env.value(QStringLiteral("networkAllowlist")).toString();
    const QStringList entries = parseAllowlistEntries(rawAllow);
    if (env.value(QStringLiteral("networkMode")).toString() != QLatin1String("allowlist") || entries.isEmpty()) {
        startEnterPrepareProcess();
        return;
    }

    // Hostname lookups must not block the GUI thread, so resolve them first.
    resolveEnterAllowlistHosts(entries, [this, rawAllow](const QStringList &cidrs) {
        if (m_enterEnvName.isEmpty())
            return;
        const QVariantMap current = environmentByName(m_enterEnvName);
        if (!cidrs.isEmpty()) {
            m_enterAllowlistSnippet = aclApplyShellSnippet(m_enterEnvName, rawAllow, cidrs,
                                                           current.value("egressRecording").toBool());
        }
        startEnterPrepareProcess();
    });
}

void Backend::resolveEnterAllowlistHosts(const QStringList &entries,
                                         const std::function<void(const QStringList &)> &done) {
    const rust::Vec<rust::String> entriesVec = aibox::ffi::toRustVec(entries);
    const QStringList hostnames = aibox::ffi::fromRustVec(aibox::ffi::extract_allowlist_hostnames(
        rust::Slice<const rust::String>(entriesVec.data(), entriesVec.size())));

    auto results = std::make_shared<QHash<QString, QStringList>>();
    auto finish = [entries, results, done]() {
        done(resolveAllowlistCidrs(entries, [results](const QString &host) {
            return results->value(host);
        }));
    };
    if (hostnames.isEmpty()) {
        finish();
        return;
    }

    auto pending = std::make_shared<int>(hostnames.size());
    for (const QString &host : hostnames) {
        QHostInfo::lookupHost(host, this,
                              [host, pending, results, finish](const QHostInfo &info) {
            QStringList ips;
            for (const QHostAddress &addr : info.addresses()) {
                if (addr.protocol() == QAbstractSocket::IPv4Protocol)
                    ips.append(addr.toString());
            }
            results->insert(host, ips);
            if (--(*pending) == 0)
                finish();
        });
    }
}

void Backend::startEnterPrepareProcess() {
    if (m_enterEnvName.isEmpty())
        return;
    if (!m_enterPrepareProcess) {
        m_enterPrepareProcess = new QProcess(this);
        m_enterPrepareProcess->setProcessChannelMode(QProcess::MergedChannels);
        connect(m_enterPrepareProcess, QOverload<int, QProcess::ExitStatus>::of(&QProcess::finished),
                this, [this](int exitCode, QProcess::ExitStatus) { handleEnterPrepared(exitCode); });
        connect(m_enterPrepareProcess, &QProcess::errorOccurred, this, [this](QProcess::ProcessError error) {
            if (error == QProcess::FailedToStart)
                finishEnter(tr("Could not start environment preparation."));
        });
    }
    if (m_enterPrepareProcess->state() != QProcess::NotRunning)
        return;
    m_enterPrepareProcess->start(QStringLiteral("/bin/bash"),
                                 {QStringLiteral("-c"), buildPrepareScript(m_enterEnvName, m_enterAllowlistSnippet)});
}

void Backend::handleEnterPrepared(int exitCode) {
    if (m_enterEnvName.isEmpty())
        return;
    const QByteArray preOut = m_enterPrepareProcess->readAllStandardOutput();
    if (preOut.contains("Warning:")) {
        recordOperationHistory(tr("Auto-snapshot warning for '%1'").arg(m_enterEnvName), m_enterEnvName,
                               QStringLiteral("warning"), exitCode,
                               QDateTime::currentDateTimeUtc().toString(Qt::ISODate), preOut);
    }
    pushEnterSecrets();
}

void Backend::pushEnterSecrets() {
    if (m_enterEnvName.isEmpty())
        return;
    QString secretErr;
    const QList<QPair<QString, QString>> pairs = resolveEnvironmentSecrets(m_enterEnvName, &secretErr);
    if (!secretErr.isEmpty()) {
        finishEnter(secretErr);
        return;
    }
    if (pairs.isEmpty()) {
        launchEnterTerminal(QString());
        return;
    }

    QString tmpErr;
    m_enterHostSecretFile = createHostTempSecretFile(buildSecretEnvFileContent(pairs), &tmpErr);
    if (m_enterHostSecretFile.isEmpty()) {
        finishEnter(tmpErr);
        return;
    }
    m_enterRemoteEnvPath = QStringLiteral("/dev/shm/aibox-%1.env")
                               .arg(QUuid::createUuid().toString(QUuid::Id128));

    if (!m_enterSecretProcess) {
        m_enterSecretProcess = new QProcess(this);
        connect(m_enterSecretProcess, QOverload<int, QProcess::ExitStatus>::of(&QProcess::finished),
                this, [this](int exitCode, QProcess::ExitStatus status) {
            handleEnterSecretsPushed(exitCode, status);
        });
    }
    m_enterSecretProcess->start(QStringLiteral("incus"),
                                {QStringLiteral("file"), QStringLiteral("push"),
                                 QStringLiteral("--mode"), QStringLiteral("0400"),
                                 m_enterHostSecretFile, m_enterEnvName + m_enterRemoteEnvPath});
}

void Backend::handleEnterSecretsPushed(int exitCode, QProcess::ExitStatus status) {
    QString errText;
    if (exitCode != 0 || status != QProcess::NormalExit) {
        errText = QString::fromUtf8(m_enterSecretProcess->readAllStandardError()).trimmed();
        if (errText.isEmpty())
            errText = tr("Failed to inject secrets into container.");
    }
    if (!m_enterHostSecretFile.isEmpty())
        QFile::remove(m_enterHostSecretFile);
    m_enterHostSecretFile.clear();
    if (!errText.isEmpty()) {
        finishEnter(errText);
        return;
    }
    launchEnterTerminal(m_enterRemoteEnvPath);
}

QString Backend::buildEnterCommand(const QString &name, const QString &remoteEnvPath) const {
    return fromRust(aibox::ffi::build_enter_command(toRust(name), toRust(remoteEnvPath)));
}

void Backend::launchEnterTerminal(const QString &remoteEnvPath) {
    const QString name = m_enterEnvName;
    if (name.isEmpty())
        return;

    QString enterCmd = buildEnterCommand(name, remoteEnvPath);
    const QVariantMap env = environmentByName(name);
    if (env.value(QStringLiteral("recordEnabled")).toBool()) {
        QString logFile;
        QString timingFile;
        if (prepareRecordedSessionPaths(name, &logFile, &timingFile)) {
            const bool hasScript = !QStandardPaths::findExecutable(QStringLiteral("script")).isEmpty();
            enterCmd = buildSessionWrapCommand(enterCmd, logFile, timingFile,
                                               QCoreApplication::applicationFilePath(),
                                               sessionsDirPath(), hasScript);
        }
    }

    m_enterEnvName.clear();
    m_enterAllowlistSnippet.clear();
    m_enterRemoteEnvPath.clear();
    m_statusMessage.clear();
    emit statusMessageChanged();
    launchCommandInTerminal(enterCmd);
    refresh();
}

void Backend::finishEnter(const QString &errorMessage) {
    if (!m_enterHostSecretFile.isEmpty())
        QFile::remove(m_enterHostSecretFile);
    m_enterHostSecretFile.clear();
    m_enterRemoteEnvPath.clear();
    m_enterEnvName.clear();
    m_enterAllowlistSnippet.clear();
    m_statusMessage.clear();
    emit statusMessageChanged();
    if (!errorMessage.isEmpty())
        setError(errorMessage);
    refresh();
}

void Backend::openWorkspaceFolder(const QString &name) {
    if (name.isEmpty())
        return;
    const QString path = workspacePathFor(name);
    QDir().mkpath(path);
    QDesktopServices::openUrl(QUrl::fromLocalFile(path));
}

void Backend::toggleStartStop(const QString &name) {
    if (name.isEmpty() || m_busy)
        return;
    const QVariantMap env = environmentByName(name);
    const bool isTemplate = env.value("isTemplate").toBool();
    const bool isRunning = env.value("running").toBool();
    const QString autoSnap = isTemplate ? QString() : buildAutoSnapshotIfNeeded(name);
    const QString allowRefresh = (!isTemplate && !isRunning) ? buildAllowlistRefreshIfNeeded(name) : QString();

    const auto plan = aibox::ffi::prepare_toggle_start_stop(
        toRust(name), isTemplate, isRunning, toRust(autoSnap), toRust(allowRefresh));
    if (!plan.ok) {
        setError(fromRust(plan.error));
        return;
    }

    if (plan.is_stopping)
        stopFileAccessProcess(name);
    runShellTask(fromRust(plan.title), fromRust(plan.script), {}, name);
}

void Backend::cloneEnvironment(const QString &sourceName, const QString &rawNewName,
                               const QString &diskGiB, const QString &ramGiB,
                               int cpuCores, bool startAfter,
                               const QString &processLimit, const QString &ioLimit,
                               const QString &workspaceParent) {
    const QVariantMap source = environmentByName(sourceName);
    const bool sourceExists = !source.isEmpty();
    const QString netMode = source.value("networkMode").toString();
    const QString netAllow = source.value("networkAllowlist").toString();
    const QString sourceType = source.value("type").toString();

    QStringList existingNames;
    existingNames.reserve(m_environments.size());
    for (const auto &v : m_environments)
        existingNames << v.toMap().value("name").toString();

    QStringList resolvedCidrs;
    if (!m_busy && sourceExists && netMode == QLatin1String("allowlist"))
        resolvedCidrs = resolveAllowlistCidrs(parseAllowlistEntries(netAllow));

    const auto existingRust = toRustVec(existingNames);
    const auto cidrsRust = toRustVec(resolvedCidrs);
    const auto plan = aibox::ffi::prepare_clone_environment(
        m_busy, toRust(sourceName), sourceExists, toRust(netMode), toRust(netAllow),
        toRust(sourceType), toRust(rawNewName),
        rust::Slice<const rust::String>(existingRust.data(), existingRust.size()),
        toRust(diskGiB), toRust(ramGiB), cpuCores, startAfter,
        rust::Slice<const rust::String>(cidrsRust.data(), cidrsRust.size()),
        toRust(processLimit), toRust(ioLimit), toRust(workspaceParent), toRust(m_workspaceBase));
    if (!plan.ok) {
        setError(fromRust(plan.error));
        return;
    }

    const QString newName = fromRust(plan.name);
    runShellTask(fromRust(plan.title), fromRust(plan.script), newName, newName);
}

void Backend::setEnvironmentTemplate(const QString &name, bool isTemplate) {
    if (name.isEmpty() || m_busy)
        return;
    const QVariantMap env = environmentByName(name);
    if (env.isEmpty())
        return;

    const auto plan = aibox::ffi::set_environment_template_script(
        toRust(name), env.value("running").toBool(), isTemplate);
    runShellTask(fromRust(plan.title), fromRust(plan.script));
}

QString Backend::aiExecCommandText(const QString &name) const {
    if (name.isEmpty())
        return {};
    const QVariantMap env = environmentByName(name);
    const bool hasSecrets = !env.value("secretNames").toStringList().isEmpty();
    const QString wsMount = env.value(QStringLiteral("workspaceMountPath")).toString();
    return fromRust(aibox::ffi::ai_exec_command_text(toRust(name), hasSecrets, toRust(wsMount)));
}

void Backend::copyAiCommand(const QString &name) {
    if (name.isEmpty())
        return;
    if (auto *cb = QGuiApplication::clipboard()) {
        cb->setText(aiExecCommandText(name));
        m_statusMessage = tr("Copied AI exec command for '%1' to clipboard.").arg(name);
        emit statusMessageChanged();
        QTimer::singleShot(3000, this, [this] {
            if (!m_busy) {
                m_statusMessage.clear();
                emit statusMessageChanged();
            }
        });
    }
}
