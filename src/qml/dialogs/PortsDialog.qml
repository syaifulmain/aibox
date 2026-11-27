import QtQuick
import QtQuick.Layouts
import "../components"

ModalDialog {
    id: portsOverlay
    property QtObject app: null
    readonly property var env: backend.selectedEnvironment
    property bool lanBind: false
    property bool lanConfirmed: false
    open: app.portsVisible
    zOrder: 87
    clipCard: true
    size: "md"
    cardHeight: Math.min(parent.height - Style.modalGutter, Style.modalWidthSm)
    onDismissed: {
        portsOverlay.lanBind = false
        portsOverlay.lanConfirmed = false
        app.portsVisible = false
    }

    function submitPort() {
        const hp = parseInt(hostPortInput.text.trim(), 10)
        const gp = parseInt(guestPortInput.text.trim(), 10)
        const err = backend.addPortForwardWithBind(
            portsOverlay.env.name || "",
            isNaN(hp) ? 0 : hp,
            isNaN(gp) ? (isNaN(hp) ? 0 : hp) : gp,
            portsOverlay.lanBind,
            portsOverlay.lanConfirmed)
        if (err.length > 0) {
            app.portForwardError = err
        } else {
            app.portForwardError = ""
            hostPortInput.text = ""
            guestPortInput.text = ""
            portsOverlay.lanBind = false
            portsOverlay.lanConfirmed = false
        }
    }

    ModalHeader {
        title: qsTr("Port Forwarding · %1").arg(portsOverlay.env.name || "")
        subtitle: qsTr("Binds container ports to 127.0.0.1 by default, or optionally 0.0.0.0 for LAN access")
        onClosed: app.portsVisible = false
    }

    Rectangle {
        visible: (portsOverlay.env.localDomain || "").length > 0
        Layout.fillWidth: true
        implicitHeight: domainRow.implicitHeight + Style.padCard * 2
        radius: Math.min(Style.radiusMd, theme.radius)
        color: theme.surfaceInput
        border.width: Style.hairline
        border.color: theme.borderSubtle

        RowLayout {
            id: domainRow
            anchors.fill: parent
            anchors.margins: Style.padCard
            spacing: Style.gap

            Text {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: portsOverlay.env.localDomain || ""
                color: theme.accentText
                font.family: Style.fontMono
                font.pixelSize: Style.fontBody
            }

            ActionButton {
                label: qsTr("Copy")
                onClicked: backend.copyText(
                    portsOverlay.env.localDomain || "",
                    qsTr("Local domain"))
            }

            ActionButton {
                label: qsTr("DNS Setup")
                onClicked: backend.copyText(
                    backend.localDomainSetupCommand(),
                    qsTr("DNS setup command"))
            }
        }
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
            visible: portsListView.count === 0
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
            model: portsOverlay.env.portForwards || []
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
                        text: (modelData.bindAddress || "127.0.0.1") + ":" + modelData.hostPort + "  →  " + modelData.guestPort
                        color: theme.textPrimary
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontBody
                    }

                    TagBadge {
                        visible: modelData.lanExposed || false
                        text: qsTr("LAN")
                        tone: "warning"
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

    CheckRow {
        text: qsTr("Expose on LAN (0.0.0.0)")
        checked: portsOverlay.lanBind
        danger: true
        disabled: backend.busy
        onToggled: {
            portsOverlay.lanBind = !portsOverlay.lanBind
            if (!portsOverlay.lanBind)
                portsOverlay.lanConfirmed = false
        }
    }

    CheckRow {
        visible: portsOverlay.lanBind
        text: qsTr("I understand devices on my local network can reach this port")
        checked: portsOverlay.lanConfirmed
        danger: true
        disabled: backend.busy
        onToggled: portsOverlay.lanConfirmed = !portsOverlay.lanConfirmed
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
