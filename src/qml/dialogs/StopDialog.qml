import QtQuick
import QtQuick.Layouts
import "../components"

// Confirms a stop before any task runs, so a single click cannot shut down a
// running environment by accident.
ModalDialog {
    id: stopDialogRoot
    property QtObject app: null
    readonly property var env: backend.selectedEnvironment
    open: app.stopConfirmVisible
    zOrder: 56
    size: "sm"
    borderColor: theme.dangerBorder
    onDismissed: app.stopConfirmVisible = false

    ModalHeader {
        title: qsTr("Stop Environment '%1'?").arg(stopDialogRoot.env.name || "")
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
            text: qsTr("This shuts down '%1' and ends all running processes. Unsaved work inside the OS may be lost. The environment and its workspace are kept.").arg(stopDialogRoot.env.name || "")
            color: theme.dangerText
            font.family: Style.fontSans
            font.pixelSize: Style.fontBody
        }
    }

    DialogFooter {
        ActionButton {
            label: qsTr("Stop Environment")
            variant: "danger"
            disabled: backend.busy || !(stopDialogRoot.env.running || false)
            onClicked: {
                if (stopDialogRoot.env.running)
                    backend.toggleStartStop(stopDialogRoot.env.name || "")
                app.stopConfirmVisible = false
            }
        }
    }
}
