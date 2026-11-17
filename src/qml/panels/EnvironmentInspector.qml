import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import "../components"

// Right pane of the Environments tab: inspector header, actions, resource
// cards, workspace access and the embedded network/snapshot sections.
Rectangle {
    id: root
    property QtObject app: null
    readonly property string snapshotNameText: snapshotsCard.snapshotNameText

    color: Style.surfaceCard
    radius: theme.radius
    border.width: Style.hairline
    border.color: Style.borderSubtle

    ScrollView {
        id: inspectorScroll
        anchors.fill: parent
        anchors.margins: Style.space20
        clip: true
        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

        ColumnLayout {
            width: inspectorScroll.availableWidth
            height: Math.max(inspectorScroll.availableHeight, implicitHeight)
            spacing: Style.space12

            // Environment header and primary action
            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space16

                ColumnLayout {
                    spacing: Style.space4
                    Layout.alignment: Qt.AlignVCenter

                    RowLayout {
                        spacing: Style.space10
                        Layout.alignment: Qt.AlignVCenter

                        Text {
                            text: backend.selectedEnvironment.name || ""
                            color: theme.foreground
                            font.family: Style.fontMono
                            font.pixelSize: Style.font18
                            font.weight: Font.Medium
                            elide: Text.ElideRight
                            Layout.maximumWidth: 340
                            Layout.alignment: Qt.AlignVCenter
                        }

                        TagBadge {
                            visible: backend.selectedEnvironment.isTemplate || false
                            text: qsTr("TEMPLATE")
                            tone: "accent"
                            badgeHeight: Style.badgeHeightLg
                            badgeRadius: Style.radius3
                            textSize: Style.font10
                            textWeight: Font.Medium
                            horizontalPadding: Style.space14
                            Layout.alignment: Qt.AlignVCenter
                        }

                        TagBadge {
                            text: backend.selectedEnvironment.status || ""
                            tone: (backend.selectedEnvironment.running || false) ? "success" : "neutral"
                            fillColor: (backend.selectedEnvironment.running || false) ? Style.successSurface : Style.surfaceInput
                            dotVisible: true
                            badgeHeight: Style.badgeHeightLg
                            badgeRadius: Style.radius3
                            textSize: Style.font10
                            textWeight: Font.Medium
                            horizontalPadding: Style.space16
                            Layout.alignment: Qt.AlignVCenter
                        }

                        TagBadge {
                            visible: (backend.selectedEnvironment.secretNames || []).length > 0
                            text: qsTr("SECRETS: %1").arg((backend.selectedEnvironment.secretNames || []).length)
                            tone: "accent"
                            fillColor: Style.surfaceInput
                            interactive: true
                            badgeHeight: Style.badgeHeightLg
                            badgeRadius: Style.radius3
                            textSize: Style.font10
                            textWeight: Font.Medium
                            horizontalPadding: Style.space14
                            Layout.alignment: Qt.AlignVCenter
                            onClicked: app.openSecretsDialog()
                        }
                    }

                    Text {
                        readonly property string pidStr: backend.selectedEnvironment.processLimit || ""
                        readonly property string ioStr: backend.selectedEnvironment.diskWriteLimit || backend.selectedEnvironment.diskReadLimit || ""
                        readonly property string limitSuffix: (pidStr.length > 0 ? "  ·  Pids " + pidStr : "")
                                                              + (ioStr.length > 0 ? "  ·  I/O " + ioStr : "")
                        text: (backend.selectedEnvironment.osName || "") + "  ·  " + (backend.selectedEnvironment.preset || "") + limitSuffix
                        color: theme.muted
                        font.family: Style.fontSans
                        font.pixelSize: Style.font12
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
                            app.openCreateDialog("", env.name || "")
                        else
                            backend.enterEnvironment(env.name || "")
                    }
                }
            }

            // Secondary action bar
            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space10

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
                    onClicked: app.openCloneDialog(backend.selectedEnvironment.name || "")
                }

                ActionButton {
                    label: qsTr("Quotas")
                    shortcutHint: "E"
                    minWidth: 92
                    disabled: backend.busy
                    Layout.alignment: Qt.AlignVCenter
                    onClicked: app.openEditQuotasDialog()
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
                        app.deleteWorkspaceOnWipe = false
                        app.wipeVisible = true
                    }
                }
            }

            // Resource monitor cards
            GridLayout {
                Layout.fillWidth: true
                columns: 3
                columnSpacing: Style.space10
                rowSpacing: Style.space10

                ResourceMeterCard {
                    Layout.fillWidth: true
                    title: qsTr("Disk Usage")
                    value: (backend.selectedEnvironment.diskUsage || "0 MB") + " / " + (backend.selectedEnvironment.diskLimit || "—")
                    badge: (backend.selectedEnvironment.diskAlertLevel === 2
                            ? qsTr("CRIT ")
                            : (backend.selectedEnvironment.diskAlertLevel === 1 ? qsTr("WARN ") : ""))
                           + (backend.selectedEnvironment.diskPercent !== undefined ? backend.selectedEnvironment.diskPercent : 0) + "%"
                    percent: backend.selectedEnvironment.diskPercent !== undefined ? backend.selectedEnvironment.diskPercent : 0
                    alertLevel: backend.selectedEnvironment.diskAlertLevel !== undefined ? backend.selectedEnvironment.diskAlertLevel : 0
                    hasBar: true
                    active: true
                }

                ResourceMeterCard {
                    Layout.fillWidth: true
                    title: qsTr("Memory Usage")
                    value: (backend.selectedEnvironment.memoryUsage || "0 MB") + " / " + (backend.selectedEnvironment.memoryLimit || "—")
                    badge: (backend.selectedEnvironment.memoryPercent !== undefined ? backend.selectedEnvironment.memoryPercent : 0) + "%"
                    percent: backend.selectedEnvironment.memoryPercent !== undefined ? backend.selectedEnvironment.memoryPercent : 0
                    hasBar: true
                    active: backend.selectedEnvironment.running || false
                }

                ResourceMeterCard {
                    Layout.fillWidth: true
                    title: qsTr("CPU Usage") + " · " + (backend.selectedEnvironment.cpuLimit || "—")
                    value: (backend.selectedEnvironment.cpuUsage || "0%") + " · " + (backend.selectedEnvironment.cpuLimit || "—")
                    badge: backend.selectedEnvironment.cpuUsage || "0%"
                    percent: backend.selectedEnvironment.cpuPercent !== undefined ? backend.selectedEnvironment.cpuPercent : 0
                    isCpu: true
                    cores: backend.selectedEnvironment.cpuCorePercents || [0, 0, 0, 0]
                    active: backend.selectedEnvironment.running || false
                }
            }

            // Shared workspace box
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 50
                radius: Math.min(Style.radius4, theme.radius)
                color: Style.surfaceInput
                border.width: Style.hairline
                border.color: Style.borderSubtle

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Style.space14
                    anchors.rightMargin: Style.space14
                    spacing: Style.space10

                    Text {
                        text: qsTr("Shared Workspace")
                        color: theme.muted
                        font.family: Style.fontSans
                        font.pixelSize: Style.font11
                    }

                    Text {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        elide: Text.ElideMiddle
                        text: (backend.selectedEnvironment.workspacePath || "") + "  ⟷  " + backend.containerWorkspacePath
                        color: theme.accent
                        font.family: Style.fontMono
                        font.pixelSize: Style.font12
                    }

                    ActionButton {
                        label: (backend.selectedEnvironment.extraMountCount || 0) > 0
                               ? qsTr("Mounts (%1)").arg(backend.selectedEnvironment.extraMountCount)
                               : qsTr("Extra Mounts")
                        minWidth: 112
                        Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                        onClicked: app.openMountsDialog()
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

            // Root file access (localhost-only SFTP) box
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 50
                radius: Math.min(Style.radius4, theme.radius)
                color: Style.surfaceInput
                border.width: Style.hairline
                border.color: (backend.selectedEnvironment.fileAccessActive || false)
                              ? theme.accent
                              : Style.borderSubtle

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Style.space14
                    anchors.rightMargin: Style.space14
                    spacing: Style.space10

                    Text {
                        text: qsTr("Root Files")
                        color: theme.muted
                        font.family: Style.fontSans
                        font.pixelSize: Style.font11
                        ToolTip.visible: rootLabelHover.hovered
                        ToolTip.delay: 500
                        ToolTip.text: qsTr("Browse this environment's root filesystem in your file manager over a localhost-only, passwordless SFTP connection. Anyone on this machine can read and write files as root while it is enabled.")
                        HoverHandler { id: rootLabelHover }
                    }

                    Text {
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
                        font.family: Style.fontMono
                        font.pixelSize: Style.font12
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

            NetworkModeCard {
                app: root.app
            }

            NetworkMonitorCard {
                app: root.app
            }

            SnapshotsCard {
                id: snapshotsCard
                app: root.app
            }

            // Headless GUI test runner
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 54
                radius: Math.min(Style.radius4, theme.radius)
                color: Style.surfaceInput
                border.width: Style.hairline
                border.color: Style.borderSubtle

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Style.space12
                    anchors.rightMargin: Style.space12
                    spacing: Style.space10

                    Text {
                        text: qsTr("Virtual Display:")
                        color: theme.muted
                        font.family: Style.fontSans
                        font.pixelSize: Style.font12
                    }

                    FlatInput {
                        id: headlessInput
                        Layout.fillWidth: true
                        placeholder: qsTr("Command for 1920x1080 Xvfb screen (e.g. flutter test, npm test, pytest)")
                        onAccepted: {
                            if (!backend.busy && headlessInput.text.length > 0)
                                backend.runHeadlessCommand(backend.selectedEnvironment.name || "", headlessInput.text)
                        }
                    }

                    ActionButton {
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
