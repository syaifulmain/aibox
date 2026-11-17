import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts

ApplicationWindow {
    id: win
    width: 1080
    height: 720
    minimumWidth: 860
    minimumHeight: 560
    visible: true
    title: qsTr("AIBox")
    color: theme.background

    Material.theme: Material.Dark
    Material.accent: theme.accent
    Material.roundedScale: theme.radius

    readonly property string sansFont: "Adwaita Sans"
    readonly property string monoFont: "JetBrainsMono Nerd Font"
    readonly property color borderSubtle: "#232738"
    readonly property color surfaceCard: "#12131c"
    readonly property color surfaceInput: "#0b0c12"
    readonly property color surfaceSelected: "#192136"
    readonly property color accentColor: theme.accent
    readonly property color redColor: theme.red
    readonly property color greenColor: theme.green

    property int activeTab: 0
    property bool helpVisible: false
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
    property string secretNameDraft: ""
    property string secretValueDraft: ""
    property bool secretDraftShow: false
    property string secretFormError: ""
    property var secretRevealMap: ({})

    readonly property bool anyModalOpen: helpVisible || createVisible || cloneVisible || wipeVisible || restoreDialogVisible || changeReviewVisible || secretsVisible || staleReportVisible || settingsVisible || historyVisible || egressVisible || portsVisible || mountsVisible || editQuotasVisible || backupsVisible || backend.firstRun

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
        win.secretNameDraft = ""
        win.secretValueDraft = ""
        win.secretDraftShow = false
        win.secretFormError = ""
        win.secretRevealMap = ({})
        backend.reloadSecrets()
        win.secretsVisible = true
    }

    function toggleSecretReveal(name) {
        const next = Object.assign({}, win.secretRevealMap || {})
        next[name] = !(next[name] === true)
        win.secretRevealMap = next
    }

    function toggleEnvSecret(secretName) {
        const env = backend.selectedEnvironment
        if (!env || !env.name || backend.busy)
            return
        const cur = (env.secretNames || []).slice()
        const idx = cur.indexOf(secretName)
        if (idx >= 0)
            cur.splice(idx, 1)
        else
            cur.push(secretName)
        backend.setEnvironmentSecrets(env.name, cur)
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
        createSharedPathField.text = ""
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
        const error = backend.saveSettings(workspacePathField.text, osStoragePathField.text)
        if (error.length > 0) {
            win.showConfigNotice(error, true)
        } else {
            workspacePathField.text = backend.workspaceBase
            osStoragePathField.text = backend.osStoragePath
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
            workspacePathField.text = backend.workspaceBase
            osStoragePathField.text = backend.osStoragePath
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
        osIdField.text = ""
        osAliasField.text = ""
        osNameField.text = ""
        osBadgeField.text = ""
        osDescriptionField.text = ""
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
        osIdField.text = win.osEditorId
        osAliasField.text = win.osEditorAlias
        osNameField.text = win.osEditorName
        osBadgeField.text = win.osEditorBadge
        osDescriptionField.text = win.osEditorDescription
        win.osEditorVisible = true
    }

    function submitOsEditor() {
        const error = backend.saveOsEntry(win.osEditorOriginalId, osIdField.text, osAliasField.text,
                                          osNameField.text, osBadgeField.text, osDescriptionField.text)
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

    // Reusable clean flat input box (replaces Material TextField completely)
    component FlatInput: Rectangle {
        id: root
        property alias text: textInput.text
        property alias echoMode: textInput.echoMode
        property string placeholder: ""
        property bool readOnly: false
        signal accepted()

        implicitHeight: 34
        Layout.preferredHeight: implicitHeight
        radius: Math.min(4, theme.radius)
        color: win.surfaceInput
        border.width: 1
        border.color: textInput.activeFocus ? theme.accent : win.borderSubtle

        Text {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            visible: textInput.text.length === 0
            text: root.placeholder
            color: theme.muted
            font.family: win.sansFont
            font.pixelSize: 12
            elide: Text.ElideRight
        }

        TextInput {
            id: textInput
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            verticalAlignment: TextInput.AlignVCenter
            color: theme.foreground
            selectionColor: theme.accent
            selectedTextColor: theme.accentForeground
            selectByMouse: true
            clip: true
            readOnly: root.readOnly
            font.family: win.sansFont
            font.pixelSize: 13
            onAccepted: root.accepted()
        }
    }

    // Chip-style numeric editor: type a number and press Enter to store it.
    // The unit ("" or "GiB") is appended automatically, and the list is capped.
    component ChipInput: Rectangle {
        id: chipRoot
        property var values: []
        property string unit: ""
        property int maxChips: 4
        property int maxValue: 1024
        property string placeholder: ""
        property string fullHint: ""
        readonly property int count: values ? values.length : 0
        readonly property bool atLimit: count >= maxChips
        signal addRequested(string value)
        signal removeRequested(int index)

        implicitHeight: chipFlow.implicitHeight + 8
        Layout.preferredHeight: implicitHeight
        radius: Math.min(6, theme.radius)
        color: win.surfaceInput
        border.width: 1
        border.color: chipInput.activeFocus ? theme.accent : win.borderSubtle

        HoverHandler { id: chipHover }

        function submit() {
            const raw = chipInput.text.trim()
            if (raw.length === 0)
                return
            const digits = raw.replace(/\s+/g, "")
            if (!/^\d+$/.test(digits) || parseInt(digits, 10) < 1) {
                win.showConfigNotice(qsTr("Enter a whole number greater than zero."), true)
                return
            }
            if (parseInt(digits, 10) > chipRoot.maxValue) {
                win.showConfigNotice(qsTr("The maximum value is %1.").arg(chipRoot.maxValue), true)
                return
            }
            if (chipRoot.atLimit) {
                win.showConfigNotice(qsTr("Limit is %1 choices. Remove one first.").arg(chipRoot.maxChips), true)
                return
            }
            const value = digits + chipRoot.unit
            for (let i = 0; i < chipRoot.count; ++i) {
                if (String(chipRoot.values[i]).toLowerCase() === value.toLowerCase()) {
                    win.showConfigNotice(qsTr("%1 is already in the list.").arg(value), true)
                    chipInput.text = ""
                    return
                }
            }
            chipInput.text = ""
            chipRoot.addRequested(value)
        }

        Flow {
            id: chipFlow
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: 4
            spacing: 6

            Repeater {
                model: chipRoot.values
                delegate: Rectangle {
                    required property string modelData
                    required property int index
                    readonly property string suffix: chipRoot.unit
                    readonly property bool removable: chipRoot.count > 1
                    readonly property string amount: (chipRoot.unit.length > 0 && modelData.endsWith(chipRoot.unit))
                                                      ? modelData.slice(0, modelData.length - chipRoot.unit.length)
                                                      : modelData

                    height: 24
                    width: chipRow.implicitWidth + 16
                    radius: Math.min(4, theme.radius)
                    color: chipMouse.containsMouse && removable ? win.surfaceSelected : win.surfaceCard
                    border.width: 1
                    border.color: chipMouse.containsMouse && removable ? theme.accent : win.borderSubtle
                    opacity: removable ? 1.0 : 0.65

                    Row {
                        id: chipRow
                        anchors.centerIn: parent
                        spacing: 5

                        Text {
                            text: "×"
                            color: chipMouse.containsMouse && removable ? theme.accent : theme.muted
                            font.family: win.monoFont
                            font.pixelSize: 12
                        }
                        Text {
                            text: amount
                            color: theme.foreground
                            font.family: win.monoFont
                            font.pixelSize: 12
                        }
                        Text {
                            visible: suffix.length > 0
                            text: suffix
                            color: theme.muted
                            font.family: win.monoFont
                            font.pixelSize: 9
                        }
                    }

                    MouseArea {
                        id: chipMouse
                        anchors.fill: parent
                        cursorShape: removable ? Qt.PointingHandCursor : Qt.ArrowCursor
                        onClicked: {
                            if (!removable) {
                                win.showConfigNotice(qsTr("Keep at least one choice."), true)
                                return
                            }
                            chipRoot.removeRequested(index)
                        }
                    }
                }
            }

            Item {
                visible: !chipRoot.atLimit
                height: 24
                width: Math.min(Math.max(176, chipFlow.width - 220), Math.max(176, chipInput.implicitWidth + 4))

                TextInput {
                    id: chipInput
                    anchors.fill: parent
                    verticalAlignment: TextInput.AlignVCenter
                    color: theme.foreground
                    selectionColor: theme.accent
                    selectedTextColor: theme.accentForeground
                    selectByMouse: true
                    clip: true
                    inputMethodHints: Qt.ImhDigitsOnly
                    font.family: win.monoFont
                    font.pixelSize: 12
                    validator: IntValidator { bottom: 1; top: chipRoot.maxValue }
                    onAccepted: chipRoot.submit()
                    Keys.onBackPressed: {
                        if (text.length === 0 && chipRoot.count > 1)
                            chipRoot.removeRequested(chipRoot.count - 1)
                    }
                }

                Text {
                    anchors.fill: parent
                    verticalAlignment: Text.AlignVCenter
                    visible: chipInput.text.length === 0
                    text: chipRoot.placeholder
                    color: theme.muted
                    font.family: win.sansFont
                    font.pixelSize: 11
                    elide: Text.ElideRight
                }
            }

            Text {
                visible: chipRoot.atLimit
                height: 24
                verticalAlignment: Text.AlignVCenter
                text: chipHover.hovered ? chipRoot.fullHint : qsTr("%1 / %2").arg(chipRoot.count).arg(chipRoot.maxChips)
                color: theme.muted
                font.family: win.sansFont
                font.pixelSize: 10
            }
        }
    }

    // Monochrome sliders icon used by the Configuration button and panel.
    component SlidersIcon: Item {
        id: slidersIcon
        property color color: theme.foreground
        implicitWidth: 14
        implicitHeight: 14

        Rectangle { x: 0; y: 2; width: 14; height: 1; color: slidersIcon.color }
        Rectangle { x: 2; y: 0; width: 3; height: 3; radius: 1.5; color: slidersIcon.color }
        Rectangle { x: 0; y: 7; width: 14; height: 1; color: slidersIcon.color }
        Rectangle { x: 9; y: 5; width: 3; height: 3; radius: 1.5; color: slidersIcon.color }
        Rectangle { x: 0; y: 12; width: 14; height: 1; color: slidersIcon.color }
        Rectangle { x: 5; y: 10; width: 3; height: 3; radius: 1.5; color: slidersIcon.color }
    }

    // Reusable sleek action button
    component ActionButton: Rectangle {
        id: btn
        property string label: ""
        property string shortcutHint: ""
        property string variant: "" // "primary" | "danger" | "success" | "accent" | ""
        property bool primary: false
        property bool danger: false
        property bool success: false
        property bool accent: false
        property bool disabled: false
        property bool slidersIcon: false
        property int minWidth: 0
        readonly property bool hovered: btnMouse.containsMouse
        signal clicked()

        readonly property bool isPrimary: primary || variant === "primary"
        readonly property bool isDanger: danger || variant === "danger"
        readonly property bool isSuccess: success || variant === "success"
        readonly property bool isAccent: accent || variant === "accent" || variant === "theme"
        readonly property color contentColor: btn.isPrimary
                                              ? theme.accentForeground
                                              : (btn.isDanger
                                                 ? theme.red
                                                 : (btn.isAccent
                                                    ? theme.accent
                                                    : (btn.isSuccess ? theme.green : theme.foreground)))

        implicitHeight: 34
        implicitWidth: minWidth > 0 ? Math.max(minWidth, btnRow.implicitWidth + 24)
                                    : (label === "?" ? 34 : btnRow.implicitWidth + 24)
        Layout.preferredHeight: implicitHeight
        Layout.preferredWidth: implicitWidth
        radius: Math.min(4, theme.radius)
        opacity: btn.disabled ? 0.4 : (btnMouse.containsMouse ? 0.9 : 1.0)
        color: btn.isPrimary
               ? theme.accent
               : (btn.isDanger
                  ? (btnMouse.containsMouse
                     ? Qt.rgba(win.redColor.r, win.redColor.g, win.redColor.b, 0.22)
                     : Qt.rgba(win.redColor.r, win.redColor.g, win.redColor.b, 0.12))
                  : (btn.isAccent
                     ? (btnMouse.containsMouse
                        ? Qt.rgba(win.accentColor.r, win.accentColor.g, win.accentColor.b, 0.22)
                        : Qt.rgba(win.accentColor.r, win.accentColor.g, win.accentColor.b, 0.12))
                     : (btn.isSuccess
                        ? (btnMouse.containsMouse
                           ? Qt.rgba(win.greenColor.r, win.greenColor.g, win.greenColor.b, 0.22)
                           : Qt.rgba(win.greenColor.r, win.greenColor.g, win.greenColor.b, 0.12))
                        : (btnMouse.containsMouse ? theme.lighterBackground : win.surfaceCard))))

        border.width: btn.isPrimary ? 0 : 1
        border.color: btn.isDanger
                      ? theme.red
                      : (btn.isAccent
                         ? theme.accent
                         : (btn.isSuccess
                            ? theme.green
                            : (btnMouse.containsMouse ? theme.muted : win.borderSubtle)))

        RowLayout {
            id: btnRow
            anchors.centerIn: parent
            spacing: 8

            SlidersIcon {
                visible: btn.slidersIcon
                color: btn.contentColor
                Layout.alignment: Qt.AlignVCenter
            }

            Text {
                visible: btn.label.length > 0
                text: btn.label
                color: btn.contentColor
                font.family: win.sansFont
                font.pixelSize: 12
                font.weight: btn.isPrimary ? Font.Medium : Font.Normal
            }

            Rectangle {
                visible: btn.shortcutHint.length > 0
                height: 18
                width: hintText.implicitWidth + 10
                radius: 2
                color: btn.isPrimary
                       ? Qt.rgba(0, 0, 0, 0.18)
                       : (btn.isDanger
                          ? Qt.rgba(win.redColor.r, win.redColor.g, win.redColor.b, 0.18)
                          : (btn.isAccent
                             ? Qt.rgba(win.accentColor.r, win.accentColor.g, win.accentColor.b, 0.18)
                             : (btn.isSuccess
                                ? Qt.rgba(win.greenColor.r, win.greenColor.g, win.greenColor.b, 0.18)
                                : win.surfaceInput)))
                border.width: btn.isPrimary ? 0 : 1
                border.color: btn.isDanger
                              ? Qt.rgba(win.redColor.r, win.redColor.g, win.redColor.b, 0.35)
                              : (btn.isAccent
                                 ? Qt.rgba(win.accentColor.r, win.accentColor.g, win.accentColor.b, 0.35)
                                 : (btn.isSuccess
                                    ? Qt.rgba(win.greenColor.r, win.greenColor.g, win.greenColor.b, 0.35)
                                    : win.borderSubtle))

                Text {
                    id: hintText
                    anchors.centerIn: parent
                    text: btn.shortcutHint
                    color: btn.isPrimary
                           ? theme.accentForeground
                           : (btn.isDanger
                              ? theme.red
                              : (btn.isAccent
                                 ? theme.accent
                                 : (btn.isSuccess ? theme.green : theme.muted)))
                    font.family: win.monoFont
                    font.pixelSize: 10
                }
            }
        }

        MouseArea {
            id: btnMouse
            anchors.fill: parent
            hoverEnabled: true
            enabled: !btn.disabled
            cursorShape: btn.disabled ? Qt.ForbiddenCursor : Qt.PointingHandCursor
            onClicked: btn.clicked()
        }
    }

    // Uniform modal header with top-right Close [Esc] button and 1px divider
    component ModalHeader: ColumnLayout {
        id: mh
        property string title: ""
        property string subtitle: ""
        property bool subtitleMono: false
        property bool showClose: true
        property bool closeDisabled: false
        default property alias extraActions: extraRow.data
        signal closed()

        Layout.fillWidth: true
        spacing: 12

        RowLayout {
            Layout.fillWidth: true
            spacing: 10

            ColumnLayout {
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                spacing: 2

                Text {
                    Layout.fillWidth: true
                    text: mh.title
                    color: theme.foreground
                    font.family: win.sansFont
                    font.pixelSize: 16
                    font.weight: Font.Medium
                    elide: Text.ElideRight
                }

                Text {
                    visible: mh.subtitle.length > 0
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    text: mh.subtitle
                    color: theme.muted
                    font.family: mh.subtitleMono ? win.monoFont : win.sansFont
                    font.pixelSize: mh.subtitleMono ? 10 : 11
                    wrapMode: mh.subtitleMono ? Text.NoWrap : Text.WordWrap
                    elide: mh.subtitleMono ? Text.ElideMiddle : Text.ElideNone
                }
            }

            RowLayout {
                id: extraRow
                spacing: 10
                Layout.alignment: Qt.AlignTop | Qt.AlignRight
            }

            ActionButton {
                visible: mh.showClose
                label: qsTr("Close")
                shortcutHint: "Esc"
                minWidth: 88
                disabled: mh.closeDisabled
                Layout.alignment: Qt.AlignTop | Qt.AlignRight
                onClicked: mh.closed()
            }
        }

        Rectangle {
            Layout.fillWidth: true
            height: 1
            color: win.borderSubtle
        }
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
        enabled: !backend.firstRun && !backend.busy && !win.createVisible && !win.cloneVisible && !win.wipeVisible && !win.restoreDialogVisible && !win.changeReviewVisible && !win.secretsVisible && !win.staleReportVisible && !win.helpVisible && !win.historyVisible && !win.egressVisible && !win.portsVisible && !win.mountsVisible && !win.editQuotasVisible && !win.backupsVisible
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
        onActivated: backend.createSnapshot(backend.selectedEnvironment.name || "", snapNameInput.text)
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
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 54
                color: theme.darkBackground

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 20
                    anchors.rightMargin: 20
                    spacing: 16

                    // Minimalist App Brand
                    RowLayout {
                        spacing: 10
                        Rectangle {
                            width: 28
                            height: 28
                            radius: Math.min(4, theme.radius)
                            color: win.surfaceSelected
                            border.width: 1
                            border.color: theme.accent
                            Text {
                                anchors.centerIn: parent
                                text: ">_"
                                color: theme.accent
                                font.family: win.monoFont
                                font.pixelSize: 12
                            }
                        }
                        Text {
                            text: qsTr("AIBox")
                            color: theme.foreground
                            font.family: win.sansFont
                            font.pixelSize: 15
                            font.weight: Font.Medium
                        }
                        Text {
                            text: "·"
                            color: theme.muted
                            font.pixelSize: 14
                        }
                        Text {
                            text: qsTr("Isolated Linux OS Manager")
                            color: theme.muted
                            font.family: win.sansFont
                            font.pixelSize: 12
                        }
                    }

                    // Segmented Tab Switcher
                    Rectangle {
                        Layout.leftMargin: 12
                        Layout.preferredHeight: 34
                        Layout.preferredWidth: segRow.implicitWidth + 8
                        Layout.alignment: Qt.AlignVCenter
                        radius: Math.min(4, theme.radius)
                        color: win.surfaceInput
                        border.width: 1
                        border.color: win.borderSubtle

                        RowLayout {
                            id: segRow
                            anchors.centerIn: parent
                            spacing: 4

                            Repeater {
                                model: [
                                    { label: qsTr("%n Environments", "", backend.environments.length), key: "1", tab: 0 },
                                    { label: qsTr("OS Images & Downloads"), key: "2", tab: 1 }
                                ]
                                delegate: Rectangle {
                                    required property var modelData
                                    height: 28
                                    width: tabItemRow.implicitWidth + 18
                                    radius: Math.min(3, theme.radius)
                                    color: win.activeTab === modelData.tab ? win.surfaceSelected : "transparent"
                                    border.width: win.activeTab === modelData.tab ? 1 : 0
                                    border.color: theme.accent

                                    RowLayout {
                                        id: tabItemRow
                                        anchors.centerIn: parent
                                        spacing: 8
                                        Text {
                                            text: modelData.label
                                            color: win.activeTab === modelData.tab ? theme.foreground : theme.muted
                                            font.family: win.sansFont
                                            font.pixelSize: 12
                                            font.weight: win.activeTab === modelData.tab ? Font.Medium : Font.Normal
                                        }
                                        Text {
                                            text: modelData.key
                                            color: theme.muted
                                            font.family: win.monoFont
                                            font.pixelSize: 10
                                        }
                                    }

                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: win.activeTab = modelData.tab
                                    }
                                }
                            }
                        }
                    }

                    Item { Layout.fillWidth: true }

                    RowLayout {
                        spacing: 10
                        Layout.alignment: Qt.AlignVCenter

                        ActionButton {
                            label: qsTr("Secrets")
                            shortcutHint: "A"
                            minWidth: 96
                            disabled: backend.firstRun
                            Layout.alignment: Qt.AlignVCenter
                            onClicked: win.openSecretsDialog()
                        }

                        ActionButton {
                            label: qsTr("Backups")
                            shortcutHint: "B"
                            minWidth: 96
                            disabled: backend.firstRun
                            Layout.alignment: Qt.AlignVCenter
                            onClicked: win.openBackupsDialog()
                        }

                        ActionButton {
                            label: backend.recentFailedHistoryCount > 0
                                   ? qsTr("History (%1!)").arg(backend.recentFailedHistoryCount)
                                   : qsTr("History")
                            shortcutHint: "H"
                            variant: backend.recentFailedHistoryCount > 0 ? "danger" : ""
                            minWidth: 96
                            disabled: backend.firstRun
                            Layout.alignment: Qt.AlignVCenter
                            onClicked: win.openHistoryDialog()
                        }

                        ActionButton {
                            slidersIcon: true
                            minWidth: 34
                            disabled: backend.firstRun || backend.busy
                            Layout.alignment: Qt.AlignVCenter
                            onClicked: win.openSettings(0)
                            ToolTip.visible: hovered
                            ToolTip.text: qsTr("Configuration (3)")
                        }

                        ActionButton {
                            label: qsTr("Refresh")
                            shortcutHint: "R"
                            minWidth: 96
                            disabled: backend.busy
                            Layout.alignment: Qt.AlignVCenter
                            onClicked: backend.refresh()
                        }

                        ActionButton {
                            label: "?"
                            minWidth: 34
                            Layout.alignment: Qt.AlignVCenter
                            onClicked: win.helpVisible = !win.helpVisible
                        }

                        ActionButton {
                            label: qsTr("+ New Environment")
                            shortcutHint: "N"
                            variant: "primary"
                            minWidth: 165
                            disabled: backend.busy
                            Layout.alignment: Qt.AlignVCenter
                            onClicked: win.openCreateDialog("")
                        }
                    }
                }

                Rectangle {
                    anchors.bottom: parent.bottom
                    width: parent.width
                    height: 1
                    color: win.borderSubtle
                }
            }

            // ==================================================================
            // LIVE TASK PROGRESS BAR / ERROR BANNER (WHEN BUSY OUTSIDE MODAL)
            // ==================================================================
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: visible ? (backend.busy ? 68 : 42) : 0
                visible: (!win.createVisible && backend.busy) || backend.errorMessage.length > 0 || (!backend.busy && backend.statusMessage.length > 0)
                color: backend.errorMessage.length > 0 ? "#241217" : win.surfaceCard

                ColumnLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 20
                    anchors.rightMargin: 20
                    anchors.topMargin: 8
                    anchors.bottomMargin: 8
                    spacing: 6

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12

                        Text {
                            visible: backend.busy
                            text: backend.taskProgress + "%"
                            color: theme.accent
                            font.family: win.monoFont
                            font.pixelSize: 12
                        }

                        Text {
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                            text: backend.errorMessage.length > 0
                                  ? backend.errorMessage
                                  : (backend.busy ? backend.taskStage : backend.statusMessage)
                            color: backend.errorMessage.length > 0 ? theme.red : theme.foreground
                            font.family: win.sansFont
                            font.pixelSize: 12
                        }

                        Text {
                            visible: backend.busy && backend.taskLog.length > 0
                            Layout.maximumWidth: 340
                            elide: Text.ElideLeft
                            text: backend.taskLog
                            color: theme.muted
                            font.family: win.monoFont
                            font.pixelSize: 11
                        }

                        ActionButton {
                            visible: backend.busy
                            implicitHeight: 26
                            label: qsTr("Cancel")
                            danger: true
                            onClicked: backend.cancelTask()
                        }

                        ActionButton {
                            visible: backend.errorMessage.length > 0
                                     && backend.adminFixCommand(backend.errorMessage).length > 0
                            implicitHeight: 26
                            label: qsTr("Copy Fix")
                            accent: true
                            onClicked: backend.copyText(backend.adminFixCommand(backend.errorMessage),
                                                        qsTr("fix command"))
                        }

                        ActionButton {
                            visible: backend.errorMessage.length > 0
                            implicitHeight: 26
                            label: qsTr("Dismiss")
                            onClicked: backend.clearError()
                        }
                    }

                    // Progress track when busy
                    Rectangle {
                        visible: backend.busy
                        Layout.fillWidth: true
                        height: 4
                        radius: 2
                        color: win.surfaceInput

                        Rectangle {
                            height: parent.height
                            width: parent.width * (Math.max(4, backend.taskProgress) / 100.0)
                            radius: 2
                            color: theme.accent
                            Behavior on width { NumberAnimation { duration: 150 } }
                        }
                    }
                }

                Rectangle {
                    anchors.bottom: parent.bottom
                    width: parent.width
                    height: 1
                    color: backend.errorMessage.length > 0 ? theme.red : win.borderSubtle
                }
            }

            // ==================================================================
            // MAIN CONTENT AREA
            // ==================================================================
            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true

                // --------------------------------------------------------------
                // TAB 0: ENVIRONMENTS (MASTER-DETAIL VIEW)
                // --------------------------------------------------------------
                RowLayout {
                    anchors.fill: parent
                    anchors.margins: 20
                    spacing: 16
                    visible: win.activeTab === 0

                    // EMPTY STATE WHEN NO ENVIRONMENTS EXIST
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        visible: backend.environments.length === 0
                        color: win.surfaceCard
                        radius: theme.radius
                        border.width: 1
                        border.color: win.borderSubtle

                        ColumnLayout {
                            anchors.centerIn: parent
                            width: Math.min(parent.width - 80, 480)
                            spacing: 12

                            Rectangle {
                                Layout.alignment: Qt.AlignHCenter
                                width: 42
                                height: 42
                                radius: Math.min(6, theme.radius)
                                color: win.surfaceInput
                                border.width: 1
                                border.color: win.borderSubtle
                                Text {
                                    anchors.centerIn: parent
                                    text: ">_"
                                    color: theme.accent
                                    font.family: win.monoFont
                                    font.pixelSize: 16
                                }
                            }

                            Text {
                                Layout.fillWidth: true
                                horizontalAlignment: Text.AlignHCenter
                                text: qsTr("No Isolated Environments Yet")
                                color: theme.foreground
                                font.family: win.sansFont
                                font.pixelSize: 16
                                font.weight: Font.Medium
                            }

                            Text {
                                Layout.fillWidth: true
                                horizontalAlignment: Text.AlignHCenter
                                wrapMode: Text.WordWrap
                                lineHeight: 1.35
                                text: qsTr("Create a clean Linux OS container or VM for your next AI, Desktop, or Mobile project. Give AI full root access inside, share files via %1, and wipe it in 1 second when done.").arg(backend.containerWorkspacePath)
                                color: theme.muted
                                font.family: win.sansFont
                                font.pixelSize: 12
                            }

                            RowLayout {
                                Layout.alignment: Qt.AlignHCenter
                                Layout.topMargin: 8
                                spacing: 10

                                ActionButton {
                                    label: qsTr("+ Create First Environment")
                                    shortcutHint: "N"
                                    primary: true
                                    disabled: backend.busy
                                    onClicked: win.openCreateDialog("")
                                }

                                ActionButton {
                                    label: qsTr("Browse OS Images")
                                    shortcutHint: "2"
                                    onClicked: win.activeTab = 1
                                }

                                ActionButton {
                                    label: qsTr("Stale & Orphans")
                                    shortcutHint: "U"
                                    onClicked: win.openStaleReportDialog()
                                }
                            }
                        }
                    }

                    // LEFT PANE: ENVIRONMENT LIST
                    Rectangle {
                        Layout.preferredWidth: 352
                        Layout.fillHeight: true
                        visible: backend.environments.length > 0
                        color: win.surfaceCard
                        radius: theme.radius
                        border.width: 1
                        border.color: win.borderSubtle

                        ColumnLayout {
                            anchors.fill: parent
                            anchors.margins: 12
                            spacing: 10

                            RowLayout {
                                Layout.fillWidth: true
                                Text {
                                    text: qsTr("Environments")
                                    color: theme.muted
                                    font.family: win.sansFont
                                    font.pixelSize: 11
                                    font.weight: Font.Medium
                                }
                                Item { Layout.fillWidth: true }
                                Text {
                                    text: qsTr("↑/↓ select · Space enter")
                                    color: theme.muted
                                    font.family: win.monoFont
                                    font.pixelSize: 10
                                }
                            }

                            ListView {
                                id: envListView
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                clip: true
                                spacing: 6
                                model: backend.environments

                                delegate: Item {
                                    id: envRow
                                    required property var modelData
                                    required property int index
                                    width: envListView.width
                                    height: 68

                                    readonly property bool selected: backend.selectedIndex === index
                                    readonly property real diskRatio: (modelData.diskQuotaBytes || 0) > 0
                                                                     ? (modelData.diskUsageBytes || 0) / modelData.diskQuotaBytes
                                                                     : 0
                                    readonly property string networkLabel: modelData.networkMode === "offline"
                                                                           ? qsTr("NET OFF")
                                                                           : (modelData.networkMode === "allowlist"
                                                                              ? qsTr("ACL") + " · " + (modelData.ipv4 || "")
                                                                              : (modelData.ipv4 || ""))

                                    Rectangle {
                                        anchors.fill: parent
                                        radius: Math.min(4, theme.radius)
                                        color: envRow.selected ? win.surfaceSelected : win.surfaceInput
                                        border.width: 1
                                        border.color: envRow.selected ? theme.accent : win.borderSubtle

                                        ColumnLayout {
                                            anchors.fill: parent
                                            anchors.margins: 12
                                            spacing: 7

                                            // Row 1: state dot, name and badges
                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 8

                                                Rectangle {
                                                    width: 8
                                                    height: 8
                                                    radius: 4
                                                    color: modelData.running ? theme.green : theme.muted
                                                }

                                                Text {
                                                    Layout.fillWidth: true
                                                    elide: Text.ElideRight
                                                    text: modelData.name
                                                    color: theme.foreground
                                                    font.family: win.monoFont
                                                    font.pixelSize: 13
                                                    font.weight: Font.Medium
                                                }

                                                Rectangle {
                                                    visible: modelData.isTemplate || false
                                                    height: 18
                                                    width: tplListBadgeText.implicitWidth + 10
                                                    radius: 2
                                                    color: win.surfaceSelected
                                                    border.width: 1
                                                    border.color: theme.accent
                                                    Text {
                                                        id: tplListBadgeText
                                                        anchors.centerIn: parent
                                                        text: qsTr("TEMPLATE")
                                                        color: theme.accent
                                                        font.family: win.monoFont
                                                        font.pixelSize: 9
                                                    }
                                                }

                                                Rectangle {
                                                    height: 18
                                                    width: statusBadgeText.implicitWidth + 10
                                                    radius: 2
                                                    color: modelData.running ? "#162920" : win.surfaceCard
                                                    border.width: 1
                                                    border.color: modelData.running ? "#284d3a" : win.borderSubtle
                                                    Text {
                                                        id: statusBadgeText
                                                        anchors.centerIn: parent
                                                        text: modelData.status
                                                        color: modelData.running ? theme.green : theme.muted
                                                        font.family: win.monoFont
                                                        font.pixelSize: 9
                                                    }
                                                }
                                            }

                                            // Row 2: disk / memory usage and network endpoint
                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 12

                                                RowLayout {
                                                    spacing: 5

                                                    Text {
                                                        text: qsTr("DISK")
                                                        color: theme.muted
                                                        font.family: win.monoFont
                                                        font.pixelSize: 8
                                                        font.letterSpacing: 1
                                                        Layout.alignment: Qt.AlignVCenter
                                                    }

                                                    Text {
                                                        text: win.sizePair(modelData.diskUsage, modelData.diskLimit)
                                                        color: envRow.diskRatio >= 0.9 ? theme.red
                                                             : (envRow.diskRatio >= 0.75 ? theme.yellow : theme.foreground)
                                                        opacity: envRow.diskRatio >= 0.75 ? 1 : 0.72
                                                        font.family: win.monoFont
                                                        font.pixelSize: 10
                                                        Layout.alignment: Qt.AlignVCenter
                                                    }
                                                }

                                                RowLayout {
                                                    spacing: 5

                                                    Text {
                                                        text: qsTr("RAM")
                                                        color: theme.muted
                                                        font.family: win.monoFont
                                                        font.pixelSize: 8
                                                        font.letterSpacing: 1
                                                        Layout.alignment: Qt.AlignVCenter
                                                    }

                                                    Text {
                                                        text: win.sizePair(modelData.memoryUsage, modelData.memoryLimit)
                                                        color: theme.foreground
                                                        opacity: 0.72
                                                        font.family: win.monoFont
                                                        font.pixelSize: 10
                                                        Layout.alignment: Qt.AlignVCenter
                                                    }
                                                }

                                                Text {
                                                    Layout.fillWidth: true
                                                    Layout.minimumWidth: 0
                                                    horizontalAlignment: Text.AlignRight
                                                    elide: Text.ElideRight
                                                    text: envRow.networkLabel
                                                    color: modelData.networkMode === "offline" ? theme.red : theme.accent
                                                    font.family: win.monoFont
                                                    font.pixelSize: 10
                                                    Layout.alignment: Qt.AlignVCenter
                                                }
                                            }
                                        }

                                        MouseArea {
                                            id: envRowMouse
                                            anchors.fill: parent
                                            preventStealing: true
                                            hoverEnabled: true
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: backend.selectedIndex = index
                                            onDoubleClicked: backend.enterEnvironment(modelData.name)
                                            ToolTip.delay: 700
                                            ToolTip.visible: envRowMouse.containsMouse
                                            ToolTip.text: qsTr("Disk %1 / %2 · RAM %3 / %4 · %5")
                                                              .arg(modelData.diskUsage || "—")
                                                              .arg(modelData.diskLimit || "—")
                                                              .arg(modelData.memoryUsage || "—")
                                                              .arg(modelData.memoryLimit || "—")
                                                              .arg(envRow.networkLabel)
                                        }
                                    }
                                }
                            }

                            Rectangle {
                                visible: backend.poolInfo.valid || false
                                Layout.fillWidth: true
                                implicitHeight: poolSidebarRow.implicitHeight + 12
                                radius: Math.min(4, theme.radius)
                                color: win.surfaceInput
                                border.width: 1
                                border.color: (backend.poolInfo.alertLevel === 2)
                                              ? theme.red
                                              : (backend.poolInfo.alertLevel === 1 ? "#e5a93c" : win.borderSubtle)

                                RowLayout {
                                    id: poolSidebarRow
                                    anchors.fill: parent
                                    anchors.margins: 6
                                    anchors.leftMargin: 10
                                    anchors.rightMargin: 10
                                    spacing: 6

                                    Text {
                                        Layout.fillWidth: true
                                        elide: Text.ElideRight
                                        text: qsTr("Pool %1: %2 / %3 (%4%)")
                                                  .arg(backend.poolInfo.name || "default")
                                                  .arg(backend.poolInfo.usedText || "0 MiB")
                                                  .arg(backend.poolInfo.totalText || "0 MiB")
                                                  .arg(backend.poolInfo.usedPercent !== undefined ? backend.poolInfo.usedPercent : 0)
                                        color: (backend.poolInfo.alertLevel === 2)
                                               ? theme.red
                                               : (backend.poolInfo.alertLevel === 1 ? "#e5a93c" : theme.muted)
                                        font.family: win.monoFont
                                        font.pixelSize: 10
                                    }

                                    Text {
                                        visible: (backend.poolInfo.allocatedBytes || 0) > 0
                                        text: qsTr("Alloc %1").arg(backend.poolInfo.allocatedText || "0 MiB")
                                        color: (backend.poolInfo.overcommitted || false) ? "#e5a93c" : theme.muted
                                        font.family: win.monoFont
                                        font.pixelSize: 10
                                    }

                                    Text {
                                        text: qsTr("Manage")
                                        color: theme.accent
                                        font.family: win.sansFont
                                        font.pixelSize: 10
                                    }
                                }

                                MouseArea {
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: win.openPoolSettings()
                                }
                            }

                            ActionButton {
                                Layout.fillWidth: true
                                label: qsTr("Stale & Orphans Report")
                                shortcutHint: "U"
                                onClicked: win.openStaleReportDialog()
                            }
                        }
                    }

                    // RIGHT PANE: SELECTED ENVIRONMENT INSPECTOR & CONTROLS
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        visible: backend.environments.length > 0
                        color: win.surfaceCard
                        radius: theme.radius
                        border.width: 1
                        border.color: win.borderSubtle

                        ScrollView {
                            id: inspectorScroll
                            anchors.fill: parent
                            anchors.margins: 20
                            clip: true
                            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

                        ColumnLayout {
                            width: inspectorScroll.availableWidth
                            height: Math.max(inspectorScroll.availableHeight, implicitHeight)
                            spacing: 12

                            // Environment Header & Primary Action
                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 16

                                ColumnLayout {
                                    spacing: 4
                                    Layout.alignment: Qt.AlignVCenter

                                    RowLayout {
                                        spacing: 10
                                        Layout.alignment: Qt.AlignVCenter

                                        Text {
                                            id: selectedEnvTitle
                                            text: backend.selectedEnvironment.name || ""
                                            color: theme.foreground
                                            font.family: win.monoFont
                                            font.pixelSize: 18
                                            font.weight: Font.Medium
                                            elide: Text.ElideRight
                                            Layout.maximumWidth: 340
                                            Layout.alignment: Qt.AlignVCenter
                                        }

                                        Rectangle {
                                            visible: backend.selectedEnvironment.isTemplate || false
                                            implicitHeight: 22
                                            implicitWidth: tplHeaderBadgeText.implicitWidth + 14
                                            Layout.preferredHeight: 22
                                            Layout.preferredWidth: implicitWidth
                                            Layout.alignment: Qt.AlignVCenter
                                            radius: 3
                                            color: win.surfaceSelected
                                            border.width: 1
                                            border.color: theme.accent

                                            Text {
                                                id: tplHeaderBadgeText
                                                anchors.centerIn: parent
                                                text: qsTr("TEMPLATE")
                                                color: theme.accent
                                                font.family: win.monoFont
                                                font.pixelSize: 10
                                                font.weight: Font.Medium
                                            }
                                        }

                                        Rectangle {
                                            implicitHeight: 22
                                            implicitWidth: statusBadgeRow.implicitWidth + 16
                                            Layout.preferredHeight: 22
                                            Layout.preferredWidth: implicitWidth
                                            Layout.alignment: Qt.AlignVCenter
                                            radius: 3
                                            color: (backend.selectedEnvironment.running || false) ? "#162920" : win.surfaceInput
                                            border.width: 1
                                            border.color: (backend.selectedEnvironment.running || false) ? "#284d3a" : win.borderSubtle

                                            RowLayout {
                                                id: statusBadgeRow
                                                anchors.centerIn: parent
                                                spacing: 6

                                                Rectangle {
                                                    width: 6
                                                    height: 6
                                                    radius: 3
                                                    color: (backend.selectedEnvironment.running || false) ? theme.green : theme.muted
                                                    Layout.alignment: Qt.AlignVCenter
                                                }

                                                Text {
                                                    id: detailStatusText
                                                    text: backend.selectedEnvironment.status || ""
                                                    color: (backend.selectedEnvironment.running || false) ? theme.green : theme.muted
                                                    font.family: win.monoFont
                                                    font.pixelSize: 10
                                                    font.weight: Font.Medium
                                                    Layout.alignment: Qt.AlignVCenter
                                                }
                                            }
                                        }

                                        Rectangle {
                                            readonly property int secCount: (backend.selectedEnvironment.secretNames || []).length
                                            visible: secCount > 0
                                            implicitHeight: 22
                                            implicitWidth: secHeaderBadgeText.implicitWidth + 14
                                            Layout.preferredHeight: 22
                                            Layout.preferredWidth: implicitWidth
                                            Layout.alignment: Qt.AlignVCenter
                                            radius: 3
                                            color: win.surfaceInput
                                            border.width: 1
                                            border.color: theme.accent

                                            Text {
                                                id: secHeaderBadgeText
                                                anchors.centerIn: parent
                                                text: qsTr("SECRETS: %1").arg((backend.selectedEnvironment.secretNames || []).length)
                                                color: theme.accent
                                                font.family: win.monoFont
                                                font.pixelSize: 10
                                                font.weight: Font.Medium
                                            }

                                            MouseArea {
                                                anchors.fill: parent
                                                cursorShape: Qt.PointingHandCursor
                                                onClicked: win.openSecretsDialog()
                                            }
                                        }
                                    }

                                    Text {
                                        readonly property string pidStr: backend.selectedEnvironment.processLimit || ""
                                        readonly property string ioStr: backend.selectedEnvironment.diskWriteLimit || backend.selectedEnvironment.diskReadLimit || ""
                                        readonly property string limitSuffix: (pidStr.length > 0 ? "  ·  Pids " + pidStr : "")
                                                                              + (ioStr.length > 0 ? "  ·  I/O " + ioStr : "")
                                        text: (backend.selectedEnvironment.osName || "") + "  ·  " + (backend.selectedEnvironment.preset || "") + limitSuffix
                                        color: theme.muted
                                        font.family: win.sansFont
                                        font.pixelSize: 12
                                    }
                                }

                                Item { Layout.fillWidth: true }

                                ActionButton {
                                    label: (backend.selectedEnvironment.isTemplate || false)
                                           ? qsTr("New from Template")
                                           : qsTr("Enter OS Terminal")
                                    shortcutHint: (backend.selectedEnvironment.isTemplate || false) ? "" : "Space"
                                    variant: "primary"
                                    minWidth: 165
                                    disabled: backend.busy
                                    Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                                    onClicked: {
                                        const env = backend.selectedEnvironment
                                        if (env.isTemplate)
                                            win.openCreateDialog("", env.name || "")
                                        else
                                            backend.enterEnvironment(env.name || "")
                                    }
                                }
                            }

                            // Secondary Action Bar
                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10

                                ActionButton {
                                    label: (backend.selectedEnvironment.running || false) ? qsTr("Stop OS") : qsTr("Start OS")
                                    shortcutHint: "S"
                                    variant: (backend.selectedEnvironment.running || false) ? "danger" : "accent"
                                    minWidth: 98
                                    disabled: backend.busy || (backend.selectedEnvironment.isTemplate || false)
                                    Layout.alignment: Qt.AlignVCenter
                                    onClicked: backend.toggleStartStop(backend.selectedEnvironment.name || "")
                                }

                                ActionButton {
                                    label: qsTr("Clone")
                                    shortcutHint: "D"
                                    minWidth: 86
                                    disabled: backend.busy
                                    Layout.alignment: Qt.AlignVCenter
                                    onClicked: win.openCloneDialog(backend.selectedEnvironment.name || "")
                                }

                                ActionButton {
                                    label: qsTr("Quotas")
                                    shortcutHint: "E"
                                    minWidth: 92
                                    disabled: backend.busy
                                    Layout.alignment: Qt.AlignVCenter
                                    onClicked: win.openEditQuotasDialog()
                                }

                                ActionButton {
                                    label: (backend.selectedEnvironment.isTemplate || false) ? qsTr("Unmark Template") : qsTr("Template")
                                    shortcutHint: "T"
                                    variant: (backend.selectedEnvironment.isTemplate || false) ? "accent" : ""
                                    minWidth: 105
                                    disabled: backend.busy
                                    Layout.alignment: Qt.AlignVCenter
                                    onClicked: backend.setEnvironmentTemplate(backend.selectedEnvironment.name || "", !(backend.selectedEnvironment.isTemplate || false))
                                }

                                ActionButton {
                                    label: qsTr("Copy AI Command")
                                    shortcutHint: "C"
                                    minWidth: 142
                                    disabled: backend.selectedEnvironment.isTemplate || false
                                    Layout.alignment: Qt.AlignVCenter
                                    onClicked: backend.copyAiCommand(backend.selectedEnvironment.name || "")
                                }

                                ActionButton {
                                    label: qsTr("Paste Image")
                                    shortcutHint: "I"
                                    minWidth: 112
                                    disabled: backend.selectedEnvironment.isTemplate || false
                                    Layout.alignment: Qt.AlignVCenter
                                    onClicked: backend.pasteImageToWorkspace(backend.selectedEnvironment.name || "")
                                }

                                Item { Layout.fillWidth: true }

                                ActionButton {
                                    label: qsTr("Wipe OS")
                                    shortcutHint: "Del"
                                    variant: "danger"
                                    minWidth: 96
                                    disabled: backend.busy
                                    Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                                    onClicked: {
                                        win.deleteWorkspaceOnWipe = false
                                        win.wipeVisible = true
                                    }
                                }
                            }

                            // 3 Hardware Resource Monitor Cards (Task Manager Style)
                            GridLayout {
                                Layout.fillWidth: true
                                columns: 3
                                columnSpacing: 10
                                rowSpacing: 10

                                Repeater {
                                    model: [
                                        {
                                            title: qsTr("Disk Usage"),
                                            val: (backend.selectedEnvironment.diskUsage || "0 MB") + " / " + (backend.selectedEnvironment.diskLimit || "—"),
                                            subtext: "",
                                            subAlert: 0,
                                            pct: backend.selectedEnvironment.diskPercent !== undefined ? backend.selectedEnvironment.diskPercent : 0,
                                            badge: (backend.selectedEnvironment.diskAlertLevel === 2
                                                    ? qsTr("CRIT ")
                                                    : (backend.selectedEnvironment.diskAlertLevel === 1 ? qsTr("WARN ") : ""))
                                                   + (backend.selectedEnvironment.diskPercent !== undefined ? backend.selectedEnvironment.diskPercent : 0) + "%",
                                            hasBar: true,
                                            isCpu: false,
                                            isNetwork: false,
                                            alertLevel: backend.selectedEnvironment.diskAlertLevel !== undefined ? backend.selectedEnvironment.diskAlertLevel : 0,
                                            cores: [],
                                            active: true
                                        },
                                        {
                                            title: qsTr("Memory Usage"),
                                            val: (backend.selectedEnvironment.memoryUsage || "0 MB") + " / " + (backend.selectedEnvironment.memoryLimit || "—"),
                                            subtext: "",
                                            subAlert: 0,
                                            pct: backend.selectedEnvironment.memoryPercent !== undefined ? backend.selectedEnvironment.memoryPercent : 0,
                                            badge: (backend.selectedEnvironment.memoryPercent !== undefined ? backend.selectedEnvironment.memoryPercent : 0) + "%",
                                            hasBar: true,
                                            isCpu: false,
                                            isNetwork: false,
                                            alertLevel: 0,
                                            cores: [],
                                            active: backend.selectedEnvironment.running || false
                                        },
                                        {
                                            title: qsTr("CPU Usage") + " · " + (backend.selectedEnvironment.cpuLimit || "—"),
                                            val: (backend.selectedEnvironment.cpuUsage || "0%") + " · " + (backend.selectedEnvironment.cpuLimit || "—"),
                                            subtext: "",
                                            subAlert: 0,
                                            pct: backend.selectedEnvironment.cpuPercent !== undefined ? backend.selectedEnvironment.cpuPercent : 0,
                                            badge: backend.selectedEnvironment.cpuUsage || "0%",
                                            hasBar: false,
                                            isCpu: true,
                                            isNetwork: false,
                                            alertLevel: 0,
                                            cores: backend.selectedEnvironment.cpuCorePercents || [0, 0, 0, 0],
                                            active: backend.selectedEnvironment.running || false
                                        }
                                    ]
                                    delegate: Rectangle {
                                        id: statCard
                                        required property var modelData
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: 94
                                        radius: Math.min(4, theme.radius)
                                        color: win.surfaceInput
                                        border.width: 1
                                        border.color: statCard.modelData.alertLevel === 2
                                                      ? theme.red
                                                      : (statCard.modelData.alertLevel === 1 ? "#e5a93c" : win.borderSubtle)

                                        ColumnLayout {
                                            anchors.fill: parent
                                            anchors.margins: 10
                                            spacing: 4

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 6

                                                Text {
                                                    Layout.fillWidth: true
                                                    elide: Text.ElideRight
                                                    text: statCard.modelData.title
                                                    color: theme.muted
                                                    font.family: win.sansFont
                                                    font.pixelSize: 11
                                                }

                                                Text {
                                                    text: statCard.modelData.badge
                                                    color: statCard.modelData.alertLevel === 2
                                                           ? theme.red
                                                           : (statCard.modelData.alertLevel === 1
                                                              ? "#e5a93c"
                                                              : (statCard.modelData.active
                                                                 ? (statCard.modelData.isNetwork ? theme.green : (statCard.modelData.pct >= 85 ? theme.red : theme.accent))
                                                                 : theme.muted))
                                                    font.family: win.monoFont
                                                    font.pixelSize: 10
                                                    font.weight: Font.Medium
                                                }
                                            }

                                            // Standard single-value row (Disk, Memory, IPv4)
                                            Text {
                                                visible: !statCard.modelData.isCpu
                                                Layout.fillWidth: true
                                                elide: Text.ElideRight
                                                text: statCard.modelData.val
                                                color: theme.foreground
                                                font.family: win.monoFont
                                                font.pixelSize: 13
                                            }

                                            Text {
                                                visible: !statCard.modelData.isCpu && (statCard.modelData.subtext || "").length > 0
                                                Layout.fillWidth: true
                                                elide: Text.ElideRight
                                                text: statCard.modelData.subtext || ""
                                                color: statCard.modelData.subAlert === 2
                                                       ? theme.red
                                                       : (statCard.modelData.subAlert === 1 ? "#e5a93c" : theme.muted)
                                                font.family: win.monoFont
                                                font.pixelSize: 10
                                            }

                                            Item {
                                                visible: !statCard.modelData.isCpu
                                                Layout.fillHeight: true
                                            }

                                            Rectangle {
                                                visible: statCard.modelData.hasBar && !statCard.modelData.isCpu
                                                Layout.fillWidth: true
                                                height: 4
                                                radius: 2
                                                color: win.surfaceCard

                                                Rectangle {
                                                    height: parent.height
                                                    width: statCard.modelData.pct > 0
                                                           ? Math.max(4, parent.width * (Math.min(100, statCard.modelData.pct) / 100.0))
                                                           : 0
                                                    radius: 2
                                                    color: statCard.modelData.isNetwork
                                                           ? (statCard.modelData.active ? theme.green : theme.muted)
                                                           : (statCard.modelData.alertLevel === 2
                                                              ? theme.red
                                                              : (statCard.modelData.alertLevel === 1
                                                                 ? "#e5a93c"
                                                                 : (statCard.modelData.pct >= 85 ? theme.red : theme.accent)))
                                                    Behavior on width { NumberAnimation { duration: 180 } }
                                                }
                                            }

                                            // Per-core CPU Task Manager Grid
                                            GridLayout {
                                                visible: statCard.modelData.isCpu
                                                Layout.fillWidth: true
                                                Layout.fillHeight: true
                                                columns: statCard.modelData.cores.length > 4 ? 3 : 2
                                                columnSpacing: 8
                                                rowSpacing: 4

                                                Repeater {
                                                    model: statCard.modelData.cores
                                                    delegate: ColumnLayout {
                                                        required property var modelData
                                                        required property int index
                                                        Layout.fillWidth: true
                                                        spacing: 2

                                                        RowLayout {
                                                            Layout.fillWidth: true
                                                            spacing: 4
                                                            Text {
                                                                text: "C" + (index + 1)
                                                                color: theme.muted
                                                                font.family: win.monoFont
                                                                font.pixelSize: 9
                                                            }
                                                            Item { Layout.fillWidth: true }
                                                            Text {
                                                                text: modelData + "%"
                                                                color: statCard.modelData.active ? theme.foreground : theme.muted
                                                                font.family: win.monoFont
                                                                font.pixelSize: 9
                                                            }
                                                        }

                                                        Rectangle {
                                                            Layout.fillWidth: true
                                                            height: 3
                                                            radius: 1.5
                                                            color: win.surfaceCard

                                                            Rectangle {
                                                                height: parent.height
                                                                width: modelData > 0
                                                                       ? Math.max(3, parent.width * (Math.min(100, modelData) / 100.0))
                                                                       : 0
                                                                radius: 1.5
                                                                color: modelData >= 85 ? theme.red : theme.accent
                                                                Behavior on width { NumberAnimation { duration: 180 } }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Shared Folder Info Box
                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 50
                                radius: Math.min(4, theme.radius)
                                color: win.surfaceInput
                                border.width: 1
                                border.color: win.borderSubtle

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 14
                                    anchors.rightMargin: 14
                                    spacing: 10

                                    Text {
                                        text: qsTr("Shared Workspace")
                                        color: theme.muted
                                        font.family: win.sansFont
                                        font.pixelSize: 11
                                    }
                                    Text {
                                        Layout.fillWidth: true
                                        Layout.minimumWidth: 0
                                        elide: Text.ElideMiddle
                                        text: (backend.selectedEnvironment.workspacePath || "") + "  ⟷  " + backend.containerWorkspacePath
                                        color: theme.accent
                                        font.family: win.monoFont
                                        font.pixelSize: 12
                                    }

                                    ActionButton {
                                        label: (backend.selectedEnvironment.extraMountCount || 0) > 0
                                               ? qsTr("Mounts (%1)").arg(backend.selectedEnvironment.extraMountCount)
                                               : qsTr("Extra Mounts")
                                        minWidth: 112
                                        Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                                        onClicked: win.openMountsDialog()
                                    }

                                    ActionButton {
                                        label: qsTr("Open Shared Folder")
                                        shortcutHint: "F"
                                        minWidth: 165
                                        Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                                        onClicked: backend.openWorkspaceFolder(backend.selectedEnvironment.name || "")
                                    }
                                }
                            }

                            // Root File Access (localhost-only SFTP) Info Box
                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 50
                                radius: Math.min(4, theme.radius)
                                color: win.surfaceInput
                                border.width: 1
                                border.color: (backend.selectedEnvironment.fileAccessActive || false)
                                              ? theme.accent
                                              : win.borderSubtle

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 14
                                    anchors.rightMargin: 14
                                    spacing: 10

                                    Text {
                                        text: qsTr("Root Files")
                                        color: theme.muted
                                        font.family: win.sansFont
                                        font.pixelSize: 11
                                        ToolTip.visible: rootLabelHover.hovered
                                        ToolTip.delay: 500
                                        ToolTip.text: qsTr("Browse this environment's root filesystem in your file manager over a localhost-only, passwordless SFTP connection. Anyone on this machine can read and write files as root while it is enabled.")
                                        HoverHandler { id: rootLabelHover }
                                    }

                                    Text {
                                        id: rootAccessText
                                        Layout.fillWidth: true
                                        Layout.minimumWidth: 0
                                        elide: Text.ElideMiddle
                                        text: (backend.selectedEnvironment.fileAccessActive || false)
                                              ? (backend.selectedEnvironment.fileAccessUrl || "")
                                              : (backend.selectedEnvironment.running
                                                 ? qsTr("Off — browse the container root in your file manager")
                                                 : qsTr("Off — start the environment to enable"))
                                        color: (backend.selectedEnvironment.fileAccessActive || false)
                                               ? theme.accent
                                               : theme.muted
                                        font.family: win.monoFont
                                        font.pixelSize: 12
                                        ToolTip.visible: rootUrlHover.hovered && (backend.selectedEnvironment.fileAccessActive || false)
                                        ToolTip.delay: 400
                                        ToolTip.text: qsTr("Click to copy this SFTP URL")
                                        HoverHandler {
                                            id: rootUrlHover
                                            cursorShape: (backend.selectedEnvironment.fileAccessActive || false)
                                                         ? Qt.PointingHandCursor : Qt.ArrowCursor
                                        }
                                        TapHandler {
                                            enabled: backend.selectedEnvironment.fileAccessActive || false
                                            onTapped: backend.copyText(
                                                backend.selectedEnvironment.fileAccessUrl || "",
                                                qsTr("SFTP URL"))
                                        }
                                    }

                                    ActionButton {
                                        visible: backend.selectedEnvironment.fileAccessActive || false
                                        label: qsTr("Open")
                                        minWidth: 78
                                        onClicked: backend.openFileAccess(backend.selectedEnvironment.name || "")
                                    }

                                    ActionButton {
                                        label: (backend.selectedEnvironment.fileAccessActive || false)
                                               ? qsTr("Disable")
                                               : qsTr("Enable")
                                        variant: (backend.selectedEnvironment.fileAccessActive || false)
                                                 ? "danger"
                                                 : "accent"
                                        minWidth: 92
                                        disabled: backend.busy
                                                  || (backend.selectedEnvironment.isTemplate || false)
                                                  || (!(backend.selectedEnvironment.fileAccessActive || false)
                                                      && !(backend.selectedEnvironment.running || false))
                                        onClicked: {
                                            const name = backend.selectedEnvironment.name || ""
                                            if (backend.selectedEnvironment.fileAccessActive || false)
                                                backend.disableFileAccess(name)
                                            else
                                                backend.enableFileAccess(name)
                                        }
                                    }
                                }
                            }

                            // Network Control Bar
                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: netCol.implicitHeight + 16
                                radius: Math.min(4, theme.radius)
                                color: win.surfaceInput
                                border.width: 1
                                border.color: backend.selectedEnvironment.networkMode === "offline"
                                              ? theme.red
                                              : (backend.selectedEnvironment.networkMode === "allowlist" ? theme.accent : win.borderSubtle)

                                ColumnLayout {
                                    id: netCol
                                    anchors.fill: parent
                                    anchors.leftMargin: 14
                                    anchors.rightMargin: 14
                                    anchors.topMargin: 8
                                    anchors.bottomMargin: 8
                                    spacing: 10

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10

                                        Text {
                                            text: qsTr("Network Mode")
                                            color: theme.muted
                                            font.family: win.sansFont
                                            font.pixelSize: 11
                                        }

                                        Repeater {
                                            model: [
                                                { id: "online", label: qsTr("Online") },
                                                { id: "allowlist", label: qsTr("Allowlist") },
                                                { id: "offline", label: qsTr("Offline") }
                                            ]
                                            delegate: Rectangle {
                                                required property var modelData
                                                readonly property string activeMode: backend.selectedEnvironment.networkMode || "online"
                                                readonly property bool selected: (modelData.id === "allowlist" && win.netAllowlistEditing)
                                                                                 || (!win.netAllowlistEditing && activeMode === modelData.id)
                                                height: 28
                                                width: netChipText.implicitWidth + 20
                                                radius: Math.min(4, theme.radius)
                                                color: selected ? win.surfaceSelected : win.surfaceCard
                                                border.width: 1
                                                border.color: selected
                                                              ? (modelData.id === "offline" ? theme.red : theme.accent)
                                                              : win.borderSubtle
                                                opacity: backend.busy ? 0.5 : 1.0

                                                Text {
                                                    id: netChipText
                                                    anchors.centerIn: parent
                                                    text: modelData.label
                                                    color: selected ? theme.foreground : theme.muted
                                                    font.family: win.sansFont
                                                    font.pixelSize: 11
                                                    font.weight: selected ? Font.Medium : Font.Normal
                                                }

                                                MouseArea {
                                                    anchors.fill: parent
                                                    enabled: !backend.busy && !(backend.selectedEnvironment.isTemplate || false)
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: {
                                                        const name = backend.selectedEnvironment.name || ""
                                                        if (modelData.id === "allowlist") {
                                                            win.netAllowlistEditing = true
                                                            if (win.netAllowlistDraft.length === 0)
                                                                win.netAllowlistDraft = backend.selectedEnvironment.networkAllowlist || ""
                                                        } else {
                                                            win.netAllowlistEditing = false
                                                            backend.setNetworkMode(name, modelData.id, "")
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        Item { Layout.fillWidth: true }

                                        ActionButton {
                                            label: (backend.selectedEnvironment.egressRecording || false)
                                                   ? qsTr("Record: ON")
                                                   : qsTr("Record: OFF")
                                            variant: (backend.selectedEnvironment.egressRecording || false) ? "accent" : ""
                                            minWidth: 104
                                            disabled: backend.busy || (backend.selectedEnvironment.isTemplate || false)
                                                      || backend.selectedEnvironment.networkMode === "offline"
                                            onClicked: backend.setEgressRecording(
                                                backend.selectedEnvironment.name || "",
                                                !(backend.selectedEnvironment.egressRecording || false))
                                        }

                                        ActionButton {
                                            label: (backend.selectedEnvironment.portForwardCount || 0) > 0
                                                   ? qsTr("Ports (%1)").arg(backend.selectedEnvironment.portForwardCount)
                                                   : qsTr("Ports")
                                            minWidth: 86
                                            disabled: backend.selectedEnvironment.isTemplate || false
                                            onClicked: win.openPortsDialog()
                                        }

                                        ActionButton {
                                            label: qsTr("Egress Log")
                                            minWidth: 96
                                            disabled: backend.selectedEnvironment.isTemplate || false
                                            onClicked: win.openEgressDialog(backend.selectedEnvironment.name || "")
                                        }

                                        ActionButton {
                                            label: backend.selectedEnvironment.networkMode === "offline"
                                                   ? qsTr("Restore Online")
                                                   : qsTr("Kill Network")
                                            shortcutHint: "K"
                                            variant: backend.selectedEnvironment.networkMode === "offline" ? "accent" : "danger"
                                            minWidth: 132
                                            disabled: backend.busy || (backend.selectedEnvironment.isTemplate || false)
                                            onClicked: {
                                                const name = backend.selectedEnvironment.name || ""
                                                const nextMode = backend.selectedEnvironment.networkMode === "offline" ? "online" : "offline"
                                                win.netAllowlistEditing = false
                                                backend.setNetworkMode(name, nextMode, "")
                                            }
                                        }
                                    }

                                    RowLayout {
                                        visible: win.netAllowlistEditing || backend.selectedEnvironment.networkMode === "allowlist"
                                        Layout.fillWidth: true
                                        spacing: 8

                                        FlatInput {
                                            id: allowlistInput
                                            Layout.fillWidth: true
                                            implicitHeight: 34
                                            text: win.netAllowlistDraft
                                            onTextChanged: win.netAllowlistDraft = text
                                            placeholder: qsTr("Allowed domains or CIDRs (e.g. github.com, api.anthropic.com, 1.1.1.1)")
                                            onAccepted: {
                                                if (!backend.busy)
                                                    backend.setNetworkMode(backend.selectedEnvironment.name || "", "allowlist", win.netAllowlistDraft)
                                            }
                                        }

                                        ActionButton {
                                            label: qsTr("Apply ACL")
                                            variant: "primary"
                                            minWidth: 96
                                            disabled: backend.busy || win.netAllowlistDraft.trim().length === 0 || (backend.selectedEnvironment.isTemplate || false)
                                            onClicked: backend.setNetworkMode(backend.selectedEnvironment.name || "", "allowlist", win.netAllowlistDraft)
                                        }

                                        ActionButton {
                                            visible: backend.selectedEnvironment.networkMode === "allowlist"
                                            label: qsTr("Refresh IPs")
                                            minWidth: 96
                                            disabled: backend.busy || (backend.selectedEnvironment.isTemplate || false)
                                            onClicked: backend.refreshNetworkAllowlist(backend.selectedEnvironment.name || "")
                                        }
                                    }
                                }
                            }

                            // Network Monitor (live ping, throughput & totals)
                            Rectangle {
                                id: netMonitorCard
                                Layout.fillWidth: true
                                Layout.preferredHeight: netMonitorCol.implicitHeight + 20
                                radius: Math.min(4, theme.radius)
                                color: win.surfaceInput
                                border.width: 1
                                border.color: win.borderSubtle

                                readonly property var stats: backend.networkStats
                                readonly property bool live: stats.active === true
                                readonly property real pingMs: stats.pingMs !== undefined ? stats.pingMs : -1
                                readonly property real lossPct: stats.packetLossPercent !== undefined ? stats.packetLossPercent : -1

                                function pingText() {
                                    if (!live || pingMs < 0)
                                        return "—"
                                    return (pingMs < 10 ? pingMs.toFixed(1) : Math.round(pingMs).toString()) + " ms"
                                }

                                function lossText() {
                                    if (!live || lossPct < 0)
                                        return "—"
                                    return (lossPct % 1 === 0 ? lossPct.toFixed(0) : lossPct.toFixed(1)) + "%"
                                }

                                ColumnLayout {
                                    id: netMonitorCol
                                    anchors.fill: parent
                                    anchors.margins: 14
                                    spacing: 10

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 8

                                        Text {
                                            text: qsTr("Network Monitor")
                                            color: theme.foreground
                                            font.family: win.sansFont
                                            font.pixelSize: 12
                                            font.weight: Font.Medium
                                        }

                                        Rectangle {
                                            Layout.alignment: Qt.AlignVCenter
                                            width: 6
                                            height: 6
                                            radius: 3
                                            color: netMonitorCard.live ? theme.green : theme.muted
                                        }

                                        Text {
                                            text: netMonitorCard.live
                                                  ? qsTr("LIVE")
                                                  : (backend.selectedEnvironment.networkMode === "offline"
                                                     ? qsTr("NET OFF")
                                                     : qsTr("IDLE"))
                                            color: netMonitorCard.live ? theme.green : theme.muted
                                            font.family: win.monoFont
                                            font.pixelSize: 10
                                            font.weight: Font.Medium
                                            Layout.alignment: Qt.AlignVCenter
                                        }

                                        Item { Layout.fillWidth: true }

                                        ActionButton {
                                            label: qsTr("Probe")
                                            minWidth: 72
                                            implicitHeight: 26
                                            disabled: backend.busy || !(backend.selectedEnvironment.running || false)
                                            Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                                            onClicked: backend.refreshNetworkStats()
                                        }
                                    }

                                    // Two equal-width columns so labels and values line up
                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 30

                                        Repeater {
                                            model: [
                                                [
                                                    { label: qsTr("Ping"), value: netMonitorCard.pingText(), tone: "normal", copyValue: "" },
                                                    { label: qsTr("Receiving"), value: netMonitorCard.stats.rxRateText || "0 KB/s", tone: "down", copyValue: "" },
                                                    { label: qsTr("Downloaded"), value: netMonitorCard.stats.rxTotalText || "0 B", tone: "normal", copyValue: "" },
                                                    { label: qsTr("IP Address"), value: netMonitorCard.stats.ip || "—", tone: "normal", copyValue: netMonitorCard.stats.ip || "" }
                                                ],
                                                [
                                                    { label: qsTr("Packet Loss"), value: netMonitorCard.lossText(), tone: netMonitorCard.lossPct > 0 ? "warn" : "normal", copyValue: "" },
                                                    { label: qsTr("Sending"), value: netMonitorCard.stats.txRateText || "0 KB/s", tone: "up", copyValue: "" },
                                                    { label: qsTr("Uploaded"), value: netMonitorCard.stats.txTotalText || "0 B", tone: "normal", copyValue: "" },
                                                    { label: qsTr("Gateway"), value: netMonitorCard.stats.gateway || "—", tone: "normal", copyValue: netMonitorCard.stats.gateway || "" }
                                                ]
                                            ]

                                            delegate: ColumnLayout {
                                                required property var modelData
                                                Layout.fillWidth: true
                                                Layout.preferredWidth: 0
                                                spacing: 8

                                                Repeater {
                                                    model: modelData

                                                    delegate: RowLayout {
                                                        id: netRow
                                                        required property var modelData

                                                        readonly property bool copyable: (modelData.copyValue || "").length > 0
                                                        property bool justCopied: false

                                                        Layout.fillWidth: true
                                                        Layout.minimumWidth: 0
                                                        spacing: 10

                                                        Text {
                                                            text: modelData.label
                                                            color: netRow.copyable && netHover.hovered ? theme.foreground : theme.muted
                                                            font.family: win.sansFont
                                                            font.pixelSize: 11
                                                        }

                                                        Item { Layout.fillWidth: true }

                                                        Text {
                                                            Layout.minimumWidth: 0
                                                            elide: Text.ElideRight
                                                            text: netRow.justCopied ? qsTr("Copied") : modelData.value
                                                            color: netRow.justCopied
                                                                   ? theme.accent
                                                                   : (modelData.value === "—"
                                                                      ? theme.muted
                                                                      : (modelData.tone === "down"
                                                                         ? theme.green
                                                                         : (modelData.tone === "up"
                                                                            ? theme.accent
                                                                            : (modelData.tone === "warn" ? "#e5a93c" : theme.foreground))))
                                                            font.family: win.monoFont
                                                            font.pixelSize: 12
                                                        }

                                                        // Monochrome copy glyph: hover hint for copyable values
                                                        Item {
                                                            visible: netRow.copyable
                                                            Layout.alignment: Qt.AlignVCenter
                                                            Layout.preferredWidth: 10
                                                            Layout.preferredHeight: 10
                                                            opacity: netRow.justCopied ? 1.0 : (netHover.hovered ? 0.95 : 0.4)

                                                            Rectangle {
                                                                x: 0
                                                                y: 3
                                                                width: 7
                                                                height: 7
                                                                radius: 1
                                                                color: "transparent"
                                                                border.width: 1
                                                                border.color: netRow.justCopied ? theme.accent : (netHover.hovered ? theme.accent : theme.muted)
                                                            }
                                                            Rectangle {
                                                                x: 3
                                                                y: 0
                                                                width: 7
                                                                height: 7
                                                                radius: 1
                                                                color: win.surfaceInput
                                                                border.width: 1
                                                                border.color: netRow.justCopied ? theme.accent : (netHover.hovered ? theme.accent : theme.muted)
                                                            }
                                                        }

                                                        HoverHandler {
                                                            id: netHover
                                                            enabled: netRow.copyable
                                                            cursorShape: Qt.PointingHandCursor
                                                        }

                                                        TapHandler {
                                                            enabled: netRow.copyable
                                                            onTapped: {
                                                                if (backend.copyText(netRow.modelData.copyValue, netRow.modelData.label)) {
                                                                    netRow.justCopied = true
                                                                    copyResetTimer.restart()
                                                                }
                                                            }
                                                        }

                                                        Timer {
                                                            id: copyResetTimer
                                                            interval: 1200
                                                            onTriggered: netRow.justCopied = false
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // SNAPSHOTS (TIME MACHINE) SECTION
                            Rectangle {
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                Layout.minimumHeight: 150
                                radius: Math.min(4, theme.radius)
                                color: win.surfaceInput
                                border.width: 1
                                border.color: win.borderSubtle

                                ColumnLayout {
                                    anchors.fill: parent
                                    anchors.margins: 14
                                    spacing: 10

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10

                                        ColumnLayout {
                                            spacing: 2
                                            Text {
                                                text: qsTr("Btrfs Snapshots (Instant Rollback)")
                                                color: theme.foreground
                                                font.family: win.sansFont
                                                font.pixelSize: 12
                                                font.weight: Font.Medium
                                            }
                                            Text {
                                                text: qsTr("Auto-snapshot runs before Enter & Start (keeps last %1)").arg(backend.autoSnapshotKeep)
                                                color: theme.muted
                                                font.family: win.sansFont
                                                font.pixelSize: 11
                                            }
                                        }

                                        Item { Layout.fillWidth: true }

                                        ActionButton {
                                            implicitHeight: 34
                                            minWidth: 112
                                            label: (backend.selectedEnvironment.autoStopOnCritical || false)
                                                   ? qsTr("Auto-stop: ON")
                                                   : qsTr("Auto-stop: OFF")
                                            variant: (backend.selectedEnvironment.autoStopOnCritical || false) ? "danger" : ""
                                            disabled: backend.busy
                                            Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                                            onClicked: backend.setEnvironmentAutoStop(
                                                backend.selectedEnvironment.name || "",
                                                !(backend.selectedEnvironment.autoStopOnCritical || false))
                                        }

                                        ActionButton {
                                            implicitHeight: 34
                                            minWidth: 86
                                            visible: backend.selectedEnvironment.workspaceIsSubvolume || false
                                            label: (backend.selectedEnvironment.workspaceSnapshotEnabled !== false)
                                                   ? qsTr("WS: ON")
                                                   : qsTr("WS: OFF")
                                            variant: (backend.selectedEnvironment.workspaceSnapshotEnabled !== false) ? "accent" : ""
                                            disabled: backend.busy
                                            Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                                            onClicked: backend.setEnvironmentWorkspaceSnapshot(
                                                backend.selectedEnvironment.name || "",
                                                !(backend.selectedEnvironment.workspaceSnapshotEnabled !== false))
                                        }

                                        ActionButton {
                                            implicitHeight: 34
                                            minWidth: 92
                                            label: (backend.selectedEnvironment.autoSnapshotEnabled !== false)
                                                   ? qsTr("Auto: ON")
                                                   : qsTr("Auto: OFF")
                                            variant: (backend.selectedEnvironment.autoSnapshotEnabled !== false) ? "accent" : ""
                                            disabled: backend.busy
                                            Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                                            onClicked: backend.setEnvironmentAutoSnapshot(
                                                backend.selectedEnvironment.name || "",
                                                !(backend.selectedEnvironment.autoSnapshotEnabled !== false))
                                        }

                                        FlatInput {
                                            id: snapNameInput
                                            Layout.preferredWidth: 165
                                            Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                                            implicitHeight: 34
                                            placeholder: qsTr("Snapshot label (optional)")
                                            onAccepted: {
                                                if (!backend.busy) {
                                                    backend.createSnapshot(backend.selectedEnvironment.name || "", snapNameInput.text)
                                                    snapNameInput.text = ""
                                                }
                                            }
                                        }

                                        ActionButton {
                                            implicitHeight: 34
                                            minWidth: 138
                                            label: qsTr("+ Save Snapshot")
                                            shortcutHint: "P"
                                            disabled: backend.busy
                                            Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                                            onClicked: {
                                                backend.createSnapshot(backend.selectedEnvironment.name || "", snapNameInput.text)
                                                snapNameInput.text = ""
                                            }
                                        }
                                    }

                                    // Per-environment Idle Auto-Stop policy (H1)
                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 8
                                        readonly property string curIdleOv: backend.selectedEnvironment.idleStopOverride || "inherit"
                                        readonly property bool curIgnorePorts: backend.selectedEnvironment.idleStopIgnorePorts || false
                                        readonly property int globalIdleMin: backend.idleStopMinutes

                                        Text {
                                            text: qsTr("Idle Auto-Stop:")
                                            color: theme.muted
                                            font.family: win.sansFont
                                            font.pixelSize: 11
                                        }

                                        Repeater {
                                            model: [
                                                { val: "inherit", lbl: parent.globalIdleMin > 0 ? qsTr("Inherit (%1m)").arg(parent.globalIdleMin) : qsTr("Inherit (Off)") },
                                                { val: "off", lbl: qsTr("Off") },
                                                { val: "15", lbl: "15m" },
                                                { val: "30", lbl: "30m" },
                                                { val: "60", lbl: "60m" },
                                                { val: "120", lbl: "120m" }
                                            ]
                                            delegate: Rectangle {
                                                required property var modelData
                                                readonly property string normOv: (backend.selectedEnvironment.idleStopOverride || "inherit")
                                                readonly property bool selected: normOv === modelData.val || (modelData.val === "inherit" && normOv.length === 0)
                                                height: 26
                                                width: idleOvTxt.implicitWidth + 14
                                                radius: Math.min(4, theme.radius)
                                                color: selected ? win.surfaceSelected : win.surfaceCard
                                                border.width: 1
                                                border.color: selected ? theme.accent : win.borderSubtle
                                                Text {
                                                    id: idleOvTxt
                                                    anchors.centerIn: parent
                                                    text: modelData.lbl
                                                    color: selected ? theme.accent : theme.muted
                                                    font.family: win.monoFont
                                                    font.pixelSize: 10
                                                }
                                                MouseArea {
                                                    anchors.fill: parent
                                                    cursorShape: Qt.PointingHandCursor
                                                    enabled: !backend.busy
                                                    onClicked: backend.setEnvironmentIdleStop(
                                                        backend.selectedEnvironment.name || "",
                                                        modelData.val,
                                                        backend.selectedEnvironment.idleStopIgnorePorts || false)
                                                }
                                            }
                                        }

                                        Rectangle {
                                            id: allowPortsPill
                                            readonly property bool allowPorts: backend.selectedEnvironment.idleStopIgnorePorts || false
                                            height: 26
                                            width: allowPortsTxt.implicitWidth + 14
                                            radius: Math.min(4, theme.radius)
                                            color: allowPorts ? win.surfaceSelected : win.surfaceCard
                                            border.width: 1
                                            border.color: allowPorts ? theme.accent : win.borderSubtle
                                            Text {
                                                id: allowPortsTxt
                                                anchors.centerIn: parent
                                                text: allowPortsPill.allowPorts ? qsTr("Ignore ports: ON") : qsTr("Ignore ports: OFF")
                                                color: allowPortsPill.allowPorts ? theme.accent : theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 10
                                            }
                                            MouseArea {
                                                anchors.fill: parent
                                                cursorShape: Qt.PointingHandCursor
                                                enabled: !backend.busy
                                                onClicked: backend.setEnvironmentIdleStop(
                                                    backend.selectedEnvironment.name || "",
                                                    backend.selectedEnvironment.idleStopOverride || "inherit",
                                                    !(backend.selectedEnvironment.idleStopIgnorePorts || false))
                                            }
                                        }

                                        Item { Layout.fillWidth: true }
                                    }

                                    // Per-environment Session Recording policy (H3)
                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 8
                                        visible: !(backend.selectedEnvironment.isTemplate || false)

                                        Text {
                                            text: qsTr("Session Recording:")
                                            color: theme.muted
                                            font.family: win.sansFont
                                            font.pixelSize: 11
                                        }

                                        Repeater {
                                            model: [
                                                { val: "inherit", label: backend.recordSessions ? qsTr("Inherit (On)") : qsTr("Inherit (Off)") },
                                                { val: "true", label: qsTr("On") },
                                                { val: "false", label: qsTr("Off") }
                                            ]
                                            delegate: Rectangle {
                                                required property var modelData
                                                readonly property string curOv: (backend.selectedEnvironment.recordOverride || "").toLowerCase()
                                                readonly property bool selected: (modelData.val === "inherit" && (curOv === "" || curOv === "inherit")) || (curOv === modelData.val)
                                                height: 26
                                                width: recOvTxt.implicitWidth + 14
                                                radius: Math.min(4, theme.radius)
                                                color: selected ? win.surfaceSelected : win.surfaceCard
                                                border.width: 1
                                                border.color: selected ? theme.accent : win.borderSubtle
                                                Text {
                                                    id: recOvTxt
                                                    anchors.centerIn: parent
                                                    text: modelData.label
                                                    color: selected ? theme.accent : theme.muted
                                                    font.family: win.monoFont
                                                    font.pixelSize: 10
                                                }
                                                MouseArea {
                                                    anchors.fill: parent
                                                    cursorShape: Qt.PointingHandCursor
                                                    enabled: !backend.busy
                                                    onClicked: backend.setEnvironmentRecordSessions(
                                                        backend.selectedEnvironment.name || "",
                                                        modelData.val)
                                                }
                                            }
                                        }

                                        Item { Layout.fillWidth: true }
                                    }

                                    Rectangle {
                                        Layout.fillWidth: true
                                        height: 1
                                        color: win.borderSubtle
                                    }

                                    // One-time conversion / cleanup banner for plain-directory workspaces on Btrfs
                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10
                                        visible: ((backend.selectedEnvironment.workspaceOnBtrfs || false) && !(backend.selectedEnvironment.workspaceIsSubvolume || false)) || (backend.selectedEnvironment.hasPreSubvolBackup || false)

                                        Text {
                                            Layout.fillWidth: true
                                            elide: Text.ElideRight
                                            text: (backend.selectedEnvironment.hasPreSubvolBackup || false)
                                                  ? qsTr("Pre-subvolume backup (.pre-subvol) is kept on disk.")
                                                  : qsTr("Workspace is a plain folder. Convert to Btrfs subvolume to enable paired %1 snapshots.").arg(backend.containerWorkspacePath)
                                            color: theme.muted
                                            font.family: win.sansFont
                                            font.pixelSize: 11
                                        }

                                        ActionButton {
                                            implicitHeight: 28
                                            visible: (backend.selectedEnvironment.workspaceOnBtrfs || false) && !(backend.selectedEnvironment.workspaceIsSubvolume || false)
                                            label: qsTr("Convert to Subvolume")
                                            disabled: backend.busy || (backend.selectedEnvironment.running || false)
                                            onClicked: backend.convertWorkspaceToSubvolume(backend.selectedEnvironment.name || "")
                                        }

                                        ActionButton {
                                            implicitHeight: 28
                                            visible: backend.selectedEnvironment.hasPreSubvolBackup || false
                                            label: qsTr("Delete .pre-subvol")
                                            danger: true
                                            disabled: backend.busy
                                            onClicked: backend.deletePreSubvolBackup(backend.selectedEnvironment.name || "")
                                        }
                                    }

                                    Text {
                                        visible: (!backend.selectedEnvironment.snapshots || backend.selectedEnvironment.snapshots.length === 0) && (!backend.selectedEnvironment.orphanWorkspaceSnapshots || backend.selectedEnvironment.orphanWorkspaceSnapshots.length === 0)
                                        text: qsTr("No snapshots saved for this environment yet.")
                                        color: theme.muted
                                        font.family: win.sansFont
                                        font.pixelSize: 12
                                    }

                                    ListView {
                                        id: snapListView
                                        Layout.fillWidth: true
                                        Layout.fillHeight: true
                                        clip: true
                                        spacing: 6
                                        model: backend.selectedEnvironment.snapshots || []

                                        delegate: Item {
                                            required property string modelData
                                            readonly property bool hasWsHalf: (backend.selectedEnvironment.workspaceSnapshots || []).indexOf(modelData) >= 0
                                            width: snapListView.width
                                            height: 36

                                            Rectangle {
                                                anchors.fill: parent
                                                radius: Math.min(3, theme.radius)
                                                color: win.surfaceCard
                                                border.width: 1
                                                border.color: win.borderSubtle

                                                RowLayout {
                                                    anchors.fill: parent
                                                    anchors.leftMargin: 12
                                                    anchors.rightMargin: 10
                                                    spacing: 10

                                                    Text {
                                                        text: "●"
                                                        color: theme.accent
                                                        font.pixelSize: 10
                                                    }

                                                    Rectangle {
                                                        visible: modelData.startsWith("auto-")
                                                        height: 18
                                                        width: autoTagText.implicitWidth + 10
                                                        radius: 2
                                                        color: win.surfaceInput
                                                        border.width: 1
                                                        border.color: win.borderSubtle
                                                        Text {
                                                            id: autoTagText
                                                            anchors.centerIn: parent
                                                            text: qsTr("AUTO")
                                                            color: theme.muted
                                                            font.family: win.monoFont
                                                            font.pixelSize: 9
                                                        }
                                                    }

                                                    Rectangle {
                                                        visible: hasWsHalf
                                                        height: 18
                                                        width: wsTagText.implicitWidth + 10
                                                        radius: 2
                                                        color: win.surfaceInput
                                                        border.width: 1
                                                        border.color: theme.accent
                                                        Text {
                                                            id: wsTagText
                                                            anchors.centerIn: parent
                                                            text: qsTr("W")
                                                            color: theme.accent
                                                            font.family: win.monoFont
                                                            font.pixelSize: 9
                                                            font.weight: Font.Bold
                                                        }
                                                    }

                                                    Text {
                                                        Layout.fillWidth: true
                                                        text: modelData
                                                        color: theme.foreground
                                                        font.family: win.monoFont
                                                        font.pixelSize: 12
                                                    }

                                                    Text {
                                                        readonly property string snapSize: (backend.selectedEnvironment.snapshotSizes
                                                                                            && backend.selectedEnvironment.snapshotSizes[modelData])
                                                                                           ? backend.selectedEnvironment.snapshotSizes[modelData]
                                                                                           : ""
                                                        readonly property string wsSize: (backend.selectedEnvironment.workspaceSnapshotSizes
                                                                                          && backend.selectedEnvironment.workspaceSnapshotSizes[modelData])
                                                                                         ? backend.selectedEnvironment.workspaceSnapshotSizes[modelData]
                                                                                         : ""
                                                        readonly property string combinedSize: {
                                                            if (snapSize.length > 0 && wsSize.length > 0)
                                                                return snapSize + " · W: " + wsSize
                                                            if (snapSize.length > 0)
                                                                return snapSize
                                                            if (wsSize.length > 0)
                                                                return "W: " + wsSize
                                                            return ""
                                                        }
                                                        visible: combinedSize.length > 0
                                                        text: combinedSize
                                                        color: theme.muted
                                                        font.family: win.monoFont
                                                        font.pixelSize: 11
                                                    }

                                                    ActionButton {
                                                        implicitHeight: 24
                                                        visible: hasWsHalf
                                                        label: qsTr("Changes")
                                                        onClicked: win.openChangeReviewDialog(modelData)
                                                    }

                                                    ActionButton {
                                                        implicitHeight: 24
                                                        label: qsTr("Restore")
                                                        disabled: backend.busy
                                                        onClicked: win.openRestoreDialog(modelData, true, hasWsHalf)
                                                    }

                                                    ActionButton {
                                                        implicitHeight: 24
                                                        label: qsTr("Delete")
                                                        danger: true
                                                        disabled: backend.busy
                                                        onClicked: backend.deleteSnapshot(backend.selectedEnvironment.name || "", modelData)
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // Workspace-only snapshots (e.g. pre-restore safety snapshots or orphans)
                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: 4
                                        visible: (backend.selectedEnvironment.orphanWorkspaceSnapshots || []).length > 0

                                        Text {
                                            text: qsTr("Workspace-only snapshots")
                                            color: theme.muted
                                            font.family: win.sansFont
                                            font.pixelSize: 11
                                        }

                                        Repeater {
                                            model: backend.selectedEnvironment.orphanWorkspaceSnapshots || []
                                            delegate: Rectangle {
                                                required property string modelData
                                                Layout.fillWidth: true
                                                height: 32
                                                radius: Math.min(3, theme.radius)
                                                color: win.surfaceCard
                                                border.width: 1
                                                border.color: win.borderSubtle

                                                RowLayout {
                                                    anchors.fill: parent
                                                    anchors.leftMargin: 12
                                                    anchors.rightMargin: 10
                                                    spacing: 10

                                                    Rectangle {
                                                        height: 18
                                                        width: wsOnlyTag.implicitWidth + 10
                                                        radius: 2
                                                        color: win.surfaceInput
                                                        border.width: 1
                                                        border.color: win.borderSubtle
                                                        Text {
                                                            id: wsOnlyTag
                                                            anchors.centerIn: parent
                                                            text: qsTr("WS ONLY")
                                                            color: theme.muted
                                                            font.family: win.monoFont
                                                            font.pixelSize: 9
                                                        }
                                                    }

                                                    Text {
                                                        Layout.fillWidth: true
                                                        text: modelData
                                                        color: theme.foreground
                                                        font.family: win.monoFont
                                                        font.pixelSize: 11
                                                    }

                                                    Text {
                                                        readonly property string wsOnlySize: (backend.selectedEnvironment.workspaceSnapshotSizes
                                                                                              && backend.selectedEnvironment.workspaceSnapshotSizes[modelData])
                                                                                             ? backend.selectedEnvironment.workspaceSnapshotSizes[modelData]
                                                                                             : ""
                                                        visible: wsOnlySize.length > 0
                                                        text: wsOnlySize
                                                        color: theme.muted
                                                        font.family: win.monoFont
                                                        font.pixelSize: 11
                                                    }

                                                    ActionButton {
                                                        implicitHeight: 24
                                                        label: qsTr("Changes")
                                                        onClicked: win.openChangeReviewDialog(modelData)
                                                    }

                                                    ActionButton {
                                                        implicitHeight: 24
                                                        label: qsTr("Restore")
                                                        disabled: backend.busy
                                                        onClicked: win.openRestoreDialog(modelData, false, true)
                                                    }

                                                    ActionButton {
                                                        implicitHeight: 24
                                                        label: qsTr("Delete")
                                                        danger: true
                                                        disabled: backend.busy
                                                        onClicked: backend.deleteSnapshot(backend.selectedEnvironment.name || "", modelData)
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // HEADLESS GUI TEST RUNNER
                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 54
                                radius: Math.min(4, theme.radius)
                                color: win.surfaceInput
                                border.width: 1
                                border.color: win.borderSubtle

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 12
                                    anchors.rightMargin: 12
                                    spacing: 10

                                    Text {
                                        text: qsTr("Virtual Display:")
                                        color: theme.muted
                                        font.family: win.sansFont
                                        font.pixelSize: 12
                                    }

                                    FlatInput {
                                        id: headlessInput
                                        Layout.fillWidth: true
                                        implicitHeight: 34
                                        placeholder: qsTr("Command for 1920x1080 Xvfb screen (e.g. flutter test, npm test, pytest)")
                                        onAccepted: {
                                            if (!backend.busy && headlessInput.text.length > 0)
                                                backend.runHeadlessCommand(backend.selectedEnvironment.name || "", headlessInput.text)
                                        }
                                    }

                                    ActionButton {
                                        implicitHeight: 34
                                        minWidth: 120
                                        label: qsTr("Run Headless")
                                        disabled: backend.busy
                                        onClicked: {
                                            if (headlessInput.text.length > 0)
                                                backend.runHeadlessCommand(backend.selectedEnvironment.name || "", headlessInput.text)
                                        }
                                    }
                                }
                            }
                        }
                        }
                    }
                }

                // --------------------------------------------------------------
                // TAB 1: OS IMAGES & ONE-TAP DOWNLOADS
                // --------------------------------------------------------------
                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 20
                    spacing: 14
                    visible: win.activeTab === 1

                    RowLayout {
                        Layout.fillWidth: true
                        ColumnLayout {
                            spacing: 2
                            Text {
                                text: qsTr("Linux OS Image Catalog")
                                color: theme.foreground
                                font.family: win.sansFont
                                font.pixelSize: 16
                                font.weight: Font.Medium
                            }
                            Text {
                                text: qsTr("Pre-download OS images to local storage so new environments launch in under 1 second, or click 'Use This OS' to start immediately.")
                                color: theme.muted
                                font.family: win.sansFont
                                font.pixelSize: 12
                            }
                        }
                    }

                    ListView {
                        id: osListView
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        clip: true
                        spacing: 8
                        model: backend.osCatalog

                        delegate: Item {
                            required property var modelData
                            width: osListView.width
                            height: 74

                            Rectangle {
                                anchors.fill: parent
                                radius: Math.min(4, theme.radius)
                                color: win.surfaceCard
                                border.width: 1
                                border.color: modelData.downloaded ? "#284d3a" : win.borderSubtle

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 16
                                    anchors.rightMargin: 16
                                    spacing: 14

                                    Rectangle {
                                        width: 36
                                        height: 36
                                        radius: Math.min(4, theme.radius)
                                        color: modelData.downloaded ? "#162920" : win.surfaceInput
                                        border.width: 1
                                        border.color: modelData.downloaded ? "#284d3a" : win.borderSubtle
                                        Text {
                                            anchors.centerIn: parent
                                            text: modelData.downloaded ? "✓" : "↓"
                                            color: modelData.downloaded ? theme.green : theme.accent
                                            font.family: win.monoFont
                                            font.pixelSize: 14
                                        }
                                    }

                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: 3

                                        RowLayout {
                                            spacing: 8
                                            Text {
                                                text: modelData.name
                                                color: theme.foreground
                                                font.family: win.sansFont
                                                font.pixelSize: 14
                                                font.weight: Font.Medium
                                            }
                                            Rectangle {
                                                height: 18
                                                width: osBadgeText.implicitWidth + 10
                                                radius: 2
                                                color: win.surfaceInput
                                                border.width: 1
                                                border.color: win.borderSubtle
                                                Text {
                                                    id: osBadgeText
                                                    anchors.centerIn: parent
                                                    text: modelData.badge
                                                    color: theme.accent
                                                    font.family: win.monoFont
                                                    font.pixelSize: 10
                                                }
                                            }
                                            Rectangle {
                                                height: 18
                                                width: cacheStateText.implicitWidth + 10
                                                radius: 2
                                                color: modelData.downloaded ? "#162920" : win.surfaceInput
                                                border.width: 1
                                                border.color: modelData.downloaded ? "#284d3a" : win.borderSubtle
                                                Text {
                                                    id: cacheStateText
                                                    anchors.centerIn: parent
                                                    text: modelData.downloading ? qsTr("Downloading...") : modelData.sizeText
                                                    color: modelData.downloaded ? theme.green : theme.muted
                                                    font.family: win.monoFont
                                                    font.pixelSize: 10
                                                }
                                            }
                                        }

                                        Text {
                                            Layout.fillWidth: true
                                            elide: Text.ElideRight
                                            text: modelData.description
                                            color: theme.muted
                                            font.family: win.sansFont
                                            font.pixelSize: 12
                                        }
                                    }

                                    ActionButton {
                                        label: modelData.downloading
                                               ? qsTr("Downloading...")
                                               : (modelData.downloaded ? qsTr("Remove Cache") : qsTr("Download OS"))
                                        variant: modelData.downloaded ? "danger" : ""
                                        minWidth: 135
                                        disabled: backend.busy
                                        onClicked: {
                                            if (modelData.downloaded)
                                                backend.removeOsImage(modelData.id)
                                            else
                                                backend.downloadOsImage(modelData.id)
                                        }
                                    }

                                    ActionButton {
                                        label: qsTr("Use This OS")
                                        variant: "primary"
                                        minWidth: 125
                                        disabled: backend.busy
                                        onClicked: win.openCreateDialog(modelData.id)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ======================================================================
        // MODAL 1: CREATE NEW ENVIRONMENT DIALOG (WITH LIVE PROGRESS & CANCEL)
        // ======================================================================
        Rectangle {
            anchors.fill: parent
            visible: win.createVisible
            color: "#c406070a"

            MouseArea {
                anchors.fill: parent
                onClicked: {
                    if (!backend.busy)
                        win.createVisible = false
                }
            }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(parent.width - 60, 680)
                // The dialog follows its content, so the form stays compact in
                // template mode instead of leaving a large empty area.
                height: Math.min(parent.height - 40, createCol.implicitHeight + 44)
                radius: Math.min(8, theme.radius)
                color: win.surfaceCard
                border.width: 1
                border.color: theme.accent

                MouseArea { anchors.fill: parent }

                ColumnLayout {
                    id: createCol
                    anchors.fill: parent
                    anchors.margins: 22
                    spacing: 12

                    ModalHeader {
                        title: backend.busy
                               ? qsTr("Creating Environment '%1'...").arg(backend.pendingEnvName.length > 0 ? backend.pendingEnvName : win.newEnvName)
                               : qsTr("Create Isolated Linux Environment")
                        subtitle: backend.busy
                                  ? qsTr("Runs in background — safe to close window or press Esc to minimize.")
                                  : qsTr("Configure OS distribution, hardware limits, and shared folder.")
                        onClosed: win.createVisible = false
                    }

                    // Error box inside modal if validation fails
                    Rectangle {
                        visible: backend.errorMessage.length > 0 && !backend.busy
                        Layout.fillWidth: true
                        height: 36
                        radius: Math.min(4, theme.radius)
                        color: "#261319"
                        border.width: 1
                        border.color: theme.red

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 12
                            Text {
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                                text: backend.errorMessage
                                color: theme.red
                                font.family: win.sansFont
                                font.pixelSize: 12
                            }
                        }
                    }

                    // ----------------------------------------------------------
                    // VIEW A: CREATION FORM (SHOWN WHEN NOT BUSY)
                    // ----------------------------------------------------------
                    ScrollView {
                        id: createFormScroll
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        Layout.preferredHeight: createFormContent.implicitHeight
                        Layout.minimumHeight: 140
                        visible: !backend.busy
                        clip: true
                        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

                        ColumnLayout {
                            id: createFormContent
                            width: createFormScroll.availableWidth
                            spacing: 12

                        // Project Name Field (Clean FlatInput, NO floating label!)
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 6

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10
                                Text {
                                    text: qsTr("Project / Environment Name")
                                    color: theme.foreground
                                    font.family: win.sansFont
                                    font.pixelSize: 12
                                    font.weight: Font.Medium
                                }
                                Text {
                                    Layout.fillWidth: true
                                    Layout.minimumWidth: 0
                                    horizontalAlignment: Text.AlignRight
                                    text: qsTr("Shared at: %1/%2")
                                              .arg(win.newEnvSharedPath.trim().length > 0 ? win.newEnvSharedPath.trim() : backend.workspaceBase)
                                              .arg(win.newEnvName.length > 0 ? win.newEnvName : "<name>")
                                    color: theme.muted
                                    font.family: win.monoFont
                                    font.pixelSize: 11
                                    elide: Text.ElideMiddle
                                }
                            }

                            FlatInput {
                                id: createNameField
                                Layout.fillWidth: true
                                placeholder: qsTr("e.g. pos-desktop, flutter-mobile, web-api")
                                text: win.newEnvName
                                onTextChanged: win.newEnvName = text
                            }
                        }

                        // Optional custom shared folder location (defaults to Settings base)
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 6

                            Text {
                                text: qsTr("Shared Folder Location (optional)")
                                color: theme.foreground
                                font.family: win.sansFont
                                font.pixelSize: 12
                                font.weight: Font.Medium
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 8

                                FlatInput {
                                    id: createSharedPathField
                                    Layout.fillWidth: true
                                    placeholder: qsTr("Default: %1").arg(backend.workspaceBase)
                                    text: win.newEnvSharedPath
                                    onTextChanged: win.newEnvSharedPath = text
                                }

                                ActionButton {
                                    label: qsTr("Browse...")
                                    minWidth: 94
                                    onClicked: createSharedFolderDialog.open()
                                }

                                ActionButton {
                                    label: qsTr("Default")
                                    minWidth: 84
                                    onClicked: {
                                        win.newEnvSharedPath = ""
                                        createSharedPathField.text = ""
                                    }
                                }
                            }
                        }

                        // Source mode switcher (shown when templates exist)
                        RowLayout {
                            visible: backend.templates.length > 0
                            Layout.fillWidth: true
                            spacing: 8

                            Repeater {
                                model: [
                                    { fromTpl: false, label: qsTr("Fresh OS") },
                                    { fromTpl: true, label: qsTr("From Template (%1)").arg(backend.templates.length) }
                                ]
                                delegate: Rectangle {
                                    required property var modelData
                                    readonly property bool selected: win.createFromTemplate === modelData.fromTpl
                                    Layout.fillWidth: true
                                    height: 34
                                    radius: Math.min(4, theme.radius)
                                    color: selected ? win.surfaceSelected : win.surfaceInput
                                    border.width: 1
                                    border.color: selected ? theme.accent : win.borderSubtle
                                    Text {
                                        anchors.centerIn: parent
                                        text: modelData.label
                                        color: selected ? theme.foreground : theme.muted
                                        font.family: win.sansFont
                                        font.pixelSize: 12
                                        font.weight: selected ? Font.Medium : Font.Normal
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: {
                                            win.createFromTemplate = modelData.fromTpl
                                            if (modelData.fromTpl && win.newEnvTemplateName.length === 0 && backend.templates.length > 0)
                                                win.newEnvTemplateName = backend.templates[0].name || ""
                                        }
                                    }
                                }
                            }
                        }

                        // Template Picker (when From Template is selected)
                        ColumnLayout {
                            visible: win.createFromTemplate && backend.templates.length > 0
                            Layout.fillWidth: true
                            spacing: 6
                            Text {
                                text: qsTr("Select Template")
                                color: theme.foreground
                                font.family: win.sansFont
                                font.pixelSize: 12
                                font.weight: Font.Medium
                            }
                            GridLayout {
                                Layout.fillWidth: true
                                columns: 2
                                columnSpacing: 8
                                rowSpacing: 8
                                Repeater {
                                    model: backend.templates
                                    delegate: Rectangle {
                                        required property var modelData
                                        readonly property bool selected: win.newEnvTemplateName === modelData.name
                                        Layout.fillWidth: true
                                        height: 50
                                        radius: Math.min(4, theme.radius)
                                        color: selected ? win.surfaceSelected : win.surfaceInput
                                        border.width: 1
                                        border.color: selected ? theme.accent : win.borderSubtle

                                        ColumnLayout {
                                            anchors.fill: parent
                                            anchors.leftMargin: 12
                                            anchors.rightMargin: 12
                                            anchors.topMargin: 7
                                            anchors.bottomMargin: 7
                                            spacing: 2

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 6
                                                Rectangle {
                                                    width: 6
                                                    height: 6
                                                    radius: 3
                                                    color: parent.parent.parent.selected ? theme.accent : theme.muted
                                                }
                                                Text {
                                                    Layout.fillWidth: true
                                                    elide: Text.ElideRight
                                                    text: modelData.name
                                                    color: parent.parent.parent.selected ? theme.foreground : theme.muted
                                                    font.family: win.monoFont
                                                    font.pixelSize: 12
                                                }
                                            }
                                            Text {
                                                Layout.fillWidth: true
                                                elide: Text.ElideRight
                                                text: modelData.osName + " · " + modelData.diskLimit + " · " + modelData.memoryLimit
                                                color: theme.muted
                                                font.family: win.sansFont
                                                font.pixelSize: 10
                                            }
                                        }

                                        MouseArea {
                                            anchors.fill: parent
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: win.newEnvTemplateName = modelData.name
                                        }
                                    }
                                }
                            }
                        }

                        // 1. Capability Preset
                        ColumnLayout {
                            visible: !win.createFromTemplate
                            Layout.fillWidth: true
                            spacing: 6
                            Text {
                                text: qsTr("Environment Capability Preset")
                                color: theme.foreground
                                font.family: win.sansFont
                                font.pixelSize: 12
                                font.weight: Font.Medium
                            }
                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 8
                                Repeater {
                                    model: backend.presets
                                    delegate: Rectangle {
                                        required property var modelData
                                        readonly property bool selected: win.newEnvPreset === modelData.id
                                        Layout.fillWidth: true
                                        height: 50
                                        radius: Math.min(4, theme.radius)
                                        color: selected ? win.surfaceSelected : win.surfaceInput
                                        border.width: 1
                                        border.color: selected ? theme.accent : win.borderSubtle

                                        ColumnLayout {
                                            anchors.fill: parent
                                            anchors.leftMargin: 12
                                            anchors.rightMargin: 12
                                            anchors.topMargin: 7
                                            anchors.bottomMargin: 7
                                            spacing: 2

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 6
                                                Rectangle {
                                                    width: 6
                                                    height: 6
                                                    radius: 3
                                                    color: parent.parent.parent.selected ? theme.accent : theme.muted
                                                }
                                                Text {
                                                    Layout.fillWidth: true
                                                    elide: Text.ElideRight
                                                    text: modelData.title
                                                    color: parent.parent.parent.selected ? theme.foreground : theme.muted
                                                    font.family: win.sansFont
                                                    font.pixelSize: 12
                                                    font.weight: parent.parent.parent.selected ? Font.Medium : Font.Normal
                                                }
                                            }
                                            Text {
                                                Layout.fillWidth: true
                                                elide: Text.ElideRight
                                                text: modelData.sub
                                                color: theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 10
                                            }
                                        }
                                        MouseArea {
                                            anchors.fill: parent
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: win.newEnvPreset = modelData.id
                                        }
                                    }
                                }
                            }
                        }

                        // 2. Operating System (Distro)
                        ColumnLayout {
                            visible: !win.createFromTemplate
                            Layout.fillWidth: true
                            spacing: 6
                            Text {
                                text: qsTr("Linux Distribution")
                                color: theme.foreground
                                font.family: win.sansFont
                                font.pixelSize: 12
                                font.weight: Font.Medium
                            }
                            GridLayout {
                                Layout.fillWidth: true
                                columns: 4
                                columnSpacing: 8
                                rowSpacing: 8
                                Repeater {
                                    model: backend.osCatalog
                                    delegate: Rectangle {
                                        required property var modelData
                                        readonly property bool selected: win.newEnvOsId === modelData.id
                                        Layout.fillWidth: true
                                        height: 46
                                        radius: Math.min(4, theme.radius)
                                        color: selected ? win.surfaceSelected : win.surfaceInput
                                        border.width: 1
                                        border.color: selected ? theme.accent : win.borderSubtle

                                        ColumnLayout {
                                            anchors.centerIn: parent
                                            spacing: 2
                                            Text {
                                                Layout.alignment: Qt.AlignHCenter
                                                text: modelData.id
                                                color: selected ? theme.foreground : theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 12
                                            }
                                            Text {
                                                Layout.alignment: Qt.AlignHCenter
                                                text: modelData.downloaded ? qsTr("✓ Cached") : modelData.badge
                                                color: modelData.downloaded ? theme.green : theme.muted
                                                font.family: win.sansFont
                                                font.pixelSize: 10
                                            }
                                        }
                                        MouseArea {
                                            anchors.fill: parent
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: win.newEnvOsId = modelData.id
                                        }
                                    }
                                }
                            }
                        }

                        // 3. Disk, RAM, and CPU Quotas
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 16

                            // Disk Size
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 6
                                Text {
                                    text: qsTr("Disk Quota")
                                    color: theme.foreground
                                    font.family: win.sansFont
                                    font.pixelSize: 12
                                    font.weight: Font.Medium
                                }
                                RowLayout {
                                    spacing: 6
                                    Repeater {
                                        model: backend.diskOptions
                                        delegate: Rectangle {
                                            required property string modelData
                                            readonly property bool selected: win.newEnvDisk === modelData
                                            Layout.fillWidth: true
                                            Layout.preferredWidth: 0
                                            height: 32
                                            radius: Math.min(4, theme.radius)
                                            color: selected ? win.surfaceSelected : win.surfaceInput
                                            border.width: 1
                                            border.color: selected ? theme.accent : win.borderSubtle
                                            Text {
                                                anchors.centerIn: parent
                                                width: parent.width - 4
                                                horizontalAlignment: Text.AlignHCenter
                                                elide: Text.ElideRight
                                                text: modelData
                                                color: selected ? theme.accent : theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 11
                                            }
                                            MouseArea {
                                                anchors.fill: parent
                                                cursorShape: Qt.PointingHandCursor
                                                onClicked: win.newEnvDisk = modelData
                                            }
                                        }
                                    }
                                }
                            }

                            // RAM Limit
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 6
                                Text {
                                    text: qsTr("RAM Limit")
                                    color: theme.foreground
                                    font.family: win.sansFont
                                    font.pixelSize: 12
                                    font.weight: Font.Medium
                                }
                                RowLayout {
                                    spacing: 6
                                    Repeater {
                                        model: backend.ramOptions
                                        delegate: Rectangle {
                                            required property string modelData
                                            readonly property bool selected: win.newEnvRam === modelData
                                            Layout.fillWidth: true
                                            Layout.preferredWidth: 0
                                            height: 32
                                            radius: Math.min(4, theme.radius)
                                            color: selected ? win.surfaceSelected : win.surfaceInput
                                            border.width: 1
                                            border.color: selected ? theme.accent : win.borderSubtle
                                            Text {
                                                anchors.centerIn: parent
                                                width: parent.width - 4
                                                horizontalAlignment: Text.AlignHCenter
                                                elide: Text.ElideRight
                                                text: modelData
                                                color: selected ? theme.accent : theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 11
                                            }
                                            MouseArea {
                                                anchors.fill: parent
                                                cursorShape: Qt.PointingHandCursor
                                                onClicked: win.newEnvRam = modelData
                                            }
                                        }
                                    }
                                }
                            }

                            // CPU Cores
                            ColumnLayout {
                                spacing: 6
                                Text {
                                    text: qsTr("CPU Cores")
                                    color: theme.foreground
                                    font.family: win.sansFont
                                    font.pixelSize: 12
                                    font.weight: Font.Medium
                                }
                                RowLayout {
                                    spacing: 6
                                    Repeater {
                                        model: backend.cpuOptions
                                        delegate: Rectangle {
                                            required property int modelData
                                            readonly property bool selected: win.newEnvCpu === modelData
                                            Layout.fillWidth: true
                                            Layout.preferredWidth: 0
                                            height: 32
                                            radius: Math.min(4, theme.radius)
                                            color: selected ? win.surfaceSelected : win.surfaceInput
                                            border.width: 1
                                            border.color: selected ? theme.accent : win.borderSubtle
                                            Text {
                                                anchors.centerIn: parent
                                                width: parent.width - 4
                                                horizontalAlignment: Text.AlignHCenter
                                                elide: Text.ElideRight
                                                text: modelData
                                                color: selected ? theme.accent : theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 11
                                            }
                                            MouseArea {
                                                anchors.fill: parent
                                                cursorShape: Qt.PointingHandCursor
                                                onClicked: win.newEnvCpu = modelData
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // 4. Runaway Limits (Process Limit & Disk I/O Rate)
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 16

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 6
                                Text {
                                    text: qsTr("Process Limit (Container)")
                                    color: theme.foreground
                                    font.family: win.sansFont
                                    font.pixelSize: 12
                                    font.weight: Font.Medium
                                }
                                RowLayout {
                                    spacing: 6
                                    Repeater {
                                        model: backend.processLimitOptions
                                        delegate: Rectangle {
                                            required property string modelData
                                            readonly property bool selected: win.newEnvProcLimit === modelData
                                            Layout.fillWidth: true
                                            Layout.preferredWidth: 0
                                            height: 30
                                            radius: Math.min(4, theme.radius)
                                            color: selected ? win.surfaceSelected : win.surfaceInput
                                            border.width: 1
                                            border.color: selected ? theme.accent : win.borderSubtle
                                            Text {
                                                id: procChipTxt
                                                anchors.centerIn: parent
                                                width: parent.width - 4
                                                horizontalAlignment: Text.AlignHCenter
                                                elide: Text.ElideRight
                                                text: modelData
                                                color: selected ? theme.accent : theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 11
                                            }
                                            MouseArea {
                                                anchors.fill: parent
                                                cursorShape: Qt.PointingHandCursor
                                                onClicked: win.newEnvProcLimit = modelData
                                            }
                                        }
                                    }
                                }
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 6
                                Text {
                                    text: qsTr("Disk I/O Limit (Read / Write)")
                                    color: theme.foreground
                                    font.family: win.sansFont
                                    font.pixelSize: 12
                                    font.weight: Font.Medium
                                }
                                RowLayout {
                                    spacing: 6
                                    Repeater {
                                        model: backend.ioLimitOptions
                                        delegate: Rectangle {
                                            required property string modelData
                                            readonly property bool selected: win.newEnvIoLimit === modelData
                                            Layout.fillWidth: true
                                            Layout.preferredWidth: 0
                                            height: 30
                                            radius: Math.min(4, theme.radius)
                                            color: selected ? win.surfaceSelected : win.surfaceInput
                                            border.width: 1
                                            border.color: selected ? theme.accent : win.borderSubtle
                                            Text {
                                                id: ioChipTxt
                                                anchors.centerIn: parent
                                                width: parent.width - 4
                                                horizontalAlignment: Text.AlignHCenter
                                                elide: Text.ElideRight
                                                text: modelData
                                                color: selected ? theme.accent : theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 11
                                            }
                                            MouseArea {
                                                anchors.fill: parent
                                                cursorShape: Qt.PointingHandCursor
                                                onClicked: win.newEnvIoLimit = modelData
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // 5. Provisioning Presets (Ubuntu / Debian / Arch)
                        ColumnLayout {
                            visible: !win.createFromTemplate
                                     && (win.newEnvOsId.indexOf("ubuntu") >= 0
                                         || win.newEnvOsId.indexOf("debian") >= 0
                                         || win.newEnvOsId.indexOf("arch") >= 0)
                            Layout.fillWidth: true
                            spacing: 6

                            RowLayout {
                                Layout.fillWidth: true
                                Text {
                                    text: qsTr("Provisioning Presets (Optional)")
                                    color: theme.foreground
                                    font.family: win.sansFont
                                    font.pixelSize: 12
                                    font.weight: Font.Medium
                                }
                                Item { Layout.fillWidth: true }
                                Text {
                                    text: qsTr("Runs via incus exec after boot (non-fatal)")
                                    color: theme.muted
                                    font.family: win.sansFont
                                    font.pixelSize: 10
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 8
                                Repeater {
                                    model: backend.provisionBundles
                                    delegate: Rectangle {
                                        required property var modelData
                                        readonly property bool selected: win.newEnvProvisionBundles.indexOf(modelData.id) >= 0
                                        Layout.fillWidth: true
                                        height: 32
                                        radius: Math.min(4, theme.radius)
                                        color: selected ? win.surfaceSelected : win.surfaceInput
                                        border.width: 1
                                        border.color: selected ? theme.accent : win.borderSubtle
                                        Text {
                                            anchors.centerIn: parent
                                            text: (selected ? "✓ " : "") + modelData.title
                                            color: selected ? theme.foreground : theme.muted
                                            font.family: win.sansFont
                                            font.pixelSize: 11
                                            font.weight: selected ? Font.Medium : Font.Normal
                                        }
                                        MouseArea {
                                            anchors.fill: parent
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: win.toggleProvisionBundle(modelData.id)
                                        }
                                    }
                                }
                            }
                        }

                        // Storage pool overcommit warning (non-blocking)
                        Rectangle {
                            readonly property string warnMsg: win.createVisible ? backend.poolWarningForCreate(win.newEnvDisk) : ""
                            visible: warnMsg.length > 0
                            Layout.fillWidth: true
                            implicitHeight: createPoolWarnTxt.implicitHeight + 16
                            radius: Math.min(4, theme.radius)
                            color: "#262013"
                            border.width: 1
                            border.color: "#e5a93c"
                            Text {
                                id: createPoolWarnTxt
                                anchors.fill: parent
                                anchors.margins: 8
                                anchors.leftMargin: 12
                                anchors.rightMargin: 12
                                wrapMode: Text.WordWrap
                                text: parent.warnMsg
                                color: "#e5a93c"
                                font.family: win.sansFont
                                font.pixelSize: 11
                            }
                        }
                        }
                        }

                        // Dialog Footer Buttons
                        RowLayout {
                            Layout.fillWidth: true
                            Layout.topMargin: 4
                            visible: !backend.busy
                            spacing: 10
                            Item { Layout.fillWidth: true }

                            ActionButton {
                                label: qsTr("Cancel")
                                minWidth: 96
                                onClicked: win.createVisible = false
                            }

                            ActionButton {
                                label: win.createFromTemplate ? qsTr("Create from Template") : qsTr("Create Environment")
                                variant: "primary"
                                minWidth: 165
                                disabled: backend.busy || win.newEnvName.trim().length === 0 || (win.createFromTemplate && win.newEnvTemplateName.length === 0)
                                onClicked: {
                                    if (win.createFromTemplate) {
                                        backend.cloneEnvironment(win.newEnvTemplateName, win.newEnvName,
                                                                 win.newEnvDisk, win.newEnvRam, win.newEnvCpu, true,
                                                                 win.newEnvProcLimit, win.newEnvIoLimit,
                                                                 win.newEnvSharedPath)
                                    } else {
                                        backend.createEnvironment(win.newEnvName, win.newEnvOsId, win.newEnvPreset,
                                                                  win.newEnvDisk, win.newEnvRam, win.newEnvCpu,
                                                                  win.newEnvProcLimit, win.newEnvIoLimit,
                                                                  win.newEnvProvisionBundles, win.newEnvSharedPath)
                                    }
                                }
                            }
                        }

                    // ----------------------------------------------------------
                    // VIEW B: LIVE PROGRESS & CANCELLATION PANEL (WHEN BUSY)
                    // ----------------------------------------------------------
                    ColumnLayout {
                        Layout.fillWidth: true
                        visible: backend.busy
                        spacing: 14

                        RowLayout {
                            Layout.fillWidth: true
                            Text {
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                                text: backend.taskStage
                                color: theme.foreground
                                font.family: win.sansFont
                                font.pixelSize: 13
                            }
                            Text {
                                text: backend.taskProgress + "%"
                                color: theme.accent
                                font.family: win.monoFont
                                font.pixelSize: 13
                            }
                        }

                        // Smooth Progress Bar
                        Rectangle {
                            Layout.fillWidth: true
                            height: 8
                            radius: 4
                            color: win.surfaceInput
                            border.width: 1
                            border.color: win.borderSubtle

                            Rectangle {
                                height: parent.height
                                width: parent.width * (Math.max(4, backend.taskProgress) / 100.0)
                                radius: 4
                                color: theme.accent
                                Behavior on width { NumberAnimation { duration: 150 } }
                            }
                        }

                        // Live Terminal Log Output Box
                        Rectangle {
                            Layout.fillWidth: true
                            height: 44
                            radius: Math.min(4, theme.radius)
                            color: win.surfaceInput
                            border.width: 1
                            border.color: win.borderSubtle

                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 12
                                anchors.rightMargin: 12
                                spacing: 8
                                Text {
                                    text: "$"
                                    color: theme.accent
                                    font.family: win.monoFont
                                    font.pixelSize: 11
                                }
                                Text {
                                    Layout.fillWidth: true
                                    elide: Text.ElideLeft
                                    text: backend.taskLog
                                    color: theme.muted
                                    font.family: win.monoFont
                                    font.pixelSize: 11
                                }
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            Text {
                                text: qsTr("Creation is locked while an operation is running.")
                                color: theme.muted
                                font.family: win.sansFont
                                font.pixelSize: 11
                            }
                            Item { Layout.fillWidth: true }
                            ActionButton {
                                label: qsTr("Cancel Creation")
                                variant: "danger"
                                minWidth: 140
                                onClicked: backend.cancelTask()
                            }
                        }
                    }
                }
            }
        }

        // ======================================================================
        // MODAL 1B: CLONE ENVIRONMENT DIALOG
        // ======================================================================
        Rectangle {
            anchors.fill: parent
            visible: win.cloneVisible
            color: "#c406070a"

            MouseArea {
                anchors.fill: parent
                onClicked: {
                    if (!backend.busy)
                        win.cloneVisible = false
                }
            }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(parent.width - 60, 520)
                height: cloneCol.implicitHeight + 48
                radius: Math.min(8, theme.radius)
                color: win.surfaceCard
                border.width: 1
                border.color: theme.accent

                MouseArea { anchors.fill: parent }

                ColumnLayout {
                    id: cloneCol
                    anchors.fill: parent
                    anchors.margins: 24
                    spacing: 14

                    ModalHeader {
                        title: qsTr("Clone Environment '%1'").arg(win.cloneSourceName)
                        subtitle: qsTr("Duplicates the OS container with Btrfs copy-on-write and creates a separate shared workspace folder.")
                        onClosed: win.cloneVisible = false
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 6

                        Text {
                            Layout.fillWidth: true
                            text: qsTr("New Environment Name")
                            color: theme.foreground
                            font.family: win.sansFont
                            font.pixelSize: 12
                            font.weight: Font.Medium
                        }

                        FlatInput {
                            id: cloneNameField
                            Layout.fillWidth: true
                            placeholder: qsTr("e.g. agent-sandbox-2")
                            text: win.cloneNewName
                            onTextChanged: win.cloneNewName = text
                            onAccepted: {
                                if (!backend.busy && win.cloneNewName.trim().length > 0) {
                                    backend.cloneEnvironment(win.cloneSourceName, win.cloneNewName)
                                    win.cloneVisible = false
                                }
                            }
                        }

                        Text {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            text: qsTr("Shared at: %1/%2").arg(backend.workspaceBase).arg(win.cloneNewName.length > 0 ? win.cloneNewName : "<name>")
                            color: theme.muted
                            font.family: win.monoFont
                            font.pixelSize: 10
                            elide: Text.ElideMiddle
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.topMargin: 4
                        spacing: 10
                        Item { Layout.fillWidth: true }

                        ActionButton {
                            label: qsTr("Cancel")
                            minWidth: 96
                            onClicked: win.cloneVisible = false
                        }

                        ActionButton {
                            label: qsTr("Clone Environment")
                            variant: "primary"
                            minWidth: 165
                            disabled: backend.busy || win.cloneNewName.trim().length === 0
                            onClicked: {
                                backend.cloneEnvironment(win.cloneSourceName, win.cloneNewName)
                                win.cloneVisible = false
                            }
                        }
                    }
                }
            }
        }

        // ======================================================================
        // MODAL 2: WIPE / DELETE CONFIRMATION DIALOG
        // ======================================================================
        Rectangle {
            anchors.fill: parent
            visible: win.wipeVisible
            color: "#c406070a"

            MouseArea {
                anchors.fill: parent
                onClicked: win.wipeVisible = false
            }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(parent.width - 60, 500)
                height: wipeCol.implicitHeight + 48
                radius: Math.min(8, theme.radius)
                color: win.surfaceCard
                border.width: 1
                border.color: theme.red

                MouseArea { anchors.fill: parent }

                ColumnLayout {
                    id: wipeCol
                    anchors.fill: parent
                    anchors.margins: 24
                    spacing: 14

                    ModalHeader {
                        title: qsTr("Wipe Environment '%1'?").arg(backend.selectedEnvironment.name || "")
                        onClosed: win.wipeVisible = false
                    }

                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        lineHeight: 1.35
                        text: qsTr("This will permanently destroy the entire Linux OS container, installed packages, databases, and snapshots inside '%1' in 1 second. Your host OS remains untouched.").arg(backend.selectedEnvironment.name || "")
                        color: theme.muted
                        font.family: win.sansFont
                        font.pixelSize: 12
                    }

                    // Custom clean checkbox row
                    Rectangle {
                        Layout.fillWidth: true
                        height: 38
                        radius: Math.min(4, theme.radius)
                        color: win.surfaceInput
                        border.width: 1
                        border.color: win.deleteWorkspaceOnWipe ? theme.red : win.borderSubtle

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 12
                            spacing: 10

                            Rectangle {
                                width: 16
                                height: 16
                                radius: 3
                                color: win.deleteWorkspaceOnWipe ? theme.red : "transparent"
                                border.width: 1
                                border.color: win.deleteWorkspaceOnWipe ? theme.red : theme.muted
                                Text {
                                    anchors.centerIn: parent
                                    visible: win.deleteWorkspaceOnWipe
                                    text: "✓"
                                    color: "black"
                                    font.family: win.monoFont
                                    font.pixelSize: 11
                                }
                            }

                            Text {
                                Layout.fillWidth: true
                                elide: Text.ElideMiddle
                                text: qsTr("Also delete shared project files and workspace snapshots in %1").arg(backend.selectedEnvironment.workspacePath || "")
                                color: theme.foreground
                                font.family: win.sansFont
                                font.pixelSize: 12
                            }
                        }

                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: win.deleteWorkspaceOnWipe = !win.deleteWorkspaceOnWipe
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.topMargin: 4
                        spacing: 10
                        Item { Layout.fillWidth: true }

                        ActionButton {
                            label: qsTr("Cancel")
                            minWidth: 96
                            onClicked: win.wipeVisible = false
                        }

                        ActionButton {
                            label: qsTr("Wipe Environment Now")
                            variant: "danger"
                            minWidth: 185
                            onClicked: {
                                backend.wipeEnvironment(backend.selectedEnvironment.name || "", win.deleteWorkspaceOnWipe)
                                win.wipeVisible = false
                            }
                        }
                    }
                }
            }
        }

        // ======================================================================
        // MODAL 2B: RESTORE SNAPSHOT CONFIRMATION (SYSTEM / WORKSPACE / BOTH)
        // ======================================================================
        Rectangle {
            anchors.fill: parent
            visible: win.restoreDialogVisible
            color: "#c406070a"

            MouseArea {
                anchors.fill: parent
                onClicked: win.restoreDialogVisible = false
            }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(parent.width - 60, 520)
                implicitHeight: restoreCol.implicitHeight + 48
                radius: Math.min(8, theme.radius)
                color: win.surfaceCard
                border.width: 1
                border.color: theme.accent

                MouseArea { anchors.fill: parent }

                ColumnLayout {
                    id: restoreCol
                    anchors.fill: parent
                    anchors.margins: 24
                    spacing: 14

                    ModalHeader {
                        title: qsTr("Restore Snapshot '%1'").arg(win.restoreSnapName)
                        subtitle: qsTr("Choose what to restore for environment '%1':").arg(backend.selectedEnvironment.name || "")
                        onClosed: win.restoreDialogVisible = false
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        Repeater {
                            model: [
                                { id: "both", title: qsTr("Both (System + Workspace)"), desc: qsTr("Restore OS container rootfs and %1 together.").arg(backend.containerWorkspacePath), enabled: win.restoreHasSystem && win.restoreHasWorkspace },
                                { id: "system", title: qsTr("System only"), desc: qsTr("Restore OS container rootfs; leave %1 untouched.").arg(backend.containerWorkspacePath), enabled: win.restoreHasSystem },
                                { id: "workspace", title: qsTr("Workspace only"), desc: qsTr("Restore %1 from Btrfs snapshot; leave OS packages untouched.").arg(backend.containerWorkspacePath), enabled: win.restoreHasWorkspace }
                            ]
                            delegate: Rectangle {
                                required property var modelData
                                Layout.fillWidth: true
                                implicitHeight: 48
                                radius: Math.min(4, theme.radius)
                                opacity: modelData.enabled ? 1.0 : 0.45
                                color: win.restoreMode === modelData.id ? win.surfaceCard : win.surfaceInput
                                border.width: 1
                                border.color: win.restoreMode === modelData.id ? theme.accent : win.borderSubtle

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 12
                                    anchors.rightMargin: 12
                                    spacing: 10

                                    Rectangle {
                                        width: 14
                                        height: 14
                                        radius: 7
                                        color: "transparent"
                                        border.width: 1
                                        border.color: win.restoreMode === modelData.id ? theme.accent : theme.muted
                                        Rectangle {
                                            anchors.centerIn: parent
                                            width: 8
                                            height: 8
                                            radius: 4
                                            visible: win.restoreMode === modelData.id
                                            color: theme.accent
                                        }
                                    }

                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: 2
                                        Text {
                                            text: modelData.title
                                            color: theme.foreground
                                            font.family: win.sansFont
                                            font.pixelSize: 12
                                            font.weight: Font.Medium
                                        }
                                        Text {
                                            text: modelData.desc
                                            color: theme.muted
                                            font.family: win.sansFont
                                            font.pixelSize: 11
                                        }
                                    }
                                }

                                MouseArea {
                                    anchors.fill: parent
                                    enabled: modelData.enabled
                                    cursorShape: modelData.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                                    onClicked: win.restoreMode = modelData.id
                                }
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        visible: win.restoreMode !== "system"
                        implicitHeight: restoreNoteText.implicitHeight + 16
                        radius: Math.min(4, theme.radius)
                        color: win.surfaceInput
                        border.width: 1
                        border.color: win.borderSubtle

                        Text {
                            id: restoreNoteText
                            anchors.fill: parent
                            anchors.margins: 8
                            wrapMode: Text.WordWrap
                            lineHeight: 1.3
                            text: (backend.selectedEnvironment.running || false)
                                  ? qsTr("A safety snapshot (pre-restore) of your current %1 will be created first, and '%2' will be stopped automatically before swapping %1.").arg(backend.containerWorkspacePath).arg(backend.selectedEnvironment.name || "")
                                  : qsTr("A safety snapshot (pre-restore) of your current %1 will be created before swapping %1.").arg(backend.containerWorkspacePath)
                            color: theme.muted
                            font.family: win.sansFont
                            font.pixelSize: 11
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.topMargin: 4
                        spacing: 10
                        Item { Layout.fillWidth: true }

                        ActionButton {
                            label: qsTr("Cancel")
                            minWidth: 96
                            onClicked: win.restoreDialogVisible = false
                        }

                        ActionButton {
                            label: qsTr("Restore Snapshot")
                            variant: "accent"
                            minWidth: 155
                            onClicked: {
                                backend.restoreSnapshot(backend.selectedEnvironment.name || "", win.restoreSnapName, win.restoreMode)
                                win.restoreDialogVisible = false
                            }
                        }
                    }
                }
            }
        }

        // ======================================================================
        // MODAL 2C: WORKSPACE CHANGE REVIEW (V)
        // ======================================================================
        Rectangle {
            anchors.fill: parent
            visible: win.changeReviewVisible
            color: "#c406070a"

            MouseArea {
                anchors.fill: parent
                onClicked: win.changeReviewVisible = false
            }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(parent.width - 56, 1020)
                height: Math.min(parent.height - 56, 680)
                radius: Math.min(8, theme.radius)
                color: win.surfaceCard
                border.width: 1
                border.color: theme.accent

                MouseArea { anchors.fill: parent }

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 24
                    spacing: 12

                    ModalHeader {
                        title: qsTr("Workspace Change Review — %1").arg(backend.selectedEnvironment.name || "")
                        onClosed: win.changeReviewVisible = false

                        ActionButton {
                            label: win.diffEditExcludes ? qsTr("Hide Excludes") : qsTr("Excludes")
                            onClicked: win.diffEditExcludes = !win.diffEditExcludes
                        }
                        ActionButton {
                            label: qsTr("Refresh")
                            disabled: backend.diffLoading || win.diffSnapName.length === 0
                            onClicked: backend.loadWorkspaceDiff(backend.selectedEnvironment.name || "", win.diffSnapName)
                        }
                    }

                    // Snapshot selector strip
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8
                        Text {
                            text: qsTr("Snapshot:")
                            color: theme.muted
                            font.family: win.sansFont
                            font.pixelSize: 12
                        }
                        ScrollView {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 36
                            clip: true
                            RowLayout {
                                spacing: 6
                                Repeater {
                                    model: backend.selectedEnvironment.workspaceSnapshots || []
                                    delegate: ActionButton {
                                        required property string modelData
                                        implicitHeight: 28
                                        label: modelData
                                        variant: win.diffSnapName === modelData ? "accent" : ""
                                        onClicked: {
                                            win.diffSnapName = modelData
                                            win.diffSelectedPath = ""
                                            win.diffSelectedStatus = ""
                                            win.diffSelectedType = ""
                                            win.diffActionError = ""
                                            backend.loadWorkspaceDiff(backend.selectedEnvironment.name || "", modelData)
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Optional diffExcludes editor
                    RowLayout {
                        Layout.fillWidth: true
                        visible: win.diffEditExcludes
                        spacing: 10
                        FlatInput {
                            id: diffExcludesInput
                            Layout.fillWidth: true
                            implicitHeight: 34
                            text: win.diffExcludesDraft
                            placeholder: qsTr("Comma-separated excludes (e.g. .git/, node_modules/, build/)")
                            onTextChanged: win.diffExcludesDraft = text
                        }
                        ActionButton {
                            implicitHeight: 34
                            label: qsTr("Save Excludes")
                            variant: "accent"
                            onClicked: {
                                const err = backend.saveDiffExcludes(diffExcludesInput.text)
                                if (err.length > 0) {
                                    win.diffActionError = err
                                } else {
                                    win.diffActionError = ""
                                    win.diffEditExcludes = false
                                    if (win.diffSnapName.length > 0)
                                        backend.loadWorkspaceDiff(backend.selectedEnvironment.name || "", win.diffSnapName)
                                }
                            }
                        }
                    }

                    // Summary bar
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12
                        Text {
                            visible: win.diffSnapName.length === 0
                            text: qsTr("No workspace Btrfs snapshots exist for this environment yet.")
                            color: theme.muted
                            font.family: win.sansFont
                            font.pixelSize: 12
                        }
                        Text {
                            visible: win.diffSnapName.length > 0 && backend.diffLoading
                            text: qsTr("Scanning workspace changes against '%1'...").arg(win.diffSnapName)
                            color: theme.accent
                            font.family: win.sansFont
                            font.pixelSize: 12
                        }
                        Text {
                            visible: win.diffSnapName.length > 0 && !backend.diffLoading && !(backend.diffSummary.error && backend.diffSummary.error.length > 0)
                            text: qsTr("+%1 Added   ~%2 Modified   -%3 Deleted   (%4 total)")
                                  .arg(backend.diffSummary.addedCount || 0)
                                  .arg(backend.diffSummary.modifiedCount || 0)
                                  .arg(backend.diffSummary.deletedCount || 0)
                                  .arg(backend.diffSummary.totalCount || 0)
                            color: theme.foreground
                            font.family: win.monoFont
                            font.pixelSize: 12
                        }
                        Rectangle {
                            visible: backend.diffSummary.truncated || false
                            height: 20
                            width: truncLabel.implicitWidth + 12
                            radius: 2
                            color: win.surfaceInput
                            border.width: 1
                            border.color: theme.yellow
                            Text {
                                id: truncLabel
                                anchors.centerIn: parent
                                text: qsTr("RESULTS TRUNCATED (5000 MAX)")
                                color: theme.yellow
                                font.family: win.monoFont
                                font.pixelSize: 10
                            }
                        }
                        Item { Layout.fillWidth: true }
                    }

                    Text {
                        Layout.fillWidth: true
                        visible: (backend.diffSummary.error && backend.diffSummary.error.length > 0) || win.diffActionError.length > 0
                        text: win.diffActionError.length > 0 ? win.diffActionError : (backend.diffSummary.error || "")
                        color: theme.red
                        font.family: win.sansFont
                        font.pixelSize: 12
                        wrapMode: Text.WordWrap
                    }

                    // Split view: file list (left) + unified diff (right)
                    RowLayout {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        spacing: 12

                        // Left pane: changed files
                        Rectangle {
                            Layout.preferredWidth: 370
                            Layout.fillHeight: true
                            radius: Math.min(4, theme.radius)
                            color: win.surfaceInput
                            border.width: 1
                            border.color: win.borderSubtle

                            Text {
                                anchors.centerIn: parent
                                visible: !backend.diffLoading && backend.diffEntries.length === 0 && win.diffSnapName.length > 0
                                text: qsTr("No workspace changes since '%1'.").arg(win.diffSnapName)
                                color: theme.muted
                                font.family: win.sansFont
                                font.pixelSize: 12
                            }

                            ListView {
                                id: diffListView
                                anchors.fill: parent
                                anchors.margins: 8
                                clip: true
                                spacing: 4
                                model: backend.diffEntries

                                delegate: Rectangle {
                                    required property var modelData
                                    readonly property bool isSelected: win.diffSelectedPath === (modelData.path || "")
                                    width: diffListView.width
                                    height: 32
                                    radius: Math.min(3, theme.radius)
                                    color: isSelected ? win.surfaceCard : "transparent"
                                    border.width: isSelected ? 1 : 0
                                    border.color: theme.accent

                                    RowLayout {
                                        anchors.fill: parent
                                        anchors.leftMargin: 8
                                        anchors.rightMargin: 8
                                        spacing: 8

                                        Text {
                                            text: modelData.status === "added" ? "+" : (modelData.status === "deleted" ? "-" : "~")
                                            color: modelData.status === "added" ? theme.green : (modelData.status === "deleted" ? theme.red : theme.yellow)
                                            font.family: win.monoFont
                                            font.pixelSize: 12
                                            font.weight: Font.Bold
                                        }

                                        Text {
                                            Layout.fillWidth: true
                                            elide: Text.ElideLeft
                                            text: modelData.path || ""
                                            color: theme.foreground
                                            font.family: win.monoFont
                                            font.pixelSize: 11
                                        }

                                        Text {
                                            text: modelData.sizeText || ""
                                            color: theme.muted
                                            font.family: win.monoFont
                                            font.pixelSize: 10
                                        }
                                    }

                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: {
                                            win.diffSelectedPath = modelData.path || ""
                                            win.diffSelectedStatus = modelData.status || ""
                                            win.diffSelectedType = modelData.type || ""
                                            win.diffActionError = ""
                                            backend.loadFileDiff(backend.selectedEnvironment.name || "", win.diffSnapName, win.diffSelectedPath, win.diffSelectedStatus)
                                        }
                                    }
                                }
                            }
                        }

                        // Right pane: unified diff & single-file restore
                        Rectangle {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            radius: Math.min(4, theme.radius)
                            color: win.surfaceInput
                            border.width: 1
                            border.color: win.borderSubtle

                            ColumnLayout {
                                anchors.fill: parent
                                anchors.margins: 10
                                spacing: 8

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 10
                                    Text {
                                        Layout.fillWidth: true
                                        elide: Text.ElideMiddle
                                        text: backend.selectedDiffInfo.length > 0
                                              ? backend.selectedDiffInfo
                                              : qsTr("Select a changed file to inspect its unified diff.")
                                        color: backend.selectedDiffInfo.length > 0 ? theme.foreground : theme.muted
                                        font.family: win.monoFont
                                        font.pixelSize: 11
                                    }
                                    ActionButton {
                                        implicitHeight: 28
                                        visible: win.diffSelectedPath.length > 0 && win.diffSelectedType !== "dir" && (win.diffSelectedStatus === "modified" || win.diffSelectedStatus === "deleted")
                                        label: qsTr("Restore this file")
                                        variant: "accent"
                                        onClicked: {
                                            const err = backend.restoreSingleFile(backend.selectedEnvironment.name || "", win.diffSnapName, win.diffSelectedPath)
                                            if (err.length > 0) {
                                                win.diffActionError = err
                                            } else {
                                                win.diffActionError = ""
                                                win.diffSelectedPath = ""
                                                win.diffSelectedStatus = ""
                                                win.diffSelectedType = ""
                                            }
                                        }
                                    }
                                }

                                Rectangle {
                                    Layout.fillWidth: true
                                    height: 1
                                    color: win.borderSubtle
                                }

                                ScrollView {
                                    Layout.fillWidth: true
                                    Layout.fillHeight: true
                                    clip: true

                                    TextArea {
                                        readOnly: true
                                        selectByMouse: true
                                        wrapMode: TextEdit.NoWrap
                                        text: backend.selectedDiffText.length > 0
                                              ? backend.selectedDiffText
                                              : (backend.selectedDiffInfo.length > 0 && win.diffSelectedPath.length > 0
                                                 ? backend.selectedDiffInfo
                                                 : "")
                                        color: backend.selectedDiffText.length > 0 ? theme.foreground : theme.muted
                                        font.family: win.monoFont
                                        font.pixelSize: 11
                                        background: null
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ======================================================================
        // MODAL 3: KEYBOARD SHORTCUTS OVERLAY (?)
        // ======================================================================
        Rectangle {
            anchors.fill: parent
            visible: win.helpVisible
            color: "#c406070a"

            MouseArea {
                anchors.fill: parent
                onClicked: win.helpVisible = false
            }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(parent.width - 60, 480)
                height: helpCol.implicitHeight + 48
                radius: Math.min(8, theme.radius)
                color: win.surfaceCard
                border.width: 1
                border.color: theme.accent

                MouseArea { anchors.fill: parent }

                ColumnLayout {
                    id: helpCol
                    anchors.fill: parent
                    anchors.margins: 24
                    spacing: 10

                    ModalHeader {
                        title: qsTr("Keyboard Shortcuts")
                        onClosed: win.helpVisible = false
                    }

                    Repeater {
                        model: [
                            { k: "Space / Enter", d: qsTr("Enter selected OS terminal in %1").arg(backend.containerWorkspacePath) },
                            { k: "N", d: qsTr("Create a new isolated OS environment") },
                            { k: "F", d: qsTr("Open shared workspace folder on Host") },
                            { k: "S", d: qsTr("Start or stop selected environment") },
                            { k: "P", d: qsTr("Create instant 1-second Btrfs snapshot") },
                            { k: "V", d: qsTr("Review %1 changes since a snapshot").arg(backend.containerWorkspacePath) },
                            { k: "D", d: qsTr("Clone selected environment") },
                            { k: "E", d: qsTr("Edit quotas and limits of selected environment") },
                            { k: "T", d: qsTr("Mark or unmark environment as template") },
                            { k: "K", d: qsTr("Kill or restore environment network") },
                            { k: "C", d: qsTr("Copy AI agent execution command") },
                            { k: "I", d: qsTr("Save clipboard image to workspace and copy its path") },
                            { k: "H", d: qsTr("View operation history and full task logs") },
                            { k: "B", d: qsTr("Export and import environment backups") },
                            { k: "A", d: qsTr("Manage global and per-environment secrets") },
                            { k: "U", d: qsTr("Inspect stale environments & disk orphans") },
                            { k: "Delete", d: qsTr("Wipe / destroy selected environment") },
                            { k: "1 / 2", d: qsTr("Switch between Environments and OS Catalog") },
                            { k: "3", d: qsTr("Open the configuration menu") },
                            { k: "R", d: qsTr("Refresh status and disk usage") },
                            { k: "?", d: qsTr("Toggle this shortcuts overlay") },
                            { k: "Q", d: qsTr("Quit AIBox") }
                        ]
                        delegate: RowLayout {
                            required property var modelData
                            Layout.fillWidth: true
                            spacing: 12
                            Rectangle {
                                width: 104
                                height: 24
                                radius: 3
                                color: win.surfaceInput
                                border.width: 1
                                border.color: win.borderSubtle
                                Text {
                                    anchors.centerIn: parent
                                    text: modelData.k
                                    color: theme.accent
                                    font.family: win.monoFont
                                    font.pixelSize: 11
                                }
                            }
                            Text {
                                Layout.fillWidth: true
                                text: modelData.d
                                color: theme.muted
                                font.family: win.sansFont
                                font.pixelSize: 12
                            }
                        }
                    }
                }
            }
        }

        // ======================================================================
        // MODAL 4: CONFIGURATION MENU (STORAGE, OS CATALOG, CREATE OPTIONS)
        // ======================================================================
        Rectangle {
            id: settingsOverlay
            anchors.fill: parent
            visible: win.settingsVisible
            color: "#c406070a"
            z: 80

            onVisibleChanged: {
                if (!visible)
                    return
                win.configNotice = ""
                win.osEditorVisible = false
                win.osEditorError = ""
                workspacePathField.text = backend.workspaceBase
                osStoragePathField.text = backend.osStoragePath
                win.syncCreateChoiceDrafts()
                win.autoSnapEnabledDraft = backend.autoSnapshotEnabled
                autoSnapKeepField.text = String(backend.autoSnapshotKeep)
                win.watchdogEnabledDraft = backend.watchdogEnabled
                win.watchdogAutoStopDraft = backend.watchdogAutoStop
                win.idleStopMinutesDraft = backend.idleStopMinutes
                win.staleDaysDraft = backend.staleDays
                win.recordSessionsDraft = backend.recordSessions
                watchdogWarnField.text = String(backend.watchdogDiskWarn)
                watchdogCritField.text = String(backend.watchdogDiskCritical)
                win.poolResizeDraft = ""
                win.poolResizeConfirmVisible = false
                win.poolResizeError = ""
                poolSizeField.text = ""
            }

            MouseArea {
                anchors.fill: parent
                onClicked: win.settingsVisible = false
            }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(parent.width - 56, 980)
                height: Math.min(parent.height - 56, 650)
                radius: Math.min(8, theme.radius)
                color: win.surfaceCard
                border.width: 1
                border.color: win.borderSubtle
                clip: true

                MouseArea { anchors.fill: parent }

                RowLayout {
                    anchors.fill: parent
                    spacing: 0

                    // ------------------------------------------------------
                    // CONFIGURATION SIDEBAR
                    // ------------------------------------------------------
                    Rectangle {
                        Layout.preferredWidth: 232
                        Layout.fillHeight: true
                        color: "#0f1018"

                        ColumnLayout {
                            anchors.fill: parent
                            anchors.margins: 16
                            spacing: 6

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10
                                Rectangle {
                                    width: 30
                                    height: 30
                                    radius: Math.min(5, theme.radius)
                                    color: win.surfaceSelected
                                    border.width: 1
                                    border.color: theme.accent
                                    SlidersIcon {
                                        anchors.centerIn: parent
                                        width: 16
                                        height: 16
                                        color: theme.accent
                                    }
                                }
                                ColumnLayout {
                                    spacing: 1
                                    Text {
                                        text: qsTr("Configuration")
                                        color: theme.foreground
                                        font.family: win.sansFont
                                        font.pixelSize: 14
                                        font.weight: Font.Medium
                                    }
                                    Text {
                                        text: qsTr("AIBox settings")
                                        color: theme.muted
                                        font.family: win.sansFont
                                        font.pixelSize: 10
                                    }
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.topMargin: 8
                                Layout.bottomMargin: 6
                                height: 1
                                color: win.borderSubtle
                            }

                            Repeater {
                                model: [
                                    { title: qsTr("Locations"), detail: qsTr("Workspace & OS images"), step: "01" },
                                    { title: qsTr("Storage Pool"), detail: qsTr("Incus capacity & growth"), step: "02" },
                                    { title: qsTr("OS Catalog"), detail: qsTr("Add, edit, remove"), step: "03" },
                                    { title: qsTr("Create Options"), detail: qsTr("Presets & quotas"), step: "04" },
                                    { title: qsTr("Safety"), detail: qsTr("Auto-snapshots"), step: "05" }
                                ]
                                delegate: Rectangle {
                                    required property var modelData
                                    required property int index
                                    Layout.fillWidth: true
                                    height: 52
                                    radius: Math.min(5, theme.radius)
                                    color: win.settingsSection === index ? win.surfaceSelected : "transparent"
                                    border.width: win.settingsSection === index ? 1 : 0
                                    border.color: theme.accent

                                    RowLayout {
                                        anchors.fill: parent
                                        anchors.leftMargin: 10
                                        anchors.rightMargin: 10
                                        spacing: 10

                                        Text {
                                            text: modelData.step
                                            color: win.settingsSection === index ? theme.accent : theme.muted
                                            font.family: win.monoFont
                                            font.pixelSize: 10
                                        }

                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            spacing: 1
                                            Text {
                                                text: modelData.title
                                                color: win.settingsSection === index ? theme.foreground : theme.muted
                                                font.family: win.sansFont
                                                font.pixelSize: 12
                                                font.weight: win.settingsSection === index ? Font.Medium : Font.Normal
                                            }
                                            Text {
                                                text: modelData.detail
                                                color: theme.muted
                                                font.family: win.sansFont
                                                font.pixelSize: 10
                                            }
                                        }
                                    }

                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: win.settingsSection = index
                                    }
                                }
                            }

                            Item { Layout.fillHeight: true }

                            Text {
                                Layout.fillWidth: true
                                text: qsTr("Config file")
                                color: theme.muted
                                font.family: win.sansFont
                                font.pixelSize: 10
                            }
                            Text {
                                Layout.fillWidth: true
                                text: backend.configFilePath
                                color: theme.muted
                                font.family: win.monoFont
                                font.pixelSize: 9
                                elide: Text.ElideMiddle
                            }
                        }
                    }

                    Rectangle {
                        width: 1
                        Layout.fillHeight: true
                        color: win.borderSubtle
                    }

                    // ------------------------------------------------------
                    // CONFIGURATION CONTENT
                    // ------------------------------------------------------
                    ColumnLayout {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        Layout.margins: 24
                        spacing: 12

                        ModalHeader {
                            title: win.settingsSection === 0
                                   ? qsTr("Storage Locations")
                                   : (win.settingsSection === 1
                                      ? qsTr("Incus Storage Pool")
                                      : (win.settingsSection === 2
                                         ? qsTr("Linux OS Catalog")
                                         : (win.settingsSection === 3 ? qsTr("Create Environment Options") : qsTr("Safety & Auto-Snapshots"))))
                            subtitle: win.settingsSection === 0
                                      ? qsTr("Choose where shared project files live and where offline OS image copies are saved.")
                                      : (win.settingsSection === 1
                                         ? qsTr("Monitor pool usage and allocated quotas, and grow the loop file in place when space runs low.")
                                         : (win.settingsSection === 2
                                            ? qsTr("Add, edit, or remove the distributions shown in the environment creation dialog.")
                                            : (win.settingsSection === 3
                                               ? qsTr("Customize the preset cards, disk quota, RAM, and CPU choices shown in the creation dialog.")
                                               : qsTr("Configure automatic Btrfs snapshots before entering or starting an environment."))))
                            onClosed: win.settingsVisible = false
                        }

                        Rectangle {
                            visible: win.configNotice.length > 0
                            Layout.fillWidth: true
                            height: visible ? 34 : 0
                            radius: Math.min(4, theme.radius)
                            color: win.configNoticeError ? "#261319" : "#132019"
                            border.width: 1
                            border.color: win.configNoticeError ? theme.red : theme.green

                            Text {
                                anchors.fill: parent
                                anchors.leftMargin: 12
                                anchors.rightMargin: 12
                                verticalAlignment: Text.AlignVCenter
                                elide: Text.ElideRight
                                text: win.configNotice
                                color: win.configNoticeError ? theme.red : theme.green
                                font.family: win.sansFont
                                font.pixelSize: 12
                            }
                        }

                        ScrollView {
                            id: configScroll
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            clip: true
                            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

                            ColumnLayout {
                                width: configScroll.availableWidth
                                spacing: 14

                                // ==================================================
                                // SECTION 0: LOCATIONS
                                // ==================================================
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    visible: win.settingsSection === 0
                                    spacing: 12

                                    Rectangle {
                                        Layout.fillWidth: true
                                        implicitHeight: sysRootInfoCol.implicitHeight + 24
                                        radius: Math.min(6, theme.radius)
                                        color: win.surfaceInput
                                        border.width: 1
                                        border.color: win.borderSubtle

                                        ColumnLayout {
                                            id: sysRootInfoCol
                                            anchors.fill: parent
                                            anchors.margins: 12
                                            spacing: 4

                                            Text {
                                                text: qsTr("System root location")
                                                color: theme.foreground
                                                font.family: win.sansFont
                                                font.pixelSize: 12
                                                font.weight: Font.Medium
                                            }
                                            Text {
                                                Layout.fillWidth: true
                                                text: qsTr("Root: %1  ·  Config: %2").arg(backend.systemPath).arg(backend.configFilePath)
                                                color: theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 10
                                                elide: Text.ElideMiddle
                                            }
                                        }
                                    }

                                    Rectangle {
                                        Layout.fillWidth: true
                                        implicitHeight: wsLocCardCol.implicitHeight + 28
                                        radius: Math.min(6, theme.radius)
                                        color: win.surfaceInput
                                        border.width: 1
                                        border.color: win.borderSubtle

                                        ColumnLayout {
                                            id: wsLocCardCol
                                            anchors.fill: parent
                                            anchors.margins: 14
                                            spacing: 8

                                            Text {
                                                text: qsTr("Workspace location")
                                                color: theme.foreground
                                                font.family: win.sansFont
                                                font.pixelSize: 12
                                                font.weight: Font.Medium
                                            }
                                            Text {
                                                Layout.fillWidth: true
                                                text: qsTr("New environments share this host folder at %1. Existing environments keep their current shared folder.").arg(backend.containerWorkspacePath)
                                                color: theme.muted
                                                font.family: win.sansFont
                                                font.pixelSize: 10
                                                wrapMode: Text.WordWrap
                                            }
                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 10
                                                FlatInput {
                                                    id: workspacePathField
                                                    Layout.fillWidth: true
                                                    placeholder: backend.defaultWorkspaceBase
                                                }
                                                ActionButton {
                                                    label: qsTr("Browse...")
                                                    minWidth: 94
                                                    onClicked: workspaceFolderDialog.open()
                                                }
                                                ActionButton {
                                                    label: qsTr("Default")
                                                    minWidth: 84
                                                    onClicked: workspacePathField.text = backend.defaultWorkspaceBase
                                                }
                                            }
                                            Text {
                                                text: qsTr("Default: %1").arg(backend.defaultWorkspaceBase)
                                                color: theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 9
                                                elide: Text.ElideMiddle
                                                Layout.fillWidth: true
                                            }
                                        }
                                    }

                                    Rectangle {
                                        Layout.fillWidth: true
                                        implicitHeight: osLocCardCol.implicitHeight + 28
                                        radius: Math.min(6, theme.radius)
                                        color: win.surfaceInput
                                        border.width: 1
                                        border.color: win.borderSubtle

                                        ColumnLayout {
                                            id: osLocCardCol
                                            anchors.fill: parent
                                            anchors.margins: 14
                                            spacing: 8

                                            Text {
                                                text: qsTr("OS image storage")
                                                color: theme.foreground
                                                font.family: win.sansFont
                                                font.pixelSize: 12
                                                font.weight: Font.Medium
                                            }
                                            Text {
                                                Layout.fillWidth: true
                                                text: qsTr("Downloaded OS images are also exported here. If the local Incus image is gone, AIBox imports the copy from this folder instead of downloading again.")
                                                color: theme.muted
                                                font.family: win.sansFont
                                                font.pixelSize: 10
                                                wrapMode: Text.WordWrap
                                            }
                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 10
                                                FlatInput {
                                                    id: osStoragePathField
                                                    Layout.fillWidth: true
                                                    placeholder: backend.defaultOsStoragePath
                                                }
                                                ActionButton {
                                                    label: qsTr("Browse...")
                                                    minWidth: 94
                                                    onClicked: osStorageFolderDialog.open()
                                                }
                                                ActionButton {
                                                    label: qsTr("Default")
                                                    minWidth: 84
                                                    onClicked: osStoragePathField.text = backend.defaultOsStoragePath
                                                }
                                            }
                                            Text {
                                                text: qsTr("Default: %1").arg(backend.defaultOsStoragePath)
                                                color: theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 9
                                                elide: Text.ElideMiddle
                                                Layout.fillWidth: true
                                            }
                                        }
                                    }

                                    RowLayout {
                                        Layout.fillWidth: true
                                        Layout.topMargin: 2
                                        spacing: 10
                                        Item { Layout.fillWidth: true }
                                        ActionButton {
                                            label: qsTr("Save Locations")
                                            variant: "primary"
                                            minWidth: 150
                                            onClicked: win.saveGeneralSettings()
                                        }
                                    }
                                }

                                // ==================================================
                                // SECTION 1: STORAGE POOL
                                // ==================================================
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    visible: win.settingsSection === 1
                                    spacing: 12

                                    Rectangle {
                                        Layout.fillWidth: true
                                        visible: !(backend.poolInfo.valid || false)
                                        implicitHeight: poolUnavailableCol.implicitHeight + 28
                                        radius: Math.min(6, theme.radius)
                                        color: win.surfaceInput
                                        border.width: 1
                                        border.color: win.borderSubtle

                                        ColumnLayout {
                                            id: poolUnavailableCol
                                            anchors.fill: parent
                                            anchors.margins: 14
                                            spacing: 8

                                            Text {
                                                text: qsTr("Pool details unavailable")
                                                color: theme.foreground
                                                font.family: win.sansFont
                                                font.pixelSize: 12
                                                font.weight: Font.Medium
                                            }
                                            Text {
                                                Layout.fillWidth: true
                                                wrapMode: Text.WordWrap
                                                text: qsTr("AIBox could not read the Incus storage pool yet. Make sure Incus is initialized with the default pool, then refresh.")
                                                color: theme.muted
                                                font.family: win.sansFont
                                                font.pixelSize: 10
                                            }
                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 10
                                                Item { Layout.fillWidth: true }
                                                ActionButton {
                                                    label: qsTr("Refresh")
                                                    minWidth: 96
                                                    disabled: backend.busy
                                                    onClicked: backend.refresh()
                                                }
                                            }
                                        }
                                    }

                                    Rectangle {
                                        Layout.fillWidth: true
                                        visible: backend.poolInfo.valid || false
                                        implicitHeight: poolSettingsCol.implicitHeight + 28
                                        radius: Math.min(6, theme.radius)
                                        color: win.surfaceInput
                                        border.width: 1
                                        border.color: (backend.poolInfo.alertLevel === 2)
                                                      ? theme.red
                                                      : ((backend.poolInfo.alertLevel === 1) ? "#e5a93c" : win.borderSubtle)

                                        ColumnLayout {
                                            id: poolSettingsCol
                                            anchors.fill: parent
                                            anchors.margins: 14
                                            spacing: 8

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 10
                                                Text {
                                                    Layout.fillWidth: true
                                                    text: qsTr("Incus storage pool")
                                                    color: theme.foreground
                                                    font.family: win.sansFont
                                                    font.pixelSize: 12
                                                    font.weight: Font.Medium
                                                }
                                                Text {
                                                    text: qsTr("%1 / %2 (%3%)")
                                                              .arg(backend.poolInfo.usedText || "0 MiB")
                                                              .arg(backend.poolInfo.totalText || "0 MiB")
                                                              .arg(backend.poolInfo.usedPercent !== undefined ? backend.poolInfo.usedPercent : 0)
                                                    color: (backend.poolInfo.alertLevel === 2)
                                                           ? theme.red
                                                           : ((backend.poolInfo.alertLevel === 1) ? "#e5a93c" : theme.muted)
                                                    font.family: win.monoFont
                                                    font.pixelSize: 10
                                                }
                                            }

                                            Rectangle {
                                                Layout.fillWidth: true
                                                height: 4
                                                radius: 2
                                                color: "#1a1d29"

                                                Rectangle {
                                                    height: parent.height
                                                    radius: 2
                                                    width: Math.max(2, parent.width * (backend.poolInfo.usedPercent || 0) / 100)
                                                    color: (backend.poolInfo.alertLevel === 2)
                                                           ? theme.red
                                                           : ((backend.poolInfo.alertLevel === 1) ? "#e5a93c" : theme.accent)
                                                }
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                elide: Text.ElideMiddle
                                                text: backend.poolInfo.resizable
                                                      ? qsTr("%1 · %2 · loop file size %3")
                                                            .arg(backend.poolInfo.name || "default")
                                                            .arg(backend.poolInfo.driver || "btrfs")
                                                            .arg(backend.poolInfo.sizeText || "—")
                                                      : qsTr("%1 · %2 · directory-backed (follows host free space)")
                                                            .arg(backend.poolInfo.name || "default")
                                                            .arg(backend.poolInfo.driver || "btrfs")
                                                color: theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 10
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                wrapMode: Text.WordWrap
                                                visible: (backend.poolInfo.allocatedBytes || 0) > 0
                                                text: qsTr("Allocated quotas: %1%2")
                                                          .arg(backend.poolInfo.allocatedText || "0 MiB")
                                                          .arg((backend.poolInfo.unboundedCount || 0) > 0
                                                               ? qsTr(" · %1 without quota").arg(backend.poolInfo.unboundedCount)
                                                               : "")
                                                color: (backend.poolInfo.overcommitted || false) ? "#e5a93c" : theme.muted
                                                font.family: win.sansFont
                                                font.pixelSize: 10
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                wrapMode: Text.WordWrap
                                                text: backend.poolInfo.resizable
                                                      ? qsTr("Grow the loop file in place, for example 60GiB (plain numbers mean GiB). Incus grows the Btrfs filesystem online; pools can never be shrunk.")
                                                      : qsTr("This pool is backed by a Btrfs subvolume on the host filesystem, so it grows automatically with the host and cannot be resized here.")
                                                color: theme.muted
                                                font.family: win.sansFont
                                                font.pixelSize: 10
                                            }

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 10
                                                visible: backend.poolInfo.resizable || false

                                                FlatInput {
                                                    id: poolSizeField
                                                    Layout.fillWidth: true
                                                    placeholder: backend.poolInfo.sizeText || "60GiB"
                                                    onAccepted: {
                                                        win.poolResizeDraft = poolSizeField.text
                                                        win.preparePoolResize()
                                                    }
                                                }
                                                ActionButton {
                                                    label: qsTr("Grow pool")
                                                    minWidth: 118
                                                    disabled: backend.busy
                                                    onClicked: {
                                                        win.poolResizeDraft = poolSizeField.text
                                                        win.preparePoolResize()
                                                    }
                                                }
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                wrapMode: Text.WordWrap
                                                visible: win.poolResizeError.length > 0
                                                text: win.poolResizeError
                                                color: theme.red
                                                font.family: win.sansFont
                                                font.pixelSize: 10
                                            }

                                            Rectangle {
                                                Layout.fillWidth: true
                                                visible: win.poolResizeConfirmVisible
                                                implicitHeight: poolConfirmCol.implicitHeight + 20
                                                radius: Math.min(4, theme.radius)
                                                color: "#1a1409"
                                                border.width: 1
                                                border.color: "#e5a93c"

                                                ColumnLayout {
                                                    id: poolConfirmCol
                                                    anchors.fill: parent
                                                    anchors.margins: 10
                                                    spacing: 6

                                                    Text {
                                                        Layout.fillWidth: true
                                                        wrapMode: Text.WordWrap
                                                        text: qsTr("Grow pool '%1' from %2 to %3? The loop file is resized in place and this cannot be undone.")
                                                                  .arg(backend.poolInfo.name || "default")
                                                                  .arg(backend.poolInfo.totalText || "0 MiB")
                                                                  .arg(win.poolResizeDraft.trim())
                                                        color: "#e5a93c"
                                                        font.family: win.sansFont
                                                        font.pixelSize: 11
                                                    }
                                                    Text {
                                                        Layout.fillWidth: true
                                                        wrapMode: Text.WordWrap
                                                        text: win.runningEnvironmentCount() > 0
                                                              ? qsTr("%1 environment(s) are running. Incus grows Btrfs pools online, but stop them first if you want maximum safety.")
                                                                    .arg(win.runningEnvironmentCount())
                                                              : qsTr("No environment is running.")
                                                        color: theme.muted
                                                        font.family: win.sansFont
                                                        font.pixelSize: 10
                                                    }
                                                    RowLayout {
                                                        Layout.fillWidth: true
                                                        spacing: 10
                                                        Item { Layout.fillWidth: true }
                                                        ActionButton {
                                                            label: qsTr("Cancel")
                                                            minWidth: 84
                                                            onClicked: win.poolResizeConfirmVisible = false
                                                        }
                                                        ActionButton {
                                                            label: qsTr("Confirm growth")
                                                            variant: "primary"
                                                            minWidth: 148
                                                            disabled: backend.busy
                                                            onClicked: win.confirmPoolResize()
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                // ==================================================
                                // SECTION 2: OS CATALOG
                                // ==================================================
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    visible: win.settingsSection === 2
                                    spacing: 10

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10
                                        Text {
                                            Layout.fillWidth: true
                                            text: qsTr("%n OS entries", "", backend.osCatalog.length)
                                            color: theme.muted
                                            font.family: win.sansFont
                                            font.pixelSize: 11
                                        }
                                        ActionButton {
                                            label: qsTr("+ Add OS")
                                            variant: "primary"
                                            minWidth: 105
                                            onClicked: win.beginAddOs()
                                        }
                                    }

                                    // Inline editor for adding or editing an OS entry.
                                    Rectangle {
                                        visible: win.osEditorVisible
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: visible ? osEditorColumn.implicitHeight + 28 : 0
                                        radius: Math.min(6, theme.radius)
                                        color: win.surfaceInput
                                        border.width: 1
                                        border.color: theme.accent

                                        ColumnLayout {
                                            id: osEditorColumn
                                            anchors.fill: parent
                                            anchors.margins: 14
                                            spacing: 8

                                            Text {
                                                text: win.osEditorOriginalId.length > 0 ? qsTr("Edit OS entry") : qsTr("New OS entry")
                                                color: theme.foreground
                                                font.family: win.sansFont
                                                font.pixelSize: 13
                                                font.weight: Font.Medium
                                            }

                                            GridLayout {
                                                Layout.fillWidth: true
                                                columns: 2
                                                columnSpacing: 10
                                                rowSpacing: 8

                                                ColumnLayout {
                                                    Layout.fillWidth: true
                                                    spacing: 4
                                                    Text {
                                                        text: qsTr("OS ID")
                                                        color: theme.muted
                                                        font.family: win.sansFont
                                                        font.pixelSize: 10
                                                    }
                                                    FlatInput {
                                                        id: osIdField
                                                        Layout.fillWidth: true
                                                        placeholder: qsTr("e.g. ubuntu-24.04")
                                                    }
                                                }

                                                ColumnLayout {
                                                    Layout.fillWidth: true
                                                    spacing: 4
                                                    Text {
                                                        text: qsTr("Image alias")
                                                        color: theme.muted
                                                        font.family: win.sansFont
                                                        font.pixelSize: 10
                                                    }
                                                    FlatInput {
                                                        id: osAliasField
                                                        Layout.fillWidth: true
                                                        placeholder: qsTr("e.g. ubuntu/24.04")
                                                    }
                                                }

                                                ColumnLayout {
                                                    Layout.fillWidth: true
                                                    spacing: 4
                                                    Text {
                                                        text: qsTr("Display name")
                                                        color: theme.muted
                                                        font.family: win.sansFont
                                                        font.pixelSize: 10
                                                    }
                                                    FlatInput {
                                                        id: osNameField
                                                        Layout.fillWidth: true
                                                        placeholder: qsTr("e.g. Ubuntu 24.04 LTS")
                                                    }
                                                }

                                                ColumnLayout {
                                                    Layout.fillWidth: true
                                                    spacing: 4
                                                    Text {
                                                        text: qsTr("Badge")
                                                        color: theme.muted
                                                        font.family: win.sansFont
                                                        font.pixelSize: 10
                                                    }
                                                    FlatInput {
                                                        id: osBadgeField
                                                        Layout.fillWidth: true
                                                        placeholder: qsTr("e.g. Recommended")
                                                    }
                                                }

                                                ColumnLayout {
                                                    Layout.fillWidth: true
                                                    Layout.columnSpan: 2
                                                    spacing: 4
                                                    Text {
                                                        text: qsTr("Description")
                                                        color: theme.muted
                                                        font.family: win.sansFont
                                                        font.pixelSize: 10
                                                    }
                                                    FlatInput {
                                                        id: osDescriptionField
                                                        Layout.fillWidth: true
                                                        placeholder: qsTr("Short description shown in the OS catalog")
                                                    }
                                                }
                                            }

                                            Text {
                                                visible: win.osEditorError.length > 0
                                                Layout.fillWidth: true
                                                text: win.osEditorError
                                                color: theme.red
                                                font.family: win.sansFont
                                                font.pixelSize: 11
                                                wrapMode: Text.WordWrap
                                            }

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 8
                                                Item { Layout.fillWidth: true }
                                                ActionButton {
                                                    label: qsTr("Cancel")
                                                    minWidth: 88
                                                    onClicked: win.osEditorVisible = false
                                                }
                                                ActionButton {
                                                    label: qsTr("Save OS")
                                                    variant: "primary"
                                                    minWidth: 108
                                                    onClicked: win.submitOsEditor()
                                                }
                                            }
                                        }
                                    }

                                    Repeater {
                                        model: backend.osCatalog
                                        delegate: Rectangle {
                                            required property var modelData
                                            Layout.fillWidth: true
                                            height: 64
                                            radius: Math.min(5, theme.radius)
                                            color: win.surfaceInput
                                            border.width: 1
                                            border.color: modelData.downloaded ? "#284d3a" : win.borderSubtle

                                            RowLayout {
                                                anchors.fill: parent
                                                anchors.leftMargin: 12
                                                anchors.rightMargin: 12
                                                spacing: 10

                                                Rectangle {
                                                    width: 30
                                                    height: 30
                                                    radius: 3
                                                    color: modelData.downloaded ? "#162920" : win.surfaceCard
                                                    border.width: 1
                                                    border.color: modelData.downloaded ? "#284d3a" : win.borderSubtle
                                                    Text {
                                                        anchors.centerIn: parent
                                                        text: modelData.downloaded ? "✓" : "↓"
                                                        color: modelData.downloaded ? theme.green : theme.accent
                                                        font.family: win.monoFont
                                                        font.pixelSize: 12
                                                    }
                                                }

                                                ColumnLayout {
                                                    Layout.fillWidth: true
                                                    spacing: 1
                                                    Text {
                                                        Layout.fillWidth: true
                                                        elide: Text.ElideRight
                                                        text: modelData.name
                                                        color: theme.foreground
                                                        font.family: win.sansFont
                                                        font.pixelSize: 12
                                                        font.weight: Font.Medium
                                                    }
                                                    Text {
                                                        Layout.fillWidth: true
                                                        elide: Text.ElideRight
                                                        text: modelData.id + "  ·  " + modelData.alias
                                                        color: theme.muted
                                                        font.family: win.monoFont
                                                        font.pixelSize: 9
                                                    }
                                                }

                                                Rectangle {
                                                    height: 18
                                                    width: osBadgeValue.implicitWidth + 12
                                                    radius: 2
                                                    color: win.surfaceCard
                                                    border.width: 1
                                                    border.color: win.borderSubtle
                                                    Text {
                                                        id: osBadgeValue
                                                        anchors.centerIn: parent
                                                        text: modelData.badge
                                                        color: theme.accent
                                                        font.family: win.monoFont
                                                        font.pixelSize: 9
                                                    }
                                                }

                                                Text {
                                                    Layout.preferredWidth: 100
                                                    horizontalAlignment: Text.AlignRight
                                                    elide: Text.ElideRight
                                                    text: modelData.sizeText
                                                    color: modelData.downloaded ? theme.green : theme.muted
                                                    font.family: win.monoFont
                                                    font.pixelSize: 9
                                                }

                                                ActionButton {
                                                    implicitHeight: 26
                                                    label: qsTr("Edit")
                                                    minWidth: 58
                                                    onClicked: win.beginEditOs(modelData)
                                                }
                                                ActionButton {
                                                    implicitHeight: 26
                                                    label: qsTr("Remove")
                                                    danger: true
                                                    minWidth: 72
                                                    onClicked: win.removeOsEntry(modelData)
                                                }
                                            }
                                        }
                                    }

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10
                                        Text {
                                            Layout.fillWidth: true
                                            wrapMode: Text.WordWrap
                                            text: qsTr("Removing an entry only removes it from the catalog. Cached image files are kept until you remove them in OS Images & Downloads.")
                                            color: theme.muted
                                            font.family: win.sansFont
                                            font.pixelSize: 10
                                        }
                                        ActionButton {
                                            label: qsTr("Restore Defaults")
                                            minWidth: 135
                                            onClicked: {
                                                const error = backend.resetOsCatalog()
                                                if (error.length > 0)
                                                    win.showConfigNotice(error, true)
                                                else
                                                    win.showConfigNotice(qsTr("Default OS catalog restored."), false)
                                            }
                                        }
                                    }
                                }

                                // ==================================================
                                // SECTION 3: CREATE OPTIONS
                                // ==================================================
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    visible: win.settingsSection === 3
                                    spacing: 12

                                    Text {
                                        text: qsTr("Capability presets")
                                        color: theme.foreground
                                        font.family: win.sansFont
                                        font.pixelSize: 12
                                        font.weight: Font.Medium
                                    }
                                    Text {
                                        Layout.fillWidth: true
                                        text: qsTr("These cards appear above the distribution list. Their behavior stays the same; edit the title and subtitle.")
                                        color: theme.muted
                                        font.family: win.sansFont
                                        font.pixelSize: 10
                                        wrapMode: Text.WordWrap
                                    }

                                    Repeater {
                                        model: backend.presets
                                        delegate: Rectangle {
                                            required property var modelData
                                            Layout.fillWidth: true
                                            height: 84
                                            radius: Math.min(6, theme.radius)
                                            color: win.surfaceInput
                                            border.width: 1
                                            border.color: win.borderSubtle

                                            RowLayout {
                                                anchors.fill: parent
                                                anchors.margins: 12
                                                spacing: 10

                                                Text {
                                                    Layout.preferredWidth: 58
                                                    text: modelData.id
                                                    color: theme.accent
                                                    font.family: win.monoFont
                                                    font.pixelSize: 9
                                                    elide: Text.ElideRight
                                                }

                                                ColumnLayout {
                                                    Layout.fillWidth: true
                                                    spacing: 3
                                                    Text {
                                                        text: qsTr("Title")
                                                        color: theme.muted
                                                        font.family: win.sansFont
                                                        font.pixelSize: 9
                                                    }
                                                    FlatInput {
                                                        id: presetTitleField
                                                        Layout.fillWidth: true
                                                        text: modelData.title
                                                    }
                                                }

                                                ColumnLayout {
                                                    Layout.fillWidth: true
                                                    spacing: 3
                                                    Text {
                                                        text: qsTr("Subtitle")
                                                        color: theme.muted
                                                        font.family: win.sansFont
                                                        font.pixelSize: 9
                                                    }
                                                    FlatInput {
                                                        id: presetSubField
                                                        Layout.fillWidth: true
                                                        text: modelData.sub
                                                    }
                                                }

                                                ActionButton {
                                                    Layout.alignment: Qt.AlignBottom
                                                    label: qsTr("Save")
                                                    minWidth: 66
                                                    onClicked: {
                                                        const error = backend.savePreset(modelData.id, presetTitleField.text, presetSubField.text)
                                                        if (error.length > 0)
                                                            win.showConfigNotice(error, true)
                                                        else
                                                            win.showConfigNotice(qsTr("Preset saved."), false)
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    Rectangle {
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: quotaCardCol.implicitHeight + 28
                                        radius: Math.min(6, theme.radius)
                                        color: win.surfaceInput
                                        border.width: 1
                                        border.color: win.borderSubtle

                                        ColumnLayout {
                                            id: quotaCardCol
                                            anchors.fill: parent
                                            anchors.margins: 14
                                            spacing: 6

                                            Text {
                                                text: qsTr("Quota and core choices")
                                                color: theme.foreground
                                                font.family: win.sansFont
                                                font.pixelSize: 12
                                                font.weight: Font.Medium
                                            }
                                            Text {
                                                Layout.fillWidth: true
                                                text: qsTr("Type a number and press Enter to add a choice (up to 4). Disk and RAM units are appended automatically; press × on a chip to remove it.")
                                                color: theme.muted
                                                font.family: win.sansFont
                                                font.pixelSize: 10
                                                wrapMode: Text.WordWrap
                                            }

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 10
                                                Text {
                                                    Layout.preferredWidth: 120
                                                    Layout.alignment: Qt.AlignVCenter
                                                    text: qsTr("Disk quota")
                                                    color: theme.foreground
                                                    font.family: win.sansFont
                                                    font.pixelSize: 11
                                                }
                                                ChipInput {
                                                    Layout.fillWidth: true
                                                    values: win.diskChoiceDraft
                                                    unit: "GiB"
                                                    maxChips: 4
                                                    maxValue: 65536
                                                    placeholder: qsTr("Type a size, then Enter")
                                                    fullHint: qsTr("Limit 4 — remove one first")
                                                    onAddRequested: function (value) {
                                                        win.updateCreateChoices("disk", win.diskChoiceDraft.concat([value]))
                                                    }
                                                    onRemoveRequested: function (index) {
                                                        const next = win.diskChoiceDraft.slice()
                                                        next.splice(index, 1)
                                                        win.updateCreateChoices("disk", next)
                                                    }
                                                }
                                            }

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 10
                                                Text {
                                                    Layout.preferredWidth: 120
                                                    Layout.alignment: Qt.AlignVCenter
                                                    text: qsTr("RAM limit")
                                                    color: theme.foreground
                                                    font.family: win.sansFont
                                                    font.pixelSize: 11
                                                }
                                                ChipInput {
                                                    Layout.fillWidth: true
                                                    values: win.ramChoiceDraft
                                                    unit: "GiB"
                                                    maxChips: 4
                                                    maxValue: 65536
                                                    placeholder: qsTr("Type a size, then Enter")
                                                    fullHint: qsTr("Limit 4 — remove one first")
                                                    onAddRequested: function (value) {
                                                        win.updateCreateChoices("ram", win.ramChoiceDraft.concat([value]))
                                                    }
                                                    onRemoveRequested: function (index) {
                                                        const next = win.ramChoiceDraft.slice()
                                                        next.splice(index, 1)
                                                        win.updateCreateChoices("ram", next)
                                                    }
                                                }
                                            }

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 10
                                                Text {
                                                    Layout.preferredWidth: 120
                                                    Layout.alignment: Qt.AlignVCenter
                                                    text: qsTr("CPU cores")
                                                    color: theme.foreground
                                                    font.family: win.sansFont
                                                    font.pixelSize: 11
                                                }
                                                ChipInput {
                                                    Layout.fillWidth: true
                                                    values: win.cpuChoiceDraft
                                                    maxChips: 4
                                                    maxValue: 1024
                                                    placeholder: qsTr("Type cores, then Enter")
                                                    fullHint: qsTr("Limit 4 — remove one first")
                                                    onAddRequested: function (value) {
                                                        win.updateCreateChoices("cpu", win.cpuChoiceDraft.concat([value]))
                                                    }
                                                    onRemoveRequested: function (index) {
                                                        const next = win.cpuChoiceDraft.slice()
                                                        next.splice(index, 1)
                                                        win.updateCreateChoices("cpu", next)
                                                    }
                                                }
                                            }

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 10
                                                Item { Layout.fillWidth: true }
                                                ActionButton {
                                                    label: qsTr("Save Options")
                                                    variant: "primary"
                                                    minWidth: 130
                                                    onClicked: win.saveQuotaOptions()
                                                }
                                            }
                                        }
                                    }

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10
                                        Item { Layout.fillWidth: true }
                                        ActionButton {
                                            label: qsTr("Restore Create Defaults")
                                            minWidth: 185
                                            onClicked: {
                                                const error = backend.resetCreateOptions()
                                                if (error.length > 0) {
                                                    win.showConfigNotice(error, true)
                                                } else {
                                                    win.syncCreateChoiceDrafts()
                                                    win.showConfigNotice(qsTr("Create options restored."), false)
                                                }
                                            }
                                        }
                                    }
                                }

                                // ==================================================
                                // SECTION 4: SAFETY & AUTO-SNAPSHOTS
                                // ==================================================
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    visible: win.settingsSection === 4
                                    spacing: 12

                                    Rectangle {
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: safetyCardCol.implicitHeight + 28
                                        radius: Math.min(6, theme.radius)
                                        color: win.surfaceInput
                                        border.width: 1
                                        border.color: win.borderSubtle

                                        ColumnLayout {
                                            id: safetyCardCol
                                            anchors.fill: parent
                                            anchors.margins: 14
                                            spacing: 10

                                            Text {
                                                text: qsTr("Automatic Btrfs Snapshots")
                                                color: theme.foreground
                                                font.family: win.sansFont
                                                font.pixelSize: 12
                                                font.weight: Font.Medium
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                text: qsTr("Creates an auto-<timestamp> snapshot before Enter OS Terminal and Start OS (at most once every 5 minutes). Manual snapshots are never pruned.")
                                                color: theme.muted
                                                font.family: win.sansFont
                                                font.pixelSize: 10
                                                wrapMode: Text.WordWrap
                                            }

                                            Rectangle {
                                                Layout.fillWidth: true
                                                height: 38
                                                radius: Math.min(4, theme.radius)
                                                color: win.surfaceCard
                                                border.width: 1
                                                border.color: win.autoSnapEnabledDraft ? theme.accent : win.borderSubtle

                                                RowLayout {
                                                    anchors.fill: parent
                                                    anchors.leftMargin: 12
                                                    anchors.rightMargin: 12
                                                    spacing: 10

                                                    Rectangle {
                                                        width: 16
                                                        height: 16
                                                        radius: 3
                                                        color: win.autoSnapEnabledDraft ? theme.accent : "transparent"
                                                        border.width: 1
                                                        border.color: win.autoSnapEnabledDraft ? theme.accent : theme.muted
                                                        Text {
                                                            anchors.centerIn: parent
                                                            visible: win.autoSnapEnabledDraft
                                                            text: "✓"
                                                            color: theme.accentForeground
                                                            font.family: win.monoFont
                                                            font.pixelSize: 11
                                                        }
                                                    }

                                                    Text {
                                                        Layout.fillWidth: true
                                                        text: qsTr("Enable auto-snapshots by default")
                                                        color: theme.foreground
                                                        font.family: win.sansFont
                                                        font.pixelSize: 12
                                                    }
                                                }

                                                MouseArea {
                                                    anchors.fill: parent
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: win.autoSnapEnabledDraft = !win.autoSnapEnabledDraft
                                                }
                                            }

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 10
                                                Text {
                                                    Layout.preferredWidth: 180
                                                    text: qsTr("Keep last auto-snapshots")
                                                    color: theme.foreground
                                                    font.family: win.sansFont
                                                    font.pixelSize: 11
                                                }
                                                FlatInput {
                                                    id: autoSnapKeepField
                                                    Layout.preferredWidth: 100
                                                    placeholder: "5"
                                                }
                                                Text {
                                                    Layout.fillWidth: true
                                                    text: qsTr("1 to 50 snapshots per environment")
                                                    color: theme.muted
                                                    font.family: win.sansFont
                                                    font.pixelSize: 10
                                                }
                                            }
                                        }
                                    }

                                    Rectangle {
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: watchdogCardCol.implicitHeight + 28
                                        radius: Math.min(6, theme.radius)
                                        color: win.surfaceInput
                                        border.width: 1
                                        border.color: win.borderSubtle

                                        ColumnLayout {
                                            id: watchdogCardCol
                                            anchors.fill: parent
                                            anchors.margins: 14
                                            spacing: 10

                                            Text {
                                                text: qsTr("Disk & Resource Watchdog")
                                                color: theme.foreground
                                                font.family: win.sansFont
                                                font.pixelSize: 12
                                                font.weight: Font.Medium
                                            }

                                            Rectangle {
                                                Layout.fillWidth: true
                                                height: 36
                                                radius: Math.min(4, theme.radius)
                                                color: win.surfaceCard
                                                border.width: 1
                                                border.color: win.watchdogEnabledDraft ? theme.accent : win.borderSubtle

                                                RowLayout {
                                                    anchors.fill: parent
                                                    anchors.leftMargin: 12
                                                    anchors.rightMargin: 12
                                                    spacing: 10

                                                    Rectangle {
                                                        width: 16
                                                        height: 16
                                                        radius: 3
                                                        color: win.watchdogEnabledDraft ? theme.accent : "transparent"
                                                        border.width: 1
                                                        border.color: win.watchdogEnabledDraft ? theme.accent : theme.muted
                                                        Text {
                                                            anchors.centerIn: parent
                                                            visible: win.watchdogEnabledDraft
                                                            text: "✓"
                                                            color: theme.accentForeground
                                                            font.family: win.monoFont
                                                            font.pixelSize: 11
                                                        }
                                                    }

                                                    Text {
                                                        Layout.fillWidth: true
                                                        text: qsTr("Send desktop alerts on high disk, CPU, or RAM")
                                                        color: theme.foreground
                                                        font.family: win.sansFont
                                                        font.pixelSize: 12
                                                    }
                                                }

                                                MouseArea {
                                                    anchors.fill: parent
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: win.watchdogEnabledDraft = !win.watchdogEnabledDraft
                                                }
                                            }

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 10
                                                Text {
                                                    Layout.preferredWidth: 180
                                                    text: qsTr("Disk warn / critical (%)")
                                                    color: theme.foreground
                                                    font.family: win.sansFont
                                                    font.pixelSize: 11
                                                }
                                                FlatInput {
                                                    id: watchdogWarnField
                                                    Layout.preferredWidth: 72
                                                    placeholder: "90"
                                                }
                                                FlatInput {
                                                    id: watchdogCritField
                                                    Layout.preferredWidth: 72
                                                    placeholder: "97"
                                                }
                                                Text {
                                                    Layout.fillWidth: true
                                                    text: qsTr("50-98 warn, > warn critical (5% hysteresis)")
                                                    color: theme.muted
                                                    font.family: win.sansFont
                                                    font.pixelSize: 10
                                                }
                                            }

                                            Rectangle {
                                                Layout.fillWidth: true
                                                height: 36
                                                radius: Math.min(4, theme.radius)
                                                color: win.surfaceCard
                                                border.width: 1
                                                border.color: win.watchdogAutoStopDraft ? theme.red : win.borderSubtle

                                                RowLayout {
                                                    anchors.fill: parent
                                                    anchors.leftMargin: 12
                                                    anchors.rightMargin: 12
                                                    spacing: 10

                                                    Rectangle {
                                                        width: 16
                                                        height: 16
                                                        radius: 3
                                                        color: win.watchdogAutoStopDraft ? theme.red : "transparent"
                                                        border.width: 1
                                                        border.color: win.watchdogAutoStopDraft ? theme.red : theme.muted
                                                        Text {
                                                            anchors.centerIn: parent
                                                            visible: win.watchdogAutoStopDraft
                                                            text: "✓"
                                                            color: theme.accentForeground
                                                            font.family: win.monoFont
                                                            font.pixelSize: 11
                                                        }
                                                    }

                                                    Text {
                                                        Layout.fillWidth: true
                                                        text: qsTr("Auto-stop running containers by default on critical disk")
                                                        color: theme.foreground
                                                        font.family: win.sansFont
                                                        font.pixelSize: 12
                                                    }
                                                }

                                                MouseArea {
                                                    anchors.fill: parent
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: win.watchdogAutoStopDraft = !win.watchdogAutoStopDraft
                                                }
                                            }
                                        }
                                    }

                                    Rectangle {
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: idleStopCardCol.implicitHeight + 28
                                        radius: Math.min(6, theme.radius)
                                        color: win.surfaceInput
                                        border.width: 1
                                        border.color: win.borderSubtle

                                        ColumnLayout {
                                            id: idleStopCardCol
                                            anchors.fill: parent
                                            anchors.margins: 14
                                            spacing: 8

                                            Text {
                                                text: qsTr("Idle Auto-Stop (Opt-in)")
                                                color: theme.foreground
                                                font.family: win.sansFont
                                                font.pixelSize: 12
                                                font.weight: Font.Medium
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                wrapMode: Text.WordWrap
                                                text: qsTr("Automatically stops running non-template containers after sustained <3% CPU with no active incus exec session or port forwards (1 min warning notification beforehand). Works only while the AIBox GUI is running — no background daemon is used.")
                                                color: theme.muted
                                                font.family: win.sansFont
                                                font.pixelSize: 11
                                            }

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 8

                                                Repeater {
                                                    model: backend.idleStopOptions
                                                    delegate: Rectangle {
                                                        required property int modelData
                                                        readonly property bool selected: win.idleStopMinutesDraft === modelData
                                                        height: 34
                                                        width: idleChipTxt.implicitWidth + 20
                                                        radius: Math.min(4, theme.radius)
                                                        color: selected ? win.surfaceSelected : win.surfaceCard
                                                        border.width: 1
                                                        border.color: selected ? theme.accent : win.borderSubtle

                                                        Text {
                                                            id: idleChipTxt
                                                            anchors.centerIn: parent
                                                            text: modelData === 0 ? qsTr("Off") : qsTr("%1 min").arg(modelData)
                                                            color: selected ? theme.accent : theme.muted
                                                            font.family: win.monoFont
                                                            font.pixelSize: 11
                                                        }

                                                        MouseArea {
                                                            anchors.fill: parent
                                                            cursorShape: Qt.PointingHandCursor
                                                            onClicked: win.idleStopMinutesDraft = modelData
                                                        }
                                                    }
                                                }
                                                Item { Layout.fillWidth: true }
                                            }
                                        }
                                    }

                                    Rectangle {
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: staleCardCol.implicitHeight + 28
                                        radius: Math.min(6, theme.radius)
                                        color: win.surfaceInput
                                        border.width: 1
                                        border.color: win.borderSubtle

                                        ColumnLayout {
                                            id: staleCardCol
                                            anchors.fill: parent
                                            anchors.margins: 14
                                            spacing: 8

                                            RowLayout {
                                                Layout.fillWidth: true
                                                Text {
                                                    text: qsTr("Stale Environment Report (U)")
                                                    color: theme.foreground
                                                    font.family: win.sansFont
                                                    font.pixelSize: 12
                                                    font.weight: Font.Medium
                                                }
                                                Item { Layout.fillWidth: true }
                                                ActionButton {
                                                    label: qsTr("Open Report")
                                                    shortcutHint: "U"
                                                    minWidth: 122
                                                    onClicked: {
                                                        win.settingsVisible = false
                                                        win.openStaleReportDialog()
                                                    }
                                                }
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                wrapMode: Text.WordWrap
                                                text: qsTr("Flags non-template environments unused for at least staleDays days (plus unknown-age environments without a usage timestamp) and scans orphaned workspaces, snapshot dirs, and unpaired backup archives.")
                                                color: theme.muted
                                                font.family: win.sansFont
                                                font.pixelSize: 11
                                            }

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 8

                                                Repeater {
                                                    model: backend.staleDayOptions
                                                    delegate: Rectangle {
                                                        required property int modelData
                                                        readonly property bool selected: win.staleDaysDraft === modelData
                                                        height: 34
                                                        width: staleDayTxt.implicitWidth + 20
                                                        radius: Math.min(4, theme.radius)
                                                        color: selected ? win.surfaceSelected : win.surfaceCard
                                                        border.width: 1
                                                        border.color: selected ? theme.accent : win.borderSubtle

                                                        Text {
                                                            id: staleDayTxt
                                                            anchors.centerIn: parent
                                                            text: qsTr("%1 d").arg(modelData)
                                                            color: selected ? theme.accent : theme.muted
                                                            font.family: win.monoFont
                                                            font.pixelSize: 11
                                                        }

                                                        MouseArea {
                                                            anchors.fill: parent
                                                            cursorShape: Qt.PointingHandCursor
                                                            onClicked: win.staleDaysDraft = modelData
                                                        }
                                                    }
                                                }
                                                Item { Layout.fillWidth: true }
                                            }
                                        }
                                    }

                                    Rectangle {
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: recCardCol.implicitHeight + 28
                                        radius: Math.min(6, theme.radius)
                                        color: win.surfaceInput
                                        border.width: 1
                                        border.color: win.borderSubtle

                                        ColumnLayout {
                                            id: recCardCol
                                            anchors.fill: parent
                                            anchors.margins: 14
                                            spacing: 8

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 10
                                                Text {
                                                    text: qsTr("Session Recording (Opt-in)")
                                                    color: theme.foreground
                                                    font.family: win.sansFont
                                                    font.pixelSize: 12
                                                    font.weight: Font.Medium
                                                }
                                                Item { Layout.fillWidth: true }
                                                Rectangle {
                                                    height: 28
                                                    width: recToggleTxt.implicitWidth + 22
                                                    radius: Math.min(4, theme.radius)
                                                    color: win.recordSessionsDraft ? win.surfaceSelected : win.surfaceCard
                                                    border.width: 1
                                                    border.color: win.recordSessionsDraft ? theme.accent : win.borderSubtle

                                                    Text {
                                                        id: recToggleTxt
                                                        anchors.centerIn: parent
                                                        text: win.recordSessionsDraft ? qsTr("Enabled") : qsTr("Disabled")
                                                        color: win.recordSessionsDraft ? theme.accent : theme.muted
                                                        font.family: win.monoFont
                                                        font.pixelSize: 11
                                                    }

                                                    MouseArea {
                                                        anchors.fill: parent
                                                        cursorShape: Qt.PointingHandCursor
                                                        onClicked: win.recordSessionsDraft = !win.recordSessionsDraft
                                                    }
                                                }
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                wrapMode: Text.WordWrap
                                                text: qsTr("Records interactive Enter shells using script -q -f with timing files, automatically scrubs S3 secrets after exit, and retains the last 20 sessions per environment (200 MB total cap). View or replay sessions under History (H) -> Sessions.")
                                                color: theme.muted
                                                font.family: win.sansFont
                                                font.pixelSize: 11
                                            }
                                        }
                                    }

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10
                                        Item { Layout.fillWidth: true }
                                        ActionButton {
                                            label: qsTr("Save Safety Settings")
                                            variant: "primary"
                                            minWidth: 165
                                            onClicked: {
                                                const parsedKeep = parseInt(autoSnapKeepField.text.trim(), 10)
                                                const keep = isNaN(parsedKeep) ? 0 : parsedKeep
                                                const snapErr = backend.saveAutoSnapshotSettings(win.autoSnapEnabledDraft, keep)
                                                if (snapErr.length > 0) {
                                                    win.showConfigNotice(snapErr, true)
                                                    return
                                                }
                                                const parsedWarn = parseInt(watchdogWarnField.text.trim(), 10)
                                                const parsedCrit = parseInt(watchdogCritField.text.trim(), 10)
                                                const wdErr = backend.saveWatchdogSettings(
                                                    win.watchdogEnabledDraft,
                                                    isNaN(parsedWarn) ? 0 : parsedWarn,
                                                    isNaN(parsedCrit) ? 0 : parsedCrit,
                                                    win.watchdogAutoStopDraft)
                                                if (wdErr.length > 0) {
                                                    win.showConfigNotice(wdErr, true)
                                                    return
                                                }
                                                const idleErr = backend.saveIdleStopSettings(win.idleStopMinutesDraft)
                                                if (idleErr.length > 0) {
                                                    win.showConfigNotice(idleErr, true)
                                                    return
                                                }
                                                const staleErr = backend.saveStaleSettings(win.staleDaysDraft)
                                                if (staleErr.length > 0) {
                                                    win.showConfigNotice(staleErr, true)
                                                    return
                                                }
                                                const recErr = backend.saveSessionRecordingSettings(win.recordSessionsDraft)
                                                if (recErr.length > 0) {
                                                    win.showConfigNotice(recErr, true)
                                                    return
                                                }
                                                autoSnapKeepField.text = String(backend.autoSnapshotKeep)
                                                watchdogWarnField.text = String(backend.watchdogDiskWarn)
                                                watchdogCritField.text = String(backend.watchdogDiskCritical)
                                                win.idleStopMinutesDraft = backend.idleStopMinutes
                                                win.staleDaysDraft = backend.staleDays
                                                win.recordSessionsDraft = backend.recordSessions
                                                win.showConfigNotice(qsTr("Safety settings saved."), false)
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ======================================================================
        // MODAL 5: FIRST-RUN SYSTEM STORAGE SETUP
        // ======================================================================
        Rectangle {
            id: firstRunOverlay
            anchors.fill: parent
            visible: backend.firstRun
            color: "#e006070a"
            z: 100

            onVisibleChanged: {
                if (visible) {
                    win.firstRunError = ""
                    firstRunPathField.text = backend.systemPath.length > 0 ? backend.systemPath : backend.defaultSystemPath
                }
            }

            MouseArea { anchors.fill: parent }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(parent.width - 60, 600)
                height: firstRunColumn.implicitHeight + 48
                radius: Math.min(8, theme.radius)
                color: win.surfaceCard
                border.width: 1
                border.color: theme.accent

                ColumnLayout {
                    id: firstRunColumn
                    anchors.fill: parent
                    anchors.margins: 24
                    spacing: 14

                    readonly property string resolvedPreview: {
                        const raw = firstRunPathField.text.trim()
                        const base = raw.length > 0 ? raw : backend.defaultSystemPath
                        return base.length > 1 && base.endsWith("/") ? base.slice(0, -1) : base
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12

                        Rectangle {
                            width: 40
                            height: 40
                            radius: Math.min(6, theme.radius)
                            color: win.surfaceSelected
                            border.width: 1
                            border.color: theme.accent
                            Text {
                                anchors.centerIn: parent
                                text: ">_"
                                color: theme.accent
                                font.family: win.monoFont
                                font.pixelSize: 14
                            }
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            spacing: 2
                            Text {
                                Layout.fillWidth: true
                                text: qsTr("Welcome to AIBox")
                                color: theme.foreground
                                font.family: win.sansFont
                                font.pixelSize: 18
                                font.weight: Font.Medium
                            }
                            Text {
                                Layout.fillWidth: true
                                wrapMode: Text.WordWrap
                                text: qsTr("Choose where AIBox stores its system data, shared workspaces, OS images, and backups. You can keep the default or pick a custom location.")
                                color: theme.muted
                                font.family: win.sansFont
                                font.pixelSize: 11
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        height: 1
                        color: win.borderSubtle
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        Text {
                            Layout.fillWidth: true
                            text: qsTr("System Storage Location")
                            color: theme.foreground
                            font.family: win.sansFont
                            font.pixelSize: 12
                            font.weight: Font.Medium
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 10

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
                            implicitHeight: previewGrid.implicitHeight + 20
                            radius: Math.min(4, theme.radius)
                            color: win.surfaceInput
                            border.width: 1
                            border.color: win.borderSubtle

                            ColumnLayout {
                                id: previewGrid
                                anchors.fill: parent
                                anchors.margins: 10
                                spacing: 4

                                Text {
                                    Layout.fillWidth: true
                                    Layout.minimumWidth: 0
                                    elide: Text.ElideMiddle
                                    text: qsTr("Workspaces:  %1/workspaces").arg(firstRunColumn.resolvedPreview)
                                    color: theme.muted
                                    font.family: win.monoFont
                                    font.pixelSize: 10
                                }
                                Text {
                                    Layout.fillWidth: true
                                    Layout.minimumWidth: 0
                                    elide: Text.ElideMiddle
                                    text: qsTr("OS Images:   %1/os-images").arg(firstRunColumn.resolvedPreview)
                                    color: theme.muted
                                    font.family: win.monoFont
                                    font.pixelSize: 10
                                }
                                Text {
                                    Layout.fillWidth: true
                                    Layout.minimumWidth: 0
                                    elide: Text.ElideMiddle
                                    text: qsTr("State & Cfg: %1/state, config.json").arg(firstRunColumn.resolvedPreview)
                                    color: theme.muted
                                    font.family: win.monoFont
                                    font.pixelSize: 10
                                }
                            }
                        }
                    }

                    Rectangle {
                        visible: win.firstRunError.length > 0
                        Layout.fillWidth: true
                        implicitHeight: firstRunErrTxt.implicitHeight + 14
                        radius: Math.min(4, theme.radius)
                        color: "#261319"
                        border.width: 1
                        border.color: theme.red
                        Text {
                            id: firstRunErrTxt
                            anchors.fill: parent
                            anchors.margins: 7
                            wrapMode: Text.WordWrap
                            text: win.firstRunError
                            color: theme.red
                            font.family: win.sansFont
                            font.pixelSize: 11
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.topMargin: 4
                        spacing: 10
                        Item { Layout.fillWidth: true }
                        ActionButton {
                            label: qsTr("Continue")
                            variant: "primary"
                            minWidth: 128
                            onClicked: win.submitFirstRun()
                        }
                    }
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
        Rectangle {
            id: historyOverlay
            anchors.fill: parent
            visible: win.historyVisible
            color: "#c406070a"
            z: 85

            MouseArea {
                anchors.fill: parent
                onClicked: win.historyVisible = false
            }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(parent.width - 56, 980)
                height: Math.min(parent.height - 56, 620)
                radius: Math.min(8, theme.radius)
                color: win.surfaceCard
                border.width: 1
                border.color: theme.accent
                clip: true

                MouseArea { anchors.fill: parent }

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 24
                    spacing: 14

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 10

                        Text {
                            text: win.historyTab === 0 ? qsTr("Operation History") : qsTr("Recorded Sessions")
                            color: theme.foreground
                            font.family: win.sansFont
                            font.pixelSize: 16
                            font.weight: Font.Medium
                        }

                        // Segmented switcher: Tasks / Sessions (H3)
                        Rectangle {
                            height: 34
                            width: histTabRow.implicitWidth + 8
                            radius: Math.min(4, theme.radius)
                            color: win.surfaceInput
                            border.width: 1
                            border.color: win.borderSubtle

                            RowLayout {
                                id: histTabRow
                                anchors.centerIn: parent
                                spacing: 4
                                Repeater {
                                    model: [
                                        { tab: 0, label: qsTr("Tasks (%1)").arg(backend.operationHistory.length) },
                                        { tab: 1, label: qsTr("Sessions (%1)").arg(backend.recordedSessions.length) }
                                    ]
                                    delegate: Rectangle {
                                        required property var modelData
                                        height: 28
                                        width: hTabTxt.implicitWidth + 14
                                        radius: Math.min(3, theme.radius)
                                        color: win.historyTab === modelData.tab ? win.surfaceSelected : "transparent"
                                        border.width: win.historyTab === modelData.tab ? 1 : 0
                                        border.color: theme.accent
                                        Text {
                                            id: hTabTxt
                                            anchors.centerIn: parent
                                            text: modelData.label
                                            color: win.historyTab === modelData.tab ? theme.foreground : theme.muted
                                            font.family: win.sansFont
                                            font.pixelSize: 11
                                        }
                                        MouseArea {
                                            anchors.fill: parent
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: {
                                                win.historyTab = modelData.tab
                                                if (modelData.tab === 1) {
                                                    backend.refreshRecordedSessions()
                                                    if (win.selectedSessionId.length === 0 && backend.recordedSessions.length > 0)
                                                        win.selectRecordedSession(backend.recordedSessions[0])
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        Item { Layout.fillWidth: true }

                        ActionButton {
                            visible: win.historyTab === 0
                            label: qsTr("Clear History")
                            danger: true
                            minWidth: 112
                            disabled: backend.operationHistory.length === 0
                            onClicked: {
                                backend.clearOperationHistory()
                                win.selectedHistoryId = ""
                                win.selectedHistoryLog = ""
                            }
                        }

                        ActionButton {
                            visible: win.historyTab === 1
                            label: qsTr("Replay in terminal")
                            variant: "primary"
                            minWidth: 146
                            disabled: !win.selectedSessionHasTiming
                            onClicked: backend.replayRecordedSession(win.selectedSessionLogPath, win.selectedSessionTimingPath)
                        }

                        ActionButton {
                            label: qsTr("Close")
                            shortcutHint: "Esc"
                            minWidth: 88
                            onClicked: win.historyVisible = false
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        height: 1
                        color: win.borderSubtle
                    }

                    RowLayout {
                        visible: win.historyTab === 0
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        spacing: 14

                        // Left pane: History Entries
                        Rectangle {
                            Layout.preferredWidth: 360
                            Layout.fillHeight: true
                            radius: Math.min(4, theme.radius)
                            color: win.surfaceInput
                            border.width: 1
                            border.color: win.borderSubtle

                            Text {
                                anchors.centerIn: parent
                                visible: backend.operationHistory.length === 0
                                text: qsTr("No recorded operations yet.")
                                color: theme.muted
                                font.family: win.sansFont
                                font.pixelSize: 12
                            }

                            ListView {
                                id: historyListView
                                anchors.fill: parent
                                anchors.margins: 8
                                clip: true
                                spacing: 6
                                model: backend.operationHistory
                                delegate: Rectangle {
                                    required property var modelData
                                    readonly property bool selected: (win.selectedHistoryId === (modelData.id || ""))
                                    width: historyListView.width
                                    height: histItemCol.implicitHeight + 16
                                    radius: Math.min(4, theme.radius)
                                    color: selected ? win.surfaceSelected : win.surfaceCard
                                    border.width: 1
                                    border.color: selected ? theme.accent : win.borderSubtle

                                    ColumnLayout {
                                        id: histItemCol
                                        anchors.fill: parent
                                        anchors.margins: 8
                                        spacing: 4

                                        RowLayout {
                                            Layout.fillWidth: true
                                            spacing: 6

                                            Rectangle {
                                                height: 18
                                                width: histBadgeTxt.implicitWidth + 10
                                                radius: 2
                                                color: modelData.status === "failed"
                                                       ? "#2c1419"
                                                       : (modelData.status === "cancelled" ? "#262014" : "#162920")
                                                border.width: 1
                                                border.color: modelData.status === "failed"
                                                              ? theme.red
                                                              : (modelData.status === "cancelled" ? "#e5a93c" : theme.green)
                                                Text {
                                                    id: histBadgeTxt
                                                    anchors.centerIn: parent
                                                    text: modelData.status === "failed"
                                                          ? qsTr("FAILED")
                                                          : (modelData.status === "cancelled" ? qsTr("CANCELLED") : qsTr("OK"))
                                                    color: modelData.status === "failed"
                                                           ? theme.red
                                                           : (modelData.status === "cancelled" ? "#e5a93c" : theme.green)
                                                    font.family: win.monoFont
                                                    font.pixelSize: 9
                                                    font.weight: Font.Medium
                                                }
                                            }

                                            Rectangle {
                                                visible: (modelData.reason || "") === "idle"
                                                height: 18
                                                width: idleReasonTxt.implicitWidth + 10
                                                radius: 2
                                                color: "#262014"
                                                border.width: 1
                                                border.color: "#e5a93c"
                                                Text {
                                                    id: idleReasonTxt
                                                    anchors.centerIn: parent
                                                    text: qsTr("IDLE")
                                                    color: "#e5a93c"
                                                    font.family: win.monoFont
                                                    font.pixelSize: 9
                                                    font.weight: Font.Medium
                                                }
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                elide: Text.ElideRight
                                                text: modelData.envName || ""
                                                color: theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 10
                                            }

                                            Text {
                                                text: modelData.durationMs !== undefined
                                                      ? (modelData.durationMs >= 1000
                                                         ? (Math.round(modelData.durationMs / 100) / 10) + "s"
                                                         : modelData.durationMs + "ms")
                                                      : ""
                                                color: theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 10
                                            }
                                        }

                                        Text {
                                            Layout.fillWidth: true
                                            elide: Text.ElideRight
                                            text: modelData.description || ""
                                            color: theme.foreground
                                            font.family: win.sansFont
                                            font.pixelSize: 12
                                        }

                                        Text {
                                            Layout.fillWidth: true
                                            elide: Text.ElideRight
                                            text: modelData.timestamp || (modelData.startedAt || "").replace("T", " ")
                                            color: theme.muted
                                            font.family: win.monoFont
                                            font.pixelSize: 10
                                        }
                                    }

                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: {
                                            win.selectedHistoryId = modelData.id || ""
                                            win.selectedHistoryLog = backend.readOperationLog(win.selectedHistoryId)
                                        }
                                    }
                                }
                            }
                        }

                        // Right pane: Full Task Log
                        Rectangle {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            radius: Math.min(4, theme.radius)
                            color: win.surfaceInput
                            border.width: 1
                            border.color: win.borderSubtle

                            ScrollView {
                                anchors.fill: parent
                                anchors.margins: 10
                                clip: true

                                TextArea {
                                    readOnly: true
                                    selectByMouse: true
                                    wrapMode: TextEdit.WrapAnywhere
                                    text: win.selectedHistoryLog.length > 0
                                          ? win.selectedHistoryLog
                                          : qsTr("Select an operation to inspect its log output.")
                                    color: win.selectedHistoryLog.length > 0 ? theme.foreground : theme.muted
                                    font.family: win.monoFont
                                    font.pixelSize: 11
                                    background: null
                                }
                            }
                        }
                    }

                    // Sessions Tab Content (H3)
                    RowLayout {
                        visible: win.historyTab === 1
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        spacing: 14

                        Rectangle {
                            Layout.preferredWidth: 340
                            Layout.fillHeight: true
                            radius: Math.min(4, theme.radius)
                            color: win.surfaceInput
                            border.width: 1
                            border.color: win.borderSubtle

                            Text {
                                anchors.centerIn: parent
                                width: parent.width - 24
                                horizontalAlignment: Text.AlignHCenter
                                wrapMode: Text.WordWrap
                                visible: backend.recordedSessions.length === 0
                                text: qsTr("No recorded interactive sessions yet. Enable Session Recording in Settings -> Safety or per environment.")
                                color: theme.muted
                                font.family: win.sansFont
                                font.pixelSize: 12
                            }

                            ListView {
                                id: sessionsListView
                                anchors.fill: parent
                                anchors.margins: 8
                                clip: true
                                spacing: 6
                                model: backend.recordedSessions
                                delegate: Rectangle {
                                    required property var modelData
                                    readonly property bool selected: (win.selectedSessionId === (modelData.id || ""))
                                    width: sessionsListView.width
                                    height: sessItemCol.implicitHeight + 16
                                    radius: Math.min(4, theme.radius)
                                    color: selected ? win.surfaceSelected : win.surfaceCard
                                    border.width: 1
                                    border.color: selected ? theme.accent : win.borderSubtle

                                    ColumnLayout {
                                        id: sessItemCol
                                        anchors.fill: parent
                                        anchors.margins: 8
                                        spacing: 4

                                        RowLayout {
                                            Layout.fillWidth: true
                                            spacing: 6
                                            Text {
                                                text: modelData.envName || ""
                                                color: theme.foreground
                                                font.family: win.monoFont
                                                font.pixelSize: 12
                                                font.weight: Font.Medium
                                                elide: Text.ElideRight
                                                Layout.fillWidth: true
                                            }
                                            Text {
                                                text: modelData.sizeDisplay || ""
                                                color: theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 10
                                            }
                                            Rectangle {
                                                height: 20
                                                width: 22
                                                radius: 3
                                                color: delSessMouse.containsMouse ? "#2c1419" : win.surfaceInput
                                                border.width: 1
                                                border.color: delSessMouse.containsMouse ? theme.red : win.borderSubtle
                                                Text {
                                                    anchors.centerIn: parent
                                                    text: "x"
                                                    color: delSessMouse.containsMouse ? theme.red : theme.muted
                                                    font.family: win.monoFont
                                                    font.pixelSize: 10
                                                }
                                                MouseArea {
                                                    id: delSessMouse
                                                    anchors.fill: parent
                                                    hoverEnabled: true
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: {
                                                        const wasSelected = (win.selectedSessionId === (modelData.id || ""))
                                                        backend.deleteRecordedSession(modelData.logPath || "", modelData.timingPath || "")
                                                        if (wasSelected) {
                                                            if (backend.recordedSessions.length > 0)
                                                                win.selectRecordedSession(backend.recordedSessions[0])
                                                            else
                                                                win.selectRecordedSession(null)
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        RowLayout {
                                            Layout.fillWidth: true
                                            spacing: 6
                                            Text {
                                                Layout.fillWidth: true
                                                elide: Text.ElideRight
                                                text: modelData.timestamp || ""
                                                color: theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 10
                                            }
                                            Text {
                                                visible: modelData.hasTiming || false
                                                text: qsTr("TIMED")
                                                color: theme.accent
                                                font.family: win.monoFont
                                                font.pixelSize: 9
                                            }
                                        }
                                    }

                                    MouseArea {
                                        anchors.fill: parent
                                        z: -1
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: win.selectRecordedSession(modelData)
                                    }
                                }
                            }
                        }

                        Rectangle {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            radius: Math.min(4, theme.radius)
                            color: win.surfaceInput
                            border.width: 1
                            border.color: win.borderSubtle

                            ScrollView {
                                anchors.fill: parent
                                anchors.margins: 10
                                clip: true

                                TextArea {
                                    readOnly: true
                                    selectByMouse: true
                                    wrapMode: TextEdit.WrapAnywhere
                                    text: win.selectedSessionText.length > 0
                                          ? win.selectedSessionText
                                          : qsTr("Select a recorded session to view ANSI-stripped terminal output (200 KB cap).")
                                    color: win.selectedSessionText.length > 0 ? theme.foreground : theme.muted
                                    font.family: win.monoFont
                                    font.pixelSize: 11
                                    background: null
                                }
                            }
                        }
                    }
                }
            }
        }

        // ======================================================================
        // MODAL 7: NETWORK EGRESS LOG VIEWER (G3)
        // ======================================================================
        Rectangle {
            id: egressOverlay
            anchors.fill: parent
            visible: win.egressVisible
            color: "#c406070a"
            z: 86

            MouseArea {
                anchors.fill: parent
                onClicked: win.egressVisible = false
            }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(parent.width - 56, 780)
                height: Math.min(parent.height - 56, 560)
                radius: Math.min(8, theme.radius)
                color: win.surfaceCard
                border.width: 1
                border.color: theme.accent
                clip: true

                MouseArea { anchors.fill: parent }

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 24
                    spacing: 12

                    ModalHeader {
                        title: qsTr("Network Egress Log · %1").arg(backend.egressLogsEnvName)
                        subtitle: qsTr("Shows outgoing destinations captured from kernel bridge ACL logs")
                        onClosed: win.egressVisible = false

                        ActionButton {
                            label: qsTr("Refresh Log")
                            minWidth: 104
                            onClicked: backend.refreshEgressLog(backend.egressLogsEnvName)
                        }
                    }

                    // Table header
                    RowLayout {
                        Layout.fillWidth: true
                        Layout.leftMargin: 10
                        Layout.rightMargin: 10
                        spacing: 10
                        Text {
                            Layout.fillWidth: true
                            text: qsTr("Destination IP")
                            color: theme.muted
                            font.family: win.sansFont
                            font.pixelSize: 11
                        }
                        Text {
                            Layout.preferredWidth: 72
                            text: qsTr("Proto")
                            color: theme.muted
                            font.family: win.sansFont
                            font.pixelSize: 11
                        }
                        Text {
                            Layout.preferredWidth: 72
                            text: qsTr("Port")
                            color: theme.muted
                            font.family: win.sansFont
                            font.pixelSize: 11
                        }
                        Text {
                            Layout.preferredWidth: 64
                            text: qsTr("Hits")
                            color: theme.muted
                            font.family: win.sansFont
                            font.pixelSize: 11
                        }
                        Text {
                            Layout.preferredWidth: 88
                            text: qsTr("Last Seen")
                            color: theme.muted
                            font.family: win.sansFont
                            font.pixelSize: 11
                        }
                        Item { Layout.preferredWidth: 96 }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        radius: Math.min(4, theme.radius)
                        color: win.surfaceInput
                        border.width: 1
                        border.color: win.borderSubtle

                        Text {
                            anchors.centerIn: parent
                            visible: backend.egressLogs.length === 0
                            text: qsTr("No egress entries captured yet. Enable 'Record: ON' on the environment and generate traffic.")
                            color: theme.muted
                            font.family: win.sansFont
                            font.pixelSize: 12
                            horizontalAlignment: Text.AlignHCenter
                            width: parent.width - 40
                            wrapMode: Text.WordWrap
                        }

                        ListView {
                            id: egressListView
                            anchors.fill: parent
                            anchors.margins: 8
                            clip: true
                            spacing: 6
                            model: backend.egressLogs
                            delegate: Rectangle {
                                required property var modelData
                                width: egressListView.width
                                height: 36
                                radius: Math.min(3, theme.radius)
                                color: win.surfaceCard
                                border.width: 1
                                border.color: win.borderSubtle

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 10
                                    anchors.rightMargin: 8
                                    spacing: 10

                                    Text {
                                        Layout.fillWidth: true
                                        text: modelData.dstIp || ""
                                        color: theme.foreground
                                        font.family: win.monoFont
                                        font.pixelSize: 12
                                    }
                                    Text {
                                        Layout.preferredWidth: 72
                                        text: modelData.proto || ""
                                        color: theme.muted
                                        font.family: win.monoFont
                                        font.pixelSize: 11
                                    }
                                    Text {
                                        Layout.preferredWidth: 72
                                        text: modelData.port || "—"
                                        color: theme.foreground
                                        font.family: win.monoFont
                                        font.pixelSize: 11
                                    }
                                    Text {
                                        Layout.preferredWidth: 64
                                        text: String(modelData.count || 0)
                                        color: theme.accent
                                        font.family: win.monoFont
                                        font.pixelSize: 11
                                    }
                                    Text {
                                        Layout.preferredWidth: 88
                                        text: modelData.lastSeen || "—"
                                        color: theme.muted
                                        font.family: win.monoFont
                                        font.pixelSize: 11
                                    }
                                    ActionButton {
                                        implicitHeight: 24
                                        minWidth: 92
                                        label: qsTr("+ Allow")
                                        disabled: backend.busy
                                        onClicked: backend.addEgressDestinationToAllowlist(
                                            backend.egressLogsEnvName, modelData.dstIp || "")
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        FolderDialog {
            id: workspaceFolderDialog
            title: qsTr("Choose Workspace Location")
            currentFolder: backend.pathToUrl(backend.workspaceBase)
            onAccepted: workspacePathField.text = backend.localPath(selectedFolder)
        }

        FolderDialog {
            id: osStorageFolderDialog
            title: qsTr("Choose OS Image Storage Location")
            currentFolder: backend.pathToUrl(backend.osStoragePath)
            onAccepted: osStoragePathField.text = backend.localPath(selectedFolder)
        }

        // ======================================================================
        // MODAL 8: PORT FORWARDING (W1)
        // ======================================================================
        Rectangle {
            id: portsOverlay
            anchors.fill: parent
            visible: win.portsVisible
            color: "#c406070a"
            z: 87

            MouseArea {
                anchors.fill: parent
                onClicked: win.portsVisible = false
            }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(parent.width - 56, 680)
                height: Math.min(parent.height - 56, 520)
                radius: Math.min(8, theme.radius)
                color: win.surfaceCard
                border.width: 1
                border.color: theme.accent
                clip: true

                MouseArea { anchors.fill: parent }

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 24
                    spacing: 12

                    ModalHeader {
                        title: qsTr("Port Forwarding · %1").arg(backend.selectedEnvironment.name || "")
                        subtitle: qsTr("Binds container ports to 127.0.0.1 on Host only (never 0.0.0.0)")
                        onClosed: win.portsVisible = false
                    }

                    Rectangle {
                        visible: win.portForwardError.length > 0
                        Layout.fillWidth: true
                        height: 34
                        radius: Math.min(4, theme.radius)
                        color: "#261319"
                        border.width: 1
                        border.color: theme.red
                        Text {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 12
                            verticalAlignment: Text.AlignVCenter
                            elide: Text.ElideRight
                            text: win.portForwardError
                            color: theme.red
                            font.family: win.sansFont
                            font.pixelSize: 11
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 10
                        FlatInput {
                            id: hostPortInput
                            Layout.preferredWidth: 145
                            implicitHeight: 34
                            placeholder: qsTr("Host port (e.g. 3000)")
                        }
                        Text {
                            text: "→"
                            color: theme.muted
                            font.family: win.monoFont
                            font.pixelSize: 13
                        }
                        FlatInput {
                            id: guestPortInput
                            Layout.preferredWidth: 155
                            implicitHeight: 34
                            placeholder: qsTr("Container port (e.g. 3000)")
                        }
                        Item { Layout.fillWidth: true }
                        ActionButton {
                            label: qsTr("+ Add Forward")
                            variant: "primary"
                            minWidth: 132
                            disabled: backend.busy
                            onClicked: {
                                const hp = parseInt(hostPortInput.text.trim(), 10)
                                const gp = parseInt(guestPortInput.text.trim(), 10)
                                const err = backend.addPortForward(
                                    backend.selectedEnvironment.name || "",
                                    isNaN(hp) ? 0 : hp,
                                    isNaN(gp) ? (isNaN(hp) ? 0 : hp) : gp)
                                if (err.length > 0) {
                                    win.portForwardError = err
                                } else {
                                    win.portForwardError = ""
                                    hostPortInput.text = ""
                                    guestPortInput.text = ""
                                }
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        radius: Math.min(4, theme.radius)
                        color: win.surfaceInput
                        border.width: 1
                        border.color: win.borderSubtle

                        Text {
                            anchors.centerIn: parent
                            visible: !backend.selectedEnvironment.portForwards || backend.selectedEnvironment.portForwards.length === 0
                            text: qsTr("No forwarded ports configured for this environment.")
                            color: theme.muted
                            font.family: win.sansFont
                            font.pixelSize: 12
                        }

                        ListView {
                            id: portsListView
                            anchors.fill: parent
                            anchors.margins: 8
                            clip: true
                            spacing: 6
                            model: backend.selectedEnvironment.portForwards || []
                            delegate: Rectangle {
                                required property var modelData
                                width: portsListView.width
                                height: 48
                                radius: Math.min(3, theme.radius)
                                color: win.surfaceCard
                                border.width: 1
                                border.color: win.borderSubtle

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 12
                                    anchors.rightMargin: 12
                                    spacing: 10
                                    Text {
                                        Layout.fillWidth: true
                                        text: "127.0.0.1:" + modelData.hostPort + "  →  " + modelData.guestPort
                                        color: theme.foreground
                                        font.family: win.monoFont
                                        font.pixelSize: 12
                                    }
                                    ActionButton {
                                        minWidth: 78
                                        label: qsTr("Open")
                                        onClicked: backend.openForwardedPort(modelData.hostPort)
                                    }
                                    ActionButton {
                                        minWidth: 84
                                        label: qsTr("Remove")
                                        danger: true
                                        disabled: backend.busy
                                        onClicked: backend.removePortForward(
                                            backend.selectedEnvironment.name || "", modelData.deviceName || "")
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ======================================================================
        // MODAL 9: EXTRA MOUNTS (W2)
        // ======================================================================
        Rectangle {
            id: mountsOverlay
            anchors.fill: parent
            visible: win.mountsVisible
            color: "#c406070a"
            z: 88

            MouseArea {
                anchors.fill: parent
                onClicked: win.mountsVisible = false
            }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(parent.width - 56, 760)
                height: Math.min(parent.height - 56, 560)
                radius: Math.min(8, theme.radius)
                color: win.surfaceCard
                border.width: 1
                border.color: theme.accent
                clip: true

                MouseArea { anchors.fill: parent }

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 24
                    spacing: 12

                    ModalHeader {
                        title: qsTr("Extra Host Mounts · %1").arg(backend.selectedEnvironment.name || "")
                        subtitle: qsTr("Mounts additional folders under /home/workspace/<name> (Read-Only by default; system/sensitive paths blocked)")
                        onClosed: win.mountsVisible = false
                    }

                    Rectangle {
                        visible: win.extraMountError.length > 0
                        Layout.fillWidth: true
                        height: 36
                        radius: Math.min(4, theme.radius)
                        color: "#261319"
                        border.width: 1
                        border.color: theme.red
                        Text {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 12
                            verticalAlignment: Text.AlignVCenter
                            elide: Text.ElideRight
                            text: win.extraMountError
                            color: theme.red
                            font.family: win.sansFont
                            font.pixelSize: 11
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 10
                        FlatInput {
                            id: extraMountPathInput
                            Layout.fillWidth: true
                            placeholder: qsTr("Host folder (e.g. ~/datasets or ~/repos/ref)")
                        }
                        ActionButton {
                            label: qsTr("Browse...")
                            minWidth: 94
                            onClicked: extraMountFolderDialog.open()
                        }
                        FlatInput {
                            id: extraMountNameInput
                            Layout.preferredWidth: 145
                            placeholder: qsTr("Mount name (e.g. data)")
                        }
                        ActionButton {
                            label: qsTr("+ Attach")
                            variant: "primary"
                            minWidth: 102
                            disabled: backend.busy
                            onClicked: {
                                const err = backend.addExtraMount(
                                    backend.selectedEnvironment.name || "",
                                    extraMountPathInput.text,
                                    extraMountNameInput.text,
                                    win.mountReadOnlyDraft,
                                    win.mountConfirmRwDraft)
                                if (err.length > 0) {
                                    win.extraMountError = err
                                } else {
                                    win.extraMountError = ""
                                    extraMountPathInput.text = ""
                                    extraMountNameInput.text = ""
                                    win.mountReadOnlyDraft = true
                                    win.mountConfirmRwDraft = false
                                }
                            }
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12

                        Rectangle {
                            height: 28
                            width: roModeTxt.implicitWidth + 24
                            radius: Math.min(4, theme.radius)
                            color: win.mountReadOnlyDraft ? win.surfaceSelected : win.surfaceInput
                            border.width: 1
                            border.color: win.mountReadOnlyDraft ? theme.accent : win.borderSubtle
                            Text {
                                id: roModeTxt
                                anchors.centerIn: parent
                                text: qsTr("Read-Only (Safe)")
                                color: win.mountReadOnlyDraft ? theme.foreground : theme.muted
                                font.family: win.sansFont
                                font.pixelSize: 11
                            }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    win.mountReadOnlyDraft = true
                                    win.mountConfirmRwDraft = false
                                }
                            }
                        }

                        Rectangle {
                            height: 28
                            width: rwModeTxt.implicitWidth + 24
                            radius: Math.min(4, theme.radius)
                            color: !win.mountReadOnlyDraft ? "#26181c" : win.surfaceInput
                            border.width: 1
                            border.color: !win.mountReadOnlyDraft ? theme.red : win.borderSubtle
                            Text {
                                id: rwModeTxt
                                anchors.centerIn: parent
                                text: qsTr("Read-Write")
                                color: !win.mountReadOnlyDraft ? theme.red : theme.muted
                                font.family: win.sansFont
                                font.pixelSize: 11
                            }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: win.mountReadOnlyDraft = false
                            }
                        }

                        Rectangle {
                            visible: !win.mountReadOnlyDraft
                            Layout.fillWidth: true
                            height: 28
                            radius: Math.min(4, theme.radius)
                            color: win.surfaceInput
                            border.width: 1
                            border.color: win.mountConfirmRwDraft ? theme.red : win.borderSubtle
                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 10
                                anchors.rightMargin: 10
                                spacing: 8
                                Rectangle {
                                    width: 14
                                    height: 14
                                    radius: 3
                                    color: win.mountConfirmRwDraft ? theme.red : "transparent"
                                    border.width: 1
                                    border.color: win.mountConfirmRwDraft ? theme.red : theme.muted
                                    Text {
                                        anchors.centerIn: parent
                                        visible: win.mountConfirmRwDraft
                                        text: "✓"
                                        color: theme.accentForeground
                                        font.family: win.monoFont
                                        font.pixelSize: 10
                                    }
                                }
                                Text {
                                    Layout.fillWidth: true
                                    elide: Text.ElideRight
                                    text: qsTr("Confirm: allow the container to modify files inside this host folder")
                                    color: win.mountConfirmRwDraft ? theme.red : theme.muted
                                    font.family: win.sansFont
                                    font.pixelSize: 11
                                }
                            }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: win.mountConfirmRwDraft = !win.mountConfirmRwDraft
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        radius: Math.min(4, theme.radius)
                        color: win.surfaceInput
                        border.width: 1
                        border.color: win.borderSubtle

                        Text {
                            anchors.centerIn: parent
                            visible: !backend.selectedEnvironment.extraMounts || backend.selectedEnvironment.extraMounts.length === 0
                            text: qsTr("No extra host mounts attached.")
                            color: theme.muted
                            font.family: win.sansFont
                            font.pixelSize: 12
                        }

                        ListView {
                            id: mountsListView
                            anchors.fill: parent
                            anchors.margins: 8
                            clip: true
                            spacing: 6
                            model: backend.selectedEnvironment.extraMounts || []
                            delegate: Rectangle {
                                required property var modelData
                                width: mountsListView.width
                                height: 48
                                radius: Math.min(3, theme.radius)
                                color: win.surfaceCard
                                border.width: 1
                                border.color: win.borderSubtle

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 12
                                    anchors.rightMargin: 12
                                    spacing: 10

                                    Rectangle {
                                        height: 18
                                        width: roBadgeTxt.implicitWidth + 10
                                        radius: 2
                                        color: modelData.readonly ? "#162920" : "#2c1419"
                                        border.width: 1
                                        border.color: modelData.readonly ? theme.green : theme.red
                                        Text {
                                            id: roBadgeTxt
                                            anchors.centerIn: parent
                                            text: modelData.readonly ? "RO" : "RW"
                                            color: modelData.readonly ? theme.green : theme.red
                                            font.family: win.monoFont
                                            font.pixelSize: 9
                                            font.weight: Font.Medium
                                        }
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        Layout.minimumWidth: 0
                                        elide: Text.ElideMiddle
                                        text: (modelData.source || "") + "  →  " + (modelData.path || "")
                                        color: theme.foreground
                                        font.family: win.monoFont
                                        font.pixelSize: 12
                                    }

                                    ActionButton {
                                        minWidth: 84
                                        label: qsTr("Remove")
                                        danger: true
                                        disabled: backend.busy
                                        onClicked: backend.removeExtraMount(
                                            backend.selectedEnvironment.name || "", modelData.deviceName || "")
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ======================================================================
        // MODAL 10: EDIT QUOTAS & LIMITS (W3)
        // ======================================================================
        Rectangle {
            id: editQuotasOverlay
            anchors.fill: parent
            visible: win.editQuotasVisible
            color: "#c406070a"
            z: 89

            onVisibleChanged: {
                if (!visible)
                    return
                quotaCpuField.text = String(win.editQuotaCpu)
                quotaRamField.text = win.editQuotaRam
                quotaDiskField.text = win.editQuotaDisk
                quotaProcField.text = win.editQuotaProc
                quotaIoField.text = win.editQuotaIo
            }

            MouseArea {
                anchors.fill: parent
                onClicked: win.editQuotasVisible = false
            }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(parent.width - 56, 540)
                height: editQuotaCol.implicitHeight + 48
                radius: Math.min(8, theme.radius)
                color: win.surfaceCard
                border.width: 1
                border.color: theme.accent

                MouseArea { anchors.fill: parent }

                ColumnLayout {
                    id: editQuotaCol
                    anchors.fill: parent
                    anchors.margins: 24
                    spacing: 14

                    ModalHeader {
                        title: qsTr("Edit Resource Quotas · %1").arg(win.editQuotaEnvName)
                        subtitle: win.editQuotaIsVm
                                  ? qsTr("VMs require a restart for CPU or RAM changes; process limits apply to containers only.")
                                  : qsTr("Applies CPU, memory, disk quota, process cap, and I/O limits live.")
                        onClosed: win.editQuotasVisible = false
                    }

                    Rectangle {
                        visible: win.editQuotaError.length > 0
                        Layout.fillWidth: true
                        height: 36
                        radius: Math.min(4, theme.radius)
                        color: "#261319"
                        border.width: 1
                        border.color: theme.red
                        Text {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 12
                            verticalAlignment: Text.AlignVCenter
                            elide: Text.ElideRight
                            text: win.editQuotaError
                            color: theme.red
                            font.family: win.sansFont
                            font.pixelSize: 11
                        }
                    }

                    GridLayout {
                        Layout.fillWidth: true
                        columns: 2
                        columnSpacing: 12
                        rowSpacing: 10

                        Text { text: qsTr("CPU Cores"); color: theme.foreground; font.family: win.sansFont; font.pixelSize: 12 }
                        FlatInput { id: quotaCpuField; Layout.fillWidth: true; implicitHeight: 34; placeholder: "4" }

                        Text { text: qsTr("RAM Limit"); color: theme.foreground; font.family: win.sansFont; font.pixelSize: 12 }
                        FlatInput { id: quotaRamField; Layout.fillWidth: true; implicitHeight: 34; placeholder: "8GiB" }

                        Text {
                            text: qsTr("Disk Quota (Used: %1)").arg(backend.selectedEnvironment.diskUsage || "0 MB")
                            color: theme.foreground
                            font.family: win.sansFont
                            font.pixelSize: 12
                        }
                        FlatInput { id: quotaDiskField; Layout.fillWidth: true; implicitHeight: 34; placeholder: "40GiB" }

                        Text {
                            visible: !win.editQuotaIsVm
                            text: qsTr("Process Limit (pids)")
                            color: theme.foreground
                            font.family: win.sansFont
                            font.pixelSize: 12
                        }
                        FlatInput {
                            id: quotaProcField
                            visible: !win.editQuotaIsVm
                            Layout.fillWidth: true
                            implicitHeight: 34
                            placeholder: "4096 or Unlimited"
                        }

                        Text { text: qsTr("Disk I/O Limit"); color: theme.foreground; font.family: win.sansFont; font.pixelSize: 12 }
                        FlatInput { id: quotaIoField; Layout.fillWidth: true; implicitHeight: 34; placeholder: "200MB or Unlimited" }
                    }

                    Rectangle {
                        readonly property string poolWarn: win.editQuotasVisible
                                                           ? backend.poolWarningForQuotaEdit(win.editQuotaEnvName, quotaDiskField.text)
                                                           : ""
                        visible: poolWarn.length > 0
                        Layout.fillWidth: true
                        implicitHeight: editPoolWarnTxt.implicitHeight + 16
                        radius: Math.min(4, theme.radius)
                        color: "#262013"
                        border.width: 1
                        border.color: "#e5a93c"
                        Text {
                            id: editPoolWarnTxt
                            anchors.fill: parent
                            anchors.margins: 8
                            anchors.leftMargin: 12
                            anchors.rightMargin: 12
                            wrapMode: Text.WordWrap
                            text: parent.poolWarn
                            color: "#e5a93c"
                            font.family: win.sansFont
                            font.pixelSize: 11
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.topMargin: 4
                        spacing: 10
                        Item { Layout.fillWidth: true }
                        ActionButton {
                            label: qsTr("Cancel")
                            minWidth: 96
                            onClicked: win.editQuotasVisible = false
                        }
                        ActionButton {
                            label: qsTr("Apply Quotas")
                            variant: "primary"
                            minWidth: 140
                            disabled: backend.busy
                            onClicked: {
                                const cpu = parseInt(quotaCpuField.text.trim(), 10)
                                const err = backend.updateEnvironmentResources(
                                    win.editQuotaEnvName,
                                    isNaN(cpu) ? 0 : cpu,
                                    quotaRamField.text,
                                    quotaDiskField.text,
                                    quotaProcField.text,
                                    quotaIoField.text)
                                if (err.length > 0) {
                                    win.editQuotaError = err
                                } else {
                                    win.editQuotasVisible = false
                                }
                            }
                        }
                    }
                }
            }
        }

        // ─── Modal 11: Backups (Export / Import) ────────────────────────────────
        Rectangle {
            id: backupsOverlay
            visible: win.backupsVisible
            anchors.fill: parent
            color: "#c406070a"
            z: 62

            MouseArea {
                anchors.fill: parent
                onClicked: win.backupsVisible = false
            }

            Rectangle {
                width: Math.min(780, parent.width - 56)
                height: Math.min(600, parent.height - 56)
                anchors.centerIn: parent
                radius: Math.min(8, theme.radius)
                color: win.surfaceCard
                border.width: 1
                border.color: theme.accent

                MouseArea { anchors.fill: parent }

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 24
                    spacing: 12

                    ModalHeader {
                        title: qsTr("Environment Backups")
                        subtitle: backend.backupsDirPath
                        subtitleMono: true
                        onClosed: win.backupsVisible = false
                    }

                    // Inline validation error
                    Rectangle {
                        visible: win.backupError.length > 0
                        Layout.fillWidth: true
                        implicitHeight: bErrTxt.implicitHeight + 14
                        radius: Math.min(4, theme.radius)
                        color: "#291418"
                        border.width: 1
                        border.color: win.redColor
                        Text {
                            id: bErrTxt
                            anchors.fill: parent
                            anchors.margins: 7
                            text: win.backupError
                            color: win.redColor
                            font.family: win.sansFont
                            font.pixelSize: 11
                            wrapMode: Text.WordWrap
                        }
                    }

                    // Export selected environment
                    Rectangle {
                        id: exportCard
                        Layout.fillWidth: true
                        implicitHeight: exportCol.implicitHeight + 20
                        radius: Math.min(4, theme.radius)
                        color: win.surfaceInput
                        border.width: 1
                        border.color: win.borderSubtle

                        readonly property var spaceInfo: (win.backupsVisible && backend.selectedEnvironment && backend.selectedEnvironment.name)
                            ? backend.checkExportSpace(backend.selectedEnvironment.name, win.backupIncludeWorkspace)
                            : ({ ok: true, estimatedDisplay: "—", availableDisplay: "—", message: "" })

                        ColumnLayout {
                            id: exportCol
                            anchors.fill: parent
                            anchors.margins: 10
                            spacing: 8

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 2
                                    Text {
                                        text: (backend.selectedEnvironment && backend.selectedEnvironment.name)
                                              ? qsTr("Export '%1'").arg(backend.selectedEnvironment.name)
                                              : qsTr("No environment selected to export")
                                        color: theme.foreground
                                        font.family: win.sansFont
                                        font.pixelSize: 13
                                        font.weight: Font.Medium
                                    }
                                    Text {
                                        text: qsTr("Est. size: %1  •  Free: %2")
                                              .arg(exportCard.spaceInfo.estimatedDisplay || "—")
                                              .arg(exportCard.spaceInfo.availableDisplay || "—")
                                        color: exportCard.spaceInfo.ok ? theme.muted : win.redColor
                                        font.family: win.monoFont
                                        font.pixelSize: 11
                                    }
                                }

                                ActionButton {
                                    label: win.backupIncludeWorkspace ? qsTr("Include Workspace: ON") : qsTr("Include Workspace: OFF")
                                    minWidth: 168
                                    onClicked: win.backupIncludeWorkspace = !win.backupIncludeWorkspace
                                }

                                ActionButton {
                                    label: qsTr("Export Backup")
                                    variant: "primary"
                                    minWidth: 130
                                    disabled: backend.busy || !(backend.selectedEnvironment && backend.selectedEnvironment.name) || !exportCard.spaceInfo.ok
                                    onClicked: {
                                        const err = backend.exportEnvironment(backend.selectedEnvironment.name, win.backupIncludeWorkspace)
                                        if (err.length > 0) {
                                            win.backupError = err
                                        } else {
                                            win.backupsVisible = false
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Saved backups list
                    Text {
                        text: qsTr("Saved Backup Archives (%1)").arg(backend.backupArchives.length)
                        color: theme.muted
                        font.family: win.sansFont
                        font.pixelSize: 11
                        font.weight: Font.Medium
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        radius: Math.min(4, theme.radius)
                        color: win.surfaceInput
                        border.width: 1
                        border.color: win.borderSubtle

                        Text {
                            anchors.centerIn: parent
                            visible: backend.backupArchives.length === 0
                            text: qsTr("No backup archives found in %1").arg(backend.backupsDirPath)
                            color: theme.muted
                            font.family: win.sansFont
                            font.pixelSize: 12
                        }

                        ListView {
                            anchors.fill: parent
                            anchors.margins: 6
                            clip: true
                            spacing: 6
                            model: backend.backupArchives
                            delegate: Rectangle {
                                required property var modelData
                                width: ListView.view.width
                                height: 48
                                radius: Math.min(3, theme.radius)
                                color: win.surfaceCard
                                border.width: 1
                                border.color: win.importBackupFile === (modelData.filePath || "") ? theme.accent : win.borderSubtle

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 12
                                    anchors.rightMargin: 12
                                    spacing: 10

                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        Layout.minimumWidth: 0
                                        spacing: 2
                                        RowLayout {
                                            Layout.fillWidth: true
                                            spacing: 8
                                            Text {
                                                Layout.fillWidth: true
                                                Layout.minimumWidth: 0
                                                text: modelData.fileName || ""
                                                color: theme.foreground
                                                font.family: win.monoFont
                                                font.pixelSize: 11
                                                elide: Text.ElideMiddle
                                            }
                                            Rectangle {
                                                visible: modelData.hasWorkspaceArchive === true
                                                width: wsTag.implicitWidth + 10
                                                height: 18
                                                radius: 2
                                                color: win.surfaceSelected
                                                border.width: 1
                                                border.color: theme.accent
                                                Text {
                                                    id: wsTag
                                                    anchors.centerIn: parent
                                                    text: qsTr("+ Workspace (%1)").arg(modelData.workspaceArchiveSizeDisplay || "")
                                                    color: theme.accent
                                                    font.family: win.monoFont
                                                    font.pixelSize: 9
                                                }
                                            }
                                        }
                                        Text {
                                            text: "%1  •  %2"
                                                  .arg(modelData.sizeDisplay || "")
                                                  .arg(modelData.createdAt || "")
                                            color: theme.muted
                                            font.family: win.monoFont
                                            font.pixelSize: 10
                                        }
                                    }

                                    ActionButton {
                                        label: qsTr("Select")
                                        minWidth: 74
                                        onClicked: {
                                            win.importBackupFile = modelData.filePath || ""
                                            win.importWsFile = modelData.workspaceArchivePath || ""
                                            win.importNewName = (modelData.envName || "restored") + "-restored"
                                            win.backupError = ""
                                        }
                                    }

                                    ActionButton {
                                        label: qsTr("Delete")
                                        variant: "danger"
                                        minWidth: 74
                                        disabled: backend.busy
                                        onClicked: backend.deleteBackup(modelData.filePath || "", modelData.workspaceArchivePath || "")
                                    }
                                }
                            }
                        }
                    }

                    // Import backup form
                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: importCol.implicitHeight + 20
                        radius: Math.min(4, theme.radius)
                        color: win.surfaceInput
                        border.width: 1
                        border.color: win.borderSubtle

                        ColumnLayout {
                            id: importCol
                            anchors.fill: parent
                            anchors.margins: 12
                            spacing: 10

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10
                                Text {
                                    text: qsTr("Import Archive:")
                                    color: theme.foreground
                                    font.family: win.sansFont
                                    font.pixelSize: 12
                                    Layout.preferredWidth: 102
                                }
                                FlatInput {
                                    Layout.fillWidth: true
                                    text: win.importBackupFile
                                    placeholder: "/path/to/env-backup.tar.gz"
                                    onTextChanged: {
                                        if (text !== win.importBackupFile) {
                                            win.importBackupFile = text
                                            win.importWsFile = ""
                                        }
                                    }
                                }
                                ActionButton {
                                    label: qsTr("Browse...")
                                    minWidth: 132
                                    onClicked: backupBrowseDialog.open()
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10
                                Text {
                                    text: qsTr("New Env Name:")
                                    color: theme.foreground
                                    font.family: win.sansFont
                                    font.pixelSize: 12
                                    Layout.preferredWidth: 102
                                }
                                FlatInput {
                                    Layout.fillWidth: true
                                    text: win.importNewName
                                    placeholder: "restored-env"
                                    onTextChanged: {
                                        if (text !== win.importNewName)
                                            win.importNewName = text
                                    }
                                }
                                ActionButton {
                                    label: qsTr("Import Backup")
                                    variant: "primary"
                                    minWidth: 132
                                    disabled: backend.busy || win.importBackupFile.trim().length === 0 || win.importNewName.trim().length === 0
                                    onClicked: {
                                        const err = backend.importEnvironment(win.importBackupFile, win.importNewName, win.importWsFile)
                                        if (err.length > 0) {
                                            win.backupError = err
                                        } else {
                                            win.backupsVisible = false
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // MODAL 10: SECRETS MANAGER (A)
        Rectangle {
            visible: win.secretsVisible
            anchors.fill: parent
            color: "#c406070a"
            z: 27

            MouseArea {
                anchors.fill: parent
                onClicked: win.secretsVisible = false
            }

            Rectangle {
                width: Math.min(760, parent.width - 56)
                height: Math.min(600, parent.height - 56)
                anchors.centerIn: parent
                radius: Math.min(8, theme.radius)
                color: win.surfaceCard
                border.width: 1
                border.color: theme.accent

                MouseArea { anchors.fill: parent }

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 24
                    spacing: 12

                    ModalHeader {
                        title: qsTr("Secrets Manager")
                        subtitle: backend.secretsStorePath + " (0600)"
                        subtitleMono: true
                        onClosed: win.secretsVisible = false
                    }

                    // Store permission or form error banner
                    Rectangle {
                        visible: backend.secretsError.length > 0 || win.secretFormError.length > 0
                        Layout.fillWidth: true
                        implicitHeight: secErrTxt.implicitHeight + 14
                        radius: Math.min(4, theme.radius)
                        color: "#291418"
                        border.width: 1
                        border.color: win.redColor
                        Text {
                            id: secErrTxt
                            anchors.fill: parent
                            anchors.margins: 7
                            text: win.secretFormError.length > 0 ? win.secretFormError : backend.secretsError
                            color: win.redColor
                            font.family: win.sansFont
                            font.pixelSize: 11
                            wrapMode: Text.WordWrap
                        }
                    }

                    // Global secret list
                    Text {
                        text: qsTr("Global Secrets (%1)").arg(backend.secretsList.length)
                        color: theme.muted
                        font.family: win.sansFont
                        font.pixelSize: 11
                        font.weight: Font.Medium
                    }

                    Text {
                        visible: (backend.selectedEnvironment.name || "").length > 0
                        text: qsTr("Inject into '%1' at session start (names only stored in user.aibox.secrets):").arg(backend.selectedEnvironment.name || "")
                        color: theme.muted
                        font.family: win.sansFont
                        font.pixelSize: 11
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }

                    Text {
                        visible: backend.secretsList.length === 0
                        text: qsTr("No secrets configured yet. Add one below.")
                        color: theme.muted
                        font.family: win.sansFont
                        font.pixelSize: 11
                    }

                    ScrollView {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        clip: true

                        ListView {
                            width: parent.width
                            spacing: 8
                            model: backend.secretsList

                            delegate: Rectangle {
                                required property var modelData
                                readonly property bool revealed: (win.secretRevealMap || {})[modelData.name] === true
                                readonly property bool assigned: (backend.selectedEnvironment.secretNames || []).indexOf(modelData.name) >= 0
                                readonly property bool hasEnv: (backend.selectedEnvironment.name || "").length > 0
                                width: ListView.view.width
                                implicitHeight: 46
                                radius: Math.min(4, theme.radius)
                                color: win.surfaceInput
                                border.width: 1
                                border.color: win.borderSubtle

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 12
                                    anchors.rightMargin: 10
                                    spacing: 10

                                    Rectangle {
                                        visible: hasEnv
                                        Layout.preferredWidth: 16
                                        Layout.preferredHeight: 16
                                        radius: 2
                                        color: assigned ? theme.accent : "transparent"
                                        border.width: 1
                                        border.color: assigned ? theme.accent : win.borderSubtle

                                        Text {
                                            anchors.centerIn: parent
                                            visible: assigned
                                            text: "x"
                                            color: theme.accentForeground
                                            font.family: win.monoFont
                                            font.pixelSize: 10
                                            font.weight: Font.Bold
                                        }

                                        MouseArea {
                                            anchors.fill: parent
                                            anchors.margins: -8
                                            enabled: !backend.busy
                                            cursorShape: backend.busy ? Qt.ForbiddenCursor : Qt.PointingHandCursor
                                            onClicked: win.toggleEnvSecret(modelData.name)
                                        }
                                    }

                                    Text {
                                        text: modelData.name
                                        color: theme.accent
                                        font.family: win.monoFont
                                        font.pixelSize: 12
                                        font.weight: Font.Medium
                                        Layout.preferredWidth: 195
                                        elide: Text.ElideRight
                                    }

                                    Text {
                                        text: revealed ? modelData.value : modelData.maskedValue
                                        color: theme.foreground
                                        font.family: win.monoFont
                                        font.pixelSize: 11
                                        Layout.fillWidth: true
                                        elide: Text.ElideRight
                                    }

                                    ActionButton {
                                        label: revealed ? qsTr("Hide") : qsTr("Show")
                                        minWidth: 64
                                        onClicked: win.toggleSecretReveal(modelData.name)
                                    }

                                    ActionButton {
                                        label: qsTr("Copy")
                                        minWidth: 62
                                        onClicked: {
                                            if (!backend.copySecret(modelData.name))
                                                win.secretFormError = qsTr("Could not copy secret '%1' to the clipboard.").arg(modelData.name)
                                        }
                                    }

                                    ActionButton {
                                        label: qsTr("Delete")
                                        variant: "danger"
                                        minWidth: 68
                                        onClicked: {
                                            const err = backend.deleteSecret(modelData.name)
                                            if (err.length > 0)
                                                win.secretFormError = err
                                            else
                                                win.secretFormError = ""
                                        }
                                    }
                                }
                            }
                        }
                    }

                    Rectangle { Layout.fillWidth: true; height: 1; color: win.borderSubtle }

                    // Add / update secret form
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        Text {
                            text: qsTr("Add or Update Secret (values are injected only at session start via tmpfs, never written to containers or backups):")
                            color: theme.muted
                            font.family: win.sansFont
                            font.pixelSize: 11
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 10

                            FlatInput {
                                Layout.preferredWidth: 210
                                implicitHeight: 34
                                text: win.secretNameDraft
                                placeholder: "ANTHROPIC_API_KEY"
                                onTextChanged: {
                                    if (text !== win.secretNameDraft)
                                        win.secretNameDraft = text
                                }
                            }

                            FlatInput {
                                Layout.fillWidth: true
                                implicitHeight: 34
                                text: win.secretValueDraft
                                echoMode: win.secretDraftShow ? TextInput.Normal : TextInput.Password
                                placeholder: qsTr("Secret value")
                                onTextChanged: {
                                    if (text !== win.secretValueDraft)
                                        win.secretValueDraft = text
                                }
                            }

                            ActionButton {
                                label: win.secretDraftShow ? qsTr("Hide") : qsTr("Show")
                                minWidth: 64
                                onClicked: win.secretDraftShow = !win.secretDraftShow
                            }

                            ActionButton {
                                label: qsTr("Save Secret")
                                variant: "primary"
                                minWidth: 116
                                disabled: win.secretNameDraft.trim().length === 0
                                onClicked: {
                                    const err = backend.saveSecret(win.secretNameDraft.trim(), win.secretValueDraft)
                                    if (err.length > 0) {
                                        win.secretFormError = err
                                    } else {
                                        win.secretFormError = ""
                                        win.secretNameDraft = ""
                                        win.secretValueDraft = ""
                                        win.secretDraftShow = false
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // MODAL 11: STALE ENVIRONMENT & ORPHAN REPORT (U)
        Rectangle {
            visible: win.staleReportVisible
            anchors.fill: parent
            color: "#c406070a"
            z: 28

            MouseArea {
                anchors.fill: parent
                onClicked: win.staleReportVisible = false
            }

            Rectangle {
                width: Math.min(860, parent.width - 56)
                height: Math.min(640, parent.height - 56)
                anchors.centerIn: parent
                radius: Math.min(8, theme.radius)
                color: win.surfaceCard
                border.width: 1
                border.color: theme.accent

                MouseArea { anchors.fill: parent }

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 24
                    spacing: 12

                    ModalHeader {
                        title: qsTr("Stale Environments & Disk Orphans")
                        subtitle: qsTr("Unused >= %1d · Reclaimable: %2 · Stale: %3 · Unknown: %4 · Orphans: %5")
                                  .arg(backend.staleReport.staleDays !== undefined ? backend.staleReport.staleDays : backend.staleDays)
                                  .arg(backend.staleReport.totalReclaimableDisplay || "0 KB")
                                  .arg(backend.staleReport.staleCount !== undefined ? backend.staleReport.staleCount : 0)
                                  .arg(backend.staleReport.unknownCount !== undefined ? backend.staleReport.unknownCount : 0)
                                  .arg(backend.staleReport.orphanCount !== undefined ? backend.staleReport.orphanCount : 0)
                        subtitleMono: true
                        onClosed: win.staleReportVisible = false

                        ActionButton {
                            label: qsTr("Rescan")
                            minWidth: 88
                            disabled: backend.busy
                            onClicked: {
                                win.staleActionError = ""
                                backend.refreshStaleReport()
                            }
                        }
                    }

                    // Inline action error banner
                    Rectangle {
                        visible: win.staleActionError.length > 0
                        Layout.fillWidth: true
                        implicitHeight: staleErrTxt.implicitHeight + 14
                        radius: Math.min(4, theme.radius)
                        color: "#291418"
                        border.width: 1
                        border.color: theme.red
                        Text {
                            id: staleErrTxt
                            anchors.fill: parent
                            anchors.margins: 7
                            text: win.staleActionError
                            color: theme.red
                            font.family: win.sansFont
                            font.pixelSize: 11
                            wrapMode: Text.WordWrap
                        }
                    }

                    // Inline confirmation bar for Wipe / Export & Wipe / Delete orphan
                    Rectangle {
                        visible: win.staleConfirmAction.length > 0
                        Layout.fillWidth: true
                        implicitHeight: staleConfirmCol.implicitHeight + 20
                        radius: Math.min(4, theme.radius)
                        color: win.surfaceInput
                        border.width: 1
                        border.color: win.staleConfirmAction === "export-wipe" ? theme.accent : theme.red

                        ColumnLayout {
                            id: staleConfirmCol
                            anchors.fill: parent
                            anchors.margins: 10
                            spacing: 8

                            Text {
                                Layout.fillWidth: true
                                text: win.staleConfirmTitle
                                color: theme.foreground
                                font.family: win.sansFont
                                font.pixelSize: 12
                                font.weight: Font.Medium
                                wrapMode: Text.WordWrap
                            }
                            Text {
                                Layout.fillWidth: true
                                text: win.staleConfirmDetail
                                color: theme.muted
                                font.family: win.sansFont
                                font.pixelSize: 11
                                wrapMode: Text.WordWrap
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10

                                Rectangle {
                                    visible: win.staleConfirmAction === "wipe" || win.staleConfirmAction === "export-wipe"
                                    height: 28
                                    width: staleWsDelRow.implicitWidth + 16
                                    radius: Math.min(4, theme.radius)
                                    color: win.staleConfirmDeleteWs ? win.surfaceSelected : win.surfaceCard
                                    border.width: 1
                                    border.color: win.staleConfirmDeleteWs ? theme.accent : win.borderSubtle

                                    RowLayout {
                                        id: staleWsDelRow
                                        anchors.centerIn: parent
                                        spacing: 6
                                        Text {
                                            text: win.staleConfirmDeleteWs ? "[x]" : "[ ]"
                                            color: win.staleConfirmDeleteWs ? theme.accent : theme.muted
                                            font.family: win.monoFont
                                            font.pixelSize: 11
                                        }
                                        Text {
                                            text: qsTr("Also delete host %1 files & snapshots").arg(backend.containerWorkspacePath)
                                            color: theme.foreground
                                            font.family: win.sansFont
                                            font.pixelSize: 11
                                        }
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: win.staleConfirmDeleteWs = !win.staleConfirmDeleteWs
                                    }
                                }

                                Item { Layout.fillWidth: true }

                                ActionButton {
                                    visible: win.staleConfirmAction === "delete-orphan"
                                    label: qsTr("Delete as admin")
                                    minWidth: 132
                                    disabled: backend.busy
                                    onClicked: {
                                        win.staleActionError = ""
                                        const err = backend.deleteOrphanAsAdmin(
                                            win.staleConfirmKind, win.staleConfirmTarget)
                                        if (err.length > 0)
                                            win.staleActionError = err
                                        else
                                            win.staleConfirmAction = ""
                                    }
                                }

                                ActionButton {
                                    label: qsTr("Cancel")
                                    minWidth: 84
                                    onClicked: win.staleConfirmAction = ""
                                }

                                ActionButton {
                                    label: win.staleConfirmAction === "export-wipe"
                                           ? qsTr("Export & Wipe")
                                           : (win.staleConfirmAction === "delete-orphan" ? qsTr("Delete Orphan") : qsTr("Confirm Wipe"))
                                    variant: win.staleConfirmAction === "export-wipe" ? "primary" : ""
                                    danger: win.staleConfirmAction !== "export-wipe"
                                    minWidth: 126
                                    disabled: backend.busy
                                    onClicked: {
                                        win.staleActionError = ""
                                        if (win.staleConfirmAction === "export-wipe") {
                                            const err = backend.exportAndWipeEnvironment(
                                                win.staleConfirmTarget,
                                                true,
                                                win.staleConfirmDeleteWs)
                                            if (err.length > 0) {
                                                win.staleActionError = err
                                            } else {
                                                win.staleConfirmAction = ""
                                                win.staleReportVisible = false
                                            }
                                        } else if (win.staleConfirmAction === "wipe") {
                                            backend.wipeEnvironment(win.staleConfirmTarget, win.staleConfirmDeleteWs)
                                            win.staleConfirmAction = ""
                                            win.staleReportVisible = false
                                        } else if (win.staleConfirmAction === "delete-orphan") {
                                            const err = backend.deleteOrphan(win.staleConfirmKind, win.staleConfirmTarget)
                                            if (err.length > 0) {
                                                win.staleActionError = err
                                            } else {
                                                win.staleConfirmAction = ""
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    ScrollView {
                        id: staleReportScroll
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        clip: true
                        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

                        ColumnLayout {
                            width: staleReportScroll.availableWidth
                            spacing: 14

                            Text {
                                text: qsTr("Stale & Unknown Environments (%1)").arg((backend.staleReport.entries || []).length)
                                color: theme.foreground
                                font.family: win.sansFont
                                font.pixelSize: 13
                                font.weight: Font.Medium
                            }

                            Text {
                                visible: (backend.staleReport.entries || []).length === 0
                                text: qsTr("No environments have been unused for %1+ days (templates excluded).").arg(backend.staleReport.staleDays || backend.staleDays)
                                color: theme.muted
                                font.family: win.sansFont
                                font.pixelSize: 12
                            }

                            Repeater {
                                model: backend.staleReport.entries || []
                                delegate: Rectangle {
                                    required property var modelData
                                    Layout.fillWidth: true
                                    implicitHeight: staleEnvRow.implicitHeight + 20
                                    radius: Math.min(4, theme.radius)
                                    color: win.surfaceInput
                                    border.width: 1
                                    border.color: modelData.status === "stale" ? "#e5a93c" : win.borderSubtle

                                    RowLayout {
                                        id: staleEnvRow
                                        anchors.fill: parent
                                        anchors.margins: 10
                                        spacing: 12

                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            spacing: 4

                                            RowLayout {
                                                spacing: 8
                                                Rectangle {
                                                    width: 8
                                                    height: 8
                                                    radius: 4
                                                    color: modelData.running ? theme.green : theme.muted
                                                }
                                                Text {
                                                    text: modelData.name || ""
                                                    color: theme.foreground
                                                    font.family: win.monoFont
                                                    font.pixelSize: 13
                                                    font.weight: Font.Medium
                                                }
                                                Rectangle {
                                                    height: 20
                                                    width: staleBadgeTxt.implicitWidth + 12
                                                    radius: 3
                                                    color: win.surfaceCard
                                                    border.width: 1
                                                    border.color: modelData.status === "stale" ? "#e5a93c" : win.borderSubtle
                                                    Text {
                                                        id: staleBadgeTxt
                                                        anchors.centerIn: parent
                                                        text: modelData.status === "stale" ? qsTr("STALE") : qsTr("UNKNOWN")
                                                        color: modelData.status === "stale" ? "#e5a93c" : theme.muted
                                                        font.family: win.monoFont
                                                        font.pixelSize: 10
                                                    }
                                                }
                                                Text {
                                                    text: (modelData.osName || "") + " · " + (modelData.lastUsedDisplay || "")
                                                    color: theme.muted
                                                    font.family: win.sansFont
                                                    font.pixelSize: 11
                                                    elide: Text.ElideRight
                                                    Layout.fillWidth: true
                                                }
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                text: qsTr("Disk: %1 · Snapshots: %2 (%3) · Workspace: %4 · Total: %5")
                                                          .arg(modelData.diskUsageDisplay || "0 KB")
                                                          .arg(modelData.snapshotCount !== undefined ? modelData.snapshotCount : 0)
                                                          .arg(modelData.snapshotTotalDisplay || "—")
                                                          .arg(modelData.workspaceSizeDisplay || "0 KB")
                                                          .arg(modelData.totalFootprintDisplay || "0 KB")
                                                color: theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 10
                                                elide: Text.ElideRight
                                            }
                                        }

                                        ActionButton {
                                            label: qsTr("Export & Wipe")
                                            variant: "primary"
                                            minWidth: 118
                                            disabled: backend.busy
                                            onClicked: {
                                                win.staleActionError = ""
                                                win.staleConfirmAction = "export-wipe"
                                                win.staleConfirmTarget = modelData.name || ""
                                                win.staleConfirmKind = ""
                                                win.staleConfirmDeleteWs = true
                                                win.staleConfirmTitle = qsTr("Export & Wipe '%1'?").arg(modelData.name || "")
                                                win.staleConfirmDetail = qsTr("Exports an instance & workspace backup archive to %1 first. Wipe proceeds ONLY if export succeeds and the archive size is > 0 bytes. Footprint: %2.")
                                                                             .arg(backend.backupsDirPath)
                                                                             .arg(modelData.totalFootprintDisplay || "0 KB")
                                            }
                                        }

                                        ActionButton {
                                            label: qsTr("Wipe")
                                            danger: true
                                            minWidth: 74
                                            disabled: backend.busy
                                            onClicked: {
                                                win.staleActionError = ""
                                                win.staleConfirmAction = "wipe"
                                                win.staleConfirmTarget = modelData.name || ""
                                                win.staleConfirmKind = ""
                                                win.staleConfirmDeleteWs = true
                                                win.staleConfirmTitle = qsTr("Wipe environment '%1' without export?").arg(modelData.name || "")
                                                win.staleConfirmDetail = qsTr("Permanently destroys the container/VM and its snapshots (%1 disk, %2 snapshots, %3 workspace).")
                                                                             .arg(modelData.diskUsageDisplay || "0 KB")
                                                                             .arg(modelData.snapshotCount || 0)
                                                                             .arg(modelData.workspaceSizeDisplay || "0 KB")
                                            }
                                        }
                                    }
                                }
                            }

                            Rectangle { Layout.fillWidth: true; height: 1; color: win.borderSubtle }

                            Text {
                                text: qsTr("Orphaned Folders, Snapshots & Backups (%1)").arg((backend.staleReport.orphans || []).length)
                                color: theme.foreground
                                font.family: win.sansFont
                                font.pixelSize: 13
                                font.weight: Font.Medium
                            }

                            Text {
                                visible: (backend.staleReport.orphans || []).length === 0
                                text: qsTr("No orphaned workspace folders, workspace snapshot directories, or unpaired backup workspace archives found.")
                                color: theme.muted
                                font.family: win.sansFont
                                font.pixelSize: 12
                            }

                            Repeater {
                                model: backend.staleReport.orphans || []
                                delegate: Rectangle {
                                    required property var modelData
                                    Layout.fillWidth: true
                                    implicitHeight: orphanRow.implicitHeight + 20
                                    radius: Math.min(4, theme.radius)
                                    color: win.surfaceInput
                                    border.width: 1
                                    border.color: win.borderSubtle

                                    RowLayout {
                                        id: orphanRow
                                        anchors.fill: parent
                                        anchors.margins: 10
                                        spacing: 10

                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            spacing: 3

                                            RowLayout {
                                                spacing: 8
                                                Text {
                                                    text: modelData.name || ""
                                                    color: theme.foreground
                                                    font.family: win.monoFont
                                                    font.pixelSize: 12
                                                    font.weight: Font.Medium
                                                }
                                                Rectangle {
                                                    height: 20
                                                    width: orphanKindTxt.implicitWidth + 10
                                                    radius: 3
                                                    color: win.surfaceCard
                                                    border.width: 1
                                                    border.color: win.borderSubtle
                                                    Text {
                                                        id: orphanKindTxt
                                                        anchors.centerIn: parent
                                                        text: modelData.kindLabel || modelData.kind || ""
                                                        color: theme.muted
                                                        font.family: win.monoFont
                                                        font.pixelSize: 10
                                                    }
                                                }
                                                Rectangle {
                                                    height: 20
                                                    width: orphanSizeTxt.implicitWidth + 10
                                                    radius: 3
                                                    color: win.surfaceCard
                                                    border.width: 1
                                                    border.color: theme.accent
                                                    Text {
                                                        id: orphanSizeTxt
                                                        anchors.centerIn: parent
                                                        text: modelData.sizeDisplay || "0 KB"
                                                        color: theme.accent
                                                        font.family: win.monoFont
                                                        font.pixelSize: 10
                                                    }
                                                }
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                text: modelData.path || ""
                                                color: theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 10
                                                elide: Text.ElideMiddle
                                            }
                                        }

                                        ActionButton {
                                            label: qsTr("Open folder")
                                            minWidth: 104
                                            onClicked: backend.openOrphanFolder(modelData.openPath || modelData.path || "")
                                        }

                                        ActionButton {
                                            label: qsTr("Delete")
                                            danger: true
                                            minWidth: 80
                                            disabled: backend.busy
                                            onClicked: {
                                                win.staleActionError = ""
                                                win.staleConfirmAction = "delete-orphan"
                                                win.staleConfirmKind = modelData.kind || ""
                                                win.staleConfirmTarget = modelData.path || ""
                                                win.staleConfirmTitle = qsTr("Delete orphan '%1' (%2)?")
                                                                            .arg(modelData.name || "")
                                                                            .arg(modelData.sizeDisplay || "0 KB")
                                                win.staleConfirmDetail = qsTr("Permanently deletes %1 (%2 · %3). This cannot be undone. Root-owned files need 'Delete as admin'.")
                                                                             .arg(modelData.path || "")
                                                                             .arg(modelData.kindLabel || "")
                                                                             .arg(modelData.sizeDisplay || "0 KB")
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        FileDialog {
            id: backupBrowseDialog
            title: qsTr("Choose Environment Backup Archive")
            currentFolder: backend.pathToUrl(backend.backupsDirPath)
            nameFilters: [qsTr("Backup archives (*.tar.gz)"), qsTr("All files (*)")]
            onAccepted: {
                win.importBackupFile = backend.localPath(selectedFile)
                win.importWsFile = ""
                if (win.importNewName.trim().length === 0)
                    win.importNewName = "restored-env"
            }
        }

        FolderDialog {
            id: extraMountFolderDialog
            title: qsTr("Choose Extra Host Folder to Mount")
            currentFolder: backend.pathToUrl(backend.workspaceBase)
            onAccepted: extraMountPathInput.text = backend.localPath(selectedFolder)
        }

        FolderDialog {
            id: createSharedFolderDialog
            title: qsTr("Choose Custom Shared Folder Location")
            currentFolder: backend.pathToUrl(win.newEnvSharedPath.trim().length > 0
                                             ? win.newEnvSharedPath.trim() : backend.workspaceBase)
            onAccepted: {
                win.newEnvSharedPath = backend.localPath(selectedFolder)
                createSharedPathField.text = win.newEnvSharedPath
            }
        }

        Timer {
            id: configNoticeTimer
            interval: 5000
            onTriggered: win.configNotice = ""
        }

    }
}
