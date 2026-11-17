#include "backend.h"
#include "backend_helpers.h"

#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonObject>
#include <QRegularExpression>
#include <QSaveFile>
#include <QStandardPaths>

#include <algorithm>

QString Backend::historyDirPath() const {
    return m_stateDir + QStringLiteral("/history");
}

QString Backend::historySeenMarkerPath() const {
    return m_stateDir + QStringLiteral("/history-seen");
}

void Backend::loadHistorySeenMarker() {
    QFile f(historySeenMarkerPath());
    if (!f.open(QIODevice::ReadOnly)) {
        m_historySeenId.clear();
        return;
    }
    m_historySeenId = QString::fromUtf8(f.readAll()).trimmed();
}

QByteArray Backend::truncateLogTail(const QByteArray &data, qint64 maxBytes) {
    if (maxBytes <= 0 || data.size() <= maxBytes)
        return data;
    QByteArray tail = data.right(maxBytes);
    const int newlineIdx = tail.indexOf('\n');
    if (newlineIdx > 0 && newlineIdx < tail.size() - 1)
        tail = tail.mid(newlineIdx + 1);
    return QByteArray("... [truncated] ...\n") + tail;
}

int Backend::recentFailedHistoryCount() const {
    int count = 0;
    for (const auto &v : m_operationHistory) {
        const QVariantMap entry = v.toMap();
        if (entry.value("status").toString() != QLatin1String("failed"))
            continue;
        if (!m_historySeenId.isEmpty()
            && entry.value("id").toString() <= m_historySeenId)
            continue;
        ++count;
    }
    return count;
}

void Backend::loadOperationHistory() {
    QVariantList loaded;
    const QDir dir(historyDirPath());
    if (dir.exists()) {
        const QFileInfoList files = dir.entryInfoList({QStringLiteral("*.json")}, QDir::Files, QDir::Name | QDir::Reversed);
        for (const QFileInfo &fi : files) {
            QFile f(fi.absoluteFilePath());
            if (!f.open(QIODevice::ReadOnly))
                continue;
            const QJsonObject obj = QJsonDocument::fromJson(f.readAll()).object();
            if (obj.value("id").toString().isEmpty())
                continue;
            loaded.append(obj.toVariantMap());
        }
    }
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
    if (description.isEmpty())
        return;
    const QString dirPath = historyDirPath();
    if (!QDir().mkpath(dirPath))
        return;

    const QDateTime now = QDateTime::currentDateTime();
    const QDateTime started = QDateTime::fromString(startedAtIso, Qt::ISODate);
    const qint64 durationMs = started.isValid() ? qMax<qint64>(0, started.msecsTo(now)) : 0;

    static int seq = 0;
    const QString id = QStringLiteral("%1-%2").arg(now.toString(QStringLiteral("yyyyMMdd-HHmmss-zzz"))).arg(++seq, 3, 10, QLatin1Char('0'));
    const QString logName = id + QStringLiteral(".log");

    const QStringList secretVals = allSecretValues();
    const QString safeDescription = redactSecrets(description, secretVals);
    const QByteArray safeLogBytes = redactSecretsBytes(logBytes, secretVals);

    QFile outLog(dirPath + QLatin1Char('/') + logName);
    if (outLog.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        outLog.write(truncateLogTail(safeLogBytes));
        outLog.close();
    }

    QJsonObject entry;
    entry["id"] = id;
    entry["timestamp"] = now.toString(QStringLiteral("yyyy-MM-dd HH:mm:ss"));
    entry["startedAt"] = startedAtIso;
    entry["durationMs"] = durationMs;
    entry["description"] = safeDescription;
    entry["envName"] = envName;
    entry["status"] = status;
    entry["exitCode"] = exitCode;
    entry["logFile"] = logName;
    if (!reason.isEmpty())
        entry["reason"] = reason;

    QFile outMeta(dirPath + QLatin1Char('/') + id + QStringLiteral(".json"));
    if (outMeta.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        outMeta.write(QJsonDocument(entry).toJson(QJsonDocument::Compact));
        outMeta.close();
    }

    // Prune oldest beyond m_historyMaxEntries
    const QDir dir(dirPath);
    const QFileInfoList allJson = dir.entryInfoList({QStringLiteral("*.json")}, QDir::Files, QDir::Name);
    if (allJson.size() > m_historyMaxEntries) {
        const int removeCount = allJson.size() - m_historyMaxEntries;
        for (int i = 0; i < removeCount; ++i) {
            const QString base = allJson.at(i).completeBaseName();
            QFile::remove(dirPath + QLatin1Char('/') + base + QStringLiteral(".json"));
            QFile::remove(dirPath + QLatin1Char('/') + base + QStringLiteral(".log"));
        }
    }

    loadOperationHistory();
}

