import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts
import "components"
import "dialogs"
import "panels"

ApplicationWindow {
    id: win
    width: 1080
    height: 720
    minimumWidth: 960
    minimumHeight: 560
    visible: true
    title: qsTr("AIBox")
    color: theme.background

    Material.theme: Material.Dark
    Material.accent: theme.accent
    Material.roundedScale: theme.radius

    property int activeTab: 0
    property bool helpVisible: false
    property bool toolsVisible: false
    property bool envActionsVisible: false
    property bool createVisible: false
    property bool cloneVisible: false
    property bool wipeVisible: false
    property bool deleteWorkspaceOnWipe: false
    property bool settingsVisible: false
    property int settingsSection: 0
    property string configNotice: ""
    property bool configNoticeError: false
    property string poolResizeDraft: ""
    property bool poolResizeConfirmVisible: false
    property string poolResizeError: ""
    property bool osEditorVisible: false
    property string osEditorOriginalId: ""
    property string osEditorId: ""
    property string osEditorAlias: ""
    property string osEditorName: ""
    property string osEditorBadge: ""
    property string osEditorDescription: ""
    property string osEditorError: ""
    property string firstRunError: ""

    // Form state for New Environment dialog
    property string newEnvName: ""
    property string newEnvOsId: backend.defaultOsId
    property string newEnvPreset: backend.defaultPresetId
    property string newEnvDisk: backend.defaultDisk
    property string newEnvRam: backend.defaultRam
    property int newEnvCpu: backend.defaultCpu
    property bool createFromTemplate: false
    property string newEnvTemplateName: ""
    property string newEnvSharedPath: ""

    // Form state for Clone dialog and Network allowlist editor
    property string cloneSourceName: ""
    property string cloneNewName: ""
    property string netAllowlistDraft: ""
    property bool netAllowlistEditing: false
    property var diskChoiceDraft: []
    property var ramChoiceDraft: []
    property var cpuChoiceDraft: []
    property bool autoSnapEnabledDraft: backend.autoSnapshotEnabled
    property bool watchdogEnabledDraft: backend.watchdogEnabled
    property bool watchdogAutoStopDraft: backend.watchdogAutoStop
    property int idleStopMinutesDraft: backend.idleStopMinutes
    property int staleDaysDraft: backend.staleDays
    property bool recordSessionsDraft: backend.recordSessions
    property int historyTab: 0
    property string selectedSessionId: ""
    property string selectedSessionLogPath: ""
    property string selectedSessionTimingPath: ""
    property bool selectedSessionHasTiming: false
    property string selectedSessionText: ""
    property bool staleReportVisible: false
    property string staleActionError: ""
    property string staleConfirmAction: ""
    property string staleConfirmTarget: ""
    property string staleConfirmKind: ""
    property string staleConfirmTitle: ""
    property string staleConfirmDetail: ""
    property bool staleConfirmDeleteWs: true
    property string newEnvProcLimit: backend.defaultProcessLimit
    property string newEnvIoLimit: backend.defaultIoLimit
    property var newEnvProvisionBundles: []
    property bool historyVisible: false
    property string selectedHistoryId: ""
    property string selectedHistoryLog: ""
    property bool egressVisible: false
    property bool portsVisible: false
    property string portForwardError: ""
    property bool mountsVisible: false
    property string extraMountError: ""
    property bool mountReadOnlyDraft: true
    property bool mountConfirmRwDraft: false
    property bool editQuotasVisible: false
    property string editQuotaError: ""
    property string editQuotaEnvName: ""
    property bool editQuotaIsVm: false
    property int editQuotaCpu: 4
    property string editQuotaRam: ""
    property string editQuotaDisk: ""
    property string editQuotaProc: ""
    property string editQuotaIo: ""
    property bool backupsVisible: false
    property bool backupIncludeWorkspace: true
    property string backupError: ""
    property string importBackupFile: ""
    property string importWsFile: ""
    property string importNewName: ""
    property bool restoreDialogVisible: false
    property string restoreSnapName: ""
    property bool restoreHasSystem: true
    property bool restoreHasWorkspace: false
    property string restoreMode: "both"
    property bool changeReviewVisible: false
    property string diffSnapName: ""
    property string diffSelectedPath: ""
    property string diffSelectedStatus: ""
    property string diffSelectedType: ""
    property bool diffEditExcludes: false
    property string diffExcludesDraft: ""
    property string diffActionError: ""
    property bool secretsVisible: false
    property string secretsScope: "global"
    property string secretsEnvName: ""
    property string secretNameDraft: ""
    property string secretValueDraft: ""
    property bool secretDraftShow: false
    property string secretFormError: ""
    property var secretRevealMap: ({})

    readonly property bool anyModalOpen: helpVisible || toolsVisible || envActionsVisible || createVisible || cloneVisible || wipeVisible || restoreDialogVisible || changeReviewVisible || secretsVisible || staleReportVisible || settingsVisible || historyVisible || egressVisible || portsVisible || mountsVisible || editQuotasVisible || backupsVisible || backend.firstRun

    function openStaleReportDialog() {
        win.staleActionError = ""
        win.staleConfirmAction = ""
        win.staleConfirmTarget = ""
        win.staleConfirmKind = ""
        win.staleConfirmTitle = ""
        win.staleConfirmDetail = ""
        win.staleConfirmDeleteWs = true
        backend.refreshStaleReport()
        win.staleReportVisible = true
    }

    function openSecretsDialog() {
        win.secretsScope = "global"
        win.secretsEnvName = ""
        win.secretNameDraft = ""
        win.secretValueDraft = ""
        win.secretDraftShow = false
        win.secretFormError = ""
        win.secretRevealMap = ({})
        backend.reloadSecrets()
        win.secretsVisible = true
    }

    function openEnvironmentSecretsDialog(envName) {
        const name = envName || (backend.selectedEnvironment.name || "")
        if (name.length === 0)
            return
        win.secretsScope = "environment"
        win.secretsEnvName = name
        win.secretNameDraft = ""
        win.secretValueDraft = ""
        win.secretDraftShow = false
        win.secretFormError = ""
        win.secretRevealMap = ({})
        backend.reloadSecrets()
        backend.reloadEnvironmentSecrets(name)
        win.secretsVisible = true
    }

    function toggleSecretReveal(name) {
        const next = Object.assign({}, win.secretRevealMap || {})
        next[name] = !(next[name] === true)
        win.secretRevealMap = next
    }

    function toggleEnvSecret(secretName) {
        const env = backend.selectedEnvironment
        const envName = win.secretsScope === "environment" ? win.secretsEnvName
                                                           : (env ? (env.name || "") : "")
        if (!env || !env.name || env.name !== envName || backend.busy)
            return
        const cur = (env.secretNames || []).slice()
        const idx = cur.indexOf(secretName)
        if (idx >= 0)
            cur.splice(idx, 1)
        else
            cur.push(secretName)
        backend.setEnvironmentSecrets(env.name, cur)
    }

    function toggleGlobalSecretVisibility(secretName) {
        const rows = backend.secretsList || []
        let visible = true
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i] || {}
            if (row.name === secretName) {
                visible = row.visibleInEnvironments === true
                break
            }
        }
        const err = backend.setGlobalSecretInstanceVisible(secretName, !visible)
        if (err.length > 0)
            win.secretFormError = err
        else
            win.secretFormError = ""
    }

    function toggleLocalSecret(secretName) {
        const envName = win.secretsScope === "environment" ? win.secretsEnvName : ""
        if (envName.length === 0 || backend.busy)
            return
        const rows = backend.environmentSecrets || []
        let assigned = false
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i] || {}
            if (row.name === secretName && row.scope === "environment") {
                assigned = row.assigned === true
                break
            }
        }
        const err = backend.setEnvironmentLocalSecretEnabled(envName, secretName, !assigned)
        if (err.length > 0)
            win.secretFormError = err
        else
            win.secretFormError = ""
    }

    function openChangeReviewDialog(snapName) {
        const env = backend.selectedEnvironment
        if (!env || !env.name)
            return
        const wsSnaps = env.workspaceSnapshots || []
        let chosen = (snapName && snapName.length > 0) ? snapName : ""
        if (chosen.length === 0 && wsSnaps.length > 0)
            chosen = wsSnaps[wsSnaps.length - 1]
        win.diffSnapName = chosen
        win.diffSelectedPath = ""
        win.diffSelectedStatus = ""
        win.diffSelectedType = ""
        win.diffEditExcludes = false
        win.diffExcludesDraft = (backend.diffExcludes || []).join(", ")
        win.diffActionError = ""
        win.changeReviewVisible = true
        if (chosen.length > 0)
            backend.loadWorkspaceDiff(env.name, chosen)
    }

    function openRestoreDialog(snapName, hasSystem, hasWorkspace) {
        win.restoreSnapName = snapName || ""
        win.restoreHasSystem = !!hasSystem
        win.restoreHasWorkspace = !!hasWorkspace
        if (hasSystem && hasWorkspace)
            win.restoreMode = "both"
        else if (hasWorkspace)
            win.restoreMode = "workspace"
        else
            win.restoreMode = "system"
        win.restoreDialogVisible = true
    }

    function openCreateDialog(osId, templateName) {
        win.newEnvName = ""
        win.newEnvSharedPath = ""
        createDialog.resetSharedPath()
        win.newEnvOsId = (osId && osId.length > 0) ? osId : backend.defaultOsId
        win.newEnvPreset = backend.defaultPresetId
        win.newEnvDisk = backend.defaultDisk
        win.newEnvRam = backend.defaultRam
        win.newEnvCpu = backend.defaultCpu
        win.newEnvProcLimit = backend.defaultProcessLimit
        win.newEnvIoLimit = backend.defaultIoLimit
        win.newEnvProvisionBundles = []
        if (templateName && templateName.length > 0) {
            win.createFromTemplate = true
            win.newEnvTemplateName = templateName
        } else {
            win.createFromTemplate = false
            win.newEnvTemplateName = backend.templates.length > 0 ? (backend.templates[0].name || "") : ""
        }
        backend.clearError()
        win.createVisible = true
    }

    function toggleProvisionBundle(bundleId) {
        var next = win.newEnvProvisionBundles.slice()
        const idx = next.indexOf(bundleId)
        if (idx >= 0)
            next.splice(idx, 1)
        else
            next.push(bundleId)
        win.newEnvProvisionBundles = next
    }

    function openPortsDialog() {
        win.portForwardError = ""
        win.portsVisible = true
    }

    function openMountsDialog() {
        win.extraMountError = ""
        win.mountReadOnlyDraft = true
        win.mountConfirmRwDraft = false
        win.mountsVisible = true
    }

    function openEditQuotasDialog() {
        const env = backend.selectedEnvironment
        if (!env || !env.name)
            return
        win.editQuotaEnvName = env.name
        win.editQuotaIsVm = (env.type === "VIRTUAL-MACHINE" || env.type === "VM")
        win.editQuotaCpu = env.cpuCores || backend.defaultCpu
        win.editQuotaRam = (env.memoryLimit && env.memoryLimit !== "Dynamic") ? env.memoryLimit : backend.defaultRam
        win.editQuotaDisk = (env.diskLimit && env.diskLimit !== "Shared") ? env.diskLimit : backend.defaultDisk
        win.editQuotaProc = (env.processLimit && env.processLimit.length > 0) ? env.processLimit : "Unlimited"
        const ioVal = (env.diskWriteLimit && env.diskWriteLimit.length > 0 && env.diskWriteLimit !== "Unlimited")
                      ? env.diskWriteLimit
                      : ((env.diskReadLimit && env.diskReadLimit.length > 0) ? env.diskReadLimit : "Unlimited")
        win.editQuotaIo = ioVal
        win.editQuotaError = ""
        win.editQuotasVisible = true
    }

    function openBackupsDialog() {
        win.backupError = ""
        backend.refreshBackups()
        win.backupsVisible = true
    }

    function openHistoryDialog() {
        win.historyTab = 0
        backend.acknowledgeOperationHistory()
        backend.refreshRecordedSessions()
        const hist = backend.operationHistory
        if (hist.length > 0) {
            win.selectedHistoryId = hist[0].id || ""
            win.selectedHistoryLog = backend.readOperationLog(win.selectedHistoryId)
        } else {
            win.selectedHistoryId = ""
            win.selectedHistoryLog = ""
        }
        const sess = backend.recordedSessions
        if (sess.length > 0) {
            win.selectRecordedSession(sess[0])
        } else {
            win.selectedSessionId = ""
            win.selectedSessionLogPath = ""
            win.selectedSessionTimingPath = ""
            win.selectedSessionHasTiming = false
            win.selectedSessionText = ""
        }
        win.historyVisible = true
    }

    function selectRecordedSession(item) {
        if (!item) {
            win.selectedSessionId = ""
            win.selectedSessionLogPath = ""
            win.selectedSessionTimingPath = ""
            win.selectedSessionHasTiming = false
            win.selectedSessionText = ""
            return
        }
        win.selectedSessionId = item.id || ""
        win.selectedSessionLogPath = item.logPath || ""
        win.selectedSessionTimingPath = item.timingPath || ""
        win.selectedSessionHasTiming = item.hasTiming || false
        win.selectedSessionText = backend.readRecordedSessionLog(win.selectedSessionLogPath)
    }

    function openEgressDialog(name) {
        if (!name || name.length === 0)
            return
        win.egressVisible = true
        backend.refreshEgressLog(name)
    }

    function openCloneDialog(sourceName) {
        if (!sourceName || sourceName.length === 0)
            return
        win.cloneSourceName = sourceName
        win.cloneNewName = sourceName + "-copy"
        backend.clearError()
        win.cloneVisible = true
    }

    function openEnvironmentActions() {
        if ((backend.selectedEnvironment.name || "").length === 0)
            return
        win.envActionsVisible = true
    }

    function syncSelectedNetworkDraft() {
        const env = backend.selectedEnvironment
        win.netAllowlistDraft = env.networkAllowlist || ""
        win.netAllowlistEditing = (env.networkMode === "allowlist")
    }

    function openSettings(section) {
        win.settingsSection = section
        win.configNotice = ""
        win.settingsVisible = true
    }

    function openPoolSettings() {
        win.poolResizeDraft = ""
        win.poolResizeConfirmVisible = false
        win.poolResizeError = ""
        win.openSettings(1)
    }

    function showConfigNotice(message, isError) {
        win.configNotice = message
        win.configNoticeError = isError
        configNoticeTimer.restart()
    }

    function saveGeneralSettings() {
        const error = backend.saveSettings(settingsPanel.workspacePathText, settingsPanel.osStoragePathText)
        if (error.length > 0) {
            win.showConfigNotice(error, true)
        } else {
            settingsPanel.setPaths(backend.workspaceBase, backend.osStoragePath)
            win.showConfigNotice(qsTr("Locations saved."), false)
        }
    }

    function submitFirstRun() {
        const error = backend.completeFirstRun(firstRunPathField.text)
        if (error.length > 0) {
            win.firstRunError = error
        } else {
            win.firstRunError = ""
            firstRunPathField.text = backend.systemPath
            settingsPanel.setPaths(backend.workspaceBase, backend.osStoragePath)
            win.activeTab = 0
        }
    }

    function beginAddOs() {
        win.osEditorOriginalId = ""
        win.osEditorId = ""
        win.osEditorAlias = ""
        win.osEditorName = ""
        win.osEditorBadge = ""
        win.osEditorDescription = ""
        win.osEditorError = ""
        settingsPanel.clearOsFields()
        win.osEditorVisible = true
    }

    function beginEditOs(entry) {
        win.osEditorOriginalId = entry.id || ""
        win.osEditorId = entry.id || ""
        win.osEditorAlias = entry.alias || ""
        win.osEditorName = entry.name || ""
        win.osEditorBadge = entry.badge || ""
        win.osEditorDescription = entry.description || ""
        win.osEditorError = ""
        settingsPanel.setOsFields(win.osEditorId, win.osEditorAlias, win.osEditorName,
                                  win.osEditorBadge, win.osEditorDescription)
        win.osEditorVisible = true
    }

    function submitOsEditor() {
        const error = backend.saveOsEntry(win.osEditorOriginalId, settingsPanel.osFieldId,
                                          settingsPanel.osFieldAlias, settingsPanel.osFieldName,
                                          settingsPanel.osFieldBadge, settingsPanel.osFieldDescription)
        if (error.length > 0) {
            win.osEditorError = error
            return
        }
        win.osEditorVisible = false
        win.showConfigNotice(qsTr("OS entry saved."), false)
    }

    function removeOsEntry(entry) {
        const error = backend.removeOsEntry(entry.id || "")
        if (error.length > 0)
            win.showConfigNotice(error, true)
        else
            win.showConfigNotice(qsTr("OS entry removed from the catalog."), false)
    }

    function syncCreateChoiceDrafts() {
        win.diskChoiceDraft = (backend.diskOptions || []).slice()
        win.ramChoiceDraft = (backend.ramOptions || []).slice()
        const cpus = backend.cpuOptions || []
        const cpu = []
        for (let i = 0; i < cpus.length; ++i)
            cpu.push(String(cpus[i]))
        win.cpuChoiceDraft = cpu
    }

    function sortedChoices(list) {
        return list.slice().sort(function (a, b) {
            return parseInt(a, 10) - parseInt(b, 10)
        })
    }

    // Chips persist instantly: every add/remove writes through the backend.
    function updateCreateChoices(kind, nextValues) {
        const next = win.sortedChoices(nextValues)
        let disk = win.diskChoiceDraft
        let ram = win.ramChoiceDraft
        let cpu = win.cpuChoiceDraft
        if (kind === "disk")
            disk = next
        else if (kind === "ram")
            ram = next
        else if (kind === "cpu")
            cpu = next

        const error = backend.saveCreateOptions(disk.join(", "), ram.join(", "), cpu.join(", "))
        if (error.length > 0) {
            win.showConfigNotice(error, true)
            return
        }
        win.diskChoiceDraft = disk
        win.ramChoiceDraft = ram
        win.cpuChoiceDraft = cpu
        win.showConfigNotice(qsTr("Create options saved."), false)
    }

    function saveQuotaOptions() {
        win.updateCreateChoices("disk", win.diskChoiceDraft)
    }

    // Compact "3.7 GiB" / "100GiB" into "3.7G" / "100G" so a sidebar row can
    // show disk, memory and network side by side without clipping.
    function compactSize(value) {
        const text = String(value === undefined || value === null ? "" : value).trim()
        if (text.length === 0)
            return "—"
        const compact = text.replace(/\s*(GiB|GB)\b/gi, "G")
                            .replace(/\s*(MiB|MB)\b/gi, "M")
                            .replace(/\s*(KiB|KB)\b/gi, "K")
        return compact.length > 0 ? compact : "—"
    }

    function sizePair(usage, limit) {
        return win.compactSize(usage) + "/" + win.compactSize(limit)
    }

    function runningEnvironmentCount() {
        let count = 0
        const envs = backend.environments || []
        for (let i = 0; i < envs.length; ++i) {
            if (envs[i] && envs[i].running)
                count++
        }
        return count
    }

    function preparePoolResize() {
        win.poolResizeError = backend.validateStoragePoolResize(win.poolResizeDraft)
        win.poolResizeConfirmVisible = win.poolResizeError.length === 0
    }

    function confirmPoolResize() {
        const error = backend.resizeStoragePool(win.poolResizeDraft)
        if (error.length > 0) {
            win.poolResizeError = error
            win.poolResizeConfirmVisible = false
            return
        }
        win.poolResizeConfirmVisible = false
        win.poolResizeError = ""
        win.showConfigNotice(qsTr("Growing the storage pool in the background..."), false)
    }

    Shortcut {
        sequence: "?"
        context: Qt.ApplicationShortcut
        onActivated: win.helpVisible = !win.helpVisible
    }
    Shortcut {
        sequence: "Escape"
        context: Qt.ApplicationShortcut
        onActivated: {
            if (backend.firstRun) {
                return
            } else if (win.helpVisible) {
                win.helpVisible = false
            } else if (win.toolsVisible) {
                win.toolsVisible = false
            } else if (win.envActionsVisible) {
                win.envActionsVisible = false
            } else if (win.portsVisible) {
                win.portsVisible = false
            } else if (win.mountsVisible) {
                win.mountsVisible = false
            } else if (win.editQuotasVisible) {
                win.editQuotasVisible = false
            } else if (win.backupsVisible) {
                win.backupsVisible = false
            } else if (win.egressVisible) {
                win.egressVisible = false
            } else if (win.historyVisible) {
                win.historyVisible = false
            } else if (win.settingsVisible) {
                win.settingsVisible = false
            } else if (win.createVisible) {
                win.createVisible = false
            } else if (win.cloneVisible) {
                win.cloneVisible = false
            } else if (win.wipeVisible) {
                win.wipeVisible = false
            } else if (win.restoreDialogVisible) {
                win.restoreDialogVisible = false
            } else if (win.changeReviewVisible) {
                win.changeReviewVisible = false
            } else if (win.secretsVisible) {
                win.secretsVisible = false
            } else if (win.staleReportVisible) {
                if (win.staleConfirmAction.length > 0)
                    win.staleConfirmAction = ""
                else
                    win.staleReportVisible = false
            } else {
                backend.clearError()
            }
        }
    }
    Shortcut {
        sequence: "A"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && !backend.firstRun
        onActivated: win.openSecretsDialog()
    }
    Shortcut {
        sequence: "U"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && !backend.firstRun
        onActivated: win.openStaleReportDialog()
    }
    Shortcut {
        sequence: "H"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen
        onActivated: win.openHistoryDialog()
    }
    Shortcut {
        sequence: "B"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && !backend.firstRun
        onActivated: win.openBackupsDialog()
    }
    Shortcut {
        sequence: "V"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && backend.environments.length > 0
        onActivated: win.openChangeReviewDialog("")
    }
    Shortcut {
        sequence: "E"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && !backend.busy && backend.environments.length > 0
        onActivated: win.openEditQuotasDialog()
    }
    Shortcut {
        sequence: "3"
        context: Qt.ApplicationShortcut
        enabled: !backend.firstRun && !backend.busy && !win.toolsVisible && !win.envActionsVisible && !win.createVisible && !win.cloneVisible && !win.wipeVisible && !win.restoreDialogVisible && !win.changeReviewVisible && !win.secretsVisible && !win.staleReportVisible && !win.helpVisible && !win.historyVisible && !win.egressVisible && !win.portsVisible && !win.mountsVisible && !win.editQuotasVisible && !win.backupsVisible
        onActivated: {
            if (win.settingsVisible)
                win.settingsVisible = false
            else
                win.openSettings(0)
        }
    }
    Shortcut {
        sequence: "Q"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && !backend.busy
        onActivated: Qt.quit()
    }
    Shortcut {
        sequence: "N"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && !backend.busy
        onActivated: win.openCreateDialog("")
    }
    Shortcut {
        sequence: "1"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen
        onActivated: win.activeTab = 0
    }
    Shortcut {
        sequence: "2"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen
        onActivated: win.activeTab = 1
    }
    Shortcut {
        sequence: "R"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && !backend.busy
        onActivated: backend.refresh()
    }
    Shortcut {
        sequence: "Space"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && !backend.busy
        onActivated: backend.primaryAction()
    }
    Shortcut {
        sequence: "Return"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && !backend.busy && backend.environments.length > 0
        onActivated: backend.enterEnvironment(backend.selectedEnvironment.name || "")
    }
    Shortcut {
        sequence: "F"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && backend.environments.length > 0
        onActivated: backend.openWorkspaceFolder(backend.selectedEnvironment.name || "")
    }
    Shortcut {
        sequence: "S"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && !backend.busy && backend.environments.length > 0
        onActivated: backend.toggleStartStop(backend.selectedEnvironment.name || "")
    }
    Shortcut {
        sequence: "P"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && !backend.busy && backend.environments.length > 0
        onActivated: backend.createSnapshot(backend.selectedEnvironment.name || "", mainContent.snapshotNameText)
    }
    Shortcut {
        sequence: "C"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && backend.environments.length > 0 && !(backend.selectedEnvironment.isTemplate || false)
        onActivated: backend.copyAiCommand(backend.selectedEnvironment.name || "")
    }
    Shortcut {
        sequence: "I"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && backend.environments.length > 0 && !(backend.selectedEnvironment.isTemplate || false)
        onActivated: backend.pasteImageToWorkspace(backend.selectedEnvironment.name || "")
    }
    Shortcut {
        sequence: "D"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && !backend.busy && backend.environments.length > 0
        onActivated: win.openCloneDialog(backend.selectedEnvironment.name || "")
    }
    Shortcut {
        sequence: "T"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && !backend.busy && backend.environments.length > 0
        onActivated: backend.setEnvironmentTemplate(backend.selectedEnvironment.name || "", !(backend.selectedEnvironment.isTemplate || false))
    }
    Shortcut {
        sequence: "K"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && !backend.busy && backend.environments.length > 0 && !(backend.selectedEnvironment.isTemplate || false)
        onActivated: {
            const name = backend.selectedEnvironment.name || ""
            const nextMode = (backend.selectedEnvironment.networkMode === "offline") ? "online" : "offline"
            backend.setNetworkMode(name, nextMode, "")
        }
    }
    Shortcut {
        sequence: "Delete"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && !backend.busy && backend.environments.length > 0
        onActivated: {
            win.deleteWorkspaceOnWipe = false
            win.wipeVisible = true
        }
    }
    Shortcut {
        sequence: "Up"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && backend.environments.length > 0
        onActivated: backend.selectedIndex = Math.max(0, backend.selectedIndex - 1)
    }
    Shortcut {
        sequence: "Down"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && backend.environments.length > 0
        onActivated: backend.selectedIndex = Math.min(backend.environments.length - 1, backend.selectedIndex + 1)
    }

    Connections {
        target: backend
        function onEnvironmentCreated(name) {
            win.createVisible = false
            win.cloneVisible = false
            win.activeTab = 0
        }
        function onSelectedIndexChanged() {
            win.syncSelectedNetworkDraft()
        }
        function onEnvironmentsChanged() {
            if (!win.netAllowlistEditing || win.netAllowlistDraft.length === 0)
                win.syncSelectedNetworkDraft()
        }
    }

    Component.onCompleted: {
        win.syncSelectedNetworkDraft()
        if (backend.busy && backend.pendingEnvName.length > 0) {
            win.newEnvName = backend.pendingEnvName
            win.createVisible = true
        }
    }

    Item {
        anchors.fill: parent
        LayoutMirroring.enabled: Qt.application.layoutDirection === Qt.RightToLeft
        LayoutMirroring.childrenInherit: true

        ColumnLayout {
            anchors.fill: parent
            spacing: 0

            // ==================================================================
            // TOP HEADER BAR
            // ==================================================================
            AppHeader {
                app: win
            }

            // ==================================================================
            // LIVE TASK PROGRESS BAR / ERROR BANNER (WHEN BUSY OUTSIDE MODAL)
            // ==================================================================
            StatusBanner {
                app: win
            }

            // ==================================================================
            // MAIN CONTENT AREA
            // ==================================================================
            MainContent {
                id: mainContent
                app: win
            }
        }

        // ======================================================================
        // MODAL 1: CREATE NEW ENVIRONMENT DIALOG (WITH LIVE PROGRESS & CANCEL)
        // ======================================================================
        CreateDialog {
            id: createDialog
            app: win
        }

        // ======================================================================
        // MODAL 1B: CLONE ENVIRONMENT DIALOG
        // ======================================================================
        CloneDialog {
            app: win
        }

        // ======================================================================
        // MODAL 1C: SECONDARY ENVIRONMENT ACTIONS
        // ======================================================================
        EnvironmentActionsDialog {
            app: win
        }

        // ======================================================================
        // MODAL 1D: TOOLS MENU (SECRETS, BACKUPS, HISTORY, ORPHANS)
        // ======================================================================
        ToolsDialog {
            app: win
        }

        // ======================================================================
        // MODAL 2: WIPE / DELETE CONFIRMATION DIALOG
        // ======================================================================
        WipeDialog {
            app: win
        }

        // ======================================================================
        // MODAL 2B: RESTORE SNAPSHOT CONFIRMATION (SYSTEM / WORKSPACE / BOTH)
        // ======================================================================
        RestoreDialog {
            app: win
        }

        // ======================================================================
        // MODAL 2C: WORKSPACE CHANGE REVIEW (V)
        // ======================================================================
        ChangeReviewDialog {
            app: win
        }

        // ======================================================================
        // MODAL 3: KEYBOARD SHORTCUTS OVERLAY (?)
        // ======================================================================
        HelpOverlay {
            app: win
        }

        // ======================================================================
        // MODAL 4: CONFIGURATION MENU (STORAGE, OS CATALOG, CREATE OPTIONS)
        // ======================================================================
        SettingsPanel {
            id: settingsPanel
            app: win
        }

        // ======================================================================
        // MODAL 5: FIRST-RUN SYSTEM STORAGE SETUP
        // ======================================================================
        ModalDialog {
            id: firstRunOverlay
            open: backend.firstRun
            scrimColor: Style.scrimStrong
            zOrder: 100
            cardWidth: Math.min(parent.width - Style.modalGutter, 600)
            contentSpacing: Style.space14

            readonly property string resolvedPreview: {
                const raw = firstRunPathField.text.trim()
                const base = raw.length > 0 ? raw : backend.defaultSystemPath
                return base.length > 1 && base.endsWith("/") ? base.slice(0, -1) : base
            }

            onVisibleChanged: {
                if (visible) {
                    win.firstRunError = ""
                    firstRunPathField.text = backend.systemPath.length > 0 ? backend.systemPath : backend.defaultSystemPath
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space12

                BrandMark {
                    markSize: 40
                    markRadius: Style.radius6
                    textSize: Style.font14
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    spacing: Style.space2

                    Text {
                        Layout.fillWidth: true
                        text: qsTr("Welcome to AIBox")
                        color: theme.foreground
                        font.family: Style.fontSans
                        font.pixelSize: Style.font18
                        font.weight: Font.Medium
                    }
                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        text: qsTr("Choose where AIBox stores its system data, shared workspaces, OS images, and backups. You can keep the default or pick a custom location.")
                        color: theme.muted
                        font.family: Style.fontSans
                        font.pixelSize: Style.font11
                    }
                }
            }

            Rectangle {
                Layout.fillWidth: true
                height: Style.hairline
                color: Style.borderSubtle
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: Style.space8

                Text {
                    Layout.fillWidth: true
                    text: qsTr("System Storage Location")
                    color: theme.foreground
                    font.family: Style.fontSans
                    font.pixelSize: Style.font12
                    font.weight: Font.Medium
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Style.space10

                    FlatInput {
                        id: firstRunPathField
                        Layout.fillWidth: true
                        text: backend.systemPath.length > 0 ? backend.systemPath : backend.defaultSystemPath
                        placeholder: backend.defaultSystemPath
                        onAccepted: win.submitFirstRun()
                    }

                    ActionButton {
                        label: qsTr("Browse...")
                        minWidth: 96
                        onClicked: firstRunFolderDialog.open()
                    }

                    ActionButton {
                        label: qsTr("Default")
                        minWidth: 84
                        onClicked: firstRunPathField.text = backend.defaultSystemPath
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: previewGrid.implicitHeight + Style.space20
                    radius: Math.min(Style.radius4, theme.radius)
                    color: Style.surfaceInput
                    border.width: Style.hairline
                    border.color: Style.borderSubtle

                    ColumnLayout {
                        id: previewGrid
                        anchors.fill: parent
                        anchors.margins: Style.space10
                        spacing: Style.space4

                        Text {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            elide: Text.ElideMiddle
                            text: qsTr("Workspaces:  %1/workspaces").arg(firstRunOverlay.resolvedPreview)
                            color: theme.muted
                            font.family: Style.fontMono
                            font.pixelSize: Style.font10
                        }
                        Text {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            elide: Text.ElideMiddle
                            text: qsTr("OS Images:   %1/os-images").arg(firstRunOverlay.resolvedPreview)
                            color: theme.muted
                            font.family: Style.fontMono
                            font.pixelSize: Style.font10
                        }
                        Text {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            elide: Text.ElideMiddle
                            text: qsTr("State & Cfg: %1/state, config.json").arg(firstRunOverlay.resolvedPreview)
                            color: theme.muted
                            font.family: Style.fontMono
                            font.pixelSize: Style.font10
                        }
                    }
                }
            }

            Rectangle {
                visible: win.firstRunError.length > 0
                Layout.fillWidth: true
                implicitHeight: firstRunErrTxt.implicitHeight + Style.space14
                radius: Math.min(Style.radius4, theme.radius)
                color: Style.dangerSurface
                border.width: Style.hairline
                border.color: theme.red
                Text {
                    id: firstRunErrTxt
                    anchors.fill: parent
                    anchors.margins: Style.space7
                    wrapMode: Text.WordWrap
                    text: win.firstRunError
                    color: theme.red
                    font.family: Style.fontSans
                    font.pixelSize: Style.font11
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: Style.space4
                spacing: Style.space10
                Item { Layout.fillWidth: true }

                ActionButton {
                    label: qsTr("Continue")
                    variant: "primary"
                    minWidth: 128
                    onClicked: win.submitFirstRun()
                }
            }
        }

        FolderDialog {
            id: firstRunFolderDialog
            title: qsTr("Choose AIBox System Storage Location")
            currentFolder: backend.pathToUrl(firstRunPathField.text.trim().length > 0 ? firstRunPathField.text.trim() : backend.defaultSystemPath)
            onAccepted: firstRunPathField.text = backend.localPath(selectedFolder)
        }

        // ======================================================================
        // MODAL 6: OPERATION HISTORY & FULL LOG VIEWER (G4)
        // ======================================================================
        HistoryDialog {
            app: win
        }

        // ======================================================================
        // MODAL 7: NETWORK EGRESS LOG VIEWER (G3)
        // ======================================================================
        EgressDialog {
            app: win
        }

        // ======================================================================
        // MODAL 8: PORT FORWARDING (W1)
        // ======================================================================
        PortsDialog {
            app: win
        }

        // ======================================================================
        // MODAL 9: EXTRA MOUNTS (W2)
        // ======================================================================
        MountsDialog {
            app: win
        }

        // ======================================================================
        // MODAL 10: EDIT QUOTAS & LIMITS (W3)
        // ======================================================================
        EditQuotasDialog {
            app: win
        }

        // ─── Modal 11: Backups (Export / Import) ────────────────────────────────
        BackupsDialog {
            app: win
        }

        // MODAL 10: SECRETS MANAGER (A)
        SecretsDialog {
            app: win
        }

        // MODAL 11: STALE ENVIRONMENT & ORPHAN REPORT (U)
        StaleReportDialog {
            app: win
        }

        Timer {
            id: configNoticeTimer
            interval: 5000
            onTriggered: win.configNotice = ""
        }

    }
}
