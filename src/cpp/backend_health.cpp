#include "backend.h"
#include "backend_helpers.h"
#include "command_runner.h"
#include "rust_bridge.h"
#include "aibox-ffi/src/health.rs.h"

#include <QDateTime>
#include <QFile>
#include <QJsonDocument>
#include <QJsonObject>
#include <QProcess>
#include <QStandardPaths>

#include <vector>

namespace {

aibox::ffi::StoragePoolInfoFfi toFfiPool(const Backend::StoragePoolInfo &pool) {
    return aibox::ffi::StoragePoolInfoFfi{
        pool.valid,
        aibox::ffi::toRust(pool.name),
        aibox::ffi::toRust(pool.driver),
        pool.usedBytes,
        pool.totalBytes,
        pool.usedPercent,
        aibox::ffi::toRust(pool.sizeText),
        pool.resizable,
    };
}

Backend::StoragePoolInfo fromFfiPool(const aibox::ffi::StoragePoolInfoFfi &pool) {
    Backend::StoragePoolInfo out;
    out.valid = pool.valid;
    out.name = aibox::ffi::fromRust(pool.name);
    out.driver = aibox::ffi::fromRust(pool.driver);
    out.usedBytes = pool.used_bytes;
    out.totalBytes = pool.total_bytes;
    out.usedPercent = pool.used_percent;
    out.sizeText = aibox::ffi::fromRust(pool.size_text);
    out.resizable = pool.resizable;
    return out;
}

Backend::HealthCheckResult fromFfiHealthResult(const aibox::ffi::HealthCheckResultFfi &r) {
    Backend::HealthCheckResult out;
    out.id = aibox::ffi::fromRust(r.id);
    out.title = aibox::ffi::fromRust(r.title);
    out.ok = r.ok;
    out.warning = r.warning;
    out.repaired = r.repaired;
    out.needsRoot = r.needs_root;
    out.detail = aibox::ffi::fromRust(r.detail);
    out.fixCommand = aibox::ffi::fromRust(r.fix_command);
    return out;
}

} // namespace

Backend::WatchdogDecision Backend::evaluateWatchdog(int percent, int previousLevel, int warnThreshold,
                                                    int criticalThreshold, bool autoStopEnabled) {
    const aibox::ffi::WatchdogDecisionFfi d = aibox::ffi::evaluate_watchdog(
        percent, previousLevel, warnThreshold, criticalThreshold, autoStopEnabled);
    return WatchdogDecision{d.new_level, d.should_notify, d.should_auto_stop};
}

