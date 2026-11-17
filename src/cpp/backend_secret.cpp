#include "backend.h"
#include "backend_helpers.h"
#include "rust_bridge.h"

#include "aibox-ffi/src/secrets.rs.h"

#include <QClipboard>
#include <QGuiApplication>
#include <QJsonArray>
#include <QJsonDocument>

namespace {

template <typename T>
rust::Slice<const T> asSlice(const rust::Vec<T> &v) {
    return rust::Slice<const T>(v.data(), v.size());
}

rust::Vec<aibox::ffi::SecretPairFfi> toRustPairs(const QList<QPair<QString, QString>> &pairs) {
    rust::Vec<aibox::ffi::SecretPairFfi> out;
    out.reserve(static_cast<std::size_t>(pairs.size()));
    for (const auto &pair : pairs) {
        out.push_back(aibox::ffi::SecretPairFfi{
            aibox::ffi::toRust(pair.first),
            aibox::ffi::toRust(pair.second),
        });
    }
    return out;
}

QList<QPair<QString, QString>> fromRustPairs(const rust::Vec<aibox::ffi::SecretPairFfi> &pairs) {
    QList<QPair<QString, QString>> out;
    out.reserve(static_cast<qsizetype>(pairs.size()));
    for (const auto &pair : pairs) {
        out.append({aibox::ffi::fromRust(pair.name), aibox::ffi::fromRust(pair.value)});
    }
    return out;
}

aibox::ffi::SecretsStateInputFfi makeSecretsInput(
    const QString &secretsPath,
    const QList<QPair<QString, QString>> &secrets,
    const QString &secretsError,
    const QStringList &hiddenGlobals,
    const QStringList &instanceSecretValues,
    const QString &envSecretsEnv,
    const QVariantList &envSecrets,
    const QString &envSecretsError,
    bool demoMode) {
    const QByteArray envJson =
        QJsonDocument(QJsonArray::fromVariantList(envSecrets)).toJson(QJsonDocument::Compact);
    return aibox::ffi::SecretsStateInputFfi{
        aibox::ffi::toRust(secretsPath),
        toRustPairs(secrets),
        aibox::ffi::toRust(secretsError),
        aibox::ffi::toRustVec(hiddenGlobals),
        aibox::ffi::toRustVec(instanceSecretValues),
        aibox::ffi::toRust(envSecretsEnv),
        rust::String(envJson.constData(), static_cast<std::size_t>(envJson.size())),
        aibox::ffi::toRust(envSecretsError),
        demoMode,
    };
}

}  // namespace

// --- S3: Secrets injection helpers ---

QString Backend::defaultSecretsStorePath() {
    return aibox::ffi::fromRust(aibox::ffi::secrets_default_store_path());
}

bool Backend::isSafeEnvironmentFileName(const QString &name) {
    return aibox::ffi::secrets_is_safe_environment_file_name(aibox::ffi::toRust(name));
}

QString Backend::instanceSecretsDirPath() const {
    return aibox::ffi::fromRust(
        aibox::ffi::secrets_instance_dir_path(aibox::ffi::toRust(m_secretsPath)));
}

QString Backend::environmentSecretsStorePath(const QString &envName) const {
    return aibox::ffi::fromRust(aibox::ffi::secrets_environment_store_path(
        aibox::ffi::toRust(m_secretsPath), aibox::ffi::toRust(envName)));
}

bool Backend::isValidSecretName(const QString &name) {
    return aibox::ffi::secrets_is_valid_name(aibox::ffi::toRust(name));
}

QStringList Backend::parseEnvSecretNames(const QString &rawCsv) {
    return aibox::ffi::fromRustVec(
        aibox::ffi::secrets_parse_env_names(aibox::ffi::toRust(rawCsv)));
}