QString Backend::readOperationLog(const QString &id) const {
    const QString safeId = QFileInfo(id).fileName();
    if (safeId.isEmpty())
        return {};
    if (m_demoMode) {
        return QStringLiteral("[demo] Task %1 completed cleanly.\n@@STAGE:100:Done.").arg(safeId);
    }
    QFile f(historyDirPath() + QLatin1Char('/') + safeId + QStringLiteral(".log"));
    if (!f.open(QIODevice::ReadOnly))
        return tr("Log file is not available.");
    return QString::fromUtf8(f.readAll());
}

void Backend::clearOperationHistory() {
    const QDir dir(historyDirPath());
    if (dir.exists()) {
        for (const QFileInfo &fi : dir.entryInfoList(QDir::Files))
            QFile::remove(fi.absoluteFilePath());
    }
    QFile::remove(historySeenMarkerPath());
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
    if (newestId.isEmpty() || newestId == m_historySeenId)
        return;
    QFile f(historySeenMarkerPath());
    if (f.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        f.write(newestId.toUtf8());
        f.close();
    } else {
        return;
    }
    m_historySeenId = newestId;
    emit operationHistoryChanged();
}

QString Backend::sessionsDirPath() const {
    return m_stateDir + QStringLiteral("/sessions");
}

bool Backend::prepareRecordedSessionPaths(const QString &envName, QString *outLogFile, QString *outTimingFile) const {
    const QString cleanEnv = sanitizeName(envName);
    if (cleanEnv.isEmpty())
        return false;
    const QString envDir = sessionsDirPath() + QLatin1Char('/') + cleanEnv;
    if (!QDir().mkpath(envDir))
        return false;
    QFile::setPermissions(sessionsDirPath(),
                          QFileDevice::ReadOwner | QFileDevice::WriteOwner | QFileDevice::ExeOwner);
    QFile::setPermissions(envDir,
                          QFileDevice::ReadOwner | QFileDevice::WriteOwner | QFileDevice::ExeOwner);

    qint64 epoch = QDateTime::currentSecsSinceEpoch();
    QString base = QString::number(epoch);
    while (QFileInfo::exists(envDir + QLatin1Char('/') + base + QStringLiteral(".log"))) {
        ++epoch;
        base = QString::number(epoch);
    }
    if (outLogFile)
        *outLogFile = envDir + QLatin1Char('/') + base + QStringLiteral(".log");
    if (outTimingFile)
        *outTimingFile = envDir + QLatin1Char('/') + base + QStringLiteral(".timing");
    return true;
}

