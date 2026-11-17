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
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0

                    Text {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: backend.selectedEnvironment.name || ""
                        color: theme.foreground
                        font.family: Style.fontMono
                        font.pixelSize: Style.font18
                        font.weight: Font.Medium
                        elide: Text.ElideRight
                    }

                    Text {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: (backend.selectedEnvironment.osName || "") + "  ·  " + (backend.selectedEnvironment.preset || "")
                        color: theme.muted
                        font.family: Style.fontSans
                        font.pixelSize: Style.font12
                    }
                }

                ActionButton {
                    label: (backend.selectedEnvironment.isTemplate || false)
                           ? qsTr("New from Template")
                           : qsTr("Enter OS Terminal")
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
                    variant: (backend.selectedEnvironment.running || false) ? "danger" : "accent"
                    minWidth: 98
                    disabled: backend.busy || (backend.selectedEnvironment.isTemplate || false)
                    Layout.alignment: Qt.AlignVCenter
                    onClicked: backend.toggleStartStop(backend.selectedEnvironment.name || "")
                }

                ActionButton {
                    label: qsTr("More Actions")
                    minWidth: 118
                    disabled: backend.busy
                    Layout.alignment: Qt.AlignVCenter
                    onClicked: app.openEnvironmentActions()
                }

                Item { Layout.fillWidth: true }
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
                    title: qsTr("CPU Usage")
                    value: (backend.selectedEnvironment.cpuUsage || "0%") + " / " + (backend.selectedEnvironment.cpuLimit || "—")
                    badge: backend.selectedEnvironment.cpuUsage || "0%"
                    percent: backend.selectedEnvironment.cpuPercent !== undefined ? backend.selectedEnvironment.cpuPercent : 0
                    hasBar: true
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

            SnapshotsCard {
                id: snapshotsCard
                app: root.app
            }
        }
    }
}
