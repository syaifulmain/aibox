import QtQuick
import QtQuick.Layouts
import "../components"

ModalDialog {
    id: wipeDialogRoot
    property QtObject app: null
    open: app.wipeVisible
    size: "sm"
    borderColor: theme.dangerBorder
    onDismissed: app.wipeVisible = false

    ModalHeader {
        title: qsTr("Wipe Environment '%1'?").arg(backend.selectedEnvironment.name || "")
        onClosed: app.wipeVisible = false
    }

    Rectangle {
        Layout.fillWidth: true
        implicitHeight: wipeWarnText.implicitHeight + Style.padCard * 2
        radius: Math.min(Style.radiusMd, theme.radius)
        color: theme.dangerSurface
        border.width: Style.hairline
        border.color: theme.dangerBorder

        Text {
            id: wipeWarnText
            anchors.fill: parent
            anchors.margins: Style.padCard
            wrapMode: Text.WordWrap
            lineHeight: 1.35
            text: qsTr("This will permanently destroy the entire Linux OS container, installed packages, databases, and snapshots inside '%1' in 1 second. Your host OS remains untouched.").arg(backend.selectedEnvironment.name || "")
            color: theme.dangerText
            font.family: Style.fontSans
            font.pixelSize: Style.fontBody
        }
    }

    // Optional extra: remove the shared workspace as well.
    CheckRow {
        checked: app.deleteWorkspaceOnWipe
        danger: true
        text: qsTr("Also delete shared project files and workspace snapshots in %1").arg(backend.selectedEnvironment.workspacePath || "")
        onToggled: app.deleteWorkspaceOnWipe = !app.deleteWorkspaceOnWipe
    }

    DialogFooter {
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
