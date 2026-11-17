#include "backend.h"
#include "backend_helpers.h"

#include <QDir>
#include <QRegularExpression>

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
    const QString localAlias = QStringLiteral("aibox-") + osId;
    const QString cacheBase = cacheBasePath(osId);
    m_downloadingOsId = osId;
    rebuildOsCatalog();

    const QString script = QStringLiteral(R"sh(
set -euo pipefail
LOCAL_ALIAS=%1
REMOTE_IMAGE=%2
CACHE=%3
CACHE_ROOT=%4

mkdir -p "$(dirname "$CACHE")"
if incus image info "$LOCAL_ALIAS" >/dev/null 2>&1; then
    echo "@@STAGE:45:OS image is already in the local Incus store..."
elif [ -s "$CACHE" ] && [ -s "$CACHE_ROOT" ]; then
    echo "@@STAGE:35:Importing OS image from OS storage..."
    incus image import "$CACHE" "$CACHE_ROOT" --alias "$LOCAL_ALIAS"
else
    echo "@@STAGE:5:Downloading %5 from the official image server..."
    script -q -e -c "incus image copy \"$REMOTE_IMAGE\" local: --alias \"$LOCAL_ALIAS\" --auto-update" /dev/null
fi

if [ ! -s "$CACHE" ] || [ ! -s "$CACHE_ROOT" ]; then
    echo "@@STAGE:85:Saving OS image to OS storage..."
    rm -f "$CACHE" "$CACHE_ROOT"
    incus image export "$LOCAL_ALIAS" "$CACHE" >/dev/null
fi
echo "@@STAGE:100:Download complete!"
)sh")
                           .arg(shellQuote(localAlias),
                                shellQuote(QStringLiteral("images:") + alias),
                                shellQuote(cacheBase),
                                shellQuote(cacheBase + ".root"),
                                osId);
    runShellTask(tr("Downloading OS image '%1'").arg(osId), script);
}

void Backend::removeOsImage(const QString &osId) {
    if (osId.isEmpty() || m_busy)
        return;
    const QString localAlias = QStringLiteral("aibox-") + osId;
    const QString cacheBase = cacheBasePath(osId);
    runShellTask(tr("Removing cached OS image '%1'").arg(osId),
                 QStringLiteral("set -euo pipefail\n"
                                "echo '@@STAGE:50:Removing image cache...'\n"
                                "if incus image info %1 >/dev/null 2>&1; then incus image delete %1; fi\n"
                                "rm -f %2 %3\n"
                                "echo '@@STAGE:100:Removed.'")
                     .arg(shellQuote(localAlias), shellQuote(cacheBase), shellQuote(cacheBase + ".root")));
}

qint64 Backend::parseByteSizeToBytes(const QString &text) {
    return parseSizeBytes(text);
}

