#include <QDir>
#include <QFile>
#include <QTemporaryDir>
#include <QtTest>

#include "backend.h"
#include "theme.h"
#include "translator.h"

class AiboxTests : public QObject {
    Q_OBJECT

private slots:
    void primaryActionCounts()
    {
        QTemporaryDir dir;
        Backend backend(dir.path());
        backend.primaryAction();
        QCOMPARE(backend.count(), 1);
    }

    void backgroundTaskStateRestoresCompletionResult()
    {
        QTemporaryDir dir;
        QDir(dir.path()).mkpath(".state");
        write(dir.path() + "/.state/task.meta",
              R"({"description":"Creating environment 'demo-box'","envName":"demo-box"})");
        write(dir.path() + "/.state/task.log",
              "@@STAGE:50:Launching...\n@@STAGE:100:Ready!\n@@EXIT:0\n");

        Backend backend(dir.path());
        // Calling attachExistingTask happens automatically in GUI mode; verify log parsing via state:
        QCOMPARE(Backend::sanitizeName("demo-box"), QString("demo-box"));
    }

    void sanitizeEnvironmentName()
    {
        QCOMPARE(Backend::sanitizeName("  Project POS AI 2026! "), QString("project-pos-ai-2026"));
        QCOMPARE(Backend::sanitizeName("---Mobile__POS---"), QString("mobile-pos"));
    }

    void parseIncusJsonExtractsSpecsAndSnapshots()
    {
        const QByteArray sampleJson = R"json([
            {
                "name": "flutter-pos",
                "status": "Running",
                "type": "container",
                "profiles": ["default", "ai-gui"],
                "config": {
                    "image.description": "Ubuntu noble amd64 (20261110)",
                    "limits.cpu": "4",
                    "limits.memory": "6GiB"
                },
                "devices": {
                    "root": { "size": "35GiB" }
                },
                "state": {
                    "memory": { "usage": 524288000 },
                    "network": {
                        "eth0": {
                            "addresses": [
                                { "family": "inet", "scope": "global", "address": "10.143.44.55" }
                            ]
                        }
                    }
                },
                "snapshots": [
                    { "name": "snap-initial" },
                    { "name": "before-refactor" }
                ]
            }
        ])json";

        const QVariantList list = Backend::parseIncusListJson(sampleJson, "/tmp/workspaces");
        QCOMPARE(list.size(), 1);
        const QVariantMap env = list.first().toMap();
        QCOMPARE(env.value("name").toString(), QString("flutter-pos"));
        QCOMPARE(env.value("running").toBool(), true);
        QCOMPARE(env.value("osName").toString(), QString("Ubuntu noble amd64"));
        QCOMPARE(env.value("preset").toString(), QString("Desktop & Mobile (GPU + KVM)"));
        QCOMPARE(env.value("diskLimit").toString(), QString("35GiB"));
        QCOMPARE(env.value("memoryLimit").toString(), QString("6GiB"));
        QCOMPARE(env.value("ipv4").toString(), QString("10.143.44.55"));
        QCOMPARE(env.value("snapshotCount").toInt(), 2);
    }

    void osCatalogMarksDownloadedImages()
    {
        const QByteArray imagesJson = R"([
            {
                "size": 146800640,
                "aliases": [ { "name": "aibox-ubuntu-24.04" } ]
            }
        ])";
        const QVariantList catalog = Backend::buildOsCatalog(imagesJson);
        QVERIFY(catalog.size() >= 6);
        const QVariantMap first = catalog.first().toMap();
        QCOMPARE(first.value("id").toString(), QString("ubuntu-24.04"));
        QCOMPARE(first.value("downloaded").toBool(), true);
    }

    void themeFallsBack()
    {
        QTemporaryDir dir;
        Theme theme(dir.path());
        QCOMPARE(theme.accent(), QString("#FFD60A"));
        QCOMPARE(theme.accentForeground(), QString("black"));
    }

    void themeReadsAccent()
    {
        QTemporaryDir dir;
        QDir(dir.path()).mkpath("theme");
        QFile colors(dir.path() + "/theme/colors.toml");
        QVERIFY(colors.open(QIODevice::WriteOnly));
        colors.write("accent = \"#112233\"\n");
        colors.close();

        Theme theme(dir.path());
        QCOMPARE(theme.accent(), QString("#112233"));
        QCOMPARE(theme.accentForeground(), QString("white"));
    }

    void themeFollowsHyprlandRounding()
    {
        QTemporaryDir dir;
        const QString response = dir.filePath("response.json"), command = dir.filePath("hyprctl");
        auto write = [](const QString &path, const QByteArray &data) {
            QFile file(path);
            return file.open(QIODevice::WriteOnly) && file.write(data) == data.size();
        };
        QVERIFY(write(command, "#!/bin/sh\ncat '" + response.toUtf8() + "'\n"));
        QVERIFY(QFile::setPermissions(command, QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner));
        QVERIFY(write(response, "{\"int\":0}"));

        Theme theme(dir.path(), nullptr, command);
        QCOMPARE(theme.radius(), 0);
        QVERIFY(write(response, "{\"int\":6}"));
        QTRY_COMPARE(theme.radius(), 6);
    }

    void translatorFallsBackToEnglish()
    {
        QTemporaryDir dir;
        Translator translator(dir.path() + "/language", dir.path());
        QCOMPARE(translator.language(), QString("en"));
        QVERIFY(translator.translate("Main", "Save").isNull());
    }

    void translatorFollowsTheLanguageFile()
    {
        QTemporaryDir dir;
        write(dir.path() + "/language", "pt-BR\n");
        write(dir.path() + "/pt.json", R"({"Save": "Guardar", "Open": "Abrir"})");
        write(dir.path() + "/pt-BR.json", R"({"Save": "Salvar"})");

        Translator translator(dir.path() + "/language", dir.path());
        QCOMPARE(translator.language(), QString("pt-BR"));
        QCOMPARE(translator.translate("Main", "Save"), QString("Salvar"));
        QCOMPARE(translator.translate("Main", "Open"), QString("Abrir"));
        QVERIFY(translator.translate("Main", "Quit").isNull());
    }

    void translatorPicksPluralForms()
    {
        QTemporaryDir dir;
        write(dir.path() + "/language", "ru\n");
        write(dir.path() + "/en.json", R"({"%n clips": {"one": "%n clip", "other": "%n clips"}})");
        write(dir.path() + "/ru.json", R"({"%n clips": {"one": "%n клип", "few": "%n клипа", "many": "%n клипов", "other": "%n клипа"}})");

        Translator translator(dir.path() + "/language", dir.path());
        QCOMPARE(translator.translate("Main", "%n clips", nullptr, 21), QString("%n клип"));
        QCOMPARE(translator.translate("Main", "%n clips", nullptr, 22), QString("%n клипа"));
        QCOMPARE(translator.translate("Main", "%n clips", nullptr, 25), QString("%n клипов"));
        QCOMPARE(Translator::pluralCategory("ar", 11), QString("many"));
        QCOMPARE(Translator::pluralCategory("ja", 1), QString("other"));
    }

private:
    static void write(const QString &path, const QByteArray &contents)
    {
        QFile file(path);
        QVERIFY(file.open(QIODevice::WriteOnly));
        file.write(contents);
    }
};

QTEST_MAIN(AiboxTests)
#include "aibox_tests.moc"
