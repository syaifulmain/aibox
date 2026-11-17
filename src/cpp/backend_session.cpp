#include "backend.h"
#include "backend_helpers.h"
#include "rust_bridge.h"

#include "aibox-ffi/src/history.rs.h"
#include "aibox-ffi/src/session.rs.h"

#include <QDir>
#include <QFileInfo>
#include <QJsonDocument>
#include <QStandardPaths>

QString Backend::historyDirPath() const {
    return aibox::ffi::fromRust(aibox::ffi::history_dir_path(aibox::ffi::toRust(m_stateDir)));
}

QString Backend::historySeenMarkerPath() const {
    return aibox::ffi::fromRust(aibox::ffi::history_seen_marker_path(aibox::ffi::toRust(m_stateDir)));
}

void Backend::loadHistorySeenMarker() {
    m_historySeenId = aibox::ffi::fromRust(
        aibox::ffi::load_history_seen_marker(aibox::ffi::toRust(m_stateDir)));
}

QByteArray Backend::truncateLogTail(const QByteArray &data, qint64 maxBytes) {
    return aibox::ffi::fromRustBytes(
        aibox::ffi::truncate_log_tail(aibox::ffi::toRustBytes(data), maxBytes));
}

int Backend::recentFailedHistoryCount() const {
    const QByteArray entriesJson =
        QJsonDocument::fromVariant(m_operationHistory).toJson(QJsonDocument::Compact);
    return aibox::ffi::recent_failed_history_count(
        rust::Str(entriesJson.constData(), static_cast<std::size_t>(entriesJson.size())),
        aibox::ffi::toRust(m_historySeenId));
}

void Backend::loadOperationHistory() {
    const QVariantList loaded = aibox::ffi::fromRustJson(
        aibox::ffi::load_operation_history_json(aibox::ffi::toRust(m_stateDir))).toList();
    if (loaded != m_operationHistory) {
        m_operationHistory = loaded;
        emit operationHistoryChanged();
    }
}

void Backend::recordHistory(const QString &description, const QString &envName, const QString &status,
                            int exitCode, const QString &startedAtIso, const QByteArray &logBytes) {
    recordOperationHistory(description, envName, status, exitCode, startedAtIso, logBytes);
}

void Backend::recordOperationHistory(const QString &description, const QString &envName, const QString &status,
                                     int exitCode, const QString &startedAtIso, const QByteArray &logBytes,
                                     const QString &reason) {
    const auto secrets = aibox::ffi::toRustVec(allSecretValues());
    const auto res = aibox::ffi::record_operation_history_json(
        aibox::ffi::toRust(m_stateDir),
        m_historyMaxEntries,
        aibox::ffi::toRust(description),
        aibox::ffi::toRust(envName),
        aibox::ffi::toRust(status),
        exitCode,
        aibox::ffi::toRust(startedAtIso),
        aibox::ffi::toRustBytes(logBytes),
        aibox::ffi::toRust(reason),
        rust::Slice<const rust::String>(secrets.data(), secrets.size()));
    if (!res.ok)
        return;

    const QVariantList loaded = aibox::ffi::fromRustJson(res.entries_json).toList();
    if (loaded != m_operationHistory) {
        m_operationHistory = loaded;
        emit operationHistoryChanged();
    }
}

QString Backend::readOperationLog(const QString &id) const {
    const auto res = aibox::ffi::read_operation_log(
        aibox::ffi::toRust(m_stateDir), aibox::ffi::toRust(id), m_demoMode);
    if (!res.found)
        return tr("Log file is not available.");
    return aibox::ffi::fromRust(res.text);
}

void Backend::clearOperationHistory() {
    aibox::ffi::clear_operation_history(aibox::ffi::toRust(m_stateDir));
    m_historySeenId.clear();
    if (!m_operationHistory.isEmpty()) {
        m_operationHistory.clear();
        emit operationHistoryChanged();
    }
}

void Backend::acknowledgeOperationHistory() {
    if (m_operationHistory.isEmpty())
        return;
    const QString newestId = m_operationHistory.first().toMap().value("id").toString();
    const QString updated = aibox::ffi::fromRust(aibox::ffi::acknowledge_operation_history(
        aibox::ffi::toRust(m_stateDir),
        aibox::ffi::toRust(newestId),
        aibox::ffi::toRust(m_historySeenId)));
    if (updated.isEmpty())
        return;
    m_historySeenId = updated;
    emit operationHistoryChanged();
}

QString Backend::sessionsDirPath() const {
    return aibox::ffi::fromRust(aibox::ffi::sessions_dir_path(aibox::ffi::toRust(m_stateDir)));
}

bool Backend::prepareRecordedSessionPaths(const QString &envName, QString *outLogFile, QString *outTimingFile) const {
    const auto res = aibox::ffi::prepare_recorded_session_paths(
        aibox::ffi::toRust(m_stateDir), aibox::ffi::toRust(envName));
    if (!res.ok)
        return false;
    if (outLogFile)
        *outLogFile = aibox::ffi::fromRust(res.log_file);
    if (outTimingFile)
        *outTimingFile = aibox::ffi::fromRust(res.timing_file);
    return true;
}

bool Backend::effectiveRecordSessions(bool globalEnabled, const QString &envOverride) {
    return aibox::ffi::effective_record_sessions(globalEnabled, aibox::ffi::toRust(envOverride));
}

