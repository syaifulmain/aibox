#include <QDateTime>
#include <QGuiApplication>
#include <QDir>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLocale>
#include <QSaveFile>
#include <QStringList>
#include <QTextStream>

#include "backend.h"
#include "backend_helpers.h"
#include "theme.h"
#include "translator.h"

namespace {

QByteArray jsonLine(const QString &fnName, const QString &caseLabel,
                    const QJsonObject &args, const QJsonValue &out) {
    QJsonObject obj;
    obj.insert(QStringLiteral("fn"), fnName);
    obj.insert(QStringLiteral("case"), caseLabel);
    obj.insert(QStringLiteral("args"), args);
    obj.insert(QStringLiteral("out"), out);
    return QJsonDocument(obj).toJson(QJsonDocument::Compact) + '\n';
}

bool writeModuleFile(const QString &outDir, const QString &module, const QByteArray &data) {
    QDir().mkpath(outDir);
    QSaveFile file(outDir + QLatin1Char('/') + module + QStringLiteral(".jsonl"));
    if (!file.open(QIODevice::WriteOnly | QIODevice::Truncate))
        return false;
    file.write(data);
    return file.commit();
}

}  // namespace

struct GoldenCapture {
static QByteArray captureUtil() {
    QByteArray out;

    const QList<QPair<QString, qint64>> formatBytesCases = {
        {QStringLiteral("zero"), 0},
        {QStringLiteral("negative_1"), -1},
        {QStringLiteral("negative_100"), -100},
        {QStringLiteral("bytes_1"), 1},
        {QStringLiteral("bytes_512"), 512},
        {QStringLiteral("bytes_1023"), 1023},
        {QStringLiteral("kib_1"), 1024},
        {QStringLiteral("kib_1_5"), 1536},
        {QStringLiteral("below_mib_1"), 1024LL * 1024LL - 1},
        {QStringLiteral("mib_1"), 1024LL * 1024LL},
        {QStringLiteral("mib_42"), 42LL * 1024LL * 1024LL},
        {QStringLiteral("below_gib_1"), 1024LL * 1024LL * 1024LL - 1},
        {QStringLiteral("gib_1"), 1024LL * 1024LL * 1024LL},
        {QStringLiteral("gib_1_05"), 1127428915LL},
        {QStringLiteral("gib_1_15"), 1234803098LL},
        {QStringLiteral("gib_1_25"), 1342177280LL},
        {QStringLiteral("gib_1_5"), 1536LL * 1024LL * 1024LL},
        {QStringLiteral("tib_1"), 1024LL * 1024LL * 1024LL * 1024LL},
    };
    for (const auto &c : formatBytesCases) {
        QJsonObject args;
        args.insert(QStringLiteral("bytes"), static_cast<double>(c.second));
        out += jsonLine(QStringLiteral("formatBytes"), c.first, args, formatBytes(c.second));
    }

    const QList<QPair<QString, qint64>> formatByteRateCases = {
        {QStringLiteral("zero"), 0},
        {QStringLiteral("negative"), -10},
        {QStringLiteral("bytes_1"), 1},
        {QStringLiteral("bytes_512"), 512},
        {QStringLiteral("bytes_1023"), 1023},
        {QStringLiteral("kib_1"), 1024},
        {QStringLiteral("kib_1_5"), 1536},
        {QStringLiteral("below_mib_1"), 1024LL * 1024LL - 1},
        {QStringLiteral("mib_1"), 1024LL * 1024LL},
        {QStringLiteral("below_gib_1"), 1024LL * 1024LL * 1024LL - 1},
        {QStringLiteral("gib_1"), 1024LL * 1024LL * 1024LL},
        {QStringLiteral("gib_2_5"), 2684354560LL},
    };
    for (const auto &c : formatByteRateCases) {
        QJsonObject args;
        args.insert(QStringLiteral("bytesPerSecond"), static_cast<double>(c.second));
        out += jsonLine(QStringLiteral("formatByteRate"), c.first, args, formatByteRate(c.second));
    }

    const QList<QPair<QString, qint64>> formatByteTotalCases = {
        {QStringLiteral("zero"), 0},
        {QStringLiteral("negative"), -5},
        {QStringLiteral("bytes_1"), 1},
        {QStringLiteral("bytes_512"), 512},
        {QStringLiteral("bytes_1023"), 1023},
        {QStringLiteral("kib_1"), 1024},
        {QStringLiteral("kib_1_5"), 1536},
        {QStringLiteral("below_mib_1"), 1024LL * 1024LL - 1},
        {QStringLiteral("mib_1"), 1024LL * 1024LL},
        {QStringLiteral("below_gib_1"), 1024LL * 1024LL * 1024LL - 1},
        {QStringLiteral("gib_1"), 1024LL * 1024LL * 1024LL},
        {QStringLiteral("below_tib_1"), 1024LL * 1024LL * 1024LL * 1024LL - 1},
        {QStringLiteral("tib_1"), 1024LL * 1024LL * 1024LL * 1024LL},
        {QStringLiteral("tib_2_5"), 2748779069440LL},
    };
    for (const auto &c : formatByteTotalCases) {
        QJsonObject args;
        args.insert(QStringLiteral("bytes"), static_cast<double>(c.second));
        out += jsonLine(QStringLiteral("formatByteTotal"), c.first, args, formatByteTotal(c.second));
    }

    const QList<QPair<QString, QString>> parseSizeCases = {
        {QStringLiteral("empty"), QStringLiteral("")},
        {QStringLiteral("whitespace"), QStringLiteral("   ")},
        {QStringLiteral("zero"), QStringLiteral("0")},
        {QStringLiteral("negative"), QStringLiteral("-5GiB")},
        {QStringLiteral("no_unit_mib"), QStringLiteral("100")},
        {QStringLiteral("decimal_no_unit"), QStringLiteral("1.5")},
        {QStringLiteral("spaced_gib"), QStringLiteral("  1.5 GiB  ")},
        {QStringLiteral("lowercase_gib"), QStringLiteral("1.5gib")},
        {QStringLiteral("bytes_b"), QStringLiteral("1024B")},
        {QStringLiteral("kib"), QStringLiteral("512 KiB")},
        {QStringLiteral("kb_lower"), QStringLiteral("256kb")},
        {QStringLiteral("mib"), QStringLiteral("42 MiB")},
        {QStringLiteral("mb_lower"), QStringLiteral("42mb")},
        {QStringLiteral("tib"), QStringLiteral("1.25 TiB")},
        {QStringLiteral("tb_lower"), QStringLiteral("2tb")},
        {QStringLiteral("garbage"), QStringLiteral("garbage")},
        {QStringLiteral("double_dot"), QStringLiteral("12.34.56 GB")},
        {QStringLiteral("half_byte"), QStringLiteral("0.5 B")},
    };
    for (const auto &c : parseSizeCases) {
        QJsonObject args;
        args.insert(QStringLiteral("sizeText"), c.second);
        out += jsonLine(QStringLiteral("parseSizeBytes"), c.first, args,
                        static_cast<double>(parseSizeBytes(c.second)));
    }

    const QList<QPair<QString, QStringList>> sizeOptionsCases = {
        {QStringLiteral("empty"), {}},
        {QStringLiteral("mixed_valid_invalid_dups"),
         {QStringLiteral("10GiB"), QStringLiteral(" 20gb "), QStringLiteral("10gib"),
          QStringLiteral("512MiB"), QStringLiteral("1.5TiB"), QStringLiteral("100MB"),
          QStringLiteral("2TB"), QStringLiteral("512KiB"), QStringLiteral("100B"),
          QStringLiteral("100"), QStringLiteral("abc"), QStringLiteral("-5GiB"),
          QStringLiteral("")}},
    };
    for (const auto &c : sizeOptionsCases) {
        QJsonObject args;
        args.insert(QStringLiteral("values"), QJsonArray::fromStringList(c.second));
        out += jsonLine(QStringLiteral("validatedSizeOptions"), c.first, args,
                        QJsonArray::fromStringList(validatedSizeOptions(c.second)));
    }

    const QList<QPair<QString, QVariantList>> cpuOptionsCases = {
        {QStringLiteral("empty"), {}},
        {QStringLiteral("mixed"),
         {1, 0, -1, 1024, 1025, QStringLiteral("2"), QStringLiteral(" 8 "),
          QStringLiteral("abc"), 4, QStringLiteral("4"), true, QVariant()}},
    };
    for (const auto &c : cpuOptionsCases) {
        QJsonObject args;
        args.insert(QStringLiteral("values"), QJsonArray::fromVariantList(c.second));
        out += jsonLine(QStringLiteral("validatedCpuOptions"), c.first, args,
                        QJsonArray::fromVariantList(validatedCpuOptions(c.second)));
    }

    const QList<QPair<QString, QString>> sanitizeCases = {
        {QStringLiteral("empty"), QStringLiteral("")},
        {QStringLiteral("simple"), QStringLiteral("my-env")},
        {QStringLiteral("spaces_and_caps"), QStringLiteral("  My Cool Env!  ")},
        {QStringLiteral("leading_trailing_dashes"), QStringLiteral("---foo--bar---")},
        {QStringLiteral("only_symbols"), QStringLiteral("@@@")},
        {QStringLiteral("unicode_dots_digits"), QStringLiteral("123.Café_Env.v1")},
        {QStringLiteral("qt_test_1"), QStringLiteral("  Project POS AI 2026! ")},
        {QStringLiteral("qt_test_2"), QStringLiteral("---Mobile__POS---")},
    };
    for (const auto &c : sanitizeCases) {
        QJsonObject args;
        args.insert(QStringLiteral("rawName"), c.second);
        out += jsonLine(QStringLiteral("Backend::sanitizeName"), c.first, args,
                        Backend::sanitizeName(c.second));
    }

    const QList<QPair<QString, QString>> quoteCases = {
        {QStringLiteral("empty"), QStringLiteral("")},
        {QStringLiteral("plain"), QStringLiteral("hello")},
        {QStringLiteral("spaces"), QStringLiteral("hello world")},
        {QStringLiteral("single_quotes"), QStringLiteral("it's 'fine'")},
        {QStringLiteral("double_quotes_dollar_newline"),
         QStringLiteral("a \"b\" $HOME $(id)\nc\\d")},
        {QStringLiteral("unicode"), QStringLiteral("café")},
    };
    for (const auto &c : quoteCases) {
        QJsonObject args;
        args.insert(QStringLiteral("value"), c.second);
        out += jsonLine(QStringLiteral("Backend::shellQuote"), c.first, args,
                        Backend::shellQuote(c.second));
    }

    const QList<QPair<QString, QString>> arrayWordsCases = {
        {QStringLiteral("empty"), QStringLiteral("")},
        {QStringLiteral("whitespace"), QStringLiteral("   \t\n  ")},
        {QStringLiteral("single"), QStringLiteral("alpha")},
        {QStringLiteral("multiple_with_quotes"),
         QStringLiteral("  --flag  it's  $VAR\tsecond\nthird  ")},
    };
    for (const auto &c : arrayWordsCases) {
        QJsonObject args;
        args.insert(QStringLiteral("words"), c.second);
        out += jsonLine(QStringLiteral("Backend::shellArrayWords"), c.first, args,
                        Backend::shellArrayWords(c.second));
    }

    const QList<QPair<QString, QString>> parseCsvCases = {
        {QStringLiteral("empty"), QStringLiteral("")},
        {QStringLiteral("mixed_separators_dups"),
         QStringLiteral(" a , b ; a\n\n c ; , b , ")},
    };
    for (const auto &c : parseCsvCases) {
        QJsonObject args;
        args.insert(QStringLiteral("text"), c.second);
        out += jsonLine(QStringLiteral("Backend::parseCsv"), c.first, args,
                        QJsonArray::fromStringList(Backend::parseCsv(c.second)));
    }

    struct TruncateCase {
        QString label;
        QByteArray data;
        qint64 maxBytes;
    };
    const QList<TruncateCase> truncateCases = {
        {QStringLiteral("empty"), QByteArray(), 100},
        {QStringLiteral("max_zero"), QByteArray("hello\nworld\n"), 0},
        {QStringLiteral("max_negative"), QByteArray("hello\nworld\n"), -5},
        {QStringLiteral("under_limit"), QByteArray("hello\nworld\n"), 50},
        {QStringLiteral("exact_limit"), QByteArray("hello\nworld\n"), 12},
        {QStringLiteral("truncates_at_newline"),
         QByteArray("line1\nline2\nline3\nline4\n"), 14},
        {QStringLiteral("newline_at_index_0"), QByteArray("abcdef\nghijk"), 6},
        {QStringLiteral("newline_at_end_only"), QByteArray("abcdefghij\n"), 6},
        {QStringLiteral("no_newline_in_tail"), QByteArray("abcdefghijklmnop"), 6},
    };
    for (const auto &c : truncateCases) {
        QJsonObject args;
        args.insert(QStringLiteral("data"), QString::fromUtf8(c.data));
        args.insert(QStringLiteral("maxBytes"), static_cast<double>(c.maxBytes));
        out += jsonLine(QStringLiteral("Backend::truncateLogTail"), c.label, args,
                        QString::fromUtf8(Backend::truncateLogTail(c.data, c.maxBytes)));
    }

    return out;
}

static QByteArray captureConfig() {
    QByteArray out;

    const QByteArray oldLocEnv = qgetenv("AIBOX_LOCATION_FILE");
    const QByteArray oldXdgEnv = qgetenv("XDG_CONFIG_HOME");
    auto restoreEnvs = [&]() {
        if (oldLocEnv.isEmpty()) qunsetenv("AIBOX_LOCATION_FILE");
        else qputenv("AIBOX_LOCATION_FILE", oldLocEnv);
        if (oldXdgEnv.isEmpty()) qunsetenv("XDG_CONFIG_HOME");
        else qputenv("XDG_CONFIG_HOME", oldXdgEnv);
    };

    const QString workDir = QStringLiteral("/tmp/aibox-golden-config-work");
    QDir(workDir).removeRecursively();
    QDir().mkpath(workDir);

    // 1. Default paths (no AIBOX_LOCATION_FILE, no XDG_CONFIG_HOME)
    qunsetenv("AIBOX_LOCATION_FILE");
    qunsetenv("XDG_CONFIG_HOME");
    out += jsonLine(QStringLiteral("Backend::defaultSystemPath"), QStringLiteral("default"),
                    QJsonObject(), Backend::defaultSystemPath());
    out += jsonLine(QStringLiteral("Backend::defaultWorkspacePath"), QStringLiteral("default"),
                    QJsonObject(), Backend::defaultWorkspacePath());
    out += jsonLine(QStringLiteral("Backend::defaultOsStoragePathValue"), QStringLiteral("default"),
                    QJsonObject(), Backend::defaultOsStoragePathValue());
    out += jsonLine(QStringLiteral("Backend::defaultLocationFilePath"), QStringLiteral("default"),
                    QJsonObject(), Backend::defaultLocationFilePath());

    {
        qputenv("XDG_CONFIG_HOME", "/tmp/aibox-golden-xdg");
        out += jsonLine(QStringLiteral("Backend::defaultLocationFilePath"), QStringLiteral("xdg_override"),
                        QJsonObject(), Backend::defaultLocationFilePath());
        qunsetenv("XDG_CONFIG_HOME");
    }
    {
        qputenv("AIBOX_LOCATION_FILE", "/tmp/aibox-golden-loc/location.json");
        out += jsonLine(QStringLiteral("Backend::defaultLocationFilePath"), QStringLiteral("env_override"),
                        QJsonObject(), Backend::defaultLocationFilePath());
        qunsetenv("AIBOX_LOCATION_FILE");
    }

    // 2. configuredSystemPath + saveLocationFile
    const QString locFile = workDir + QStringLiteral("/loc/location.json");
    qputenv("AIBOX_LOCATION_FILE", locFile.toUtf8());
    out += jsonLine(QStringLiteral("Backend::configuredSystemPath"), QStringLiteral("missing_file"),
                    QJsonObject(), Backend::configuredSystemPath());
    {
        const bool saved = Backend::saveLocationFile(QStringLiteral("/tmp/aibox-golden-home/custom-sys/../sys"));
        QFile f(locFile);
        QString content;
        if (f.open(QIODevice::ReadOnly))
            content = QString::fromUtf8(f.readAll());
        QJsonObject res;
        res.insert(QStringLiteral("saved"), saved);
        res.insert(QStringLiteral("content"), content);
        res.insert(QStringLiteral("configured"), Backend::configuredSystemPath());
        out += jsonLine(QStringLiteral("Backend::saveLocationFile"), QStringLiteral("roundtrip"),
                        QJsonObject(), res);
    }
    {
        struct LocCase { QString label; QByteArray json; };
        const QList<LocCase> locCases = {
            {QStringLiteral("tilde"), R"({"systemPath":"~"})"},
            {QStringLiteral("tilde_sub"), R"({"systemPath":"~/my-aibox"})"},
            {QStringLiteral("relative"), R"({"systemPath":"rel/aibox"})"},
            {QStringLiteral("invalid_json"), R"({not json)"},
            {QStringLiteral("empty_path"), R"({"systemPath":"  "})"},
        };
        for (const auto &c : locCases) {
            QFile f(locFile);
            if (f.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
                f.write(c.json);
                f.close();
            }
            out += jsonLine(QStringLiteral("Backend::configuredSystemPath"), c.label,
                            QJsonObject(), Backend::configuredSystemPath());
        }
    }
    QDir(workDir + QStringLiteral("/loc")).removeRecursively();

    // 3. resolveProjectAiboxDir
    {
        const QString legacyProj = workDir + QStringLiteral("/proj_legacy");
        QDir().mkpath(legacyProj + QStringLiteral("/.aibox"));
        const QString bothProj = workDir + QStringLiteral("/proj_both");
        QDir().mkpath(bothProj + QStringLiteral("/.aibox"));
        QDir().mkpath(bothProj + QStringLiteral("/aibox"));

        struct ProjCase { QString label; QString projectDir; QString baseCwd; };
        const QList<ProjCase> projCases = {
            {QStringLiteral("empty_both"), QString(), QString()},
            {QStringLiteral("empty_proj_with_cwd"), QString(), QStringLiteral("/tmp/some-cwd")},
            {QStringLiteral("tilde"), QStringLiteral("~"), QStringLiteral("/tmp/cwd")},
            {QStringLiteral("tilde_sub"), QStringLiteral("~/proj"), QStringLiteral("/tmp/cwd")},
            {QStringLiteral("relative"), QStringLiteral("sub/proj"), QStringLiteral("/tmp/cwd")},
            {QStringLiteral("ends_aibox"), QStringLiteral("/tmp/proj/aibox"), QString()},
            {QStringLiteral("ends_dot_aibox"), QStringLiteral("/tmp/proj/.aibox"), QString()},
            {QStringLiteral("legacy_dot_aibox_only"), legacyProj, QString()},
            {QStringLiteral("both_aibox_and_dot_aibox"), bothProj, QString()},
        };
        for (const auto &c : projCases) {
            QJsonObject args;
            args.insert(QStringLiteral("projectDir"), c.projectDir);
            args.insert(QStringLiteral("baseCwd"), c.baseCwd);
            out += jsonLine(QStringLiteral("Backend::resolveProjectAiboxDir"), c.label, args,
                            Backend::resolveProjectAiboxDir(c.projectDir, c.baseCwd));
        }
    }

    // 4. resolveWorkspacePath
    {
        struct WsCase { QString label; QString rawParent; QString envName; QString defaultBase; };
        const QList<WsCase> wsCases = {
            {QStringLiteral("default_base"), QString(), QStringLiteral("demo"), QStringLiteral("/tmp/ws")},
            {QStringLiteral("empty_both"), QString(), QStringLiteral("demo"), QString()},
            {QStringLiteral("tilde_blocked"), QStringLiteral("~"), QStringLiteral("demo"), QStringLiteral("/tmp/ws")},
            {QStringLiteral("tilde_sub_ok"), QStringLiteral("~/ws"), QStringLiteral("demo"), QStringLiteral("/tmp/ws")},
            {QStringLiteral("relative_blocked"), QStringLiteral("rel/ws"), QStringLiteral("demo"), QStringLiteral("/tmp/ws")},
            {QStringLiteral("root_blocked"), QStringLiteral("/"), QStringLiteral("demo"), QStringLiteral("/tmp/ws")},
            {QStringLiteral("etc_sub_blocked"), QStringLiteral("/etc/aibox"), QStringLiteral("demo"), QStringLiteral("/tmp/ws")},
            {QStringLiteral("home_blocked"), QDir::homePath(), QStringLiteral("demo"), QStringLiteral("/tmp/ws")},
            {QStringLiteral("empty_env_name"), QStringLiteral("/tmp/custom"), QStringLiteral("  "), QStringLiteral("/tmp/ws")},
            {QStringLiteral("valid_custom"), QStringLiteral("/tmp/custom/../ws"), QStringLiteral("my-env"), QStringLiteral("/tmp/ws")},
        };
        for (const auto &c : wsCases) {
            QJsonObject args;
            args.insert(QStringLiteral("rawParent"), c.rawParent);
            args.insert(QStringLiteral("envName"), c.envName);
            args.insert(QStringLiteral("defaultBase"), c.defaultBase);
            QString err;
            const QString path = Backend::resolveWorkspacePath(c.rawParent, c.envName, c.defaultBase, &err);
            QJsonObject res;
            res.insert(QStringLiteral("path"), path);
            res.insert(QStringLiteral("error"), err);
            out += jsonLine(QStringLiteral("Backend::resolveWorkspacePath"), c.label, args, res);
        }
    }

    // 5. Defaults
    out += jsonLine(QStringLiteral("Backend::defaultOsEntries"), QStringLiteral("default"),
                    QJsonObject(), QJsonArray::fromVariantList(Backend::defaultOsEntries()));
    out += jsonLine(QStringLiteral("Backend::defaultPresets"), QStringLiteral("default"),
                    QJsonObject(), QJsonArray::fromVariantList(Backend::defaultPresets()));
    out += jsonLine(QStringLiteral("Backend::defaultDiskOptions"), QStringLiteral("default"),
                    QJsonObject(), QJsonArray::fromStringList(Backend::defaultDiskOptions()));
    out += jsonLine(QStringLiteral("Backend::defaultRamOptions"), QStringLiteral("default"),
                    QJsonObject(), QJsonArray::fromStringList(Backend::defaultRamOptions()));
    out += jsonLine(QStringLiteral("Backend::defaultCpuOptions"), QStringLiteral("default"),
                    QJsonObject(), QJsonArray::fromVariantList(Backend::defaultCpuOptions()));

    // 6. Backend instance: normalizedPath, cacheBasePath, default config save, broken config load+save
    {
        const QString freshDir = workDir + QStringLiteral("/fresh/aibox");
        Backend backend(freshDir, Backend::PathMode::ProjectDir);
        QJsonObject normArgs;
        for (const auto &pair : QList<QPair<QString, QString>>{
                 {QString(), QStringLiteral("/tmp/fb")},
                 {QStringLiteral("~"), QStringLiteral("/tmp/fb")},
                 {QStringLiteral("~/sub"), QStringLiteral("/tmp/fb")},
                 {QStringLiteral("rel/path"), QStringLiteral("/tmp/fb")},
                 {QStringLiteral("/a/b/../c/"), QStringLiteral("/tmp/fb")},
             }) {
            normArgs.insert(QStringLiteral("raw"), pair.first);
            normArgs.insert(QStringLiteral("fallback"), pair.second);
            out += jsonLine(QStringLiteral("Backend::normalizedPath"), pair.first.isEmpty() ? QStringLiteral("empty") : pair.first,
                            normArgs, backend.normalizedPath(pair.first, pair.second));
        }

        for (const QString &osId : {QStringLiteral("ubuntu-24.04"), QStringLiteral("foo/bar:baz")}) {
            QJsonObject args;
            args.insert(QStringLiteral("osId"), osId);
            out += jsonLine(QStringLiteral("Backend::cacheBasePath"), osId, args, backend.cacheBasePath(osId));
        }

        QJsonObject presetArgs;
        presetArgs.insert(QStringLiteral("id"), QStringLiteral("full"));
        out += jsonLine(QStringLiteral("Backend::presetById"), QStringLiteral("full"), presetArgs,
                        QJsonObject::fromVariantMap(backend.presetById(QStringLiteral("full"))));
        presetArgs.insert(QStringLiteral("id"), QStringLiteral("missing"));
        out += jsonLine(QStringLiteral("Backend::presetById"), QStringLiteral("missing"), presetArgs,
                        QJsonObject::fromVariantMap(backend.presetById(QStringLiteral("missing"))));

        QJsonObject osArgs;
        osArgs.insert(QStringLiteral("id"), QStringLiteral("archlinux"));
        out += jsonLine(QStringLiteral("Backend::osEntryById"), QStringLiteral("archlinux"), osArgs,
                        QJsonObject::fromVariantMap(backend.osEntryById(QStringLiteral("archlinux"))));
        osArgs.insert(QStringLiteral("id"), QStringLiteral("missing"));
        out += jsonLine(QStringLiteral("Backend::osEntryById"), QStringLiteral("missing"), osArgs,
                        QJsonObject::fromVariantMap(backend.osEntryById(QStringLiteral("missing"))));

        backend.saveConfig();
        QFile f(freshDir + QStringLiteral("/config.json"));
        QString savedJson;
        if (f.open(QIODevice::ReadOnly))
            savedJson = QString::fromUtf8(f.readAll());
        out += jsonLine(QStringLiteral("Backend::saveConfig"), QStringLiteral("fresh_default"),
                        QJsonObject(), savedJson);
    }

    // 7. Broken config load + normalize + save
    {
        const QString brokenDir = workDir + QStringLiteral("/broken/aibox");
        QDir().mkpath(brokenDir);
        const QByteArray brokenJson = R"json({
            "firstRunCompleted": true,
            "workspaceBase": "~/custom-ws",
            "osStoragePath": "rel-os-storage",
            "osCatalog": [
                {"id": "  Custom-OS ", "alias": "", "name": "", "badge": "", "description": "  desc  "},
                {"id": "custom-os", "alias": "dup"},
                {"id": "", "alias": "empty"}
            ],
            "presets": [
                {"id": "full", "title": "Full Custom", "sub": "Sub"},
                {"id": "", "title": "NoId"}
            ],
            "diskOptions": ["bad", "10GiB", "10gib", "512MiB"],
            "ramOptions": ["nope"],
            "cpuOptions": [0, -1, 4, "4", "abc", 8, 2000],
            "defaultPresetId": "unknown-preset",
            "defaultOsId": "unknown-os",
            "defaultDisk": "999GiB",
            "defaultRam": "999GiB",
            "defaultCpu": 99,
            "autoSnapshotEnabled": false,
            "autoSnapshotKeep": 999,
            "watchdogEnabled": false,
            "watchdogDiskWarn": 10,
            "watchdogDiskCritical": 5,
            "watchdogAutoStop": true,
            "idleStopMinutes": -5,
            "staleDays": 0,
            "recordSessions": true,
            "diffExcludes": [" *.log ", "", "*.log", ".git"]
        })json";
        QFile bf(brokenDir + QStringLiteral("/config.json"));
        if (bf.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
            bf.write(brokenJson);
            bf.close();
        }
        Backend backend(brokenDir, Backend::PathMode::ProjectDir);
        backend.saveConfig();
        QFile sf(brokenDir + QStringLiteral("/config.json"));
        QString normalizedJson;
        if (sf.open(QIODevice::ReadOnly))
            normalizedJson = QString::fromUtf8(sf.readAll());
        QJsonObject args;
        args.insert(QStringLiteral("input"), QString::fromUtf8(brokenJson));
        out += jsonLine(QStringLiteral("Backend::loadAndSaveBrokenConfig"), QStringLiteral("broken"),
                        args, normalizedJson);
    }

    QDir(workDir).removeRecursively();
    restoreEnvs();
    return out;
}

