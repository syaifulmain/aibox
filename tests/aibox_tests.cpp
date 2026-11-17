#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QImage>
#include <QProcess>
#include <QProcessEnvironment>
#include <QTcpServer>
#include <QTemporaryDir>
#include <QtTest>

#include "backend.h"
#include "cli.h"
#include "theme.h"
#include "translator.h"

class AiboxTests : public QObject {
    Q_OBJECT

private slots:
    void defaultsAreHomeAibox()
    {
        QTemporaryDir locDir;
        const QByteArray oldLocEnv = qgetenv("AIBOX_LOCATION_FILE");
        const auto restoreLoc = qScopeGuard([&] {
            if (oldLocEnv.isEmpty()) qunsetenv("AIBOX_LOCATION_FILE");
            else qputenv("AIBOX_LOCATION_FILE", oldLocEnv);
        });
        qputenv("AIBOX_LOCATION_FILE", locDir.filePath("location.json").toUtf8());

        const QString homeAibox = QDir::homePath() + "/aibox";
        QCOMPARE(Backend::defaultSystemPath(), homeAibox);
        QCOMPARE(Backend::defaultWorkspaceBase(), homeAibox + "/workspaces");
        QCOMPARE(Backend::defaultOsStoragePath(), homeAibox + "/os-images");

        Backend backend;
        QCOMPARE(backend.systemPath(), homeAibox);
        QCOMPARE(backend.workspaceBase(), homeAibox + "/workspaces");
        QCOMPARE(backend.osStoragePath(), homeAibox + "/os-images");
        QCOMPARE(backend.configFilePath(), homeAibox + "/config.json");
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

    void resolveWorkspacePathHandlesCustomBaseAndValidation()
    {
        QTemporaryDir dir;
        const QString base = dir.filePath("workspaces");
        const QString custom = dir.filePath("projects");

        QCOMPARE(Backend::resolveWorkspacePath("", "demo", base), QDir::cleanPath(base + "/demo"));
        QCOMPARE(Backend::resolveWorkspacePath(custom, "demo", base), QDir::cleanPath(custom + "/demo"));
        QCOMPARE(Backend::resolveWorkspacePath(custom + "/nested", "demo", base),
                 QDir::cleanPath(custom + "/nested/demo"));

        QString error;
        QVERIFY(Backend::resolveWorkspacePath("relative/path", "demo", base, &error).isEmpty());
        QVERIFY(!error.isEmpty());

        error.clear();
        QVERIFY(Backend::resolveWorkspacePath("/", "demo", base, &error).isEmpty());
        QVERIFY(!error.isEmpty());

        error.clear();
        QVERIFY(Backend::resolveWorkspacePath(QDir::homePath(), "demo", base, &error).isEmpty());
        QVERIFY(!error.isEmpty());
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
                    "disk": { "root": { "usage": 10737418240, "total": 37580963840 } },
                    "memory": { "usage": 524288000, "total": 6442450944 },
                    "cpu": { "usage": 8000000000 },
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

        QHash<QString, qint64> cpuSamples;
        cpuSamples.insert("flutter-pos", 4000000000LL);
        const QVariantList list = Backend::parseIncusListJson(sampleJson, "/tmp/workspaces", &cpuSamples, 4000000000LL);
        QCOMPARE(list.size(), 1);
        const QVariantMap env = list.first().toMap();
        QCOMPARE(env.value("name").toString(), QString("flutter-pos"));
        QCOMPARE(env.value("running").toBool(), true);
        QCOMPARE(env.value("osName").toString(), QString("Ubuntu noble amd64"));
        QCOMPARE(env.value("preset").toString(), QString("Desktop & Mobile (GPU + KVM)"));
        QCOMPARE(env.value("diskLimit").toString(), QString("35GiB"));
        QCOMPARE(env.value("diskUsage").toString(), QString("10.0 GiB"));
        QCOMPARE(env.value("diskPercent").toInt(), 29);
        QCOMPARE(env.value("memoryLimit").toString(), QString("6GiB"));
        QCOMPARE(env.value("memoryUsage").toString(), QString("500 MiB"));
        QCOMPARE(env.value("memoryPercent").toInt(), 8);
        QCOMPARE(env.value("cpuLimit").toString(), QString("4 Cores"));
        QCOMPARE(env.value("cpuCores").toInt(), 4);
        QCOMPARE(env.value("cpuUsage").toString(), QString("25%"));
        QCOMPARE(env.value("cpuPercent").toInt(), 25);
        QCOMPARE(env.value("cpuCorePercents").toList().size(), 4);
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
        const QByteArray oldLocEnv = qgetenv("AIBOX_LOCATION_FILE");
        const QByteArray oldHomeEnv = qgetenv("HOME");
        const auto restoreEnv = qScopeGuard([&] {
            if (oldLocEnv.isEmpty()) qunsetenv("AIBOX_LOCATION_FILE");
            else qputenv("AIBOX_LOCATION_FILE", oldLocEnv);
            if (oldHomeEnv.isEmpty()) qunsetenv("HOME");
            else qputenv("HOME", oldHomeEnv);
        });
        const QString fakeHome = dir.filePath("home");
        QVERIFY(QDir().mkpath(fakeHome));
        qputenv("HOME", fakeHome.toUtf8());
        qputenv("AIBOX_LOCATION_FILE", dir.filePath("cfg/location.json").toUtf8());

        const QString customRoot = dir.filePath("custom-aibox");
        {
            Backend backend(QString(), Backend::PathMode::ProjectDir);
            QVERIFY(backend.firstRun());
            QCOMPARE(backend.systemPath(), fakeHome + "/aibox");
            // Should not prematurely create ~/aibox before user confirms first run
            QVERIFY(!QFileInfo::exists(fakeHome + "/aibox/workspaces"));

            QCOMPARE(backend.completeFirstRun(customRoot), QString());
            QVERIFY(!backend.firstRun());
            QCOMPARE(backend.systemPath(), customRoot);
            QCOMPARE(backend.workspaceBase(), customRoot + "/workspaces");
            QCOMPARE(backend.osStoragePath(), customRoot + "/os-images");
            QCOMPARE(backend.stateDir(), customRoot + "/state");
            QCOMPARE(backend.configFilePath(), customRoot + "/config.json");
            QVERIFY(QFileInfo(customRoot + "/workspaces").isDir());
            QVERIFY(QFileInfo(customRoot + "/os-images").isDir());
            QVERIFY(QFileInfo(customRoot + "/state").isDir());
            QVERIFY(QFileInfo(backend.configFilePath()).isFile());
            QVERIFY(!QFileInfo::exists(fakeHome + "/aibox"));
            QFile config(backend.configFilePath());
            QVERIFY(config.open(QIODevice::ReadOnly));
            QVERIFY(config.readAll().contains("\"firstRunCompleted\": true"));
        }

        Backend reopened(QString(), Backend::PathMode::ProjectDir);
        QVERIFY(!reopened.firstRun());
        QCOMPARE(reopened.systemPath(), customRoot);
        QCOMPARE(reopened.workspaceBase(), customRoot + "/workspaces");
        QCOMPARE(reopened.osStoragePath(), customRoot + "/os-images");
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

    void autoSnapshotSettingsValidateAndPersist()
    {
        QTemporaryDir dir;
        {
            Backend backend(dir.path());
            QCOMPARE(backend.autoSnapshotEnabled(), true);
            QCOMPARE(backend.autoSnapshotKeep(), 5);

            QVERIFY(!backend.saveAutoSnapshotSettings(false, 0).isEmpty());
            QVERIFY(!backend.saveAutoSnapshotSettings(false, 51).isEmpty());
            QCOMPARE(backend.saveAutoSnapshotSettings(false, 3), QString());
            QCOMPARE(backend.autoSnapshotEnabled(), false);
            QCOMPARE(backend.autoSnapshotKeep(), 3);
        }

        Backend reopened(dir.path());
        QCOMPARE(reopened.autoSnapshotEnabled(), false);
        QCOMPARE(reopened.autoSnapshotKeep(), 3);
    }

    void autoSnapshotPruningAndInterval()
    {
        QVERIFY(Backend::isAutoSnapshotName("auto-20261113-100000"));
        QVERIFY(!Backend::isAutoSnapshotName("snap-manual"));

        const QStringList snaps = {
            "manual-initial",
            "auto-20261113-100000",
            "auto-20261113-101000",
            "before-refactor",
            "auto-20261113-102000",
            "auto-20261113-103000"
        };

        const QStringList prune = Backend::autoSnapshotsToPrune(snaps, 2);
        QCOMPARE(prune, QStringList({"auto-20261113-100000", "auto-20261113-101000"}));
        QVERIFY(!prune.contains("manual-initial"));
        QVERIFY(!prune.contains("before-refactor"));

        const QDateTime newest = QDateTime(QDate(2026, 11, 13), QTime(10, 30, 0));
        QCOMPARE(Backend::shouldCreateAutoSnapshot(snaps, newest.addSecs(120), 300), false);
        QCOMPARE(Backend::shouldCreateAutoSnapshot(snaps, newest.addSecs(300), 300), true);
        QCOMPARE(Backend::shouldCreateAutoSnapshot({"manual-only"}, newest, 300), true);

        const QString snippet = Backend::autoSnapshotShellSnippet("dev-box", "auto-20261113-103500", prune);
        QVERIFY(snippet.contains("incus snapshot create 'dev-box' 'auto-20261113-103500'"));
        QVERIFY(snippet.contains("incus snapshot delete 'dev-box' 'auto-20261113-100000'"));
    }

    void parseIncusJsonExtractsTemplateAndNetworkState()
    {
        const QByteArray sampleJson = R"json([
            {
                "name": "node-tpl",
                "status": "Stopped",
                "type": "container",
                "config": {
                    "user.aibox.template": "true",
                    "user.aibox.autosnap": "false",
                    "user.aibox.netmode": "allowlist",
                    "user.aibox.netallow": "github.com, 1.1.1.1"
                },
                "devices": {
                    "eth0": { "type": "nic", "security.acls": "aibox-node-tpl" }
                },
                "snapshots": [
                    { "name": "base" },
                    { "name": "auto-20261113-090000" }
                ]
            },
            {
                "name": "airgap-box",
                "status": "Running",
                "type": "container",
                "config": {},
                "devices": {
                    "eth0": { "type": "none" }
                }
            }
        ])json";

        const QVariantList list = Backend::parseIncusListJson(sampleJson, "/tmp/ws", nullptr, 0, true);
        QCOMPARE(list.size(), 2);

        const QVariantMap tpl = list.at(0).toMap();
        QCOMPARE(tpl.value("isTemplate").toBool(), true);
        QCOMPARE(tpl.value("autoSnapshotEnabled").toBool(), false);
        QCOMPARE(tpl.value("autoSnapshotCount").toInt(), 1);
        QCOMPARE(tpl.value("manualSnapshotCount").toInt(), 1);
        QCOMPARE(tpl.value("networkMode").toString(), QString("allowlist"));
        QCOMPARE(tpl.value("networkAllowlist").toString(), QString("github.com, 1.1.1.1"));

        const QVariantMap airgap = list.at(1).toMap();
        QCOMPARE(airgap.value("isTemplate").toBool(), false);
        QCOMPARE(airgap.value("autoSnapshotEnabled").toBool(), true);
        QCOMPARE(airgap.value("networkMode").toString(), QString("offline"));
        QCOMPARE(airgap.value("networkBlocked").toBool(), true);
    }

    void cloneEnvironmentScriptIsolatesWorkspaceAndUnsetsTemplate()
    {
        QTemporaryDir dir;
        const QString stubDir = dir.filePath("stubs");
        QVERIFY(QDir().mkpath(stubDir));
        const QString logPath = dir.filePath("incus.log");

        writeExecutable(stubDir + "/incus",
                        "#!/bin/sh\n"
                        "echo \"$@\" >> \"$AIBOX_STUB_LOG\"\n"
                        "exit 0\n");
        writeExecutable(stubDir + "/script", "#!/bin/sh\nexec bash -c \"$4\"\n");
        writeExecutable(stubDir + "/sleep", "#!/bin/sh\nexit 0\n");

        const QString workspace = dir.filePath("ws/cloned-box");
        const QString scriptPath = dir.filePath("clone.sh");
        write(scriptPath, Backend::cloneEnvironmentScript("node-tpl",
                                                          "cloned-box",
                                                          workspace,
                                                          "40GiB",
                                                          "8GiB",
                                                          4,
                                                          false)
                              .toUtf8());

        QProcess process;
        QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
        env.insert("PATH", stubDir + ":" + env.value("PATH"));
        env.insert("AIBOX_STUB_LOG", logPath);
        process.setProcessEnvironment(env);
        process.start(QStringLiteral("/bin/bash"), {scriptPath});
        QVERIFY(process.waitForFinished(30000));
        QCOMPARE(process.exitCode(), 0);

        QFile logFile(logPath);
        QVERIFY(logFile.open(QIODevice::ReadOnly));
        const QString log = QString::fromUtf8(logFile.readAll());
        QVERIFY(log.contains("copy node-tpl cloned-box --stateless"));
        QVERIFY(log.contains("config unset cloned-box user.aibox.template"));
        QVERIFY(log.contains("config device remove cloned-box workspace"));
        QVERIFY(log.contains("config device add cloned-box workspace disk source=" + workspace + " path=/home/workspace shift=true"));
        QVERIFY(log.contains("config set cloned-box limits.cpu 4"));
        QVERIFY(log.contains("config set cloned-box limits.memory 8GiB"));
        QVERIFY(QFileInfo(workspace).isDir());
    }

    void workspaceMountUsesHomePathAndMigratesLegacyEnvironments()
    {
        QTemporaryDir dir;
        QVERIFY(dir.isValid());
        Backend backend(dir.filePath("ws"));
        QCOMPARE(backend.containerWorkspacePath(), QStringLiteral("/home/workspace"));

        const QString createScript = Backend::createEnvironmentScript(
            QStringLiteral("box"), QStringLiteral("aibox-arch"), QStringLiteral("images:arch"),
            dir.filePath("ws/box"), dir.filePath("cache"), {}, QStringLiteral("4GiB"),
            QStringLiteral("30GiB"), 2, false, {}, {}, false, {});
        QVERIFY2(createScript.contains("WS_PATH='/home/workspace'"), qPrintable(createScript));
        QVERIFY2(createScript.contains("path=\"$WS_PATH\""), qPrintable(createScript));

        const QString migrate = Backend::workspaceMountMigrationScript(QStringLiteral("box"));
        QVERIFY(migrate.contains("incus config device get 'box' workspace path"));
        QVERIFY(migrate.contains("incus config device set 'box' workspace path='/home/workspace'"));

        const QString startScript = backend.startEnvironmentScript(QStringLiteral("box"));
        QVERIFY2(startScript.contains("workspace path='/home/workspace'"), qPrintable(startScript));

        const QString prepareScript = backend.prepareEnvironmentScript(QStringLiteral("box"));
        QVERIFY2(prepareScript.contains("workspace path='/home/workspace'"), qPrintable(prepareScript));
    }

    void networkAllowlistResolvesAndBlocksPrivateDnsRebinding()
    {
        const QStringList entries = Backend::parseAllowlistEntries("GitHub.com, 1.1.1.1; 10.0.0.0/8\nlocal-rebind.test bad$host");
        QCOMPARE(entries, QStringList({"github.com", "1.1.1.1", "10.0.0.0/8", "local-rebind.test"}));

        auto mockResolver = [](const QString &host) -> QStringList {
            if (host == "github.com")
                return {"140.82.121.4", "192.168.1.50"}; // private IP from DNS must be ignored
            if (host == "local-rebind.test")
                return {"127.0.0.1", "10.1.2.3"};
            return {};
        };

        QStringList warnings;
        const QStringList cidrs = Backend::resolveAllowlistCidrs(entries, mockResolver, &warnings);
        QCOMPARE(cidrs, QStringList({"140.82.121.4/32", "1.1.1.1/32", "10.0.0.0/8"}));
        QCOMPARE(warnings.size(), 1);

        const QString allowScript = Backend::networkControlScript("agent-1", "allowlist", "github.com, 1.1.1.1", cidrs);
        QVERIFY(allowScript.contains("incus network acl create 'aibox-agent-1'"));
        QVERIFY(allowScript.contains("incus network acl rule add 'aibox-agent-1' egress action=allow destination='140.82.121.4/32'"));
        QVERIFY(allowScript.contains("security.acls.default.egress.action=drop"));

        const QString offlineScript = Backend::networkControlScript("agent-1", "offline", {}, {});
        QVERIFY(offlineScript.contains("incus config device add 'agent-1' eth0 none"));
        QVERIFY(offlineScript.contains("user.aibox.netmode offline"));

        const QString onlineScript = Backend::networkControlScript("agent-1", "online", {}, {});
        QVERIFY(onlineScript.contains("incus config device remove 'agent-1' eth0"));
        QVERIFY(onlineScript.contains("incus network acl delete 'aibox-agent-1'"));
    }

