import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import "../components"

// Right pane of the Environments tab: inspector header, actions, resource
// cards, workspace access and the embedded network/snapshot sections.
Item {
    id: root
    property QtObject app: null
    readonly property var env: backend.selectedEnvironment
    readonly property int coreCount: (root.env.cpuCorePercents || []).length
    property string cpuExpandedEnv: ""
    readonly property bool cpuExpanded: cpuExpandedEnv.length > 0
                                         && cpuExpandedEnv === (root.env.name || "")
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
                model: root.coreCount

                delegate: ColumnLayout {
                    required property int index
                    readonly property real pct: (root.env.cpuCorePercents && index < root.env.cpuCorePercents.length)
                                                ? root.env.cpuCorePercents[index]
                                                : 0

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
                            text: pct + "%"
                            color: pct >= 85 ? theme.dangerText : theme.textPrimary
                            font.family: Style.fontMono
                            font.pixelSize: Style.fontCaption
                            font.weight: Font.Medium
                        }

                        Item { Layout.fillWidth: true }
                    }

                    MeterBar {
                        Layout.fillWidth: true
                        implicitHeight: Style.gapSm
                        value: pct
                        minFillWidth: Style.gapXs
                        trackColor: theme.surfaceTrack
                        fillColor: pct >= 85 ? theme.danger : theme.accent
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
                        text: root.env.name || ""
                        color: theme.textPrimary
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontDisplay
                        font.weight: Font.Medium
                        elide: Text.ElideRight
                    }

                    Text {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: (root.env.osName || "") + "  ·  " + (root.env.preset || "")
                        color: theme.textSecondary
                        font.family: Style.fontSans
                        font.pixelSize: Style.fontSmall
                    }
                }

                RowLayout {
                    spacing: Style.gap
                    Layout.alignment: Qt.AlignVCenter | Qt.AlignRight

                    ActionButton {
                        label: (root.env.running || false) ? qsTr("Stop OS") : qsTr("Start OS")
                        variant: (root.env.running || false) ? "danger" : "accent"
                        disabled: backend.busy || (root.env.isTemplate || false)
                        onClicked: app.requestStartStop()
                    }

                    ActionButton {
                        label: qsTr("More Actions")
                        disabled: backend.busy
                        onClicked: app.openEnvironmentActions()
                    }

                    ActionButton {
                        label: (root.env.isTemplate || false)
                               ? qsTr("New from Template")
                               : qsTr("Enter OS Terminal")
                        variant: "primary"
                        disabled: backend.busy
                        onClicked: {
                            if (root.env.isTemplate)
                                app.openCreateDialog("", root.env.name || "")
                            else
                                backend.enterEnvironment(root.env.name || "")
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
                    value: (root.env.diskUsage || "0 MB") + " / " + (root.env.diskLimit || "—")
                    badge: (root.env.diskAlertLevel === 2
                            ? qsTr("CRIT ")
                            : (root.env.diskAlertLevel === 1 ? qsTr("WARN ") : ""))
                           + (root.env.diskPercent !== undefined ? root.env.diskPercent : 0) + "%"
                    percent: root.env.diskPercent !== undefined ? root.env.diskPercent : 0
                    alertLevel: root.env.diskAlertLevel !== undefined ? root.env.diskAlertLevel : 0
                    hasBar: true
                    active: true
                }

                ResourceMeterCard {
                    Layout.fillWidth: true
                    Layout.preferredHeight: cpuCard.Layout.preferredHeight
                    title: qsTr("Memory Usage")
                    value: (root.env.memoryUsage || "0 MB") + " / " + (root.env.memoryLimit || "—")
                    badge: (root.env.memoryPercent !== undefined ? root.env.memoryPercent : 0) + "%"
                    percent: root.env.memoryPercent !== undefined ? root.env.memoryPercent : 0
                    hasBar: true
                    active: root.env.running || false
                }

                ResourceMeterCard {
                    id: cpuCard
                    Layout.fillWidth: true
                    title: qsTr("CPU Usage")
                    value: (root.env.cpuUsage || "0%") + " / " + (root.env.cpuLimit || "—")
                    badge: root.env.cpuUsage || "0%"
                    percent: root.env.cpuPercent !== undefined ? root.env.cpuPercent : 0
                    hasBar: true
                    active: root.env.running || false
                    interactive: true
                    expanded: root.cpuExpanded
                    expandedContent: cpuCoresGrid
                    hint: root.cpuExpanded
                          ? qsTr("Click to collapse core usage")
                          : qsTr("Click to expand core usage")
                    onClicked: root.cpuExpandedEnv = root.cpuExpanded
                                                    ? ""
                                                    : (root.env.name || "")
                }
            }

            Card {
                Layout.fillWidth: true
                title: qsTr("Swap Memory")
                subtitle: (root.env.type === "VIRTUAL-MACHINE"
                           || root.env.type === "VM")
                          ? qsTr("VM swap is managed inside the guest.")
                          : qsTr("Container swap is provided by the host.")

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Style.gap * 2

                    KeyValue {
                        label: qsTr("Allowance")
                        value: {
                            if ((root.env.type || "") === "VIRTUAL-MACHINE" || (root.env.type || "") === "VM")
                                return qsTr("Guest-managed")
                            const raw = root.env.swapLimit || ""
                            if (raw.length === 0)
                                return qsTr("Host")
                            if (raw === "Off")
                                return qsTr("Off")
                            return raw
                        }
                    }

                    KeyValue {
                        visible: (root.env.type || "") !== "VIRTUAL-MACHINE"
                                 && (root.env.type || "") !== "VM"
                        label: qsTr("Usage")
                        value: {
                            if (!(root.env.running || false))
                                return "—"
                            return (root.env.swapUsage || "0 B")
                                   + " / " + (root.env.swapLimit || qsTr("Host"))
                        }
                    }
                }

                MeterBar {
                    visible: (root.env.type || "") !== "VIRTUAL-MACHINE"
                             && (root.env.type || "") !== "VM"
                             && (root.env.running || false)
                    Layout.fillWidth: true
                    value: root.env.swapPercent || 0
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
                            label: (root.env.extraMountCount || 0) > 0
                                   ? qsTr("Mounts (%1)").arg(root.env.extraMountCount)
                                   : qsTr("Extra Mounts")
                            onClicked: app.openMountsDialog()
                        },
                        ActionButton {
                            label: qsTr("Open Shared Folder")
                            onClicked: backend.openWorkspaceFolder(root.env.name || "")
                        }
                    ]

                    KeyValue {
                        label: qsTr("Host ↔ Guest")
                        value: (root.env.workspacePath || "") + "  ⟷  " + backend.containerWorkspacePath
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
                    borderColor: (root.env.fileAccessActive || false)
                                 ? theme.accentBorder
                                 : theme.borderSubtle
                    actions: [
                        ActionButton {
                            visible: root.env.fileAccessActive || false
                            label: qsTr("Open")
                            onClicked: backend.openFileAccess(root.env.name || "")
                        },
                        ToggleChip {
                            label: qsTr("Enabled")
                            checked: root.env.fileAccessActive || false
                            disabled: backend.busy
                                      || (root.env.isTemplate || false)
                                      || (!(root.env.fileAccessActive || false)
                                          && !(root.env.running || false))
                            onToggled: {
                                const name = root.env.name || ""
                                if (root.env.fileAccessActive || false)
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
                        text: (root.env.fileAccessActive || false)
                              ? (root.env.fileAccessUrl || "")
                              : (root.env.running
                                 ? qsTr("Off — browse the container root in your file manager")
                                 : qsTr("Off — start the environment to enable"))
                        color: (root.env.fileAccessActive || false)
                               ? theme.accentText
                               : theme.textMuted
                        font.family: (root.env.fileAccessActive || false)
                                     ? Style.fontMono
                                     : Style.fontSans
                        font.pixelSize: (root.env.fileAccessActive || false)
                                        ? Style.fontBody
                                        : Style.fontSmall
                        ToolTip.visible: rootUrlHover.hovered
                        ToolTip.delay: 400
                        ToolTip.text: (root.env.fileAccessActive || false)
                                      ? qsTr("Click to copy this SFTP URL")
                                      : qsTr("Browse this environment's root filesystem in your file manager over a localhost-only, passwordless SFTP connection. Anyone on this machine can read and write files as root while it is enabled.")

                        HoverHandler {
                            id: rootUrlHover
                            cursorShape: (root.env.fileAccessActive || false)
                                         ? Qt.PointingHandCursor : Qt.ArrowCursor
                        }

                        TapHandler {
                            enabled: root.env.fileAccessActive || false
                            onTapped: backend.copyText(
                                root.env.fileAccessUrl || "",
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