static QByteArray captureSettings() {
    QByteArray out;
    const QByteArray oldLocEnv = qgetenv("AIBOX_LOCATION_FILE");
    const QByteArray oldXdgEnv = qgetenv("XDG_CONFIG_HOME");
    auto restoreEnvs = [&]() {
        if (oldLocEnv.isEmpty()) qunsetenv("AIBOX_LOCATION_FILE");
        else qputenv("AIBOX_LOCATION_FILE", oldLocEnv);
        if (oldXdgEnv.isEmpty()) qunsetenv("XDG_CONFIG_HOME");
        else qputenv("XDG_CONFIG_HOME", oldXdgEnv);
    };
    qunsetenv("AIBOX_LOCATION_FILE");
    qunsetenv("XDG_CONFIG_HOME");

    const QString workDir = QStringLiteral("/tmp/aibox-golden-settings-work");
    QDir(workDir).removeRecursively();
    QDir().mkpath(workDir);

    auto readFileStr = [](const QString &path) -> QString {
        QFile f(path);
        if (!f.open(QIODevice::ReadOnly))
            return {};
        return QString::fromUtf8(f.readAll());
    };

    // 1. staleDayOptions
    {
        const QString dir = workDir + QStringLiteral("/stale-opts");
        QDir().mkpath(dir);
        Backend b(dir, Backend::PathMode::ProjectDir);
        QJsonArray arr;
        for (const auto &v : b.staleDayOptions())
            arr.append(v.toInt());
        out += jsonLine(QStringLiteral("Backend::staleDayOptions"), QStringLiteral("default"),
                        QJsonObject(), arr);
    }

    // 2. completeFirstRun: empty path keeps workspace, custom path uses structured layout, file path errors
    {
        const QString dir = workDir + QStringLiteral("/firstrun-empty");
        QDir().mkpath(dir);
        Backend b(dir, Backend::PathMode::WorkspaceDir);
        const QString err = b.completeFirstRun(QString());
        QJsonObject res;
        res.insert(QStringLiteral("error"), err);
        res.insert(QStringLiteral("firstRun"), b.m_firstRun);
        res.insert(QStringLiteral("systemPath"), b.m_systemPath);
        res.insert(QStringLiteral("workspaceBase"), b.m_workspaceBase);
        res.insert(QStringLiteral("osStoragePath"), b.m_osStoragePath);
        out += jsonLine(QStringLiteral("Backend::completeFirstRun"), QStringLiteral("empty_default_mode"),
                        QJsonObject{{QStringLiteral("systemPath"), QString()}}, res);
    }
    {
        const QString dir = workDir + QStringLiteral("/firstrun-custom");
        QDir().mkpath(dir);
        Backend b(dir, Backend::PathMode::ProjectDir);
        const QString target = workDir + QStringLiteral("/firstrun-custom-target");
        const QString err = b.completeFirstRun(target);
        QJsonObject res;
        res.insert(QStringLiteral("error"), err);
        res.insert(QStringLiteral("firstRun"), b.m_firstRun);
        res.insert(QStringLiteral("systemPath"), b.m_systemPath);
        res.insert(QStringLiteral("workspaceBase"), b.m_workspaceBase);
        res.insert(QStringLiteral("osStoragePath"), b.m_osStoragePath);
        res.insert(QStringLiteral("configJson"), readFileStr(target + QStringLiteral("/config.json")));
        out += jsonLine(QStringLiteral("Backend::completeFirstRun"), QStringLiteral("structured_custom"),
                        QJsonObject{{QStringLiteral("systemPath"), target}}, res);
    }
    {
        const QString dir = workDir + QStringLiteral("/firstrun-err");
        QDir().mkpath(dir);
        const QString blockingFile = workDir + QStringLiteral("/blocking-file");
        QFile bf(blockingFile);
        if (bf.open(QIODevice::WriteOnly)) {
            bf.write("x");
            bf.close();
        }
        Backend b(dir, Backend::PathMode::ProjectDir);
        const QString err = b.completeFirstRun(blockingFile);
        out += jsonLine(QStringLiteral("Backend::completeFirstRun"), QStringLiteral("not_a_dir"),
                        QJsonObject{{QStringLiteral("systemPath"), blockingFile}},
                        QJsonObject{{QStringLiteral("error"), err}});
    }

    // 3. saveSettings
    {
        const QString dir = workDir + QStringLiteral("/save-settings");
        QDir().mkpath(dir);
        Backend b(dir, Backend::PathMode::ProjectDir);
        const QString ws = workDir + QStringLiteral("/save-settings/ws-new");
        const QString os = workDir + QStringLiteral("/save-settings/os-new");
        const QString err = b.saveSettings(ws, os);
        out += jsonLine(QStringLiteral("Backend::saveSettings"), QStringLiteral("ok"),
                        QJsonObject{{QStringLiteral("workspace"), ws}, {QStringLiteral("osStorage"), os}},
                        QJsonObject{{QStringLiteral("error"), err},
                                    {QStringLiteral("configJson"), readFileStr(b.m_configPath)}});

        const QString blockingFile = workDir + QStringLiteral("/blocking-file");
        const QString errWs = b.saveSettings(blockingFile, os);
        out += jsonLine(QStringLiteral("Backend::saveSettings"), QStringLiteral("bad_workspace"),
                        QJsonObject{{QStringLiteral("workspace"), blockingFile}, {QStringLiteral("osStorage"), os}},
                        QJsonObject{{QStringLiteral("error"), errWs}});

        const QString errOs = b.saveSettings(ws, blockingFile);
        out += jsonLine(QStringLiteral("Backend::saveSettings"), QStringLiteral("bad_os_storage"),
                        QJsonObject{{QStringLiteral("workspace"), ws}, {QStringLiteral("osStorage"), blockingFile}},
                        QJsonObject{{QStringLiteral("error"), errOs}});
    }

    // 4. saveOsEntry, removeOsEntry, resetOsCatalog
    {
        const QString dir = workDir + QStringLiteral("/os-catalog");
        QDir().mkpath(dir);
        Backend b(dir, Backend::PathMode::ProjectDir);

        auto callSaveOs = [&](const QString &label, const QString &origId, const QString &id,
                              const QString &alias, const QString &name, const QString &badge,
                              const QString &desc) {
            const QString err = b.saveOsEntry(origId, id, alias, name, badge, desc);
            out += jsonLine(QStringLiteral("Backend::saveOsEntry"), label,
                            QJsonObject{{QStringLiteral("originalId"), origId},
                                        {QStringLiteral("id"), id},
                                        {QStringLiteral("alias"), alias},
                                        {QStringLiteral("name"), name},
                                        {QStringLiteral("badge"), badge},
                                        {QStringLiteral("description"), desc}},
                            QJsonObject{{QStringLiteral("error"), err},
                                        {QStringLiteral("configJson"), readFileStr(b.m_configPath)}});
        };

        callSaveOs(QStringLiteral("invalid_id"), QString(), QStringLiteral("Bad ID!"), QString(), QString(), QString(), QString());
        callSaveOs(QStringLiteral("invalid_alias"), QString(), QStringLiteral("my-os"), QStringLiteral("bad alias!"), QString(), QString(), QString());
        callSaveOs(QStringLiteral("duplicate_id"), QString(), QStringLiteral("ubuntu-24.04"), QString(), QString(), QString(), QString());
        callSaveOs(QStringLiteral("missing_original"), QStringLiteral("no-such"), QStringLiteral("my-os"), QString(), QString(), QString(), QString());
        callSaveOs(QStringLiteral("add_defaults"), QString(), QStringLiteral("  Debian-12  "), QString(), QString(), QString(), QStringLiteral("  Stable  "));
        callSaveOs(QStringLiteral("edit_existing"), QStringLiteral("debian-12"), QStringLiteral("debian-12-lts"),
                   QStringLiteral("debian/12"), QStringLiteral("Debian 12 LTS"), QStringLiteral("LTS"), QStringLiteral("Updated"));

        const QString errMissing = b.removeOsEntry(QStringLiteral("no-such"));
        out += jsonLine(QStringLiteral("Backend::removeOsEntry"), QStringLiteral("missing"),
                        QJsonObject{{QStringLiteral("id"), QStringLiteral("no-such")}},
                        QJsonObject{{QStringLiteral("error"), errMissing}});

        const QString errRemove = b.removeOsEntry(QStringLiteral("debian-12-lts"));
        out += jsonLine(QStringLiteral("Backend::removeOsEntry"), QStringLiteral("ok"),
                        QJsonObject{{QStringLiteral("id"), QStringLiteral("debian-12-lts")}},
                        QJsonObject{{QStringLiteral("error"), errRemove},
                                    {QStringLiteral("configJson"), readFileStr(b.m_configPath)}});

        // Remove until 1 left
        while (b.m_osEntries.size() > 1)
            b.removeOsEntry(b.m_osEntries.first().toMap().value(QStringLiteral("id")).toString());
        const QString lastId = b.m_osEntries.first().toMap().value(QStringLiteral("id")).toString();
        const QString errLast = b.removeOsEntry(lastId);
        out += jsonLine(QStringLiteral("Backend::removeOsEntry"), QStringLiteral("last_entry"),
                        QJsonObject{{QStringLiteral("id"), lastId}},
                        QJsonObject{{QStringLiteral("error"), errLast}});

        const QString errReset = b.resetOsCatalog();
        out += jsonLine(QStringLiteral("Backend::resetOsCatalog"), QStringLiteral("ok"),
                        QJsonObject(),
                        QJsonObject{{QStringLiteral("error"), errReset},
                                    {QStringLiteral("configJson"), readFileStr(b.m_configPath)}});
    }

    // 5. saveCreateOptions, savePreset, resetCreateOptions
    {
        const QString dir = workDir + QStringLiteral("/create-opts");
        QDir().mkpath(dir);
        Backend b(dir, Backend::PathMode::ProjectDir);

        auto callCreateOpts = [&](const QString &label, const QString &disk, const QString &ram, const QString &cpu) {
            const QString err = b.saveCreateOptions(disk, ram, cpu);
            out += jsonLine(QStringLiteral("Backend::saveCreateOptions"), label,
                            QJsonObject{{QStringLiteral("disk"), disk}, {QStringLiteral("ram"), ram}, {QStringLiteral("cpu"), cpu}},
                            QJsonObject{{QStringLiteral("error"), err},
                                        {QStringLiteral("configJson"), readFileStr(b.m_configPath)}});
        };

        callCreateOpts(QStringLiteral("empty_disk"), QString(), QStringLiteral("4GiB"), QStringLiteral("2"));
        callCreateOpts(QStringLiteral("bad_disk"), QStringLiteral("35GiB, nope"), QStringLiteral("4GiB"), QStringLiteral("2"));
        callCreateOpts(QStringLiteral("empty_ram"), QStringLiteral("35GiB"), QString(), QStringLiteral("2"));
        callCreateOpts(QStringLiteral("bad_ram"), QStringLiteral("35GiB"), QStringLiteral("4GiB, nope"), QStringLiteral("2"));
        callCreateOpts(QStringLiteral("bad_cpu_range"), QStringLiteral("35GiB"), QStringLiteral("4GiB"), QStringLiteral("0"));
        callCreateOpts(QStringLiteral("bad_cpu_high"), QStringLiteral("35GiB"), QStringLiteral("4GiB"), QStringLiteral("1025"));
        callCreateOpts(QStringLiteral("empty_cpu"), QStringLiteral("35GiB"), QStringLiteral("4GiB"), QString());
        callCreateOpts(QStringLiteral("ok"), QStringLiteral("20GiB, 40GiB"), QStringLiteral("4GiB, 8GiB"), QStringLiteral("2, 4, 8"));

        out += jsonLine(QStringLiteral("Backend::savePreset"), QStringLiteral("empty_title"),
                        QJsonObject{{QStringLiteral("id"), QStringLiteral("full")}, {QStringLiteral("title"), QStringLiteral("  ")}, {QStringLiteral("sub"), QStringLiteral("s")}},
                        QJsonObject{{QStringLiteral("error"), b.savePreset(QStringLiteral("full"), QStringLiteral("  "), QStringLiteral("s"))}});
        out += jsonLine(QStringLiteral("Backend::savePreset"), QStringLiteral("missing"),
                        QJsonObject{{QStringLiteral("id"), QStringLiteral("nope")}, {QStringLiteral("title"), QStringLiteral("T")}, {QStringLiteral("sub"), QStringLiteral("s")}},
                        QJsonObject{{QStringLiteral("error"), b.savePreset(QStringLiteral("nope"), QStringLiteral("T"), QStringLiteral("s"))}});
        const QString errPreset = b.savePreset(QStringLiteral("full"), QStringLiteral("  Full Dev  "), QStringLiteral("  Custom  "));
        out += jsonLine(QStringLiteral("Backend::savePreset"), QStringLiteral("ok"),
                        QJsonObject{{QStringLiteral("id"), QStringLiteral("full")}, {QStringLiteral("title"), QStringLiteral("  Full Dev  ")}, {QStringLiteral("sub"), QStringLiteral("  Custom  ")}},
                        QJsonObject{{QStringLiteral("error"), errPreset},
                                    {QStringLiteral("configJson"), readFileStr(b.m_configPath)}});

        const QString errReset = b.resetCreateOptions();
        out += jsonLine(QStringLiteral("Backend::resetCreateOptions"), QStringLiteral("ok"),
                        QJsonObject(),
                        QJsonObject{{QStringLiteral("error"), errReset},
                                    {QStringLiteral("configJson"), readFileStr(b.m_configPath)}});
    }

    // 6. Auto-snapshot, watchdog, idle-stop, stale, session recording, diff excludes
    {
        const QString dir = workDir + QStringLiteral("/misc-settings");
        QDir().mkpath(dir);
        Backend b(dir, Backend::PathMode::ProjectDir);

        out += jsonLine(QStringLiteral("Backend::saveAutoSnapshotSettings"), QStringLiteral("low"),
                        QJsonObject{{QStringLiteral("enabled"), true}, {QStringLiteral("keep"), 0}},
                        QJsonObject{{QStringLiteral("error"), b.saveAutoSnapshotSettings(true, 0)}});
        out += jsonLine(QStringLiteral("Backend::saveAutoSnapshotSettings"), QStringLiteral("high"),
                        QJsonObject{{QStringLiteral("enabled"), true}, {QStringLiteral("keep"), 51}},
                        QJsonObject{{QStringLiteral("error"), b.saveAutoSnapshotSettings(true, 51)}});
        out += jsonLine(QStringLiteral("Backend::saveAutoSnapshotSettings"), QStringLiteral("ok"),
                        QJsonObject{{QStringLiteral("enabled"), false}, {QStringLiteral("keep"), 12}},
                        QJsonObject{{QStringLiteral("error"), b.saveAutoSnapshotSettings(false, 12)},
                                    {QStringLiteral("configJson"), readFileStr(b.m_configPath)}});

        out += jsonLine(QStringLiteral("Backend::saveWatchdogSettings"), QStringLiteral("warn_low"),
                        QJsonObject{{QStringLiteral("enabled"), true}, {QStringLiteral("warn"), 49}, {QStringLiteral("crit"), 95}, {QStringLiteral("autoStop"), false}},
                        QJsonObject{{QStringLiteral("error"), b.saveWatchdogSettings(true, 49, 95, false)}});
        out += jsonLine(QStringLiteral("Backend::saveWatchdogSettings"), QStringLiteral("crit_le_warn"),
                        QJsonObject{{QStringLiteral("enabled"), true}, {QStringLiteral("warn"), 80}, {QStringLiteral("crit"), 80}, {QStringLiteral("autoStop"), false}},
                        QJsonObject{{QStringLiteral("error"), b.saveWatchdogSettings(true, 80, 80, false)}});
        out += jsonLine(QStringLiteral("Backend::saveWatchdogSettings"), QStringLiteral("crit_high"),
                        QJsonObject{{QStringLiteral("enabled"), true}, {QStringLiteral("warn"), 80}, {QStringLiteral("crit"), 100}, {QStringLiteral("autoStop"), false}},
                        QJsonObject{{QStringLiteral("error"), b.saveWatchdogSettings(true, 80, 100, false)}});
        out += jsonLine(QStringLiteral("Backend::saveWatchdogSettings"), QStringLiteral("ok"),
                        QJsonObject{{QStringLiteral("enabled"), false}, {QStringLiteral("warn"), 75}, {QStringLiteral("crit"), 95}, {QStringLiteral("autoStop"), true}},
                        QJsonObject{{QStringLiteral("error"), b.saveWatchdogSettings(false, 75, 95, true)},
                                    {QStringLiteral("configJson"), readFileStr(b.m_configPath)}});

        out += jsonLine(QStringLiteral("Backend::saveIdleStopSettings"), QStringLiteral("low"),
                        QJsonObject{{QStringLiteral("minutes"), -1}},
                        QJsonObject{{QStringLiteral("error"), b.saveIdleStopSettings(-1)}});
        out += jsonLine(QStringLiteral("Backend::saveIdleStopSettings"), QStringLiteral("high"),
                        QJsonObject{{QStringLiteral("minutes"), 1441}},
                        QJsonObject{{QStringLiteral("error"), b.saveIdleStopSettings(1441)}});
        out += jsonLine(QStringLiteral("Backend::saveIdleStopSettings"), QStringLiteral("ok"),
                        QJsonObject{{QStringLiteral("minutes"), 45}},
                        QJsonObject{{QStringLiteral("error"), b.saveIdleStopSettings(45)},
                                    {QStringLiteral("configJson"), readFileStr(b.m_configPath)}});

        out += jsonLine(QStringLiteral("Backend::saveStaleSettings"), QStringLiteral("low"),
                        QJsonObject{{QStringLiteral("days"), 0}},
                        QJsonObject{{QStringLiteral("error"), b.saveStaleSettings(0)}});
        out += jsonLine(QStringLiteral("Backend::saveStaleSettings"), QStringLiteral("high"),
                        QJsonObject{{QStringLiteral("days"), 3651}},
                        QJsonObject{{QStringLiteral("error"), b.saveStaleSettings(3651)}});
        out += jsonLine(QStringLiteral("Backend::saveStaleSettings"), QStringLiteral("ok"),
                        QJsonObject{{QStringLiteral("days"), 60}},
                        QJsonObject{{QStringLiteral("error"), b.saveStaleSettings(60)},
                                    {QStringLiteral("configJson"), readFileStr(b.m_configPath)}});

        out += jsonLine(QStringLiteral("Backend::saveSessionRecordingSettings"), QStringLiteral("ok_true"),
                        QJsonObject{{QStringLiteral("enabled"), true}},
                        QJsonObject{{QStringLiteral("error"), b.saveSessionRecordingSettings(true)},
                                    {QStringLiteral("configJson"), readFileStr(b.m_configPath)}});

        out += jsonLine(QStringLiteral("Backend::saveDiffExcludes"), QStringLiteral("ok"),
                        QJsonObject{{QStringLiteral("raw"), QStringLiteral("*.log, .git;\nnode_modules\n*.log")}},
                        QJsonObject{{QStringLiteral("error"), b.saveDiffExcludes(QStringLiteral("*.log, .git;\nnode_modules\n*.log"))},
                                    {QStringLiteral("configJson"), readFileStr(b.m_configPath)}});
    }

    QDir(workDir).removeRecursively();
    restoreEnvs();
    return out;
}

