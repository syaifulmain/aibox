#include "backend.h"
#include "backend_helpers.h"

#include <QClipboard>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QRegularExpression>
#include <QSet>
#include <QStandardPaths>
#include <QUuid>

#include <algorithm>
#include <cerrno>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>

// --- S3: Secrets injection helpers ---

QString Backend::defaultSecretsStorePath() {
    const QString cfg = QStandardPaths::writableLocation(QStandardPaths::ConfigLocation);
    const QString base = cfg.trimmed().isEmpty() ? QDir::homePath() + QStringLiteral("/.config") : cfg.trimmed();
    return QDir::cleanPath(base + QStringLiteral("/aibox/secrets.json"));
}

bool Backend::isValidSecretName(const QString &name) {
    static const QRegularExpression nameRe(QStringLiteral("^[A-Z_][A-Z0-9_]*$"));
    return nameRe.match(name).hasMatch();
}

QStringList Backend::parseEnvSecretNames(const QString &rawCsv) {
    QStringList names;
    for (const QString &part : rawCsv.split(QLatin1Char(','), Qt::SkipEmptyParts)) {
        const QString clean = part.trimmed();
        if (isValidSecretName(clean) && !names.contains(clean))
            names.append(clean);
    }
    return names;
}

QList<QPair<QString, QString>> Backend::loadSecretsStore(const QString &filePath, QString *errorOut) {
    if (errorOut)
        errorOut->clear();
    if (filePath.trimmed().isEmpty())
        return {};

    struct stat st {};
    const QByteArray encoded = QFile::encodeName(filePath);
    if (::stat(encoded.constData(), &st) != 0) {
        if (errno == ENOENT)
            return {};
        if (errorOut)
            *errorOut = tr("Could not inspect secrets file permissions: %1").arg(filePath);
        return {};
    }

    if ((st.st_mode & (S_IRWXG | S_IRWXO)) != 0) {
        if (errorOut) {
            *errorOut = tr("Refusing to read %1 with insecure permissions (%2). Run: chmod 600 %1")
                            .arg(filePath, QString::number(static_cast<int>(st.st_mode & 0777), 8));
        }
        return {};
    }

    QFile file(filePath);
    if (!file.open(QIODevice::ReadOnly)) {
        if (errorOut)
            *errorOut = tr("Could not open secrets file: %1").arg(filePath);
        return {};
    }

    QJsonParseError err;
    const QJsonDocument doc = QJsonDocument::fromJson(file.readAll(), &err);
    if (err.error != QJsonParseError::NoError) {
        if (errorOut)
            *errorOut = tr("Invalid JSON in secrets file: %1").arg(err.errorString());
        return {};
    }

    QJsonArray arr;
    if (doc.isObject())
        arr = doc.object().value(QStringLiteral("secrets")).toArray();
    else if (doc.isArray())
        arr = doc.array();

    QList<QPair<QString, QString>> result;
    QSet<QString> seen;
    for (const QJsonValue &val : arr) {
        if (!val.isObject())
            continue;
        const QJsonObject obj = val.toObject();
        const QString name = obj.value(QStringLiteral("name")).toString();
        const QString secretVal = obj.value(QStringLiteral("value")).toString();
        if (!isValidSecretName(name) || seen.contains(name))
            continue;
        seen.insert(name);
        result.append({name, secretVal});
    }
    return result;
}