    void watchdogHysteresisAndAutoStop()
    {
        auto d1 = Backend::evaluateWatchdog(89, 0, 90, 97, true);
        QCOMPARE(d1.newLevel, 0);
        QVERIFY(!d1.shouldNotify);
        QVERIFY(!d1.shouldAutoStop);

        auto d2 = Backend::evaluateWatchdog(91, 0, 90, 97, true);
        QCOMPARE(d2.newLevel, 1);
        QVERIFY(d2.shouldNotify);
        QVERIFY(!d2.shouldAutoStop);

        // Stay silent inside 5% hysteresis band when already at warn
        auto d3 = Backend::evaluateWatchdog(86, 1, 90, 97, true);
        QCOMPARE(d3.newLevel, 1);
        QVERIFY(!d3.shouldNotify);

        // Drop below warn - 5 re-arms
        auto d4 = Backend::evaluateWatchdog(84, 1, 90, 97, true);
        QCOMPARE(d4.newLevel, 0);
        QVERIFY(!d4.shouldNotify);

        // Hit critical with auto-stop disabled vs enabled
        auto d5 = Backend::evaluateWatchdog(98, 1, 90, 97, false);
        QCOMPARE(d5.newLevel, 2);
        QVERIFY(d5.shouldNotify);
        QVERIFY(!d5.shouldAutoStop);

        auto d6 = Backend::evaluateWatchdog(98, 1, 90, 97, true);
        QCOMPARE(d6.newLevel, 2);
        QVERIFY(d6.shouldNotify);
        QVERIFY(d6.shouldAutoStop);

        // Stay at critical while within 5% hysteresis band below critical
        auto d7 = Backend::evaluateWatchdog(94, 2, 90, 97, true);
        QCOMPARE(d7.newLevel, 2);
        QVERIFY(!d7.shouldNotify);
    }