static QByteArray captureTheme() {
    QByteArray out;
    const QString workDir = QStringLiteral("/tmp/aibox-golden-theme-work");
    QDir(workDir).removeRecursively();

    auto writeColors = [&](const QString &name, const QByteArray &content) {
        const QString dir = workDir + QLatin1Char('/') + name;
        QDir().mkpath(dir + QStringLiteral("/theme"));
        QFile f(dir + QStringLiteral("/theme/colors.toml"));
        if (f.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
            f.write(content);
            f.close();
        }
        return dir;
    };

    auto themeOut = [](const Theme &t) {
        return QJsonObject{
            {QStringLiteral("accent"), t.accent()},
            {QStringLiteral("accentForeground"), t.accentForeground()},
            {QStringLiteral("background"), t.background()},
            {QStringLiteral("darkBackground"), t.darkBackground()},
            {QStringLiteral("lighterBackground"), t.lighterBackground()},
            {QStringLiteral("foreground"), t.foreground()},
            {QStringLiteral("muted"), t.muted()},
            {QStringLiteral("green"), t.green()},
            {QStringLiteral("red"), t.red()},
            {QStringLiteral("yellow"), t.yellow()},
        };
    };

    {
        const QString dir = workDir + QStringLiteral("/missing");
        QDir().mkpath(dir);
        Theme t(dir);
        out += jsonLine(QStringLiteral("Theme::parseColors"), QStringLiteral("missing"),
                        QJsonObject{{QStringLiteral("toml"), QString()}}, themeOut(t));
    }

    const QList<QPair<QString, QByteArray>> tomlCases = {
        {QStringLiteral("partial_accent_only"), "accent = \"#112233\"\n"},
        {QStringLiteral("single_quotes_and_spaces"), "  accent = '#abcdef'  \n  foreground = '#123456'\n"},
        {QStringLiteral("bright_foreground_overrides"), "foreground = \"#111111\"\nbright_foreground = \"#eeeeee\"\n"},
        {QStringLiteral("invalid_hex_ignored"), "accent = \"#12345\"\nbackground = \"red\"\n"},
        {QStringLiteral("full_tokyo_night"),
         "mode = \"dark\"\n"
         "accent = \"#7aa2f7\"\n"
         "background = \"#1a1b26\"\n"
         "dark_background = \"#13141c\"\n"
         "lighter_background = \"#24283b\"\n"
         "foreground = \"#a9b1d6\"\n"
         "dark_foreground = \"#565f89\"\n"
         "bright_foreground = \"#c0caf5\"\n"
         "red = \"#f7768e\"\n"
         "yellow = \"#e0af68\"\n"
         "green = \"#9ece6a\"\n"},
    };
    for (const auto &c : tomlCases) {
        const QString dir = writeColors(c.first, c.second);
        Theme t(dir);
        out += jsonLine(QStringLiteral("Theme::parseColors"), c.first,
                        QJsonObject{{QStringLiteral("toml"), QString::fromUtf8(c.second)}},
                        themeOut(t));
    }

    const QStringList accentSamples = {
        QStringLiteral("#FFD60A"),
        QStringLiteral("#112233"),
        QStringLiteral("#000000"),
        QStringLiteral("#ffffff"),
        QStringLiteral("#7aa2f7"),
        QStringLiteral("#8bc9eb"),
        QStringLiteral("#6e6e6e"),
        QStringLiteral("#737373"),
        QStringLiteral("#747474"),
    };
    for (const auto &hex : accentSamples) {
        const QString dir = writeColors(QStringLiteral("acc_") + hex.mid(1),
                                        QByteArray("accent = \"") + hex.toUtf8() + "\"\n");
        Theme t(dir);
        out += jsonLine(QStringLiteral("Theme::accentForeground"), hex,
                        QJsonObject{{QStringLiteral("accent"), hex}},
                        t.accentForeground());
    }

    QDir(workDir).removeRecursively();
    return out;
}

static QByteArray captureI18n() {
    QByteArray out;
    const QString workDir = QStringLiteral("/tmp/aibox-golden-i18n-work");
    QDir(workDir).removeRecursively();
    QDir().mkpath(workDir);

    auto writeFile = [](const QString &path, const QByteArray &data) {
        QFile f(path);
        if (f.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
            f.write(data);
            f.close();
        }
    };

    out += jsonLine(QStringLiteral("Translator::readLanguage"), QStringLiteral("missing"),
                    QJsonObject{{QStringLiteral("content"), QJsonValue::Null}},
                    Translator::readLanguage(workDir + QStringLiteral("/missing-lang")));

    const QList<QPair<QString, QByteArray>> langCases = {
        {QStringLiteral("empty"), ""},
        {QStringLiteral("pt_br"), "pt-BR\n"},
        {QStringLiteral("da_ws"), "  da  \nignored\n"},
        {QStringLiteral("zh_cn"), "zh-CN\n"},
        {QStringLiteral("fil"), "fil\n"},
        {QStringLiteral("invalid_long"), "english\n"},
        {QStringLiteral("invalid_underscore"), "en_US\n"},
        {QStringLiteral("invalid_digits"), "123\n"},
    };
    for (const auto &c : langCases) {
        const QString p = workDir + QStringLiteral("/lang_") + c.first;
        writeFile(p, c.second);
        out += jsonLine(QStringLiteral("Translator::readLanguage"), c.first,
                        QJsonObject{{QStringLiteral("content"), QString::fromUtf8(c.second)}},
                        Translator::readLanguage(p));
    }

    struct PluralCase {
        const char *lang;
        QList<long long> counts;
    };
    const PluralCase pluralCases[] = {
        {"ja", {0, 1, 2}},
        {"ko", {1}},
        {"zh-CN", {1}},
        {"zh-TW", {1}},
        {"th", {1}},
        {"vi", {1}},
        {"id", {1}},
        {"ms", {1}},
        {"hi", {0, 1, 2}},
        {"bn", {0, 1, 2}},
        {"fa", {0, 1, 2}},
        {"si", {0, 1, 2}},
        {"fr", {0, 1, 2, 1000000, 2000000, 1000001}},
        {"pt-BR", {0, 1, 2, 1000000}},
        {"ca", {0, 1, 2, 1000000}},
        {"es-MX", {0, 1, 2, 1000000}},
        {"it", {0, 1, 2, 1000000}},
        {"pt", {0, 1, 2, 1000000}},
        {"fil", {1, 4, 6, 9, 10, 14}},
        {"is", {1, 11, 21, 2}},
        {"cs", {1, 2, 4, 5}},
        {"sk", {1, 2, 4, 5}},
        {"pl", {1, 2, 4, 5, 12, 14, 22, 25}},
        {"ru", {1, 2, 5, 11, 12, 21, 22, 25}},
        {"uk", {1, 2, 5, 11, 12, 21, 22, 25}},
        {"hr", {1, 2, 5, 11, 12, 21, 22, 25}},
        {"sr", {1, 2, 5, 11, 12, 21, 22, 25}},
        {"sl", {1, 2, 3, 4, 5, 101, 102, 103}},
        {"lt", {1, 2, 10, 11, 19, 21, 22}},
        {"lv", {0, 1, 10, 11, 19, 21, 22}},
        {"ro", {0, 1, 2, 19, 20, 101, 102}},
        {"ar", {0, 1, 2, 3, 10, 11, 99, 100, 101, 103, 111}},
        {"he", {0, 1, 2, 3}},
        {"ga", {1, 2, 3, 6, 7, 10, 11}},
        {"en", {0, 1, 2, -1, -2}},
        {"da", {0, 1, 2}},
    };
    for (const auto &pc : pluralCases) {
        const QString lang = QString::fromLatin1(pc.lang);
        for (long long count : pc.counts) {
            const QString label = lang + QLatin1Char('_') + QString::number(count);
            out += jsonLine(QStringLiteral("Translator::pluralCategory"), label,
                            QJsonObject{{QStringLiteral("language"), lang},
                                        {QStringLiteral("count"), static_cast<double>(count)}},
                            Translator::pluralCategory(lang, count));
        }
    }

    // Layered catalog lookup
    {
        const QString catDir = workDir + QStringLiteral("/cat1");
        QDir().mkpath(catDir);
        writeFile(catDir + QStringLiteral("/language"), "pt-BR\n");
        writeFile(catDir + QStringLiteral("/en.json"),
                  R"({"%n clips": {"one": "%n clip", "other": "%n clips"}})");
        writeFile(catDir + QStringLiteral("/pt.json"),
                  R"({"Save": "Guardar", "Open": "Abrir", "Empty": ""})");
        writeFile(catDir + QStringLiteral("/pt-BR.json"),
                  R"({"Save": "Salvar"})");
        Translator tr(catDir + QStringLiteral("/language"), catDir);
        out += jsonLine(QStringLiteral("Translator::translate"), QStringLiteral("pt_br_layered"),
                        QJsonObject{{QStringLiteral("language"), tr.language()}},
                        QJsonObject{
                            {QStringLiteral("Save"), tr.translate("Main", "Save")},
                            {QStringLiteral("Open"), tr.translate("Main", "Open")},
                            {QStringLiteral("EmptyIsNull"), tr.translate("Main", "Empty").isNull()},
                            {QStringLiteral("QuitIsNull"), tr.translate("Main", "Quit").isNull()},
                            {QStringLiteral("Clips1"), tr.translate("Main", "%n clips", nullptr, 1)},
                            {QStringLiteral("Clips2"), tr.translate("Main", "%n clips", nullptr, 2)},
                        });
    }
    {
        const QString catDir = workDir + QStringLiteral("/cat2");
        QDir().mkpath(catDir);
        writeFile(catDir + QStringLiteral("/language"), "ru\n");
        writeFile(catDir + QStringLiteral("/en.json"),
                  R"({"%n clips": {"one": "%n clip", "other": "%n clips"}})");
        writeFile(catDir + QStringLiteral("/ru.json"),
                  R"({"%n clips": {"one": "%n клип", "few": "%n клипа", "many": "%n клипов", "other": "%n клипа"}})");
        Translator tr(catDir + QStringLiteral("/language"), catDir);
        out += jsonLine(QStringLiteral("Translator::translate"), QStringLiteral("ru_plurals"),
                        QJsonObject{{QStringLiteral("language"), tr.language()}},
                        QJsonObject{
                            {QStringLiteral("n21"), tr.translate("Main", "%n clips", nullptr, 21)},
                            {QStringLiteral("n22"), tr.translate("Main", "%n clips", nullptr, 22)},
                            {QStringLiteral("n25"), tr.translate("Main", "%n clips", nullptr, 25)},
                            {QStringLiteral("nNeg"), tr.translate("Main", "%n clips", nullptr, -1)},
                        });
    }

    QDir(workDir).removeRecursively();
    return out;
}

static QByteArray captureSecrets() {
    QByteArray out;

    const QByteArray oldXdgEnv = qgetenv("XDG_CONFIG_HOME");
    const auto restoreXdg = qScopeGuard([&] {
        if (oldXdgEnv.isEmpty()) qunsetenv("XDG_CONFIG_HOME");
        else qputenv("XDG_CONFIG_HOME", oldXdgEnv);
    });
    qunsetenv("XDG_CONFIG_HOME");

    out += jsonLine(QStringLiteral("Backend::defaultSecretsStorePath"), QStringLiteral("default"),
                    QJsonObject{}, Backend::defaultSecretsStorePath());

    const QString max64(64, QLatin1Char('a'));
    const QString over65(65, QLatin1Char('a'));
    const QList<QPair<QString, QString>> envFileCases = {
        {QStringLiteral("alpha"), QStringLiteral("alpha")},
        {QStringLiteral("env_1"), QStringLiteral("env-1")},
        {QStringLiteral("digit_start"), QStringLiteral("0box")},
        {QStringLiteral("max64"), max64},
        {QStringLiteral("over65"), over65},
        {QStringLiteral("empty"), QStringLiteral("")},
        {QStringLiteral("leading_dash"), QStringLiteral("-bad")},
        {QStringLiteral("uppercase"), QStringLiteral("Alpha")},
        {QStringLiteral("underscore"), QStringLiteral("a_b")},
        {QStringLiteral("traversal"), QStringLiteral("../etc")},
    };
    for (const auto &c : envFileCases) {
        out += jsonLine(QStringLiteral("Backend::isSafeEnvironmentFileName"), c.first,
                        QJsonObject{{QStringLiteral("name"), c.second}},
                        Backend::isSafeEnvironmentFileName(c.second));
    }

    const QList<QPair<QString, QString>> secretNameCases = {
        {QStringLiteral("openai"), QStringLiteral("OPENAI_API_KEY")},
        {QStringLiteral("private"), QStringLiteral("_PRIVATE_1")},
        {QStringLiteral("single"), QStringLiteral("A")},
        {QStringLiteral("empty"), QStringLiteral("")},
        {QStringLiteral("lowercase"), QStringLiteral("lowercase")},
        {QStringLiteral("leading_digit"), QStringLiteral("1INVALID")},
        {QStringLiteral("hyphen"), QStringLiteral("BAD-NAME")},
        {QStringLiteral("equals"), QStringLiteral("BAD=VAL")},
    };
    for (const auto &c : secretNameCases) {
        out += jsonLine(QStringLiteral("Backend::isValidSecretName"), c.first,
                        QJsonObject{{QStringLiteral("name"), c.second}},
                        Backend::isValidSecretName(c.second));
    }

    const QList<QPair<QString, QString>> csvCases = {
        {QStringLiteral("empty"), QStringLiteral("")},
        {QStringLiteral("mixed"),
         QStringLiteral("OPENAI_API_KEY, bad_name, KEY=val, GITHUB_TOKEN, OPENAI_API_KEY")},
        {QStringLiteral("blanks"), QStringLiteral(" , , _A , B1_2 ")},
    };
    for (const auto &c : csvCases) {
        QJsonArray arr;
        for (const QString &s : Backend::parseEnvSecretNames(c.second))
            arr.append(s);
        out += jsonLine(QStringLiteral("Backend::parseEnvSecretNames"), c.first,
                        QJsonObject{{QStringLiteral("rawCsv"), c.second}}, arr);
    }

    {
        const QList<QPair<QString, QString>> pairs = {
            {QStringLiteral("OPENAI_API_KEY"), QStringLiteral("sk-test-short")},
            {QStringLiteral("bad-name"), QStringLiteral("sk-test-ignored")},
            {QStringLiteral("TRICKY_VAL"), QStringLiteral("it's $HOME $(id) `uname`\nline2")},
            {QStringLiteral("EMPTY_VAL"), QStringLiteral("")},
        };
        QJsonArray inArr;
        for (const auto &p : pairs) {
            inArr.append(QJsonObject{{QStringLiteral("name"), p.first},
                                     {QStringLiteral("value"), p.second}});
        }
        out += jsonLine(QStringLiteral("Backend::buildSecretEnvFileContent"),
                        QStringLiteral("mixed"),
                        QJsonObject{{QStringLiteral("secrets"), inArr}},
                        Backend::buildSecretEnvFileContent(pairs));
    }

    struct RedactCase {
        QString label;
        QString text;
        QStringList values;
    };
    const QList<RedactCase> redactCases = {
        {QStringLiteral("empty_text"), QStringLiteral(""), {QStringLiteral("sk-test-1")}},
        {QStringLiteral("empty_values"), QStringLiteral("hello sk-test-1"), {}},
        {QStringLiteral("overlapping"),
         QStringLiteral("using sk-test-short-extra and sk-test-short inside output"),
         {QStringLiteral("sk-test-short"), QStringLiteral("sk-test-short-extra"),
          QStringLiteral(""), QStringLiteral("sk-test-short")}},
        {QStringLiteral("utf8_bytes"),
         QStringLiteral("café sk-test-αβγ end"),
         {QStringLiteral("sk-test-αβγ")}},
    };
    for (const auto &c : redactCases) {
        QJsonArray vals;
        for (const QString &v : c.values)
            vals.append(v);
        out += jsonLine(QStringLiteral("Backend::redactSecrets"), c.label,
                        QJsonObject{{QStringLiteral("text"), c.text},
                                    {QStringLiteral("values"), vals}},
                        Backend::redactSecrets(c.text, c.values));
        out += jsonLine(QStringLiteral("Backend::redactSecretsBytes"), c.label,
                        QJsonObject{{QStringLiteral("text"), c.text},
                                    {QStringLiteral("values"), vals}},
                        QString::fromUtf8(Backend::redactSecretsBytes(c.text.toUtf8(), c.values)));
    }

    const QString workDir = QStringLiteral("/tmp/aibox-golden-secrets");
    QDir(workDir).removeRecursively();
    QDir().mkpath(workDir);
    {
        const QString storePath = workDir + QStringLiteral("/cfg/secrets.json");
        const QList<QPair<QString, QString>> initial = {
            {QStringLiteral("API_KEY"), QStringLiteral("sk-test-0001")},
            {QStringLiteral("API_KEY"), QStringLiteral("sk-test-dup")},
            {QStringLiteral("SECOND_KEY"), QStringLiteral("sk-test-0002")},
        };
        const QStringList disabled = {QStringLiteral("SECOND_KEY"), QStringLiteral("bad-name")};
        const QStringList hidden = {QStringLiteral("API_KEY"), QStringLiteral("API_KEY")};
        const QString saveErr = Backend::saveSecretsStore(storePath, initial, disabled, hidden);

        QFile rawFile(storePath);
        QString rawJson;
        if (rawFile.open(QIODevice::ReadOnly))
            rawJson = QString::fromUtf8(rawFile.readAll());

        QString loadErr;
        const auto loaded = Backend::loadSecretsStore(storePath, &loadErr);
        QJsonArray loadedArr;
        for (const auto &p : loaded) {
            loadedArr.append(QJsonObject{{QStringLiteral("name"), p.first},
                                         {QStringLiteral("value"), p.second}});
        }
        QJsonArray disabledLoaded;
        for (const QString &n : Backend::loadSecretNameList(storePath, QStringLiteral("disabled")))
            disabledLoaded.append(n);
        QJsonArray hiddenLoaded;
        for (const QString &n :
             Backend::loadSecretNameList(storePath, QStringLiteral("hiddenInInstances")))
            hiddenLoaded.append(n);

        out += jsonLine(QStringLiteral("Backend::secretsStoreRoundTrip"), QStringLiteral("full"),
                        QJsonObject{},
                        QJsonObject{
                            {QStringLiteral("saveErr"), saveErr},
                            {QStringLiteral("rawJson"), rawJson},
                            {QStringLiteral("loadErr"), loadErr},
                            {QStringLiteral("loaded"), loadedArr},
                            {QStringLiteral("disabled"), disabledLoaded},
                            {QStringLiteral("hiddenInInstances"), hiddenLoaded},
                        });

        const QString badNameErr =
            Backend::saveSecretsStore(storePath, {{QStringLiteral("bad-name"), QStringLiteral("x")}});
        out += jsonLine(QStringLiteral("Backend::saveSecretsStore"), QStringLiteral("invalid_name"),
                        QJsonObject{}, badNameErr);

        const QString badJsonPath = workDir + QStringLiteral("/bad.json");
        {
            QFile f(badJsonPath);
            if (f.open(QIODevice::WriteOnly | QIODevice::Truncate))
                f.write("{not valid");
            f.setPermissions(QFileDevice::ReadOwner | QFileDevice::WriteOwner);
        }
        QString badJsonErr;
        const auto badLoaded = Backend::loadSecretsStore(badJsonPath, &badJsonErr);
        out += jsonLine(QStringLiteral("Backend::loadSecretsStore"), QStringLiteral("invalid_json"),
                        QJsonObject{},
                        QJsonObject{{QStringLiteral("count"), badLoaded.size()},
                                    {QStringLiteral("hasErr"), !badJsonErr.isEmpty()}});
    }
    QDir(workDir).removeRecursively();

    return out;
}

