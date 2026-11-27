import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import "../components"

// Right pane of the Environments tab: inspector header, actions, resource
// cards, workspace access and the embedded network/snapshot sections.
Item {
    id: root
    property QtObject app: null
    property string cpuExpandedEnv: ""
    readonly property bool cpuExpanded: cpuExpandedEnv.length > 0
                                         && cpuExpandedEnv === (backend.selectedEnvironment.name || "")
    readonly property string snapshotNameText: snapshotsCard.snapshotNameText

    // Per-core load shown inside the expanded CPU card, two columns of cores.
    Component {
        id: cpuCoresGrid

        GridLayout {
            columns: 2
            columnSpacing: Style.gapMd
            rowSpacing: Style.gapSm
            width: parent ? parent.width : 0

            Repeater {
                model: backend.selectedEnvironment.cpuCorePercents || []

                delegate: ColumnLayout {
                    required property int index
                    required property var modelData

                    Layout.fillWidth: true
                    spacing: Style.gapXs

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Style.gapSm

                        Text {
                            elide: Text.ElideRight
                            text: qsTr("Core %1").arg(index + 1)
                            color: theme.textMuted
                            font.family: Style.fontMono
                            font.pixelSize: Style.fontCaption
                        }

                        Text {
                            text: modelData + "%"
                            color: modelData >= 85 ? theme.dangerText : theme.textPrimary
                            font.family: Style.fontMono
                            font.pixelSize: Style.fontCaption
                            font.weight: Font.Medium
                        }

                        Item { Layout.fillWidth: true }
                    }

                    MeterBar {
                        Layout.fillWidth: true
                        implicitHeight: Style.gapSm
                        value: modelData
                        minFillWidth: Style.gapXs
                        trackColor: theme.surfaceTrack
                        fillColor: modelData >= 85 ? theme.danger : theme.accent
                    }
                }
            }
        }
    }

    ScrollView {
        id: inspectorScroll
        anchors.fill: parent
        clip: true
        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
        ScrollBar.vertical.policy: ScrollBar.AlwaysOff

        ColumnLayout {
            width: inspectorScroll.availableWidth
            height: Math.max(inspectorScroll.availableHeight, implicitHeight)
            spacing: Style.gap

            // Environment header and actions in a single row
            RowLayout {
                Layout.fillWidth: true
                spacing: Style.gap

                ColumnLayout {
                    spacing: Style.gapSm
                    Layout.alignment: Qt.AlignVCenter
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0

                    Text {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: backend.selectedEnvironment.name || ""
                        color: theme.textPrimary
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontDisplay
                        font.weight: Font.Medium
                        elide: Text.ElideRight
                    }

                    Text {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: (backend.selectedEnvironment.osName || "") + "  ·  " + (backend.selectedEnvironment.preset || "")
                        color: theme.textSecondary
                        font.family: Style.fontSans
                        font.pixelSize: Style.fontSmall
                    }
                }

                RowLayout {
                    spacing: Style.gap
                    Layout.alignment: Qt.AlignVCenter | Qt.AlignRight

                    ActionButton {
                        label: (backend.selectedEnvironment.running || false) ? qsTr("Stop OS") : qsTr("Start OS")
                        variant: (backend.selectedEnvironment.running || false) ? "danger" : "accent"
                        disabled: backend.busy || (backend.selectedEnvironment.isTemplate || false)
                        onClicked: app.requestStartStop()
                    }

                    ActionButton {
                        label: qsTr("More Actions")
                        disabled: backend.busy
                        onClicked: app.openEnvironmentActions()
                    }

                    ActionButton {
                        label: (backend.selectedEnvironment.isTemplate || false)
                               ? qsTr("New from Template")
                               : qsTr("Enter OS Terminal")
                        variant: "primary"
                        disabled: backend.busy
                        onClicked: {
                            const env = backend.selectedEnvironment
                            if (env.isTemplate)
                                app.openCreateDialog("", env.name || "")
                            else
                                backend.enterEnvironment(env.name || "")
                        }
                    }
                }
            }

            // Resource monitor cards
            GridLayout {
                Layout.fillWidth: true
                columns: 3
                columnSpacing: Style.gap
                rowSpacing: Style.gap

                ResourceMeterCard {
                    Layout.fillWidth: true
                    Layout.preferredHeight: cpuCard.Layout.preferredHeight
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
                    Layout.preferredHeight: cpuCard.Layout.preferredHeight
                    title: qsTr("Memory Usage")
                    value: (backend.selectedEnvironment.memoryUsage || "0 MB") + " / " + (backend.selectedEnvironment.memoryLimit || "—")
                    badge: (backend.selectedEnvironment.memoryPercent !== undefined ? backend.selectedEnvironment.memoryPercent : 0) + "%"
                    percent: backend.selectedEnvironment.memoryPercent !== undefined ? backend.selectedEnvironment.memoryPercent : 0
                    hasBar: true
                    active: backend.selectedEnvironment.running || false
                }

                ResourceMeterCard {
                    id: cpuCard
                    Layout.fillWidth: true
                    title: qsTr("CPU Usage")
                    value: (backend.selectedEnvironment.cpuUsage || "0%") + " / " + (backend.selectedEnvironment.cpuLimit || "—")
                    badge: backend.selectedEnvironment.cpuUsage || "0%"
                    percent: backend.selectedEnvironment.cpuPercent !== undefined ? backend.selectedEnvironment.cpuPercent : 0
                    hasBar: true
                    active: backend.selectedEnvironment.running || false
                    interactive: true
                    expanded: root.cpuExpanded
                    expandedContent: cpuCoresGrid
                    hint: root.cpuExpanded
                          ? qsTr("Click to collapse core usage")
                          : qsTr("Click to expand core usage")
                    onClicked: root.cpuExpandedEnv = root.cpuExpanded
                                                    ? ""
                                                    : (backend.selectedEnvironment.name || "")
                }
            }

            Card {
                Layout.fillWidth: true
                title: qsTr("Swap Memory")
                subtitle: (backend.selectedEnvironment.type === "VIRTUAL-MACHINE"
                           || backend.selectedEnvironment.type === "VM")
                          ? qsTr("VM swap is managed inside the guest.")
                          : qsTr("Container swap is provided by the host.")

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Style.gap * 2

                    KeyValue {
                        label: qsTr("Allowance")
                        value: {
                            const env = backend.selectedEnvironment
                            if ((env.type || "") === "VIRTUAL-MACHINE" || (env.type || "") === "VM")
                                return qsTr("Guest-managed")
                            const raw = env.swapLimit || ""
                            if (raw.length === 0)
                                return qsTr("Host")
                            if (raw === "Off")
                                return qsTr("Off")
                            return raw
                        }
                    }

                    KeyValue {
                        visible: (backend.selectedEnvironment.type || "") !== "VIRTUAL-MACHINE"
                                 && (backend.selectedEnvironment.type || "") !== "VM"
                        label: qsTr("Usage")
                        value: {
                            if (!(backend.selectedEnvironment.running || false))
                                return "—"
                            return (backend.selectedEnvironment.swapUsage || "0 B")
                                   + " / " + (backend.selectedEnvironment.swapLimit || qsTr("Host"))
                        }
                    }
                }

                MeterBar {
                    visible: (backend.selectedEnvironment.type || "") !== "VIRTUAL-MACHINE"
                             && (backend.selectedEnvironment.type || "") !== "VM"
                             && (backend.selectedEnvironment.running || false)
                    Layout.fillWidth: true
                    value: backend.selectedEnvironment.swapPercent || 0
                }
            }

            // Shared workspace & Root file access cards
            GridLayout {
                Layout.fillWidth: true
                Layout.fillHeight: false
                columns: 2
                columnSpacing: Style.gap
                rowSpacing: Style.gap

                // Shared workspace card
                Card {
                    id: sharedWorkspaceCard
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    Layout.preferredHeight: Math.max(sharedWorkspaceCard.implicitHeight, rootFilesCard.implicitHeight)
                    title: qsTr("Shared Workspace")
                    actions: [
                        ActionButton {
                            label: (backend.selectedEnvironment.extraMountCount || 0) > 0
                                   ? qsTr("Mounts (%1)").arg(backend.selectedEnvironment.extraMountCount)
                                   : qsTr("Extra Mounts")
                            onClicked: app.openMountsDialog()
                        },
                        ActionButton {
                            label: qsTr("Open Shared Folder")
                            onClicked: backend.openWorkspaceFolder(backend.selectedEnvironment.name || "")
                        }
                    ]

                    KeyValue {
                        label: qsTr("Host ↔ Guest")
                        value: (backend.selectedEnvironment.workspacePath || "") + "  ⟷  " + backend.containerWorkspacePath
                        bodyValue: true
                    }
                }

                // Root file access (localhost-only SFTP) card
                Card {
                    id: rootFilesCard
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    Layout.preferredHeight: Math.max(sharedWorkspaceCard.implicitHeight, rootFilesCard.implicitHeight)
                    title: qsTr("Root Files")
                    borderColor: (backend.selectedEnvironment.fileAccessActive || false)
                                 ? theme.accentBorder
                                 : theme.borderSubtle
                    actions: [
                        ActionButton {
                            visible: backend.selectedEnvironment.fileAccessActive || false
                            label: qsTr("Open")
                            onClicked: backend.openFileAccess(backend.selectedEnvironment.name || "")
                        },
                        ToggleChip {
                            label: qsTr("Enabled")
                            checked: backend.selectedEnvironment.fileAccessActive || false
                            disabled: backend.busy
                                      || (backend.selectedEnvironment.isTemplate || false)
                                      || (!(backend.selectedEnvironment.fileAccessActive || false)
                                          && !(backend.selectedEnvironment.running || false))
                            onToggled: {
                                const name = backend.selectedEnvironment.name || ""
                                if (backend.selectedEnvironment.fileAccessActive || false)
                                    backend.disableFileAccess(name)
                                else
                                    backend.enableFileAccess(name)
                            }
                        }
                    ]

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
                               ? theme.accentText
                               : theme.textMuted
                        font.family: (backend.selectedEnvironment.fileAccessActive || false)
                                     ? Style.fontMono
                                     : Style.fontSans
                        font.pixelSize: (backend.selectedEnvironment.fileAccessActive || false)
                                        ? Style.fontBody
                                        : Style.fontSmall
                        ToolTip.visible: rootUrlHover.hovered
                        ToolTip.delay: 400
                        ToolTip.text: (backend.selectedEnvironment.fileAccessActive || false)
                                      ? qsTr("Click to copy this SFTP URL")
                                      : qsTr("Browse this environment's root filesystem in your file manager over a localhost-only, passwordless SFTP connection. Anyone on this machine can read and write files as root while it is enabled.")

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
