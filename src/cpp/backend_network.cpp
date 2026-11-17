#include "backend.h"
#include "backend_helpers.h"
#include "host_resolver.h"
#include "rust_bridge.h"
#include "aibox-ffi/src/network.rs.h"

#include <QDesktopServices>
#include <QUrl>

QStringList Backend::parseAllowlistEntries(const QString &rawText) {
    return aibox::ffi::fromRustVec(aibox::ffi::parse_allowlist_entries(aibox::ffi::toRust(rawText)));
}

QStringList Backend::resolveAllowlistCidrs(const QStringList &entries, const HostResolver &resolver,
                                           QStringList *warnings) {
    const rust::Vec<rust::String> entriesVec = aibox::ffi::toRustVec(entries);
    std::function<rust::Vec<rust::String>(rust::Str)> bridgedFn;
    if (resolver) {
        bridgedFn = [&resolver](rust::Str host) {
            return aibox::ffi::toRustVec(resolver(aibox::ffi::fromRust(host)));
        };
    }
    const aibox::ffi::HostResolverBridge bridge{resolver ? &bridgedFn : nullptr};
    const aibox::ffi::ResolveAllowlistResultFfi res = aibox::ffi::resolve_allowlist_cidrs(
        rust::Slice<const rust::String>(entriesVec.data(), entriesVec.size()),
        static_cast<bool>(resolver),
        bridge);
    if (warnings) {
        for (const rust::String &w : res.warnings)
            warnings->append(aibox::ffi::fromRust(w));
    }
    return aibox::ffi::fromRustVec(res.cidrs);
}

QString Backend::networkControlScript(const QString &name, const QString &mode,
                                      const QString &allowlistRaw, const QStringList &resolvedCidrs,
                                      bool egressLogging) {
    const rust::Vec<rust::String> cidrsVec = aibox::ffi::toRustVec(resolvedCidrs);
    return aibox::ffi::fromRust(aibox::ffi::network_control_script(
        aibox::ffi::toRust(name),
        aibox::ffi::toRust(mode),
        aibox::ffi::toRust(allowlistRaw),
        rust::Slice<const rust::String>(cidrsVec.data(), cidrsVec.size()),
        egressLogging));
}

QString Backend::buildAllowlistRefreshIfNeeded(const QString &name) const {
    const QVariantMap env = environmentByName(name);
    return aibox::ffi::fromRust(aibox::ffi::build_allowlist_refresh_if_needed(
        aibox::ffi::toRust(name),
        aibox::ffi::toRust(env.value("networkMode").toString()),
        aibox::ffi::toRust(env.value("networkAllowlist").toString()),
        env.value("egressRecording").toBool()));
}

void Backend::setNetworkMode(const QString &name, const QString &mode, const QString &allowlist) {
    if (name.isEmpty() || m_busy)
        return;
    const QVariantMap env = environmentByName(name);
    const aibox::ffi::NetworkTaskPlanFfi plan = aibox::ffi::prepare_set_network_mode(
        aibox::ffi::toRust(name),
        aibox::ffi::toRust(mode),
        aibox::ffi::toRust(allowlist),
        env.value("egressRecording").toBool());
    if (!plan.error.empty()) {
        setError(aibox::ffi::fromRust(plan.error));
        return;
    }
    runShellTask(aibox::ffi::fromRust(plan.description), aibox::ffi::fromRust(plan.script), {}, name);
}

void Backend::refreshNetworkAllowlist(const QString &name) {
    const QVariantMap env = environmentByName(name);
    if (env.isEmpty())
        return;
    setNetworkMode(name, QStringLiteral("allowlist"), env.value("networkAllowlist").toString());
}

void Backend::setEgressRecording(const QString &name, bool enabled) {
    if (name.isEmpty() || m_busy)
        return;
    const QVariantMap env = environmentByName(name);
    if (env.isEmpty())
        return;

    const aibox::ffi::NetworkTaskPlanFfi plan = aibox::ffi::prepare_set_egress_recording(
        aibox::ffi::toRust(name),
        aibox::ffi::toRust(env.value("networkMode").toString()),
        aibox::ffi::toRust(env.value("networkAllowlist").toString()),
        enabled);
    if (!plan.error.empty()) {
        setError(aibox::ffi::fromRust(plan.error));
        return;
    }
    runShellTask(aibox::ffi::fromRust(plan.description), aibox::ffi::fromRust(plan.script), {}, name);
}

QVariantList Backend::parseKernelEgressLogs(const QString &journalOutput, const QString &containerIp,
                                            const QString &hostIface, int maxEntries) {
    const rust::String json = aibox::ffi::parse_kernel_egress_logs_json(
        aibox::ffi::toRust(journalOutput),
        aibox::ffi::toRust(containerIp),
        aibox::ffi::toRust(hostIface),
        maxEntries);
    QVariantList out = aibox::ffi::fromRustJson(json).toList();
    for (QVariant &v : out) {
        QVariantMap row = v.toMap();
        row.insert(QStringLiteral("count"), row.value(QStringLiteral("count")).toInt());
        v = row;
    }
    return out;
}

