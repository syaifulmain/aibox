import QtQuick
import QtQuick.Layouts
import "../components"

ModalDialog {
    id: restoreDialogRoot
    property QtObject app: null
    open: app.restoreDialogVisible
    cardWidth: Math.min(parent.width - Style.modalGutter, 520)
    contentSpacing: Style.space14
    onDismissed: app.restoreDialogVisible = false

    ModalHeader {
        title: qsTr("Restore Snapshot '%1'").arg(app.restoreSnapName)
        subtitle: qsTr("Choose what to restore for environment '%1':").arg(backend.selectedEnvironment.name || "")
        onClosed: app.restoreDialogVisible = false
    }

    ColumnLayout {
        Layout.fillWidth: true
        spacing: Style.space8

        Repeater {
            model: [
                { id: "both", title: qsTr("Both (System + Workspace)"), desc: qsTr("Restore OS container rootfs and %1 together.").arg(backend.containerWorkspacePath), enabled: app.restoreHasSystem && app.restoreHasWorkspace },
                { id: "system", title: qsTr("System only"), desc: qsTr("Restore OS container rootfs; leave %1 untouched.").arg(backend.containerWorkspacePath), enabled: app.restoreHasSystem },
                { id: "workspace", title: qsTr("Workspace only"), desc: qsTr("Restore %1 from Btrfs snapshot; leave OS packages untouched.").arg(backend.containerWorkspacePath), enabled: app.restoreHasWorkspace }
            ]
            delegate: Rectangle {
                required property var modelData
                Layout.fillWidth: true
                implicitHeight: 48
                radius: Math.min(Style.radius4, theme.radius)
                opacity: modelData.enabled ? 1.0 : 0.45
                color: app.restoreMode === modelData.id ? Style.surfaceCard : Style.surfaceInput
                border.width: Style.hairline
                border.color: app.restoreMode === modelData.id ? theme.accent : Style.borderSubtle

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Style.space12
                    anchors.rightMargin: Style.space12
                    spacing: Style.space10

                    // Radio indicator
                    Rectangle {
                        width: 14
                        height: 14
                        radius: 7
                        color: "transparent"
                        border.width: Style.hairline
                        border.color: app.restoreMode === modelData.id ? theme.accent : theme.muted
                        Rectangle {
                            anchors.centerIn: parent
                            width: 8
                            height: 8
                            radius: 4
                            visible: app.restoreMode === modelData.id
                            color: theme.accent
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: Style.space2
                        Text {
                            text: modelData.title
                            color: theme.foreground
                            font.family: Style.fontSans
                            font.pixelSize: Style.font12
                            font.weight: Font.Medium
                        }
                        Text {
                            text: modelData.desc
                            color: theme.muted
                            font.family: Style.fontSans
                            font.pixelSize: Style.font11
                        }
                    }
                }

                MouseArea {
                    anchors.fill: parent
                    enabled: modelData.enabled
                    cursorShape: modelData.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                    onClicked: app.restoreMode = modelData.id
                }
            }
        }
    }

    Rectangle {
        Layout.fillWidth: true
        visible: app.restoreMode !== "system"
        implicitHeight: restoreNoteText.implicitHeight + Style.space16
        radius: Math.min(Style.radius4, theme.radius)
        color: Style.surfaceInput
        border.width: Style.hairline
        border.color: Style.borderSubtle

        Text {
            id: restoreNoteText
            anchors.fill: parent
            anchors.margins: Style.space8
            wrapMode: Text.WordWrap
            lineHeight: 1.3
            text: (backend.selectedEnvironment.running || false)
                  ? qsTr("A safety snapshot (pre-restore) of your current %1 will be created first, and '%2' will be stopped automatically before swapping %1.").arg(backend.containerWorkspacePath).arg(backend.selectedEnvironment.name || "")
                  : qsTr("A safety snapshot (pre-restore) of your current %1 will be created before swapping %1.").arg(backend.containerWorkspacePath)
            color: theme.muted
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
            label: qsTr("Cancel")
            minWidth: 96
            onClicked: app.restoreDialogVisible = false
        }

        ActionButton {
            label: qsTr("Restore Snapshot")
            variant: "accent"
            minWidth: 155
            onClicked: {
                backend.restoreSnapshot(backend.selectedEnvironment.name || "", app.restoreSnapName, app.restoreMode)
                app.restoreDialogVisible = false
            }
        }
    }
}
