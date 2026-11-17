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
