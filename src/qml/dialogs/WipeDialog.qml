import QtQuick
import QtQuick.Layouts
import "../components"

ModalDialog {
    id: wipeDialogRoot
    property QtObject app: null
    open: app.wipeVisible
    cardWidth: Math.min(parent.width - Style.modalGutterWide, 500)
    contentSpacing: Style.space14
    borderColor: theme.red
    onDismissed: app.wipeVisible = false

    ModalHeader {
        title: qsTr("Wipe Environment '%1'?").arg(backend.selectedEnvironment.name || "")
        onClosed: app.wipeVisible = false
    }

    Text {
        Layout.fillWidth: true
        wrapMode: Text.WordWrap
        lineHeight: 1.35
        text: qsTr("This will permanently destroy the entire Linux OS container, installed packages, databases, and snapshots inside '%1' in 1 second. Your host OS remains untouched.").arg(backend.selectedEnvironment.name || "")
        color: theme.muted
        font.family: Style.fontSans
        font.pixelSize: Style.font12
    }

    // Optional extra: remove the shared workspace as well.
    Rectangle {
        Layout.fillWidth: true
        height: 38
        radius: Math.min(Style.radius4, theme.radius)
        color: Style.surfaceInput
        border.width: Style.hairline
        border.color: app.deleteWorkspaceOnWipe ? theme.red : Style.borderSubtle

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: Style.space12
            anchors.rightMargin: Style.space12
            spacing: Style.space10

            Rectangle {
                width: 16
                height: 16
                radius: Style.radius3
                color: app.deleteWorkspaceOnWipe ? theme.red : "transparent"
                border.width: Style.hairline
                border.color: app.deleteWorkspaceOnWipe ? theme.red : theme.muted
                Text {
                    anchors.centerIn: parent
                    visible: app.deleteWorkspaceOnWipe
                    text: "✓"
                    color: "black"
                    font.family: Style.fontMono
                    font.pixelSize: Style.font11
                }
            }

            Text {
                Layout.fillWidth: true
                elide: Text.ElideMiddle
                text: qsTr("Also delete shared project files and workspace snapshots in %1").arg(backend.selectedEnvironment.workspacePath || "")
                color: theme.foreground
                font.family: Style.fontSans
                font.pixelSize: Style.font12
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
        Layout.topMargin: Style.space4
        spacing: Style.space10
        Item { Layout.fillWidth: true }

        ActionButton {
            label: qsTr("Cancel")
            minWidth: 96
            onClicked: app.wipeVisible = false
        }

        ActionButton {
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