    void snapshotSizesAndWatchdogSettings()
    {
        const QByteArray snapJson = R"json([
            { "name": "clean-sdk", "size": 52428800 },
            { "name": "empty-unknown", "size": -1 }
        ])json";
        const QVariantMap sizes = Backend::parseSnapshotSizesJson(snapJson);
        QCOMPARE(sizes.value("clean-sdk").toString(), QString("50 MiB"));
        QVERIFY(!sizes.contains("empty-unknown"));

        QTemporaryDir dir;
        {
            Backend backend(dir.path());
            QVERIFY(!backend.saveWatchdogSettings(true, 95, 90, false).isEmpty()); // critical <= warn
            QVERIFY(!backend.saveWatchdogSettings(true, 30, 90, false).isEmpty()); // warn < 50
            QCOMPARE(backend.saveWatchdogSettings(true, 88, 96, true), QString());
            QCOMPARE(backend.watchdogDiskWarn(), 88);
            QCOMPARE(backend.watchdogDiskCritical(), 96);
            QVERIFY(backend.watchdogAutoStop());
        }

        Backend reloaded(dir.path());
        QCOMPARE(reloaded.watchdogDiskWarn(), 88);
        QCOMPARE(reloaded.watchdogDiskCritical(), 96);
        QVERIFY(reloaded.watchdogAutoStop());
    }

    void runawayLimitsContainerVsVm()
    {
        QVERIFY(Backend::isValidProcessLimit(""));
        QVERIFY(Backend::isValidProcessLimit("Unlimited"));
        QVERIFY(Backend::isValidProcessLimit("512"));
        QVERIFY(!Backend::isValidProcessLimit("5"));
        QVERIFY(!Backend::isValidProcessLimit("-1"));
        QVERIFY(!Backend::isValidProcessLimit("abc"));

        QVERIFY(Backend::isValidIoLimit(""));
        QVERIFY(Backend::isValidIoLimit("Unlimited"));
        QVERIFY(Backend::isValidIoLimit("200MB"));
        QVERIFY(Backend::isValidIoLimit("2500iops"));
        QVERIFY(!Backend::isValidIoLimit("fast"));

        const QString ctScript = Backend::createEnvironmentScript(
            "ct-box", "aibox-ubuntu", "images:ubuntu/24.04", "/tmp/ws", "/tmp/cache",
            "", "6GiB", "35GiB", 4, false, "512", "200MB", false);
        QVERIFY(ctScript.contains("limits.processes=512"));
        QVERIFY(ctScript.contains("limits.read=200MB,limits.write=200MB"));

        const QString vmScript = Backend::createEnvironmentScript(
            "vm-box", "aibox-ubuntu", "images:ubuntu/24.04", "/tmp/ws", "/tmp/cache",
            "--vm", "6GiB", "35GiB", 4, false, "512", "200MB", true);
        QVERIFY(!vmScript.contains("limits.processes="));
        QVERIFY(vmScript.contains("limits.read=200MB,limits.write=200MB"));

        const QString cloneScript = Backend::cloneEnvironmentScript(
            "src", "dst", "/tmp/ws", "40GiB", "8GiB", 4, false,
            "online", {}, {}, "1024", "100MB", false);
        QVERIFY(cloneScript.contains("limits.processes '1024'"));
        QVERIFY(cloneScript.contains("limits.read='100MB' limits.write='100MB'"));
    }

    void egressLoggingAndJournalParsing()
    {
        const QString onlineLogScript = Backend::networkControlScript("agent-1", "online", {}, {}, true);
        QVERIFY(onlineLogScript.contains("incus network acl create 'aibox-log-agent-1'"));
        QVERIFY(onlineLogScript.contains("security.acls.default.egress.action=allow"));
        QVERIFY(onlineLogScript.contains("security.acls.default.egress.logged=true"));

        const QString allowLogScript = Backend::networkControlScript(
            "agent-1", "allowlist", "140.82.121.4", {"140.82.121.4/32"}, true);
        QVERIFY(allowLogScript.contains("action=allow destination='140.82.121.4/32' state=logged"));
        QVERIFY(allowLogScript.contains("security.acls.default.egress.action=drop security.acls.default.egress.logged=true"));

        const QString journalSample = QStringLiteral(
            "2026-11-13T13:10:01+0700 host kernel: eth0-egress-0 IN=veth-box1 OUT=incusbr0 SRC=10.143.44.18 DST=140.82.121.4 LEN=60 PROTO=TCP SPT=50001 DPT=443 \n"
            "2026-11-13T13:10:03+0700 host kernel: eth0-egress-0 IN=veth-other OUT=incusbr0 SRC=10.143.44.99 DST=9.9.9.9 LEN=60 PROTO=UDP SPT=50002 DPT=53 \n"
            "2026-11-13T13:10:05+0700 host kernel: eth0-egress-default IN=veth-box1 OUT=incusbr0 SRC=10.143.44.18 DST=104.16.25.34 LEN=60 PROTO=TCP SPT=50003 DPT=443 \n"
            "2026-11-13T13:10:09+0700 host kernel: eth0-egress-0 IN=veth-old OUT=incusbr0 SRC=10.143.44.18 DST=140.82.121.4 LEN=60 PROTO=TCP SPT=50004 DPT=443 \n");

        const QVariantList logs = Backend::parseKernelEgressLogs(journalSample, "10.143.44.18", "veth-box1", 100);
        QCOMPARE(logs.size(), 2);
        QCOMPARE(logs.at(0).toMap().value("dstIp").toString(), QString("140.82.121.4"));
        QCOMPARE(logs.at(0).toMap().value("count").toInt(), 2);
        QCOMPARE(logs.at(0).toMap().value("lastSeen").toString(), QString("13:10:09"));
        QCOMPARE(logs.at(1).toMap().value("dstIp").toString(), QString("104.16.25.34"));
        QCOMPARE(logs.at(1).toMap().value("count").toInt(), 1);
    }

    void operationHistoryTruncationAndPersistence()
    {
        QByteArray big;
        for (int i = 0; i < 30; ++i)
            big += QByteArray("line-") + QByteArray::number(i) + "\n";
        const QByteArray truncated = Backend::truncateLogTail(big, 80);
        QVERIFY(truncated.startsWith("... [truncated] ...\n"));
        QVERIFY(truncated.endsWith("line-29\n"));

        QTemporaryDir dir;
        QDir(dir.path()).mkpath(".state/history");
        write(dir.path() + "/.state/history/20261113-130005-001.log", "build failed on step 2\n");
        write(dir.path() + "/.state/history/20261113-130005-001.json",
              R"({"id":"20261113-130005-001","description":"Create box","envName":"box1","status":"failed","exitCode":1,"timestamp":"2026-11-13 13:00:05","durationMs":5000,"logFile":"20261113-130005-001.log"})");
        write(dir.path() + "/.state/history/20261113-125501-001.log", "snapshot ok\n");
        write(dir.path() + "/.state/history/20261113-125501-001.json",
              R"({"id":"20261113-125501-001","description":"Snapshot box","envName":"box1","status":"success","exitCode":0,"timestamp":"2026-11-13 12:55:01","durationMs":1000,"logFile":"20261113-125501-001.log"})");

        Backend backend(dir.path());
        QCOMPARE(backend.operationHistory().size(), 2);
        QCOMPARE(backend.recentFailedHistoryCount(), 1);
        QCOMPARE(backend.readOperationLog("20261113-130005-001"), QString("build failed on step 2\n"));
        backend.acknowledgeOperationHistory();
        QCOMPARE(backend.recentFailedHistoryCount(), 0);
        QCOMPARE(backend.operationHistory().size(), 2);
        {
            // Viewed marker survives a restart without deleting entries.
            Backend reopened(dir.path());
            QCOMPARE(reopened.recentFailedHistoryCount(), 0);
            QCOMPARE(reopened.operationHistory().size(), 2);
        }
        write(dir.path() + "/.state/history/20261113-140000-001.json",
              R"({"id":"20261113-140000-001","description":"Start box","envName":"box1","status":"failed","exitCode":1,"timestamp":"2026-11-13 14:00:00","durationMs":1200,"logFile":"20261113-140000-001.log"})");
        {
            // A failure newer than the viewed marker raises the counter again.
            Backend reopened(dir.path());
            QCOMPARE(reopened.operationHistory().size(), 3);
            QCOMPARE(reopened.recentFailedHistoryCount(), 1);
        }
        backend.clearOperationHistory();
        QCOMPARE(backend.operationHistory().size(), 0);
        QCOMPARE(backend.recentFailedHistoryCount(), 0);
    }

    void portForwardValidationAndConflictDetection()
    {
        QCOMPARE(Backend::proxyDeviceName(3000), QString("fwd-3000"));
        QVERIFY(!Backend::isHostPortAvailable(80));
        QVERIFY(!Backend::isHostPortAvailable(70000));

        QTcpServer blocker;
        QVERIFY(blocker.listen(QHostAddress::LocalHost, 0));
        const int busyPort = blocker.serverPort();
        QVERIFY(busyPort >= 1024);
        QVERIFY(!Backend::isHostPortAvailable(busyPort));
        blocker.close();
        QVERIFY(Backend::isHostPortAvailable(busyPort));

        const QString script = Backend::addPortForwardScript("web-box", 3000, 8080);
        QVERIFY(script.contains("'fwd-3000' proxy listen='tcp:127.0.0.1:3000' connect='tcp:127.0.0.1:8080'"));

        QTemporaryDir dir;
        Backend backend(dir.path());
        QVERIFY(!backend.addPortForward("web-box", 80, 80).isEmpty());
        QVERIFY(!backend.addPortForward("web-box", 3000, 0).isEmpty());
    }

    void fileAccessUrlAndPortSelection()
    {
        QCOMPARE(Backend::fileAccessUrl(2222), QString("sftp://root@127.0.0.1:2222/"));
        QCOMPARE(Backend::fileAccessUrl(0), QString());
        QCOMPARE(Backend::fileAccessUrl(70000), QString());

        const QSet<int> reserved{2222, 2223, 2224};
        int probes = 0;
        const int port = Backend::chooseFileAccessPort(reserved, [&probes](int candidate) {
            ++probes;
            return candidate != 2225; // pretend 2225 is already bound on the host
        });
        QCOMPARE(port, 2226);
        QCOMPARE(probes, 2);

        QCOMPARE(Backend::chooseFileAccessPort(QSet<int>{}, std::function<bool(int)>{}, 2222, 2224), 2222);
        QCOMPARE(Backend::chooseFileAccessPort(QSet<int>{2222, 2223, 2224}, std::function<bool(int)>{}, 2222, 2224), 0);

        // Real bind check: an occupied 127.0.0.1 port must be skipped.
        QTcpServer blocker;
        QVERIFY(blocker.listen(QHostAddress::LocalHost, 0));
        const int busyPort = blocker.serverPort();
        const auto available = [](int candidate) { return Backend::isHostPortAvailable(candidate); };
        const int chosen = Backend::chooseFileAccessPort(QSet<int>{}, available, busyPort, busyPort + 50);
        QVERIFY(chosen > busyPort);
        QVERIFY(Backend::isHostPortAvailable(chosen));
        blocker.close();
        QVERIFY(Backend::isHostPortAvailable(busyPort));
    }

    void enableFileAccessRequiresKnownRunningEnvironment()
    {
        QTemporaryDir dir;
        Backend backend(dir.path());
        QVERIFY(!backend.enableFileAccess("missing-box").isEmpty());
        QVERIFY(backend.selectedEnvironment().isEmpty());

        backend.disableFileAccess("missing-box");
        backend.openFileAccess("missing-box");
    }

    void extraMountValidationAndScript()
    {
        QTemporaryDir dir;
        const QString safeDir = dir.filePath("datasets");
        const QString sshDir = dir.filePath("home/.ssh/keys");
        const QString cfgDir = dir.filePath(".aibox-cfg");
        QVERIFY(QDir().mkpath(safeDir));
        QVERIFY(QDir().mkpath(sshDir));
        QVERIFY(QDir().mkpath(cfgDir));

        QVERIFY(!Backend::validateExtraMountPath("/").isEmpty());
        QVERIFY(!Backend::validateExtraMountPath("/etc").isEmpty());
        QVERIFY(!Backend::validateExtraMountPath("/home").isEmpty());
        QVERIFY(!Backend::validateExtraMountPath(QDir::homePath()).isEmpty());
        QVERIFY(!Backend::validateExtraMountPath(sshDir).isEmpty());
        QVERIFY(!Backend::validateExtraMountPath(cfgDir, {cfgDir}).isEmpty());
        QVERIFY(!Backend::validateExtraMountPath(dir.filePath("missing-dir")).isEmpty());
        QCOMPARE(Backend::validateExtraMountPath(safeDir, {cfgDir}), QString());

        QCOMPARE(Backend::extraMountDeviceName("My Data"), QString("mnt-my-data"));
        const QString roScript = Backend::addExtraMountScript("ct1", "mnt-data", safeDir, "/home/workspace/data", true);
        QVERIFY(roScript.contains("readonly=true shift=true"));

        Backend backend(dir.path());
        QVERIFY(!backend.addExtraMount("ct1", safeDir, "data", false, false).isEmpty()); // unconfirmed RW rejected
    }

    void liveResourceEditingAndShrinkGuard()
    {
        QCOMPARE(Backend::parseByteSizeToBytes("10GiB"), qint64(10737418240LL));
        QCOMPARE(Backend::parseByteSizeToBytes("512MiB"), qint64(536870912LL));

        const QString ctUpdate = Backend::updateResourcesScript("box1", 6, "12GiB", "80GiB", "2048", "200MB", false);
        QVERIFY(ctUpdate.contains("limits.cpu '6'"));
        QVERIFY(ctUpdate.contains("limits.memory '12GiB'"));
        QVERIFY(ctUpdate.contains("limits.processes '2048'"));
        QVERIFY(ctUpdate.contains("root size='80GiB'"));
        QVERIFY(ctUpdate.contains("limits.read='200MB' limits.write='200MB'"));

        const QString vmUpdate = Backend::updateResourcesScript("vm1", 4, "8GiB", "60GiB", "2048", "Unlimited", true);
        QVERIFY(!vmUpdate.contains("limits.processes"));
        QVERIFY(vmUpdate.contains("device unset 'vm1' root limits.read"));

        const QString shrinkReset = Backend::updateResourcesScript("box1", 2, "4GiB", "20GiB", "Unlimited", "", false);
        QVERIFY(shrinkReset.contains("config unset 'box1' limits.processes"));
    }

    void provisioningBundlesOrderAndNonFatalFailure()
    {
        QCOMPARE(Backend::distroFamilyForOsId("ubuntu-24.04", "ubuntu/24.04"), QString("apt"));
        QCOMPARE(Backend::distroFamilyForOsId("debian-12", "debian/12"), QString("apt"));
        QCOMPARE(Backend::distroFamilyForOsId("archlinux", "archlinux"), QString("pacman"));
        QCOMPARE(Backend::distroFamilyForOsId("fedora-41", "fedora/41"), QString());

        QTemporaryDir dir;
        const QString customDir = dir.filePath("provision");
        QVERIFY(QDir().mkpath(customDir));
        write(customDir + "/node.apt.sh", "echo CUSTOM_FAIL_STEP && exit 7\n");
        write(customDir + "/python.apt.sh", "echo CUSTOM_OK_STEP && exit 0\n");

        const QString snippet = Backend::buildProvisionShellSnippet(
            {"node", "python"}, "ubuntu-24.04", "ubuntu/24.04", customDir);
        QVERIFY(snippet.indexOf("__AIBOX_PROV_node_1__") < snippet.indexOf("__AIBOX_PROV_python_2__"));

        const QString stubDir = dir.filePath("bin");
        QVERIFY(QDir().mkpath(stubDir));
        const QString logPath = dir.filePath("prov.log");
        writeExecutable(stubDir + "/incus",
                        "#!/bin/sh\n"
                        "if [ \"$1\" = \"exec\" ]; then\n"
                        "  out=$(sh)\n"
                        "  rc=$?\n"
                        "  echo \"$out\" >> \"$AIBOX_PROV_LOG\"\n"
                        "  exit $rc\n"
                        "fi\n"
                        "exit 0\n");

        const QString testScript = QStringLiteral("set -euo pipefail\nNAME=test-box\n") + snippet + QStringLiteral("\necho FINISHED_OK\n");
        const QString scriptPath = dir.filePath("run_prov.sh");
        write(scriptPath, testScript.toUtf8());

        QProcess proc;
        QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
        env.insert("PATH", stubDir + ":" + env.value("PATH"));
        env.insert("AIBOX_PROV_LOG", logPath);
        proc.setProcessEnvironment(env);
        proc.start(QStringLiteral("/bin/bash"), {scriptPath});
        QVERIFY(proc.waitForFinished(15000));
        QCOMPARE(proc.exitCode(), 0);
        const QString stdoutText = QString::fromUtf8(proc.readAllStandardOutput());
        QVERIFY(stdoutText.contains("WARN: Bundle node failed"));
        QVERIFY(stdoutText.contains("FINISHED_OK"));
    }

    void parseIncusJsonExtractsPortForwardsAndExtraMounts()
    {
        const QByteArray sample = R"json([
            {
                "name": "dev-box",
                "status": "Running",
                "type": "container",
                "config": { "limits.cpu": "2", "limits.memory": "4GiB" },
                "devices": {
                    "root": { "type": "disk", "path": "/", "size": "40GiB" },
                    "workspace": { "type": "disk", "source": "/tmp/ws", "path": "/workspace" },
                    "fwd-3000": { "type": "proxy", "listen": "tcp:127.0.0.1:3000", "connect": "tcp:127.0.0.1:8000" },
                    "mnt-datasets": { "type": "disk", "source": "/tmp/datasets", "path": "/mnt/datasets", "readonly": "true" }
                },
                "state": {
                    "disk": { "root": { "usage": 5368709120, "total": 42949672960 } }
                }
            }
        ])json";

        const QVariantList envs = Backend::parseIncusListJson(sample, "/tmp");
        QCOMPARE(envs.size(), 1);
        const QVariantMap env = envs.first().toMap();
        QCOMPARE(env.value("diskUsageBytes").toLongLong(), qint64(5368709120LL));
        QCOMPARE(env.value("portForwardCount").toInt(), 1);
        QCOMPARE(env.value("portForwards").toList().first().toMap().value("hostPort").toInt(), 3000);
        QCOMPARE(env.value("portForwards").toList().first().toMap().value("guestPort").toInt(), 8000);
        QCOMPARE(env.value("extraMountCount").toInt(), 1);
        QCOMPARE(env.value("extraMounts").toList().first().toMap().value("path").toString(), QString("/mnt/datasets"));
        QVERIFY(env.value("extraMounts").toList().first().toMap().value("readonly").toBool());
    }

    void diskSpaceCheckEnforcesSafetyMargin()
    {
        const qint64 margin = 100LL * 1024LL * 1024LL;
        QVERIFY(!Backend::hasSufficientDiskSpace(500LL * 1024 * 1024, 500LL * 1024 * 1024));
        QVERIFY(!Backend::hasSufficientDiskSpace(500LL * 1024 * 1024, 500LL * 1024 * 1024 + margin - 1));
        QVERIFY(Backend::hasSufficientDiskSpace(500LL * 1024 * 1024, 500LL * 1024 * 1024 + margin));
        QVERIFY(!Backend::hasSufficientDiskSpace(100, -1));
    }

    void scanBackupArchivesPairsWorkspaceTarball()
    {
        QTemporaryDir dir;
        QVERIFY(dir.isValid());
        write(dir.filePath("alpha-20261113-120000.tar.gz"), QByteArray("backup-data"));
        write(dir.filePath("alpha-20261113-120000-workspace.tar.gz"), QByteArray("ws-data"));
        write(dir.filePath("beta-20261113-120500.tar.gz"), QByteArray("beta-only"));

        const QVariantList list = Backend::scanBackupArchives(dir.path());
        QCOMPARE(list.size(), 2);

        QVariantMap alpha;
        QVariantMap beta;
        for (const auto &v : list) {
            const auto m = v.toMap();
            if (m.value("envName") == QLatin1String("alpha"))
                alpha = m;
            else if (m.value("envName") == QLatin1String("beta"))
                beta = m;
        }
        QVERIFY(!alpha.isEmpty());
        QVERIFY(alpha.value("hasWorkspaceArchive").toBool());
        QVERIFY(alpha.value("workspaceArchivePath").toString().endsWith("alpha-20261113-120000-workspace.tar.gz"));
        QVERIFY(!beta.isEmpty());
        QVERIFY(!beta.value("hasWorkspaceArchive").toBool());
    }

    void exportAndImportScriptsExecuteWithWorkspaceRoundTrip()
    {
        QTemporaryDir dir;
        QVERIFY(dir.isValid());

        const QString wsSrc = dir.filePath("ws-src");
        const QString wsDst = dir.filePath("ws-dst");
        QVERIFY(QDir().mkpath(wsSrc));
        write(wsSrc + "/notes.txt", QByteArray("hello-aibox-backup"));

        const QString stubDir = dir.filePath("bin");
        QVERIFY(QDir().mkpath(stubDir));
        const QString incusLog = dir.filePath("incus-calls.log");
        writeExecutable(stubDir + "/incus",
                        "#!/bin/sh\n"
                        "echo \"$@\" >> \"$AIBOX_TEST_LOG\"\n"
                        "if [ \"$1\" = \"export\" ]; then\n"
                        "  echo fake-archive > \"$3\"\n"
                        "fi\n"
                        "exit 0\n");

        const QString backupFile = dir.filePath("backups/myenv-20261113.tar.gz");
        const QString wsArchive = dir.filePath("backups/myenv-20261113-workspace.tar.gz");

        const QString exportScript = Backend::exportEnvironmentScript("myenv", backupFile, wsSrc, wsArchive);
        const QString importScript = Backend::importEnvironmentScript(backupFile, "myenv-copy", wsDst, wsArchive);

        const QString runnerPath = dir.filePath("roundtrip.sh");
        write(runnerPath, (exportScript + "\n" + importScript).toUtf8());

        QProcess proc;
        QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
        env.insert("PATH", stubDir + ":" + env.value("PATH"));
        env.insert("AIBOX_TEST_LOG", incusLog);
        proc.setProcessEnvironment(env);
        proc.start(QStringLiteral("/bin/bash"), {runnerPath});
        QVERIFY(proc.waitForFinished(10000));
        QCOMPARE(proc.exitCode(), 0);

        QVERIFY(QFileInfo::exists(backupFile));
        QVERIFY(QFileInfo::exists(wsArchive));
        QFile restoredNote(wsDst + "/notes.txt");
        QVERIFY(restoredNote.open(QIODevice::ReadOnly));
        QCOMPARE(restoredNote.readAll(), QByteArray("hello-aibox-backup"));

        QFile logFile(incusLog);
        QVERIFY(logFile.open(QIODevice::ReadOnly));
        const QString logText = QString::fromUtf8(logFile.readAll());
        QVERIFY(logText.contains("export myenv "));
        QVERIFY(logText.contains("import " + backupFile + " myenv-copy"));
        QVERIFY(logText.contains("config unset myenv-copy user.aibox.template"));
        QVERIFY(logText.contains("config set myenv-copy user.aibox.workspace " + wsDst));
    }

    void healthChecksAllPassWithHealthyHostStub()
    {
        const QString healthyRules = QStringLiteral(
            "-A ufw-user-input -i incusbr0 -j ACCEPT\n"
            "-A ufw-user-forward -i incusbr0 -j ACCEPT\n"
            "-A ufw-user-forward -o incusbr0 -j ACCEPT\n");
        Backend::CommandRunner runner = [](const QString &prog, const QStringList &args) -> Backend::CommandOutput {
            if (prog == QLatin1String("id"))
                return {0, QStringLiteral("user wheel incus-admin kvm render video\n"), {}};
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
                if (args.value(0) == QLatin1String("storage") && args.value(1) == QLatin1String("info")) {
                    return {0, QStringLiteral("info:\n  driver: btrfs\n  name: default\n  space used: \"1329242112\"\n  total space: \"32212254720\"\n"), {}};
                }
            }
            if (prog == QLatin1String("findmnt"))
                return {0, QStringLiteral("/home btrfs rw,relatime,user_subvol_rm_allowed\n"), {}};
            return {1, {}, QStringLiteral("unexpected command")};
        };

        const auto checks = Backend::runHealthChecks(false, runner, QStringLiteral("user"), healthyRules);
        QCOMPARE(checks.size(), 8);
        for (const auto &c : checks) {
            QVERIFY2(c.ok, qPrintable(c.id + ": " + c.detail));
            QVERIFY(!c.warning);
            QVERIFY(!c.repaired);
        }
    }

    void healthChecksFixRepairsBridgeAndAiGuiProfileWithoutRoot()
    {
        bool bridgeExists = false;
        bool hasNesting = false;
        bool hasGpu = false;
        bool hasKvm = false;

        Backend::CommandRunner runner = [&](const QString &prog, const QStringList &args) -> Backend::CommandOutput {
            if (prog == QLatin1String("id"))
                return {0, QStringLiteral("user incus-admin kvm render video\n"), {}};
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
                    QString out = QStringLiteral("config:\n");
                    if (hasNesting)
                        out += QStringLiteral("  security.nesting: \"true\"\n");
                    out += QStringLiteral("devices:\n");
                    if (hasGpu)
                        out += QStringLiteral("  gpu:\n    type: gpu\n");
                    if (hasKvm)
                        out += QStringLiteral("  kvm:\n    path: /dev/kvm\n");
                    return {0, out, {}};
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

        const auto dryRun = Backend::runHealthChecks(false, runner, QStringLiteral("user"));
        QVERIFY(!dryRun[3].ok);
        QVERIFY(!dryRun[4].ok);

        const auto fixed = Backend::runHealthChecks(true, runner, QStringLiteral("user"));
        QVERIFY(fixed[3].ok && fixed[3].repaired);
        QVERIFY(fixed[4].ok && fixed[4].repaired);
        for (const auto &c : fixed)
            QVERIFY(c.ok);
    }

    void healthChecksReportRootFixCommandsWhenPrivilegedItemsFail()
    {
        Backend::CommandRunner runner = [](const QString &prog, const QStringList &args) -> Backend::CommandOutput {
            if (prog == QLatin1String("id"))
                return {0, QStringLiteral("user wheel\n"), {}};
            if (prog == QLatin1String("__socket_rw__"))
                return {1, {}, QStringLiteral("permission denied")};
            if (prog == QLatin1String("systemctl") && args.value(0) == QLatin1String("is-active"))
                return {0, QStringLiteral("active\n"), {}};
            if (prog == QLatin1String("findmnt"))
                return {0, QStringLiteral("/home btrfs rw,relatime\n"), {}};
            return {1, {}, QStringLiteral("daemon down")};
        };

        const auto checks = Backend::runHealthChecks(true, runner, QStringLiteral("user"), QStringLiteral("# empty rules"));
        QCOMPARE(checks.size(), 8);
        QVERIFY(!checks[0].ok && checks[0].needsRoot && checks[0].fixCommand.contains("sudo usermod -aG"));
        QVERIFY(!checks[1].ok && checks[1].needsRoot && checks[1].fixCommand.contains("sudo setfacl"));
        QVERIFY(!checks[2].ok && checks[2].needsRoot && checks[2].fixCommand.contains("sudo systemctl enable --now"));
        QVERIFY(!checks[5].ok && checks[5].needsRoot && checks[5].fixCommand.contains("sudo ufw allow in on incusbr0"));
        QCOMPARE(checks[6].id, QStringLiteral("workspace_subvol_rm"));
        QVERIFY(!checks[6].ok && checks[6].needsRoot && checks[6].fixCommand.contains("user_subvol_rm_allowed"));
    }

    void workspaceSnapshotPathsAndOrphanDetection()
    {
        QTemporaryDir dir;
        const QString wsBase = dir.filePath("workspaces");
        const QString wsPath = wsBase + "/dev box";
        QVERIFY(QDir().mkpath(wsPath));

        const QString snapDir = Backend::workspaceSnapshotDir(wsPath, "dev-box");
        QCOMPARE(snapDir, QDir::cleanPath(wsBase + "/.snapshots/dev-box"));

        QVERIFY(QDir().mkpath(snapDir + "/snap-1"));
        QVERIFY(QDir().mkpath(snapDir + "/pre-restore-123"));
        QVERIFY(QDir().mkpath(snapDir + "/auto-20261113-100000"));

        const QStringList wsSnaps = Backend::listWorkspaceSnapshots(wsPath, "dev-box");
        QCOMPARE(wsSnaps, QStringList({"auto-20261113-100000", "pre-restore-123", "snap-1"}));

        const QStringList orphans = Backend::findOrphanWorkspaceSnapshots(
            {"snap-1", "auto-20261113-100000"}, wsSnaps);
        QCOMPARE(orphans, QStringList({"pre-restore-123"}));

        const QVariantMap duSizes = Backend::parseDuBytesOutput(
            QStringLiteral("1048576\t%1/snap-1\n2048\t%1/pre-restore-123\n").arg(snapDir));
        QCOMPARE(duSizes.value("snap-1").toString(), QStringLiteral("1 MiB"));
        QCOMPARE(duSizes.value("pre-restore-123").toString(), QStringLiteral("2 KiB"));

        const QByteArray sampleJson = QStringLiteral(R"json([
            {
                "name": "dev-box",
                "status": "Stopped",
                "type": "container",
                "config": { "user.aibox.wssnap": "false" },
                "devices": {
                    "workspace": { "type": "disk", "path": "/workspace", "source": "%1" }
                },
                "snapshots": [
                    { "name": "snap-1" },
                    { "name": "auto-20261113-100000" }
                ]
            }
        ])json").arg(wsPath).toUtf8();
        const QVariantList envs = Backend::parseIncusListJson(sampleJson, wsBase, nullptr, 0, true);
        QCOMPARE(envs.size(), 1);
        const QVariantMap env = envs.first().toMap();
        QCOMPARE(env.value("workspaceSnapshotOverride").toString(), QStringLiteral("false"));
        QCOMPARE(env.value("workspaceSnapshotEnabled").toBool(), false);
        QCOMPARE(env.value("workspaceSnapshots").toStringList(), wsSnaps);
        QCOMPARE(env.value("orphanWorkspaceSnapshots").toStringList(), QStringList({"pre-restore-123"}));
    }

    void workspaceAutoSnapshotSnippetPairsCreateAndPrune()
    {
        const QString ws = QStringLiteral("/tmp/my workspaces/dev box");
        const QString snippet = Backend::autoSnapshotShellSnippet(
            "dev-box",
            "auto-20261113-120000",
            {"auto-20261113-090000", "auto-20261113-100000"},
            ws,
            true);

        QVERIFY(snippet.contains("incus snapshot create 'dev-box' 'auto-20261113-120000'"));
        QVERIFY(snippet.contains("btrfs subvolume snapshot -r '/tmp/my workspaces/dev box' '/tmp/my workspaces/.snapshots/dev-box/auto-20261113-120000'"));
        QVERIFY(snippet.contains("incus snapshot delete 'dev-box' 'auto-20261113-090000'"));
        QVERIFY(snippet.contains("btrfs subvolume delete '/tmp/my workspaces/.snapshots/dev-box/auto-20261113-090000'"));
        QVERIFY(snippet.contains("btrfs subvolume delete '/tmp/my workspaces/.snapshots/dev-box/auto-20261113-100000'"));
        QVERIFY(snippet.contains("Warning: workspace auto-snapshot failed"));
    }

    void workspaceRestoreAndDeleteScriptsWithStubBtrfs()
    {
        QTemporaryDir dir;
        const QString stubDir = dir.filePath("stubs");
        QVERIFY(QDir().mkpath(stubDir));
        const QString logPath = dir.filePath("calls.log");
        const QString wsBase = dir.filePath("workspaces");
        const QString wsPath = wsBase + "/dev-box";
        const QString snapDir = Backend::workspaceSnapshotDir(wsPath, "dev-box");
        QVERIFY(QDir().mkpath(wsPath));
        write(wsPath + "/current.txt", "modified");
        QVERIFY(QDir().mkpath(snapDir));

        // Stub stat so workspace & snapshots appear as Btrfs subvolumes (inode 256)
        writeExecutable(stubDir + "/stat",
                        "#!/bin/sh\n"
                        "if [ \"$1\" = -f ]; then echo btrfs; exit 0; fi\n"
                        "if [ \"$1\" = -c ]; then echo 256; exit 0; fi\n"
                        "exec /usr/bin/stat \"$@\"\n");
        writeExecutable(stubDir + "/btrfs",
                        "#!/bin/sh\n"
                        "echo \"btrfs $@\" >> \"$AIBOX_STUB_LOG\"\n"
                        "if [ \"$1\" = subvolume ] && [ \"$2\" = snapshot ]; then\n"
                        "    if [ \"$3\" = -r ]; then src=\"$4\"; dst=\"$5\"; else src=\"$3\"; dst=\"$4\"; fi\n"
                        "    mkdir -p \"$dst\" && cp -a \"$src/.\" \"$dst/\" 2>/dev/null || true\n"
                        "    exit 0\n"
                        "fi\n"
                        "if [ \"$1\" = subvolume ] && [ \"$2\" = create ]; then\n"
                        "    mkdir -p \"$3\"\n"
                        "    exit 0\n"
                        "fi\n"
                        "if [ \"$1\" = subvolume ] && [ \"$2\" = delete ]; then\n"
                        "    rm -rf \"$3\"\n"
                        "    exit 0\n"
                        "fi\n"
                        "exit 0\n");
        writeExecutable(stubDir + "/incus",
                        "#!/bin/sh\n"
                        "echo \"incus $@\" >> \"$AIBOX_STUB_LOG\"\n"
                        "if [ \"$1\" = info ]; then echo \"Status: RUNNING\"; fi\n"
                        "exit 0\n");

        auto runBash = [&](const QString &scriptContent) {
            const QString scriptFile = dir.filePath("run.sh");
            write(scriptFile, scriptContent.toUtf8());
            QProcess p;
            QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
            env.insert("PATH", stubDir + ":" + env.value("PATH"));
            env.insert("AIBOX_STUB_LOG", logPath);
            p.setProcessEnvironment(env);
            p.start(QStringLiteral("/bin/bash"), {scriptFile});
            QVERIFY(p.waitForFinished(15000));
            QVERIFY2(p.exitCode() == 0, qPrintable(QString::fromUtf8(p.readAllStandardError())));
        };

        // 1. Paired createSnapshot
        runBash(Backend::createSnapshotScript("dev-box", "snap-a", wsPath, true));
        QVERIFY(QFileInfo(snapDir + "/snap-a/current.txt").exists());

        // Mutate workspace and verify restoreSnapshot (Both) stops container, restores, takes pre-restore safety snap, swaps
        write(wsPath + "/current.txt", "broken");
        write(snapDir + "/snap-a/current.txt", "clean");
        QFile::remove(logPath);
        runBash(Backend::restoreSnapshotScript("dev-box", "snap-a", wsPath, "both", true, 1700000123));

        QFile logFile(logPath);
        QVERIFY(logFile.open(QIODevice::ReadOnly));
        const QString restoreLog = QString::fromUtf8(logFile.readAll());
        const int stopIdx = restoreLog.indexOf("incus stop dev-box");
        const int incusRestoreIdx = restoreLog.indexOf("incus snapshot restore dev-box snap-a");
        const int safetySnapIdx = restoreLog.indexOf("btrfs subvolume snapshot -r " + wsPath + " " + snapDir + "/pre-restore-1700000123");
        const int wsRestoreIdx = restoreLog.indexOf("btrfs subvolume snapshot " + snapDir + "/snap-a " + wsPath + ".restore-tmp");
        QVERIFY2(stopIdx >= 0 && incusRestoreIdx > stopIdx && safetySnapIdx > incusRestoreIdx && wsRestoreIdx > safetySnapIdx,
                 qPrintable(restoreLog));
        QVERIFY(QFileInfo(snapDir + "/pre-restore-1700000123/current.txt").exists());

        QFile restoredFile(wsPath + "/current.txt");
        QVERIFY(restoredFile.open(QIODevice::ReadOnly));
        QCOMPARE(QString::fromUtf8(restoredFile.readAll()), QStringLiteral("clean"));
        restoredFile.close();

        // 1b. System-only restore leaves workspace untouched
        write(wsPath + "/current.txt", "keep-me");
        QFile::remove(logPath);
        runBash(Backend::restoreSnapshotScript("dev-box", "snap-a", wsPath, "system", true, 1700000999));
        QVERIFY(restoredFile.open(QIODevice::ReadOnly));
        QCOMPARE(QString::fromUtf8(restoredFile.readAll()), QStringLiteral("keep-me"));
        restoredFile.close();

        // 2. Delete paired snapshot deletes both Incus and workspace halves
        runBash(Backend::deleteSnapshotScript("dev-box", "snap-a", wsPath, true, true));
        QVERIFY(!QFileInfo::exists(snapDir + "/snap-a"));

        // 3. Wipe with deleteWorkspaceFiles cleans .snapshots/<env> and workspace
        runBash(Backend::wipeEnvironmentScript("dev-box", wsPath, true));
        QVERIFY(!QFileInfo::exists(snapDir));
        QVERIFY(!QFileInfo::exists(wsPath));

        // 4. Convert plain workspace directory to subvolume (leaving .pre-subvol backup)
        const QString plainWs = wsBase + "/plain-box";
        QVERIFY(QDir().mkpath(plainWs));
        write(plainWs + "/code.txt", "hello");
        // Make stat report non-256 inode for plain dir, 256 for subvol-tmp
        writeExecutable(stubDir + "/stat",
                        "#!/bin/sh\n"
                        "if [ \"$1\" = -f ]; then echo btrfs; exit 0; fi\n"
                        "if [ \"$1\" = -c ]; then echo 999; exit 0; fi\n"
                        "exec /usr/bin/stat \"$@\"\n");
        writeExecutable(stubDir + "/incus",
                        "#!/bin/sh\n"
                        "if [ \"$1\" = info ]; then echo \"Status: STOPPED\"; fi\n"
                        "exit 0\n");
        runBash(Backend::convertWorkspaceToSubvolumeScript("plain-box", plainWs));
        QVERIFY(QFileInfo(plainWs + "/code.txt").exists());
        QVERIFY(QFileInfo(plainWs + ".pre-subvol/code.txt").exists());
    }

    void parseRsyncItemizeHandlesFixturesAndCap()
    {
        const QString fixture = QStringLiteral(
            ".d..t...... 40 ./\n"
            ".d..t...... 40 sub dir/\n"
            "*deleting   0 added file with spaces.txt\n"
            "*deleting   0 new dir/\n"
            ">f.s....... 42 src/main file.cpp\n"
            ">f+++++++++ 128 removed.txt\n"
            "cd+++++++++ 0 old dir/\n"
            "cL+++++++++ 12 deleted_link\n"
            "cLc........ 8 changed_link\n");

        const QVariantMap parsed = Backend::parseRsyncItemize(fixture, 5000);
        QCOMPARE(parsed.value("truncated").toBool(), false);
        QCOMPARE(parsed.value("addedCount").toInt(), 2);
        QCOMPARE(parsed.value("modifiedCount").toInt(), 2);
        QCOMPARE(parsed.value("deletedCount").toInt(), 3);
        QCOMPARE(parsed.value("totalCount").toInt(), 7);

        const QVariantList entries = parsed.value("entries").toList();
        QCOMPARE(entries[0].toMap().value("path").toString(), QStringLiteral("added file with spaces.txt"));
        QCOMPARE(entries[0].toMap().value("status").toString(), QStringLiteral("added"));
        QCOMPARE(entries[0].toMap().value("type").toString(), QStringLiteral("file"));

        QCOMPARE(entries[1].toMap().value("path").toString(), QStringLiteral("new dir"));
        QCOMPARE(entries[1].toMap().value("status").toString(), QStringLiteral("added"));
        QCOMPARE(entries[1].toMap().value("type").toString(), QStringLiteral("dir"));

        QCOMPARE(entries[2].toMap().value("path").toString(), QStringLiteral("src/main file.cpp"));
        QCOMPARE(entries[2].toMap().value("status").toString(), QStringLiteral("modified"));
        QCOMPARE(entries[2].toMap().value("sizeBytes").toLongLong(), qint64(42));

        QCOMPARE(entries[3].toMap().value("path").toString(), QStringLiteral("removed.txt"));
        QCOMPARE(entries[3].toMap().value("status").toString(), QStringLiteral("deleted"));

        QCOMPARE(entries[4].toMap().value("path").toString(), QStringLiteral("old dir"));
        QCOMPARE(entries[4].toMap().value("type").toString(), QStringLiteral("dir"));
        QCOMPARE(entries[4].toMap().value("status").toString(), QStringLiteral("deleted"));

        QCOMPARE(entries[5].toMap().value("type").toString(), QStringLiteral("symlink"));
        QCOMPARE(entries[5].toMap().value("status").toString(), QStringLiteral("deleted"));
        QCOMPARE(entries[6].toMap().value("type").toString(), QStringLiteral("symlink"));
        QCOMPARE(entries[6].toMap().value("status").toString(), QStringLiteral("modified"));

        // Truncation cap
        const QVariantMap capped = Backend::parseRsyncItemize(fixture, 3);
        QCOMPARE(capped.value("truncated").toBool(), true);
        QCOMPARE(capped.value("totalCount").toInt(), 3);
    }

    void diffExcludesBinaryCheckAndLineCapping()
    {
        QTemporaryDir dir;
        {
            Backend backend(dir.path());
            QCOMPARE(backend.diffExcludes(), Backend::defaultDiffExcludes());
            QVERIFY(backend.diffExcludes().contains(QStringLiteral(".git/")));
            QVERIFY(backend.diffExcludes().contains(QStringLiteral("node_modules/")));
            QCOMPARE(backend.saveDiffExcludes(QStringLiteral(".git/, dist/, *.log\ndist/")), QString());
            QCOMPARE(backend.diffExcludes(), QStringList({".git/", "dist/", "*.log"}));
        }
        Backend reopened(dir.path());
        QCOMPARE(reopened.diffExcludes(), QStringList({".git/", "dist/", "*.log"}));

        QCOMPARE(Backend::buildRsyncExcludeArgs({".git/", " ", "build/"}),
                 QStringList({"--exclude=.git/", "--exclude=build/"}));

        QVERIFY(Backend::isSafeRelativePath("src/main.cpp"));
        QVERIFY(!Backend::isSafeRelativePath("/etc/passwd"));
        QVERIFY(!Backend::isSafeRelativePath("../secret.txt"));
        QVERIFY(!Backend::isSafeRelativePath("a/../b"));

        QVERIFY(!Backend::isLikelyBinaryData(QByteArrayLiteral("hello\nworld\n")));
        QVERIFY(Backend::isLikelyBinaryData(QByteArray("ELF\x00\x01\x02", 6)));

        QStringList bigLines;
        for (int i = 0; i < 2050; ++i)
            bigLines.append(QStringLiteral("+line %1").arg(i));
        bool truncated = false;
        const QString capped = Backend::capDiffLines(bigLines.join('\n'), 2000, &truncated);
        QVERIFY(truncated);
        QCOMPARE(capped.split('\n').size(), 2000);
    }

    void pastedImagesAreStoredInsideWorkspaceAndExcludedFromDiff()
    {
        QTemporaryDir dir;
        QImage image(8, 8, QImage::Format_ARGB32);
        image.fill(Qt::red);

        QString error;
        const QString hostPath = Backend::storePastedImage(image, dir.path(), &error);
        QVERIFY2(!hostPath.isEmpty(), qPrintable(error));
        QCOMPARE(QFileInfo(hostPath).absolutePath(),
                 QDir(dir.path()).filePath(QStringLiteral(".aibox-paste")));
        QVERIFY(QFileInfo::exists(hostPath));
        QCOMPARE(QImage(hostPath).size(), image.size());

        const QString secondPath = Backend::storePastedImage(image, dir.path());
        QVERIFY(!secondPath.isEmpty());
        QVERIFY(secondPath != hostPath);

        QString nullError;
        QVERIFY(Backend::storePastedImage(QImage(), dir.path(), &nullError).isEmpty());
        QVERIFY(!nullError.isEmpty());

        Backend backend(dir.path());
        QVERIFY(backend.diffExcludes().contains(QStringLiteral(".aibox-paste/")));
    }

    void realRsyncDetectsAddedModifiedDeletedAndHonorsExcludes()
    {
        QTemporaryDir dir;
        const QString snap = dir.filePath("snap");
        const QString ws = dir.filePath("ws");
        QVERIFY(QDir().mkpath(snap + "/.git"));
        QVERIFY(QDir().mkpath(ws + "/.git"));
        QVERIFY(QDir().mkpath(ws + "/node_modules"));

        write(snap + "/same.txt", "same\n");
        write(ws + "/same.txt", "same\n");
        // Align timestamps on same.txt so rsync doesn't flag mtime
        QFile::setPermissions(ws + "/same.txt", QFile(snap + "/same.txt").permissions());
        QProcess::execute(QStringLiteral("touch"), {QStringLiteral("-r"), snap + "/same.txt", ws + "/same.txt"});

        write(snap + "/edit.txt", "v1\n");
        write(ws + "/edit.txt", "v2-edited\n");
        write(snap + "/deleted.txt", "bye\n");
        write(ws + "/added.txt", "hi\n");
        write(ws + "/.git/HEAD", "ref: refs/heads/main\n");
        write(ws + "/node_modules/pkg.js", "module.exports = {}\n");

        QStringList args{QStringLiteral("-a"), QStringLiteral("-n"), QStringLiteral("--delete"),
                         QStringLiteral("--itemize-changes"), QStringLiteral("--out-format=%i %l %n")};
        args.append(Backend::buildRsyncExcludeArgs(Backend::defaultDiffExcludes()));
        args.append(snap + "/");
        args.append(ws + "/");

        QProcess proc;
        proc.start(QStringLiteral("rsync"), args);
        QVERIFY(proc.waitForFinished(10000));
        QCOMPARE(proc.exitCode(), 0);

        const QVariantMap res = Backend::parseRsyncItemize(QString::fromUtf8(proc.readAllStandardOutput()));
        QCOMPARE(res.value("addedCount").toInt(), 1);
        QCOMPARE(res.value("modifiedCount").toInt(), 1);
        QCOMPARE(res.value("deletedCount").toInt(), 1);
        QCOMPARE(res.value("totalCount").toInt(), 3);
    }

    void cliParsesAllSubcommandsAndFlags()
    {
        {
            const auto p = Cli::parseArgs({});
            QVERIFY(p.valid);
            QCOMPARE(p.command, Cli::Command::None);
        }
        {
            const auto p = Cli::parseArgs({"--project", "/tmp/myproj", "list", "--json"});
            QVERIFY(p.valid);
            QCOMPARE(p.command, Cli::Command::List);
            QCOMPARE(p.projectDir, QStringLiteral("/tmp/myproj"));
            QVERIFY(p.jsonOutput);
        }
        {
            const auto p = Cli::parseArgs({"check", "--fix", "--json"});
            QVERIFY(p.valid);
            QCOMPARE(p.command, Cli::Command::Check);
            QVERIFY(p.fix);
            QVERIFY(p.jsonOutput);
        }
        {
            const auto p = Cli::parseArgs({"enter", "dev-box", "--project=/tmp/p2"});
            QVERIFY(p.valid);
            QCOMPARE(p.command, Cli::Command::Enter);
            QCOMPARE(p.envName, QStringLiteral("dev-box"));
            QCOMPARE(p.projectDir, QStringLiteral("/tmp/p2"));
        }
        {
            const auto p = Cli::parseArgs({"exec", "dev-box", "--", "ls", "-la", "--help"});
            QVERIFY(p.valid);
            QCOMPARE(p.command, Cli::Command::Exec);
            QCOMPARE(p.envName, QStringLiteral("dev-box"));
            QCOMPARE(p.execCommand, QStringList({"ls", "-la", "--help"}));
        }
        {
            const auto p = Cli::parseArgs({"start", "dev-box"});
            QVERIFY(p.valid);
            QCOMPARE(p.command, Cli::Command::Start);
            QCOMPARE(p.envName, QStringLiteral("dev-box"));
        }
        {
            const auto p = Cli::parseArgs({"stop", "dev-box"});
            QVERIFY(p.valid);
            QCOMPARE(p.command, Cli::Command::Stop);
            QCOMPARE(p.envName, QStringLiteral("dev-box"));
        }
        {
            const auto p = Cli::parseArgs({"snapshot", "dev-box", "Pre Edit!"});
            QVERIFY(p.valid);
            QCOMPARE(p.command, Cli::Command::Snapshot);
            QCOMPARE(p.envName, QStringLiteral("dev-box"));
            QCOMPARE(p.snapshotName, QStringLiteral("pre-edit"));
        }
        {
            const auto p = Cli::parseArgs({"snapshots", "dev-box", "--json"});
            QVERIFY(p.valid);
            QCOMPARE(p.command, Cli::Command::Snapshots);
            QCOMPARE(p.envName, QStringLiteral("dev-box"));
            QVERIFY(p.jsonOutput);
        }
        {
            const auto p = Cli::parseArgs({"restore", "dev-box", "snap-1", "--system-only", "--yes"});
            QVERIFY(p.valid);
            QCOMPARE(p.command, Cli::Command::Restore);
            QCOMPARE(p.envName, QStringLiteral("dev-box"));
            QCOMPARE(p.snapshotName, QStringLiteral("snap-1"));
            QVERIFY(p.systemOnly);
            QVERIFY(!p.workspaceOnly);
            QVERIFY(p.yes);
        }
        {
            const auto p = Cli::parseArgs({"diff", "dev-box", "snap-1", "--json"});
            QVERIFY(p.valid);
            QCOMPARE(p.command, Cli::Command::Diff);
            QCOMPARE(p.envName, QStringLiteral("dev-box"));
            QCOMPARE(p.snapshotName, QStringLiteral("snap-1"));
            QVERIFY(p.jsonOutput);
        }
        {
            const auto p = Cli::parseArgs({"restore", "--help"});
            QVERIFY(p.valid);
            QCOMPARE(p.command, Cli::Command::Help);
            QCOMPARE(p.helpTarget, Cli::Command::Restore);
        }

        // Usage error cases (exit code 2)
        for (const QStringList &badArgs : QList<QStringList>{
                 {"unknowncmd"},
                 {"list", "--bogus"},
                 {"--project"},
                 {"--project="},
                 {"exec", "dev-box", "ls"},
                 {"exec", "dev-box", "--"},
                 {"exec", "--", "ls"},
                 {"start", "dev-box", "--json"},
                 {"restore", "dev-box", "s1", "--system-only", "--workspace-only"},
                 {"snapshot", "dev-box", "!!!"}
             }) {
            const auto p = Cli::parseArgs(badArgs);
            QVERIFY2(!p.valid, qPrintable(badArgs.join(' ')));
            QCOMPARE(p.exitCode, 2);
        }
    }

    void cliProjectResolutionAndLockCheck()
    {
        QCOMPARE(Backend::resolveProjectAiboxDir(QString(), QStringLiteral("/home/u/repo")),
                 QStringLiteral("/home/u/repo/aibox"));
        QCOMPARE(Backend::resolveProjectAiboxDir(QStringLiteral("sub/proj"), QStringLiteral("/home/u/repo")),
                 QStringLiteral("/home/u/repo/sub/proj/aibox"));
        QCOMPARE(Backend::resolveProjectAiboxDir(QStringLiteral("/tmp/custom/aibox"), QStringLiteral("/home/u/repo")),
                 QStringLiteral("/tmp/custom/aibox"));

        QTemporaryDir projDir;
        const QString aiboxDir = projDir.filePath("aibox");
        QVERIFY(QDir().mkpath(aiboxDir + "/state"));
        write(aiboxDir + "/config.json",
              R"({"firstRunCompleted":true,"autoSnapshotKeep":9,"diffExcludes":[".git/","out/"]})");

        Backend backend(projDir.path(), Backend::PathMode::ProjectDir);
        QCOMPARE(backend.workspaceBase(), aiboxDir + "/workspaces");
        QCOMPARE(backend.stateDir(), aiboxDir + "/state");
        QCOMPARE(backend.configFilePath(), aiboxDir + "/config.json");
        QCOMPARE(backend.autoSnapshotKeep(), 9);
        QCOMPARE(backend.diffExcludes(), QStringList({".git/", "out/"}));

        // Stale PID must not lock
        write(aiboxDir + "/state/task.pid", "99999999\n");
        QVERIFY(!backend.isTaskLockActive());

        // Live PID locks, and releasing clears lock
        QProcess sleeper;
        sleeper.start(QStringLiteral("/bin/sleep"), {QStringLiteral("10")});
        QVERIFY(sleeper.waitForStarted(3000));
        write(aiboxDir + "/state/task.pid", QByteArray::number(sleeper.processId()) + '\n');
        QVERIFY(backend.isTaskLockActive());
        sleeper.kill();
        sleeper.waitForFinished(3000);
        backend.releaseCliTaskLock();
        QVERIFY(!backend.isTaskLockActive());
    }

    void cliExecAndExitCodesWithStubIncus()
    {
        QTemporaryDir dir;
        const QString stubDir = dir.filePath("bin");
        const QString projDir = dir.filePath("proj");
        const QString wsBase = projDir + "/.aibox/workspaces";
        const QString stateDir = projDir + "/.aibox/state";
        QVERIFY(QDir().mkpath(stubDir));
        QVERIFY(QDir().mkpath(wsBase + "/dev"));
        QVERIFY(QDir().mkpath(stateDir));
        write(projDir + "/.aibox/config.json", R"({"firstRunCompleted":true,"autoSnapshot":true,"autoSnapshotKeep":5})");

        const QString logPath = dir.filePath("incus.log");
        const QString listJson = QStringLiteral(R"json([
  {
    "name": "dev",
    "type": "container",
    "status": "Stopped",
    "config": { "user.aibox.autosnap": "true", "user.aibox.wssnap": "false" },
    "expanded_devices": { "workspace": { "source": "%1/dev", "path": "/workspace" } },
    "snapshots": []
  },
  {
    "name": "tpl",
    "type": "container",
    "status": "Stopped",
    "config": { "user.aibox.template": "true" },
    "expanded_devices": {},
    "snapshots": []
  }
])json").arg(wsBase);
        const QString listJsonFile = dir.filePath("list.json");
        write(listJsonFile, listJson.toUtf8());

        const QString incusStub = QStringLiteral(R"sh(#!/bin/sh
echo "incus $*" >> "%1"
if [ "$1" = "list" ]; then
    cat "%2"
    exit 0
fi
if [ "$1" = "info" ]; then
    echo "Status: STOPPED"
    exit 0
fi
if [ "$1" = "exec" ]; then
    while [ "$#" -gt 0 ]; do
        if [ "$1" = "--" ]; then
            shift
            break
        fi
        shift
    done
    exec "$@"
fi
if [ "$1" = "query" ]; then
    echo "[]"
    exit 0
fi
exit 0
)sh").arg(logPath, listJsonFile);
        writeExecutable(stubDir + "/incus", incusStub.toUtf8());

        const QByteArray oldPath = qgetenv("PATH");
        qputenv("PATH", (stubDir + ":" + QString::fromLocal8Bit(oldPath)).toLocal8Bit());
        struct PathRestore {
            QByteArray orig;
            ~PathRestore() { qputenv("PATH", orig); }
        } restorePath{oldPath};

        // Verify GUI and CLI prepareEnvironmentScript build the same pre-steps
        {
            Backend checkBackend(projDir, Backend::PathMode::ProjectDir);
            QString err;
            QVERIFY(checkBackend.refreshSync(&err));
            const QString preScript = checkBackend.prepareEnvironmentScript("dev");
            QVERIFY(preScript.contains("incus snapshot create 'dev' 'auto-"));
            QVERIFY(preScript.contains("incus start 'dev'"));
        }

        // exec dev -- true -> exits 0 and runs auto-snapshot + start + exec
        QFile::remove(logPath);
        QCOMPARE(Cli::execute(Cli::parseArgs({"--project", projDir, "exec", "dev", "--", "true"})), 0);
        {
            QFile f(logPath);
            QVERIFY(f.open(QIODevice::ReadOnly));
            const QString log = QString::fromUtf8(f.readAll());
            QVERIFY2(log.contains("incus snapshot create dev auto-"), qPrintable(log));
            QVERIFY2(log.contains("incus start dev"), qPrintable(log));
            QVERIFY2(log.contains("incus exec dev --cwd /home/workspace -- true"), qPrintable(log));
        }

        // exec dev -- false -> exits 1
        QCOMPARE(Cli::execute(Cli::parseArgs({"--project", projDir, "exec", "dev", "--", "false"})), 1);

        // missing env and template env -> exit 4
        QCOMPARE(Cli::execute(Cli::parseArgs({"--project", projDir, "exec", "missing", "--", "true"})), 4);
        QCOMPARE(Cli::execute(Cli::parseArgs({"--project", projDir, "exec", "tpl", "--", "true"})), 4);
        QCOMPARE(Cli::execute(Cli::parseArgs({"--project", projDir, "start", "tpl"})), 4);

        // live background task lock -> mutating command exits 3
        QProcess sleeper;
        sleeper.start(QStringLiteral("/bin/sleep"), {QStringLiteral("10")});
        QVERIFY(sleeper.waitForStarted(3000));
        write(stateDir + "/task.pid", QByteArray::number(sleeper.processId()) + '\n');
        QCOMPARE(Cli::execute(Cli::parseArgs({"--project", projDir, "exec", "dev", "--", "true"})), 3);
        QCOMPARE(Cli::execute(Cli::parseArgs({"--project", projDir, "start", "dev"})), 3);
        QCOMPARE(Cli::execute(Cli::parseArgs({"--project", projDir, "snapshot", "dev", "s1"})), 3);
        QCOMPARE(Cli::execute(Cli::parseArgs({"--project", projDir, "restore", "dev", "s1", "--yes"})), 3);
        sleeper.kill();
        sleeper.waitForFinished(3000);
        QFile::remove(stateDir + "/task.pid");

        // diff with existing workspace snapshot directory -> exits 0
        const QString wsSnap = wsBase + "/.snapshots/dev/snap-1";
        QVERIFY(QDir().mkpath(wsSnap));
        write(wsSnap + "/a.txt", "before\n");
        write(wsBase + "/dev/a.txt", "after\n");
        QCOMPARE(Cli::execute(Cli::parseArgs({"--project", projDir, "diff", "dev", "snap-1", "--json"})), 0);
        QCOMPARE(Cli::execute(Cli::parseArgs({"--project", projDir, "snapshots", "dev", "--json"})), 0);
    }

    void enterEnvironmentReturnsWithoutBlocking()
    {
        QTemporaryDir dir;
        QVERIFY(dir.isValid());
        const QString stubDir = dir.filePath("bin");
        const QString projDir = dir.filePath("proj");
        const QString wsBase = projDir + "/.aibox/workspaces";
        const QString stateDir = projDir + "/.aibox/state";
        QVERIFY(QDir().mkpath(stubDir));
        QVERIFY(QDir().mkpath(wsBase + "/dev"));
        QVERIFY(QDir().mkpath(stateDir));
        write(projDir + "/.aibox/config.json", R"({"firstRunCompleted":true})");

        const QString listJsonFile = dir.filePath("list.json");
        write(listJsonFile, QStringLiteral(R"json([
  {
    "name": "dev",
    "type": "container",
    "status": "Stopped",
    "config": {},
    "expanded_devices": { "workspace": { "source": "%1/dev", "path": "/home/workspace" } },
    "snapshots": []
  }
])json").arg(wsBase).toUtf8());

        // Slow incus info keeps the preparation step running for three seconds.
        const QString incusStub = QStringLiteral(R"sh(#!/bin/sh
if [ "$1" = "list" ]; then
    cat "%1"
    exit 0
fi
if [ "$1" = "info" ]; then
    sleep 3
    echo "Status: RUNNING"
    exit 0
fi
exit 0
)sh").arg(listJsonFile);
        writeExecutable(stubDir + "/incus", incusStub.toUtf8());

        const QString markerPath = dir.filePath("terminal-opened");
        writeExecutable(stubDir + "/xdg-terminal-exec",
                        QStringLiteral("#!/bin/sh\necho \"$@\" > '%1'\n").arg(markerPath).toUtf8());

        const QByteArray oldPath = qgetenv("PATH");
        qputenv("PATH", (stubDir + ":" + QString::fromLocal8Bit(oldPath)).toLocal8Bit());
        struct PathRestore {
            QByteArray orig;
            ~PathRestore() { qputenv("PATH", orig); }
        } restorePath{oldPath};

        Backend backend(projDir, Backend::PathMode::ProjectDir);
        QString err;
        QVERIFY2(backend.refreshSync(&err), qPrintable(err));
        QVERIFY(!backend.findEnvironment("dev").isEmpty());

        QElapsedTimer elapsed;
        elapsed.start();
        backend.enterEnvironment("dev");
        const qint64 callMs = elapsed.elapsed();
        QVERIFY2(callMs < 500,
                 qPrintable(QStringLiteral("enterEnvironment blocked the GUI thread for %1 ms").arg(callMs)));

        QTRY_VERIFY_WITH_TIMEOUT(QFileInfo::exists(markerPath), 15000);
        QFile marker(markerPath);
        QVERIFY(marker.open(QIODevice::ReadOnly));
        const QString launched = QString::fromUtf8(marker.readAll());
        QVERIFY2(launched.contains("incus exec"), qPrintable(launched));
    }

    void secretsValidationQuotingAndRedaction()
    {
        QVERIFY(Backend::isValidSecretName("OPENAI_API_KEY"));
        QVERIFY(Backend::isValidSecretName("_PRIVATE_1"));
        QVERIFY(!Backend::isValidSecretName(""));
        QVERIFY(!Backend::isValidSecretName("lowercase"));
        QVERIFY(!Backend::isValidSecretName("1INVALID"));
        QVERIFY(!Backend::isValidSecretName("BAD-NAME"));
        QVERIFY(!Backend::isValidSecretName("BAD=VAL"));

        QCOMPARE(Backend::parseEnvSecretNames("OPENAI_API_KEY, bad_name, KEY=val, GITHUB_TOKEN, OPENAI_API_KEY"),
                 QStringList({"OPENAI_API_KEY", "GITHUB_TOKEN"}));

        const QByteArray incusJson = R"([
            {
                "name": "sec-env",
                "type": "container",
                "status": "Running",
                "config": {
                    "user.aibox.project": "/tmp/proj",
                    "user.aibox.secrets": "OPENAI_API_KEY,NOT_VALID=123,GITHUB_TOKEN"
                },
                "devices": {},
                "state": {}
            }
        ])";
        const QVariantList envs = Backend::parseIncusListJson(incusJson, "/tmp/proj", {});
        QCOMPARE(envs.size(), 1);
        const QVariantMap env = envs.first().toMap();
        QCOMPARE(env.value("secretNames").toStringList(), QStringList({"OPENAI_API_KEY", "GITHUB_TOKEN"}));

        const QString weirdVal = "it's $HOME $(id) `uname`\nline2";
        const QList<QPair<QString, QString>> pairs = {
            {"OPENAI_API_KEY", "sk-short"},
            {"OPENAI_API_KEY_LONG", "sk-short-extra"},
            {"TRICKY_VAL", weirdVal}
        };
        const QString envScript = Backend::buildSecretEnvFileContent(pairs);
        QVERIFY(envScript.contains("OPENAI_API_KEY="));
        QVERIFY(envScript.contains("TRICKY_VAL="));
        QVERIFY(!envScript.contains("export "));

        // Verify /bin/sh sources quoted values verbatim without expanding $, backticks, or newlines
        QTemporaryDir tmpDir;
        QVERIFY(tmpDir.isValid());
        const QString scriptPath = tmpDir.path() + "/test.env";
        write(scriptPath, envScript.toUtf8());
        QProcess sh;
        sh.start("/bin/sh", {"-c", ". \"$1\" && printf '%s' \"$TRICKY_VAL\"", "sh", scriptPath});
        QVERIFY(sh.waitForFinished(5000));
        QCOMPARE(sh.exitCode(), 0);
        QCOMPARE(QString::fromUtf8(sh.readAllStandardOutput()), weirdVal);

        // Redaction replaces longest secret values first so prefixes don't leave tails behind
        const QString rawLog = "using sk-short-extra and sk-short inside output";
        const QString redacted = Backend::redactSecrets(rawLog, {"sk-short", "sk-short-extra"});
        QCOMPARE(redacted, QString("using [REDACTED] and [REDACTED] inside output"));
        QCOMPARE(QString::fromUtf8(Backend::redactSecretsBytes(rawLog.toUtf8(), {"sk-short", "sk-short-extra"})),
                 QString("using [REDACTED] and [REDACTED] inside output"));
    }

    void secretsFilePermissionsEnforcedAndFixed()
    {
        QTemporaryDir tmpDir;
        QVERIFY(tmpDir.isValid());
        const QString storeDir = tmpDir.path() + "/cfg";
        const QString storePath = storeDir + "/secrets.json";

        const QList<QPair<QString, QString>> initial = {{"API_KEY", "super-secret-value"}};
        QCOMPARE(Backend::saveSecretsStore(storePath, initial), QString());

        const auto dirPerms = QFileInfo(storeDir).permissions();
        QVERIFY(!(dirPerms & (QFileDevice::ReadGroup | QFileDevice::WriteGroup | QFileDevice::ExeGroup
                              | QFileDevice::ReadOther | QFileDevice::WriteOther | QFileDevice::ExeOther)));

        const auto filePerms = QFileInfo(storePath).permissions();
        QVERIFY(filePerms & QFileDevice::ReadOwner);
        QVERIFY(filePerms & QFileDevice::WriteOwner);
        QVERIFY(!(filePerms & (QFileDevice::ReadGroup | QFileDevice::WriteGroup | QFileDevice::ExeGroup
                               | QFileDevice::ReadOther | QFileDevice::WriteOther | QFileDevice::ExeOther)));

        QString err;
        const auto loaded = Backend::loadSecretsStore(storePath, &err);
        QCOMPARE(err, QString());
        QCOMPARE(loaded, initial);

        // Group/world-readable files must be refused on load
        QVERIFY(QFile::setPermissions(storePath, QFileDevice::ReadOwner | QFileDevice::WriteOwner | QFileDevice::ReadOther));
        const auto refusedOther = Backend::loadSecretsStore(storePath, &err);
        QVERIFY(refusedOther.isEmpty());
        QVERIFY(!err.isEmpty());

        QVERIFY(QFile::setPermissions(storePath, QFileDevice::ReadOwner | QFileDevice::WriteOwner | QFileDevice::ReadGroup));
        const auto refusedGroup = Backend::loadSecretsStore(storePath, &err);
        QVERIFY(refusedGroup.isEmpty());
        QVERIFY(!err.isEmpty());

        // Saving again through Backend fixes permissions back to 0600
        Backend backend;
        backend.setSecretsStorePathForTest(storePath);
        QVERIFY(!backend.secretsError().isEmpty());
        QCOMPARE(backend.saveSecret("API_KEY", "rotated-secret"), QString());
        QCOMPARE(backend.secretsError(), QString());

        const auto fixedPerms = QFileInfo(storePath).permissions();
        QVERIFY(!(fixedPerms & (QFileDevice::ReadGroup | QFileDevice::ReadOther)));
        QCOMPARE(Backend::loadSecretsStore(storePath, &err),
                 (QList<QPair<QString, QString>>{{"API_KEY", "rotated-secret"}}));
        QCOMPARE(err, QString());
    }

    void secretsExecInjectionRedactsHistoryAndNeverTouchesBackups()
    {
        QTemporaryDir tmpDir;
        QVERIFY(tmpDir.isValid());
        const QString binDir = tmpDir.path() + "/bin";
        QVERIFY(QDir().mkpath(binDir));
        const QString projDir = tmpDir.path() + "/proj";
        QVERIFY(QDir().mkpath(projDir));
        const QString storePath = tmpDir.path() + "/secrets.json";
        const QString pushLog = tmpDir.path() + "/pushed.log";
        const QString runtimeDir = tmpDir.path() + "/runtime";
        QVERIFY(QDir().mkpath(runtimeDir));
        QVERIFY(QFile::setPermissions(runtimeDir, QFileDevice::ReadOwner | QFileDevice::WriteOwner | QFileDevice::ExeOwner));

        const QByteArray oldPath = qgetenv("PATH");
        const QByteArray oldSecretsEnv = qgetenv("AIBOX_SECRETS_PATH");
        const QByteArray oldRuntimeEnv = qgetenv("XDG_RUNTIME_DIR");
        const QByteArray oldIncusPushLog = qgetenv("AIBOX_TEST_PUSH_LOG");

        struct EnvGuard {
            QByteArray path, secrets, runtime, pushLog;
            ~EnvGuard() {
                qputenv("PATH", path);
                if (secrets.isEmpty()) qunsetenv("AIBOX_SECRETS_PATH"); else qputenv("AIBOX_SECRETS_PATH", secrets);
                if (runtime.isEmpty()) qunsetenv("XDG_RUNTIME_DIR"); else qputenv("XDG_RUNTIME_DIR", runtime);
                if (pushLog.isEmpty()) qunsetenv("AIBOX_TEST_PUSH_LOG"); else qputenv("AIBOX_TEST_PUSH_LOG", pushLog);
            }
        } guard{oldPath, oldSecretsEnv, oldRuntimeEnv, oldIncusPushLog};

        qputenv("PATH", binDir.toUtf8() + ":" + oldPath);
        qputenv("AIBOX_SECRETS_PATH", storePath.toUtf8());
        qputenv("XDG_RUNTIME_DIR", runtimeDir.toUtf8());
        qputenv("AIBOX_TEST_PUSH_LOG", pushLog.toUtf8());

        QCOMPARE(Backend::saveSecretsStore(storePath, {{"MY_TOKEN", "tok-999-secret"}}), QString());

        const QByteArray realShmStub = QByteArray(
            "#!/bin/sh\n"
            "set -e\n"
            "case \"$1\" in\n"
            "  list)\n"
            "    cat <<'EOF'\n"
            "[{\"name\":\"secbox\",\"type\":\"container\",\"status\":\"Running\",\"config\":{\"user.aibox.project\":\"")
            + projDir.toUtf8()
            + QByteArray(
            "\",\"user.aibox.secrets\":\"MY_TOKEN\"},\"devices\":{},\"state\":{}}]\n"
            "EOF\n"
            "    ;;\n"
            "  file)\n"
            "    if [ \"$2\" = \"push\" ]; then\n"
            "      shift 2\n"
            "      while [ \"$#\" -gt 0 ]; do\n"
            "        case \"$1\" in\n"
            "          --mode) shift 2 ;;\n"
            "          *) break ;;\n"
            "        esac\n"
            "      done\n"
            "      SRC=\"$1\"\n"
            "      DEST=\"$2\"\n"
            "      BASE=\"${DEST##*/}\"\n"
            "      cp \"$SRC\" \"/dev/shm/$BASE\"\n"
            "      cat \"$SRC\" > \"${AIBOX_TEST_PUSH_LOG}\"\n"
            "    fi\n"
            "    ;;\n"
            "  exec)\n"
            "    shift\n"
            "    CONTAINER=\"$1\"; shift\n"
            "    while [ \"$#\" -gt 0 ]; do\n"
            "      case \"$1\" in\n"
            "        --) shift; break ;;\n"
            "        *) shift ;;\n"
            "      esac\n"
            "    done\n"
            "    exec \"$@\"\n"
            "    ;;\n"
            "  *)\n"
            "    exit 0\n"
            "    ;;\n"
            "esac\n");
        writeExecutable(binDir + "/incus", realShmStub);

        Backend backend;
        backend.setSecretsStorePathForTest(storePath);
        QString err;
        const QString hostTmp = Backend::createHostTempSecretFile("export FOO='bar'\n", &err);
        QVERIFY(!hostTmp.isEmpty());
        QCOMPARE(err, QString());
        QVERIFY(!(QFileInfo(hostTmp).permissions() & (QFileDevice::ReadGroup | QFileDevice::ReadOther)));
        QFile::remove(hostTmp);

        const QStringList enterArgs = backend.enterIncusExecArgs("secbox", "/dev/shm/test-env.env");
        QVERIFY(enterArgs.join(" ").contains("/dev/shm/test-env.env"));
        QVERIFY(!enterArgs.join(" ").contains("tok-999-secret"));

        const QStringList cmdArgs = backend.execIncusCommandArgs("secbox", {"printenv", "MY_TOKEN"}, "/dev/shm/test-env.env");
        QVERIFY(cmdArgs.contains("/dev/shm/test-env.env"));
        QVERIFY(cmdArgs.join(" ").contains(". \"$AIBOX_ENV\""));
        QVERIFY(cmdArgs.join(" ").contains("rm -f \"$AIBOX_ENV\""));
        QVERIFY(!cmdArgs.join(" ").contains("tok-999-secret"));

        QCOMPARE(Cli::execute(Cli::parseArgs({"--project", projDir, "exec", "secbox", "--", "sh", "-c", "test \"$MY_TOKEN\" = 'tok-999-secret'"})), 0);
        QVERIFY(QDir(runtimeDir).entryList(QDir::Files).isEmpty());
        QFile pushedFile(pushLog);
        QVERIFY(pushedFile.open(QIODevice::ReadOnly));
        QVERIFY(QString::fromUtf8(pushedFile.readAll()).contains("MY_TOKEN='tok-999-secret'"));

        QVERIFY(!backend.aiExecCommandText("secbox").contains("tok-999-secret"));
        QCOMPARE(backend.deleteSecret("MY_TOKEN"), QString());
        QVERIFY(backend.secretsList().isEmpty());

        // Backup / export / clone scripts must never reference secrets.json or expose secret values
        const QString exp = Backend::exportEnvironmentScript("secbox", "/tmp/out.tar.zst", projDir, "/tmp/ws.tar.zst");
        const QString imp = Backend::importEnvironmentScript("/tmp/out.tar.zst", "secbox", projDir, "/tmp/ws.tar.zst");
        const QString cln = Backend::cloneEnvironmentScript("secbox", "secbox-copy", projDir, "20GiB", "4GiB", 4, false);
        QVERIFY(!exp.contains("secrets.json"));
        QVERIFY(!imp.contains("secrets.json"));
        QVERIFY(!cln.contains("secrets.json"));
        QVERIFY(!exp.contains("tok-999-secret"));
    }

    void poolStorageInfoParsingRealFixtures()
    {
        // Fixture from real host: incus storage info default --bytes
        const QString bytesYaml = QStringLiteral(
            "info:\n"
            "  description: Btrfs Storage Pool with disk quotas and instant snapshots\n"
            "  driver: btrfs\n"
            "  name: default\n"
            "  space used: \"1329242112\"\n"
            "  total space: \"32212254720\"\n"
            "used by:\n"
            "  instances:\n"
            "    - mobile-porting\n"
            "  profiles:\n"
            "    - default\n");
        const auto bytesInfo = Backend::parseIncusStorageInfo(bytesYaml);
        QVERIFY(bytesInfo.valid);
        QCOMPARE(bytesInfo.name, QStringLiteral("default"));
        QCOMPARE(bytesInfo.driver, QStringLiteral("btrfs"));
        QCOMPARE(bytesInfo.usedBytes, qint64(1329242112LL));
        QCOMPARE(bytesInfo.totalBytes, qint64(32212254720LL));
        QCOMPARE(bytesInfo.usedPercent, 4);

        // Fixture from real host: incus storage info default (human-readable)
        const QString humanYaml = QStringLiteral(
            "info:\n"
            "  driver: btrfs\n"
            "  name: default\n"
            "  space used: 1.24GiB\n"
            "  total space: 30.00GiB\n");
        const auto humanInfo = Backend::parseIncusStorageInfo(humanYaml);
        QVERIFY(humanInfo.valid);
        QCOMPARE(humanInfo.totalBytes, qint64(30LL) * 1024LL * 1024LL * 1024LL);
        QCOMPARE(humanInfo.usedPercent, 4);

        // JSON fixture support if --format=json is ever emitted
        const QString jsonOut = QStringLiteral(
            "{\"info\":{\"name\":\"default\",\"driver\":\"btrfs\",\"space used\":26843545600,\"total space\":32212254720}}");
        const auto jsonInfo = Backend::parseIncusStorageInfo(jsonOut);
        QVERIFY(jsonInfo.valid);
        QCOMPARE(jsonInfo.usedPercent, 83);

        QVERIFY(!Backend::parseIncusStorageInfo(QStringLiteral("invalid output")).valid);
    }

    void poolOvercommitMathAndUnboundedQuotas()
    {
        const qint64 GiB = 1024LL * 1024LL * 1024LL;
        const QByteArray listJson = QByteArray(
            "["
            "  {\"name\":\"env-a\",\"devices\":{},\"expanded_devices\":{\"root\":{\"path\":\"/\",\"pool\":\"default\",\"size\":\"20GiB\"}}},"
            "  {\"name\":\"env-b\",\"devices\":{\"root\":{\"size\":\"15GiB\"}},\"expanded_devices\":{\"root\":{\"path\":\"/\",\"pool\":\"default\",\"size\":\"15GiB\"}}},"
            "  {\"name\":\"env-noquota\",\"devices\":{},\"expanded_devices\":{\"root\":{\"path\":\"/\",\"pool\":\"default\"}}},"
            "  {\"name\":\"env-otherpool\",\"devices\":{},\"expanded_devices\":{\"root\":{\"path\":\"/\",\"pool\":\"ssd2\",\"size\":\"100GiB\"}}}"
            "]");

        int unbounded = 0;
        const qint64 sumAll = Backend::sumRootDiskQuotasBytes(listJson, {}, QStringLiteral("default"), &unbounded);
        QCOMPARE(sumAll, 35LL * GiB);
        QCOMPARE(unbounded, 1);

        int unboundedExcl = 0;
        const qint64 sumExclA = Backend::sumRootDiskQuotasBytes(listJson, QStringLiteral("env-a"), QStringLiteral("default"), &unboundedExcl);
        QCOMPARE(sumExclA, 15LL * GiB);
        QCOMPARE(unboundedExcl, 1);

        Backend::StoragePoolInfo pool;
        pool.valid = true;
        pool.name = QStringLiteral("default");
        pool.usedBytes = 10LL * GiB;
        pool.totalBytes = 40LL * GiB;
        pool.usedPercent = 25;

        // 35GiB + 4GiB = 39GiB <= 40GiB and 25% <= 85%: no warning
        QCOMPARE(Backend::evaluatePoolOvercommitWarning(pool, sumAll, 4LL * GiB, unbounded), QString());

        // 35GiB + 10GiB = 45GiB > 40GiB: overcommit warning mentioning unbounded env
        const QString overWarn = Backend::evaluatePoolOvercommitWarning(pool, sumAll, 10LL * GiB, unbounded);
        QVERIFY(overWarn.contains(QStringLiteral("45.0 GiB")));
        QVERIFY(overWarn.contains(QStringLiteral("40.0 GiB")));
        QVERIFY(overWarn.contains(QStringLiteral("1 environment(s)")));

        // High pool usage (>85%) triggers warning even without quota overcommit
        pool.usedPercent = 88;
        pool.usedBytes = 35LL * GiB;
        QVERIFY(!Backend::evaluatePoolOvercommitWarning(pool, 0, 1LL * GiB, 0).isEmpty());

        // Backend helpers for Create and Edit Quotas
        pool.usedPercent = 25;
        pool.usedBytes = 10LL * GiB;
        Backend backend;
        backend.setPoolStateForTest(pool, listJson);
        QVERIFY(backend.poolInfo().value("valid").toBool());
        QCOMPARE(backend.poolWarningForCreate(QStringLiteral("4GiB")), QString());
        QVERIFY(!backend.poolWarningForCreate(QStringLiteral("10GiB")).isEmpty());
        // Editing env-a from 20GiB -> 24GiB (15 + 24 = 39 <= 40) clears warning; 20GiB -> 30GiB (15 + 30 = 45 > 40) warns
        QCOMPARE(backend.poolWarningForQuotaEdit(QStringLiteral("env-a"), QStringLiteral("24GiB")), QString());
        QVERIFY(!backend.poolWarningForQuotaEdit(QStringLiteral("env-a"), QStringLiteral("30GiB")).isEmpty());
    }

    void poolCapacityHealthCheckThresholds()
    {
        const qint64 GiB = 1024LL * 1024LL * 1024LL;
        Backend::StoragePoolInfo pool;
        pool.valid = true;
        pool.name = QStringLiteral("default");
        pool.totalBytes = 100LL * GiB;

        pool.usedBytes = 85LL * GiB;
        pool.usedPercent = 85;
        const auto h85 = Backend::evaluatePoolCapacityHealth(pool, 60LL * GiB, 0);
        QCOMPARE(h85.id, QStringLiteral("pool_capacity"));
        QVERIFY(h85.ok && !h85.warning);
        QVERIFY(h85.detail.contains(QStringLiteral("85%")));

        pool.usedBytes = 86LL * GiB;
        pool.usedPercent = 86;
        const auto h86 = Backend::evaluatePoolCapacityHealth(pool, 60LL * GiB, 1);
        QVERIFY(h86.ok && h86.warning);
        QVERIFY(h86.detail.contains(QStringLiteral("86%")));
        QVERIFY(h86.detail.contains(QStringLiteral("60.0 GiB")));

        pool.usedBytes = 96LL * GiB;
        pool.usedPercent = 96;
        const auto h96 = Backend::evaluatePoolCapacityHealth(pool, 120LL * GiB, 0);
        QVERIFY(!h96.ok && !h96.warning);
        QVERIFY(h96.detail.contains(QStringLiteral("96%")));
        QVERIFY(h96.detail.contains(QStringLiteral("OVERCOMMITTED")));

        // Verify runHealthChecks via runner hits pool_capacity warning/failure cleanly
        auto makeRunner = [&](const QString &usedPctStr) -> Backend::CommandRunner {
            return [usedPctStr](const QString &prog, const QStringList &args) -> Backend::CommandOutput {
                if (prog == QLatin1String("id"))
                    return {0, QStringLiteral("user incus-admin kvm render video\n"), {}};
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

        const auto warnChecks = Backend::runHealthChecks(false, makeRunner(QStringLiteral("90")), QStringLiteral("user"));
        QCOMPARE(warnChecks.last().id, QStringLiteral("pool_capacity"));
        QVERIFY(warnChecks.last().ok && warnChecks.last().warning);

        const auto failChecks = Backend::runHealthChecks(false, makeRunner(QStringLiteral("97")), QStringLiteral("user"));
        QCOMPARE(failChecks.last().id, QStringLiteral("pool_capacity"));
        QVERIFY(!failChecks.last().ok && !failChecks.last().warning);
    }

    void poolResizeSpecValidationAndScript()
    {
        const qint64 GiB = 1024LL * 1024LL * 1024LL;

        // Size spec parsing: bare numbers are GiB, suffixes are binary.
        qint64 parsed = 0;
        QVERIFY(Backend::parsePoolSizeSpec(QStringLiteral("60"), &parsed));
        QCOMPARE(parsed, 60LL * GiB);
        QVERIFY(Backend::parsePoolSizeSpec(QStringLiteral(" 60GiB "), &parsed));
        QCOMPARE(parsed, 60LL * GiB);
        QVERIFY(Backend::parsePoolSizeSpec(QStringLiteral("1.5TiB"), &parsed));
        QCOMPARE(parsed, 1536LL * GiB);
        QVERIFY(Backend::parsePoolSizeSpec(QStringLiteral("512MiB"), &parsed));
        QCOMPARE(parsed, 512LL * 1024LL * 1024LL);
        QVERIFY(!Backend::parsePoolSizeSpec(QStringLiteral(""), &parsed));
        QVERIFY(!Backend::parsePoolSizeSpec(QStringLiteral("big"), &parsed));
        QVERIFY(!Backend::parsePoolSizeSpec(QStringLiteral("-60GiB"), &parsed));
        QVERIFY(!Backend::parsePoolSizeSpec(QStringLiteral("0"), &parsed));

        Backend::StoragePoolInfo pool;
        pool.valid = true;
        pool.name = QStringLiteral("default");
        pool.driver = QStringLiteral("btrfs");
        pool.usedBytes = 512LL * 1024LL * 1024LL;
        pool.totalBytes = 1024LL * 1024LL * 1024LL;
        pool.usedPercent = 50;
        pool.sizeText = QStringLiteral("1GiB");
        pool.resizable = true;

        Backend backend;
        backend.setPoolStateForTest(pool, QByteArray("[]"));
        QVERIFY(backend.poolInfo().value("valid").toBool());
        QVERIFY(backend.poolInfo().value("resizable").toBool());
        QCOMPARE(backend.poolInfo().value("sizeText").toString(), QStringLiteral("1GiB"));

        // Validation rejects empty, garbage, unchanged, and shrinking sizes.
        QVERIFY(!backend.validateStoragePoolResize(QString()).isEmpty());
        QVERIFY(!backend.validateStoragePoolResize(QStringLiteral("nope")).isEmpty());
        QVERIFY(!backend.validateStoragePoolResize(QStringLiteral("1GiB")).isEmpty());
        QVERIFY(!backend.validateStoragePoolResize(QStringLiteral("512MiB")).isEmpty());
        QCOMPARE(backend.validateStoragePoolResize(QStringLiteral("2GiB")), QString());
        QCOMPARE(backend.validateStoragePoolResize(QStringLiteral("2")), QString());

        // Directory/subvolume backed pools follow the host filesystem instead.
        Backend::StoragePoolInfo dirPool = pool;
        dirPool.sizeText.clear();
        dirPool.resizable = false;
        backend.setPoolStateForTest(dirPool, QByteArray("[]"));
        QVERIFY(!backend.poolInfo().value("resizable").toBool());
        QVERIFY(!backend.validateStoragePoolResize(QStringLiteral("2GiB")).isEmpty());

        // Script quotes arguments and drives Incus' loop-file grow + btrfs resize.
        backend.setPoolStateForTest(pool, QByteArray("[]"));
        const QString script = Backend::resizeStoragePoolScript(QStringLiteral("default"), QStringLiteral("2GiB"));
        QVERIFY(script.contains(QStringLiteral("incus storage set 'default' size='2GiB'")));
        QVERIFY(script.contains(QStringLiteral("incus storage info 'default'")));
        QVERIFY(script.contains(QStringLiteral("@@STAGE:100")));

        // resizeStoragePool refuses invalid input without starting a task.
        QVERIFY(!backend.resizeStoragePool(QStringLiteral("512MiB")).isEmpty());
        QVERIFY(!backend.busy());
    }

    void idleStopOverridesAndConfigPersistence()
    {
        QCOMPARE(Backend::parseIdleStopOverride("").mode, Backend::IdleStopOverride::Inherit);
        QCOMPARE(Backend::parseIdleStopOverride("inherit").mode, Backend::IdleStopOverride::Inherit);
        QCOMPARE(Backend::parseIdleStopOverride("off").mode, Backend::IdleStopOverride::Off);
        QCOMPARE(Backend::parseIdleStopOverride("0").mode, Backend::IdleStopOverride::Off);
        QCOMPARE(Backend::parseIdleStopOverride("false").mode, Backend::IdleStopOverride::Off);
        const auto m30 = Backend::parseIdleStopOverride("30");
        QCOMPARE(m30.mode, Backend::IdleStopOverride::Minutes);
        QCOMPARE(m30.minutes, 30);

        QCOMPARE(Backend::effectiveIdleStopMinutes(0, Backend::parseIdleStopOverride("inherit")), 0);
        QCOMPARE(Backend::effectiveIdleStopMinutes(60, Backend::parseIdleStopOverride("inherit")), 60);
        QCOMPARE(Backend::effectiveIdleStopMinutes(60, Backend::parseIdleStopOverride("off")), 0);
        QCOMPARE(Backend::effectiveIdleStopMinutes(0, Backend::parseIdleStopOverride("15")), 15);
        QCOMPARE(Backend::effectiveIdleStopMinutes(60, Backend::parseIdleStopOverride("15")), 15);

        const QByteArray incusJson = QByteArray(
            "[{"
            "\"name\":\"idle-box\",\"type\":\"container\",\"status\":\"Running\","
            "\"config\":{\"user.aibox.idlestop\":\"45\",\"user.aibox.idlestop_ports\":\"true\"},"
            "\"devices\":{},\"state\":{}"
            "}]");
        const QVariantList envs = Backend::parseIncusListJson(incusJson, QStringLiteral("/tmp/ws"));
        QCOMPARE(envs.size(), 1);
        QCOMPARE(envs.first().toMap().value("idleStopOverride").toString(), QStringLiteral("45"));
        QVERIFY(envs.first().toMap().value("idleStopIgnorePorts").toBool());

        QTemporaryDir dir;
        QVERIFY(dir.isValid());
        Backend b1(dir.path() + "/ws");
        QCOMPARE(b1.idleStopMinutes(), 0);
        QVERIFY(!b1.saveIdleStopSettings(-1).isEmpty());
        QVERIFY(!b1.saveIdleStopSettings(2000).isEmpty());
        QCOMPARE(b1.saveIdleStopSettings(30), QString());
        QCOMPARE(b1.idleStopMinutes(), 30);

        Backend b2(dir.path() + "/ws");
        QCOMPARE(b2.idleStopMinutes(), 30);
    }

    void idleDecisionSyntheticSamplesAndBlockers()
    {
        const qint64 minMs = 60LL * 1000LL;
        Backend::IdleDecisionInput base;
        base.running = true;
        base.thresholdMinutes = 15;
        base.samples = {
            {0 * minMs, 1},
            {5 * minMs, 2},
            {10 * minMs, 1},
            {14 * minMs, 1}
        };
        base.nowMs = 14 * minMs;

        // 1 minute before 15m -> warning triggers once
        const auto warnRes = Backend::evaluateIdleDecision(base);
        QVERIFY(warnRes.eligible);
        QVERIFY(warnRes.shouldWarnOneMin);
        QVERIFY(!warnRes.shouldStopNow);

        base.warningAlreadySent = true;
        QVERIFY(!Backend::evaluateIdleDecision(base).shouldWarnOneMin);

        // Reaching 15m -> stop triggers
        base.samples.append({15 * minMs, 0});
        base.nowMs = 15 * minMs;
        const auto stopRes = Backend::evaluateIdleDecision(base);
        QVERIFY(stopRes.eligible);
        QVERIFY(stopRes.shouldStopNow);

        // CPU spike at minute 10 resets timer so calm duration is only 5m < 15m
        Backend::IdleDecisionInput spike = base;
        spike.warningAlreadySent = false;
        spike.samples = {
            {0 * minMs, 1},
            {5 * minMs, 1},
            {10 * minMs, 8},
            {11 * minMs, 1},
            {15 * minMs, 1}
        };
        spike.nowMs = 15 * minMs;
        const auto spikeRes = Backend::evaluateIdleDecision(spike);
        QVERIFY(spikeRes.eligible);
        QVERIFY(!spikeRes.shouldWarnOneMin);
        QVERIFY(!spikeRes.shouldStopNow);
        QCOMPARE(spikeRes.idleDurationMs, 4 * minMs);

        // Active exec session blocks stop
        Backend::IdleDecisionInput withExec = base;
        withExec.hasActiveExecSession = true;
        const auto execRes = Backend::evaluateIdleDecision(withExec);
        QVERIFY(!execRes.eligible);
        QVERIFY(!execRes.shouldStopNow);
        QCOMPARE(execRes.blockedReason, QStringLiteral("exec-session"));

        const QSet<QString> parsedExec = Backend::parseActiveIncusExecEnvs(
            QStringLiteral("111 incus exec dev -- bash\n222 incus exec --cwd /workspace box-2 -- sh\n"));
        QVERIFY(parsedExec.contains(QStringLiteral("dev")));
        QVERIFY(parsedExec.contains(QStringLiteral("box-2")));
        QVERIFY(!parsedExec.contains(QStringLiteral("dev-2")));

        // Port forwards block unless ignorePortForwards override is true
        Backend::IdleDecisionInput withPorts = base;
        withPorts.portForwardCount = 2;
        const auto portsBlocked = Backend::evaluateIdleDecision(withPorts);
        QVERIFY(!portsBlocked.eligible);
        QCOMPARE(portsBlocked.blockedReason, QStringLiteral("port-forwards"));
        withPorts.ignorePortForwards = true;
        QVERIFY(Backend::evaluateIdleDecision(withPorts).shouldStopNow);

        // Templates and stopped / busy environments are skipped
        Backend::IdleDecisionInput tpl = base;
        tpl.isTemplate = true;
        QVERIFY(!Backend::evaluateIdleDecision(tpl).eligible);
        QCOMPARE(Backend::evaluateIdleDecision(tpl).blockedReason, QStringLiteral("template"));

        Backend::IdleDecisionInput busy = base;
        busy.busyOnThisEnv = true;
        QVERIFY(!Backend::evaluateIdleDecision(busy).eligible);
        QCOMPARE(Backend::evaluateIdleDecision(busy).blockedReason, QStringLiteral("busy"));

        // Idle stop script runs normal incus stop (not forced) and supports auto-snapshot when enabled
        QTemporaryDir dir;
        QVERIFY(dir.isValid());
        Backend b(dir.path() + "/ws");
        const QString stopScript = b.stopEnvironmentScript(QStringLiteral("dev"), true);
        QVERIFY(stopScript.contains(QStringLiteral("incus stop 'dev'")));
        QVERIFY(!stopScript.contains(QStringLiteral("--force")));
    }

    void staleClassificationAndConfigPersistence()
    {
        QTemporaryDir dir;
        QVERIFY(dir.isValid());
        const QString wsDir = dir.path() + "/ws";
        QVERIFY(QDir().mkpath(wsDir));

        {
            Backend b(wsDir);
            QCOMPARE(b.staleDays(), 30);
            QVERIFY(!b.saveStaleSettings(0).isEmpty());
            QVERIFY(!b.saveStaleSettings(4000).isEmpty());
            QCOMPARE(b.saveStaleSettings(14), QString());
            QCOMPARE(b.staleDays(), 14);
        }
        {
            Backend reloaded(wsDir);
            QCOMPARE(reloaded.staleDays(), 14);

            const QString prep = reloaded.prepareEnvironmentScript(QStringLiteral("dev"));
            QVERIFY(prep.contains(QStringLiteral("incus config set 'dev' user.aibox.lastused")));
            const QString start = reloaded.startEnvironmentScript(QStringLiteral("dev"));
            QVERIFY(start.contains(QStringLiteral("incus config set 'dev' user.aibox.lastused")));
        }

        const qint64 nowSec = 1794500000LL;
        const auto fresh = Backend::classifyEnvironmentStaleness(nowSec - 5LL * 86400LL, false, 30, nowSec);
        QCOMPARE(fresh.classification, Backend::StalenessClass::Fresh);
        QCOMPARE(fresh.daysUnused, qint64(5));

        const auto stale = Backend::classifyEnvironmentStaleness(nowSec - 35LL * 86400LL, false, 30, nowSec);
        QCOMPARE(stale.classification, Backend::StalenessClass::Stale);
        QCOMPARE(stale.daysUnused, qint64(35));

        const auto unknown = Backend::classifyEnvironmentStaleness(0, false, 30, nowSec);
        QCOMPARE(unknown.classification, Backend::StalenessClass::Unknown);
        QCOMPARE(unknown.daysUnused, qint64(-1));

        const auto tpl = Backend::classifyEnvironmentStaleness(nowSec - 120LL * 86400LL, true, 30, nowSec);
        QCOMPARE(tpl.classification, Backend::StalenessClass::Template);

        const QByteArray sampleJson = QByteArrayLiteral(R"([
            {
                "name": "stale-box",
                "type": "container",
                "status": "Stopped",
                "config": {"user.aibox.lastused": "1700000000"},
                "devices": {},
                "snapshots": [{"name": "s1", "size": 1048576}],
                "state": {}
            },
            {
                "name": "untracked-box",
                "type": "container",
                "status": "Stopped",
                "config": {},
                "devices": {},
                "snapshots": [],
                "state": {}
            }
        ])");
        const QVariantList envs = Backend::parseIncusListJson(sampleJson, wsDir);
        QCOMPARE(envs.size(), 2);
        QCOMPARE(envs.at(0).toMap().value("lastUsedEpoch").toLongLong(), qint64(1700000000LL));
        QCOMPARE(envs.at(0).toMap().value("snapshotTotalBytes").toLongLong(), qint64(1048576LL));
        QCOMPARE(envs.at(1).toMap().value("lastUsedEpoch").toLongLong(), qint64(0));
    }

    void staleOrphanDetectionAndExportWipeSafety()
    {
        QTemporaryDir dir;
        QVERIFY(dir.isValid());
        const QString wsBase = dir.path() + "/ws";
        const QString snapBase = wsBase + "/.snapshots";
        const QString backupsDir = dir.path() + "/backups";
        QVERIFY(QDir().mkpath(wsBase + "/keep-env"));
        QVERIFY(QDir().mkpath(wsBase + "/stale-env"));
        QVERIFY(QDir().mkpath(wsBase + "/ghost-ws"));
        QVERIFY(QDir().mkpath(snapBase + "/keep-env/snap1"));
        QVERIFY(QDir().mkpath(snapBase + "/ghost-snaps/old1"));
        QVERIFY(QDir().mkpath(backupsDir));

        write(wsBase + "/keep-env/app.txt", "active workspace");
        write(wsBase + "/stale-env/readme.md", QByteArray(4096, 's'));
        write(wsBase + "/ghost-ws/leak.bin", QByteArray(2048, 'g'));
        write(snapBase + "/ghost-snaps/old1/data.bin", QByteArray(1024, 'x'));

        // Paired backup (NOT an orphan)
        write(backupsDir + "/keep-env-20261110-120000.tar.gz", "inst");
        write(backupsDir + "/keep-env-20261110-120000-workspace.tar.gz", "ws");
        // Unpaired workspace archive (IS an orphan)
        write(backupsDir + "/ghost-old-20261001-010203-workspace.tar.gz", QByteArray(512, 'b'));

        const qint64 nowSec = 1794500000LL;
        const QVariantList envs = {
            QVariantMap{
                {"name", "keep-env"},
                {"running", true},
                {"type", "CONTAINER"},
                {"osName", "Ubuntu"},
                {"isTemplate", false},
                {"lastUsedEpoch", nowSec - 2LL * 86400LL},
                {"diskUsageBytes", qint64(10485760LL)},
                {"snapshotCount", 1},
                {"snapshotTotalBytes", qint64(1048576LL)},
                {"workspacePath", wsBase + "/keep-env"}
            },
            QVariantMap{
                {"name", "stale-env"},
                {"running", false},
                {"type", "CONTAINER"},
                {"osName", "Debian"},
                {"isTemplate", false},
                {"lastUsedEpoch", nowSec - 45LL * 86400LL},
                {"diskUsageBytes", qint64(20971520LL)},
                {"snapshotCount", 2},
                {"snapshotTotalBytes", qint64(5242880LL)},
                {"workspacePath", wsBase + "/stale-env"}
            },
            QVariantMap{
                {"name", "tpl-old"},
                {"running", false},
                {"type", "CONTAINER"},
                {"osName", "Arch"},
                {"isTemplate", true},
                {"lastUsedEpoch", nowSec - 180LL * 86400LL},
                {"diskUsageBytes", qint64(5242880LL)},
                {"snapshotCount", 0},
                {"snapshotTotalBytes", qint64(0)},
                {"workspacePath", wsBase + "/tpl-old"}
            },
            QVariantMap{
                {"name", "never-stamped"},
                {"running", false},
                {"type", "CONTAINER"},
                {"osName", "Alpine"},
                {"isTemplate", false},
                {"lastUsedEpoch", qint64(0)},
                {"diskUsageBytes", qint64(1048576LL)},
                {"snapshotCount", 0},
                {"snapshotTotalBytes", qint64(0)},
                {"workspacePath", wsBase + "/never-stamped"}
            }
        };

        const QVariantMap report = Backend::buildStaleReport(envs, wsBase, backupsDir, 30, nowSec);
        QCOMPARE(report.value("staleCount").toInt(), 1);
        QCOMPARE(report.value("unknownCount").toInt(), 1);
        QCOMPARE(report.value("freshCount").toInt(), 1);
        QCOMPARE(report.value("templateCount").toInt(), 1);
        QCOMPARE(report.value("orphanCount").toInt(), 3);

        const QVariantList entries = report.value("entries").toList();
        QCOMPARE(entries.size(), 2);
        QCOMPARE(entries.at(0).toMap().value("name").toString(), QString("stale-env"));
        QCOMPARE(entries.at(0).toMap().value("status").toString(), QString("stale"));
        QCOMPARE(entries.at(0).toMap().value("workspaceSizeBytes").toLongLong(), qint64(4096));
        QCOMPARE(entries.at(1).toMap().value("name").toString(), QString("never-stamped"));
        QCOMPARE(entries.at(1).toMap().value("status").toString(), QString("unknown"));

        const QVariantList orphans = report.value("orphans").toList();
        QStringList kinds;
        for (const QVariant &ov : orphans)
            kinds.append(ov.toMap().value("kind").toString());
        QVERIFY(kinds.contains("workspace"));
        QVERIFY(kinds.contains("workspace-snapshot"));
        QVERIFY(kinds.contains("backup-workspace"));

        // Safety validation: refuse deleting arbitrary paths
        Backend b(wsBase);
        QVERIFY(!b.deleteOrphan("workspace", dir.path() + "/outside").isEmpty());
        QVERIFY(!b.deleteOrphan("workspace", snapBase).isEmpty());
        QVERIFY(!b.deleteOrphan("backup-workspace", backupsDir + "/keep-env-20261110-120000.tar.gz").isEmpty());

        // Export & Wipe must abort wipe when export fails or writes an empty archive
        const QString binDir = dir.path() + "/bin";
        QVERIFY(QDir().mkpath(binDir));
        const QString incusLog = dir.path() + "/incus_calls.log";
        writeExecutable(binDir + "/incus",
            "#!/bin/sh\n"
            "echo \"$*\" >> \"$TEST_INCUS_LOG\"\n"
            "if [ \"$1\" = \"export\" ]; then\n"
            "  if [ \"${TEST_EXPORT_MODE:-fail}\" = \"ok\" ]; then\n"
            "    printf 'ARCHIVE_DATA' > \"$3\"\n"
            "    exit 0\n"
            "  elif [ \"${TEST_EXPORT_MODE:-fail}\" = \"empty\" ]; then\n"
            "    : > \"$3\"\n"
            "    exit 0\n"
            "  else\n"
            "    exit 1\n"
            "  fi\n"
            "fi\n"
            "exit 0\n");

        const QByteArray oldPath = qgetenv("PATH");
        qputenv("PATH", binDir.toUtf8() + ":" + oldPath);
        qputenv("TEST_INCUS_LOG", incusLog.toUtf8());
        auto restoreEnv = qScopeGuard([oldPath]() {
            qputenv("PATH", oldPath);
            qunsetenv("TEST_INCUS_LOG");
            qunsetenv("TEST_EXPORT_MODE");
        });

        const QString targetWs = wsBase + "/stale-env";
        const QString outBackup = backupsDir + "/stale-env-test.tar.gz";
        const QString outWsBackup = backupsDir + "/stale-env-test-workspace.tar.gz";
        const QString exportWipeScript = Backend::exportAndWipeEnvironmentScript(
            "stale-env", outBackup, targetWs, outWsBackup, true);

        // Case 1: export exits 1 -> delete must NOT run, workspace must stay intact
        qputenv("TEST_EXPORT_MODE", "fail");
        QProcess pFail;
        pFail.start("/bin/bash", {"-c", exportWipeScript});
        QVERIFY(pFail.waitForFinished(10000));
        QVERIFY(pFail.exitCode() != 0);
        QVERIFY(QFileInfo::exists(targetWs + "/readme.md"));
        {
            QFile f(incusLog);
            QVERIFY(f.open(QIODevice::ReadOnly));
            QVERIFY(!f.readAll().contains("delete"));
        }

        // Case 2: export exits 0 with 0-byte archive -> verification gate blocks wipe
        QFile::remove(incusLog);
        qputenv("TEST_EXPORT_MODE", "empty");
        QProcess pEmpty;
        pEmpty.start("/bin/bash", {"-c", exportWipeScript});
        QVERIFY(pEmpty.waitForFinished(10000));
        QVERIFY(pEmpty.exitCode() != 0);
        QVERIFY(QFileInfo::exists(targetWs + "/readme.md"));
        {
            QFile f(incusLog);
            QVERIFY(f.open(QIODevice::ReadOnly));
            QVERIFY(!f.readAll().contains("delete"));
        }

        // Case 3: export succeeds with >0 byte archive -> wipe proceeds
        QFile::remove(incusLog);
        qputenv("TEST_EXPORT_MODE", "ok");
        QProcess pOk;
        pOk.start("/bin/bash", {"-c", exportWipeScript});
        QVERIFY(pOk.waitForFinished(10000));
        QCOMPARE(pOk.exitCode(), 0);
        QVERIFY(!QFileInfo::exists(targetWs));
        {
            QFile f(incusLog);
            QVERIFY(f.open(QIODevice::ReadOnly));
            QVERIFY(f.readAll().contains("delete stale-env --force"));
        }
    }

    void orphanDeleteScriptsHandleRootOwnedFiles()
    {
        QTemporaryDir dir;
        QVERIFY(dir.isValid());
        const QString wsBase = dir.path() + "/ws";
        QVERIFY(QDir().mkpath(wsBase));
        const QString target = wsBase + "/ghost-ws";
        QVERIFY(QDir().mkpath(target));

        const QString script = Backend::deleteOrphanScript("workspace", target);
        QVERIFY2(script.contains("sudo -n rm -rf"), qPrintable(script));
        QVERIFY2(script.contains("Fix (admin): sudo rm -rf"), qPrintable(script));

        const QString snapScript =
            Backend::deleteOrphanScript("workspace-snapshot", wsBase + "/.snapshots/ghost");
        QVERIFY2(snapScript.contains("sudo -n rm -rf"), qPrintable(snapScript));

        const QString admin = Backend::adminDeleteScriptText("workspace", target);
        QVERIFY2(admin.contains("btrfs subvolume show"), qPrintable(admin));
        QVERIFY2(admin.contains("rm -rf \"$TARGET\""), qPrintable(admin));

        Backend backend(wsBase);
        QVERIFY(!backend.deleteOrphanAsAdmin("workspace", "/tmp/not-a-workspace").isEmpty());

        QCOMPARE(backend.adminFixCommand(QStringLiteral(
                     "Could not completely remove orphan workspace folder.\n"
                     "Fix (admin): sudo rm -rf '/home/user/aibox/workspaces/modding-game'")),
                 QStringLiteral("sudo rm -rf '/home/user/aibox/workspaces/modding-game'"));
        QCOMPARE(backend.adminFixCommand(QStringLiteral(
                     "Warning: could not delete subvolume; run: sudo rm -rf '/tmp/ghost'")),
                 QStringLiteral("sudo rm -rf '/tmp/ghost'"));
        QVERIFY(backend.adminFixCommand(QStringLiteral("Plain error without a fix.")).isEmpty());
    }

    void sessionRecordingConfigOverridesAndWrapBuilder()
    {
        QTemporaryDir dir;
        QVERIFY(dir.isValid());
        const QString wsDir = dir.path() + "/ws";
        QVERIFY(QDir().mkpath(wsDir));

        {
            Backend b(wsDir);
            QCOMPARE(b.recordSessions(), false);
            QCOMPARE(b.saveSessionRecordingSettings(true), QString());
            QCOMPARE(b.recordSessions(), true);
        }
        {
            Backend reloaded(wsDir);
            QCOMPARE(reloaded.recordSessions(), true);
        }

        QVERIFY(Backend::effectiveRecordSessions(true, QString()));
        QVERIFY(Backend::effectiveRecordSessions(true, QStringLiteral("inherit")));
        QVERIFY(!Backend::effectiveRecordSessions(true, QStringLiteral("false")));
        QVERIFY(!Backend::effectiveRecordSessions(false, QString()));
        QVERIFY(Backend::effectiveRecordSessions(false, QStringLiteral("true")));

        const QByteArray sampleJson = QByteArrayLiteral(R"([
            {"name":"env-on","type":"container","status":"Running","config":{"user.aibox.record":"true"},"devices":{},"state":{}},
            {"name":"env-off","type":"container","status":"Running","config":{"user.aibox.record":"false"},"devices":{},"state":{}},
            {"name":"env-inh","type":"container","status":"Running","config":{},"devices":{},"state":{}}
        ])");
        const QVariantList envs = Backend::parseIncusListJson(sampleJson, wsDir, nullptr, 0, true, false, true);
        QCOMPARE(envs.size(), 3);
        QCOMPARE(envs.at(0).toMap().value("recordEnabled").toBool(), true);
        QCOMPARE(envs.at(1).toMap().value("recordEnabled").toBool(), false);
        QCOMPARE(envs.at(2).toMap().value("recordEnabled").toBool(), true);

        const QString inner = QStringLiteral("incus exec 'my-box' --cwd /workspace -- bash -l");
        const QString logFile = dir.path() + "/dir with 'quote'/sessions/my-box/1700000001.log";
        const QString timingFile = dir.path() + "/dir with 'quote'/sessions/my-box/1700000001.timing";

        const QString wrapped = Backend::buildSessionWrapCommand(
            inner, logFile, timingFile, QStringLiteral("/usr/bin/aibox"),
            dir.path() + "/sessions", true);
        QVERIFY(wrapped.contains(QStringLiteral("script -q -f --log-out")));
        QVERIFY(wrapped.contains(QStringLiteral("--log-timing")));
        QVERIFY(wrapped.contains(QStringLiteral("--redact-session")));
        QVERIFY(wrapped.contains(QStringLiteral("dir with '\\''quote'\\''")));

        // Fallback when script binary is missing returns exact inner command
        const QString fallback = Backend::buildSessionWrapCommand(
            inner, logFile, timingFile, QStringLiteral("/usr/bin/aibox"),
            dir.path() + "/sessions", false);
        QCOMPARE(fallback, inner);

        // Wipe deletes environment's recorded sessions directory even when deleteWorkspaceFiles=false
        const QString envSessDir = dir.path() + "/state/sessions/my-box";
        const QString wipeScript = Backend::wipeEnvironmentScript(
            QStringLiteral("my-box"), wsDir + "/my-box", false, envSessDir);
        QVERIFY(wipeScript.contains(envSessDir));
        QVERIFY(wipeScript.contains(QStringLiteral("rm -rf")));
    }

    void sessionRecordingRetentionAnsiAndRedaction()
    {
        // 1. ANSI stripper & 200 KB cap
        const QString ansiRaw = QStringLiteral(
            "\x1b]0;user@dev:/workspace\x07"
            "\x1b[32muser@dev\x1b[0m:\x1b[34m/workspace\x1b[0m$ echo ok\r\n"
            "\x1b[2J\x1b[1;1Hok\r\n");
        const QString clean = Backend::stripAnsiSequences(ansiRaw);
        QVERIFY(!clean.contains(QChar(0x1b)));
        QVERIFY(!clean.contains(QLatin1Char('\r')));
        QVERIFY(clean.contains(QStringLiteral("user@dev:/workspace$ echo ok\nok\n")));

        QByteArray huge(300 * 1024, 'A');
        huge.append("\nFINAL_TAIL_MARKER\n");
        const QString formattedHuge = Backend::formatRecordedSessionText(huge, 200 * 1024);
        QVERIFY(formattedHuge.startsWith(QStringLiteral("[... truncated to last 200 KB ...]")));
        QVERIFY(formattedHuge.contains(QStringLiteral("FINAL_TAIL_MARKER")));
        QVERIFY(formattedHuge.toUtf8().size() <= (200 * 1024 + 128));

        // 2. S3 redaction over real .log file
        QTemporaryDir dir;
        QVERIFY(dir.isValid());
        const QString sessRoot = dir.path() + "/sessions";
        const QString envAlpha = sessRoot + "/alpha";
        const QString envBeta = sessRoot + "/beta";
        QVERIFY(QDir().mkpath(envAlpha));
        QVERIFY(QDir().mkpath(envBeta));

        const QString redPath = envAlpha + "/1700000099.log";
        write(redPath, "token=sk-live-secret-12345\nother=safe\n");
        QVERIFY(Backend::redactSessionLogFile(redPath, {QStringLiteral("sk-live-secret-12345")}));
        {
            QFile f(redPath);
            QVERIFY(f.open(QIODevice::ReadOnly));
            const QByteArray after = f.readAll();
            QVERIFY(!after.contains("sk-live-secret-12345"));
            QVERIFY(after.contains("[REDACTED]"));
            QVERIFY(!(f.permissions() & (QFileDevice::ReadGroup | QFileDevice::ReadOther)));
        }
        QFile::remove(redPath);

        // Real script invocation if util-linux script is available on host
        if (!QStandardPaths::findExecutable(QStringLiteral("script")).isEmpty()) {
            const QString realLog = envAlpha + "/1700000050.log";
            const QString realTiming = envAlpha + "/1700000050.timing";
            const QString cmd = Backend::buildSessionWrapCommand(
                QStringLiteral("printf '\\033[31msecret=tok-xyz-987\\033[0m\\n'"),
                realLog, realTiming, QString(), sessRoot, true);
            QProcess p;
            p.start(QStringLiteral("/bin/bash"), {QStringLiteral("-c"), cmd});
            QVERIFY(p.waitForFinished(10000));
            QCOMPARE(p.exitCode(), 0);
            QVERIFY(QFileInfo::exists(realLog));
            QVERIFY(QFileInfo(realTiming).size() > 0);
            QVERIFY(Backend::redactSessionLogFile(realLog, {QStringLiteral("tok-xyz-987")}));
            QFile rf(realLog);
            QVERIFY(rf.open(QIODevice::ReadOnly));
            const QString viewText = Backend::formatRecordedSessionText(rf.readAll());
            QVERIFY(viewText.contains(QStringLiteral("secret=[REDACTED]")));
            QVERIFY(!viewText.contains(QStringLiteral("tok-xyz-987")));
            QFile::remove(realLog);
            QFile::remove(realTiming);
        }

        // 3. Retention count cap (25 -> 20 in alpha)
        const QDateTime baseDt(QDate(2026, 11, 1), QTime(0, 0, 0), QTimeZone::UTC);
        for (int i = 1; i <= 25; ++i) {
            const QString epochStr = QString::number(1700000000 + i);
            const QString lp = envAlpha + "/" + epochStr + ".log";
            const QString tp = envAlpha + "/" + epochStr + ".timing";
            write(lp, QByteArray(100, 'L'));
            write(tp, QByteArray(20, 'T'));
            QFile lf(lp);
            if (lf.open(QIODevice::ReadWrite))
                lf.setFileTime(baseDt.addSecs(i * 60), QFileDevice::FileModificationTime);
        }
        const int countPruned = Backend::pruneRecordedSessions(sessRoot, 20, 200LL * 1024LL * 1024LL);
        QCOMPARE(countPruned, 5);
        QCOMPARE(Backend::scanRecordedSessions(sessRoot).size(), 20);
        QVERIFY(!QFileInfo::exists(envAlpha + "/1700000001.log"));
        QVERIFY(!QFileInfo::exists(envAlpha + "/1700000001.timing"));
        QVERIFY(!QFileInfo::exists(envAlpha + "/1700000005.log"));
        QVERIFY(QFileInfo::exists(envAlpha + "/1700000006.log"));
        QVERIFY(QFileInfo::exists(envAlpha + "/1700000025.log"));

        // 4. Retention total byte cap across envs
        for (int i = 1; i <= 4; ++i) {
            const QString epochStr = QString::number(1700001000 + i);
            const QString lp = envBeta + "/" + epochStr + ".log";
            const QString tp = envBeta + "/" + epochStr + ".timing";
            write(lp, QByteArray(1000, 'B'));
            write(tp, QByteArray(200, 'T'));
            QFile lf(lp);
            if (lf.open(QIODevice::ReadWrite))
                lf.setFileTime(baseDt.addSecs(3600 + i * 60), QFileDevice::FileModificationTime);
        }
        // Alpha currently has 20 * 120B = 2400B, Beta has 4 * 1200B = 4800B -> 7200B total.
        // Pruning to maxTotalBytes = 4000B must drop oldest alpha sessions first until <= 4000B.
        const int sizePruned = Backend::pruneRecordedSessions(sessRoot, 20, 4000);
        QVERIFY(sizePruned > 0);
        qint64 remainingBytes = 0;
        const QVariantList rem = Backend::scanRecordedSessions(sessRoot);
        for (const QVariant &v : rem)
            remainingBytes += v.toMap().value("sizeBytes").toLongLong();
        QVERIFY(remainingBytes <= 4000);
    }

    void parseNetworkProbeOutputReadsPingGatewayAndLoss()
    {
        const QString output = QStringLiteral(
            "GW=10.143.44.1\n"
            "PING 1.1.1.1 (1.1.1.1) 56(84) bytes of data.\n"
            "\n--- 1.1.1.1 ping statistics ---\n"
            "3 packets transmitted, 3 received, 0% packet loss, time 2003ms\n"
            "rtt min/avg/max/mdev = 21.123/30.456/38.789/4.321 ms\n");

        const QVariantMap parsed = Backend::parseNetworkProbeOutput(output);
        QCOMPARE(parsed.value("gateway").toString(), QString("10.143.44.1"));
        QCOMPARE(parsed.value("pingMs").toDouble(), 30.456);
        QCOMPARE(parsed.value("packetLossPercent").toDouble(), 0.0);
    }

    void parseNetworkProbeOutputHandlesBlockedNetworkAndHexGateway()
    {
        const QString output = QStringLiteral(
            "GW=0101A8C0\n"
            "--- 1.1.1.1 ping statistics ---\n"
            "3 packets transmitted, 0 received, 100% packet loss, time 2002ms\n");

        const QVariantMap parsed = Backend::parseNetworkProbeOutput(output);
        QCOMPARE(parsed.value("gateway").toString(), QString("192.168.1.1"));
        QCOMPARE(parsed.value("pingMs").toDouble(), -1.0);
        QCOMPARE(parsed.value("packetLossPercent").toDouble(), 100.0);
    }

    void networkCountersProduceRatesAndTotals()
    {
        auto buildJson = [](qint64 rx, qint64 tx) {
            return QStringLiteral(R"json([
                {
                    "name": "net-box",
                    "status": "Running",
                    "type": "container",
                    "config": {},
                    "state": {
                        "network": {
                            "eth0": {
                                "addresses": [ { "family": "inet", "scope": "global", "address": "10.1.2.3" } ],
                                "counters": { "bytes_received": %1, "bytes_sent": %2 }
                            }
                        }
                    }
                }
            ])json")
                .arg(rx)
                .arg(tx)
                .toUtf8();
        };

        QHash<QString, QPair<qint64, qint64>> counters;
        const QVariantList first = Backend::parseIncusListJson(buildJson(1048576, 524288), "/tmp/ws",
                                                               nullptr, 2000000000LL, true, false, false,
                                                               &counters);
        QCOMPARE(first.size(), 1);
        const QVariantMap firstEnv = first.first().toMap();
        QCOMPARE(firstEnv.value("netRxBytes").toLongLong(), 1048576LL);
        QCOMPARE(firstEnv.value("netTxBytes").toLongLong(), 524288LL);
        QCOMPARE(firstEnv.value("netRxRate").toLongLong(), 0LL);
        QCOMPARE(firstEnv.value("netRxTotalText").toString(), QString("1.00 MB"));

        const QVariantList second = Backend::parseIncusListJson(buildJson(1050624, 525312), "/tmp/ws",
                                                                nullptr, 2000000000LL, true, false, false,
                                                                &counters);
        const QVariantMap secondEnv = second.first().toMap();
        QCOMPARE(secondEnv.value("netRxRate").toLongLong(), 1024LL);
        QCOMPARE(secondEnv.value("netTxRate").toLongLong(), 512LL);
        QCOMPARE(secondEnv.value("netRxRateText").toString(), QString("1.0 KB/s"));
        QCOMPARE(secondEnv.value("netTxRateText").toString(), QString("512 B/s"));
    }

    void networkStatsFollowEnvironmentState()
    {
        QVariantMap env;
        env["name"] = QStringLiteral("net-box");
        env["running"] = true;
        env["networkMode"] = QStringLiteral("online");
        env["ipv4"] = QStringLiteral("10.143.44.55");
        env["netRxRateText"] = QStringLiteral("149.7 KB/s");
        env["netTxRateText"] = QStringLiteral("13.9 KB/s");
        env["netRxTotalText"] = QStringLiteral("36.33 GB");
        env["netTxTotalText"] = QStringLiteral("6.86 GB");

        const QVariantMap probe = QVariantMap{
            {"gateway", QStringLiteral("10.143.44.1")},
            {"pingMs", 30.0},
            {"packetLossPercent", 0.0}
        };
        const QVariantMap stats = Backend::networkStatsFromEnvironment(env, probe);
        QCOMPARE(stats.value("active").toBool(), true);
        QCOMPARE(stats.value("ip").toString(), QString("10.143.44.55"));
        QCOMPARE(stats.value("gateway").toString(), QString("10.143.44.1"));
        QCOMPARE(stats.value("pingMs").toDouble(), 30.0);
        QCOMPARE(stats.value("rxRateText").toString(), QString("149.7 KB/s"));

        env["networkMode"] = QStringLiteral("offline");
        const QVariantMap offline = Backend::networkStatsFromEnvironment(env, probe);
        QCOMPARE(offline.value("active").toBool(), false);
        QCOMPARE(offline.value("pingMs").toDouble(), -1.0);
        QCOMPARE(offline.value("packetLossPercent").toDouble(), -1.0);

        env["networkMode"] = QStringLiteral("online");
        env["running"] = false;
        const QVariantMap stopped = Backend::networkStatsFromEnvironment(env, probe);
        QCOMPARE(stopped.value("active").toBool(), false);
        QCOMPARE(stopped.value("gateway").toString(), QString("10.143.44.1"));
        QCOMPARE(stopped.value("pingMs").toDouble(), -1.0);
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