QList<QPair<QString, QString>> Backend::loadSecretsStore(const QString &filePath, QString *errorOut) {
    const auto res = aibox::ffi::secrets_load_store(aibox::ffi::toRust(filePath));
    if (errorOut)
        *errorOut = aibox::ffi::fromRust(res.error);
    return fromRustPairs(res.pairs);
}

QStringList Backend::loadSecretNameList(const QString &filePath, const QString &key) {
    return aibox::ffi::fromRustVec(aibox::ffi::secrets_load_name_list(
        aibox::ffi::toRust(filePath), aibox::ffi::toRust(key)));
}

QString Backend::saveSecretsStore(const QString &filePath,
                                  const QList<QPair<QString, QString>> &secrets,
                                  const QStringList &disabledNames,
                                  const QStringList &hiddenInInstances) {
    return aibox::ffi::fromRust(aibox::ffi::secrets_save_store(
        aibox::ffi::toRust(filePath), asSlice(toRustPairs(secrets)),
        asSlice(aibox::ffi::toRustVec(disabledNames)),
        asSlice(aibox::ffi::toRustVec(hiddenInInstances))));
}

QString Backend::buildSecretEnvFileContent(const QList<QPair<QString, QString>> &secrets) {
    return aibox::ffi::fromRust(
        aibox::ffi::secrets_build_env_file_content(asSlice(toRustPairs(secrets))));
}

QString Backend::redactSecrets(const QString &text, const QStringList &values) {
    return aibox::ffi::fromRust(aibox::ffi::secrets_redact_str(
        aibox::ffi::toRust(text), asSlice(aibox::ffi::toRustVec(values))));
}

QByteArray Backend::redactSecretsBytes(const QByteArray &data, const QStringList &values) {
    return aibox::ffi::fromRustBytes(aibox::ffi::secrets_redact_bytes(
        aibox::ffi::toRustBytes(data), asSlice(aibox::ffi::toRustVec(values))));
}

QString Backend::createHostTempSecretFile(const QString &content, QString *errorOut) {
    const auto res = aibox::ffi::secrets_create_host_temp_file(aibox::ffi::toRust(content));
    if (errorOut)
        *errorOut = aibox::ffi::fromRust(res.error);
    return aibox::ffi::fromRust(res.path);
}

void Backend::setSecretsStorePathForTest(const QString &path) {
    m_secretsPath = path.isEmpty() ? defaultSecretsStorePath() : normalizedPath(path, {});
    reloadSecrets();
}

void Backend::reloadSecrets() {
    const auto input = makeSecretsInput(
        m_secretsPath, m_secrets, m_secretsError, m_hiddenGlobalSecrets,
        m_instanceSecretValues, m_environmentSecretsEnv, m_environmentSecrets,
        m_environmentSecretsError, m_demoMode);
    const auto out = aibox::ffi::secrets_reload(input);
    m_secrets = fromRustPairs(out.secrets);
    m_secretsError = aibox::ffi::fromRust(out.secrets_error);
    m_hiddenGlobalSecrets = aibox::ffi::fromRustVec(out.hidden_global_secrets);
    m_instanceSecretValues = aibox::ffi::fromRustVec(out.instance_secret_values);
    emit secretsChanged();
}

void Backend::refreshInstanceSecretValuesCache() {
    const auto input = makeSecretsInput(
        m_secretsPath, m_secrets, m_secretsError, m_hiddenGlobalSecrets,
        m_instanceSecretValues, m_environmentSecretsEnv, m_environmentSecrets,
        m_environmentSecretsError, m_demoMode);
    m_instanceSecretValues = aibox::ffi::fromRustVec(
        aibox::ffi::secrets_refresh_instance_cache(input));
}