void Backend::evaluateAllWatchdogs() {
    if (!m_watchdogEnabled || m_demoMode)
        return;

    const bool canNotify = isGuiSession() &&
                           !QStandardPaths::findExecutable(QStringLiteral("notify-send")).isEmpty();

    std::vector<aibox::ffi::WatchdogEnvInputFfi> inputs;
    inputs.reserve(static_cast<std::size_t>(m_environments.size()));
    for (int i = 0; i < m_environments.size(); ++i) {
        const QVariantMap env = m_environments.at(i).toMap();
        const QString name = env.value(QStringLiteral("name")).toString();
        inputs.push_back(aibox::ffi::WatchdogEnvInputFfi{
            aibox::ffi::toRust(name),
            env.value(QStringLiteral("running")).toBool(),
            env.value(QStringLiteral("diskPercent")).toInt(),
            aibox::ffi::toRust(env.value(QStringLiteral("diskUsage")).toString()),
            aibox::ffi::toRust(env.value(QStringLiteral("diskLimit")).toString()),
            env.value(QStringLiteral("diskAlertLevel")).toInt(),
            env.value(QStringLiteral("autoStopOnCritical")).toBool(),
            env.value(QStringLiteral("cpuPercent")).toInt(),
            env.value(QStringLiteral("memoryPercent")).toInt(),
            m_watchdogDiskLevel.value(name, 0),
            m_watchdogCpuLevel.value(name, 0),
            m_watchdogCpuHighStreak.value(name, 0),
            m_watchdogRamLevel.value(name, 0),
            m_watchdogRamHighStreak.value(name, 0),
        });
    }

    const aibox::ffi::WatchdogBatchResultFfi batch = aibox::ffi::evaluate_all_watchdogs(
        rust::Slice<const aibox::ffi::WatchdogEnvInputFfi>(inputs.data(), inputs.size()),
        m_watchdogDiskWarn, m_watchdogDiskCritical, m_watchdogCpuWarn, m_watchdogRamWarn, m_busy);

    for (std::size_t i = 0; i < batch.envs.size() && i < static_cast<std::size_t>(m_environments.size()); ++i) {
        const auto &er = batch.envs[i];
        const QString name = aibox::ffi::fromRust(er.name);
        m_watchdogDiskLevel.insert(name, er.new_disk_level);
        m_watchdogCpuLevel.insert(name, er.new_cpu_level);
        m_watchdogCpuHighStreak.insert(name, er.new_cpu_streak);
        m_watchdogRamLevel.insert(name, er.new_ram_level);
        m_watchdogRamHighStreak.insert(name, er.new_ram_streak);
        if (er.disk_alert_level_changed) {
            QVariantMap env = m_environments.at(static_cast<qsizetype>(i)).toMap();
            env[QStringLiteral("diskAlertLevel")] = er.new_disk_level;
            m_environments[static_cast<qsizetype>(i)] = env;
        }
    }

    if (canNotify) {
        for (const auto &n : batch.notifications) {
            const QString envName = aibox::ffi::fromRust(n.env_name);
            QString msg;
            if (n.kind == 0 || n.kind == 1) {
                const QString used = aibox::ffi::fromRust(n.disk_usage);
                const QString limit = aibox::ffi::fromRust(n.disk_limit);
                msg = (n.kind == 1)
                          ? tr("Disk critical (%1%) in '%2' (%3 of %4).").arg(n.percent).arg(envName, used, limit)
                          : tr("Disk warning (%1%) in '%2' (%3 of %4).").arg(n.percent).arg(envName, used, limit);
                if (n.auto_stopping)
                    msg += QStringLiteral(" ") + tr("Auto-stopping container to protect host.");
                QProcess::startDetached(QStringLiteral("notify-send"),
                                        {QStringLiteral("-u"), n.critical ? QStringLiteral("critical") : QStringLiteral("normal"),
                                         QStringLiteral("-a"), QStringLiteral("AIBox"), QStringLiteral("AIBox Watchdog"), msg});
            } else if (n.kind == 2) {
                QProcess::startDetached(QStringLiteral("notify-send"),
                                        {QStringLiteral("-a"), QStringLiteral("AIBox"), QStringLiteral("AIBox Watchdog"),
                                         tr("Sustained high CPU (%1%) in '%2'.").arg(n.percent).arg(envName)});
            } else {
                QProcess::startDetached(QStringLiteral("notify-send"),
                                        {QStringLiteral("-a"), QStringLiteral("AIBox"), QStringLiteral("AIBox Watchdog"),
                                         tr("High memory pressure (%1%) in '%2'.").arg(n.percent).arg(envName)});
            }
        }
    }

    const QString autoStopTarget = aibox::ffi::fromRust(batch.auto_stop_target);
    if (!autoStopTarget.isEmpty()) {
        runShellTask(tr("Watchdog auto-stopping '%1' (disk >= %2%)").arg(autoStopTarget).arg(m_watchdogDiskCritical),
                     aibox::ffi::fromRust(batch.auto_stop_script), {}, autoStopTarget);
    }
}