static QByteArray captureSession() {
    QByteArray out;

    struct EffectiveCase {
        QString label;
        bool globalEnabled;
        QString envOverride;
    };
    const QList<EffectiveCase> effCases = {
        {QStringLiteral("global_true_empty"), true, QStringLiteral("")},
        {QStringLiteral("global_true_inherit"), true, QStringLiteral("inherit")},
        {QStringLiteral("global_true_false"), true, QStringLiteral("false")},
        {QStringLiteral("global_true_zero"), true, QStringLiteral(" 0 ")},
        {QStringLiteral("global_false_empty"), false, QStringLiteral("")},
        {QStringLiteral("global_false_true"), false, QStringLiteral("true")},
        {QStringLiteral("global_false_one"), false, QStringLiteral(" 1 ")},
        {QStringLiteral("global_false_mixed"), false, QStringLiteral(" TRUE ")},
    };
    for (const auto &c : effCases) {
        out += jsonLine(QStringLiteral("Backend::effectiveRecordSessions"), c.label,
                        QJsonObject{{QStringLiteral("globalEnabled"), c.globalEnabled},
                                    {QStringLiteral("envOverride"), c.envOverride}},
                        Backend::effectiveRecordSessions(c.globalEnabled, c.envOverride));
    }

    struct WrapCase {
        QString label;
        QString inner;
        QString logFile;
        QString timingFile;
        QString redactBin;
        QString sessionsDir;
        bool scriptAvailable;
    };
    const QList<WrapCase> wrapCases = {
        {QStringLiteral("full_with_quotes"),
         QStringLiteral("incus exec 'my-box' --cwd /workspace -- bash -l"),
         QStringLiteral("/tmp/aibox-golden-home/dir with 'quote'/sessions/my-box/1700000001.log"),
         QStringLiteral("/tmp/aibox-golden-home/dir with 'quote'/sessions/my-box/1700000001.timing"),
         QStringLiteral("/usr/bin/aibox"),
         QStringLiteral("/tmp/aibox-golden-home/sessions"),
         true},
        {QStringLiteral("no_redact_bin"),
         QStringLiteral("bash -l"),
         QStringLiteral("/tmp/aibox-golden-home/sessions/box/1.log"),
         QStringLiteral("/tmp/aibox-golden-home/sessions/box/1.timing"),
         QStringLiteral(""),
         QStringLiteral(""),
         true},
        {QStringLiteral("no_sessions_dir"),
         QStringLiteral("bash -l"),
         QStringLiteral("/tmp/aibox-golden-home/sessions/box/1.log"),
         QStringLiteral("/tmp/aibox-golden-home/sessions/box/1.timing"),
         QStringLiteral("/usr/bin/aibox"),
         QStringLiteral(""),
         true},
        {QStringLiteral("script_unavailable"),
         QStringLiteral("incus exec 'my-box' -- bash -l"),
         QStringLiteral("/tmp/aibox-golden-home/sessions/box/1.log"),
         QStringLiteral("/tmp/aibox-golden-home/sessions/box/1.timing"),
         QStringLiteral("/usr/bin/aibox"),
         QStringLiteral("/tmp/aibox-golden-home/sessions"),
         false},
        {QStringLiteral("empty_inner"),
         QStringLiteral("   "),
         QStringLiteral("/tmp/aibox-golden-home/sessions/box/1.log"),
         QStringLiteral("/tmp/aibox-golden-home/sessions/box/1.timing"),
         QStringLiteral("/usr/bin/aibox"),
         QStringLiteral("/tmp/aibox-golden-home/sessions"),
         true},
    };
    for (const auto &c : wrapCases) {
        out += jsonLine(QStringLiteral("Backend::buildSessionWrapCommand"), c.label,
                        QJsonObject{{QStringLiteral("inner"), c.inner},
                                    {QStringLiteral("logFile"), c.logFile},
                                    {QStringLiteral("timingFile"), c.timingFile},
                                    {QStringLiteral("redactBin"), c.redactBin},
                                    {QStringLiteral("sessionsDir"), c.sessionsDir},
                                    {QStringLiteral("scriptAvailable"), c.scriptAvailable}},
                        Backend::buildSessionWrapCommand(c.inner, c.logFile, c.timingFile,
                                                         c.redactBin, c.sessionsDir, c.scriptAvailable));
    }

    const QList<QPair<QString, QString>> ansiCases = {
        {QStringLiteral("empty"), QStringLiteral("")},
        {QStringLiteral("osc_csi_crlf"),
         QStringLiteral("\x1b]0;user@dev:/workspace\x07"
                        "\x1b[32muser@dev\x1b[0m:\x1b[34m/workspace\x1b[0m$ echo ok\r\n"
                        "\x1b[2J\x1b[1;1Hok\r\n")},
        {QStringLiteral("osc_st_and_esc2"),
         QStringLiteral("\x1b]2;title\x1b\\\x1b(B\x1b=hello\x1bMworld")},
    };
    for (const auto &c : ansiCases) {
        out += jsonLine(QStringLiteral("Backend::stripAnsiSequences"), c.first,
                        QJsonObject{{QStringLiteral("rawText"), c.second}},
                        Backend::stripAnsiSequences(c.second));
    }

    {
        out += jsonLine(QStringLiteral("Backend::formatRecordedSessionText"), QStringLiteral("empty"),
                        QJsonObject{{QStringLiteral("raw"), QStringLiteral("")},
                                    {QStringLiteral("maxBytes"), 204800}},
                        Backend::formatRecordedSessionText(QByteArray(), 204800));
        const QString shortRaw = QStringLiteral("\x1b[32mok\x1b[0m\r\n");
        out += jsonLine(QStringLiteral("Backend::formatRecordedSessionText"), QStringLiteral("short"),
                        QJsonObject{{QStringLiteral("raw"), shortRaw},
                                    {QStringLiteral("maxBytes"), 204800}},
                        Backend::formatRecordedSessionText(shortRaw.toUtf8(), 204800));
        QByteArray big;
        for (int i = 0; i < 120; ++i)
            big += QByteArray("line-") + QByteArray::number(i) + "-0123456789abcdef\n";
        out += jsonLine(QStringLiteral("Backend::formatRecordedSessionText"), QStringLiteral("truncated_min_cap"),
                        QJsonObject{{QStringLiteral("raw"), QString::fromUtf8(big)},
                                    {QStringLiteral("maxBytes"), 512}},
                        Backend::formatRecordedSessionText(big, 512));
    }

    const QString workDir = QStringLiteral("/tmp/aibox-golden-session");
    QDir(workDir).removeRecursively();
    QDir().mkpath(workDir + QStringLiteral("/alpha"));
    QDir().mkpath(workDir + QStringLiteral("/beta"));
    {
        const QString redPath = workDir + QStringLiteral("/alpha/1700000099.log");
        {
            QFile f(redPath);
            if (f.open(QIODevice::WriteOnly | QIODevice::Truncate))
                f.write("token=sk-test-12345\nother=safe\n");
        }
        const bool changed = Backend::redactSessionLogFile(redPath, {QStringLiteral("sk-test-12345")});
        const bool unchanged = Backend::redactSessionLogFile(redPath, {QStringLiteral("sk-test-12345")});
        QString after;
        {
            QFile f(redPath);
            if (f.open(QIODevice::ReadOnly))
                after = QString::fromUtf8(f.readAll());
        }
        out += jsonLine(QStringLiteral("Backend::redactSessionLogFile"), QStringLiteral("basic"),
                        QJsonObject{},
                        QJsonObject{{QStringLiteral("changed"), changed},
                                    {QStringLiteral("unchanged"), unchanged},
                                    {QStringLiteral("after"), after}});
        QFile::remove(redPath);

        const QDateTime baseDt(QDate(2026, 11, 1), QTime(0, 0, 0), QTimeZone::UTC);
        for (int i = 1; i <= 5; ++i) {
            const QString epochStr = QString::number(1700000000 + i);
            const QString lp = workDir + QStringLiteral("/alpha/") + epochStr + QStringLiteral(".log");
            const QString tp = workDir + QStringLiteral("/alpha/") + epochStr + QStringLiteral(".timing");
            QFile lf(lp);
            if (lf.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
                lf.write(QByteArray(100, 'L'));
                lf.close();
            }
            if (lf.open(QIODevice::ReadWrite))
                lf.setFileTime(baseDt.addSecs(i * 60), QFileDevice::FileModificationTime);
            if (i % 2 == 1) {
                QFile tf(tp);
                if (tf.open(QIODevice::WriteOnly | QIODevice::Truncate))
                    tf.write(QByteArray(20, 'T'));
            }
        }
        const int pruned = Backend::pruneRecordedSessions(workDir, 3, 1024 * 1024);
        const QVariantList scanned = Backend::scanRecordedSessions(workDir);
        QJsonArray scannedArr;
        for (const QVariant &v : scanned)
            scannedArr.append(QJsonObject::fromVariantMap(v.toMap()));
        out += jsonLine(QStringLiteral("Backend::pruneAndScanRecordedSessions"), QStringLiteral("basic"),
                        QJsonObject{},
                        QJsonObject{{QStringLiteral("pruned"), pruned},
                                    {QStringLiteral("scanned"), scannedArr}});
    }
    QDir(workDir).removeRecursively();

    return out;
}

static QByteArray captureSnapshot() {
    QByteArray out;

    for (const QString &name : {QStringLiteral("auto-20261113-100000"),
                                QStringLiteral("auto-"),
                                QStringLiteral("snap-manual"),
                                QStringLiteral(""),
                                QStringLiteral("AUTO-20261113-100000")}) {
        out += jsonLine(QStringLiteral("Backend::isAutoSnapshotName"), name,
                        QJsonObject{{QStringLiteral("snapshotName"), name}},
                        Backend::isAutoSnapshotName(name));
    }

    for (const QString &name : {QStringLiteral("auto-20261113-100000"),
                                QStringLiteral("auto-invalid"),
                                QStringLiteral("snap-1"),
                                QStringLiteral("auto-20260230-000000")}) {
        const QDateTime dt = Backend::parseAutoSnapshotTimestamp(name);
        out += jsonLine(QStringLiteral("Backend::parseAutoSnapshotTimestamp"), name,
                        QJsonObject{{QStringLiteral("snapshotName"), name}},
                        QJsonObject{{QStringLiteral("valid"), dt.isValid()},
                                    {QStringLiteral("iso"), dt.isValid() ? dt.toString(Qt::ISODate) : QString()}});
    }

    const QStringList baseSnaps = {
        QStringLiteral("manual-initial"),
        QStringLiteral("auto-20261113-100000"),
        QStringLiteral("auto-20261113-101000"),
        QStringLiteral("before-refactor"),
        QStringLiteral("auto-20261113-102000"),
        QStringLiteral("auto-20261113-103000"),
    };
    struct ShouldCase {
        const char *label;
        QStringList snaps;
        QString nowIso;
        int minInterval;
    };
    const QList<ShouldCase> shouldCases = {
        {"too_soon", baseSnaps, QStringLiteral("2026-11-13T10:32:00"), 300},
        {"exact_boundary", baseSnaps, QStringLiteral("2026-11-13T10:35:00"), 300},
        {"manual_only", {QStringLiteral("manual-only")}, QStringLiteral("2026-11-13T10:32:00"), 300},
        {"empty", {}, QStringLiteral("2026-11-13T10:32:00"), 300},
        {"negative_interval", baseSnaps, QStringLiteral("2026-11-13T10:30:00"), -5},
    };
    for (const auto &c : shouldCases) {
        const QDateTime nowDt = QDateTime::fromString(c.nowIso, Qt::ISODate);
        out += jsonLine(QStringLiteral("Backend::shouldCreateAutoSnapshot"), QString::fromLatin1(c.label),
                        QJsonObject{{QStringLiteral("snapshots"), QJsonArray::fromStringList(c.snaps)},
                                    {QStringLiteral("nowIso"), c.nowIso},
                                    {QStringLiteral("minIntervalSeconds"), c.minInterval}},
                        Backend::shouldCreateAutoSnapshot(c.snaps, nowDt, c.minInterval));
    }

    struct PruneCase {
        const char *label;
        QStringList snaps;
        int keepCount;
    };
    const QList<PruneCase> pruneCases = {
        {"keep_2", baseSnaps, 2},
        {"keep_0", baseSnaps, 0},
        {"keep_neg", baseSnaps, -1},
        {"keep_10", baseSnaps, 10},
        {"invalid_tiebreak",
         {QStringLiteral("auto-bad-b"), QStringLiteral("auto-bad-a"), QStringLiteral("auto-20261113-100000")},
         1},
    };
    for (const auto &c : pruneCases) {
        out += jsonLine(QStringLiteral("Backend::autoSnapshotsToPrune"), QString::fromLatin1(c.label),
                        QJsonObject{{QStringLiteral("snapshots"), QJsonArray::fromStringList(c.snaps)},
                                    {QStringLiteral("keepCount"), c.keepCount}},
                        QJsonArray::fromStringList(Backend::autoSnapshotsToPrune(c.snaps, c.keepCount)));
    }

    for (const auto &pair : QList<QPair<QString, QString>>{
             {QStringLiteral("/tmp/workspaces/dev box"), QStringLiteral("dev-box")},
             {QStringLiteral("/tmp/workspaces/dev-box/"), QStringLiteral("dev-box")},
             {QStringLiteral(""), QStringLiteral("dev-box")},
             {QStringLiteral("/tmp/workspaces/dev-box"), QStringLiteral("")},
         }) {
        out += jsonLine(QStringLiteral("Backend::workspaceSnapshotDir"),
                        pair.first + QLatin1Char('|') + pair.second,
                        QJsonObject{{QStringLiteral("workspacePath"), pair.first},
                                    {QStringLiteral("envName"), pair.second}},
                        Backend::workspaceSnapshotDir(pair.first, pair.second));
    }

    out += jsonLine(QStringLiteral("Backend::findOrphanWorkspaceSnapshots"), QStringLiteral("mixed"),
                    QJsonObject{{QStringLiteral("incusSnapshots"),
                                 QJsonArray::fromStringList({QStringLiteral("snap-1"), QStringLiteral("auto-1")})},
                                {QStringLiteral("wsSnapshots"),
                                 QJsonArray::fromStringList({QStringLiteral("auto-1"), QStringLiteral("pre-restore-1"), QStringLiteral("snap-1")})}},
                    QJsonArray::fromStringList(Backend::findOrphanWorkspaceSnapshots(
                        {QStringLiteral("snap-1"), QStringLiteral("auto-1")},
                        {QStringLiteral("auto-1"), QStringLiteral("pre-restore-1"), QStringLiteral("snap-1")})));

    for (const auto &c : QList<QPair<QString, QByteArray>>{
             {QStringLiteral("valid"),
              R"json([{"name":"clean-sdk","size":52428800},{"name":"zero","size":0},{"name":"empty-unknown","size":-1},{"name":"","size":1024}])json"},
             {QStringLiteral("empty"), QByteArray("")},
             {QStringLiteral("invalid"), QByteArray("{not json")},
             {QStringLiteral("object"), QByteArray(R"json({"name":"a","size":10})json")},
         }) {
        out += jsonLine(QStringLiteral("Backend::parseSnapshotSizesJson"), c.first,
                        QJsonObject{{QStringLiteral("json"), QString::fromUtf8(c.second)}},
                        QJsonObject::fromVariantMap(Backend::parseSnapshotSizesJson(c.second)));
    }

    const QString duSample = QStringLiteral(
        "1048576\t/tmp/ws/.snapshots/dev-box/snap-1\n"
        "2048\t/tmp/ws/.snapshots/dev-box/pre-restore-123\n"
        "0\t/tmp/ws/.snapshots/dev-box/empty\n"
        "-1\t/tmp/ws/.snapshots/dev-box/neg\n"
        "badline\n");
    out += jsonLine(QStringLiteral("Backend::parseDuBytesOutput"), QStringLiteral("sample"),
                    QJsonObject{{QStringLiteral("output"), duSample}},
                    QJsonObject::fromVariantMap(Backend::parseDuBytesOutput(duSample)));

    for (const QString &ws : {QStringLiteral("/tmp/my workspaces/dev box"), QStringLiteral("/tmp/ws/o'reilly")}) {
        out += jsonLine(QStringLiteral("Backend::workspaceCreateSnippet"), ws,
                        QJsonObject{{QStringLiteral("workspacePath"), ws}},
                        Backend::workspaceCreateSnippet(ws));
    }

    for (const QString &env : {QStringLiteral("dev-box"), QStringLiteral("box'1")}) {
        out += jsonLine(QStringLiteral("Backend::workspaceMountMigrationScript"), env,
                        QJsonObject{{QStringLiteral("envName"), env}},
                        Backend::workspaceMountMigrationScript(env));
    }

    for (const auto &c : QList<QStringList>{
             {QStringLiteral("/tmp/ws/dev-box"), QStringLiteral("/tmp/ws/.snapshots/dev-box"), QStringLiteral("snap-1")},
             {QStringLiteral("/tmp/ws/o'reilly"), QStringLiteral("/tmp/ws/.snapshots/o'reilly"), QStringLiteral("snap'1")},
             {QStringLiteral(""), QStringLiteral("/tmp/ws/.snapshots/dev-box"), QStringLiteral("snap-1")},
         }) {
        out += jsonLine(QStringLiteral("Backend::workspaceSnapshotSnippet"), c.join(QLatin1Char('|')),
                        QJsonObject{{QStringLiteral("workspacePath"), c.at(0)},
                                    {QStringLiteral("snapDir"), c.at(1)},
                                    {QStringLiteral("snapName"), c.at(2)}},
                        Backend::workspaceSnapshotSnippet(c.at(0), c.at(1), c.at(2)));
    }

    for (const bool warnOnly : {true, false}) {
        for (const QString &p : {QStringLiteral("/tmp/ws/.snapshots/dev-box/snap-1"),
                                 QStringLiteral("/tmp/ws/.snapshots/o'reilly/snap'1"),
                                 QStringLiteral("")}) {
            out += jsonLine(QStringLiteral("Backend::workspaceDeleteSnapshotSnippet"),
                            (warnOnly ? QStringLiteral("warn|") : QStringLiteral("fail|")) + p,
                            QJsonObject{{QStringLiteral("snapPath"), p},
                                        {QStringLiteral("warnOnly"), warnOnly}},
                            Backend::workspaceDeleteSnapshotSnippet(p, warnOnly));
        }
    }

    for (const qint64 epoch : {qint64(0), qint64(1700000123)}) {
        out += jsonLine(QStringLiteral("Backend::workspaceRestoreSnippet"),
                        QStringLiteral("epoch_%1").arg(epoch),
                        QJsonObject{{QStringLiteral("workspacePath"), QStringLiteral("/tmp/ws/o'reilly")},
                                    {QStringLiteral("snapDir"), QStringLiteral("/tmp/ws/.snapshots/o'reilly")},
                                    {QStringLiteral("snapName"), QStringLiteral("snap'1")},
                                    {QStringLiteral("safetyEpoch"), static_cast<double>(epoch)}},
                        Backend::workspaceRestoreSnippet(QStringLiteral("/tmp/ws/o'reilly"),
                                                         QStringLiteral("/tmp/ws/.snapshots/o'reilly"),
                                                         QStringLiteral("snap'1"), epoch));
    }

    out += jsonLine(QStringLiteral("Backend::convertWorkspaceToSubvolumeScript"), QStringLiteral("quoted"),
                    QJsonObject{{QStringLiteral("envName"), QStringLiteral("box'1")},
                                {QStringLiteral("workspacePath"), QStringLiteral("/tmp/ws/o'reilly")}},
                    Backend::convertWorkspaceToSubvolumeScript(QStringLiteral("box'1"),
                                                               QStringLiteral("/tmp/ws/o'reilly")));

    out += jsonLine(QStringLiteral("Backend::autoSnapshotShellSnippet"), QStringLiteral("no_ws_no_prune"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("dev-box")},
                                {QStringLiteral("newSnapName"), QStringLiteral("auto-20261113-120000")},
                                {QStringLiteral("pruneNames"), QJsonArray()},
                                {QStringLiteral("workspacePath"), QStringLiteral("")},
                                {QStringLiteral("includeWorkspace"), false}},
                    Backend::autoSnapshotShellSnippet(QStringLiteral("dev-box"),
                                                      QStringLiteral("auto-20261113-120000"), {}));
    out += jsonLine(QStringLiteral("Backend::autoSnapshotShellSnippet"), QStringLiteral("with_ws_and_prune"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("box'1")},
                                {QStringLiteral("newSnapName"), QStringLiteral("auto-20261113-120000")},
                                {QStringLiteral("pruneNames"),
                                 QJsonArray::fromStringList({QStringLiteral("auto-20261113-090000"), QStringLiteral("auto'old")})},
                                {QStringLiteral("workspacePath"), QStringLiteral("/tmp/my workspaces/box'1")},
                                {QStringLiteral("includeWorkspace"), true}},
                    Backend::autoSnapshotShellSnippet(QStringLiteral("box'1"),
                                                      QStringLiteral("auto-20261113-120000"),
                                                      {QStringLiteral("auto-20261113-090000"), QStringLiteral("auto'old")},
                                                      QStringLiteral("/tmp/my workspaces/box'1"),
                                                      true));

    for (const bool includeWs : {false, true}) {
        out += jsonLine(QStringLiteral("Backend::createSnapshotScript"),
                        includeWs ? QStringLiteral("with_ws") : QStringLiteral("no_ws"),
                        QJsonObject{{QStringLiteral("name"), QStringLiteral("box'1")},
                                    {QStringLiteral("snapName"), QStringLiteral("snap'1")},
                                    {QStringLiteral("workspacePath"), QStringLiteral("/tmp/ws/box'1")},
                                    {QStringLiteral("includeWorkspace"), includeWs}},
                        Backend::createSnapshotScript(QStringLiteral("box'1"), QStringLiteral("snap'1"),
                                                      QStringLiteral("/tmp/ws/box'1"), includeWs));
    }

    struct RestoreCase {
        const char *label;
        QString name;
        QString snap;
        QString ws;
        QString mode;
        bool includeWs;
        qint64 epoch;
    };
    const QList<RestoreCase> restoreCases = {
        {"both_with_ws_epoch", QStringLiteral("box'1"), QStringLiteral("snap'1"), QStringLiteral("/tmp/ws/box'1"), QStringLiteral("both"), true, 1700000123},
        {"both_no_ws", QStringLiteral("dev-box"), QStringLiteral("snap-1"), QStringLiteral("/tmp/ws/dev-box"), QStringLiteral("both"), false, 0},
        {"system_only", QStringLiteral("dev-box"), QStringLiteral("snap-1"), QStringLiteral("/tmp/ws/dev-box"), QStringLiteral("system"), true, 1700000123},
        {"workspace_only", QStringLiteral("dev-box"), QStringLiteral("snap-1"), QStringLiteral("/tmp/ws/dev-box"), QStringLiteral("workspace"), false, 1700000123},
        {"workspace_empty_ws", QStringLiteral("dev-box"), QStringLiteral("snap-1"), QStringLiteral(""), QStringLiteral("workspace"), true, 0},
        {"mixed_case_both", QStringLiteral("dev-box"), QStringLiteral("snap-1"), QStringLiteral("/tmp/ws/dev-box"), QStringLiteral("  BOTH  "), true, 0},
    };
    for (const auto &c : restoreCases) {
        out += jsonLine(QStringLiteral("Backend::restoreSnapshotScript"), QString::fromLatin1(c.label),
                        QJsonObject{{QStringLiteral("name"), c.name},
                                    {QStringLiteral("snapName"), c.snap},
                                    {QStringLiteral("workspacePath"), c.ws},
                                    {QStringLiteral("mode"), c.mode},
                                    {QStringLiteral("includeWorkspace"), c.includeWs},
                                    {QStringLiteral("safetyEpoch"), static_cast<double>(c.epoch)}},
                        Backend::restoreSnapshotScript(c.name, c.snap, c.ws, c.mode, c.includeWs, c.epoch));
    }

    for (const bool hasIncus : {true, false}) {
        for (const bool hasWs : {true, false}) {
            out += jsonLine(QStringLiteral("Backend::deleteSnapshotScript"),
                            QStringLiteral("incus_%1_ws_%2").arg(hasIncus ? 1 : 0).arg(hasWs ? 1 : 0),
                            QJsonObject{{QStringLiteral("name"), QStringLiteral("box'1")},
                                        {QStringLiteral("snapName"), QStringLiteral("snap'1")},
                                        {QStringLiteral("workspacePath"), QStringLiteral("/tmp/ws/box'1")},
                                        {QStringLiteral("hasIncusSnap"), hasIncus},
                                        {QStringLiteral("hasWorkspaceSnap"), hasWs}},
                            Backend::deleteSnapshotScript(QStringLiteral("box'1"), QStringLiteral("snap'1"),
                                                          QStringLiteral("/tmp/ws/box'1"), hasIncus, hasWs));
        }
    }

    out += jsonLine(QStringLiteral("Backend::wipeEnvironmentScript"), QStringLiteral("container_only"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("dev-box")},
                                {QStringLiteral("workspacePath"), QStringLiteral("/tmp/ws/dev-box")},
                                {QStringLiteral("deleteWorkspaceFiles"), false},
                                {QStringLiteral("sessionsEnvDir"), QStringLiteral("")},
                                {QStringLiteral("environmentSecretsPath"), QStringLiteral("")}},
                    Backend::wipeEnvironmentScript(QStringLiteral("dev-box"), QStringLiteral("/tmp/ws/dev-box"), false));
    out += jsonLine(QStringLiteral("Backend::wipeEnvironmentScript"), QStringLiteral("full_quoted"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("box'1")},
                                {QStringLiteral("workspacePath"), QStringLiteral("/tmp/ws/box'1")},
                                {QStringLiteral("deleteWorkspaceFiles"), true},
                                {QStringLiteral("sessionsEnvDir"), QStringLiteral("/tmp/sessions/box'1")},
                                {QStringLiteral("environmentSecretsPath"), QStringLiteral("/tmp/secrets/box'1.json")}},
                    Backend::wipeEnvironmentScript(QStringLiteral("box'1"), QStringLiteral("/tmp/ws/box'1"), true,
                                                   QStringLiteral("/tmp/sessions/box'1"),
                                                   QStringLiteral("/tmp/secrets/box'1.json")));

    return out;
}