QString Backend::saveSecretsStore(const QString &filePath, const QList<QPair<QString, QString>> &secrets) {
    const QString cleanPath = QDir::cleanPath(filePath.trimmed());
    if (cleanPath.isEmpty())
        return tr("Invalid secrets store path.");

    QJsonArray arr;
    QSet<QString> seen;
    for (const auto &pair : secrets) {
        if (!isValidSecretName(pair.first))
            return tr("Invalid secret name '%1'. Use ^[A-Z_][A-Z0-9_]*$.").arg(pair.first);
        if (seen.contains(pair.first))
            continue;
        seen.insert(pair.first);
        QJsonObject obj;
        obj[QStringLiteral("name")] = pair.first;
        obj[QStringLiteral("value")] = pair.second;
        arr.append(obj);
    }

    const QString dirPath = QFileInfo(cleanPath).absolutePath();
    if (!QDir().mkpath(dirPath))
        return tr("Could not create secrets directory: %1").arg(dirPath);
    if (::chmod(QFile::encodeName(dirPath).constData(), 0700) != 0)
        return tr("Could not set 0700 permissions on directory: %1").arg(dirPath);

    QJsonObject root;
    root[QStringLiteral("secrets")] = arr;
    const QByteArray payload = QJsonDocument(root).toJson(QJsonDocument::Indented);

    const QString tmpPath = cleanPath + QStringLiteral(".tmp.%1")
                                            .arg(QUuid::createUuid().toString(QUuid::Id128));
    const QByteArray tmpEncoded = QFile::encodeName(tmpPath);
    const int fd = ::open(tmpEncoded.constData(), O_WRONLY | O_CREAT | O_EXCL, S_IRUSR | S_IWUSR);
    if (fd < 0)
        return tr("Could not create temporary secrets file: %1").arg(tmpPath);
    ::fchmod(fd, S_IRUSR | S_IWUSR);

    qint64 totalWritten = 0;
    while (totalWritten < payload.size()) {
        const ssize_t n = ::write(fd, payload.constData() + totalWritten,
                                  static_cast<size_t>(payload.size() - totalWritten));
        if (n < 0) {
            if (errno == EINTR)
                continue;
            ::close(fd);
            QFile::remove(tmpPath);
            return tr("Could not write secrets file: %1").arg(cleanPath);
        }
        totalWritten += n;
    }
    ::close(fd);

    const QByteArray dstEncoded = QFile::encodeName(cleanPath);
    if (::rename(tmpEncoded.constData(), dstEncoded.constData()) != 0) {
        QFile::remove(tmpPath);
        return tr("Could not replace secrets file: %1").arg(cleanPath);
    }
    if (::chmod(dstEncoded.constData(), 0600) != 0)
        return tr("Could not set 0600 permissions on secrets file: %1").arg(cleanPath);

    return {};
}

QString Backend::buildSecretEnvFileContent(const QList<QPair<QString, QString>> &secrets) {
    QString out;
    for (const auto &pair : secrets) {
        if (!isValidSecretName(pair.first))
            continue;
        out += pair.first + QLatin1Char('=') + shellQuote(pair.second) + QLatin1Char('\n');
    }
    return out;
}

QString Backend::redactSecrets(const QString &text, const QStringList &values) {
    if (text.isEmpty() || values.isEmpty())
        return text;

    QStringList sorted;
    sorted.reserve(values.size());
    for (const QString &val : values) {
        if (!val.isEmpty() && !sorted.contains(val))
            sorted.append(val);
    }
    std::sort(sorted.begin(), sorted.end(), [](const QString &a, const QString &b) {
        return a.size() > b.size();
    });

    QString result = text;
    for (const QString &secretVal : sorted)
        result.replace(secretVal, QStringLiteral("[REDACTED]"));
    return result;
}

QByteArray Backend::redactSecretsBytes(const QByteArray &data, const QStringList &values) {
    if (data.isEmpty() || values.isEmpty())
        return data;
    return redactSecrets(QString::fromUtf8(data), values).toUtf8();
}

QString Backend::createHostTempSecretFile(const QString &content, QString *errorOut) {
    if (errorOut)
        errorOut->clear();

    QString runtimeDir = qEnvironmentVariable("XDG_RUNTIME_DIR").trimmed();
    if (runtimeDir.isEmpty() || !QFileInfo(runtimeDir).isDir() || !QFileInfo(runtimeDir).isWritable())
        runtimeDir = QDir::tempPath();

    const QString path = QDir::cleanPath(
        runtimeDir + QStringLiteral("/aibox-%1.env").arg(QUuid::createUuid().toString(QUuid::Id128)));
    const QByteArray encoded = QFile::encodeName(path);
    const int fd = ::open(encoded.constData(), O_WRONLY | O_CREAT | O_EXCL, S_IRUSR | S_IWUSR);
    if (fd < 0) {
        if (errorOut)
            *errorOut = tr("Could not create temporary runtime secret file.");
        return {};
    }
    ::fchmod(fd, S_IRUSR | S_IWUSR);

    const QByteArray bytes = content.toUtf8();
    qint64 written = 0;
    while (written < bytes.size()) {
        const ssize_t n = ::write(fd, bytes.constData() + written, static_cast<size_t>(bytes.size() - written));
        if (n < 0) {
            if (errno == EINTR)
                continue;
            ::close(fd);
            QFile::remove(path);
            if (errorOut)
                *errorOut = tr("Could not write temporary runtime secret file.");
            return {};
        }
        written += n;
    }
    ::close(fd);
    return path;
}