bool Backend::effectiveRecordSessions(bool globalEnabled, const QString &envOverride) {
    const QString ov = envOverride.trimmed().toLower();
    if (ov == QLatin1String("true") || ov == QLatin1String("1"))
        return true;
    if (ov == QLatin1String("false") || ov == QLatin1String("0"))
        return false;
    return globalEnabled;
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
    if (!scriptAvailable || innerCommand.trimmed().isEmpty() ||
        logFile.trimmed().isEmpty() || timingFile.trimmed().isEmpty()) {
        return innerCommand;
    }

    const QString logDir = QFileInfo(logFile).absolutePath();
    const QString qLogDir = shellQuote(logDir);
    const QString qLog = shellQuote(logFile);
    const QString qTiming = shellQuote(timingFile);
    const QString qInner = shellQuote(innerCommand);

    QString postCmd;
    if (!redactBinPath.trimmed().isEmpty()) {
        postCmd = QStringLiteral("    %1 --redact-session %2%3 >/dev/null 2>&1 || true\n")
                      .arg(shellQuote(redactBinPath),
                           qLog,
                           sessionsDir.trimmed().isEmpty()
                               ? QString()
                               : QStringLiteral(" ") + shellQuote(sessionsDir));
    }

    return QStringLiteral(
        "if command -v script >/dev/null 2>&1; then\n"
        "    mkdir -p %1 && chmod 700 %1 2>/dev/null || true\n"
        "    touch %2 %3 && chmod 600 %2 %3 2>/dev/null || true\n"
        "    script -q -f --log-out %2 --log-timing %3 -c %4\n"
        "    _aibox_rc=$?\n"
        "%5"
        "    exit $_aibox_rc\n"
        "else\n"
        "    %6\n"
        "fi")
        .arg(qLogDir, qLog, qTiming, qInner, postCmd, innerCommand);
}

QString Backend::stripAnsiSequences(const QString &rawText) {
    QString out = rawText;
    out.replace(QStringLiteral("\r\n"), QStringLiteral("\n"));
    out.remove(QLatin1Char('\r'));
    // OSC sequences: ESC ] ... (BEL or ST)
    static const QRegularExpression oscRe(QStringLiteral("\\x1b\\][^\\x07\\x1b]*(?:\\x07|\\x1b\\\\)"));
    out.remove(oscRe);
    // CSI sequences: ESC [ params intermediate final
    static const QRegularExpression csiRe(QStringLiteral("\\x1b\\[[0-9;?]*[ -/]*[@-~]"));
    out.remove(csiRe);
    // Character-set / 2-byte ESC sequences
    static const QRegularExpression esc2Re(QStringLiteral("\\x1b[()#%*+\\-/][0-9A-Za-z]?"));
    out.remove(esc2Re);
    static const QRegularExpression escShortRe(QStringLiteral("\\x1b[@-Z\\\\-_]"));
    out.remove(escShortRe);
    out.remove(QLatin1Char('\x07'));
    return out;
}

QString Backend::formatRecordedSessionText(const QByteArray &rawBytes, int maxBytes) {
    if (rawBytes.isEmpty())
        return {};
    const int cap = qMax(1024, maxBytes);
    bool truncated = false;
    QByteArray slice = rawBytes;
    const int rawBudget = cap * 2;
    if (slice.size() > rawBudget) {
        slice = slice.right(rawBudget);
        truncated = true;
    }
    QString clean = stripAnsiSequences(QString::fromUtf8(slice));
    QByteArray utf8 = clean.toUtf8();
    if (utf8.size() > cap) {
        truncated = true;
        utf8 = utf8.right(cap);
        const int nl = utf8.indexOf('\n');
        if (nl >= 0 && nl < 256)
            utf8 = utf8.mid(nl + 1);
        clean = QString::fromUtf8(utf8);
    }
    if (truncated)
        clean.prepend(QStringLiteral("[... truncated to last 200 KB ...]\n"));
    return clean;
}

bool Backend::redactSessionLogFile(const QString &logFilePath, const QStringList &secretValues) {
    if (logFilePath.trimmed().isEmpty() || secretValues.isEmpty())
        return false;
    QFile in(logFilePath);
    if (!in.exists() || !in.open(QIODevice::ReadOnly))
        return false;
    const QByteArray raw = in.readAll();
    in.close();
    const QByteArray scrubbed = redactSecretsBytes(raw, secretValues);
    if (scrubbed == raw)
        return false;
    QSaveFile out(logFilePath);
    if (!out.open(QIODevice::WriteOnly))
        return false;
    out.write(scrubbed);
    if (!out.commit())
        return false;
    QFile::setPermissions(logFilePath, QFileDevice::ReadOwner | QFileDevice::WriteOwner);
    return true;
}

