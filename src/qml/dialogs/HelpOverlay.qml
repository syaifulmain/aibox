import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts
import "../components"

Rectangle {
    id: helpOverlayRoot
    property QtObject app: null
    anchors.fill: parent
    visible: app.helpVisible
    color: "#c406070a"

    MouseArea {
        anchors.fill: parent
        onClicked: app.helpVisible = false
    }

    Rectangle {
        anchors.centerIn: parent
        width: Math.min(parent.width - 60, 480)
        height: helpCol.implicitHeight + 48
        radius: Math.min(8, theme.radius)
        color: app.surfaceCard
        border.width: 1
        border.color: theme.accent

        MouseArea { anchors.fill: parent }

        ColumnLayout {
            id: helpCol
            anchors.fill: parent
            anchors.margins: 24
            spacing: 10

            ModalHeader {
                app: helpOverlayRoot.app
                title: qsTr("Keyboard Shortcuts")
                onClosed: app.helpVisible = false
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
                        color: app.surfaceInput
                        border.width: 1
                        border.color: app.borderSubtle
                        Text {
                            anchors.centerIn: parent
                            text: modelData.k
                            color: theme.accent
                            font.family: app.monoFont
                            font.pixelSize: 11
                        }
                    }
                    Text {
                        Layout.fillWidth: true
                        text: modelData.d
                        color: theme.muted
                        font.family: app.sansFont
                        font.pixelSize: 12
                    }
                }
            }
        }
    }
}
