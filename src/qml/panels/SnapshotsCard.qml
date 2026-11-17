import QtQuick
import QtQuick.Layouts
import "../components"

// Snapshot list plus the per-environment snapshot, idle-stop and session
// recording policies.
Rectangle {
    id: root
    property QtObject app: null
    readonly property string snapshotNameText: snapNameInput.text

    Layout.fillWidth: true
    Layout.fillHeight: true
    Layout.minimumHeight: 150
    radius: Math.min(Style.radius4, theme.radius)
    color: Style.surfaceInput
    border.width: Style.hairline
    border.color: Style.borderSubtle

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Style.space14
        spacing: Style.space10

        // Header and snapshot actions
        RowLayout {
            Layout.fillWidth: true
            spacing: Style.space10

            ColumnLayout {
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                spacing: Style.space2

                Text {
                    text: qsTr("Btrfs Snapshots")
                    color: theme.foreground
                    font.family: Style.fontSans
                    font.pixelSize: Style.font12
                    font.weight: Font.Medium
                }
                Text {
                    Layout.fillWidth: true
                    elide: Text.ElideRight
                    text: qsTr("Before Enter & Start")
                    color: theme.muted
                    font.family: Style.fontSans
                    font.pixelSize: Style.font11
                }
            }

            Item { Layout.fillWidth: true }

            FlatInput {
                id: snapNameInput
                Layout.preferredWidth: 165
                Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                placeholder: qsTr("Snapshot label (optional)")
                onAccepted: {
                    if (!backend.busy) {
                        backend.createSnapshot(backend.selectedEnvironment.name || "", snapNameInput.text)
                        snapNameInput.text = ""
                    }
                }
            }

            ActionButton {
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

        // Compact snapshot and safety policies
        RowLayout {
            Layout.fillWidth: true
            spacing: Style.space8

            SelectChip {
                readonly property bool policyOn: backend.selectedEnvironment.autoSnapshotEnabled !== false
                text: policyOn ? qsTr("Auto-snapshot: ON") : qsTr("Auto-snapshot: OFF")
                selected: policyOn
                mono: false
                textSize: Style.font10
                chipHeight: Style.controlHeightXs
                horizontalPadding: Style.space12
                interactive: !backend.busy
                onClicked: backend.setEnvironmentAutoSnapshot(
                    backend.selectedEnvironment.name || "", !policyOn)
            }

            SelectChip {
                readonly property bool policyOn: backend.selectedEnvironment.workspaceSnapshotEnabled !== false
                visible: backend.selectedEnvironment.workspaceIsSubvolume || false
                text: policyOn ? qsTr("Workspace: ON") : qsTr("Workspace: OFF")
                selected: policyOn
                mono: false
                textSize: Style.font10
                chipHeight: Style.controlHeightXs
                horizontalPadding: Style.space12
                interactive: !backend.busy
                onClicked: backend.setEnvironmentWorkspaceSnapshot(
                    backend.selectedEnvironment.name || "", !policyOn)
            }

            SelectChip {
                readonly property bool policyOn: backend.selectedEnvironment.autoStopOnCritical || false
                text: policyOn ? qsTr("Auto-stop critical: ON") : qsTr("Auto-stop critical: OFF")
                selected: policyOn
                mono: false
                textSize: Style.font10
                selectedFill: Style.dangerSurface
                selectedBorderColor: theme.red
                selectedTextColor: theme.red
                chipHeight: Style.controlHeightXs
                horizontalPadding: Style.space12
                interactive: !backend.busy
                onClicked: backend.setEnvironmentAutoStop(
                    backend.selectedEnvironment.name || "", !policyOn)
            }

            Item { Layout.fillWidth: true }
        }

        Rectangle {
            Layout.fillWidth: true
            height: Style.hairline
            color: Style.borderSubtle
        }

        // One-time conversion banner for plain-directory workspaces on Btrfs
        RowLayout {
            Layout.fillWidth: true
            spacing: Style.space10
            visible: ((backend.selectedEnvironment.workspaceOnBtrfs || false) && !(backend.selectedEnvironment.workspaceIsSubvolume || false)) || (backend.selectedEnvironment.hasPreSubvolBackup || false)

            Text {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: (backend.selectedEnvironment.hasPreSubvolBackup || false)
                      ? qsTr("Pre-subvolume backup (.pre-subvol) is kept on disk.")
                      : qsTr("Workspace is a plain folder. Convert to Btrfs subvolume to enable paired %1 snapshots.").arg(backend.containerWorkspacePath)
                color: theme.muted
                font.family: Style.fontSans
                font.pixelSize: Style.font11
            }

            ActionButton {
                implicitHeight: Style.controlHeightSm
                visible: (backend.selectedEnvironment.workspaceOnBtrfs || false) && !(backend.selectedEnvironment.workspaceIsSubvolume || false)
                label: qsTr("Convert to Subvolume")
                disabled: backend.busy || (backend.selectedEnvironment.running || false)
                onClicked: backend.convertWorkspaceToSubvolume(backend.selectedEnvironment.name || "")
            }

            ActionButton {
                implicitHeight: Style.controlHeightSm
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
            font.family: Style.fontSans
            font.pixelSize: Style.font12
        }

        ListView {
            id: snapListView
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: Style.space6
            model: backend.selectedEnvironment.snapshots || []

            delegate: Rectangle {
                required property string modelData
                readonly property bool hasWsHalf: (backend.selectedEnvironment.workspaceSnapshots || []).indexOf(modelData) >= 0
                width: snapListView.width
                height: Style.rowHeight
                radius: Math.min(Style.radius3, theme.radius)
                color: Style.surfaceCard
                border.width: Style.hairline
                border.color: Style.borderSubtle

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Style.space12
                    anchors.rightMargin: Style.space10
                    spacing: Style.space10

                    Text {
                        text: "●"
                        color: theme.accent
                        font.pixelSize: Style.font10
                    }

                    TagBadge {
                        visible: modelData.startsWith("auto-")
                        text: qsTr("AUTO")
                        tone: "quiet"
                    }

                    TagBadge {
                        visible: hasWsHalf
                        text: qsTr("W")
                        tone: "accent"
                        fillColor: Style.surfaceInput
                        textWeight: Font.Bold
                    }

                    Text {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        elide: Text.ElideRight
                        text: modelData
                        color: theme.foreground
                        font.family: Style.fontMono
                        font.pixelSize: Style.font12
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
                        font.family: Style.fontMono
                        font.pixelSize: Style.font11
                    }

                    ActionButton {
                        implicitHeight: Style.controlHeightXxs
                        visible: hasWsHalf
                        label: qsTr("Changes")
                        onClicked: app.openChangeReviewDialog(modelData)
                    }

                    ActionButton {
                        implicitHeight: Style.controlHeightXxs
                        label: qsTr("Restore")
                        disabled: backend.busy
                        onClicked: app.openRestoreDialog(modelData, true, hasWsHalf)
                    }

                    ActionButton {
                        implicitHeight: Style.controlHeightXxs
                        label: qsTr("Delete")
                        danger: true
                        disabled: backend.busy
                        onClicked: backend.deleteSnapshot(backend.selectedEnvironment.name || "", modelData)
                    }
                }
            }
        }

        // Workspace-only snapshots (pre-restore safety snapshots or orphans)
        ColumnLayout {
            Layout.fillWidth: true
            spacing: Style.space4
            visible: (backend.selectedEnvironment.orphanWorkspaceSnapshots || []).length > 0

            Text {
                text: qsTr("Workspace-only snapshots")
                color: theme.muted
                font.family: Style.fontSans
                font.pixelSize: Style.font11
            }

            Repeater {
                model: backend.selectedEnvironment.orphanWorkspaceSnapshots || []

                delegate: Rectangle {
                    required property string modelData
                    Layout.fillWidth: true
                    height: Style.rowHeight
                    radius: Math.min(Style.radius3, theme.radius)
                    color: Style.surfaceCard
                    border.width: Style.hairline
                    border.color: Style.borderSubtle

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: Style.space12
                        anchors.rightMargin: Style.space10
                        spacing: Style.space10

                        TagBadge {
                            text: qsTr("WS ONLY")
                            tone: "quiet"
                        }

                        Text {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            elide: Text.ElideRight
                            text: modelData
                            color: theme.foreground
                            font.family: Style.fontMono
                            font.pixelSize: Style.font11
                        }

                        Text {
                            readonly property string wsOnlySize: (backend.selectedEnvironment.workspaceSnapshotSizes
                                                                  && backend.selectedEnvironment.workspaceSnapshotSizes[modelData])
                                                                 ? backend.selectedEnvironment.workspaceSnapshotSizes[modelData]
                                                                 : ""
                            visible: wsOnlySize.length > 0
                            text: wsOnlySize
                            color: theme.muted
                            font.family: Style.fontMono
                            font.pixelSize: Style.font11
                        }

                        ActionButton {
                            implicitHeight: Style.controlHeightXxs
                            label: qsTr("Changes")
                            onClicked: app.openChangeReviewDialog(modelData)
                        }

                        ActionButton {
                            implicitHeight: Style.controlHeightXxs
                            label: qsTr("Restore")
                            disabled: backend.busy
                            onClicked: app.openRestoreDialog(modelData, false, true)
                        }

                        ActionButton {
                            implicitHeight: Style.controlHeightXxs
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