namespace {
struct SessionFilePair {
    QString logPath;
    QString timingPath;
    qint64 mtimeMs = 0;
    qint64 totalBytes = 0;
};

void removeSessionPairFiles(const SessionFilePair &pair) {
    if (!pair.logPath.isEmpty())
        QFile::remove(pair.logPath);
    if (!pair.timingPath.isEmpty())
        QFile::remove(pair.timingPath);
}
} // namespace

int Backend::pruneRecordedSessions(const QString &sessionsDir, int maxPerEnv, qint64 maxTotalBytes) {
    const QDir root(sessionsDir);
    if (!root.exists())
        return 0;

    int deleted = 0;
    QList<SessionFilePair> surviving;
    const QFileInfoList envDirs = root.entryInfoList(QDir::Dirs | QDir::NoDotAndDotDot, QDir::Name);
    for (const QFileInfo &envDirInfo : envDirs) {
        const QDir envDir(envDirInfo.absoluteFilePath());
        const QFileInfoList logs = envDir.entryInfoList({QStringLiteral("*.log")}, QDir::Files, QDir::Name);
        QList<SessionFilePair> pairs;
        pairs.reserve(logs.size());
        for (const QFileInfo &logFi : logs) {
            const QString base = logFi.completeBaseName();
            const QString timingPath = envDir.absoluteFilePath(base + QStringLiteral(".timing"));
            const QFileInfo timingFi(timingPath);
            SessionFilePair pair;
            pair.logPath = logFi.absoluteFilePath();
            pair.timingPath = timingFi.exists() ? timingPath : QString();
            pair.mtimeMs = logFi.lastModified().toMSecsSinceEpoch();
            pair.totalBytes = logFi.size() + (timingFi.exists() ? timingFi.size() : 0);
            pairs.append(pair);
        }

        // Newest first for per-env cap
        std::sort(pairs.begin(), pairs.end(), [](const SessionFilePair &a, const SessionFilePair &b) {
            if (a.mtimeMs != b.mtimeMs)
                return a.mtimeMs > b.mtimeMs;
            return a.logPath > b.logPath;
        });

        for (int i = 0; i < pairs.size(); ++i) {
            if (maxPerEnv >= 0 && i >= maxPerEnv) {
                removeSessionPairFiles(pairs.at(i));
                ++deleted;
            } else {
                surviving.append(pairs.at(i));
            }
        }
    }

    if (maxTotalBytes > 0) {
        qint64 totalBytes = 0;
        for (const auto &p : surviving)
            totalBytes += p.totalBytes;

        if (totalBytes > maxTotalBytes) {
            // Oldest first for global size cap
            std::sort(surviving.begin(), surviving.end(), [](const SessionFilePair &a, const SessionFilePair &b) {
                if (a.mtimeMs != b.mtimeMs)
                    return a.mtimeMs < b.mtimeMs;
                return a.logPath < b.logPath;
            });
            for (const auto &p : surviving) {
                if (totalBytes <= maxTotalBytes)
                    break;
                removeSessionPairFiles(p);
                totalBytes -= p.totalBytes;
                ++deleted;
            }
        }
    }

    return deleted;
}