void Backend::evaluateIdleAutoStops() {
    if (m_demoMode)
        return;

    bool anyTracking = false;
    for (const auto &v : m_environments) {
        const QVariantMap env = v.toMap();
        const int eff = effectiveIdleStopMinutes(m_idleStopMinutes,
                                                 parseIdleStopOverride(env.value("idleStopOverride").toString()));
        if (env.value("running").toBool() && !env.value("isTemplate").toBool() && eff > 0) {
            anyTracking = true;
            break;
        }
    }
    if (!anyTracking) {
        m_idleSamples.clear();
        m_idleWarned.clear();
        return;
    }

    QSet<QString> activeExecEnvs;
    const QString user = qEnvironmentVariable("USER").trimmed();
    QProcess pgrepProc;
    QStringList pgrepArgs;
    if (!user.isEmpty())
        pgrepArgs << QStringLiteral("-u") << user;
    pgrepArgs << QStringLiteral("-af") << QStringLiteral("incus exec");
    pgrepProc.start(QStringLiteral("pgrep"), pgrepArgs);
    if (pgrepProc.waitForFinished(800) && pgrepProc.exitCode() == 0)
        activeExecEnvs = parseActiveIncusExecEnvs(QString::fromUtf8(pgrepProc.readAllStandardOutput()));

    const qint64 nowMs = QDateTime::currentMSecsSinceEpoch();
    const qint64 cutoffMs = nowMs - 25LL * 60LL * 60LL * 1000LL;
    const bool canNotify = isGuiSession() &&
                           !QStandardPaths::findExecutable(QStringLiteral("notify-send")).isEmpty();

    QString activeTaskEnv;
    if (m_busy) {
        QFile metaFile(m_stateDir + QStringLiteral("/task.meta"));
        if (metaFile.open(QIODevice::ReadOnly))
            activeTaskEnv = QJsonDocument::fromJson(metaFile.readAll()).object().value(QStringLiteral("targetEnvName")).toString();
    }

    QString idleStopTarget;
    for (const auto &v : m_environments) {
        const QVariantMap env = v.toMap();
        const QString name = env.value("name").toString();
        const bool running = env.value("running").toBool();
        const bool isTemplate = env.value("isTemplate").toBool();
        const int portCount = env.value("portForwardCount").toInt();
        const bool ignorePorts = env.value("idleStopIgnorePorts").toBool();
        const int thresholdMin = effectiveIdleStopMinutes(
            m_idleStopMinutes,
            parseIdleStopOverride(env.value("idleStopOverride").toString()));
        const bool busyOnThisEnv = m_busy && (activeTaskEnv == name || m_pendingEnvName == name);
        const bool hasExec = activeExecEnvs.contains(name);

        if (!running || isTemplate || thresholdMin <= 0 || busyOnThisEnv || hasExec ||
            (portCount > 0 && !ignorePorts)) {
            m_idleSamples.remove(name);
            m_idleWarned.remove(name);
            continue;
        }

        auto &samples = m_idleSamples[name];
        samples.append(IdleCpuSample{nowMs, env.value("cpuPercent").toInt()});
        while (!samples.isEmpty() && samples.first().timestampMs < cutoffMs)
            samples.removeFirst();

        IdleDecisionInput in;
        in.running = running;
        in.isTemplate = isTemplate;
        in.busyOnThisEnv = busyOnThisEnv;
        in.hasActiveExecSession = hasExec;
        in.portForwardCount = portCount;
        in.ignorePortForwards = ignorePorts;
        in.thresholdMinutes = thresholdMin;
        in.nowMs = nowMs;
        in.warningAlreadySent = m_idleWarned.contains(name);
        in.samples = samples;

        const IdleDecisionResult res = evaluateIdleDecision(in);
        if (res.shouldWarnOneMin) {
            m_idleWarned.insert(name);
            if (canNotify) {
                QProcess::startDetached(QStringLiteral("notify-send"),
                                        {QStringLiteral("-a"), QStringLiteral("AIBox"),
                                         QStringLiteral("AIBox Idle Auto-Stop"),
                                         tr("'%1' has been idle and will stop in 1 minute.").arg(name)});
            }
        }
        if (res.shouldStopNow && idleStopTarget.isEmpty() && !m_busy)
            idleStopTarget = name;
    }

    if (!idleStopTarget.isEmpty()) {
        m_idleSamples.remove(idleStopTarget);
        m_idleWarned.remove(idleStopTarget);
        runShellTask(tr("Idle auto-stopping '%1' (reason: idle)").arg(idleStopTarget),
                     stopEnvironmentScript(idleStopTarget, true),
                     {}, idleStopTarget, QStringLiteral("idle"));
    }
}

void Backend::setEnvironmentAutoStop(const QString &name, bool enabled) {
    if (name.isEmpty() || m_busy)
        return;
    runShellTask(enabled ? tr("Enabling critical disk auto-stop for '%1'").arg(name)
                         : tr("Disabling critical disk auto-stop for '%1'").arg(name),
                 aibox::ffi::fromRust(aibox::ffi::set_environment_auto_stop_script(
                     aibox::ffi::toRust(name), enabled)),
                 {}, name);
}

void Backend::setEnvironmentIdleStop(const QString &name, const QString &value, bool allowWithPortForwards) {
    if (name.isEmpty() || m_busy)
        return;
    m_idleSamples.remove(name);
    m_idleWarned.remove(name);
    runShellTask(tr("Updating idle auto-stop policy for '%1'").arg(name),
                 aibox::ffi::fromRust(aibox::ffi::set_environment_idle_stop_script(
                     aibox::ffi::toRust(name), aibox::ffi::toRust(value), allowWithPortForwards)),
                 {}, name);
}

bool Backend::isValidProcessLimit(const QString &value) {
    return aibox::ffi::is_valid_process_limit(aibox::ffi::toRust(value));
}

bool Backend::isValidIoLimit(const QString &value) {
    return aibox::ffi::is_valid_io_limit(aibox::ffi::toRust(value));
}