QString Backend::updateResourcesScript(const QString &name, int cpuCores, const QString &ramGiB,
                                       const QString &diskGiB, const QString &processLimit,
                                       const QString &ioLimit, bool isVm) {
    const QString qName = shellQuote(name);
    QStringList lines;
    lines << QStringLiteral("set -euo pipefail");
    lines << QStringLiteral("echo '@@STAGE:25:Updating CPU and memory quotas...'");
    if (cpuCores > 0)
        lines << QStringLiteral("incus config set %1 limits.cpu %2").arg(qName, shellQuote(QString::number(cpuCores)));
    if (!ramGiB.trimmed().isEmpty())
        lines << QStringLiteral("incus config set %1 limits.memory %2").arg(qName, shellQuote(ramGiB.trimmed()));

    if (!isVm) {
        const QString cleanProc = processLimit.trimmed();
        if (cleanProc.isEmpty() || cleanProc.compare(QLatin1String("unlimited"), Qt::CaseInsensitive) == 0)
            lines << QStringLiteral("incus config unset %1 limits.processes >/dev/null 2>&1 || true").arg(qName);
        else
            lines << QStringLiteral("incus config set %1 limits.processes %2").arg(qName, shellQuote(cleanProc));
    }

    lines << QStringLiteral("echo '@@STAGE:65:Updating root disk quota and I/O limits...'");
    if (!diskGiB.trimmed().isEmpty()) {
        lines << QStringLiteral("incus config device set %1 root size=%2 >/dev/null 2>&1 || incus config device override %1 root size=%2")
                     .arg(qName, shellQuote(diskGiB.trimmed()));
    }

    const QString cleanIo = ioLimit.trimmed();
    if (cleanIo.isEmpty() || cleanIo.compare(QLatin1String("unlimited"), Qt::CaseInsensitive) == 0) {
        lines << QStringLiteral("incus config device unset %1 root limits.read >/dev/null 2>&1 || true").arg(qName);
        lines << QStringLiteral("incus config device unset %1 root limits.write >/dev/null 2>&1 || true").arg(qName);
    } else {
        lines << QStringLiteral("incus config device set %1 root limits.read=%2 limits.write=%2 >/dev/null 2>&1 || incus config device override %1 root limits.read=%2 limits.write=%2")
                     .arg(qName, shellQuote(cleanIo));
    }

    lines << QStringLiteral("echo '@@STAGE:100:Quotas updated.'");
    return lines.join(QLatin1Char('\n'));
}

QString Backend::updateEnvironmentResources(const QString &name, int cpuCores, const QString &ramGiB,
                                            const QString &diskGiB, const QString &processLimit,
                                            const QString &ioLimit) {
    if (name.isEmpty())
        return tr("Select an environment first.");
    if (m_busy)
        return tr("Another operation is currently in progress.");
    if (cpuCores < 1 || cpuCores > 1024)
        return tr("CPU cores must be between 1 and 1024.");

    static const QRegularExpression sizeRe(QStringLiteral("^\\d+(?:\\.\\d+)?(?:GiB|MiB|TiB|GB|MB|TB)$"),
                                           QRegularExpression::CaseInsensitiveOption);
    if (!sizeRe.match(ramGiB.trimmed()).hasMatch())
        return tr("The RAM limit is invalid. Use a value such as 8GiB.");
    if (!sizeRe.match(diskGiB.trimmed()).hasMatch())
        return tr("The disk quota is invalid. Use a value such as 50GiB.");

    const QString cleanProc = processLimit.trimmed().compare(QLatin1String("unlimited"), Qt::CaseInsensitive) == 0
                                  ? QString()
                                  : processLimit.trimmed();
    const QString cleanIo = ioLimit.trimmed().compare(QLatin1String("unlimited"), Qt::CaseInsensitive) == 0
                                ? QString()
                                : ioLimit.trimmed();
    if (!isValidProcessLimit(cleanProc))
        return tr("The process limit is invalid. Use an integer >= 10 (e.g. 4096) or Unlimited.");
    if (!isValidIoLimit(cleanIo))
        return tr("The disk I/O limit is invalid. Use a rate such as 200MB, 2500iops, or Unlimited.");

    const QVariantMap env = environmentByName(name);
    if (env.isEmpty())
        return tr("Environment '%1' was not found.").arg(name);

    const qint64 usedBytes = env.value("diskUsageBytes").toLongLong();
    const qint64 targetBytes = parseByteSizeToBytes(diskGiB.trimmed());
    if (usedBytes > 0 && targetBytes > 0 && targetBytes < usedBytes) {
        return tr("Cannot shrink disk quota (%1) below current usage (%2).")
            .arg(diskGiB.trimmed(), env.value("diskUsage").toString());
    }

    const QString envType = env.value("type").toString().toUpper();
    const bool isVm = (envType == QLatin1String("VIRTUAL-MACHINE") || envType == QLatin1String("VM"));
    runShellTask(tr("Updating resource quotas for '%1'").arg(name),
                 updateResourcesScript(name, cpuCores, ramGiB, diskGiB, cleanProc, cleanIo, isVm),
                 {}, name);
    return {};
}