void Backend::reloadEnvironmentSecrets(const QString &envName) {
    const QStringList assigned = environmentByName(envName.trimmed())
                                     .value(QStringLiteral("secretNames")).toStringList();
    const auto input = makeSecretsInput(
        m_secretsPath, m_secrets, m_secretsError, m_hiddenGlobalSecrets,
        m_instanceSecretValues, m_environmentSecretsEnv, m_environmentSecrets,
        m_environmentSecretsError, m_demoMode);
    const auto out = aibox::ffi::secrets_reload_environment(
        input, aibox::ffi::toRust(envName), asSlice(aibox::ffi::toRustVec(assigned)));
    m_environmentSecretsEnv = aibox::ffi::fromRust(out.environment_secrets_env);
    m_environmentSecrets = aibox::ffi::fromRustJson(out.environment_secrets_json).toList();
    m_environmentSecretsError = aibox::ffi::fromRust(out.environment_secrets_error);
    emit environmentSecretsChanged();
}

void Backend::updateEnvironmentSecretAssignments(const QString &envName, const QStringList &names) {
    const QByteArray envJson =
        QJsonDocument(QJsonArray::fromVariantList(m_environmentSecrets))
            .toJson(QJsonDocument::Compact);
    const auto res = aibox::ffi::secrets_update_env_assignments(
        rust::String(envJson.constData(), static_cast<std::size_t>(envJson.size())),
        aibox::ffi::toRust(m_environmentSecretsEnv),
        aibox::ffi::toRust(envName),
        asSlice(aibox::ffi::toRustVec(names)));
    if (!res.error.empty()) {
        m_environmentSecrets = aibox::ffi::fromRustJson(res.path).toList();
        emit environmentSecretsChanged();
    }
}

void Backend::clearGlobalSecretFromEnvironments(const QString &name) {
    rust::Vec<aibox::ffi::EnvSecretUpdateFfi> updates;
    for (int i = 0; i < m_environments.size(); ++i) {
        QVariantMap env = m_environments.at(i).toMap();
        QStringList names = env.value(QStringLiteral("secretNames")).toStringList();
        if (!names.contains(name))
            continue;

        names.removeAll(name);
        env[QStringLiteral("secretNames")] = names;
        m_environments[i] = env;

        if (!m_demoMode) {
            updates.push_back(aibox::ffi::EnvSecretUpdateFfi{
                aibox::ffi::toRust(env.value(QStringLiteral("name")).toString()),
                aibox::ffi::toRustVec(names),
            });
        }
    }

    if (updates.empty() && m_demoMode) {
        emit environmentsChanged();
        return;
    }
    if (updates.empty() && !m_demoMode)
        return;

    emit environmentsChanged();
    if (m_demoMode)
        return;

    const QString fullScript =
        aibox::ffi::fromRust(aibox::ffi::secrets_build_clear_global_script(std::move(updates)));
    runShellTask(tr("Removing secret '%1' from environments").arg(name), fullScript);
}

QVariantList Backend::secretsList() const {
    return aibox::ffi::fromRustJson(aibox::ffi::secrets_list_json(
                                        asSlice(toRustPairs(m_secrets)),
                                        asSlice(aibox::ffi::toRustVec(m_hiddenGlobalSecrets))))
        .toList();
}

QStringList Backend::allSecretValues() const {
    return aibox::ffi::fromRustVec(aibox::ffi::secrets_all_values(
        asSlice(toRustPairs(m_secrets)), asSlice(aibox::ffi::toRustVec(m_instanceSecretValues))));
}

QString Backend::saveSecret(const QString &name, const QString &value) {
    const auto input = makeSecretsInput(
        m_secretsPath, m_secrets, m_secretsError, m_hiddenGlobalSecrets,
        m_instanceSecretValues, m_environmentSecretsEnv, m_environmentSecrets,
        m_environmentSecretsError, m_demoMode);
    const auto out = aibox::ffi::secrets_save_global(
        input, aibox::ffi::toRust(name), aibox::ffi::toRust(value));
    const QString err = aibox::ffi::fromRust(out.error);
    if (!err.isEmpty())
        return err;

    m_secrets = fromRustPairs(out.secrets);
    m_hiddenGlobalSecrets = aibox::ffi::fromRustVec(out.hidden_global_secrets);
    m_secretsError = aibox::ffi::fromRust(out.secrets_error);
    emit secretsChanged();
    return {};
}