QList<Backend::HealthCheckResult> Backend::runHealthChecks(bool fix, const CommandRunner &customRunner,
                                                           const QString &customUser,
                                                           const QString &ufwRulesContent,
                                                           const QString &workspaceBase) {
    std::function<void(rust::Str, rust::Slice<const rust::String>, std::int32_t &,
                       rust::String &, rust::String &)>
        adapter;
    if (customRunner) {
        adapter = [&customRunner](rust::Str program, rust::Slice<const rust::String> args,
                                  std::int32_t &exitCode, rust::String &stdOut, rust::String &stdErr) {
            QStringList qtArgs;
            qtArgs.reserve(static_cast<qsizetype>(args.size()));
            for (const auto &a : args)
                qtArgs.append(aibox::ffi::fromRust(a));
            const CommandOutput out = customRunner(
                QString::fromUtf8(program.data(), static_cast<qsizetype>(program.size())), qtArgs);
            exitCode = out.exitCode;
            stdOut = aibox::ffi::toRust(out.stdOut);
            stdErr = aibox::ffi::toRust(out.stdErr);
        };
    }
    const aibox::ffi::CommandRunnerBridge bridge{customRunner ? &adapter : nullptr};
    const rust::Vec<aibox::ffi::HealthCheckResultFfi> ffiResults = aibox::ffi::run_health_checks(
        fix, static_cast<bool>(customRunner), bridge, aibox::ffi::toRust(customUser),
        aibox::ffi::toRust(ufwRulesContent), aibox::ffi::toRust(workspaceBase));

    QList<HealthCheckResult> results;
    results.reserve(static_cast<qsizetype>(ffiResults.size()));
    for (const auto &r : ffiResults)
        results.append(fromFfiHealthResult(r));
    return results;
}

Backend::StoragePoolInfo Backend::parseIncusStorageInfo(const QString &output) {
    return fromFfiPool(aibox::ffi::parse_incus_storage_info(aibox::ffi::toRust(output)));
}

qint64 Backend::sumRootDiskQuotasBytes(const QByteArray &incusListJson,
                                       const QString &excludeEnvName,
                                       const QString &poolName,
                                       int *unboundedCountOut) {
    const aibox::ffi::SumQuotasResultFfi res = aibox::ffi::sum_root_disk_quotas_bytes(
        rust::Str(incusListJson.constData(), static_cast<std::size_t>(incusListJson.size())),
        aibox::ffi::toRust(excludeEnvName), aibox::ffi::toRust(poolName));
    if (unboundedCountOut)
        *unboundedCountOut = res.unbounded_count;
    return res.bytes;
}

QString Backend::evaluatePoolOvercommitWarning(const StoragePoolInfo &pool,
                                             qint64 existingQuotasBytes,
                                             qint64 requestedBytes,
                                             int unboundedCount) {
    return aibox::ffi::fromRust(aibox::ffi::evaluate_pool_overcommit_warning(
        toFfiPool(pool), existingQuotasBytes, requestedBytes, unboundedCount));
}

Backend::HealthCheckResult Backend::evaluatePoolCapacityHealth(const StoragePoolInfo &pool,
                                                               qint64 allocatedQuotasBytes,
                                                               int unboundedCount) {
    return fromFfiHealthResult(aibox::ffi::evaluate_pool_capacity_health(
        toFfiPool(pool), allocatedQuotasBytes, unboundedCount));
}

QString Backend::poolWarningForCreate(const QString &diskSpec) const {
    return aibox::ffi::fromRust(aibox::ffi::pool_warning_for_create(
        toFfiPool(m_lastPoolInfo), m_lastPoolAllocatedBytes, m_lastPoolUnboundedCount,
        aibox::ffi::toRust(diskSpec)));
}

QString Backend::poolWarningForQuotaEdit(const QString &envName, const QString &diskSpec) const {
    rust::Vec<rust::String> names;
    rust::Vec<std::int64_t> quotas;
    if (m_lastListJson.isEmpty()) {
        names.reserve(static_cast<std::size_t>(m_environments.size()));
        quotas.reserve(static_cast<std::size_t>(m_environments.size()));
        for (const auto &v : m_environments) {
            const QVariantMap env = v.toMap();
            names.push_back(aibox::ffi::toRust(env.value(QStringLiteral("name")).toString()));
            quotas.push_back(env.value(QStringLiteral("diskQuotaBytes")).toLongLong());
        }
    }
    return aibox::ffi::fromRust(aibox::ffi::pool_warning_for_quota_edit(
        toFfiPool(m_lastPoolInfo),
        rust::Str(m_lastListJson.constData(), static_cast<std::size_t>(m_lastListJson.size())),
        rust::Slice<const rust::String>(names.data(), names.size()),
        rust::Slice<const std::int64_t>(quotas.data(), quotas.size()),
        aibox::ffi::toRust(envName), aibox::ffi::toRust(diskSpec)));
}

