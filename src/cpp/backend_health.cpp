#include "backend.h"
#include "backend_helpers.h"

#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QRegularExpression>
#include <QSet>
#include <QStandardPaths>

#include <cmath>
#include <unistd.h>

Backend::WatchdogDecision Backend::evaluateWatchdog(int percent, int previousLevel, int warnThreshold,
                                                    int criticalThreshold, bool autoStopEnabled) {
    WatchdogDecision decision;
    const int warn = qBound(50, warnThreshold, 98);
    const int crit = qBound(warn + 1, criticalThreshold, 100);
    const int prev = qBound(0, previousLevel, 2);

    if (percent >= crit) {
        decision.newLevel = 2;
    } else if (percent >= warn) {
        // Keep level 2 until dropping 5 points below critical (hysteresis)
        if (prev == 2 && percent >= crit - 5)
            decision.newLevel = 2;
        else
            decision.newLevel = 1;
    } else {
        // Keep level 1/2 if within 5-point hysteresis band below warn threshold
        if (prev >= 1 && percent >= warn - 5)
            decision.newLevel = 1;
        else
            decision.newLevel = 0;
    }

    decision.shouldNotify = (decision.newLevel > prev);
    decision.shouldAutoStop = (decision.newLevel == 2 && autoStopEnabled);
    return decision;
}

void Backend::evaluateAllWatchdogs() {
    if (!m_watchdogEnabled || m_demoMode)
        return;

    const bool canNotify = isGuiSession() &&
                           !QStandardPaths::findExecutable(QStringLiteral("notify-send")).isEmpty();

    QString autoStopTarget;
    for (int i = 0; i < m_environments.size(); ++i) {
        QVariantMap env = m_environments.at(i).toMap();
        const QString name = env.value("name").toString();
        const bool running = env.value("running").toBool();
        const int diskPct = env.value("diskPercent").toInt();
        const bool envAutoStop = env.value("autoStopOnCritical").toBool();

        const int prevDisk = m_watchdogDiskLevel.value(name, 0);
        const WatchdogDecision d = evaluateWatchdog(diskPct, prevDisk, m_watchdogDiskWarn, m_watchdogDiskCritical,
                                                    running && envAutoStop);
        m_watchdogDiskLevel.insert(name, d.newLevel);

        if (env.value("diskAlertLevel").toInt() != d.newLevel) {
            env["diskAlertLevel"] = d.newLevel;
            m_environments[i] = env;
        }

        if (d.shouldNotify && canNotify) {
            const bool isCrit = (d.newLevel == 2);
            QString msg = isCrit
                              ? tr("Disk critical (%1%) in '%2' (%3 of %4).")
                                    .arg(diskPct)
                                    .arg(name, env.value("diskUsage").toString(), env.value("diskLimit").toString())
                              : tr("Disk warning (%1%) in '%2' (%3 of %4).")
                                    .arg(diskPct)
                                    .arg(name, env.value("diskUsage").toString(), env.value("diskLimit").toString());
            if (d.shouldAutoStop)
                msg += QStringLiteral(" ") + tr("Auto-stopping container to protect host.");
            QProcess::startDetached(QStringLiteral("notify-send"),
                                    {QStringLiteral("-u"), isCrit ? QStringLiteral("critical") : QStringLiteral("normal"),
                                     QStringLiteral("-a"), QStringLiteral("AIBox"), QStringLiteral("AIBox Watchdog"), msg});
        }

        if (d.shouldAutoStop && autoStopTarget.isEmpty() && !m_busy)
            autoStopTarget = name;

        // Sustained CPU / RAM streak alerts (warn after 3 consecutive high polls when running)
        if (running) {
            const int cpuPct = env.value("cpuPercent").toInt();
            const int cpuStreak = (cpuPct >= m_watchdogCpuWarn) ? (m_watchdogCpuHighStreak.value(name, 0) + 1) : 0;
            m_watchdogCpuHighStreak.insert(name, cpuStreak);
            const int prevCpuLvl = m_watchdogCpuLevel.value(name, 0);
            if (cpuStreak >= 3 && prevCpuLvl == 0) {
                m_watchdogCpuLevel.insert(name, 1);
                if (canNotify) {
                    QProcess::startDetached(QStringLiteral("notify-send"),
                                            {QStringLiteral("-a"), QStringLiteral("AIBox"),
                                             QStringLiteral("AIBox Watchdog"),
                                             tr("Sustained high CPU (%1%) in '%2'.").arg(cpuPct).arg(name)});
                }
            } else if (cpuPct < m_watchdogCpuWarn - 10) {
                m_watchdogCpuLevel.insert(name, 0);
            }

            const int ramPct = env.value("memoryPercent").toInt();
            const int ramStreak = (ramPct >= m_watchdogRamWarn) ? (m_watchdogRamHighStreak.value(name, 0) + 1) : 0;
            m_watchdogRamHighStreak.insert(name, ramStreak);
            const int prevRamLvl = m_watchdogRamLevel.value(name, 0);
            if (ramStreak >= 3 && prevRamLvl == 0) {
                m_watchdogRamLevel.insert(name, 1);
                if (canNotify) {
                    QProcess::startDetached(QStringLiteral("notify-send"),
                                            {QStringLiteral("-a"), QStringLiteral("AIBox"),
                                             QStringLiteral("AIBox Watchdog"),
                                             tr("High memory pressure (%1%) in '%2'.").arg(ramPct).arg(name)});
                }
            } else if (ramPct < m_watchdogRamWarn - 10) {
                m_watchdogRamLevel.insert(name, 0);
            }
        } else {
            m_watchdogCpuHighStreak.insert(name, 0);
            m_watchdogCpuLevel.insert(name, 0);
            m_watchdogRamHighStreak.insert(name, 0);
            m_watchdogRamLevel.insert(name, 0);
        }
    }

    if (!autoStopTarget.isEmpty()) {
        runShellTask(tr("Watchdog auto-stopping '%1' (disk >= %2%)").arg(autoStopTarget).arg(m_watchdogDiskCritical),
                     QStringLiteral("echo '@@STAGE:50:Stopping container on critical disk quota...'; "
                                    "incus stop %1 --timeout=15 || incus stop %1 --force; "
                                    "echo '@@STAGE:100:Stopped by watchdog.'")
                         .arg(shellQuote(autoStopTarget)),
                     {}, autoStopTarget);
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
                 QStringLiteral("echo '@@STAGE:50:Updating watchdog auto-stop policy...'; "
                                "incus config set %1 user.aibox.autostop %2; "
                                "echo '@@STAGE:100:Updated.'")
                     .arg(shellQuote(name), enabled ? QStringLiteral("true") : QStringLiteral("false")),
                 {}, name);
}

