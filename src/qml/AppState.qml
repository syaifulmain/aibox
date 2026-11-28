import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import "components"

// ApplicationWindow base holding shared UI state and window-level actions
// so Main.qml stays a concise shell and child panels/dialogs bind to `app`.
ApplicationWindow {
    id: win
    width: Style.windowWidth
    height: Style.windowHeight
    minimumWidth: Style.windowMinWidth
    minimumHeight: Style.windowMinHeight
    visible: true
    title: qsTr("AIBox")
    color: theme.surfaceCanvas

    onActiveChanged: backend.setWindowActive(win.active && win.visibility !== Window.Minimized && win.visibility !== Window.Hidden)
    onVisibilityChanged: backend.setWindowActive(win.active && win.visibility !== Window.Minimized && win.visibility !== Window.Hidden)

    Behavior on color {
        enabled: !theme.reduceMotion
        ColorAnimation { duration: theme.reduceMotion ? 0 : 150 }
    }

    Material.theme: theme.isDark ? Material.Dark : Material.Light
    Material.accent: theme.accent
    Material.background: theme.surfaceCanvas
    Material.foreground: theme.textPrimary
    Material.primary: theme.accent
    Material.roundedScale: theme.radius

    property var dialogHostRef: null

    property int activeTab: 0
    property bool helpVisible: false
    property bool themeDebugVisible: false
    property bool toolsVisible: false
    property bool envActionsVisible: false
    property bool createVisible: false
    property bool cloneVisible: false
    property bool wipeVisible: false
    property bool stopConfirmVisible: false
    property bool deleteWorkspaceOnWipe: false
    property bool settingsVisible: false
    onSettingsVisibleChanged: {
        if (!settingsVisible)
            osEditorVisible = false
    }
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
    property string networkDraftEnvName: ""
    property string netAllowlistDraft: ""
    property bool netAllowlistEditing: false
    property var diskChoiceDraft: []
    property var ramChoiceDraft: []
    property var swapChoiceDraft: []
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
    property string newEnvSwap: "Off"
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
    property string editQuotaSwap: "Host"
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
    property string secretFormError: ""

    readonly property bool anyModalOpen: helpVisible || toolsVisible || envActionsVisible || createVisible || cloneVisible || wipeVisible || stopConfirmVisible || restoreDialogVisible || changeReviewVisible || secretsVisible || staleReportVisible || settingsVisible || historyVisible || egressVisible || portsVisible || mountsVisible || editQuotasVisible || backupsVisible || backend.firstRun

    Timer {
        id: configNoticeTimer
        interval: 5000
        onTriggered: win.configNotice = ""
    }

    function requestStartStop() {
        if (backend.busy)
            return
        const env = backend.selectedEnvironment
        const name = env.name || ""
        if (name.length === 0)
            return
        if (env.running || false) {
            win.stopConfirmVisible = true
            return
        }
        backend.toggleStartStop(name)
    }

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
        win.secretFormError = ""
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
        win.secretFormError = ""
        backend.reloadSecrets()
        backend.reloadEnvironmentSecrets(name)
        win.secretsVisible = true
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
        win.secretFormError = err.length > 0 ? err : ""
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
        win.secretFormError = err.length > 0 ? err : ""
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
        if (win.dialogHostRef && win.dialogHostRef.createDialog)
            win.dialogHostRef.createDialog.resetSharedPath()
        win.newEnvOsId = (osId && osId.length > 0) ? osId : backend.defaultOsId
        win.newEnvPreset = backend.defaultPresetId
        win.newEnvDisk = backend.defaultDisk
        win.newEnvRam = backend.defaultRam
        win.newEnvCpu = backend.defaultCpu
        win.newEnvProcLimit = backend.defaultProcessLimit
        win.newEnvIoLimit = backend.defaultIoLimit
        win.newEnvSwap = "Off"
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
        win.editQuotaSwap = (env.swapLimit && env.swapLimit.length > 0) ? env.swapLimit : "Host"
        win.editQuotaError = ""
        win.editQuotasVisible = true
    }

    function openBackupsDialog() {
        win.backupError = ""
        win.importBackupFile = ""
        win.importWsFile = ""
        win.importNewName = ""
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
        win.networkDraftEnvName = env.name || ""
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
        const panel = win.dialogHostRef ? win.dialogHostRef.settingsPanel : null
        if (!panel)
            return
        const error = backend.saveSettings(panel.workspacePathText, panel.osStoragePathText)
        if (error.length > 0) {
            win.showConfigNotice(error, true)
        } else {
            panel.setPaths(backend.workspaceBase, backend.osStoragePath)
            win.showConfigNotice(qsTr("Locations saved."), false)
        }
    }

    function onFirstRunCompleted() {
        const panel = win.dialogHostRef ? win.dialogHostRef.settingsPanel : null
        if (panel)
            panel.setPaths(backend.workspaceBase, backend.osStoragePath)
        win.activeTab = 0
    }

    function beginAddOs() {
        win.osEditorOriginalId = ""
        win.osEditorId = ""
        win.osEditorAlias = ""
        win.osEditorName = ""
        win.osEditorBadge = ""
        win.osEditorDescription = ""
        win.osEditorError = ""
        const panel = win.dialogHostRef ? win.dialogHostRef.settingsPanel : null
        if (panel)
            panel.clearOsFields()
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
        const panel = win.dialogHostRef ? win.dialogHostRef.settingsPanel : null
        if (panel) {
            panel.setOsFields(win.osEditorId, win.osEditorAlias, win.osEditorName,
                              win.osEditorBadge, win.osEditorDescription)
        }
        win.osEditorVisible = true
    }

    function submitOsEditor() {
        const panel = win.dialogHostRef ? win.dialogHostRef.settingsPanel : null
        if (!panel)
            return
        const error = backend.saveOsEntry(win.osEditorOriginalId, panel.osFieldId,
                                          panel.osFieldAlias, panel.osFieldName,
                                          panel.osFieldBadge, panel.osFieldDescription)
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
        win.swapChoiceDraft = (backend.swapOptions || []).slice()
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
        const next = kind === "swap" ? nextValues.slice() : win.sortedChoices(nextValues)
        let disk = win.diskChoiceDraft
        let ram = win.ramChoiceDraft
        let swap = win.swapChoiceDraft
        let cpu = win.cpuChoiceDraft
        if (kind === "disk")
            disk = next
        else if (kind === "ram")
            ram = next
        else if (kind === "swap")
            swap = next
        else if (kind === "cpu")
            cpu = next

        const error = backend.saveCreateOptions(disk.join(", "), ram.join(", "),
                                                swap.join(", "), cpu.join(", "))
        if (error.length > 0) {
            win.showConfigNotice(error, true)
            return
        }
        win.diskChoiceDraft = disk
        win.ramChoiceDraft = ram
        win.swapChoiceDraft = swap
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
}