void Backend::setSecretsStorePathForTest(const QString &path) {
    m_secretsPath = path.isEmpty() ? defaultSecretsStorePath() : QDir::cleanPath(path);
    reloadSecrets();
}

void Backend::reloadSecrets() {
    QString err;
    const auto loaded = loadSecretsStore(m_secretsPath, &err);
    m_secrets = loaded;
    m_secretsError = err;
    emit secretsChanged();
}

QVariantList Backend::secretsList() const {
    QVariantList list;
    for (const auto &pair : m_secrets) {
        const QString masked = pair.second.isEmpty() ? QStringLiteral("(empty)") : QStringLiteral("••••••••");
        list.append(QVariantMap{
            {QStringLiteral("name"), pair.first},
            {QStringLiteral("value"), pair.second},
            {QStringLiteral("maskedValue"), masked}
        });
    }
    return list;
}

QStringList Backend::allSecretValues() const {
    QStringList values;
    for (const auto &pair : m_secrets) {
        if (!pair.second.isEmpty())
            values.append(pair.second);
    }
    return values;
}

QString Backend::saveSecret(const QString &name, const QString &value) {
    const QString cleanName = name.trimmed();
    if (!isValidSecretName(cleanName))
        return tr("Secret name must match ^[A-Z_][A-Z0-9_]*$.");

    if (m_demoMode) {
        bool updated = false;
        for (int i = 0; i < m_secrets.size(); ++i) {
            if (m_secrets.at(i).first == cleanName) {
                m_secrets[i].second = value;
                updated = true;
                break;
            }
        }
        if (!updated)
            m_secrets.append({cleanName, value});
        m_secretsError.clear();
        emit secretsChanged();
        return {};
    }

    QString loadErr;
    QList<QPair<QString, QString>> next = loadSecretsStore(m_secretsPath, &loadErr);
    if (!loadErr.isEmpty()) {
        // Keep current in-memory list when recovering permissions on save.
        next = m_secrets;
    }

    bool updated = false;
    for (int i = 0; i < next.size(); ++i) {
        if (next.at(i).first == cleanName) {
            next[i].second = value;
            updated = true;
            break;
        }
    }
    if (!updated)
        next.append({cleanName, value});

    const QString saveErr = saveSecretsStore(m_secretsPath, next);
    if (!saveErr.isEmpty())
        return saveErr;

    m_secrets = next;
    m_secretsError.clear();
    emit secretsChanged();
    return {};
}

QString Backend::deleteSecret(const QString &name) {
    const QString cleanName = name.trimmed();
    QList<QPair<QString, QString>> next;
    for (const auto &pair : m_secrets) {
        if (pair.first != cleanName)
            next.append(pair);
    }

    if (m_demoMode) {
        m_secrets = next;
        m_secretsError.clear();
        emit secretsChanged();
        return {};
    }

    const QString saveErr = saveSecretsStore(m_secretsPath, next);
    if (!saveErr.isEmpty())
        return saveErr;

    m_secrets = next;
    m_secretsError.clear();
    emit secretsChanged();
    return {};
}

bool Backend::copySecret(const QString &name) {
    const QString cleanName = name.trimmed();
    for (const auto &pair : m_secrets) {
        if (pair.first != cleanName)
            continue;
        if (pair.second.isEmpty())
            return false;
        auto *cb = QGuiApplication::clipboard();
        if (!cb)
            return false;
        cb->setText(pair.second);
        // The status message names the secret only, so the value never lands
        // in the visible UI or an operation log.
        m_statusMessage = tr("Copied secret '%1' to clipboard.").arg(cleanName);
        emit statusMessageChanged();
        QTimer::singleShot(3000, this, [this] {
            if (!m_busy) {
                m_statusMessage.clear();
                emit statusMessageChanged();
            }
        });
        return true;
    }
    return false;
}

