import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts
import "../components"

Rectangle {
    id: restoreDialogRoot
    property QtObject app: null
    anchors.fill: parent
    visible: app.restoreDialogVisible
    color: "#c406070a"

    MouseArea {
        anchors.fill: parent
        onClicked: app.restoreDialogVisible = false
    }

    Rectangle {
        anchors.centerIn: parent
        width: Math.min(parent.width - 60, 520)
        implicitHeight: restoreCol.implicitHeight + 48
        radius: Math.min(8, theme.radius)
        color: app.surfaceCard
        border.width: 1
        border.color: theme.accent

        MouseArea { anchors.fill: parent }

        ColumnLayout {
            id: restoreCol
            anchors.fill: parent
            anchors.margins: 24
            spacing: 14

            ModalHeader {
                app: restoreDialogRoot.app
                title: qsTr("Restore Snapshot '%1'").arg(app.restoreSnapName)
                subtitle: qsTr("Choose what to restore for environment '%1':").arg(backend.selectedEnvironment.name || "")
                onClosed: app.restoreDialogVisible = false
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 8

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
                        radius: Math.min(4, theme.radius)
                        opacity: modelData.enabled ? 1.0 : 0.45
                        color: app.restoreMode === modelData.id ? app.surfaceCard : app.surfaceInput
                        border.width: 1
                        border.color: app.restoreMode === modelData.id ? theme.accent : app.borderSubtle

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
                                spacing: 2
                                Text {
                                    text: modelData.title
                                    color: theme.foreground
                                    font.family: app.sansFont
                                    font.pixelSize: 12
                                    font.weight: Font.Medium
                                }
                                Text {
                                    text: modelData.desc
                                    color: theme.muted
                                    font.family: app.sansFont
                                    font.pixelSize: 11
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
                implicitHeight: restoreNoteText.implicitHeight + 16
                radius: Math.min(4, theme.radius)
                color: app.surfaceInput
                border.width: 1
                border.color: app.borderSubtle

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
                    font.family: app.sansFont
                    font.pixelSize: 11
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: 4
                spacing: 10
                Item { Layout.fillWidth: true }

                ActionButton {
                    app: restoreDialogRoot.app
                    label: qsTr("Cancel")
                    minWidth: 96
                    onClicked: app.restoreDialogVisible = false
                }

                ActionButton {
                    app: restoreDialogRoot.app
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
    }
}