void Backend::setEnvironmentIdleStop(const QString &name, const QString &value, bool allowWithPortForwards) {
    if (name.isEmpty() || m_busy)
        return;
    const QString norm = value.trimmed().toLower();
    const IdleStopOverride parsed = parseIdleStopOverride(norm);
    const QString stored = (parsed.mode == IdleStopOverride::Off)
                               ? QStringLiteral("off")
                               : (parsed.mode == IdleStopOverride::Minutes
                                      ? QString::number(parsed.minutes)
                                      : QStringLiteral("inherit"));
    m_idleSamples.remove(name);
    m_idleWarned.remove(name);
    runShellTask(tr("Updating idle auto-stop policy for '%1'").arg(name),
                 QStringLiteral("echo '@@STAGE:50:Updating idle auto-stop policy...'; "
                                "incus config set %1 user.aibox.idlestop %2 user.aibox.idlestop_ports %3; "
                                "echo '@@STAGE:100:Updated.'")
                     .arg(shellQuote(name), shellQuote(stored),
                          allowWithPortForwards ? QStringLiteral("true") : QStringLiteral("false")),
                 {}, name);
}

bool Backend::isValidProcessLimit(const QString &value) {
    const QString trimmed = value.trimmed();
    if (trimmed.isEmpty() || trimmed.compare(QLatin1String("unlimited"), Qt::CaseInsensitive) == 0)
        return true;
    static const QRegularExpression re(QStringLiteral("^[1-9][0-9]*$"));
    if (!re.match(trimmed).hasMatch())
        return false;
    bool ok = false;
    const int count = trimmed.toInt(&ok);
    return ok && count >= 10 && count <= 1000000;
}

bool Backend::isValidIoLimit(const QString &value) {
    const QString trimmed = value.trimmed();
    if (trimmed.isEmpty() || trimmed.compare(QLatin1String("unlimited"), Qt::CaseInsensitive) == 0)
        return true;
    static const QRegularExpression re(QStringLiteral("^[1-9][0-9]*(?:B|kB|KB|MB|GB|KiB|MiB|GiB|iops)$"));
    return re.match(trimmed).hasMatch();
}

