import QtQuick
import QtQuick.Layouts
import "../components"

ModalDialog {
    id: portsOverlay
    property QtObject app: null
    open: app.portsVisible
    zOrder: 87
    clipCard: true
    size: "md"
    cardHeight: Math.min(parent.height - Style.modalGutter, Style.modalWidthSm)
    onDismissed: app.portsVisible = false

    function submitPort() {
        const hp = parseInt(hostPortInput.text.trim(), 10)
        const gp = parseInt(guestPortInput.text.trim(), 10)
        const err = backend.addPortForward(
            backend.selectedEnvironment.name || "",
            isNaN(hp) ? 0 : hp,
            isNaN(gp) ? (isNaN(hp) ? 0 : hp) : gp)
        if (err.length > 0) {
            app.portForwardError = err
        } else {
            app.portForwardError = ""
            hostPortInput.text = ""
            guestPortInput.text = ""
        }
    }

    ModalHeader {
        title: qsTr("Port Forwarding · %1").arg(backend.selectedEnvironment.name || "")
        subtitle: qsTr("Binds container ports to 127.0.0.1 on Host only (never 0.0.0.0)")
        onClosed: app.portsVisible = false
    }

    Rectangle {
        visible: app.portForwardError.length > 0
        Layout.fillWidth: true
        implicitHeight: portErrTxt.implicitHeight + Style.padCard * 2
        radius: Math.min(Style.radiusMd, theme.radius)
        color: theme.dangerSurface
        border.width: Style.hairline
        border.color: theme.dangerBorder
        Text {
            id: portErrTxt
            anchors.fill: parent
            anchors.margins: Style.padCard
            wrapMode: Text.WordWrap
            text: app.portForwardError
            color: theme.dangerText
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }
    }

    Rectangle {
        Layout.fillWidth: true
        Layout.fillHeight: true
        radius: Math.min(Style.radiusMd, theme.radius)
        color: theme.surfaceInput
        border.width: Style.hairline
        border.color: theme.borderSubtle

        Text {
            anchors.centerIn: parent
            visible: !backend.selectedEnvironment.portForwards || backend.selectedEnvironment.portForwards.length === 0
            text: qsTr("No forwarded ports configured for this environment.")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontBody
        }

        ListView {
            id: portsListView
            anchors.fill: parent
            anchors.margins: Style.gapMd
            clip: true
            spacing: Style.gapSm
            model: backend.selectedEnvironment.portForwards || []
            delegate: Rectangle {
                required property var modelData
                width: portsListView.width
                implicitHeight: portRow.implicitHeight + Style.padCard * 2
                radius: Math.min(Style.radiusSm, theme.radius)
                color: theme.surfaceCard
                border.width: Style.hairline
                border.color: theme.borderSubtle

                RowLayout {
                    id: portRow
                    anchors.fill: parent
                    anchors.leftMargin: Style.padCard
                    anchors.rightMargin: Style.padCard
                    spacing: Style.gap

                    Text {
                        Layout.fillWidth: true
                        text: "127.0.0.1:" + modelData.hostPort + "  →  " + modelData.guestPort
                        color: theme.textPrimary
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontBody
                    }

                    ActionButton {
                        label: qsTr("Open")
                        onClicked: backend.openForwardedPort(modelData.hostPort)
                    }

                    ActionButton {
                        label: qsTr("Copy")
                        onClicked: backend.copyText(
                            "http://127.0.0.1:" + modelData.hostPort,
                            qsTr("Port URL"))
                    }

                    ActionButton {
                        label: qsTr("Remove")
                        variant: "danger"
                        disabled: backend.busy
                        onClicked: backend.removePortForward(
                            backend.selectedEnvironment.name || "", modelData.deviceName || "")
                    }
                }
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: Style.gap

        FlatInput {
            id: hostPortInput
            Layout.fillWidth: true
            placeholder: qsTr("Host port (e.g. 3000)")
            onAccepted: portsOverlay.submitPort()
        }

        Text {
            text: "→"
            color: theme.textMuted
            font.family: Style.fontMono
            font.pixelSize: Style.fontBody
        }

        FlatInput {
            id: guestPortInput
            Layout.fillWidth: true
            placeholder: qsTr("Container port (e.g. 3000)")
            onAccepted: portsOverlay.submitPort()
        }
    }

    DialogFooter {
        ActionButton {
            label: qsTr("Add Port")
            variant: "primary"
            disabled: backend.busy
            onClicked: portsOverlay.submitPort()
        }
    }
}