QString Backend::deleteSecret(const QString &name) {
    const auto input = makeSecretsInput(
        m_secretsPath, m_secrets, m_secretsError, m_hiddenGlobalSecrets,
        m_instanceSecretValues, m_environmentSecretsEnv, m_environmentSecrets,
        m_environmentSecretsError, m_demoMode);
    const auto out = aibox::ffi::secrets_delete_global(input, aibox::ffi::toRust(name));
    const QString err = aibox::ffi::fromRust(out.error);
    if (!err.isEmpty())
        return err;

    m_secrets = fromRustPairs(out.secrets);
    m_hiddenGlobalSecrets = aibox::ffi::fromRustVec(out.hidden_global_secrets);
    m_secretsError = aibox::ffi::fromRust(out.secrets_error);
    emit secretsChanged();
    return {};
}

QString Backend::setGlobalSecretInstanceVisible(const QString &name, bool visible) {
    const auto input = makeSecretsInput(
        m_secretsPath, m_secrets, m_secretsError, m_hiddenGlobalSecrets,
        m_instanceSecretValues, m_environmentSecretsEnv, m_environmentSecrets,
        m_environmentSecretsError, m_demoMode);
    const auto out = aibox::ffi::secrets_set_global_visible(
        input, aibox::ffi::toRust(name), visible);
    const QString err = aibox::ffi::fromRust(out.error);
    if (!err.isEmpty())
        return err;

    m_secrets = fromRustPairs(out.secrets);
    m_hiddenGlobalSecrets = aibox::ffi::fromRustVec(out.hidden_global_secrets);
    m_secretsError = aibox::ffi::fromRust(out.secrets_error);
    if (!visible)
        clearGlobalSecretFromEnvironments(name.trimmed());
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
    const QStringList assigned =
        environmentByName(env).value(QStringLiteral("secretNames")).toStringList();
    const auto input = makeSecretsInput(
        m_secretsPath, m_secrets, m_secretsError, m_hiddenGlobalSecrets,
        m_instanceSecretValues, m_environmentSecretsEnv, m_environmentSecrets,
        m_environmentSecretsError, m_demoMode);
    const auto out = aibox::ffi::secrets_save_environment(
        input, aibox::ffi::toRust(envName), aibox::ffi::toRust(name),
        aibox::ffi::toRust(value), asSlice(aibox::ffi::toRustVec(assigned)));
    const QString err = aibox::ffi::fromRust(out.error);
    if (!err.isEmpty())
        return err;

    m_instanceSecretValues = aibox::ffi::fromRustVec(out.instance_secret_values);
    m_environmentSecretsEnv = aibox::ffi::fromRust(out.environment_secrets_env);
    m_environmentSecrets = aibox::ffi::fromRustJson(out.environment_secrets_json).toList();
    m_environmentSecretsError = aibox::ffi::fromRust(out.environment_secrets_error);
    if (m_demoMode)
        emit environmentSecretsChanged();
    else {
        if (m_environmentSecretsEnv == env)
            emit environmentSecretsChanged();
        emit secretsChanged();
    }
    return {};
}