QList<Backend::HealthCheckResult> Backend::runHealthChecks(bool fix, const CommandRunner &customRunner,
                                                           const QString &customUser,
                                                           const QString &ufwRulesContent,
                                                           const QString &workspaceBase) {
    CommandRunner run = customRunner;
    if (!run) {
        run = [](const QString &program, const QStringList &args) -> CommandOutput {
            CommandOutput out;
            if (program == QLatin1String("__socket_rw__") && !args.isEmpty()) {
                const QString sock = args.first();
                if (!QFileInfo::exists(sock)) {
                    out.exitCode = 1;
                    out.stdErr = QStringLiteral("Socket file not found.");
                    return out;
                }
                const bool rw = (::access(sock.toLocal8Bit().constData(), R_OK | W_OK) == 0);
                out.exitCode = rw ? 0 : 1;
                out.stdErr = rw ? QString() : QStringLiteral("No read/write access on socket.");
                return out;
            }
            QProcess proc;
            proc.start(program, args);
            if (!proc.waitForFinished(8000)) {
                proc.kill();
                out.exitCode = 124;
                out.stdErr = QStringLiteral("Command timed out.");
                return out;
            }
            out.exitCode = proc.exitCode();
            out.stdOut = QString::fromUtf8(proc.readAllStandardOutput());
            out.stdErr = QString::fromUtf8(proc.readAllStandardError());
            return out;
        };
    }

    QString user = customUser.trimmed();
    if (user.isEmpty())
        user = qEnvironmentVariable("USER").trimmed();
    if (user.isEmpty())
        user = qEnvironmentVariable("LOGNAME").trimmed();

    QList<HealthCheckResult> results;

    // 1. User group memberships
    {
        HealthCheckResult r;
        r.id = QStringLiteral("groups");
        r.title = QStringLiteral("User group membership");
        const CommandOutput out = run(QStringLiteral("id"), {QStringLiteral("-nG"), user});
        const QStringList groups = out.stdOut.split(QRegularExpression(QStringLiteral("\\s+")), Qt::SkipEmptyParts);
        const QStringList required = {QStringLiteral("incus-admin"), QStringLiteral("kvm"),
                                      QStringLiteral("render"), QStringLiteral("video")};
        QStringList missing;
        for (const QString &req : required) {
            if (!groups.contains(req))
                missing << req;
        }
        if (out.exitCode == 0 && missing.isEmpty()) {
            r.ok = true;
            r.detail = QStringLiteral("User '%1' belongs to incus-admin, kvm, render, video.").arg(user);
        } else {
            r.ok = false;
            r.needsRoot = true;
            r.detail = missing.isEmpty()
                           ? QStringLiteral("Could not inspect groups for '%1'.").arg(user)
                           : QStringLiteral("Missing group(s): %1").arg(missing.join(QStringLiteral(", ")));
            r.fixCommand = QStringLiteral("sudo usermod -aG incus-admin,kvm,render,video %1").arg(user);
        }
        results.append(r);
    }

    // 2. Incus UNIX socket ACL
    {
        HealthCheckResult r;
        r.id = QStringLiteral("socket_acl");
        r.title = QStringLiteral("Incus socket ACL");
        const CommandOutput out = run(QStringLiteral("__socket_rw__"), {QStringLiteral("/var/lib/incus/unix.socket")});
        if (out.exitCode == 0) {
            r.ok = true;
            r.detail = QStringLiteral("Read/write access confirmed on /var/lib/incus/unix.socket.");
        } else {
            r.ok = false;
            r.needsRoot = true;
            r.detail = out.stdErr.trimmed().isEmpty()
                           ? QStringLiteral("Missing R/W permission on /var/lib/incus/unix.socket.")
                           : out.stdErr.trimmed();
            r.fixCommand = QStringLiteral("sudo setfacl -m u:%1:rw /var/lib/incus/unix.socket").arg(user);
        }
        results.append(r);
    }

    // 3. Incus daemon reachability
    bool daemonReachable = false;
    QByteArray cachedListJson;
    {
        HealthCheckResult r;
        r.id = QStringLiteral("incus_daemon");
        r.title = QStringLiteral("Incus daemon reachable");
        const CommandOutput out = run(QStringLiteral("incus"), {QStringLiteral("list"), QStringLiteral("--format=json")});
        if (out.exitCode == 0) {
            daemonReachable = true;
            cachedListJson = out.stdOut.toUtf8();
            r.ok = true;
            r.detail = QStringLiteral("Incus daemon responded cleanly.");
        } else {
            r.ok = false;
            r.needsRoot = true;
            r.detail = out.stdErr.trimmed().isEmpty()
                           ? QStringLiteral("Incus daemon is not reachable.")
                           : out.stdErr.trimmed();
            r.fixCommand = QStringLiteral("sudo systemctl enable --now incus.socket incus.service");
        }
        results.append(r);
    }

    // 4. Bridge network incusbr0
    {
        HealthCheckResult r;
        r.id = QStringLiteral("bridge_incusbr0");
        r.title = QStringLiteral("Bridge network incusbr0");
        auto checkBridge = [&]() -> bool {
            const CommandOutput out = run(QStringLiteral("incus"), {QStringLiteral("network"), QStringLiteral("show"), QStringLiteral("incusbr0")});
            return out.exitCode == 0;
        };
        if (checkBridge()) {
            r.ok = true;
            r.detail = QStringLiteral("Network bridge incusbr0 is configured.");
        } else {
            if (fix && daemonReachable) {
                const CommandOutput rep = run(QStringLiteral("incus"),
                                              {QStringLiteral("network"), QStringLiteral("create"), QStringLiteral("incusbr0"),
                                               QStringLiteral("ipv4.address=auto"), QStringLiteral("ipv4.nat=true"),
                                               QStringLiteral("ipv6.address=none")});
                if (rep.exitCode == 0 && checkBridge()) {
                    r.ok = true;
                    r.repaired = true;
                    r.detail = QStringLiteral("Created bridge incusbr0 automatically.");
                }
            }
            if (!r.ok) {
                r.needsRoot = !daemonReachable;
                r.detail = QStringLiteral("Managed bridge incusbr0 is missing.");
                r.fixCommand = QStringLiteral("incus network create incusbr0 ipv4.address=auto ipv4.nat=true ipv6.address=none");
            }
        }
        results.append(r);
    }

    // 5. ai-gui profile
    {
        HealthCheckResult r;
        r.id = QStringLiteral("ai_gui_profile");
        r.title = QStringLiteral("ai-gui profile (nesting, gpu, kvm)");
        auto inspectProfile = [&]() -> std::pair<bool, QString> {
            const CommandOutput out = run(QStringLiteral("incus"), {QStringLiteral("profile"), QStringLiteral("show"), QStringLiteral("ai-gui")});
            if (out.exitCode != 0)
                return {false, QStringLiteral("Profile ai-gui does not exist.")};
            const QString txt = out.stdOut;
            QStringList missing;
            if (!txt.contains(QStringLiteral("security.nesting")))
                missing << QStringLiteral("security.nesting=true");
            if (!txt.contains(QStringLiteral("gpu:")))
                missing << QStringLiteral("gpu");
            if (!txt.contains(QStringLiteral("kvm:")) && !txt.contains(QStringLiteral("/dev/kvm")))
                missing << QStringLiteral("kvm");
            if (!missing.isEmpty())
                return {false, QStringLiteral("Profile ai-gui missing: %1").arg(missing.join(QStringLiteral(", ")))};
            return {true, QStringLiteral("Profile ai-gui configured with nesting, gpu, and kvm.")};
        };

        auto [okNow, detailNow] = inspectProfile();
        if (okNow) {
            r.ok = true;
            r.detail = detailNow;
        } else {
            if (fix && daemonReachable) {
                const CommandOutput showBefore = run(QStringLiteral("incus"), {QStringLiteral("profile"), QStringLiteral("show"), QStringLiteral("ai-gui")});
                if (showBefore.exitCode != 0)
                    run(QStringLiteral("incus"), {QStringLiteral("profile"), QStringLiteral("create"), QStringLiteral("ai-gui")});
                run(QStringLiteral("incus"), {QStringLiteral("profile"), QStringLiteral("set"), QStringLiteral("ai-gui"), QStringLiteral("security.nesting=true")});
                const CommandOutput showMid = run(QStringLiteral("incus"), {QStringLiteral("profile"), QStringLiteral("show"), QStringLiteral("ai-gui")});
                if (!showMid.stdOut.contains(QStringLiteral("gpu:")))
                    run(QStringLiteral("incus"), {QStringLiteral("profile"), QStringLiteral("device"), QStringLiteral("add"), QStringLiteral("ai-gui"), QStringLiteral("gpu"), QStringLiteral("gpu")});
                if (!showMid.stdOut.contains(QStringLiteral("kvm:")) && !showMid.stdOut.contains(QStringLiteral("/dev/kvm")))
                    run(QStringLiteral("incus"), {QStringLiteral("profile"), QStringLiteral("device"), QStringLiteral("add"), QStringLiteral("ai-gui"), QStringLiteral("kvm"), QStringLiteral("unix-char"), QStringLiteral("path=/dev/kvm")});

                auto [okAfter, detailAfter] = inspectProfile();
                if (okAfter) {
                    r.ok = true;
                    r.repaired = true;
                    r.detail = QStringLiteral("Repaired ai-gui profile automatically.");
                } else {
                    detailNow = detailAfter;
                }
            }
            if (!r.ok) {
                r.needsRoot = !daemonReachable;
                r.detail = detailNow;
                r.fixCommand = QStringLiteral("incus profile create ai-gui; incus profile set ai-gui security.nesting=true; incus profile device add ai-gui gpu gpu; incus profile device add ai-gui kvm unix-char path=/dev/kvm");
            }
        }
        results.append(r);
    }

    // 6. UFW firewall rules for incusbr0
    {
        HealthCheckResult r;
        r.id = QStringLiteral("ufw_rules");
        r.title = QStringLiteral("UFW firewall rules for incusbr0");
        const CommandOutput ufwState = run(QStringLiteral("systemctl"), {QStringLiteral("is-active"), QStringLiteral("ufw")});
        if (ufwState.stdOut.trimmed() != QStringLiteral("active")) {
            r.ok = true;
            r.detail = QStringLiteral("UFW firewall is inactive.");
        } else {
            QString rules = ufwRulesContent;
            bool readOk = !rules.isEmpty();
            if (!readOk) {
                QFile rf(QStringLiteral("/etc/ufw/user.rules"));
                if (rf.open(QIODevice::ReadOnly)) {
                    rules = QString::fromUtf8(rf.readAll());
                    readOk = true;
                }
            }
            const bool hasIn = rules.contains(QRegularExpression(QStringLiteral("ufw-user-input\\s+-i\\s+incusbr0\\s+-j\\s+ACCEPT")));
            const bool hasFwdIn = rules.contains(QRegularExpression(QStringLiteral("ufw-user-forward\\s+-i\\s+incusbr0\\s+-j\\s+ACCEPT")));
            const bool hasFwdOut = rules.contains(QRegularExpression(QStringLiteral("ufw-user-forward\\s+-o\\s+incusbr0\\s+-j\\s+ACCEPT")));
            if (readOk && hasIn && hasFwdIn && hasFwdOut) {
                r.ok = true;
                r.detail = QStringLiteral("UFW allows input & routed traffic on incusbr0.");
            } else {
                r.ok = false;
                r.needsRoot = true;
                r.detail = readOk
                               ? QStringLiteral("UFW is missing allow/route rules for incusbr0.")
                               : QStringLiteral("Could not inspect /etc/ufw/user.rules.");
                r.fixCommand = QStringLiteral("sudo ufw allow in on incusbr0 && sudo ufw route allow in on incusbr0 && sudo ufw route allow out on incusbr0 && sudo ufw reload");
            }
        }
        results.append(r);
    }

    // 7. Workspace Btrfs user_subvol_rm_allowed mount option (S0/S1)
    {
        HealthCheckResult r;
        r.id = QStringLiteral("workspace_subvol_rm");
        r.title = QStringLiteral("Workspace Btrfs subvolume deletion");
        QString probePath = QDir::cleanPath(workspaceBase.trimmed().isEmpty() ? defaultWorkspacePath() : workspaceBase.trimmed());
        for (int i = 0; i < 16 && !probePath.isEmpty() && !QFileInfo::exists(probePath); ++i) {
            const QString parent = QFileInfo(probePath).dir().path();
            if (parent == probePath)
                break;
            probePath = parent;
        }
        const CommandOutput out = run(QStringLiteral("findmnt"),
                                      {QStringLiteral("-rn"), QStringLiteral("-o"),
                                       QStringLiteral("TARGET,FSTYPE,OPTIONS"),
                                       QStringLiteral("-T"), probePath});
        if (out.exitCode != 0 || out.stdOut.trimmed().isEmpty()) {
            r.ok = true;
            r.detail = QStringLiteral("Could not query mount options for %1.").arg(probePath);
        } else {
            const QStringList cols = out.stdOut.trimmed().split(QLatin1Char(' '), Qt::SkipEmptyParts);
            const QString mountTarget = cols.value(0);
            const QString fsType = cols.value(1).toLower();
            const QStringList opts = cols.value(2).split(QLatin1Char(','), Qt::SkipEmptyParts);
            if (fsType != QLatin1String("btrfs")) {
                r.ok = true;
                r.detail = QStringLiteral("Workspace mount %1 uses %2 (user_subvol_rm_allowed not applicable).")
                               .arg(mountTarget, fsType);
            } else if (opts.contains(QStringLiteral("user_subvol_rm_allowed"))) {
                r.ok = true;
                r.detail = QStringLiteral("Btrfs mount %1 enables user_subvol_rm_allowed.").arg(mountTarget);
            } else {
                r.ok = false;
                r.needsRoot = true;
                r.detail = QStringLiteral("Btrfs mount %1 lacks user_subvol_rm_allowed; btrfs subvolume delete needs sudo (add option to /etc/fstab).").arg(mountTarget);
                r.fixCommand = QStringLiteral("sudo mount -o remount,user_subvol_rm_allowed %1").arg(mountTarget);
            }
        }
        results.append(r);
    }

    // 8. Incus storage pool capacity & overcommit check (H2)
    {
        StoragePoolInfo poolInfo;
        if (daemonReachable) {
            CommandOutput stOut = run(QStringLiteral("incus"),
                                      {QStringLiteral("storage"), QStringLiteral("info"),
                                       QStringLiteral("default"), QStringLiteral("--bytes")});
            if (stOut.exitCode != 0 || stOut.stdOut.trimmed().isEmpty()) {
                stOut = run(QStringLiteral("incus"),
                            {QStringLiteral("storage"), QStringLiteral("info"), QStringLiteral("default")});
            }
            if (stOut.exitCode == 0)
                poolInfo = parseIncusStorageInfo(stOut.stdOut);
        }
        int unboundedCount = 0;
        const qint64 allocatedBytes = sumRootDiskQuotasBytes(cachedListJson, {}, poolInfo.name, &unboundedCount);
        results.append(evaluatePoolCapacityHealth(poolInfo, allocatedBytes, unboundedCount));
    }

    return results;
}