void Backend::setEnvironmentSecrets(const QString &name, const QStringList &secretNames) {
    if (name.isEmpty())
        return;

    const QStringList cleanNames = parseEnvSecretNames(secretNames.join(QLatin1Char(',')));
    if (m_demoMode) {
        for (int i = 0; i < m_environments.size(); ++i) {
            QVariantMap env = m_environments.at(i).toMap();
            if (env.value("name").toString() == name) {
                env["secretNames"] = cleanNames;
                m_environments[i] = env;
                emit environmentsChanged();
                break;
            }
        }
        return;
    }

    if (m_busy)
        return;

    const QString qName = shellQuote(name);
    QString script;
    if (cleanNames.isEmpty()) {
        script = QStringLiteral(
            "set -euo pipefail\n"
            "echo '@@STAGE:50:Clearing environment secret bindings...'\n"
            "incus config unset %1 user.aibox.secrets >/dev/null 2>&1 || true\n"
            "echo '@@STAGE:100:Secrets updated.'").arg(qName);
    } else {
        script = QStringLiteral(
            "set -euo pipefail\n"
            "echo '@@STAGE:50:Updating environment secret bindings...'\n"
            "incus config set %1 user.aibox.secrets %2\n"
            "echo '@@STAGE:100:Secrets updated.'").arg(qName, shellQuote(cleanNames.join(QLatin1Char(','))));
    }
    runShellTask(tr("Updating secrets for '%1'").arg(name), script, {}, name);
}

QList<QPair<QString, QString>> Backend::resolveEnvironmentSecrets(const QString &envName, QString *errorOut) const {
    if (errorOut)
        errorOut->clear();
    const QVariantMap env = environmentByName(envName);
    const QStringList assigned = env.value("secretNames").toStringList();
    if (assigned.isEmpty())
        return {};

    QString loadErr;
    QList<QPair<QString, QString>> store;
    if (m_demoMode) {
        store = m_secrets;
    } else {
        store = loadSecretsStore(m_secretsPath, &loadErr);
        if (!loadErr.isEmpty()) {
            if (errorOut)
                *errorOut = loadErr;
            return {};
        }
    }

    QList<QPair<QString, QString>> resolved;
    for (const QString &reqName : assigned) {
        for (const auto &pair : store) {
            if (pair.first == reqName) {
                resolved.append(pair);
                break;
            }
        }
    }
    return resolved;
}

bool Backend::pushEnvironmentSecretsToContainer(const QString &envName, QString *outRemoteEnvPath,
                                                QString *errorOut) const {
    if (outRemoteEnvPath)
        outRemoteEnvPath->clear();
    if (errorOut)
        errorOut->clear();

    QString resolveErr;
    const QList<QPair<QString, QString>> pairs = resolveEnvironmentSecrets(envName, &resolveErr);
    if (!resolveErr.isEmpty()) {
        if (errorOut)
            *errorOut = resolveErr;
        return false;
    }
    if (pairs.isEmpty())
        return true;

    QString tmpErr;
    const QString hostFile = createHostTempSecretFile(buildSecretEnvFileContent(pairs), &tmpErr);
    if (hostFile.isEmpty()) {
        if (errorOut)
            *errorOut = tmpErr;
        return false;
    }

    const QString remotePath = QStringLiteral("/dev/shm/aibox-%1.env")
                                   .arg(QUuid::createUuid().toString(QUuid::Id128));
    QProcess pushProc;
    pushProc.start(QStringLiteral("incus"),
                   {QStringLiteral("file"), QStringLiteral("push"),
                    QStringLiteral("--mode"), QStringLiteral("0400"),
                    hostFile, envName + remotePath});
    const bool finished = pushProc.waitForFinished(15000);
    QFile::remove(hostFile);

    if (!finished || pushProc.exitStatus() != QProcess::NormalExit || pushProc.exitCode() != 0) {
        if (errorOut) {
            const QString errTxt = QString::fromUtf8(pushProc.readAllStandardError()).trimmed();
            *errorOut = errTxt.isEmpty() ? tr("Failed to inject secrets into container.") : errTxt;
        }
        return false;
    }

    if (outRemoteEnvPath)
        *outRemoteEnvPath = remotePath;
    return true;
}