void Backend::setEnvironmentRecordSessions(const QString &name, const QString &mode) {
    if (name.isEmpty() || m_busy)
        return;
    const QString norm = mode.trimmed().toLower();
    const QString stored = (norm == QLatin1String("true"))
                               ? QStringLiteral("true")
                               : (norm == QLatin1String("false") ? QStringLiteral("false") : QStringLiteral("inherit"));
    runShellTask(tr("Updating session recording for '%1'").arg(name),
                 QStringLiteral("echo '@@STAGE:50:Updating session recording policy...'; "
                                "incus config set %1 user.aibox.record %2; "
                                "echo '@@STAGE:100:Updated.'")
                     .arg(shellQuote(name), shellQuote(stored)),
                 {}, name);
}

QString Backend::buildSessionWrapCommand(const QString &innerCommand, const QString &logFile,
                                         const QString &timingFile,
                                         const QString &redactBinPath,
                                         const QString &sessionsDir,
                                         bool scriptAvailable) {
    return aibox::ffi::fromRust(aibox::ffi::build_session_wrap_command(
        aibox::ffi::toRust(innerCommand),
        aibox::ffi::toRust(logFile),
        aibox::ffi::toRust(timingFile),
        aibox::ffi::toRust(redactBinPath),
        aibox::ffi::toRust(sessionsDir),
        scriptAvailable));
}

QString Backend::stripAnsiSequences(const QString &rawText) {
    return aibox::ffi::fromRust(aibox::ffi::strip_ansi_sequences(aibox::ffi::toRust(rawText)));
}

QString Backend::formatRecordedSessionText(const QByteArray &rawBytes, int maxBytes) {
    return aibox::ffi::fromRust(
        aibox::ffi::format_recorded_session_text(aibox::ffi::toRustBytes(rawBytes), maxBytes));
}

bool Backend::redactSessionLogFile(const QString &logFilePath, const QStringList &secretValues) {
    const auto secrets = aibox::ffi::toRustVec(secretValues);
    return aibox::ffi::redact_session_log_file(
        aibox::ffi::toRust(logFilePath),
        rust::Slice<const rust::String>(secrets.data(), secrets.size()));
}

int Backend::pruneRecordedSessions(const QString &sessionsDir, int maxPerEnv, qint64 maxTotalBytes) {
    return aibox::ffi::prune_recorded_sessions(
        aibox::ffi::toRust(sessionsDir), maxPerEnv, maxTotalBytes);
}

QVariantList Backend::scanRecordedSessions(const QString &sessionsDir) {
    return aibox::ffi::fromRustJson(
               aibox::ffi::scan_recorded_sessions_json(aibox::ffi::toRust(sessionsDir)))
        .toList();
}

void Backend::refreshRecordedSessions() {
    if (m_demoMode) {
        emit recordedSessionsChanged();
        return;
    }

    const QString sDir = sessionsDirPath();
    const QStringList secretVals = allSecretValues();
    if (!secretVals.isEmpty() && QDir(sDir).exists()) {
        const QVariantList preScan = scanRecordedSessions(sDir);
        for (const QVariant &v : preScan)
            redactSessionLogFile(v.toMap().value(QStringLiteral("logPath")).toString(), secretVals);
    }

    pruneRecordedSessions(sDir);
    const QVariantList scanned = scanRecordedSessions(sDir);
    if (scanned != m_recordedSessions) {
        m_recordedSessions = scanned;
        emit recordedSessionsChanged();
    }
}

QString Backend::readRecordedSessionLog(const QString &logPath) const {
    const auto res = aibox::ffi::read_recorded_session_log(aibox::ffi::toRust(logPath), m_demoMode);
    if (!res.found)
        return tr("Session log is not available.");
    return aibox::ffi::fromRust(res.text);
}

void Backend::replayRecordedSession(const QString &logPath, const QString &timingPath) {
    if (m_demoMode) {
        m_statusMessage = tr("[Demo] Replaying recorded session in terminal with scriptreplay.");
        emit statusMessageChanged();
        return;
    }
    if (logPath.trimmed().isEmpty() || timingPath.trimmed().isEmpty() ||
        !QFileInfo::exists(logPath) || !QFileInfo::exists(timingPath)) {
        setError(tr("Recorded session or timing file is missing."));
        return;
    }
    if (QStandardPaths::findExecutable(QStringLiteral("scriptreplay")).isEmpty()) {
        setError(tr("scriptreplay (util-linux) is not installed on the host."));
        return;
    }
    const QString replayCmd = aibox::ffi::fromRust(aibox::ffi::build_replay_command(
        aibox::ffi::toRust(logPath), aibox::ffi::toRust(timingPath)));
    launchCommandInTerminal(replayCmd);
}

QString Backend::deleteRecordedSession(const QString &logPath, const QString &timingPath) {
    if (m_demoMode) {
        QVariantList next;
        for (const QVariant &v : m_recordedSessions) {
            if (v.toMap().value(QStringLiteral("logPath")).toString() != logPath)
                next.append(v);
        }
        m_recordedSessions = next;
        emit recordedSessionsChanged();
        return {};
    }

    const auto res = aibox::ffi::delete_recorded_session(
        aibox::ffi::toRust(sessionsDirPath()),
        aibox::ffi::toRust(logPath),
        aibox::ffi::toRust(timingPath));
    if (!res.ok)
        return tr("Refusing to delete session outside %1.").arg(aibox::ffi::fromRust(res.invalid_root));

    refreshRecordedSessions();
    return {};
}
