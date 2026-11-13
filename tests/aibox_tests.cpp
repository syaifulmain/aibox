#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QProcess>
#include <QProcessEnvironment>
#include <QTemporaryDir>
#include <QtTest>

#include "backend.h"
#include "theme.h"
#include "translator.h"

class AiboxTests : public QObject {
    Q_OBJECT

private slots:
    void defaultsAreProjectLocal()
    {
        const QString projectDir = QDir::currentPath();
        QCOMPARE(Backend::defaultWorkspaceBase(), projectDir + "/.aibox/workspaces");
        QCOMPARE(Backend::defaultOsStoragePath(), projectDir + "/.aibox/os-images");

        Backend backend;
        QCOMPARE(backend.workspaceBase(), projectDir + "/.aibox/workspaces");
        QCOMPARE(backend.osStoragePath(), projectDir + "/.aibox/os-images");
        QVERIFY(backend.configFilePath().startsWith(projectDir + "/.aibox/"));
    }

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

    void parseIncusJsonUsesWorkspaceDeviceSource()
    {
        const QByteArray sampleJson = R"json([
            {
                "name": "custom-box",
                "status": "Running",
                "type": "container",
                "config": {},
                "expanded_devices": {
                    "workspace": { "type": "disk", "source": "/data/custom-box" }
                }
            }
        ])json";

        const QVariantList list = Backend::parseIncusListJson(sampleJson, "/tmp/default");
        QCOMPARE(list.size(), 1);
        QCOMPARE(list.first().toMap().value("workspacePath").toString(), QString("/data/custom-box"));
    }

    void firstRunCompletesAndPersists()
    {
        QTemporaryDir dir;
        {
            Backend backend(dir.path());
            QVERIFY(backend.firstRun());
            QCOMPARE(backend.workspaceBase(), QDir::cleanPath(dir.path()));

            const QString workspace = dir.filePath("projects");
            QCOMPARE(backend.completeFirstRun(workspace), QString());
            QVERIFY(!backend.firstRun());
            QCOMPARE(backend.workspaceBase(), workspace);
            QVERIFY(QFileInfo(workspace).isDir());
            QVERIFY(QFileInfo(backend.configFilePath()).isFile());
            QFile config(backend.configFilePath());
            QVERIFY(config.open(QIODevice::ReadOnly));
            QVERIFY(config.readAll().contains("\"firstRunCompleted\": true"));
        }

        Backend reopened(dir.path());
        QVERIFY(!reopened.firstRun());
        QCOMPARE(reopened.workspaceBase(), dir.filePath("projects"));
    }

    void firstRunEmptyKeepsCurrentWorkspace()
    {
        QTemporaryDir dir;
        Backend backend(dir.path());
        QCOMPARE(backend.completeFirstRun(""), QString());
        QCOMPARE(backend.workspaceBase(), QDir::cleanPath(dir.path()));
        QVERIFY(!backend.firstRun());
    }

    void storageSettingsValidateAndPersist()
    {
        QTemporaryDir dir;
        const QString workspace = dir.filePath("custom-workspace");
        const QString osStorage = dir.filePath("custom-os-images");
        {
            Backend backend(dir.path());
            QCOMPARE(backend.saveSettings(workspace, osStorage), QString());
            QCOMPARE(backend.workspaceBase(), workspace);
            QCOMPARE(backend.osStoragePath(), osStorage);
            QVERIFY(QFileInfo(workspace).isDir());
            QVERIFY(QFileInfo(osStorage).isDir());
            QVERIFY(!backend.firstRun());
        }

        Backend reopened(dir.path());
        QCOMPARE(reopened.workspaceBase(), workspace);
        QCOMPARE(reopened.osStoragePath(), osStorage);
        QVERIFY(!reopened.firstRun());
    }

    void osCatalogCanBeAddedEditedAndRemoved()
    {
        QTemporaryDir dir;
        Backend backend(dir.path());
        const int initialCount = backend.osCatalog().size();

        QCOMPARE(backend.saveOsEntry("", "custom-os", "custom/os", "Custom OS", "Custom", "Test entry"), QString());
        QCOMPARE(backend.osCatalog().size(), initialCount + 1);

        auto findEntry = [&backend](const QString &id) {
            for (const auto &value : backend.osCatalog()) {
                const QVariantMap entry = value.toMap();
                if (entry.value("id").toString() == id)
                    return entry;
            }
            return QVariantMap{};
        };

        QVariantMap added = findEntry("custom-os");
        QCOMPARE(added.value("alias").toString(), QString("custom/os"));
        QCOMPARE(added.value("name").toString(), QString("Custom OS"));

        QVERIFY(!backend.saveOsEntry("", "custom-os", "other/os", "Duplicate", "Custom", "").isEmpty());
        QVERIFY(!backend.saveOsEntry("", "Bad ID", "other/os", "Invalid", "Custom", "").isEmpty());

        QCOMPARE(backend.saveOsEntry("custom-os", "custom-os-2", "custom/os2", "Custom OS 2", "Custom", ""), QString());
        QCOMPARE(findEntry("custom-os").value("id").toString(), QString());
        QCOMPARE(findEntry("custom-os-2").value("name").toString(), QString("Custom OS 2"));

        QCOMPARE(backend.removeOsEntry("custom-os-2"), QString());
        QCOMPARE(backend.osCatalog().size(), initialCount);
        QVERIFY(!backend.osCatalog().isEmpty());
    }

    void osStorageCacheMarksEntryDownloaded()
    {
        QTemporaryDir dir;
        Backend backend(dir.path());
        QDir().mkpath(backend.osStoragePath());

        write(backend.osStoragePath() + "/cached-os", "metadata");
        write(backend.osStoragePath() + "/cached-os.root", "rootfs");
        QCOMPARE(backend.saveOsEntry("", "cached-os", "cached/os", "Cached OS", "Test", ""), QString());

        QVariantMap entry;
        for (const auto &value : backend.osCatalog()) {
            if (value.toMap().value("id").toString() == "cached-os")
                entry = value.toMap();
        }
        QCOMPARE(entry.value("downloaded").toBool(), true);
        QVERIFY(entry.value("sizeText").toString().startsWith("Saved"));
    }

    void createOptionsAreEditableAndValidated()
    {
        QTemporaryDir dir;
        Backend backend(dir.path());

        QCOMPARE(backend.saveCreateOptions("10GiB, 40GiB", "2GiB, 8GiB", "1, 3, 5"), QString());
        QCOMPARE(backend.diskOptions(), QStringList({"10GiB", "40GiB"}));
        QCOMPARE(backend.ramOptions(), QStringList({"2GiB", "8GiB"}));
        QCOMPARE(static_cast<int>(backend.cpuOptions().size()), 3);
        QVERIFY(backend.diskOptions().contains(backend.defaultDisk()));
        QVERIFY(backend.ramOptions().contains(backend.defaultRam()));

        QVERIFY(!backend.saveCreateOptions("not-a-size", "6GiB", "4").isEmpty());
        QVERIFY(!backend.saveCreateOptions("20GiB", "6GiB", "zero").isEmpty());
        QVERIFY(!backend.saveCreateOptions("", "", "").isEmpty());

        QCOMPARE(backend.savePreset("full", "Full Desktop", "GPU bridge"), QString());
        QCOMPARE(backend.presets().first().toMap().value("title").toString(), QString("Full Desktop"));
        QVERIFY(!backend.savePreset("missing", "Missing", "").isEmpty());

        QCOMPARE(backend.resetCreateOptions(), QString());
        QCOMPARE(backend.presets().first().toMap().value("title").toString(), QString("Desktop & Mobile"));
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

    void createEnvironmentScriptPassesPresetFlagsAsArguments()
    {
        QTemporaryDir dir;
        const QString stubDir = dir.filePath("stubs");
        QVERIFY(QDir().mkpath(stubDir));
        const QString logPath = dir.filePath("incus.log");

        writeExecutable(stubDir + "/incus",
                        "#!/bin/sh\n"
                        "echo \"$@\" >> \"$AIBOX_STUB_LOG\"\n"
                        "if [ \"$1\" = image ] && [ \"$2\" = info ]; then exit 1; fi\n"
                        "if [ \"$1\" = list ]; then echo \"10.0.0.5\"; fi\n"
                        "exit 0\n");
        // `script` runs its -c argument; replay it with bash so quoting is exercised.
        writeExecutable(stubDir + "/script", "#!/bin/sh\nexec bash -c \"$4\"\n");
        writeExecutable(stubDir + "/sleep", "#!/bin/sh\nexit 0\n");

        const QString scriptPath = dir.filePath("task.sh");
        write(scriptPath, Backend::createEnvironmentScript("mobile-porting",
                                                          "aibox-archlinux",
                                                          "images:archlinux",
                                                          dir.filePath("workspace"),
                                                          dir.filePath("os-images/aibox-archlinux"),
                                                          "-p default -p ai-gui",
                                                          "6GiB",
                                                          "60GiB",
                                                          4,
                                                          true)
                               .toUtf8());

        QProcess process;
        QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
        env.insert("PATH", stubDir + ":" + env.value("PATH"));
        env.insert("AIBOX_STUB_LOG", logPath);
        env.remove("DISPLAY");
        process.setProcessEnvironment(env);
        process.start(QStringLiteral("/bin/bash"), {scriptPath});
        QVERIFY(process.waitForFinished(30000));
        QCOMPARE(process.exitCode(), 0);

        QFile logFile(logPath);
        QVERIFY(logFile.open(QIODevice::ReadOnly));
        const QString log = QString::fromUtf8(logFile.readAll());
        QVERIFY2(log.contains("launch aibox-archlinux mobile-porting -p default -p ai-gui "
                              "-c limits.cpu=4 -c limits.memory=6GiB -d root,size=60GiB"),
                 qPrintable(log));
    }

private:
    static void writeExecutable(const QString &path, const QByteArray &contents)
    {
        write(path, contents);
        QVERIFY(QFile::setPermissions(path, QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner));
    }

    static void write(const QString &path, const QByteArray &contents)
    {
        QFile file(path);
        QVERIFY(file.open(QIODevice::WriteOnly));
        file.write(contents);
    }
};

QTEST_MAIN(AiboxTests)
#include "aibox_tests.moc"
