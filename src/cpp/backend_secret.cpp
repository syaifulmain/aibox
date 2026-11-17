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

bool Backend::isSafeEnvironmentFileName(const QString &name) {
    static const QRegularExpression nameRe(QStringLiteral("^[a-z0-9][a-z0-9-]*$"));
    return name.size() <= 64 && nameRe.match(name).hasMatch();
}

QString Backend::instanceSecretsDirPath() const {
    if (m_secretsPath.trimmed().isEmpty())
        return {};
    return QDir::cleanPath(QFileInfo(m_secretsPath).absolutePath() + QStringLiteral("/secrets.d"));
}

QString Backend::environmentSecretsStorePath(const QString &envName) const {
    const QString cleanName = envName.trimmed();
    const QString dirPath = instanceSecretsDirPath();
    if (dirPath.isEmpty() || !isSafeEnvironmentFileName(cleanName))
        return {};
    return QDir::cleanPath(dirPath + QLatin1Char('/') + cleanName + QStringLiteral(".json"));
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

QStringList Backend::loadSecretNameList(const QString &filePath, const QString &key) {
    QStringList names;
    if (key.trimmed().isEmpty())
        return names;
    QFile file(filePath);
    if (!file.open(QIODevice::ReadOnly))
        return names;

    const QJsonDocument doc = QJsonDocument::fromJson(file.readAll());
    if (!doc.isObject())
        return names;

    const QJsonArray arr = doc.object().value(key).toArray();
    for (const QJsonValue &val : arr) {
        const QString name = val.toString();
        if (isValidSecretName(name) && !names.contains(name))
            names.append(name);
    }
    return names;
}

QString Backend::saveSecretsStore(const QString &filePath, const QList<QPair<QString, QString>> &secrets,
                                  const QStringList &disabledNames,
                                  const QStringList &hiddenInInstances) {
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
    if (!disabledNames.isEmpty()) {
        QJsonArray disabledArr;
        QSet<QString> disabledSeen;
        for (const QString &name : disabledNames) {
            if (!isValidSecretName(name) || disabledSeen.contains(name))
                continue;
            disabledSeen.insert(name);
            disabledArr.append(name);
        }
        if (!disabledArr.isEmpty())
            root[QStringLiteral("disabled")] = disabledArr;
    }
    if (!hiddenInInstances.isEmpty()) {
        QJsonArray hiddenArr;
        QSet<QString> hiddenSeen;
        for (const QString &name : hiddenInInstances) {
            if (!isValidSecretName(name) || hiddenSeen.contains(name))
                continue;
            hiddenSeen.insert(name);
            hiddenArr.append(name);
        }
        if (!hiddenArr.isEmpty())
            root[QStringLiteral("hiddenInInstances")] = hiddenArr;
    }
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
    m_hiddenGlobalSecrets = loadSecretNameList(m_secretsPath, QStringLiteral("hiddenInInstances"));
    refreshInstanceSecretValuesCache();
    emit secretsChanged();
}

void Backend::refreshInstanceSecretValuesCache() {
    m_instanceSecretValues.clear();
    if (m_demoMode) {
        for (const QVariant &row : m_environmentSecrets) {
            const QString value = row.toMap().value(QStringLiteral("value")).toString();
            if (!value.isEmpty() && !m_instanceSecretValues.contains(value))
                m_instanceSecretValues.append(value);
        }
        return;
    }

    QDir dir(instanceSecretsDirPath());
    if (!dir.exists())
        return;
    const QStringList files = dir.entryList({QStringLiteral("*.json")}, QDir::Files);
    for (const QString &fileName : files) {
        const auto pairs = loadSecretsStore(dir.filePath(fileName), nullptr);
        for (const auto &pair : pairs) {
            if (!pair.second.isEmpty() && !m_instanceSecretValues.contains(pair.second))
                m_instanceSecretValues.append(pair.second);
        }
    }
}

void Backend::reloadEnvironmentSecrets(const QString &envName) {
    m_environmentSecretsEnv = envName.trimmed();
    m_environmentSecrets.clear();
    m_environmentSecretsError.clear();

    const QString storePath = environmentSecretsStorePath(m_environmentSecretsEnv);
    if (storePath.isEmpty()) {
        m_environmentSecretsError = tr("Select an environment to manage its secrets.");
        emit environmentSecretsChanged();
        return;
    }

    QList<QPair<QString, QString>> local;
    QStringList disabled;
    if (!m_demoMode) {
        QString loadErr;
        local = loadSecretsStore(storePath, &loadErr);
        if (!loadErr.isEmpty())
            m_environmentSecretsError = loadErr;
        else
            disabled = loadSecretNameList(storePath, QStringLiteral("disabled"));
    }

    const QStringList assigned = environmentByName(m_environmentSecretsEnv)
                                     .value(QStringLiteral("secretNames")).toStringList();
    const auto appendRow = [this](const QString &name, const QString &value, const QString &scope,
                                  bool isAssigned) {
        m_environmentSecrets.append(QVariantMap{
            {QStringLiteral("name"), name},
            {QStringLiteral("value"), value},
            {QStringLiteral("maskedValue"),
             value.isEmpty() ? QStringLiteral("(empty)") : QStringLiteral("••••••••")},
            {QStringLiteral("scope"), scope},
            {QStringLiteral("assigned"), isAssigned}
        });
    };

    // Instance-only secrets come first because they belong to this environment.
    for (const auto &pair : local)
        appendRow(pair.first, pair.second, QStringLiteral("environment"),
                  !disabled.contains(pair.first));
    for (const auto &pair : m_secrets) {
        if (m_hiddenGlobalSecrets.contains(pair.first))
            continue;
        appendRow(pair.first, pair.second, QStringLiteral("global"), assigned.contains(pair.first));
    }

    emit environmentSecretsChanged();
}

void Backend::updateEnvironmentSecretAssignments(const QString &envName, const QStringList &names) {
    if (m_environmentSecretsEnv != envName.trimmed())
        return;

    bool changed = false;
    for (int i = 0; i < m_environmentSecrets.size(); ++i) {
        QVariantMap row = m_environmentSecrets.at(i).toMap();
        if (row.value(QStringLiteral("scope")).toString() != QLatin1String("global"))
            continue;
        const bool assigned = names.contains(row.value(QStringLiteral("name")).toString());
        if (row.value(QStringLiteral("assigned")).toBool() == assigned)
            continue;
        row[QStringLiteral("assigned")] = assigned;
        m_environmentSecrets[i] = row;
        changed = true;
    }
    if (changed)
        emit environmentSecretsChanged();
}

void Backend::clearGlobalSecretFromEnvironments(const QString &name) {
    QString script;
    QStringList touched;
    for (int i = 0; i < m_environments.size(); ++i) {
        QVariantMap env = m_environments.at(i).toMap();
        QStringList names = env.value(QStringLiteral("secretNames")).toStringList();
        if (!names.contains(name))
            continue;

        names.removeAll(name);
        env[QStringLiteral("secretNames")] = names;
        m_environments[i] = env;
        touched.append(env.value(QStringLiteral("name")).toString());

        if (m_demoMode)
            continue;
        const QString qEnv = shellQuote(env.value(QStringLiteral("name")).toString());
        if (names.isEmpty()) {
            script += QStringLiteral("incus config unset %1 user.aibox.secrets >/dev/null 2>&1 || true\n")
                          .arg(qEnv);
        } else {
            script += QStringLiteral("incus config set %1 user.aibox.secrets %2\n")
                          .arg(qEnv, shellQuote(names.join(QLatin1Char(','))));
        }
    }

    if (touched.isEmpty())
        return;

    emit environmentsChanged();
    if (m_demoMode)
        return;

    const QString fullScript = QStringLiteral(
        "set -euo pipefail\n"
        "echo '@@STAGE:30:Removing hidden secret bindings...'\n")
        + script
        + QStringLiteral("echo '@@STAGE:100:Secret bindings updated.'");
    runShellTask(tr("Removing secret '%1' from environments").arg(name), fullScript);
}

QVariantList Backend::secretsList() const {
    QVariantList list;
    for (const auto &pair : m_secrets) {
        const QString masked = pair.second.isEmpty() ? QStringLiteral("(empty)") : QStringLiteral("••••••••");
        list.append(QVariantMap{
            {QStringLiteral("name"), pair.first},
            {QStringLiteral("value"), pair.second},
            {QStringLiteral("maskedValue"), masked},
            {QStringLiteral("visibleInEnvironments"), !m_hiddenGlobalSecrets.contains(pair.first)}
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
    for (const QString &value : m_instanceSecretValues) {
        if (!value.isEmpty() && !values.contains(value))
            values.append(value);
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

    QStringList hidden = loadSecretNameList(m_secretsPath, QStringLiteral("hiddenInInstances"));
    if (hidden.isEmpty() && !m_hiddenGlobalSecrets.isEmpty())
        hidden = m_hiddenGlobalSecrets;
    const QString saveErr = saveSecretsStore(m_secretsPath, next, {}, hidden);
    if (!saveErr.isEmpty())
        return saveErr;

    m_secrets = next;
    m_hiddenGlobalSecrets = hidden;
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
        m_hiddenGlobalSecrets.removeAll(cleanName);
        m_secretsError.clear();
        emit secretsChanged();
        return {};
    }

    QStringList hidden = loadSecretNameList(m_secretsPath, QStringLiteral("hiddenInInstances"));
    hidden.removeAll(cleanName);
    const QString saveErr = saveSecretsStore(m_secretsPath, next, {}, hidden);
    if (!saveErr.isEmpty())
        return saveErr;

    m_secrets = next;
    m_hiddenGlobalSecrets = hidden;
    m_secretsError.clear();
    emit secretsChanged();
    return {};
}

QString Backend::setGlobalSecretInstanceVisible(const QString &name, bool visible) {
    const QString cleanName = name.trimmed();
    if (!isValidSecretName(cleanName))
        return tr("Invalid secret name '%1'.").arg(cleanName);

    const auto applyVisibility = [this, &cleanName, visible]() {
        if (visible)
            m_hiddenGlobalSecrets.removeAll(cleanName);
        else if (!m_hiddenGlobalSecrets.contains(cleanName))
            m_hiddenGlobalSecrets.append(cleanName);
    };

    if (m_demoMode) {
        bool found = false;
        for (const auto &pair : m_secrets) {
            if (pair.first == cleanName) {
                found = true;
                break;
            }
        }
        if (!found)
            return tr("Secret '%1' was not found.").arg(cleanName);
        applyVisibility();
        if (!visible)
            clearGlobalSecretFromEnvironments(cleanName);
        emit secretsChanged();
        if (!m_environmentSecretsEnv.isEmpty())
            reloadEnvironmentSecrets(m_environmentSecretsEnv);
        return {};
    }

    QString loadErr;
    const auto pairs = loadSecretsStore(m_secretsPath, &loadErr);
    if (!loadErr.isEmpty())
        return loadErr;

    bool found = false;
    for (const auto &pair : pairs) {
        if (pair.first == cleanName) {
            found = true;
            break;
        }
    }
    if (!found)
        return tr("Secret '%1' was not found.").arg(cleanName);

    QStringList hidden = loadSecretNameList(m_secretsPath, QStringLiteral("hiddenInInstances"));
    hidden.removeAll(cleanName);
    if (!visible)
        hidden.append(cleanName);

    const QString saveErr = saveSecretsStore(m_secretsPath, pairs, {}, hidden);
    if (!saveErr.isEmpty())
        return saveErr;

    m_secrets = pairs;
    m_hiddenGlobalSecrets = hidden;
    m_secretsError.clear();
    if (!visible)
        clearGlobalSecretFromEnvironments(cleanName);
    emit secretsChanged();
    if (!m_environmentSecretsEnv.isEmpty())
        reloadEnvironmentSecrets(m_environmentSecretsEnv);
    return {};
}

bool Backend::copySecretValue(const QString &name, const QString &value) {
    if (value.isEmpty())
        return false;
    auto *cb = QGuiApplication::clipboard();
    if (!cb)
        return false;
    cb->setText(value);
    // The status message names the secret only, so the value never lands
    // in the visible UI or an operation log.
    m_statusMessage = tr("Copied secret '%1' to clipboard.").arg(name);
    emit statusMessageChanged();
    QTimer::singleShot(3000, this, [this] {
        if (!m_busy) {
            m_statusMessage.clear();
            emit statusMessageChanged();
        }
    });
    return true;
}

bool Backend::copySecret(const QString &name) {
    const QString cleanName = name.trimmed();
    for (const auto &pair : m_secrets) {
        if (pair.first == cleanName)
            return copySecretValue(cleanName, pair.second);
    }
    return false;
}

QString Backend::saveEnvironmentSecret(const QString &envName, const QString &name,
                                       const QString &value) {
    const QString env = envName.trimmed();
    const QString storePath = environmentSecretsStorePath(env);
    if (storePath.isEmpty())
        return tr("Select an environment before adding a secret.");

    const QString cleanName = name.trimmed();
    if (!isValidSecretName(cleanName))
        return tr("Secret name must match ^[A-Z_][A-Z0-9_]*$.");

    // Global secrets are read-only in an environment. Reject duplicate names
    // so injection can never pick an ambiguous value.
    QList<QPair<QString, QString>> globals = m_secrets;
    if (!m_demoMode) {
        QString globalErr;
        const auto fresh = loadSecretsStore(m_secretsPath, &globalErr);
        if (globalErr.isEmpty())
            globals = fresh;
    }
    for (const auto &pair : globals) {
        if (pair.first == cleanName) {
            return tr("'%1' is a global secret. Manage it in Tools → Secrets.").arg(cleanName);
        }
    }

    if (m_demoMode) {
        if (m_environmentSecretsEnv != env)
            reloadEnvironmentSecrets(env);
        bool updated = false;
        for (int i = 0; i < m_environmentSecrets.size(); ++i) {
            QVariantMap row = m_environmentSecrets.at(i).toMap();
            if (row.value(QStringLiteral("scope")).toString() != QLatin1String("environment")
                || row.value(QStringLiteral("name")).toString() != cleanName) {
                continue;
            }
            row[QStringLiteral("value")] = value;
            row[QStringLiteral("maskedValue")] =
                value.isEmpty() ? QStringLiteral("(empty)") : QStringLiteral("••••••••");
            m_environmentSecrets[i] = row;
            updated = true;
            break;
        }
        if (!updated) {
            m_environmentSecrets.prepend(QVariantMap{
                {QStringLiteral("name"), cleanName},
                {QStringLiteral("value"), value},
                {QStringLiteral("maskedValue"),
                 value.isEmpty() ? QStringLiteral("(empty)") : QStringLiteral("••••••••")},
                {QStringLiteral("scope"), QStringLiteral("environment")},
                {QStringLiteral("assigned"), true}
            });
        }
        m_environmentSecretsError.clear();
        refreshInstanceSecretValuesCache();
        emit environmentSecretsChanged();
        return {};
    }

    QString loadErr;
    QList<QPair<QString, QString>> next = loadSecretsStore(storePath, &loadErr);
    if (!loadErr.isEmpty())
        return loadErr;
    const QStringList disabled = loadSecretNameList(storePath, QStringLiteral("disabled"));

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

    const QString saveErr = saveSecretsStore(storePath, next, disabled);
    if (!saveErr.isEmpty())
        return saveErr;

    if (m_environmentSecretsEnv == env)
        reloadEnvironmentSecrets(env);
    refreshInstanceSecretValuesCache();
    emit secretsChanged();
    return {};
}

QString Backend::deleteEnvironmentSecret(const QString &envName, const QString &name) {
    const QString env = envName.trimmed();
    const QString storePath = environmentSecretsStorePath(env);
    if (storePath.isEmpty())
        return tr("Select an environment before deleting a secret.");

    const QString cleanName = name.trimmed();
    if (!isValidSecretName(cleanName))
        return tr("Invalid secret name '%1'.").arg(cleanName);

    bool isGlobal = false;
    for (const auto &pair : m_secrets) {
        if (pair.first == cleanName) {
            isGlobal = true;
            break;
        }
    }
    if (!isGlobal && !m_demoMode) {
        const auto globals = loadSecretsStore(m_secretsPath, nullptr);
        for (const auto &pair : globals) {
            if (pair.first == cleanName) {
                isGlobal = true;
                break;
            }
        }
    }
    if (isGlobal) {
        return tr("'%1' is a global secret and cannot be deleted from an environment. "
                  "Manage it in Tools → Secrets.").arg(cleanName);
    }

    if (m_demoMode) {
        if (m_environmentSecretsEnv != env)
            reloadEnvironmentSecrets(env);
        bool found = false;
        QVariantList next;
        for (const QVariant &item : m_environmentSecrets) {
            const QVariantMap row = item.toMap();
            if (row.value(QStringLiteral("scope")).toString() == QLatin1String("environment")
                && row.value(QStringLiteral("name")).toString() == cleanName) {
                found = true;
                continue;
            }
            next.append(item);
        }
        if (!found)
            return tr("Secret '%1' does not belong to environment '%2'.").arg(cleanName, env);
        m_environmentSecrets = next;
        m_environmentSecretsError.clear();
        refreshInstanceSecretValuesCache();
        emit environmentSecretsChanged();
        return {};
    }

    QString loadErr;
    const auto current = loadSecretsStore(storePath, &loadErr);
    if (!loadErr.isEmpty())
        return loadErr;
    QStringList disabled = loadSecretNameList(storePath, QStringLiteral("disabled"));

    bool found = false;
    QList<QPair<QString, QString>> next;
    for (const auto &pair : current) {
        if (pair.first == cleanName) {
            found = true;
            continue;
        }
        next.append(pair);
    }
    if (!found)
        return tr("Secret '%1' does not belong to environment '%2'.").arg(cleanName, env);
    disabled.removeAll(cleanName);

    const QString saveErr = saveSecretsStore(storePath, next, disabled);
    if (!saveErr.isEmpty())
        return saveErr;

    if (m_environmentSecretsEnv == env)
        reloadEnvironmentSecrets(env);
    refreshInstanceSecretValuesCache();
    emit secretsChanged();
    return {};
}

bool Backend::copyEnvironmentSecret(const QString &envName, const QString &name) {
    const QString env = envName.trimmed();
    const QString cleanName = name.trimmed();
    if (m_environmentSecretsEnv != env)
        reloadEnvironmentSecrets(env);
    for (const QVariant &item : m_environmentSecrets) {
        const QVariantMap row = item.toMap();
        if (row.value(QStringLiteral("name")).toString() == cleanName)
            return copySecretValue(cleanName, row.value(QStringLiteral("value")).toString());
    }
    return false;
}

QString Backend::setEnvironmentLocalSecretEnabled(const QString &envName, const QString &name,
                                                  bool enabled) {
    const QString env = envName.trimmed();
    const QString storePath = environmentSecretsStorePath(env);
    if (storePath.isEmpty())
        return tr("Select an environment before changing a secret.");

    const QString cleanName = name.trimmed();
    if (!isValidSecretName(cleanName))
        return tr("Invalid secret name '%1'.").arg(cleanName);

    if (m_demoMode) {
        for (int i = 0; i < m_environmentSecrets.size(); ++i) {
            QVariantMap row = m_environmentSecrets.at(i).toMap();
            if (row.value(QStringLiteral("scope")).toString() != QLatin1String("environment")
                || row.value(QStringLiteral("name")).toString() != cleanName) {
                continue;
            }
            row[QStringLiteral("assigned")] = enabled;
            m_environmentSecrets[i] = row;
            emit environmentSecretsChanged();
            return {};
        }
        return tr("Secret '%1' does not belong to environment '%2'.").arg(cleanName, env);
    }

    QString loadErr;
    const auto pairs = loadSecretsStore(storePath, &loadErr);
    if (!loadErr.isEmpty())
        return loadErr;

    bool found = false;
    for (const auto &pair : pairs) {
        if (pair.first == cleanName) {
            found = true;
            break;
        }
    }
    if (!found)
        return tr("Secret '%1' does not belong to environment '%2'.").arg(cleanName, env);

    QStringList disabled = loadSecretNameList(storePath, QStringLiteral("disabled"));
    disabled.removeAll(cleanName);
    if (!enabled)
        disabled.append(cleanName);

    const QString saveErr = saveSecretsStore(storePath, pairs, disabled);
    if (!saveErr.isEmpty())
        return saveErr;

    if (m_environmentSecretsEnv == env)
        reloadEnvironmentSecrets(env);
    return {};
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
                if (m_environmentSecretsEnv == name)
                    reloadEnvironmentSecrets(name);
                break;
            }
        }
        return;
    }

    if (m_busy)
        return;

    // Reflect the toggle immediately; the environment refresh confirms it
    // after the Incus config task finishes.
    updateEnvironmentSecretAssignments(name, cleanNames);

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

    QList<QPair<QString, QString>> resolved;
    if (!assigned.isEmpty()) {
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

        for (const QString &reqName : assigned) {
            if (m_hiddenGlobalSecrets.contains(reqName))
                continue;
            for (const auto &pair : store) {
                if (pair.first == reqName) {
                    resolved.append(pair);
                    break;
                }
            }
        }
    }

    // Instance-only secrets are injected automatically for their own
    // environment, without an assignment entry in user.aibox.secrets.
    if (!m_demoMode) {
        const QString localPath = environmentSecretsStorePath(envName);
        if (!localPath.isEmpty()) {
            QString localErr;
            const auto local = loadSecretsStore(localPath, &localErr);
            if (!localErr.isEmpty()) {
                if (errorOut)
                    *errorOut = localErr;
                return {};
            }
            const QStringList disabled = loadSecretNameList(localPath, QStringLiteral("disabled"));
            for (const auto &pair : local) {
                if (disabled.contains(pair.first))
                    continue;
                bool alreadyResolved = false;
                for (const auto &existing : resolved) {
                    if (existing.first == pair.first) {
                        alreadyResolved = true;
                        break;
                    }
                }
                if (!alreadyResolved)
                    resolved.append(pair);
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