QString Backend::deleteEnvironmentSecret(const QString &envName, const QString &name) {
    const QString env = envName.trimmed();
    const QStringList assigned =
        environmentByName(env).value(QStringLiteral("secretNames")).toStringList();
    const auto input = makeSecretsInput(
        m_secretsPath, m_secrets, m_secretsError, m_hiddenGlobalSecrets,
        m_instanceSecretValues, m_environmentSecretsEnv, m_environmentSecrets,
        m_environmentSecretsError, m_demoMode);
    const auto out = aibox::ffi::secrets_delete_environment(
        input, aibox::ffi::toRust(envName), aibox::ffi::toRust(name),
        asSlice(aibox::ffi::toRustVec(assigned)));
    const QString err = aibox::ffi::fromRust(out.error);
    if (!err.isEmpty())
        return err;

    m_instanceSecretValues = aibox::ffi::fromRustVec(out.instance_secret_values);
    m_environmentSecretsEnv = aibox::ffi::fromRust(out.environment_secrets_env);
    m_environmentSecrets = aibox::ffi::fromRustJson(out.environment_secrets_json).toList();
    m_environmentSecretsError = aibox::ffi::fromRust(out.environment_secrets_error);
    if (m_demoMode)
        emit environmentSecretsChanged();
    else {
        if (m_environmentSecretsEnv == env)
            emit environmentSecretsChanged();
        emit secretsChanged();
    }
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
    const QStringList assigned =
        environmentByName(env).value(QStringLiteral("secretNames")).toStringList();
    const auto input = makeSecretsInput(
        m_secretsPath, m_secrets, m_secretsError, m_hiddenGlobalSecrets,
        m_instanceSecretValues, m_environmentSecretsEnv, m_environmentSecrets,
        m_environmentSecretsError, m_demoMode);
    const auto out = aibox::ffi::secrets_set_local_enabled(
        input, aibox::ffi::toRust(envName), aibox::ffi::toRust(name), enabled,
        asSlice(aibox::ffi::toRustVec(assigned)));
    const QString err = aibox::ffi::fromRust(out.error);
    if (!err.isEmpty())
        return err;

    m_environmentSecrets = aibox::ffi::fromRustJson(out.environment_secrets_json).toList();
    m_environmentSecretsError = aibox::ffi::fromRust(out.environment_secrets_error);
    if (m_demoMode || m_environmentSecretsEnv == env)
        emit environmentSecretsChanged();
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

    const QString script = aibox::ffi::fromRust(aibox::ffi::secrets_build_set_env_script(
        aibox::ffi::toRust(name), asSlice(aibox::ffi::toRustVec(cleanNames))));
    runShellTask(tr("Updating secrets for '%1'").arg(name), script, {}, name);
}

QList<QPair<QString, QString>> Backend::resolveEnvironmentSecrets(const QString &envName,
                                                                  QString *errorOut) const {
    const QStringList assigned =
        environmentByName(envName).value(QStringLiteral("secretNames")).toStringList();
    const auto input = makeSecretsInput(
        m_secretsPath, m_secrets, m_secretsError, m_hiddenGlobalSecrets,
        m_instanceSecretValues, m_environmentSecretsEnv, m_environmentSecrets,
        m_environmentSecretsError, m_demoMode);
    const auto res = aibox::ffi::secrets_resolve_environment(
        input, aibox::ffi::toRust(envName), asSlice(aibox::ffi::toRustVec(assigned)));
    if (errorOut)
        *errorOut = aibox::ffi::fromRust(res.error);
    return fromRustPairs(res.pairs);
}

bool Backend::pushEnvironmentSecretsToContainer(const QString &envName, QString *outRemoteEnvPath,
                                                QString *errorOut) const {
    if (outRemoteEnvPath)
        outRemoteEnvPath->clear();
    if (errorOut)
        errorOut->clear();

    const QStringList assigned =
        environmentByName(envName).value(QStringLiteral("secretNames")).toStringList();
    const auto input = makeSecretsInput(
        m_secretsPath, m_secrets, m_secretsError, m_hiddenGlobalSecrets,
        m_instanceSecretValues, m_environmentSecretsEnv, m_environmentSecrets,
        m_environmentSecretsError, m_demoMode);
    const auto res = aibox::ffi::secrets_push_to_container(
        input, aibox::ffi::toRust(envName), asSlice(aibox::ffi::toRustVec(assigned)));
    if (outRemoteEnvPath)
        *outRemoteEnvPath = aibox::ffi::fromRust(res.remote_env_path);
    if (errorOut)
        *errorOut = aibox::ffi::fromRust(res.error);
    return res.ok;
}