static QByteArray captureDiff() {
    QByteArray out;

    out += jsonLine(QStringLiteral("Backend::defaultDiffExcludes"), QStringLiteral("default"),
                    QJsonObject{}, QJsonArray::fromStringList(Backend::defaultDiffExcludes()));

    const QList<QPair<const char *, QString>> parseCases = {
        {"empty", QStringLiteral("")},
        {"mixed_seps_dedup", QStringLiteral(" .git/ , dist/ ; *.log\r\ndist/\n  \nbuild/ ")},
    };
    for (const auto &c : parseCases) {
        out += jsonLine(QStringLiteral("Backend::parseDiffExcludes"), QString::fromLatin1(c.first),
                        QJsonObject{{QStringLiteral("rawText"), c.second}},
                        QJsonArray::fromStringList(Backend::parseDiffExcludes(c.second)));
    }

    const QStringList excludeInput = {QStringLiteral(".git/"), QStringLiteral(" "), QStringLiteral("  build/  ")};
    out += jsonLine(QStringLiteral("Backend::buildRsyncExcludeArgs"), QStringLiteral("trimmed"),
                    QJsonObject{{QStringLiteral("excludes"), QJsonArray::fromStringList(excludeInput)}},
                    QJsonArray::fromStringList(Backend::buildRsyncExcludeArgs(excludeInput)));

    out += jsonLine(QStringLiteral("Backend::buildWorkspaceDiffArgs"), QStringLiteral("cleaned_paths"),
                    QJsonObject{{QStringLiteral("snapPath"), QStringLiteral("/tmp/snaps/../snap1/")},
                                {QStringLiteral("wsPath"), QStringLiteral("/tmp/ws/box1")},
                                {QStringLiteral("excludes"), QJsonArray::fromStringList({QStringLiteral(".git/"), QStringLiteral("build/")})}},
                    QJsonArray::fromStringList(Backend::buildWorkspaceDiffArgs(
                        QStringLiteral("/tmp/snaps/../snap1/"), QStringLiteral("/tmp/ws/box1"),
                        {QStringLiteral(".git/"), QStringLiteral("build/")})));

    const QList<QPair<const char *, QString>> pathCases = {
        {"valid_nested", QStringLiteral("src/main.cpp")},
        {"valid_dots_in_name", QStringLiteral(".../.hidden/a..b.txt")},
        {"empty", QStringLiteral("")},
        {"abs_slash", QStringLiteral("/etc/passwd")},
        {"parent_start", QStringLiteral("../secret.txt")},
        {"parent_mid", QStringLiteral("a/../b")},
        {"dot_start", QStringLiteral("./a")},
        {"dot_mid", QStringLiteral("a/./b")},
        {"only_slashes", QStringLiteral("///")},
        {"double_slash_valid", QStringLiteral("a//b")},
        {"nul_byte", QString::fromLatin1("a\0b", 3)},
    };
    for (const auto &c : pathCases) {
        out += jsonLine(QStringLiteral("Backend::isSafeRelativePath"), QString::fromLatin1(c.first),
                        QJsonObject{{QStringLiteral("relativePath"), c.second}},
                        Backend::isSafeRelativePath(c.second));
    }

    const QByteArray nulAt8191 = QByteArray(8191, 'a') + QByteArray("\0", 1);
    const QByteArray nulAt8192 = QByteArray(8192, 'a') + QByteArray("\0", 1);
    const QList<QPair<const char *, QByteArray>> binCases = {
        {"empty", QByteArray()},
        {"ascii", QByteArrayLiteral("hello\nworld\n")},
        {"utf8", QByteArrayLiteral("héllo 🌍\n")},
        {"elf_nul", QByteArray("ELF\x00\x01\x02", 6)},
        {"nul_at_8191", nulAt8191},
        {"nul_at_8192", nulAt8192},
    };
    for (const auto &c : binCases) {
        out += jsonLine(QStringLiteral("Backend::isLikelyBinaryData"), QString::fromLatin1(c.first),
                        QJsonObject{{QStringLiteral("sampleHex"), QString::fromLatin1(c.second.toHex())}},
                        Backend::isLikelyBinaryData(c.second));
    }

    const QList<std::tuple<const char *, QString, int>> capCases = {
        {"under_cap", QStringLiteral("a\nb\nc"), 5},
        {"exact_cap", QStringLiteral("a\nb\nc"), 3},
        {"over_cap", QStringLiteral("a\nb\nc\nd"), 2},
        {"trailing_newline_exact", QStringLiteral("a\nb\n"), 3},
        {"trailing_newline_over", QStringLiteral("a\nb\n"), 2},
        {"zero_clamped", QStringLiteral("a\nb"), 0},
    };
    for (const auto &c : capCases) {
        bool truncated = false;
        const QString capped = Backend::capDiffLines(std::get<1>(c), std::get<2>(c), &truncated);
        out += jsonLine(QStringLiteral("Backend::capDiffLines"), QString::fromLatin1(std::get<0>(c)),
                        QJsonObject{{QStringLiteral("diffOutput"), std::get<1>(c)},
                                    {QStringLiteral("maxLines"), std::get<2>(c)}},
                        QJsonObject{{QStringLiteral("text"), capped},
                                    {QStringLiteral("truncated"), truncated}});
    }

    const QString itemizeFixture = QStringLiteral(
        ".d..t...... 40 ./\r\n"
        ".d..t...... 40 sub dir/\r\n"
        "*deleting   0 added file with spaces.txt\n"
        "*deleting   0 new dir/\n"
        ">f.s....... 42 src/main file.cpp\n"
        ">f+++++++++ 128 removed.txt\n"
        "cd+++++++++ 0 old dir/\n"
        "cL+++++++++ 12 deleted_link\n"
        "cLc........ 8 changed_link\n"
        "unmatched line\n");
    for (const int maxEntries : {5000, 3, 0}) {
        out += jsonLine(QStringLiteral("Backend::parseRsyncItemize"),
                        QStringLiteral("cap_%1").arg(maxEntries),
                        QJsonObject{{QStringLiteral("output"), itemizeFixture},
                                    {QStringLiteral("maxEntries"), maxEntries}},
                        QJsonValue::fromVariant(Backend::parseRsyncItemize(itemizeFixture, maxEntries)));
    }

    // processWorkspaceDiffOutput with real workspace files for added size/symlink enrichment
    const QString wsWork = QStringLiteral("/tmp/aibox-golden-diff-ws");
    QDir(wsWork).removeRecursively();
    QDir().mkpath(wsWork + QStringLiteral("/new dir"));
    {
        QFile f(wsWork + QStringLiteral("/added file.txt"));
        if (f.open(QIODevice::WriteOnly))
            f.write("hello world");
    }
    QFile::link(QStringLiteral("added file.txt"), wsWork + QStringLiteral("/added_link"));

    const QString procFixture = QStringLiteral(
        "*deleting   0 added file.txt\n"
        "*deleting   0 added_link\n"
        "*deleting   0 new dir/\n"
        "*deleting   0 missing_added.txt\n"
        ">f.s....... 42 modified.txt\n"
        ">f+++++++++ 10 deleted.txt\n");
    QVariantList procEntries;
    const QVariantMap procSummary = Backend::processWorkspaceDiffOutput(
        QStringLiteral("snap-1"), wsWork, procFixture, &procEntries);
    out += jsonLine(QStringLiteral("Backend::processWorkspaceDiffOutput"), QStringLiteral("enriched"),
                    QJsonObject{{QStringLiteral("snapshotName"), QStringLiteral("snap-1")},
                                {QStringLiteral("wsPath"), wsWork},
                                {QStringLiteral("rsyncStdout"), procFixture}},
                    QJsonObject{{QStringLiteral("summary"), QJsonObject::fromVariantMap(procSummary)},
                                {QStringLiteral("entries"), QJsonArray::fromVariantList(procEntries)}});
    QDir(wsWork).removeRecursively();

    return out;
}

static QByteArray captureWorkspace() {
    QByteArray out;

    for (const int port : {2222, 1, 65535, 0, -1, 70000}) {
        out += jsonLine(QStringLiteral("Backend::fileAccessUrl"),
                        QStringLiteral("port_%1").arg(port),
                        QJsonObject{{QStringLiteral("port"), port}},
                        Backend::fileAccessUrl(port));
    }

    {
        const QSet<int> reserved{2222, 2223, 2224};
        int probes = 0;
        const int port = Backend::chooseFileAccessPort(reserved, [&probes](int candidate) {
            ++probes;
            return candidate != 2225;
        });
        out += jsonLine(QStringLiteral("Backend::chooseFileAccessPort"),
                        QStringLiteral("skip_reserved_and_busy"),
                        QJsonObject{{QStringLiteral("reserved"), QJsonArray{2222, 2223, 2224}},
                                    {QStringLiteral("busy"), QJsonArray{2225}},
                                    {QStringLiteral("startPort"), 2222},
                                    {QStringLiteral("maxPort"), 2299}},
                        QJsonObject{{QStringLiteral("port"), port},
                                    {QStringLiteral("probes"), probes}});
    }
    out += jsonLine(QStringLiteral("Backend::chooseFileAccessPort"),
                    QStringLiteral("first_free_no_predicate"),
                    QJsonObject{{QStringLiteral("reserved"), QJsonArray{}},
                                {QStringLiteral("busy"), QJsonArray{}},
                                {QStringLiteral("startPort"), 2222},
                                {QStringLiteral("maxPort"), 2224}},
                    Backend::chooseFileAccessPort(QSet<int>{}, std::function<bool(int)>{}, 2222, 2224));
    out += jsonLine(QStringLiteral("Backend::chooseFileAccessPort"),
                    QStringLiteral("all_reserved"),
                    QJsonObject{{QStringLiteral("reserved"), QJsonArray{2222, 2223, 2224}},
                                {QStringLiteral("busy"), QJsonArray{}},
                                {QStringLiteral("startPort"), 2222},
                                {QStringLiteral("maxPort"), 2224}},
                    Backend::chooseFileAccessPort(QSet<int>{2222, 2223, 2224}, std::function<bool(int)>{}, 2222, 2224));

    {
        const QString faWork = QStringLiteral("/tmp/aibox-golden-fa-state");
        QDir(faWork).removeRecursively();
        Backend backend(faWork);
        out += jsonLine(QStringLiteral("Backend::fileAccessPaths"),
                        QStringLiteral("box_with_spaces"),
                        QJsonObject{{QStringLiteral("stateDir"), faWork},
                                    {QStringLiteral("name"), QStringLiteral("My Box!")}},
                        QJsonObject{{QStringLiteral("dir"), backend.fileAccessDirPath()},
                                    {QStringLiteral("state"), backend.fileAccessStatePath(QStringLiteral("My Box!"))},
                                    {QStringLiteral("log"), backend.fileAccessLogPath(QStringLiteral("My Box!"))}});
        QDir(faWork).removeRecursively();
    }

    const QString wsWork = QStringLiteral("/tmp/aibox-golden-ws-work");
    QDir(wsWork).removeRecursively();
    const QString safeDir = wsWork + QStringLiteral("/datasets");
    const QString sshDir = wsWork + QStringLiteral("/home/.ssh/keys");
    const QString aiboxDir = wsWork + QStringLiteral("/proj/.aibox");
    const QString cfgDir = wsWork + QStringLiteral("/cfg");
    const QString cfgChildDir = wsWork + QStringLiteral("/cfg/sub");
    QDir().mkpath(safeDir);
    QDir().mkpath(sshDir);
    QDir().mkpath(aiboxDir);
    QDir().mkpath(cfgChildDir);

    const QStringList forbidden{cfgDir};
    const QList<QPair<const char *, QString>> mountPathCases = {
        {"empty", QStringLiteral("")},
        {"whitespace", QStringLiteral("   ")},
        {"missing_dir", wsWork + QStringLiteral("/missing-dir")},
        {"root", QStringLiteral("/")},
        {"etc", QStringLiteral("/etc")},
        {"usr_bin", QStringLiteral("/usr/bin")},
        {"home_dir", QDir::homePath()},
        {"tilde", QStringLiteral("~")},
        {"ssh_dir", sshDir},
        {"aibox_dir", aiboxDir},
        {"forbidden_cfg_exact", cfgDir},
        {"forbidden_cfg_child", cfgChildDir},
        {"valid_dir", safeDir},
    };
    for (const auto &c : mountPathCases) {
        out += jsonLine(QStringLiteral("Backend::validateExtraMountPath"),
                        QString::fromLatin1(c.first),
                        QJsonObject{{QStringLiteral("rawPath"), c.second},
                                    {QStringLiteral("forbidden"), QJsonArray::fromStringList(forbidden)}},
                        Backend::validateExtraMountPath(c.second, forbidden));
    }

    for (const auto &c : QList<QPair<const char *, QString>>{
             {"my_data", QStringLiteral("My Data")},
             {"empty", QStringLiteral("")},
             {"special_chars", QStringLiteral("  foo_bar!  ")},
         }) {
        out += jsonLine(QStringLiteral("Backend::extraMountDeviceName"),
                        QString::fromLatin1(c.first),
                        QJsonObject{{QStringLiteral("rawName"), c.second}},
                        Backend::extraMountDeviceName(c.second));
    }

    out += jsonLine(QStringLiteral("Backend::addExtraMountScript"),
                    QStringLiteral("readonly"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("ct1")},
                                {QStringLiteral("deviceName"), QStringLiteral("mnt-data")},
                                {QStringLiteral("hostPath"), safeDir},
                                {QStringLiteral("containerPath"), QStringLiteral("/home/workspace/data")},
                                {QStringLiteral("readOnly"), true}},
                    Backend::addExtraMountScript(QStringLiteral("ct1"), QStringLiteral("mnt-data"),
                                                 safeDir, QStringLiteral("/home/workspace/data"), true));
    out += jsonLine(QStringLiteral("Backend::addExtraMountScript"),
                    QStringLiteral("readwrite_quoted"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("box'1")},
                                {QStringLiteral("deviceName"), QStringLiteral("mnt-my-data")},
                                {QStringLiteral("hostPath"), QStringLiteral("/tmp/my data")},
                                {QStringLiteral("containerPath"), QStringLiteral("/home/workspace/my data")},
                                {QStringLiteral("readOnly"), false}},
                    Backend::addExtraMountScript(QStringLiteral("box'1"), QStringLiteral("mnt-my-data"),
                                                 QStringLiteral("/tmp/my data"),
                                                 QStringLiteral("/home/workspace/my data"), false));

    {
        Backend backend(wsWork + QStringLiteral("/be"));
        out += jsonLine(QStringLiteral("Backend::addExtraMount"),
                        QStringLiteral("unconfirmed_rw"),
                        QJsonObject{{QStringLiteral("name"), QStringLiteral("ct1")},
                                    {QStringLiteral("hostPath"), safeDir},
                                    {QStringLiteral("mountName"), QStringLiteral("data")},
                                    {QStringLiteral("readOnly"), false},
                                    {QStringLiteral("confirmReadWrite"), false}},
                        backend.addExtraMount(QStringLiteral("ct1"), safeDir, QStringLiteral("data"), false, false));
        out += jsonLine(QStringLiteral("Backend::addExtraMount"),
                        QStringLiteral("empty_name"),
                        QJsonObject{{QStringLiteral("name"), QStringLiteral("")},
                                    {QStringLiteral("hostPath"), safeDir},
                                    {QStringLiteral("mountName"), QStringLiteral("data")},
                                    {QStringLiteral("readOnly"), true},
                                    {QStringLiteral("confirmReadWrite"), false}},
                        backend.addExtraMount(QStringLiteral(""), safeDir, QStringLiteral("data"), true, false));
        out += jsonLine(QStringLiteral("Backend::addExtraMount"),
                        QStringLiteral("reserved_mount_name"),
                        QJsonObject{{QStringLiteral("name"), QStringLiteral("ct1")},
                                    {QStringLiteral("hostPath"), safeDir},
                                    {QStringLiteral("mountName"), QStringLiteral("workspace")},
                                    {QStringLiteral("readOnly"), true},
                                    {QStringLiteral("confirmReadWrite"), false}},
                        backend.addExtraMount(QStringLiteral("ct1"), safeDir, QStringLiteral("workspace"), true, false));
    }

    QDir(wsWork).removeRecursively();
    return out;
}

static QByteArray captureNetwork() {
    QByteArray out;

    // 1. parseAllowlistEntries
    const QStringList allowInputs = {
        QStringLiteral(""),
        QStringLiteral("GitHub.com, 1.1.1.1; 10.0.0.0/8\nlocal-rebind.test bad$host github.com"),
        QStringLiteral("  api.example.com ; 192.168.1.0/24 , 8.8.8.8  invalid_host!  sub-1.domain.org "),
        QStringLiteral("2001:db8::1, 1.2.3.4/32, [::1]"),
    };
    for (int i = 0; i < allowInputs.size(); ++i) {
        out += jsonLine(QStringLiteral("Backend::parseAllowlistEntries"),
                        QStringLiteral("case_%1").arg(i),
                        QJsonObject{{QStringLiteral("rawText"), allowInputs.at(i)}},
                        QJsonArray::fromStringList(Backend::parseAllowlistEntries(allowInputs.at(i))));
    }

    // 2. resolveAllowlistCidrs (with mock resolver, testing all private/reserved IPv4 ranges + valid/invalid entries)
    {
        const QStringList entries = {
            QStringLiteral("github.com"),
            QStringLiteral("1.1.1.1"),
            QStringLiteral("10.0.0.0/8"),
            QStringLiteral("192.168.1.10/24"),
            QStringLiteral("999.0.0.1/32"),
            QStringLiteral("1.2.3.4/99"),
            QStringLiteral("2001:db8::1"),
            QStringLiteral("bad_host!"),
            QStringLiteral("-leading.dash"),
            QStringLiteral("rebind.test"),
            QStringLiteral("mixed.test"),
            QStringLiteral("github.com"),
        };
        auto resolver = [](const QString &host) -> QStringList {
            if (host == QLatin1String("github.com"))
                return {QStringLiteral("140.82.121.4"), QStringLiteral("192.168.1.50"), QStringLiteral("140.82.121.4")};
            if (host == QLatin1String("rebind.test"))
                return {
                    QStringLiteral("0.0.0.1"),
                    QStringLiteral("10.1.2.3"),
                    QStringLiteral("100.64.0.1"),
                    QStringLiteral("100.127.255.254"),
                    QStringLiteral("127.0.0.1"),
                    QStringLiteral("169.254.169.254"),
                    QStringLiteral("172.16.0.1"),
                    QStringLiteral("172.31.255.254"),
                    QStringLiteral("192.168.0.1"),
                    QStringLiteral("224.0.0.1"),
                    QStringLiteral("255.255.255.255"),
                    QStringLiteral("::1"),
                    QStringLiteral("not-an-ip"),
                };
            if (host == QLatin1String("mixed.test"))
                return {QStringLiteral("100.63.255.254"), QStringLiteral("172.15.255.254"), QStringLiteral("172.32.0.1")};
            return {};
        };
        QStringList warnings;
        const QStringList cidrs = Backend::resolveAllowlistCidrs(entries, resolver, &warnings);
        out += jsonLine(QStringLiteral("Backend::resolveAllowlistCidrs"),
                        QStringLiteral("comprehensive"),
                        QJsonObject{{QStringLiteral("entries"), QJsonArray::fromStringList(entries)}},
                        QJsonObject{{QStringLiteral("cidrs"), QJsonArray::fromStringList(cidrs)},
                                    {QStringLiteral("warnings"), QJsonArray::fromStringList(warnings)}});
    }

    // 3. aclApplyShellSnippet
    out += jsonLine(QStringLiteral("aclApplyShellSnippet"),
                    QStringLiteral("with_quotes_and_logging"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("box'1")},
                                {QStringLiteral("allowlistRaw"), QStringLiteral("github.com, '1.1.1.1'")},
                                {QStringLiteral("resolvedCidrs"), QJsonArray{QStringLiteral("140.82.121.4/32"), QStringLiteral("1.1.1.1/32'bad")}},
                                {QStringLiteral("egressLogging"), true}},
                    aclApplyShellSnippet(QStringLiteral("box'1"),
                                         QStringLiteral("github.com, '1.1.1.1'"),
                                         {QStringLiteral("140.82.121.4/32"), QStringLiteral("1.1.1.1/32'bad")},
                                         true));

    // 4. networkControlScript across modes, egressLogging, and quotes
    struct NetScriptCase {
        const char *label;
        const char *name;
        const char *mode;
        const char *allowRaw;
        QStringList cidrs;
        bool egress;
    };
    const QList<NetScriptCase> netCases = {
        {"online_plain", "agent-1", "online", "", {}, false},
        {"online_egress", "agent-1", "online", "", {}, true},
        {"open_plain", "agent-1", "open", "", {}, false},
        {"offline_plain", "agent-1", "offline", "", {}, false},
        {"offline_egress_ignored", "agent-1", "offline", "", {}, true},
        {"allowlist_plain", "agent-1", "allowlist", "github.com, 1.1.1.1", {QStringLiteral("140.82.121.4/32"), QStringLiteral("1.1.1.1/32")}, false},
        {"allowlist_egress", "agent-1", "allowlist", "github.com", {QStringLiteral("140.82.121.4/32")}, true},
        {"allowlist_quotes", "box'1", "allowlist", "a'b", {QStringLiteral("1.2.3.4/32"), QStringLiteral("5.6.7.8/32'x")}, true},
    };
    for (const auto &c : netCases) {
        out += jsonLine(QStringLiteral("Backend::networkControlScript"),
                        QString::fromLatin1(c.label),
                        QJsonObject{{QStringLiteral("name"), QString::fromLatin1(c.name)},
                                    {QStringLiteral("mode"), QString::fromLatin1(c.mode)},
                                    {QStringLiteral("allowlistRaw"), QString::fromLatin1(c.allowRaw)},
                                    {QStringLiteral("resolvedCidrs"), QJsonArray::fromStringList(c.cidrs)},
                                    {QStringLiteral("egressLogging"), c.egress}},
                        Backend::networkControlScript(QString::fromLatin1(c.name),
                                                      QString::fromLatin1(c.mode),
                                                      QString::fromLatin1(c.allowRaw),
                                                      c.cidrs,
                                                      c.egress));
    }

    // 5. parseKernelEgressLogs
    const QString journalSample = QStringLiteral(
        "2026-11-13T13:10:01+0700 host kernel: eth0-egress-0 IN=veth-box1 OUT=incusbr0 SRC=10.143.44.18 DST=140.82.121.4 LEN=60 PROTO=TCP SPT=50001 DPT=443 \n"
        "2026-11-13T13:10:03+0700 host kernel: eth0-egress-0 IN=veth-other OUT=incusbr0 SRC=10.143.44.99 DST=9.9.9.9 LEN=60 PROTO=UDP SPT=50002 DPT=53 \n"
        "2026-11-13T13:10:05+0700 host kernel: eth0-egress-default IN=veth-box1 OUT=incusbr0 SRC=10.143.44.18 DST=104.16.25.34 LEN=60 PROTO=TCP SPT=50003 DPT=443 \n"
        "Nov 13 13:10:07 host kernel: eth0-egress-0 IN=veth-old OUT=incusbr0 SRC=10.143.44.18 DST=1.1.1.1 LEN=60 PROTO=ICMP \n"
        "2026-11-13T13:10:09+0700 host kernel: eth0-egress-0 IN=veth-old OUT=incusbr0 SRC=10.143.44.18 DST=140.82.121.4 LEN=60 PROTO=TCP SPT=50004 DPT=443 \n"
        "2026-11-13T13:10:11+0700 host kernel: unrelated IN=veth-box1 SRC=10.143.44.18 DST=8.8.8.8 PROTO=UDP DPT=53 \n");
    out += jsonLine(QStringLiteral("Backend::parseKernelEgressLogs"),
                    QStringLiteral("sample_all"),
                    QJsonObject{{QStringLiteral("journalOutput"), journalSample},
                                {QStringLiteral("containerIp"), QStringLiteral("10.143.44.18")},
                                {QStringLiteral("hostIface"), QStringLiteral("veth-box1")},
                                {QStringLiteral("maxEntries"), 100}},
                    QJsonValue::fromVariant(Backend::parseKernelEgressLogs(
                        journalSample, QStringLiteral("10.143.44.18"), QStringLiteral("veth-box1"), 100)));
    out += jsonLine(QStringLiteral("Backend::parseKernelEgressLogs"),
                    QStringLiteral("cap_2_dash_ip"),
                    QJsonObject{{QStringLiteral("journalOutput"), journalSample},
                                {QStringLiteral("containerIp"), QStringLiteral("—")},
                                {QStringLiteral("hostIface"), QStringLiteral("veth-box1")},
                                {QStringLiteral("maxEntries"), 2}},
                    QJsonValue::fromVariant(Backend::parseKernelEgressLogs(
                        journalSample, QStringLiteral("—"), QStringLiteral("veth-box1"), 2)));
    out += jsonLine(QStringLiteral("Backend::parseKernelEgressLogs"),
                    QStringLiteral("empty_filters"),
                    QJsonObject{{QStringLiteral("journalOutput"), journalSample},
                                {QStringLiteral("containerIp"), QStringLiteral("—")},
                                {QStringLiteral("hostIface"), QStringLiteral("  ")},
                                {QStringLiteral("maxEntries"), 10}},
                    QJsonValue::fromVariant(Backend::parseKernelEgressLogs(
                        journalSample, QStringLiteral("—"), QStringLiteral("  "), 10)));

    // 6. proxyDeviceName & addPortForwardScript
    out += jsonLine(QStringLiteral("Backend::proxyDeviceName"),
                    QStringLiteral("port_3000"),
                    QJsonObject{{QStringLiteral("hostPort"), 3000}},
                    Backend::proxyDeviceName(3000));
    out += jsonLine(QStringLiteral("Backend::addPortForwardScript"),
                    QStringLiteral("basic"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("web-box")},
                                {QStringLiteral("hostPort"), 3000},
                                {QStringLiteral("guestPort"), 8080}},
                    Backend::addPortForwardScript(QStringLiteral("web-box"), 3000, 8080));
    out += jsonLine(QStringLiteral("Backend::addPortForwardScript"),
                    QStringLiteral("quoted_name"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("web'box")},
                                {QStringLiteral("hostPort"), 8443},
                                {QStringLiteral("guestPort"), 443}},
                    Backend::addPortForwardScript(QStringLiteral("web'box"), 8443, 443));

    // 7. addPortForward validation errors
    const QString netWork = QStringLiteral("/tmp/aibox-golden-net-work");
    QDir(netWork).removeRecursively();
    QDir().mkpath(netWork);
    {
        Backend backend(netWork + QStringLiteral("/be"));
        out += jsonLine(QStringLiteral("Backend::addPortForward"),
                        QStringLiteral("empty_name"),
                        QJsonObject{{QStringLiteral("name"), QStringLiteral("")},
                                    {QStringLiteral("hostPort"), 3000},
                                    {QStringLiteral("guestPort"), 80}},
                        backend.addPortForward(QStringLiteral(""), 3000, 80));
        out += jsonLine(QStringLiteral("Backend::addPortForward"),
                        QStringLiteral("privileged_host_port"),
                        QJsonObject{{QStringLiteral("name"), QStringLiteral("web-box")},
                                    {QStringLiteral("hostPort"), 80},
                                    {QStringLiteral("guestPort"), 80}},
                        backend.addPortForward(QStringLiteral("web-box"), 80, 80));
        out += jsonLine(QStringLiteral("Backend::addPortForward"),
                        QStringLiteral("invalid_guest_port"),
                        QJsonObject{{QStringLiteral("name"), QStringLiteral("web-box")},
                                    {QStringLiteral("hostPort"), 3000},
                                    {QStringLiteral("guestPort"), 0}},
                        backend.addPortForward(QStringLiteral("web-box"), 3000, 0));
        out += jsonLine(QStringLiteral("Backend::addPortForward"),
                        QStringLiteral("missing_env"),
                        QJsonObject{{QStringLiteral("name"), QStringLiteral("web-box")},
                                    {QStringLiteral("hostPort"), 3000},
                                    {QStringLiteral("guestPort"), 8080}},
                        backend.addPortForward(QStringLiteral("web-box"), 3000, 8080));
    }
    QDir(netWork).removeRecursively();

    return out;
}

