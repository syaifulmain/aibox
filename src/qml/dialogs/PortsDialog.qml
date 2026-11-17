import QtQuick
import QtQuick.Layouts
import "../components"

ModalDialog {
    id: portsOverlay
    property QtObject app: null
    open: app.portsVisible
    zOrder: 87
    clipCard: true
    cardWidth: Math.min(parent.width - Style.modalGutter, 680)
    cardHeight: Math.min(parent.height - Style.modalEdge, 520)
    onDismissed: app.portsVisible = false

    ModalHeader {
        title: qsTr("Port Forwarding · %1").arg(backend.selectedEnvironment.name || "")
        subtitle: qsTr("Binds container ports to 127.0.0.1 on Host only (never 0.0.0.0)")
        onClosed: app.portsVisible = false
    }

    Rectangle {
        visible: app.portForwardError.length > 0
        Layout.fillWidth: true
        height: Style.controlHeight
        radius: Math.min(Style.radius4, theme.radius)
        color: Style.dangerSurface
        border.width: Style.hairline
        border.color: theme.red
        Text {
            anchors.fill: parent
            anchors.leftMargin: Style.space12
            anchors.rightMargin: Style.space12
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
            text: app.portForwardError
            color: theme.red
            font.family: Style.fontSans
            font.pixelSize: Style.font11
        }
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: Style.space10

        FlatInput {
            id: hostPortInput
            Layout.preferredWidth: 145
            placeholder: qsTr("Host port (e.g. 3000)")
        }

        Text {
            text: "→"
            color: theme.muted
            font.family: Style.fontMono
            font.pixelSize: Style.font13
        }

        FlatInput {
            id: guestPortInput
            Layout.preferredWidth: 155
            placeholder: qsTr("Container port (e.g. 3000)")
        }

        Item { Layout.fillWidth: true }

        ActionButton {
            label: qsTr("+ Add Forward")
            variant: "primary"
            minWidth: 132
            disabled: backend.busy
            onClicked: {
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
        }
    }

    Rectangle {
        Layout.fillWidth: true
        Layout.fillHeight: true
        radius: Math.min(Style.radius4, theme.radius)
        color: Style.surfaceInput
        border.width: Style.hairline
        border.color: Style.borderSubtle

        Text {
            anchors.centerIn: parent
            visible: !backend.selectedEnvironment.portForwards || backend.selectedEnvironment.portForwards.length === 0
            text: qsTr("No forwarded ports configured for this environment.")
            color: theme.muted
            font.family: Style.fontSans
            font.pixelSize: Style.font12
        }

        ListView {
            id: portsListView
            anchors.fill: parent
            anchors.margins: Style.space8
            clip: true
            spacing: Style.space6
            model: backend.selectedEnvironment.portForwards || []
            delegate: Rectangle {
                required property var modelData
                width: portsListView.width
                height: 48
                radius: Math.min(Style.radius3, theme.radius)
                color: Style.surfaceCard
                border.width: Style.hairline
                border.color: Style.borderSubtle

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Style.space12
                    anchors.rightMargin: Style.space12
                    spacing: Style.space10

                    Text {
                        Layout.fillWidth: true
                        text: "127.0.0.1:" + modelData.hostPort + "  →  " + modelData.guestPort
                        color: theme.foreground
                        font.family: Style.fontMono
                        font.pixelSize: Style.font12
                    }

                    ActionButton {
                        minWidth: 78
                        label: qsTr("Open")
                        onClicked: backend.openForwardedPort(modelData.hostPort)
                    }

                    ActionButton {
                        minWidth: 78
                        label: qsTr("Copy")
                        onClicked: backend.copyText(
                            "http://127.0.0.1:" + modelData.hostPort,
                            qsTr("Port URL"))
                    }

                    ActionButton {
                        minWidth: 84
                        label: qsTr("Remove")
                        danger: true
                        disabled: backend.busy
                        onClicked: backend.removePortForward(
                            backend.selectedEnvironment.name || "", modelData.deviceName || "")
                    }
                }
            }
        }
    }
}