QString Backend::resizeStoragePoolScript(const QString &poolName, const QString &sizeSpec) {
    return aibox::ffi::fromRust(aibox::ffi::resize_storage_pool_script(
        aibox::ffi::toRust(poolName), aibox::ffi::toRust(sizeSpec)));
}

bool Backend::parsePoolSizeSpec(const QString &spec, qint64 *outBytes) {
    const aibox::ffi::PoolSizeSpecResultFfi res =
        aibox::ffi::parse_pool_size_spec(aibox::ffi::toRust(spec));
    if (outBytes)
        *outBytes = res.bytes;
    return res.ok;
}

QString Backend::validateStoragePoolResize(const QString &sizeSpec) const {
    return aibox::ffi::fromRust(aibox::ffi::validate_storage_pool_resize(
        m_busy, isTaskLockActive(), toFfiPool(m_lastPoolInfo), aibox::ffi::toRust(sizeSpec)));
}

QString Backend::resizeStoragePool(const QString &sizeSpec) {
    const QString error = validateStoragePoolResize(sizeSpec);
    if (!error.isEmpty())
        return error;

    const QString cleanSize = sizeSpec.trimmed();
    runShellTask(tr("Growing storage pool '%1' to %2").arg(m_lastPoolInfo.name, cleanSize),
                 resizeStoragePoolScript(m_lastPoolInfo.name, cleanSize));
    return {};
}

void Backend::setPoolStateForTest(const StoragePoolInfo &pool, const QByteArray &incusListJson) {
    m_lastPoolInfo = pool;
    m_lastListJson = incusListJson;
    m_lastPoolUnboundedCount = 0;
    m_lastPoolAllocatedBytes = sumRootDiskQuotasBytes(incusListJson, {}, pool.name, &m_lastPoolUnboundedCount);
    const QVariantMap nextMap = poolInfoToVariantMap(pool, m_lastPoolAllocatedBytes, m_lastPoolUnboundedCount);
    if (nextMap != m_poolInfo) {
        m_poolInfo = nextMap;
        emit poolInfoChanged();
    }
}

Backend::IdleStopOverride Backend::parseIdleStopOverride(const QString &raw) {
    const aibox::ffi::IdleStopOverrideFfi ov = aibox::ffi::parse_idle_stop_override(aibox::ffi::toRust(raw));
    IdleStopOverride out;
    out.mode = (ov.mode == 1) ? IdleStopOverride::Off
                              : ((ov.mode == 2) ? IdleStopOverride::Minutes : IdleStopOverride::Inherit);
    out.minutes = ov.minutes;
    return out;
}

int Backend::effectiveIdleStopMinutes(int globalMinutes, const IdleStopOverride &ov) {
    const aibox::ffi::IdleStopOverrideFfi ffiOv{
        (ov.mode == IdleStopOverride::Off) ? 1 : ((ov.mode == IdleStopOverride::Minutes) ? 2 : 0),
        ov.minutes,
    };
    return aibox::ffi::effective_idle_stop_minutes(globalMinutes, ffiOv);
}

Backend::IdleDecisionResult Backend::evaluateIdleDecision(const IdleDecisionInput &in) {
    rust::Vec<aibox::ffi::IdleCpuSampleFfi> samples;
    samples.reserve(static_cast<std::size_t>(in.samples.size()));
    for (const auto &s : in.samples)
        samples.push_back(aibox::ffi::IdleCpuSampleFfi{s.timestampMs, s.cpuPercent});

    const aibox::ffi::IdleDecisionInputFfi ffiIn{
        in.running,
        in.isTemplate,
        in.busyOnThisEnv,
        in.hasActiveExecSession,
        in.portForwardCount,
        in.ignorePortForwards,
        in.thresholdMinutes,
        in.nowMs,
        in.warningAlreadySent,
        std::move(samples),
    };
    const aibox::ffi::IdleDecisionResultFfi ffiRes = aibox::ffi::evaluate_idle_decision(ffiIn);
    IdleDecisionResult res;
    res.eligible = ffiRes.eligible;
    res.idleDurationMs = ffiRes.idle_duration_ms;
    res.averageCpuPercent = ffiRes.average_cpu_percent;
    res.shouldWarnOneMin = ffiRes.should_warn_one_min;
    res.shouldStopNow = ffiRes.should_stop_now;
    res.blockedReason = aibox::ffi::fromRust(ffiRes.blocked_reason);
    return res;
}