QVariantList Backend::scanRecordedSessions(const QString &sessionsDir) {
    QVariantList result;
    const QDir root(sessionsDir);
    if (!root.exists())
        return result;

    const QFileInfoList envDirs = root.entryInfoList(QDir::Dirs | QDir::NoDotAndDotDot, QDir::Name);
    for (const QFileInfo &envDirInfo : envDirs) {
        const QString envName = envDirInfo.fileName();
        const QDir envDir(envDirInfo.absoluteFilePath());
        const QFileInfoList logs = envDir.entryInfoList({QStringLiteral("*.log")}, QDir::Files);
        for (const QFileInfo &logFi : logs) {
            const QString base = logFi.completeBaseName();
            const QString timingPath = envDir.absoluteFilePath(base + QStringLiteral(".timing"));
            const QFileInfo timingFi(timingPath);
            const bool hasTiming = timingFi.exists() && timingFi.size() > 0;
            bool ok = false;
            const qint64 epoch = base.toLongLong(&ok);
            const QDateTime dt = (ok && epoch > 0)
                                     ? QDateTime::fromSecsSinceEpoch(epoch)
                                     : logFi.lastModified();
            const qint64 totalBytes = logFi.size() + (timingFi.exists() ? timingFi.size() : 0);

            QVariantMap item;
            item["id"] = envName + QLatin1Char('/') + base;
            item["envName"] = envName;
            item["epoch"] = dt.toSecsSinceEpoch();
            item["timestamp"] = dt.toString(QStringLiteral("yyyy-MM-dd HH:mm:ss"));
            item["logPath"] = logFi.absoluteFilePath();
            item["timingPath"] = hasTiming ? timingPath : QString();
            item["hasTiming"] = hasTiming;
            item["sizeBytes"] = totalBytes;
            item["sizeDisplay"] = formatBytes(totalBytes);
            result.append(item);
        }
    }

    std::sort(result.begin(), result.end(), [](const QVariant &a, const QVariant &b) {
        return a.toMap().value(QStringLiteral("epoch")).toLongLong() >
               b.toMap().value(QStringLiteral("epoch")).toLongLong();
    });
    return result;
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
    if (logPath.trimmed().isEmpty())
        return {};
    if (m_demoMode && logPath.startsWith(QLatin1String("demo://"))) {
        return formatRecordedSessionText(QByteArrayLiteral(
            "Script started on 2026-11-13 17:10:00+07:00 [COMMAND=\"incus exec desktop-studio --cwd /home/workspace -- bash -l\"]\n"
            "\x1b[32muser@desktop-studio:/home/workspace$\x1b[0m ls -la\n"
            "total 12\n"
            "drwxrwx--- 1 root user 120 Nov 13 17:09 .\n"
            "\x1b[32muser@desktop-studio:/home/workspace$\x1b[0m echo $ANTHROPIC_API_KEY\n"
            "[REDACTED]\n"
            "\x1b[32muser@desktop-studio:/home/workspace$\x1b[0m exit\n"
            "Script done on 2026-11-13 17:12:14+07:00 [COMMAND_EXIT_CODE=\"0\"]\n"));
    }

    QFile f(logPath);
    if (!f.open(QIODevice::ReadOnly))
        return tr("Session log is not available.");
    return formatRecordedSessionText(f.readAll());
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
    const QString replayCmd = QStringLiteral(
        "scriptreplay -t %1 -O %2; printf '\\n[Replay complete — press Enter to close]'; read -r _")
                                  .arg(shellQuote(timingPath), shellQuote(logPath));
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

    const QString cleanSessionsRoot = QDir::cleanPath(sessionsDirPath());
    const QString cleanLog = QDir::cleanPath(logPath.trimmed());
    if (cleanLog.isEmpty() || !cleanLog.startsWith(cleanSessionsRoot + QLatin1Char('/')))
        return tr("Refusing to delete session outside %1.").arg(cleanSessionsRoot);

    QFile::remove(cleanLog);
    const QString cleanTiming = QDir::cleanPath(timingPath.trimmed());
    if (!cleanTiming.isEmpty() && cleanTiming.startsWith(cleanSessionsRoot + QLatin1Char('/')))
        QFile::remove(cleanTiming);

    refreshRecordedSessions();
    return {};
}
