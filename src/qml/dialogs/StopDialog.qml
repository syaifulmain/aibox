import QtQuick
import QtQuick.Layouts
import "../components"

// Confirms a stop before any task runs, so a single click cannot shut down a
// running environment by accident.
ModalDialog {
    id: stopDialogRoot
    property QtObject app: null
    open: app.stopConfirmVisible
    zOrder: 56
    size: "sm"
    borderColor: theme.dangerBorder
    onDismissed: app.stopConfirmVisible = false

    ModalHeader {
        title: qsTr("Stop Environment '%1'?").arg(backend.selectedEnvironment.name || "")
        onClosed: app.stopConfirmVisible = false
    }

    Rectangle {
        Layout.fillWidth: true
        implicitHeight: stopWarnText.implicitHeight + Style.padCard * 2
        radius: Math.min(Style.radiusMd, theme.radius)
        color: theme.dangerSurface
        border.width: Style.hairline
        border.color: theme.dangerBorder

        Text {
            id: stopWarnText
            anchors.fill: parent
            anchors.margins: Style.padCard
            wrapMode: Text.WordWrap
            lineHeight: 1.35
            text: qsTr("This shuts down '%1' and ends all running processes. Unsaved work inside the OS may be lost. The environment and its workspace are kept.").arg(backend.selectedEnvironment.name || "")
            color: theme.dangerText
            font.family: Style.fontSans
            font.pixelSize: Style.fontBody
        }
    }

    DialogFooter {
        ActionButton {
            label: qsTr("Stop Environment")
            variant: "danger"
            disabled: backend.busy
            onClicked: {
                backend.toggleStartStop(backend.selectedEnvironment.name || "")
                app.stopConfirmVisible = false
            }
        }
    }
}
