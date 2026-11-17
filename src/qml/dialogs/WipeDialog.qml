import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts
import "../components"

Rectangle {
    id: wipeDialogRoot
    property QtObject app: null
    anchors.fill: parent
    visible: app.wipeVisible
    color: "#c406070a"

    MouseArea {
        anchors.fill: parent
        onClicked: app.wipeVisible = false
    }

    Rectangle {
        anchors.centerIn: parent
        width: Math.min(parent.width - 60, 500)
        height: wipeCol.implicitHeight + 48
        radius: Math.min(8, theme.radius)
        color: app.surfaceCard
        border.width: 1
        border.color: theme.red

        MouseArea { anchors.fill: parent }

        ColumnLayout {
            id: wipeCol
            anchors.fill: parent
            anchors.margins: 24
            spacing: 14

            ModalHeader {
                app: wipeDialogRoot.app
                title: qsTr("Wipe Environment '%1'?").arg(backend.selectedEnvironment.name || "")
                onClosed: app.wipeVisible = false
            }

            Text {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                lineHeight: 1.35
                text: qsTr("This will permanently destroy the entire Linux OS container, installed packages, databases, and snapshots inside '%1' in 1 second. Your host OS remains untouched.").arg(backend.selectedEnvironment.name || "")
                color: theme.muted
                font.family: app.sansFont
                font.pixelSize: 12
            }

            // Custom clean checkbox row
            Rectangle {
                Layout.fillWidth: true
                height: 38
                radius: Math.min(4, theme.radius)
                color: app.surfaceInput
                border.width: 1
                border.color: app.deleteWorkspaceOnWipe ? theme.red : app.borderSubtle

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    spacing: 10

                    Rectangle {
                        width: 16
                        height: 16
                        radius: 3
                        color: app.deleteWorkspaceOnWipe ? theme.red : "transparent"
                        border.width: 1
                        border.color: app.deleteWorkspaceOnWipe ? theme.red : theme.muted
                        Text {
                            anchors.centerIn: parent
                            visible: app.deleteWorkspaceOnWipe
                            text: "✓"
                            color: "black"
                            font.family: app.monoFont
                            font.pixelSize: 11
                        }
                    }

                    Text {
                        Layout.fillWidth: true
                        elide: Text.ElideMiddle
                        text: qsTr("Also delete shared project files and workspace snapshots in %1").arg(backend.selectedEnvironment.workspacePath || "")
                        color: theme.foreground
                        font.family: app.sansFont
                        font.pixelSize: 12
                    }
                }

                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: app.deleteWorkspaceOnWipe = !app.deleteWorkspaceOnWipe
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: 4
                spacing: 10
                Item { Layout.fillWidth: true }

                ActionButton {
                    app: wipeDialogRoot.app
                    label: qsTr("Cancel")
                    minWidth: 96
                    onClicked: app.wipeVisible = false
                }

                ActionButton {
                    app: wipeDialogRoot.app
                    label: qsTr("Wipe Environment Now")
                    variant: "danger"
                    minWidth: 185
                    onClicked: {
                        backend.wipeEnvironment(backend.selectedEnvironment.name || "", app.deleteWorkspaceOnWipe)
                        app.wipeVisible = false
                    }
                }
            }
        }
    }
}
