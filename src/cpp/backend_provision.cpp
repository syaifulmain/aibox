#include "backend.h"
#include "backend_helpers.h"
#include "rust_bridge.h"

#include "aibox-ffi/src/provision.rs.h"

#include <QDir>

using aibox::ffi::fromRust;
using aibox::ffi::toRust;

void Backend::downloadOsImage(const QString &osId) {
    if (osId.isEmpty() || m_busy)
        return;
    const QVariantMap entry = osEntryById(osId);
    if (entry.isEmpty()) {
        setError(tr("The OS entry '%1' is not in the catalog.").arg(osId));
        return;
    }

    QDir().mkpath(m_osStoragePath);
    const QString alias = entry.value("alias").toString();
    const QString cacheBase = cacheBasePath(osId);
    m_downloadingOsId = osId;
    rebuildOsCatalog();

    const auto plan = aibox::ffi::download_os_image_script(toRust(osId), toRust(alias), toRust(cacheBase));
    runShellTask(fromRust(plan.title), fromRust(plan.script));
}

void Backend::removeOsImage(const QString &osId) {
    if (osId.isEmpty() || m_busy)
        return;
    const QString cacheBase = cacheBasePath(osId);
    const auto plan = aibox::ffi::remove_os_image_script(toRust(osId), toRust(cacheBase));
    runShellTask(fromRust(plan.title), fromRust(plan.script));
}

qint64 Backend::parseByteSizeToBytes(const QString &text) {
    return parseSizeBytes(text);
}

QString Backend::updateResourcesScript(const QString &name, int cpuCores, const QString &ramGiB,
                                       const QString &diskGiB, const QString &processLimit,
                                       const QString &ioLimit, bool isVm) {
    return fromRust(aibox::ffi::update_resources_script(
        toRust(name), cpuCores, toRust(ramGiB), toRust(diskGiB),
        toRust(processLimit), toRust(ioLimit), isVm));
}

QString Backend::updateEnvironmentResources(const QString &name, int cpuCores, const QString &ramGiB,
                                            const QString &diskGiB, const QString &processLimit,
                                            const QString &ioLimit) {
    const QVariantMap env = environmentByName(name);
    const bool envExists = !env.isEmpty();
    const qint64 usedBytes = env.value("diskUsageBytes").toLongLong();
    const QString diskUsageLabel = env.value("diskUsage").toString();
    const QString envType = env.value("type").toString().toUpper();
    const bool isVm = (envType == QLatin1String("VIRTUAL-MACHINE") || envType == QLatin1String("VM"));

    const auto plan = aibox::ffi::validate_and_prepare_update_resources(
        toRust(name), m_busy, cpuCores, toRust(ramGiB), toRust(diskGiB),
        toRust(processLimit), toRust(ioLimit), envExists, usedBytes,
        toRust(diskUsageLabel), isVm);
    if (!plan.ok)
        return fromRust(plan.error);

    runShellTask(fromRust(plan.title), fromRust(plan.script), {}, name);
    return {};
}