static qint64 parseStorageMetricBytes(const QString &rawVal) {
    QString clean = rawVal.trimmed();
    if ((clean.startsWith(QLatin1Char('"')) && clean.endsWith(QLatin1Char('"'))) ||
        (clean.startsWith(QLatin1Char('\'')) && clean.endsWith(QLatin1Char('\'')))) {
        clean = clean.mid(1, clean.size() - 2).trimmed();
    }
    if (clean.isEmpty())
        return 0;
    bool intOk = false;
    const qint64 exactBytes = clean.toLongLong(&intOk);
    if (intOk)
        return qMax<qint64>(0, exactBytes);
    return parseSizeBytes(clean);
}

Backend::StoragePoolInfo Backend::parseIncusStorageInfo(const QString &output) {
    StoragePoolInfo info;
    const QString trimmed = output.trimmed();
    if (trimmed.isEmpty())
        return info;

    if (trimmed.startsWith(QLatin1Char('{'))) {
        const QJsonDocument doc = QJsonDocument::fromJson(trimmed.toUtf8());
        if (doc.isObject()) {
            QJsonObject obj = doc.object();
            if (obj.contains(QStringLiteral("info")) && obj.value(QStringLiteral("info")).isObject())
                obj = obj.value(QStringLiteral("info")).toObject();
            if (!obj.value(QStringLiteral("name")).toString().isEmpty())
                info.name = obj.value(QStringLiteral("name")).toString();
            info.driver = obj.value(QStringLiteral("driver")).toString();
            const auto usedVal = obj.contains(QStringLiteral("space used"))
                                     ? obj.value(QStringLiteral("space used"))
                                     : obj.value(QStringLiteral("used"));
            const auto totalVal = obj.contains(QStringLiteral("total space"))
                                      ? obj.value(QStringLiteral("total space"))
                                      : obj.value(QStringLiteral("total"));
            info.usedBytes = usedVal.isString() ? parseStorageMetricBytes(usedVal.toString())
                                                : qMax<qint64>(0, usedVal.toVariant().toLongLong());
            info.totalBytes = totalVal.isString() ? parseStorageMetricBytes(totalVal.toString())
                                                  : qMax<qint64>(0, totalVal.toVariant().toLongLong());
            if (info.totalBytes > 0) {
                info.usedPercent = qBound(0,
                                          static_cast<int>(std::llround((static_cast<double>(info.usedBytes) * 100.0) /
                                                                        static_cast<double>(info.totalBytes))),
                                          100);
                info.valid = true;
            }
            return info;
        }
    }

    bool inUsedBy = false;
    for (const QString &rawLine : trimmed.split(QLatin1Char('\n'))) {
        if (!rawLine.startsWith(QLatin1Char(' ')) && !rawLine.startsWith(QLatin1Char('\t'))) {
            const QString topKey = rawLine.trimmed();
            if (topKey.startsWith(QLatin1String("used by:"))) {
                inUsedBy = true;
                continue;
            }
            if (topKey.startsWith(QLatin1String("info:"))) {
                inUsedBy = false;
                continue;
            }
        }
        if (inUsedBy)
            continue;
        const int colon = rawLine.indexOf(QLatin1Char(':'));
        if (colon <= 0)
            continue;
        const QString key = rawLine.left(colon).trimmed().toLower();
        const QString val = rawLine.mid(colon + 1).trimmed();
        if (key == QLatin1String("name") && !val.isEmpty())
            info.name = val;
        else if (key == QLatin1String("driver"))
            info.driver = val;
        else if (key == QLatin1String("space used"))
            info.usedBytes = parseStorageMetricBytes(val);
        else if (key == QLatin1String("total space"))
            info.totalBytes = parseStorageMetricBytes(val);
    }

    if (info.totalBytes > 0) {
        info.usedPercent = qBound(0,
                                  static_cast<int>(std::llround((static_cast<double>(info.usedBytes) * 100.0) /
                                                                static_cast<double>(info.totalBytes))),
                                  100);
        info.valid = true;
    }
    return info;
}