static QByteArray captureBackup() {
    QByteArray out;

    // 1. hasSufficientDiskSpace
    const qint64 mib = 1024LL * 1024LL;
    const struct {
        const char *label;
        qint64 req;
        qint64 avail;
    } spaceCases[] = {
        {"exact_req_no_margin", 500 * mib, 500 * mib},
        {"one_byte_short", 500 * mib, 600 * mib - 1},
        {"exact_margin", 500 * mib, 600 * mib},
        {"negative_avail", 100, -1},
        {"negative_req", -1, 1000 * mib},
    };
    for (const auto &c : spaceCases) {
        out += jsonLine(QStringLiteral("Backend::hasSufficientDiskSpace"),
                        QString::fromLatin1(c.label),
                        QJsonObject{{QStringLiteral("requiredBytes"), c.req},
                                    {QStringLiteral("availableBytes"), c.avail}},
                        Backend::hasSufficientDiskSpace(c.req, c.avail));
    }

    // 2. directorySizeBytes
    const QString backupWork = QStringLiteral("/tmp/aibox-golden-backup-work");
    QDir(backupWork).removeRecursively();
    QDir().mkpath(backupWork + QStringLiteral("/tree/sub"));
    {
        QFile f1(backupWork + QStringLiteral("/tree/a.bin"));
        if (f1.open(QIODevice::WriteOnly)) {
            f1.write(QByteArray(123, 'a'));
        }
        QFile f2(backupWork + QStringLiteral("/tree/.hidden"));
        if (f2.open(QIODevice::WriteOnly)) {
            f2.write(QByteArray(45, 'h'));
        }
        QFile f3(backupWork + QStringLiteral("/tree/sub/b.bin"));
        if (f3.open(QIODevice::WriteOnly)) {
            f3.write(QByteArray(67, 'b'));
        }
        QFile fOut(backupWork + QStringLiteral("/outside.bin"));
        if (fOut.open(QIODevice::WriteOnly)) {
            fOut.write(QByteArray(999, 'o'));
        }
        QFile::link(backupWork + QStringLiteral("/outside.bin"),
                    backupWork + QStringLiteral("/tree/link.bin"));
    }
    out += jsonLine(QStringLiteral("Backend::directorySizeBytes"),
                    QStringLiteral("empty"),
                    QJsonObject{{QStringLiteral("dirPath"), QStringLiteral("")}},
                    Backend::directorySizeBytes(QString()));
    out += jsonLine(QStringLiteral("Backend::directorySizeBytes"),
                    QStringLiteral("missing"),
                    QJsonObject{{QStringLiteral("dirPath"), backupWork + QStringLiteral("/missing")}},
                    Backend::directorySizeBytes(backupWork + QStringLiteral("/missing")));
    out += jsonLine(QStringLiteral("Backend::directorySizeBytes"),
                    QStringLiteral("single_file"),
                    QJsonObject{{QStringLiteral("dirPath"), backupWork + QStringLiteral("/outside.bin")}},
                    Backend::directorySizeBytes(backupWork + QStringLiteral("/outside.bin")));
    out += jsonLine(QStringLiteral("Backend::directorySizeBytes"),
                    QStringLiteral("tree_with_hidden_and_symlink"),
                    QJsonObject{{QStringLiteral("dirPath"), backupWork + QStringLiteral("/tree")}},
                    Backend::directorySizeBytes(backupWork + QStringLiteral("/tree")));

    // 3. scanBackupArchives (with deterministic mtimes)
    const QString scanDir = backupWork + QStringLiteral("/scan");
    QDir().mkpath(scanDir);
    auto writeWithMtime = [](const QString &path, const QByteArray &data, qint64 epochSecs) {
        QFile f(path);
        if (f.open(QIODevice::WriteOnly)) {
            f.write(data);
            f.flush();
            f.setFileTime(QDateTime::fromSecsSinceEpoch(epochSecs), QFileDevice::FileModificationTime);
            f.close();
        }
    };
    writeWithMtime(scanDir + QStringLiteral("/alpha-20261113-120000.tar.gz"), QByteArray("backup-data"), 1794571200);
    writeWithMtime(scanDir + QStringLiteral("/alpha-20261113-120000-workspace.tar.gz"), QByteArray("ws-data"), 1794571205);
    writeWithMtime(scanDir + QStringLiteral("/beta-20261113-120500.tar.gz"), QByteArray("beta-only"), 1794571500);
    writeWithMtime(scanDir + QStringLiteral("/ignored.txt"), QByteArray("ignore"), 1794571600);
    out += jsonLine(QStringLiteral("Backend::scanBackupArchives"),
                    QStringLiteral("missing_dir"),
                    QJsonObject{{QStringLiteral("backupsDir"), backupWork + QStringLiteral("/no-scan")}},
                    QJsonValue::fromVariant(Backend::scanBackupArchives(backupWork + QStringLiteral("/no-scan"))));
    out += jsonLine(QStringLiteral("Backend::scanBackupArchives"),
                    QStringLiteral("paired_and_unpaired"),
                    QJsonObject{{QStringLiteral("backupsDir"), scanDir}},
                    QJsonValue::fromVariant(Backend::scanBackupArchives(scanDir)));

    // 4. exportEnvironmentScript, exportAndWipeEnvironmentScript, importEnvironmentScript
    out += jsonLine(QStringLiteral("Backend::exportEnvironmentScript"),
                    QStringLiteral("with_workspace"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("myenv")},
                                {QStringLiteral("backupFile"), QStringLiteral("/tmp/backups/myenv.tar.gz")},
                                {QStringLiteral("workspacePath"), QStringLiteral("/tmp/ws/myenv")},
                                {QStringLiteral("workspaceArchiveFile"), QStringLiteral("/tmp/backups/myenv-workspace.tar.gz")}},
                    Backend::exportEnvironmentScript(QStringLiteral("myenv"),
                                                     QStringLiteral("/tmp/backups/myenv.tar.gz"),
                                                     QStringLiteral("/tmp/ws/myenv"),
                                                     QStringLiteral("/tmp/backups/myenv-workspace.tar.gz")));
    out += jsonLine(QStringLiteral("Backend::exportEnvironmentScript"),
                    QStringLiteral("without_workspace_quoted"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("my'env")},
                                {QStringLiteral("backupFile"), QStringLiteral("/tmp/b/my'env.tar.gz")},
                                {QStringLiteral("workspacePath"), QStringLiteral("")},
                                {QStringLiteral("workspaceArchiveFile"), QStringLiteral("")}},
                    Backend::exportEnvironmentScript(QStringLiteral("my'env"),
                                                     QStringLiteral("/tmp/b/my'env.tar.gz"),
                                                     QString(),
                                                     QString()));

    out += jsonLine(QStringLiteral("Backend::exportAndWipeEnvironmentScript"),
                    QStringLiteral("with_workspace_and_delete"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("stale-env")},
                                {QStringLiteral("backupFile"), QStringLiteral("/tmp/b/stale.tar.gz")},
                                {QStringLiteral("workspacePath"), QStringLiteral("/tmp/ws/stale-env")},
                                {QStringLiteral("workspaceArchiveFile"), QStringLiteral("/tmp/b/stale-workspace.tar.gz")},
                                {QStringLiteral("deleteWorkspaceFiles"), true},
                                {QStringLiteral("sessionsEnvDir"), QStringLiteral("/tmp/sessions/stale-env")},
                                {QStringLiteral("environmentSecretsPath"), QStringLiteral("/tmp/secrets/stale-env.json")}},
                    Backend::exportAndWipeEnvironmentScript(QStringLiteral("stale-env"),
                                                            QStringLiteral("/tmp/b/stale.tar.gz"),
                                                            QStringLiteral("/tmp/ws/stale-env"),
                                                            QStringLiteral("/tmp/b/stale-workspace.tar.gz"),
                                                            true,
                                                            QStringLiteral("/tmp/sessions/stale-env"),
                                                            QStringLiteral("/tmp/secrets/stale-env.json")));
    out += jsonLine(QStringLiteral("Backend::exportAndWipeEnvironmentScript"),
                    QStringLiteral("without_workspace_keep_files"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("keep-ws")},
                                {QStringLiteral("backupFile"), QStringLiteral("/tmp/b/keep.tar.gz")},
                                {QStringLiteral("workspacePath"), QStringLiteral("/tmp/ws/keep-ws")},
                                {QStringLiteral("workspaceArchiveFile"), QStringLiteral("")},
                                {QStringLiteral("deleteWorkspaceFiles"), false},
                                {QStringLiteral("sessionsEnvDir"), QStringLiteral("")},
                                {QStringLiteral("environmentSecretsPath"), QStringLiteral("")}},
                    Backend::exportAndWipeEnvironmentScript(QStringLiteral("keep-ws"),
                                                            QStringLiteral("/tmp/b/keep.tar.gz"),
                                                            QStringLiteral("/tmp/ws/keep-ws"),
                                                            QString(),
                                                            false));

    out += jsonLine(QStringLiteral("Backend::importEnvironmentScript"),
                    QStringLiteral("with_workspace"),
                    QJsonObject{{QStringLiteral("backupFile"), QStringLiteral("/tmp/b/myenv.tar.gz")},
                                {QStringLiteral("targetName"), QStringLiteral("myenv-copy")},
                                {QStringLiteral("workspacePath"), QStringLiteral("/tmp/ws/myenv-copy")},
                                {QStringLiteral("workspaceArchiveFile"), QStringLiteral("/tmp/b/myenv-workspace.tar.gz")}},
                    Backend::importEnvironmentScript(QStringLiteral("/tmp/b/myenv.tar.gz"),
                                                     QStringLiteral("myenv-copy"),
                                                     QStringLiteral("/tmp/ws/myenv-copy"),
                                                     QStringLiteral("/tmp/b/myenv-workspace.tar.gz")));
    out += jsonLine(QStringLiteral("Backend::importEnvironmentScript"),
                    QStringLiteral("without_workspace"),
                    QJsonObject{{QStringLiteral("backupFile"), QStringLiteral("/tmp/b/myenv.tar.gz")},
                                {QStringLiteral("targetName"), QStringLiteral("myenv-copy")},
                                {QStringLiteral("workspacePath"), QStringLiteral("/tmp/ws/myenv-copy")},
                                {QStringLiteral("workspaceArchiveFile"), QStringLiteral("")}},
                    Backend::importEnvironmentScript(QStringLiteral("/tmp/b/myenv.tar.gz"),
                                                     QStringLiteral("myenv-copy"),
                                                     QStringLiteral("/tmp/ws/myenv-copy"),
                                                     QString()));

    // 5. parseActiveIncusExecEnvs
    const QString pgrepSample = QStringLiteral(
        "1001 incus exec --cwd /workspace -e FOO=bar --env BAZ=1 dev-box -- bash\n"
        "1002 /usr/bin/incus exec --user 1000 --mode=interactive alpha-box\n"
        "1003 incus exec -- dev-box\n"
        "1004 incus shell other-box\n"
        "1005 incus exec dev-box -- ls\n");
    QStringList sortedExecEnvs = Backend::parseActiveIncusExecEnvs(pgrepSample).values();
    sortedExecEnvs.sort();
    out += jsonLine(QStringLiteral("Backend::parseActiveIncusExecEnvs"),
                    QStringLiteral("mixed_flags"),
                    QJsonObject{{QStringLiteral("pgrepOutput"), pgrepSample}},
                    QJsonArray::fromStringList(sortedExecEnvs));

    // 6. classifyEnvironmentStaleness
    const qint64 nowSec = 1794500000LL;
    auto stalenessToJson = [](const Backend::StalenessResult &r) {
        return QJsonObject{
            {QStringLiteral("classification"), static_cast<int>(r.classification)},
            {QStringLiteral("daysUnused"), r.daysUnused},
            {QStringLiteral("statusKey"), r.statusKey},
        };
    };
    const struct {
        const char *label;
        qint64 lastUsed;
        bool isTpl;
        int staleDays;
    } staleCases[] = {
        {"template", nowSec - 120 * 86400LL, true, 30},
        {"unknown_zero", 0, false, 30},
        {"fresh_5d", nowSec - 5 * 86400LL, false, 30},
        {"stale_exact_30d", nowSec - 30 * 86400LL, false, 30},
        {"stale_35d", nowSec - 35 * 86400LL, false, 30},
        {"clamped_stale_days_zero", nowSec - 2 * 86400LL, false, 0},
    };
    for (const auto &c : staleCases) {
        out += jsonLine(QStringLiteral("Backend::classifyEnvironmentStaleness"),
                        QString::fromLatin1(c.label),
                        QJsonObject{{QStringLiteral("lastUsedEpoch"), c.lastUsed},
                                    {QStringLiteral("isTemplate"), c.isTpl},
                                    {QStringLiteral("staleDays"), c.staleDays},
                                    {QStringLiteral("nowEpoch"), nowSec}},
                        stalenessToJson(Backend::classifyEnvironmentStaleness(
                            c.lastUsed, c.isTpl, c.staleDays, nowSec)));
    }

    // 7. detectOrphans & buildStaleReport
    const QString wsBase = backupWork + QStringLiteral("/ws");
    const QString snapBase = wsBase + QStringLiteral("/.snapshots");
    const QString bkpDir = backupWork + QStringLiteral("/backups");
    QDir().mkpath(wsBase + QStringLiteral("/keep-env"));
    QDir().mkpath(wsBase + QStringLiteral("/stale-env"));
    QDir().mkpath(wsBase + QStringLiteral("/ghost-ws"));
    QDir().mkpath(snapBase + QStringLiteral("/keep-env/snap1"));
    QDir().mkpath(snapBase + QStringLiteral("/ghost-snaps/old1"));
    QDir().mkpath(bkpDir);
    writeWithMtime(wsBase + QStringLiteral("/keep-env/app.txt"), QByteArray("active"), 1794500000);
    writeWithMtime(wsBase + QStringLiteral("/stale-env/readme.md"), QByteArray(4096, 's'), 1794500000);
    writeWithMtime(wsBase + QStringLiteral("/ghost-ws/leak.bin"), QByteArray(2048, 'g'), 1794500000);
    writeWithMtime(snapBase + QStringLiteral("/ghost-snaps/old1/data.bin"), QByteArray(1024, 'x'), 1794500000);
    writeWithMtime(bkpDir + QStringLiteral("/keep-env-20261110-120000.tar.gz"), QByteArray("inst"), 1794500000);
    writeWithMtime(bkpDir + QStringLiteral("/keep-env-20261110-120000-workspace.tar.gz"), QByteArray("ws"), 1794500000);
    writeWithMtime(bkpDir + QStringLiteral("/ghost-old-20261001-010203-workspace.tar.gz"), QByteArray(512, 'b'), 1794500000);

    const QVariantList envs = {
        QVariantMap{
            {QStringLiteral("name"), QStringLiteral("keep-env")},
            {QStringLiteral("running"), true},
            {QStringLiteral("type"), QStringLiteral("CONTAINER")},
            {QStringLiteral("osName"), QStringLiteral("Ubuntu")},
            {QStringLiteral("isTemplate"), false},
            {QStringLiteral("lastUsedEpoch"), nowSec - 2LL * 86400LL},
            {QStringLiteral("diskUsageBytes"), qint64(10485760LL)},
            {QStringLiteral("snapshotCount"), 1},
            {QStringLiteral("snapshotTotalBytes"), qint64(1048576LL)},
            {QStringLiteral("workspacePath"), wsBase + QStringLiteral("/keep-env")},
        },
        QVariantMap{
            {QStringLiteral("name"), QStringLiteral("stale-env")},
            {QStringLiteral("running"), false},
            {QStringLiteral("type"), QStringLiteral("CONTAINER")},
            {QStringLiteral("osName"), QStringLiteral("Debian")},
            {QStringLiteral("isTemplate"), false},
            {QStringLiteral("lastUsedEpoch"), nowSec - 45LL * 86400LL},
            {QStringLiteral("diskUsageBytes"), qint64(20971520LL)},
            {QStringLiteral("snapshotCount"), 2},
            {QStringLiteral("snapshotTotalBytes"), qint64(5242880LL)},
            {QStringLiteral("workspacePath"), wsBase + QStringLiteral("/stale-env")},
        },
        QVariantMap{
            {QStringLiteral("name"), QStringLiteral("tpl-old")},
            {QStringLiteral("running"), false},
            {QStringLiteral("type"), QStringLiteral("CONTAINER")},
            {QStringLiteral("osName"), QStringLiteral("Arch")},
            {QStringLiteral("isTemplate"), true},
            {QStringLiteral("lastUsedEpoch"), nowSec - 180LL * 86400LL},
            {QStringLiteral("diskUsageBytes"), qint64(5242880LL)},
            {QStringLiteral("snapshotCount"), 0},
            {QStringLiteral("snapshotTotalBytes"), qint64(0)},
            {QStringLiteral("workspacePath"), wsBase + QStringLiteral("/tpl-old")},
        },
        QVariantMap{
            {QStringLiteral("name"), QStringLiteral("never-stamped")},
            {QStringLiteral("running"), false},
            {QStringLiteral("type"), QStringLiteral("CONTAINER")},
            {QStringLiteral("osName"), QStringLiteral("Alpine")},
            {QStringLiteral("isTemplate"), false},
            {QStringLiteral("lastUsedEpoch"), qint64(0)},
            {QStringLiteral("diskUsageBytes"), qint64(1048576LL)},
            {QStringLiteral("snapshotCount"), 0},
            {QStringLiteral("snapshotTotalBytes"), qint64(0)},
            {QStringLiteral("workspacePath"), wsBase + QStringLiteral("/never-stamped")},
        },
    };
    out += jsonLine(QStringLiteral("Backend::detectOrphans"),
                    QStringLiteral("three_kinds"),
                    QJsonObject{{QStringLiteral("environments"), QJsonValue::fromVariant(envs)},
                                {QStringLiteral("workspaceBase"), wsBase},
                                {QStringLiteral("backupsDir"), bkpDir}},
                    QJsonValue::fromVariant(Backend::detectOrphans(envs, wsBase, bkpDir)));
    QVariantMap staleReport = Backend::buildStaleReport(envs, wsBase, bkpDir, 30, nowSec);
    if (staleReport.value(QStringLiteral("generatedAt")).toString().size() == 19) {
        staleReport[QStringLiteral("generatedAt")] = QStringLiteral("<NOW>");
    }
    out += jsonLine(QStringLiteral("Backend::buildStaleReport"),
                    QStringLiteral("full_report"),
                    QJsonObject{{QStringLiteral("environments"), QJsonValue::fromVariant(envs)},
                                {QStringLiteral("workspaceBase"), wsBase},
                                {QStringLiteral("backupsDir"), bkpDir},
                                {QStringLiteral("staleDays"), 30},
                                {QStringLiteral("nowEpoch"), nowSec}},
                    QJsonValue::fromVariant(staleReport));

    // 8. deleteOrphanScript & adminDeleteScriptText
    for (const QString &kind : {QStringLiteral("backup-workspace"),
                                QStringLiteral("workspace-snapshot"),
                                QStringLiteral("workspace")}) {
        const QString target = (kind == QLatin1String("backup-workspace"))
                                   ? (bkpDir + QStringLiteral("/ghost-old-20261001-010203-workspace.tar.gz"))
                                   : (kind == QLatin1String("workspace-snapshot"))
                                         ? (snapBase + QStringLiteral("/ghost-snaps"))
                                         : (wsBase + QStringLiteral("/ghost-ws"));
        out += jsonLine(QStringLiteral("Backend::deleteOrphanScript"),
                        kind,
                        QJsonObject{{QStringLiteral("kind"), kind},
                                    {QStringLiteral("targetPath"), target}},
                        Backend::deleteOrphanScript(kind, target));
        out += jsonLine(QStringLiteral("Backend::adminDeleteScriptText"),
                        kind,
                        QJsonObject{{QStringLiteral("kind"), kind},
                                    {QStringLiteral("targetPath"), target}},
                        Backend::adminDeleteScriptText(kind, target));
    }

    // 9. validateOrphanPath & validation errors
    {
        Backend backend(wsBase);
        const QString bkpBase = backend.backupsDirPath();
        const struct {
            const char *label;
            const char *kind;
            QString path;
        } valCases[] = {
            {"valid_workspace", "workspace", wsBase + QStringLiteral("/ghost-ws")},
            {"outside_workspace", "workspace", backupWork + QStringLiteral("/outside")},
            {"snapshots_dir_as_workspace", "workspace", snapBase},
            {"hidden_workspace", "workspace", wsBase + QStringLiteral("/.secret")},
            {"valid_snapshot", "workspace-snapshot", snapBase + QStringLiteral("/ghost-snaps")},
            {"outside_snapshot", "workspace-snapshot", wsBase + QStringLiteral("/ghost-ws")},
            {"valid_backup_ws", "backup-workspace", bkpBase + QStringLiteral("/foo-workspace.tar.gz")},
            {"non_ws_backup", "backup-workspace", bkpBase + QStringLiteral("/foo.tar.gz")},
        };
        for (const auto &c : valCases) {
            out += jsonLine(QStringLiteral("Backend::validateOrphanPath"),
                            QString::fromLatin1(c.label),
                            QJsonObject{{QStringLiteral("kind"), QString::fromLatin1(c.kind)},
                                        {QStringLiteral("cleanPath"), c.path},
                                        {QStringLiteral("workspaceBase"), wsBase},
                                        {QStringLiteral("backupsDir"), bkpBase}},
                            backend.validateOrphanPath(QString::fromLatin1(c.kind), c.path));
        }

        out += jsonLine(QStringLiteral("Backend::exportEnvironment"),
                        QStringLiteral("missing_env"),
                        QJsonObject{{QStringLiteral("name"), QStringLiteral("no-such-env")},
                                    {QStringLiteral("includeWorkspace"), true}},
                        backend.exportEnvironment(QStringLiteral("no-such-env"), true));
        out += jsonLine(QStringLiteral("Backend::importEnvironment"),
                        QStringLiteral("invalid_name"),
                        QJsonObject{{QStringLiteral("backupFile"), QStringLiteral("/tmp/x.tar.gz")},
                                    {QStringLiteral("newName"), QStringLiteral("!!!")},
                                    {QStringLiteral("workspaceArchiveFile"), QStringLiteral("")}},
                        backend.importEnvironment(QStringLiteral("/tmp/x.tar.gz"), QStringLiteral("!!!"), QString()));
        out += jsonLine(QStringLiteral("Backend::importEnvironment"),
                        QStringLiteral("missing_backup_file"),
                        QJsonObject{{QStringLiteral("backupFile"), QStringLiteral("/tmp/does-not-exist-aibox.tar.gz")},
                                    {QStringLiteral("newName"), QStringLiteral("valid-env")},
                                    {QStringLiteral("workspaceArchiveFile"), QStringLiteral("")}},
                        backend.importEnvironment(QStringLiteral("/tmp/does-not-exist-aibox.tar.gz"),
                                                  QStringLiteral("valid-env"), QString()));
        out += jsonLine(QStringLiteral("Backend::deleteOrphan"),
                        QStringLiteral("empty_path"),
                        QJsonObject{{QStringLiteral("kind"), QStringLiteral("workspace")},
                                    {QStringLiteral("path"), QStringLiteral("   ")}},
                        backend.deleteOrphan(QStringLiteral("workspace"), QStringLiteral("   ")));
        out += jsonLine(QStringLiteral("Backend::deleteOrphan"),
                        QStringLiteral("unknown_kind"),
                        QJsonObject{{QStringLiteral("kind"), QStringLiteral("bogus")},
                                    {QStringLiteral("path"), wsBase + QStringLiteral("/ghost-ws")}},
                        backend.deleteOrphan(QStringLiteral("bogus"), wsBase + QStringLiteral("/ghost-ws")));
    }

    QDir(backupWork).removeRecursively();
    return out;
}

