#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSaveFile>
#include <QStringList>
#include <QTextStream>

#include "backend.h"
#include "backend_helpers.h"

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
};

int main(int argc, char *argv[]) {
    QCoreApplication app(argc, argv);

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