qint64 Backend::sumRootDiskQuotasBytes(const QByteArray &incusListJson,
                                       const QString &excludeEnvName,
                                       const QString &poolName,
                                       int *unboundedCountOut) {
    if (unboundedCountOut)
        *unboundedCountOut = 0;

    const QJsonDocument doc = QJsonDocument::fromJson(incusListJson);
    if (!doc.isArray())
        return 0;

    qint64 total = 0;
    for (const auto &val : doc.array()) {
        const QJsonObject obj = val.toObject();
        const QString name = obj.value(QStringLiteral("name")).toString();
        if (name.isEmpty() || (!excludeEnvName.isEmpty() && name == excludeEnvName))
            continue;

        const QJsonObject expanded = obj.value(QStringLiteral("expanded_devices")).toObject();
        const QJsonObject direct = obj.value(QStringLiteral("devices")).toObject();
        QJsonObject rootDev = expanded.value(QStringLiteral("root")).toObject();
        if (rootDev.isEmpty())
            rootDev = direct.value(QStringLiteral("root")).toObject();

        const QString rootPool = rootDev.value(QStringLiteral("pool")).toString();
        if (!poolName.isEmpty() && !rootPool.isEmpty() && rootPool != poolName)
            continue;

        QString sizeStr = rootDev.value(QStringLiteral("size")).toString().trimmed();
        if (sizeStr.isEmpty())
            sizeStr = direct.value(QStringLiteral("root")).toObject().value(QStringLiteral("size")).toString().trimmed();

        const qint64 quota = parseSizeBytes(sizeStr);
        if (quota > 0) {
            total += quota;
        } else if (unboundedCountOut) {
            ++(*unboundedCountOut);
        }
    }
    return total;
}