static QJsonObject healthResultToJson(const Backend::HealthCheckResult &r) {
    return QJsonObject{
        {QStringLiteral("id"), r.id},
        {QStringLiteral("title"), r.title},
        {QStringLiteral("ok"), r.ok},
        {QStringLiteral("warning"), r.warning},
        {QStringLiteral("repaired"), r.repaired},
        {QStringLiteral("needsRoot"), r.needsRoot},
        {QStringLiteral("detail"), r.detail},
        {QStringLiteral("fixCommand"), r.fixCommand},
    };
}

static QJsonObject poolInfoToJson(const Backend::StoragePoolInfo &p) {
    return QJsonObject{
        {QStringLiteral("valid"), p.valid},
        {QStringLiteral("name"), p.name},
        {QStringLiteral("driver"), p.driver},
        {QStringLiteral("usedBytes"), static_cast<double>(p.usedBytes)},
        {QStringLiteral("totalBytes"), static_cast<double>(p.totalBytes)},
        {QStringLiteral("usedPercent"), p.usedPercent},
        {QStringLiteral("sizeText"), p.sizeText},
        {QStringLiteral("resizable"), p.resizable},
    };
}

static QByteArray captureHealth() {
    QByteArray out;
    const qint64 GiB = 1024LL * 1024LL * 1024LL;

    // 1. evaluateWatchdog
    struct WatchdogCase {
        const char *label;
        int pct;
        int prev;
        int warn;
        int crit;
        bool autoStop;
    };
    const WatchdogCase wdCases[] = {
        {"below_warn", 89, 0, 90, 97, true},
        {"enter_warn", 91, 0, 90, 97, true},
        {"hysteresis_warn_stay", 86, 1, 90, 97, true},
        {"hysteresis_warn_clear", 84, 1, 90, 97, true},
        {"enter_crit_no_autostop", 98, 1, 90, 97, false},
        {"enter_crit_autostop", 98, 1, 90, 97, true},
        {"hysteresis_crit_stay", 94, 2, 90, 97, true},
        {"clamped_thresholds", 50, 0, 10, 10, true},
    };
    for (const auto &c : wdCases) {
        const auto d = Backend::evaluateWatchdog(c.pct, c.prev, c.warn, c.crit, c.autoStop);
        out += jsonLine(QStringLiteral("Backend::evaluateWatchdog"),
                        QString::fromLatin1(c.label),
                        QJsonObject{{QStringLiteral("percent"), c.pct},
                                    {QStringLiteral("previousLevel"), c.prev},
                                    {QStringLiteral("warnThreshold"), c.warn},
                                    {QStringLiteral("criticalThreshold"), c.crit},
                                    {QStringLiteral("autoStopEnabled"), c.autoStop}},
                        QJsonObject{{QStringLiteral("newLevel"), d.newLevel},
                                    {QStringLiteral("shouldNotify"), d.shouldNotify},
                                    {QStringLiteral("shouldAutoStop"), d.shouldAutoStop}});
    }

    // 2. isValidProcessLimit & isValidIoLimit
    const QStringList procCases = {
        QString(), QStringLiteral("Unlimited"), QStringLiteral("512"),
        QStringLiteral("10"), QStringLiteral("1000000"), QStringLiteral("5"),
        QStringLiteral("9"), QStringLiteral("1000001"), QStringLiteral("-1"),
        QStringLiteral("abc"),
    };
    for (int i = 0; i < procCases.size(); ++i) {
        const QString &v = procCases.at(i);
        out += jsonLine(QStringLiteral("Backend::isValidProcessLimit"),
                        QStringLiteral("case_%1").arg(i),
                        QJsonObject{{QStringLiteral("value"), v}},
                        Backend::isValidProcessLimit(v));
    }
    const QStringList ioCases = {
        QString(), QStringLiteral("Unlimited"), QStringLiteral("200MB"),
        QStringLiteral("2500iops"), QStringLiteral("10GiB"), QStringLiteral("0MB"),
        QStringLiteral("fast"),
    };
    for (int i = 0; i < ioCases.size(); ++i) {
        const QString &v = ioCases.at(i);
        out += jsonLine(QStringLiteral("Backend::isValidIoLimit"),
                        QStringLiteral("case_%1").arg(i),
                        QJsonObject{{QStringLiteral("value"), v}},
                        Backend::isValidIoLimit(v));
    }

    // 3. parseIdleStopOverride & effectiveIdleStopMinutes
    const QStringList ovCases = {
        QString(), QStringLiteral("inherit"), QStringLiteral("default"),
        QStringLiteral("off"), QStringLiteral("0"), QStringLiteral("false"),
        QStringLiteral("30"), QStringLiteral("1440"), QStringLiteral("1441"),
        QStringLiteral("abc"),
    };
    for (int i = 0; i < ovCases.size(); ++i) {
        const QString &raw = ovCases.at(i);
        const auto ov = Backend::parseIdleStopOverride(raw);
        const QString modeStr = (ov.mode == Backend::IdleStopOverride::Off)
                                    ? QStringLiteral("off")
                                    : (ov.mode == Backend::IdleStopOverride::Minutes
                                           ? QStringLiteral("minutes")
                                           : QStringLiteral("inherit"));
        out += jsonLine(QStringLiteral("Backend::parseIdleStopOverride"),
                        QStringLiteral("case_%1").arg(i),
                        QJsonObject{{QStringLiteral("raw"), raw}},
                        QJsonObject{{QStringLiteral("mode"), modeStr},
                                    {QStringLiteral("minutes"), ov.minutes}});
    }
    const QList<QPair<int, QString>> effCases = {
        {0, QStringLiteral("inherit")},
        {60, QStringLiteral("inherit")},
        {60, QStringLiteral("off")},
        {0, QStringLiteral("15")},
        {60, QStringLiteral("15")},
        {2000, QStringLiteral("inherit")},
    };
    for (int i = 0; i < effCases.size(); ++i) {
        const int g = effCases.at(i).first;
        const QString &raw = effCases.at(i).second;
        const auto ov = Backend::parseIdleStopOverride(raw);
        out += jsonLine(QStringLiteral("Backend::effectiveIdleStopMinutes"),
                        QStringLiteral("case_%1").arg(i),
                        QJsonObject{{QStringLiteral("globalMinutes"), g},
                                    {QStringLiteral("raw"), raw}},
                        Backend::effectiveIdleStopMinutes(g, ov));
    }

    // 4. evaluateIdleDecision
    auto idleInputToJson = [](const Backend::IdleDecisionInput &in) {
        QJsonArray samplesArr;
        for (const auto &s : in.samples) {
            samplesArr.append(QJsonObject{
                {QStringLiteral("timestampMs"), static_cast<double>(s.timestampMs)},
                {QStringLiteral("cpuPercent"), s.cpuPercent},
            });
        }
        return QJsonObject{
            {QStringLiteral("running"), in.running},
            {QStringLiteral("isTemplate"), in.isTemplate},
            {QStringLiteral("busyOnThisEnv"), in.busyOnThisEnv},
            {QStringLiteral("hasActiveExecSession"), in.hasActiveExecSession},
            {QStringLiteral("portForwardCount"), in.portForwardCount},
            {QStringLiteral("ignorePortForwards"), in.ignorePortForwards},
            {QStringLiteral("thresholdMinutes"), in.thresholdMinutes},
            {QStringLiteral("nowMs"), static_cast<double>(in.nowMs)},
            {QStringLiteral("warningAlreadySent"), in.warningAlreadySent},
            {QStringLiteral("samples"), samplesArr},
        };
    };
    auto emitIdle = [&](const char *label, const Backend::IdleDecisionInput &in) {
        const auto res = Backend::evaluateIdleDecision(in);
        out += jsonLine(QStringLiteral("Backend::evaluateIdleDecision"),
                        QString::fromLatin1(label),
                        idleInputToJson(in),
                        QJsonObject{
                            {QStringLiteral("eligible"), res.eligible},
                            {QStringLiteral("idleDurationMs"), static_cast<double>(res.idleDurationMs)},
                            {QStringLiteral("averageCpuPercent"), res.averageCpuPercent},
                            {QStringLiteral("shouldWarnOneMin"), res.shouldWarnOneMin},
                            {QStringLiteral("shouldStopNow"), res.shouldStopNow},
                            {QStringLiteral("blockedReason"), res.blockedReason},
                        });
    };

    const qint64 minMs = 60LL * 1000LL;
    Backend::IdleDecisionInput baseIdle;
    baseIdle.running = true;
    baseIdle.thresholdMinutes = 15;
    baseIdle.samples = {{0 * minMs, 1}, {5 * minMs, 2}, {10 * minMs, 1}, {14 * minMs, 1}};
    baseIdle.nowMs = 14 * minMs;
    emitIdle("warn_one_min", baseIdle);

    {
        auto in = baseIdle;
        in.warningAlreadySent = true;
        emitIdle("warn_already_sent", in);
    }
    {
        auto in = baseIdle;
        in.samples.append({15 * minMs, 0});
        in.nowMs = 15 * minMs;
        emitIdle("stop_now", in);
    }
    {
        auto in = baseIdle;
        in.samples = {{0 * minMs, 1}, {5 * minMs, 1}, {10 * minMs, 8}, {11 * minMs, 1}, {15 * minMs, 1}};
        in.nowMs = 15 * minMs;
        emitIdle("cpu_spike_resets_timer", in);
    }
    {
        auto in = baseIdle;
        in.samples = {{0 * minMs, 1}, {5 * minMs, 5}};
        in.nowMs = 5 * minMs;
        emitIdle("cpu_spike_at_end", in);
    }
    {
        auto in = baseIdle;
        in.hasActiveExecSession = true;
        emitIdle("blocked_exec", in);
    }
    {
        auto in = baseIdle;
        in.portForwardCount = 2;
        emitIdle("blocked_ports", in);
        in.ignorePortForwards = true;
        in.samples.append({15 * minMs, 0});
        in.nowMs = 15 * minMs;
        emitIdle("ports_ignored_stop", in);
    }
    {
        auto in = baseIdle;
        in.isTemplate = true;
        emitIdle("blocked_template", in);
    }
    {
        auto in = baseIdle;
        in.busyOnThisEnv = true;
        emitIdle("blocked_busy", in);
    }
    {
        auto in = baseIdle;
        in.running = false;
        emitIdle("blocked_stopped", in);
    }
    {
        auto in = baseIdle;
        in.thresholdMinutes = 0;
        emitIdle("blocked_disabled", in);
    }

    // 5. parseIncusStorageInfo
    const QList<QPair<QString, QString>> storageFixtures = {
        {QStringLiteral("bytes_yaml"),
         QStringLiteral("info:\n"
                        "  description: Btrfs Storage Pool\n"
                        "  driver: btrfs\n"
                        "  name: default\n"
                        "  space used: \"1329242112\"\n"
                        "  total space: \"32212254720\"\n"
                        "used by:\n"
                        "  instances:\n"
                        "    - mobile-porting\n")},
        {QStringLiteral("human_yaml"),
         QStringLiteral("info:\n"
                        "  driver: btrfs\n"
                        "  name: default\n"
                        "  space used: 1.24GiB\n"
                        "  total space: 30.00GiB\n")},
        {QStringLiteral("json_info"),
         QStringLiteral("{\"info\":{\"name\":\"default\",\"driver\":\"btrfs\",\"space used\":26843545600,\"total space\":32212254720}}")},
        {QStringLiteral("json_flat"),
         QStringLiteral("{\"name\":\"ssd\",\"driver\":\"dir\",\"used\":\"10GiB\",\"total\":\"100GiB\"}")},
        {QStringLiteral("invalid"), QStringLiteral("invalid output")},
        {QStringLiteral("empty"), QString()},
    };
    for (const auto &c : storageFixtures) {
        const auto info = Backend::parseIncusStorageInfo(c.second);
        out += jsonLine(QStringLiteral("Backend::parseIncusStorageInfo"),
                        c.first,
                        QJsonObject{{QStringLiteral("output"), c.second}},
                        poolInfoToJson(info));
    }

    // 6. sumRootDiskQuotasBytes
    const QString listJsonStr = QStringLiteral(
        "["
        "  {\"name\":\"env-a\",\"devices\":{},\"expanded_devices\":{\"root\":{\"path\":\"/\",\"pool\":\"default\",\"size\":\"20GiB\"}}},"
        "  {\"name\":\"env-b\",\"devices\":{\"root\":{\"size\":\"15GiB\"}},\"expanded_devices\":{\"root\":{\"path\":\"/\",\"pool\":\"default\",\"size\":\"15GiB\"}}},"
        "  {\"name\":\"env-noquota\",\"devices\":{},\"expanded_devices\":{\"root\":{\"path\":\"/\",\"pool\":\"default\"}}},"
        "  {\"name\":\"env-otherpool\",\"devices\":{},\"expanded_devices\":{\"root\":{\"path\":\"/\",\"pool\":\"ssd2\",\"size\":\"100GiB\"}}}"
        "]");
    struct QuotaCase {
        const char *label;
        QString json;
        QString exclude;
        QString pool;
    };
    const QuotaCase quotaCases[] = {
        {"all_default", listJsonStr, QString(), QStringLiteral("default")},
        {"exclude_env_a", listJsonStr, QStringLiteral("env-a"), QStringLiteral("default")},
        {"other_pool", listJsonStr, QString(), QStringLiteral("ssd2")},
        {"invalid_json", QStringLiteral("not-json"), QString(), QStringLiteral("default")},
    };
    for (const auto &c : quotaCases) {
        int unbounded = 0;
        const qint64 sum = Backend::sumRootDiskQuotasBytes(c.json.toUtf8(), c.exclude, c.pool, &unbounded);
        out += jsonLine(QStringLiteral("Backend::sumRootDiskQuotasBytes"),
                        QString::fromLatin1(c.label),
                        QJsonObject{{QStringLiteral("incusListJson"), c.json},
                                    {QStringLiteral("excludeEnvName"), c.exclude},
                                    {QStringLiteral("poolName"), c.pool}},
                        QJsonObject{{QStringLiteral("bytes"), static_cast<double>(sum)},
                                    {QStringLiteral("unboundedCount"), unbounded}});
    }

    // 7. evaluatePoolOvercommitWarning & evaluatePoolCapacityHealth & poolInfoToVariantMap
    Backend::StoragePoolInfo pool40;
    pool40.valid = true;
    pool40.name = QStringLiteral("default");
    pool40.driver = QStringLiteral("btrfs");
    pool40.usedBytes = 10LL * GiB;
    pool40.totalBytes = 40LL * GiB;
    pool40.usedPercent = 25;
    pool40.sizeText = QStringLiteral("40GiB");
    pool40.resizable = true;

    Backend::StoragePoolInfo pool88 = pool40;
    pool88.usedBytes = 35LL * GiB;
    pool88.usedPercent = 88;

    Backend::StoragePoolInfo poolInvalid;

    struct OvercommitCase {
        const char *label;
        Backend::StoragePoolInfo pool;
        qint64 existing;
        qint64 requested;
        int unbounded;
    };
    const OvercommitCase ocCases[] = {
        {"ok", pool40, 35LL * GiB, 4LL * GiB, 1},
        {"overcommit_unbounded", pool40, 35LL * GiB, 10LL * GiB, 1},
        {"high_pool_usage", pool88, 0, 1LL * GiB, 0},
        {"invalid_pool", poolInvalid, 0, 1LL * GiB, 0},
    };
    for (const auto &c : ocCases) {
        out += jsonLine(QStringLiteral("Backend::evaluatePoolOvercommitWarning"),
                        QString::fromLatin1(c.label),
                        QJsonObject{{QStringLiteral("pool"), poolInfoToJson(c.pool)},
                                    {QStringLiteral("existingQuotasBytes"), static_cast<double>(c.existing)},
                                    {QStringLiteral("requestedBytes"), static_cast<double>(c.requested)},
                                    {QStringLiteral("unboundedCount"), c.unbounded}},
                        Backend::evaluatePoolOvercommitWarning(c.pool, c.existing, c.requested, c.unbounded));
    }

    Backend::StoragePoolInfo pool100 = pool40;
    pool100.totalBytes = 100LL * GiB;
    pool100.usedBytes = 85LL * GiB;
    pool100.usedPercent = 85;
    Backend::StoragePoolInfo pool86 = pool100;
    pool86.usedBytes = 86LL * GiB;
    pool86.usedPercent = 86;
    Backend::StoragePoolInfo pool96 = pool100;
    pool96.usedBytes = 96LL * GiB;
    pool96.usedPercent = 96;

    struct CapCase {
        const char *label;
        Backend::StoragePoolInfo pool;
        qint64 allocated;
        int unbounded;
    };
    const CapCase capCases[] = {
        {"invalid", poolInvalid, 0, 0},
        {"healthy_85", pool100, 60LL * GiB, 0},
        {"warning_86", pool86, 60LL * GiB, 1},
        {"critical_96", pool96, 120LL * GiB, 0},
    };
    for (const auto &c : capCases) {
        const auto h = Backend::evaluatePoolCapacityHealth(c.pool, c.allocated, c.unbounded);
        out += jsonLine(QStringLiteral("Backend::evaluatePoolCapacityHealth"),
                        QString::fromLatin1(c.label),
                        QJsonObject{{QStringLiteral("pool"), poolInfoToJson(c.pool)},
                                    {QStringLiteral("allocatedQuotasBytes"), static_cast<double>(c.allocated)},
                                    {QStringLiteral("unboundedCount"), c.unbounded}},
                        healthResultToJson(h));

        const QVariantMap vm = poolInfoToVariantMap(c.pool, c.allocated, c.unbounded);
        out += jsonLine(QStringLiteral("poolInfoToVariantMap"),
                        QString::fromLatin1(c.label),
                        QJsonObject{{QStringLiteral("pool"), poolInfoToJson(c.pool)},
                                    {QStringLiteral("allocatedBytes"), static_cast<double>(c.allocated)},
                                    {QStringLiteral("unboundedCount"), c.unbounded}},
                        QJsonObject::fromVariantMap(vm));
    }

    // 8. parsePoolSizeSpec & resizeStoragePoolScript
    const QStringList sizeSpecs = {
        QStringLiteral("60"), QStringLiteral(" 60GiB "), QStringLiteral("1.5TiB"),
        QStringLiteral("512MiB"), QStringLiteral("2048MB"), QString(),
        QStringLiteral("big"), QStringLiteral("-60GiB"), QStringLiteral("0"),
    };
    for (int i = 0; i < sizeSpecs.size(); ++i) {
        qint64 bytes = 0;
        const bool ok = Backend::parsePoolSizeSpec(sizeSpecs.at(i), &bytes);
        out += jsonLine(QStringLiteral("Backend::parsePoolSizeSpec"),
                        QStringLiteral("case_%1").arg(i),
                        QJsonObject{{QStringLiteral("spec"), sizeSpecs.at(i)}},
                        QJsonObject{{QStringLiteral("ok"), ok},
                                    {QStringLiteral("bytes"), static_cast<double>(bytes)}});
    }
    out += jsonLine(QStringLiteral("Backend::resizeStoragePoolScript"),
                    QStringLiteral("default_60GiB"),
                    QJsonObject{{QStringLiteral("poolName"), QStringLiteral("default")},
                                {QStringLiteral("sizeSpec"), QStringLiteral(" 60GiB ")}},
                    Backend::resizeStoragePoolScript(QStringLiteral("default"), QStringLiteral(" 60GiB ")));

    // 9. runHealthChecks (stub runners)
    const QString fixedWsBase = QStringLiteral("/tmp/aibox-golden-home/aibox");
    const QString healthyRules = QStringLiteral(
        "-A ufw-user-input -i incusbr0 -j ACCEPT\n"
        "-A ufw-user-forward -i incusbr0 -j ACCEPT\n"
        "-A ufw-user-forward -o incusbr0 -j ACCEPT\n");
    auto emitHealth = [&](const char *label, bool fix, const QString &ufwRules,
                          const Backend::CommandRunner &runner) {
        const auto checks = Backend::runHealthChecks(fix, runner, QStringLiteral("testuser"), ufwRules, fixedWsBase);
        QJsonArray arr;
        for (const auto &r : checks)
            arr.append(healthResultToJson(r));
        out += jsonLine(QStringLiteral("Backend::runHealthChecks"),
                        QString::fromLatin1(label),
                        QJsonObject{{QStringLiteral("fix"), fix},
                                    {QStringLiteral("targetUser"), QStringLiteral("testuser")},
                                    {QStringLiteral("ufwRulesContent"), ufwRules},
                                    {QStringLiteral("workspaceBase"), fixedWsBase}},
                        arr);
    };

    emitHealth("all_pass", false, healthyRules,
               [](const QString &prog, const QStringList &args) -> Backend::CommandOutput {
                   if (prog == QLatin1String("id"))
                       return {0, QStringLiteral("testuser wheel incus-admin kvm render video\n"), {}};
                   if (prog == QLatin1String("__socket_rw__"))
                       return {0, {}, {}};
                   if (prog == QLatin1String("systemctl") && args.value(0) == QLatin1String("is-active"))
                       return {0, QStringLiteral("active\n"), {}};
                   if (prog == QLatin1String("incus")) {
                       if (args.value(0) == QLatin1String("list"))
                           return {0, QStringLiteral("[]"), {}};
                       if (args.value(0) == QLatin1String("network") && args.value(1) == QLatin1String("show"))
                           return {0, QStringLiteral("name: incusbr0\nmanaged: true\n"), {}};
                       if (args.value(0) == QLatin1String("profile") && args.value(1) == QLatin1String("show"))
                           return {0, QStringLiteral("config:\n  security.nesting: \"true\"\ndevices:\n  gpu:\n    type: gpu\n  kvm:\n    path: /dev/kvm\n"), {}};
                       if (args.value(0) == QLatin1String("storage") && args.value(1) == QLatin1String("info"))
                           return {0, QStringLiteral("info:\n  driver: btrfs\n  name: default\n  space used: \"1329242112\"\n  total space: \"32212254720\"\n"), {}};
                   }
                   if (prog == QLatin1String("findmnt"))
                       return {0, QStringLiteral("/home btrfs rw,relatime,user_subvol_rm_allowed\n"), {}};
                   return {1, {}, QStringLiteral("unexpected command")};
               });

    {
        bool bridgeExists = false;
        bool hasNesting = false;
        bool hasGpu = false;
        bool hasKvm = false;
        Backend::CommandRunner fixRunner = [&](const QString &prog, const QStringList &args) -> Backend::CommandOutput {
            if (prog == QLatin1String("id"))
                return {0, QStringLiteral("testuser incus-admin kvm render video\n"), {}};
            if (prog == QLatin1String("__socket_rw__"))
                return {0, {}, {}};
            if (prog == QLatin1String("systemctl"))
                return {3, QStringLiteral("inactive\n"), {}};
            if (prog == QLatin1String("incus")) {
                if (args.value(0) == QLatin1String("list"))
                    return {0, QStringLiteral("[]"), {}};
                if (args.value(0) == QLatin1String("network") && args.value(1) == QLatin1String("show"))
                    return bridgeExists ? Backend::CommandOutput{0, QStringLiteral("name: incusbr0\n"), {}}
                                        : Backend::CommandOutput{1, {}, QStringLiteral("not found")};
                if (args.value(0) == QLatin1String("network") && args.value(1) == QLatin1String("create") && args.value(2) == QLatin1String("incusbr0")) {
                    bridgeExists = true;
                    return {0, {}, {}};
                }
                if (args.value(0) == QLatin1String("profile") && args.value(1) == QLatin1String("show")) {
                    QString pOut = QStringLiteral("config:\n");
                    if (hasNesting)
                        pOut += QStringLiteral("  security.nesting: \"true\"\n");
                    pOut += QStringLiteral("devices:\n");
                    if (hasGpu)
                        pOut += QStringLiteral("  gpu:\n    type: gpu\n");
                    if (hasKvm)
                        pOut += QStringLiteral("  kvm:\n    path: /dev/kvm\n");
                    return {0, pOut, {}};
                }
                if (args.value(0) == QLatin1String("profile") && args.value(1) == QLatin1String("set")) {
                    hasNesting = true;
                    return {0, {}, {}};
                }
                if (args.value(0) == QLatin1String("profile") && args.value(1) == QLatin1String("device") && args.value(2) == QLatin1String("add")) {
                    if (args.value(4) == QLatin1String("gpu"))
                        hasGpu = true;
                    if (args.value(4) == QLatin1String("kvm"))
                        hasKvm = true;
                    return {0, {}, {}};
                }
            }
            return {1, {}, QStringLiteral("unhandled")};
        };
        emitHealth("fix_dry_run", false, QString(), fixRunner);
        emitHealth("fix_repaired", true, QString(), fixRunner);
    }

    emitHealth("root_failures", true, QStringLiteral("# empty rules"),
               [](const QString &prog, const QStringList &args) -> Backend::CommandOutput {
                   if (prog == QLatin1String("id"))
                       return {0, QStringLiteral("testuser wheel\n"), {}};
                   if (prog == QLatin1String("__socket_rw__"))
                       return {1, {}, QStringLiteral("permission denied")};
                   if (prog == QLatin1String("systemctl") && args.value(0) == QLatin1String("is-active"))
                       return {0, QStringLiteral("active\n"), {}};
                   if (prog == QLatin1String("findmnt"))
                       return {0, QStringLiteral("/home btrfs rw,relatime\n"), {}};
                   return {1, {}, QStringLiteral("daemon down")};
               });

    auto makePoolPctRunner = [](const QString &usedPctStr) -> Backend::CommandRunner {
        return [usedPctStr](const QString &prog, const QStringList &args) -> Backend::CommandOutput {
            if (prog == QLatin1String("id"))
                return {0, QStringLiteral("testuser incus-admin kvm render video\n"), {}};
            if (prog == QLatin1String("__socket_rw__"))
                return {0, {}, {}};
            if (prog == QLatin1String("systemctl"))
                return {3, QStringLiteral("inactive\n"), {}};
            if (prog == QLatin1String("incus")) {
                if (args.value(0) == QLatin1String("list"))
                    return {0, QStringLiteral("[]"), {}};
                if (args.value(0) == QLatin1String("network"))
                    return {0, QStringLiteral("name: incusbr0\n"), {}};
                if (args.value(0) == QLatin1String("profile"))
                    return {0, QStringLiteral("config:\n  security.nesting: \"true\"\ndevices:\n  gpu:\n    type: gpu\n  kvm:\n    path: /dev/kvm\n"), {}};
                if (args.value(0) == QLatin1String("storage"))
                    return {0, QStringLiteral("info:\n  driver: btrfs\n  name: default\n  space used: \"%1\"\n  total space: \"100\"\n").arg(usedPctStr), {}};
            }
            if (prog == QLatin1String("findmnt"))
                return {0, QStringLiteral("/home btrfs rw,user_subvol_rm_allowed\n"), {}};
            return {1, {}, {}};
        };
    };
    emitHealth("pool_warn_90", false, QString(), makePoolPctRunner(QStringLiteral("90")));
    emitHealth("pool_fail_97", false, QString(), makePoolPctRunner(QStringLiteral("97")));

    return out;
}

