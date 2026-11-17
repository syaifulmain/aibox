#include <QCoreApplication>
#include <QDir>
#include <QFile>
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
        {QStringLiteral("negative"), -100},
        {QStringLiteral("bytes_512"), 512},
        {QStringLiteral("kib_1_5"), 1536},
        {QStringLiteral("mib_1"), 1024LL * 1024LL},
        {QStringLiteral("mib_42"), 42LL * 1024LL * 1024LL},
        {QStringLiteral("gib_1_5"), 1536LL * 1024LL * 1024LL},
    };
    for (const auto &c : formatBytesCases) {
        QJsonObject args;
        args.insert(QStringLiteral("bytes"), static_cast<double>(c.second));
        out += jsonLine(QStringLiteral("formatBytes"), c.first, args, formatBytes(c.second));
    }

    const QList<QPair<QString, QString>> sanitizeCases = {
        {QStringLiteral("simple"), QStringLiteral("my-env")},
        {QStringLiteral("spaces_and_caps"), QStringLiteral("  My Cool Env!  ")},
        {QStringLiteral("leading_trailing_dashes"), QStringLiteral("---foo--bar---")},
        {QStringLiteral("only_symbols"), QStringLiteral("@@@")},
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
    };
    for (const auto &c : quoteCases) {
        QJsonObject args;
        args.insert(QStringLiteral("value"), c.second);
        out += jsonLine(QStringLiteral("Backend::shellQuote"), c.first, args,
                        Backend::shellQuote(c.second));
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