QString Backend::evaluatePoolOvercommitWarning(const StoragePoolInfo &pool,
                                             qint64 existingQuotasBytes,
                                             qint64 requestedBytes,
                                             int unboundedCount) {
    if (!pool.valid || pool.totalBytes <= 0)
        return {};

    const qint64 req = qMax<qint64>(0, requestedBytes);
    const qint64 projected = existingQuotasBytes + req;
    const bool overcommit = (projected > pool.totalBytes);
    const bool highPool = (pool.usedPercent > 85);
    if (!overcommit && !highPool)
        return {};

    QStringList parts;
    if (highPool) {
        parts << QObject::tr("Pool '%1' is %2% full (%3 / %4).")
                     .arg(pool.name)
                     .arg(pool.usedPercent)
                     .arg(formatBytes(pool.usedBytes), formatBytes(pool.totalBytes));
    }
    if (overcommit) {
        parts << QObject::tr("Total allocated quotas (%1) will exceed pool capacity (%2).")
                     .arg(formatBytes(projected), formatBytes(pool.totalBytes));
    }
    if (unboundedCount > 0) {
        parts << QObject::tr("%1 environment(s) have no disk quota.")
                     .arg(unboundedCount);
    }
    return parts.join(QLatin1Char(' '));
}

Backend::HealthCheckResult Backend::evaluatePoolCapacityHealth(const StoragePoolInfo &pool,
                                                               qint64 allocatedQuotasBytes,
                                                               int unboundedCount) {
    HealthCheckResult r;
    r.id = QStringLiteral("pool_capacity");
    r.title = QStringLiteral("Storage pool '%1' capacity").arg(pool.name);
    if (!pool.valid || pool.totalBytes <= 0) {
        r.ok = true;
        r.warning = false;
        r.detail = QStringLiteral("Could not query storage pool '%1' capacity.").arg(pool.name);
        return r;
    }

    QString nums = QStringLiteral("%1 / %2 (%3%), %4 allocated across quotas")
                       .arg(formatBytes(pool.usedBytes), formatBytes(pool.totalBytes))
                       .arg(pool.usedPercent)
                       .arg(formatBytes(allocatedQuotasBytes));
    if (unboundedCount > 0)
        nums += QStringLiteral(", %1 without disk quota").arg(unboundedCount);
    if (allocatedQuotasBytes > pool.totalBytes)
        nums += QStringLiteral(" [OVERCOMMITTED]");

    if (pool.usedPercent > 95) {
        r.ok = false;
        r.warning = false;
        r.needsRoot = false;
        r.detail = QStringLiteral("Critical pool usage (>95%): %1.").arg(nums);
        r.fixCommand = QStringLiteral("Wipe unused environments or delete old snapshots on pool '%1'.").arg(pool.name);
    } else if (pool.usedPercent > 85) {
        r.ok = true;
        r.warning = true;
        r.needsRoot = false;
        r.detail = QStringLiteral("High pool usage warning (>85%): %1.").arg(nums);
    } else {
        r.ok = true;
        r.warning = false;
        r.detail = QStringLiteral("Healthy pool usage: %1.").arg(nums);
    }
    return r;
}

