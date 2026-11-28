import QtQuick
import QtQuick.Layouts
import "../components"

// Snapshot list plus the per-environment snapshot, idle-stop and session
// recording policies.
Card {
    id: root
    property QtObject app: null
    property var env: backend.selectedEnvironment
    readonly property string snapshotNameText: snapNameInput.text

    function clearSnapshotName() {
        snapNameInput.text = ""
    }

    Layout.fillHeight: true
    Layout.minimumHeight: 150
    title: qsTr("Btrfs Snapshots")
    subtitle: qsTr("Before Enter & Start")
    actions: [
        FlatInput {
            id: snapNameInput
            Layout.preferredWidth: 165
            Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
            placeholder: qsTr("Snapshot label (optional)")
            onAccepted: {
                if (!backend.busy) {
                    backend.createSnapshot(root.env.name || "", snapNameInput.text)
                    snapNameInput.text = ""
                }
            }
        },
        ActionButton {
            label: qsTr("+ Save Snapshot")
            disabled: backend.busy
            Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
            onClicked: {
                backend.createSnapshot(root.env.name || "", snapNameInput.text)
                snapNameInput.text = ""
            }
        }
    ]

    // Snapshot and safety policy toggles (34 px)
    RowLayout {
        Layout.fillWidth: true
        spacing: Style.gap

        ToggleChip {
            readonly property bool policyOn: root.env.autoSnapshotEnabled !== false
            label: qsTr("Auto-snapshot")
            checked: policyOn
            disabled: backend.busy
            onToggled: backend.setEnvironmentAutoSnapshot(
                root.env.name || "", !policyOn)
        }

        ToggleChip {
            readonly property bool policyOn: root.env.workspaceSnapshotEnabled !== false
            visible: root.env.workspaceIsSubvolume || false
            label: qsTr("Workspace")
            checked: policyOn
            disabled: backend.busy
            onToggled: backend.setEnvironmentWorkspaceSnapshot(
                root.env.name || "", !policyOn)
        }

        ToggleChip {
            readonly property bool policyOn: root.env.autoStopOnCritical || false
            label: qsTr("Auto-stop critical")
            checked: policyOn
            disabled: backend.busy
            onToggled: backend.setEnvironmentAutoStop(
                root.env.name || "", !policyOn)
        }

        Item { Layout.fillWidth: true }
    }

    Rectangle {
        Layout.fillWidth: true
        height: Style.hairline
        color: theme.borderSubtle
    }

    // One-time conversion banner for plain-directory workspaces on Btrfs
    RowLayout {
        Layout.fillWidth: true
        spacing: Style.gap
        visible: ((root.env.workspaceOnBtrfs || false) && !(root.env.workspaceIsSubvolume || false)) || (root.env.hasPreSubvolBackup || false)

        Text {
            Layout.fillWidth: true
            elide: Text.ElideRight
            text: (root.env.hasPreSubvolBackup || false)
                  ? qsTr("Pre-subvolume backup (.pre-subvol) is kept on disk.")
                  : qsTr("Workspace is a plain folder. Convert to Btrfs subvolume to enable paired %1 snapshots.").arg(backend.containerWorkspacePath)
            color: theme.textSecondary
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }

        ActionButton {
            visible: (root.env.workspaceOnBtrfs || false) && !(root.env.workspaceIsSubvolume || false)
            label: qsTr("Convert to Subvolume")
            disabled: backend.busy || (root.env.running || false)
            onClicked: backend.convertWorkspaceToSubvolume(root.env.name || "")
        }

        ActionButton {
            visible: root.env.hasPreSubvolBackup || false
            label: qsTr("Delete .pre-subvol")
            variant: "danger"
            disabled: backend.busy
            onClicked: backend.deletePreSubvolBackup(root.env.name || "")
        }
    }

    Text {
        visible: (!root.env.snapshots || root.env.snapshots.length === 0) && (!root.env.orphanWorkspaceSnapshots || root.env.orphanWorkspaceSnapshots.length === 0)
        Layout.fillWidth: true
        horizontalAlignment: Text.AlignHCenter
        text: qsTr("No snapshots saved for this environment yet.")
        color: theme.textMuted
        font.family: Style.fontSans
        font.pixelSize: Style.fontSmall
    }

    ListView {
        id: snapListView
        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        reuseItems: true
        spacing: Style.gapSm
        model: root.env.snapshots || []

        delegate: Rectangle {
            required property string modelData
            readonly property bool hasWsHalf: (root.env.workspaceSnapshots || []).indexOf(modelData) >= 0
            width: snapListView.width
            height: Style.rowHeight
            radius: Math.min(Style.radiusSm, theme.radius)
            color: theme.surfaceInput
            border.width: Style.hairline
            border.color: theme.borderSubtle

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: Style.padCard
                anchors.rightMargin: Style.gap
                spacing: Style.gap

                Text {
                    text: "●"
                    color: theme.accent
                    font.pixelSize: Style.fontCaption
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
                    fillColor: theme.surfaceCard
                    textWeight: Font.Bold
                }

                Text {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    elide: Text.ElideRight
                    text: modelData
                    color: theme.textPrimary
                    font.family: Style.fontMono
                    font.pixelSize: Style.fontBody
                }

                Text {
                    readonly property string snapSize: (root.env.snapshotSizes
                                                        && root.env.snapshotSizes[modelData])
                                                       ? root.env.snapshotSizes[modelData]
                                                       : ""
                    readonly property string wsSize: (root.env.workspaceSnapshotSizes
                                                      && root.env.workspaceSnapshotSizes[modelData])
                                                     ? root.env.workspaceSnapshotSizes[modelData]
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
                    color: theme.textMuted
                    font.family: Style.fontMono
                    font.pixelSize: Style.fontSmall
                }

                ActionButton {
                    compact: true
                    visible: hasWsHalf
                    label: qsTr("Changes")
                    onClicked: app.openChangeReviewDialog(modelData)
                }

                ActionButton {
                    compact: true
                    label: qsTr("Restore")
                    disabled: backend.busy
                    onClicked: app.openRestoreDialog(modelData, true, hasWsHalf)
                }

                ActionButton {
                    compact: true
                    label: qsTr("Delete")
                    variant: "danger"
                    disabled: backend.busy
                    onClicked: backend.deleteSnapshot(root.env.name || "", modelData)
                }
            }
        }
    }

    // Workspace-only snapshots (pre-restore safety snapshots or orphans)
    ColumnLayout {
        Layout.fillWidth: true
        spacing: Style.gapSm
        visible: (root.env.orphanWorkspaceSnapshots || []).length > 0

        Text {
            text: qsTr("Workspace-only snapshots")
            color: theme.textSecondary
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }

        Repeater {
            model: root.env.orphanWorkspaceSnapshots || []

            delegate: Rectangle {
                required property string modelData
                Layout.fillWidth: true
                height: Style.rowHeight
                radius: Math.min(Style.radiusSm, theme.radius)
                color: theme.surfaceInput
                border.width: Style.hairline
                border.color: theme.borderSubtle

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Style.padCard
                    anchors.rightMargin: Style.gap
                    spacing: Style.gap

                    TagBadge {
                        text: qsTr("WS ONLY")
                        tone: "quiet"
                    }

                    Text {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        elide: Text.ElideRight
                        text: modelData
                        color: theme.textPrimary
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontSmall
                    }

                    Text {
                        readonly property string wsOnlySize: (root.env.workspaceSnapshotSizes
                                                              && root.env.workspaceSnapshotSizes[modelData])
                                                             ? root.env.workspaceSnapshotSizes[modelData]
                                                             : ""
                        visible: wsOnlySize.length > 0
                        text: wsOnlySize
                        color: theme.textMuted
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontSmall
                    }

                    ActionButton {
                        compact: true
                        label: qsTr("Changes")
                        onClicked: app.openChangeReviewDialog(modelData)
                    }

                    ActionButton {
                        compact: true
                        label: qsTr("Restore")
                        disabled: backend.busy
                        onClicked: app.openRestoreDialog(modelData, false, true)
                    }

                    ActionButton {
                        compact: true
                        label: qsTr("Delete")
                        variant: "danger"
                        disabled: backend.busy
                        onClicked: backend.deleteSnapshot(root.env.name || "", modelData)
                    }
                }
            }
        }
    }
}