void Backend::refreshEgressLog(const QString &name) {
    if (name.isEmpty())
        return;
    const QVariantMap env = environmentByName(name);
    if (env.isEmpty())
        return;

    m_egressLogsEnvName = name;
    if (m_demoMode) {
        m_egressLogs = {
            QVariantMap{{"dstIp", QStringLiteral("140.82.121.4")}, {"proto", QStringLiteral("TCP")}, {"port", QStringLiteral("443")}, {"count", 18}, {"lastSeen", QStringLiteral("13:18:42")}},
            QVariantMap{{"dstIp", QStringLiteral("160.79.104.10")}, {"proto", QStringLiteral("TCP")}, {"port", QStringLiteral("443")}, {"count", 9}, {"lastSeen", QStringLiteral("13:17:15")}},
            QVariantMap{{"dstIp", QStringLiteral("104.16.25.34")}, {"proto", QStringLiteral("TCP")}, {"port", QStringLiteral("443")}, {"count", 4}, {"lastSeen", QStringLiteral("13:15:09")}}
        };
        emit egressLogsChanged();
        return;
    }

    const QString containerIp = env.value("ipv4").toString();
    const QString hostIface = env.value("hostInterface").toString();

    auto *proc = new QProcess(this);
    connect(proc, &QProcess::finished, this, [this, proc, name, containerIp, hostIface](int, QProcess::ExitStatus) {
        const QString out = QString::fromUtf8(proc->readAllStandardOutput());
        proc->deleteLater();
        if (m_egressLogsEnvName != name)
            return;
        m_egressLogs = parseKernelEgressLogs(out, containerIp, hostIface, 100);
        emit egressLogsChanged();
    });
    proc->start(QStringLiteral("journalctl"),
                {QStringLiteral("-k"), QStringLiteral("-n"), QStringLiteral("4000"),
                 QStringLiteral("--no-pager"), QStringLiteral("--output=short-iso")});
}

void Backend::addEgressDestinationToAllowlist(const QString &name, const QString &destinationIp) {
    if (name.isEmpty() || m_busy)
        return;
    const QVariantMap env = environmentByName(name);
    const QString updated = aibox::ffi::fromRust(aibox::ffi::add_egress_destination_allowlist(
        aibox::ffi::toRust(env.value("networkAllowlist").toString()),
        aibox::ffi::toRust(destinationIp)));
    if (updated.isEmpty())
        return;
    setNetworkMode(name, QStringLiteral("allowlist"), updated);
}

QString Backend::proxyDeviceName(int hostPort) {
    return aibox::ffi::fromRust(aibox::ffi::proxy_device_name(hostPort));
}

bool Backend::isHostPortAvailable(int hostPort) {
    return aibox::ffi::is_host_port_available(hostPort);
}

QString Backend::addPortForwardScript(const QString &name, int hostPort, int guestPort) {
    return aibox::ffi::fromRust(aibox::ffi::add_port_forward_script(
        aibox::ffi::toRust(name), hostPort, guestPort));
}

QString Backend::addPortForward(const QString &name, int hostPort, int guestPort) {
    const QVariantMap env = name.isEmpty() ? QVariantMap() : environmentByName(name);
    rust::Vec<std::int32_t> existingPorts;
    for (const auto &v : env.value("portForwards").toList())
        existingPorts.push_back(v.toMap().value("hostPort").toInt());

    const QString err = aibox::ffi::fromRust(aibox::ffi::validate_add_port_forward(
        aibox::ffi::toRust(name),
        m_busy,
        hostPort,
        guestPort,
        !env.isEmpty(),
        rust::Slice<const std::int32_t>(existingPorts.data(), existingPorts.size())));
    if (!err.isEmpty())
        return err;

    runShellTask(tr("Forwarding 127.0.0.1:%1 -> %2:%3").arg(hostPort).arg(name).arg(guestPort),
                 addPortForwardScript(name, hostPort, guestPort), {}, name);
    return {};
}

void Backend::removePortForward(const QString &name, const QString &deviceName) {
    if (name.isEmpty() || deviceName.isEmpty() || m_busy)
        return;
    runShellTask(tr("Removing port forward '%1' on '%2'").arg(deviceName, name),
                 aibox::ffi::fromRust(aibox::ffi::remove_port_forward_script(
                     aibox::ffi::toRust(name), aibox::ffi::toRust(deviceName))),
                 {}, name);
}

void Backend::openForwardedPort(int hostPort) {
    const QString url = aibox::ffi::fromRust(aibox::ffi::open_forwarded_port_url(hostPort));
    if (!url.isEmpty() && isGuiSession())
        QDesktopServices::openUrl(QUrl(url));
}