QString Backend::poolWarningForCreate(const QString &diskSpec) const {
    if (!m_lastPoolInfo.valid)
        return {};
    const qint64 reqBytes = parseSizeBytes(diskSpec);
    return evaluatePoolOvercommitWarning(m_lastPoolInfo, m_lastPoolAllocatedBytes, reqBytes, m_lastPoolUnboundedCount);
}

QString Backend::poolWarningForQuotaEdit(const QString &envName, const QString &diskSpec) const {
    if (!m_lastPoolInfo.valid)
        return {};
    const qint64 newBytes = parseSizeBytes(diskSpec);
    if (!m_lastListJson.isEmpty()) {
        int unbounded = 0;
        const qint64 otherAllocated = sumRootDiskQuotasBytes(m_lastListJson, envName, m_lastPoolInfo.name, &unbounded);
        return evaluatePoolOvercommitWarning(m_lastPoolInfo, otherAllocated, newBytes, unbounded);
    }
    qint64 otherAllocated = 0;
    int unbounded = 0;
    for (const auto &v : m_environments) {
        const QVariantMap env = v.toMap();
        if (env.value(QStringLiteral("name")).toString() == envName)
            continue;
        const qint64 q = env.value(QStringLiteral("diskQuotaBytes")).toLongLong();
        if (q > 0)
            otherAllocated += q;
        else
            ++unbounded;
    }
    return evaluatePoolOvercommitWarning(m_lastPoolInfo, otherAllocated, newBytes, unbounded);
}

QString Backend::resizeStoragePoolScript(const QString &poolName, const QString &sizeSpec) {
    const QString qPool = shellQuote(poolName);
    const QString qSize = shellQuote(sizeSpec.trimmed());
    return QStringLiteral("set -euo pipefail\n"
                          "echo '@@STAGE:20:Growing storage pool %1 to %2...'\n"
                          "incus storage set %1 size=%2\n"
                          "echo '@@STAGE:80:Verifying new pool capacity...'\n"
                          "incus storage info %1 --bytes 2>/dev/null || incus storage info %1\n"
                          "echo '@@STAGE:100:Storage pool grown to %2.'")
        .arg(qPool, qSize);
}

bool Backend::parsePoolSizeSpec(const QString &spec, qint64 *outBytes) {
    if (outBytes)
        *outBytes = 0;
    static const QRegularExpression re(
        QStringLiteral("^\\s*(\\d+(?:[\\.,]\\d+)?)\\s*(TiB|TB|GiB|GB|MiB|MB)?\\s*$"),
        QRegularExpression::CaseInsensitiveOption);
    const QRegularExpressionMatch match = re.match(spec);
    if (!match.hasMatch())
        return false;

    QString number = match.captured(1);
    number.replace(QLatin1Char(','), QLatin1Char('.'));
    bool ok = false;
    const double value = number.toDouble(&ok);
    if (!ok || value <= 0.0)
        return false;

    // Incus treats bare storage sizes as GiB, so "60" means 60GiB here too.
    const QString unit = match.captured(2).toUpper();
    double scale = 1024.0 * 1024.0 * 1024.0;
    if (unit == QLatin1String("TIB") || unit == QLatin1String("TB"))
        scale = 1024.0 * 1024.0 * 1024.0 * 1024.0;
    else if (unit == QLatin1String("MIB") || unit == QLatin1String("MB"))
        scale = 1024.0 * 1024.0;

    const double bytes = value * scale;
    if (bytes < 1.0 || bytes >= 9.0e18)
        return false;
    if (outBytes)
        *outBytes = static_cast<qint64>(std::llround(bytes));
    return true;
}