static QByteArray captureProvision() {
    QByteArray out;

    // 1. createEnvironmentScript matrix
    struct CreateCase {
        const char *label;
        const char *name;
        const char *localAlias;
        const char *remoteImage;
        const char *workspace;
        const char *cacheBase;
        const char *profileFlags;
        const char *ramGiB;
        const char *diskGiB;
        int cpuCores;
        bool x11;
        const char *processLimit;
        const char *ioLimit;
        bool isVm;
    };
    const CreateCase createCases[] = {
        {"ct_gui_full", "my-box", "aibox-archlinux", "images:archlinux",
         "/tmp/aibox-golden-home/ws/my-box", "/tmp/aibox-golden-home/cache/archlinux",
         "-p default -p ai-gui", "6GiB", "35GiB", 4, true, "512", "200MB", false},
        {"ct_headless_no_limits", "headless-1", "aibox-ubuntu", "images:ubuntu/24.04",
         "/tmp/aibox-golden-home/custom-ws/headless-1", "/tmp/aibox-golden-home/cache/ubuntu",
         "-p default -p ai-headless", "4GiB", "25GiB", 2, false, "", "Unlimited", false},
        {"ct_full_preset_unlimited_proc", "full-1", "aibox-debian", "images:debian/12",
         "/tmp/aibox-golden-home/ws/full-1", "/tmp/aibox-golden-home/cache/debian",
         "-p default -p ai-full", "8GiB", "50GiB", 0, true, "Unlimited", "2500iops", false},
        {"vm_gui_ignores_proc_limit", "vm-box", "aibox-ubuntu", "images:ubuntu/24.04",
         "/tmp/aibox-golden-home/ws/vm-box", "/tmp/aibox-golden-home/cache/ubuntu",
         "--vm -p default -p ai-gui", "8GiB", "60GiB", 4, true, "512", "200MB", true},
        {"vm_no_x11", "vm-headless", "aibox-alpine", "images:alpine/3.20",
         "/tmp/aibox-golden-home/ws/vm-headless", "/tmp/aibox-golden-home/cache/alpine",
         "--vm -p default", "2GiB", "20GiB", 2, false, "", "", true},
        {"quoted_name_and_empty_flags", "box'1", "aibox-arch", "images:archlinux",
         "/tmp/aibox-golden-home/ws/box'1", "/tmp/aibox-golden-home/cache/arch",
         "", "4GiB", "30GiB", 2, false, "1024", "100MB", false},
    };
    for (const auto &c : createCases) {
        const QString res = Backend::createEnvironmentScript(
            QString::fromUtf8(c.name),
            QString::fromUtf8(c.localAlias),
            QString::fromUtf8(c.remoteImage),
            QString::fromUtf8(c.workspace),
            QString::fromUtf8(c.cacheBase),
            QString::fromUtf8(c.profileFlags),
            QString::fromUtf8(c.ramGiB),
            QString::fromUtf8(c.diskGiB),
            c.cpuCores,
            c.x11,
            QString::fromUtf8(c.processLimit),
            QString::fromUtf8(c.ioLimit),
            c.isVm);
        out += jsonLine(QStringLiteral("Backend::createEnvironmentScript"),
                        QString::fromLatin1(c.label),
                        QJsonObject{{QStringLiteral("name"), QString::fromUtf8(c.name)},
                                    {QStringLiteral("localAlias"), QString::fromUtf8(c.localAlias)},
                                    {QStringLiteral("remoteImage"), QString::fromUtf8(c.remoteImage)},
                                    {QStringLiteral("workspace"), QString::fromUtf8(c.workspace)},
                                    {QStringLiteral("cacheBase"), QString::fromUtf8(c.cacheBase)},
                                    {QStringLiteral("profileFlags"), QString::fromUtf8(c.profileFlags)},
                                    {QStringLiteral("ramGiB"), QString::fromUtf8(c.ramGiB)},
                                    {QStringLiteral("diskGiB"), QString::fromUtf8(c.diskGiB)},
                                    {QStringLiteral("cpuCores"), c.cpuCores},
                                    {QStringLiteral("x11"), c.x11},
                                    {QStringLiteral("processLimit"), QString::fromUtf8(c.processLimit)},
                                    {QStringLiteral("ioLimit"), QString::fromUtf8(c.ioLimit)},
                                    {QStringLiteral("isVm"), c.isVm}},
                        res);
    }

    // 2. cloneEnvironmentScript matrix
    struct CloneCase {
        const char *label;
        const char *sourceName;
        const char *newName;
        const char *workspace;
        const char *diskGiB;
        const char *ramGiB;
        int cpuCores;
        bool startAfter;
        const char *netMode;
        const char *netAllowlist;
        QStringList cidrs;
        const char *processLimit;
        const char *ioLimit;
        bool isVm;
    };
    const CloneCase cloneCases[] = {
        {"online_no_start", "node-tpl", "cloned-1", "/tmp/aibox-golden-home/ws/cloned-1",
         "40GiB", "8GiB", 4, false, "online", "", {}, "1024", "100MB", false},
        {"offline_start_after", "node-tpl", "cloned-2", "/tmp/aibox-golden-home/ws/cloned-2",
         "50GiB", "16GiB", 8, true, "offline", "", {}, "Unlimited", "Unlimited", false},
        {"allowlist_vm_with_cidrs", "vm-tpl", "cloned-vm", "/tmp/aibox-golden-home/ws/cloned-vm",
         "60GiB", "8GiB", 4, true, "allowlist", "github.com, 1.1.1.1",
         {QStringLiteral("140.82.121.4/32"), QStringLiteral("1.1.1.1/32")},
         "512", "200MB", true},
        {"empty_limits_and_zero_cpu", "src'1", "dst'1", "/tmp/aibox-golden-home/ws/dst'1",
         "", "", 0, false, "", "", {}, "", "", false},
    };
    for (const auto &c : cloneCases) {
        const QString res = Backend::cloneEnvironmentScript(
            QString::fromUtf8(c.sourceName),
            QString::fromUtf8(c.newName),
            QString::fromUtf8(c.workspace),
            QString::fromUtf8(c.diskGiB),
            QString::fromUtf8(c.ramGiB),
            c.cpuCores,
            c.startAfter,
            QString::fromUtf8(c.netMode),
            QString::fromUtf8(c.netAllowlist),
            c.cidrs,
            QString::fromUtf8(c.processLimit),
            QString::fromUtf8(c.ioLimit),
            c.isVm);
        out += jsonLine(QStringLiteral("Backend::cloneEnvironmentScript"),
                        QString::fromLatin1(c.label),
                        QJsonObject{{QStringLiteral("sourceName"), QString::fromUtf8(c.sourceName)},
                                    {QStringLiteral("newName"), QString::fromUtf8(c.newName)},
                                    {QStringLiteral("workspace"), QString::fromUtf8(c.workspace)},
                                    {QStringLiteral("diskGiB"), QString::fromUtf8(c.diskGiB)},
                                    {QStringLiteral("ramGiB"), QString::fromUtf8(c.ramGiB)},
                                    {QStringLiteral("cpuCores"), c.cpuCores},
                                    {QStringLiteral("startAfter"), c.startAfter},
                                    {QStringLiteral("netMode"), QString::fromUtf8(c.netMode)},
                                    {QStringLiteral("netAllowlist"), QString::fromUtf8(c.netAllowlist)},
                                    {QStringLiteral("resolvedCidrs"), QJsonArray::fromStringList(c.cidrs)},
                                    {QStringLiteral("processLimit"), QString::fromUtf8(c.processLimit)},
                                    {QStringLiteral("ioLimit"), QString::fromUtf8(c.ioLimit)},
                                    {QStringLiteral("isVm"), c.isVm}},
                        res);
    }

    // 3. updateResourcesScript
    struct UpdateResCase {
        const char *label;
        const char *name;
        int cpuCores;
        const char *ramGiB;
        const char *diskGiB;
        const char *processLimit;
        const char *ioLimit;
        bool isVm;
    };
    const UpdateResCase updateResCases[] = {
        {"ct_all", "box1", 6, "12GiB", "80GiB", "2048", "200MB", false},
        {"vm_unlimited_io", "vm1", 4, "8GiB", "60GiB", "2048", "Unlimited", true},
        {"ct_unlimited_proc_empty_io", "box1", 2, "4GiB", "20GiB", "Unlimited", "", false},
        {"empty_and_zero_cpu", "box'1", 0, "", "", "", "", false},
    };
    for (const auto &c : updateResCases) {
        const QString res = Backend::updateResourcesScript(
            QString::fromUtf8(c.name), c.cpuCores,
            QString::fromUtf8(c.ramGiB), QString::fromUtf8(c.diskGiB),
            QString::fromUtf8(c.processLimit), QString::fromUtf8(c.ioLimit), c.isVm);
        out += jsonLine(QStringLiteral("Backend::updateResourcesScript"),
                        QString::fromLatin1(c.label),
                        QJsonObject{{QStringLiteral("name"), QString::fromUtf8(c.name)},
                                    {QStringLiteral("cpuCores"), c.cpuCores},
                                    {QStringLiteral("ramGiB"), QString::fromUtf8(c.ramGiB)},
                                    {QStringLiteral("diskGiB"), QString::fromUtf8(c.diskGiB)},
                                    {QStringLiteral("processLimit"), QString::fromUtf8(c.processLimit)},
                                    {QStringLiteral("ioLimit"), QString::fromUtf8(c.ioLimit)},
                                    {QStringLiteral("isVm"), c.isVm}},
                        res);
    }

    // 4. buildPrepareScript, startEnvironmentScript, stopEnvironmentScript
    const QString provDir = QStringLiteral("/tmp/aibox-golden-home/prov-be");
    QDir(provDir).removeRecursively();
    QDir().mkpath(provDir);
    Backend provBe(provDir, Backend::PathMode::ProjectDir);

    out += jsonLine(QStringLiteral("Backend::buildPrepareScript"),
                    QStringLiteral("no_allow_refresh"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("box1")},
                                {QStringLiteral("allowRefresh"), QString()}},
                    provBe.buildPrepareScript(QStringLiteral("box1"), QString()));
    out += jsonLine(QStringLiteral("Backend::buildPrepareScript"),
                    QStringLiteral("with_allow_refresh"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("box1")},
                                {QStringLiteral("allowRefresh"), QStringLiteral("echo refresh")}},
                    provBe.buildPrepareScript(QStringLiteral("box1"), QStringLiteral("echo refresh")));

    out += jsonLine(QStringLiteral("Backend::startEnvironmentScript"),
                    QStringLiteral("basic"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("box1")}},
                    provBe.startEnvironmentScript(QStringLiteral("box1")));
    out += jsonLine(QStringLiteral("Backend::stopEnvironmentScript"),
                    QStringLiteral("no_autosnap"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("box1")},
                                {QStringLiteral("includeAutoSnapshot"), false}},
                    provBe.stopEnvironmentScript(QStringLiteral("box1"), false));
    out += jsonLine(QStringLiteral("Backend::stopEnvironmentScript"),
                    QStringLiteral("with_autosnap_empty_env"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("box1")},
                                {QStringLiteral("includeAutoSnapshot"), true}},
                    provBe.stopEnvironmentScript(QStringLiteral("box1"), true));
    QDir(provDir).removeRecursively();

    // 5. enterIncusExecArgs, execIncusCommandArgs, buildEnterCommand
    const QByteArray prevDisplay = qgetenv("DISPLAY");
    const QByteArray prevTerm = qgetenv("TERM");
    qunsetenv("DISPLAY");
    qunsetenv("TERM");
    out += jsonLine(QStringLiteral("Backend::enterIncusExecArgs"),
                    QStringLiteral("defaults_no_env"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("box1")},
                                {QStringLiteral("remoteEnvPath"), QString()},
                                {QStringLiteral("display"), QString()},
                                {QStringLiteral("term"), QString()}},
                    QJsonArray::fromStringList(provBe.enterIncusExecArgs(QStringLiteral("box1"), QString())));
    qputenv("DISPLAY", ":1");
    qputenv("TERM", "alacritty");
    out += jsonLine(QStringLiteral("Backend::enterIncusExecArgs"),
                    QStringLiteral("custom_env_and_secret_file"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("box1")},
                                {QStringLiteral("remoteEnvPath"), QStringLiteral("/run/aibox-env'1")},
                                {QStringLiteral("display"), QStringLiteral(":1")},
                                {QStringLiteral("term"), QStringLiteral("alacritty")}},
                    QJsonArray::fromStringList(provBe.enterIncusExecArgs(QStringLiteral("box1"), QStringLiteral("/run/aibox-env'1"))));
    if (prevDisplay.isNull()) qunsetenv("DISPLAY"); else qputenv("DISPLAY", prevDisplay);
    if (prevTerm.isNull()) qunsetenv("TERM"); else qputenv("TERM", prevTerm);

    const QStringList cmdArgs = {QStringLiteral("bash"), QStringLiteral("-lc"), QStringLiteral("echo hi")};
    out += jsonLine(QStringLiteral("Backend::execIncusCommandArgs"),
                    QStringLiteral("no_remote_env"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("box1")},
                                {QStringLiteral("cmd"), QJsonArray::fromStringList(cmdArgs)},
                                {QStringLiteral("remoteEnvPath"), QString()}},
                    QJsonArray::fromStringList(provBe.execIncusCommandArgs(QStringLiteral("box1"), cmdArgs, QString())));
    out += jsonLine(QStringLiteral("Backend::execIncusCommandArgs"),
                    QStringLiteral("with_remote_env"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("box1")},
                                {QStringLiteral("cmd"), QJsonArray::fromStringList(cmdArgs)},
                                {QStringLiteral("remoteEnvPath"), QStringLiteral("/run/aibox-env")}},
                    QJsonArray::fromStringList(provBe.execIncusCommandArgs(QStringLiteral("box1"), cmdArgs, QStringLiteral("/run/aibox-env"))));

    out += jsonLine(QStringLiteral("Backend::buildEnterCommand"),
                    QStringLiteral("no_remote_env"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("box'1")},
                                {QStringLiteral("remoteEnvPath"), QString()}},
                    provBe.buildEnterCommand(QStringLiteral("box'1"), QString()));
    out += jsonLine(QStringLiteral("Backend::buildEnterCommand"),
                    QStringLiteral("with_remote_env"),
                    QJsonObject{{QStringLiteral("name"), QStringLiteral("box1")},
                                {QStringLiteral("remoteEnvPath"), QStringLiteral("/run/aibox-env'1")}},
                    provBe.buildEnterCommand(QStringLiteral("box1"), QStringLiteral("/run/aibox-env'1")));

    return out;
}
};

int main(int argc, char *argv[]) {
    QGuiApplication app(argc, argv);

    QString outDir;
    QString onlyModule;
    const QStringList args = app.arguments();
    for (int i = 1; i < args.size(); ++i) {
        if (args.at(i) == QLatin1String("--out-dir") && i + 1 < args.size()) {
            outDir = args.at(++i);
        } else if (!args.at(i).startsWith(QLatin1Char('-'))) {
            onlyModule = args.at(i);
        }
    }

    if (outDir.isEmpty()) {
        QTextStream(stderr) << "Usage: golden_capture --out-dir <dir> [module]\n";
        return 2;
    }

    struct ModuleEntry {
        const char *name;
        QByteArray (*capture)();
    };
    const ModuleEntry modules[] = {
        {"util", &GoldenCapture::captureUtil},
        {"config", &GoldenCapture::captureConfig},
        {"settings", &GoldenCapture::captureSettings},
        {"theme", &GoldenCapture::captureTheme},
        {"i18n", &GoldenCapture::captureI18n},
        {"secrets", &GoldenCapture::captureSecrets},
        {"session", &GoldenCapture::captureSession},
        {"snapshot", &GoldenCapture::captureSnapshot},
        {"diff", &GoldenCapture::captureDiff},
        {"workspace", &GoldenCapture::captureWorkspace},
        {"network", &GoldenCapture::captureNetwork},
        {"backup", &GoldenCapture::captureBackup},
        {"health", &GoldenCapture::captureHealth},
        {"provision", &GoldenCapture::captureProvision},
    };

    bool matched = onlyModule.isEmpty();
    for (const auto &m : modules) {
        const QString name = QString::fromLatin1(m.name);
        if (!onlyModule.isEmpty() && onlyModule != name)
            continue;
        matched = true;
        if (!writeModuleFile(outDir, name, m.capture())) {
            QTextStream(stderr) << "Failed to write module " << name << "\n";
            return 1;
        }
    }

    if (!matched) {
        QTextStream(stderr) << "Unknown golden module: " << onlyModule << "\n";
        return 2;
    }

    return 0;
}