QString Backend::validateStoragePoolResize(const QString &sizeSpec) const {
    if (m_busy)
        return tr("Wait for the current AIBox task to finish before growing the storage pool.");
    if (isTaskLockActive())
        return tr("Another AIBox task is currently running.");
    if (!m_lastPoolInfo.valid || m_lastPoolInfo.totalBytes <= 0)
        return tr("The Incus storage pool is not available yet. Refresh and try again.");
    if (!m_lastPoolInfo.resizable)
        return tr("Pool '%1' is not loop-file backed, so its capacity follows the host filesystem and cannot be grown from AIBox.")
            .arg(m_lastPoolInfo.name);

    qint64 targetBytes = 0;
    if (!parsePoolSizeSpec(sizeSpec, &targetBytes))
        return tr("Enter a pool size such as 60GiB (plain numbers are treated as GiB).");

    if (targetBytes <= m_lastPoolInfo.totalBytes)
        return tr("The new size (%1) must be larger than the current pool size (%2); Incus cannot shrink a pool.")
            .arg(formatBytes(targetBytes), formatBytes(m_lastPoolInfo.totalBytes));

    const qint64 delta = targetBytes - m_lastPoolInfo.totalBytes;
    const qint64 available = availableDiskBytes(QStringLiteral("/var/lib/incus"));
    if (available > 0 && delta > available) {
        return tr("Growing the pool needs %1 more space, but only %2 is free on the host filesystem.")
            .arg(formatBytes(delta), formatBytes(available));
    }
    return {};
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
    IdleStopOverride ov;
    const QString norm = raw.trimmed().toLower();
    if (norm.isEmpty() || norm == QLatin1String("inherit") || norm == QLatin1String("default")) {
        ov.mode = IdleStopOverride::Inherit;
        ov.minutes = 0;
        return ov;
    }
    if (norm == QLatin1String("off") || norm == QLatin1String("false") || norm == QLatin1String("0")) {
        ov.mode = IdleStopOverride::Off;
        ov.minutes = 0;
        return ov;
    }
    bool ok = false;
    const int mins = norm.toInt(&ok);
    if (ok && mins > 0 && mins <= 1440) {
        ov.mode = IdleStopOverride::Minutes;
        ov.minutes = mins;
        return ov;
    }
    ov.mode = IdleStopOverride::Inherit;
    ov.minutes = 0;
    return ov;
}

int Backend::effectiveIdleStopMinutes(int globalMinutes, const IdleStopOverride &ov) {
    if (ov.mode == IdleStopOverride::Off)
        return 0;
    if (ov.mode == IdleStopOverride::Minutes)
        return qBound(1, ov.minutes, 1440);
    return (globalMinutes > 0) ? qBound(1, globalMinutes, 1440) : 0;
}

Backend::IdleDecisionResult Backend::evaluateIdleDecision(const IdleDecisionInput &in) {
    IdleDecisionResult res;
    if (!in.running) {
        res.blockedReason = QStringLiteral("stopped");
        return res;
    }
    if (in.isTemplate) {
        res.blockedReason = QStringLiteral("template");
        return res;
    }
    if (in.thresholdMinutes <= 0) {
        res.blockedReason = QStringLiteral("disabled");
        return res;
    }
    if (in.busyOnThisEnv) {
        res.blockedReason = QStringLiteral("busy");
        return res;
    }
    if (in.hasActiveExecSession) {
        res.blockedReason = QStringLiteral("exec-session");
        return res;
    }
    if (in.portForwardCount > 0 && !in.ignorePortForwards) {
        res.blockedReason = QStringLiteral("port-forwards");
        return res;
    }

    res.eligible = true;
    if (in.samples.isEmpty())
        return res;

    // CPU spikes (>= 3%) reset the idle timer; inspect contiguous calm samples since last spike
    int startIdx = 0;
    for (int i = in.samples.size() - 1; i >= 0; --i) {
        if (in.samples.at(i).cpuPercent >= 3) {
            startIdx = i + 1;
            break;
        }
    }
    if (startIdx >= in.samples.size()) {
        res.blockedReason = QStringLiteral("cpu-spike");
        return res;
    }

    double sumCpu = 0.0;
    const int calmCount = in.samples.size() - startIdx;
    for (int i = startIdx; i < in.samples.size(); ++i)
        sumCpu += static_cast<double>(qMax(0, in.samples.at(i).cpuPercent));
    res.averageCpuPercent = sumCpu / static_cast<double>(calmCount);

    const qint64 firstCalmMs = in.samples.at(startIdx).timestampMs;
    const qint64 refNowMs = (in.nowMs > 0) ? in.nowMs : in.samples.last().timestampMs;
    res.idleDurationMs = qMax<qint64>(0, refNowMs - firstCalmMs);

    if (res.averageCpuPercent >= 3.0) {
        res.idleDurationMs = 0;
        res.blockedReason = QStringLiteral("cpu-high");
        return res;
    }

    const qint64 windowMs = qint64(in.thresholdMinutes) * 60LL * 1000LL;
    const qint64 warnLeadMs = (in.thresholdMinutes >= 2) ? 60000LL : (windowMs / 2);
    if (res.idleDurationMs >= windowMs) {
        res.shouldStopNow = true;
    } else if (!in.warningAlreadySent && res.idleDurationMs >= (windowMs - warnLeadMs)) {
        res.shouldWarnOneMin = true;
    }
    return res;
}
