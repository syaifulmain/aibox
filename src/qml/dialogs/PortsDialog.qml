import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts
import "../components"

Rectangle {
    property QtObject app: null
    id: portsOverlay
    anchors.fill: parent
    visible: app.portsVisible
    color: "#c406070a"
    z: 87

    MouseArea {
        anchors.fill: parent
        onClicked: app.portsVisible = false
    }

    Rectangle {
        anchors.centerIn: parent
        width: Math.min(parent.width - 56, 680)
        height: Math.min(parent.height - 56, 520)
        radius: Math.min(8, theme.radius)
        color: app.surfaceCard
        border.width: 1
        border.color: theme.accent
        clip: true

        MouseArea { anchors.fill: parent }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 24
            spacing: 12

            ModalHeader {
                app: portsOverlay.app
                title: qsTr("Port Forwarding · %1").arg(backend.selectedEnvironment.name || "")
                subtitle: qsTr("Binds container ports to 127.0.0.1 on Host only (never 0.0.0.0)")
                onClosed: app.portsVisible = false
            }

            Rectangle {
                visible: app.portForwardError.length > 0
                Layout.fillWidth: true
                height: 34
                radius: Math.min(4, theme.radius)
                color: "#261319"
                border.width: 1
                border.color: theme.red
                Text {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    verticalAlignment: Text.AlignVCenter
                    elide: Text.ElideRight
                    text: app.portForwardError
                    color: theme.red
                    font.family: app.sansFont
                    font.pixelSize: 11
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 10
                FlatInput {
                    app: portsOverlay.app
                    id: hostPortInput
                    Layout.preferredWidth: 145
                    implicitHeight: 34
                    placeholder: qsTr("Host port (e.g. 3000)")
                }
                Text {
                    text: "→"
                    color: theme.muted
                    font.family: app.monoFont
                    font.pixelSize: 13
                }
                FlatInput {
                    app: portsOverlay.app
                    id: guestPortInput
                    Layout.preferredWidth: 155
                    implicitHeight: 34
                    placeholder: qsTr("Container port (e.g. 3000)")
                }
                Item { Layout.fillWidth: true }
                ActionButton {
                    app: portsOverlay.app
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
                radius: Math.min(4, theme.radius)
                color: app.surfaceInput
                border.width: 1
                border.color: app.borderSubtle

                Text {
                    anchors.centerIn: parent
                    visible: !backend.selectedEnvironment.portForwards || backend.selectedEnvironment.portForwards.length === 0
                    text: qsTr("No forwarded ports configured for this environment.")
                    color: theme.muted
                    font.family: app.sansFont
                    font.pixelSize: 12
                }

                ListView {
                    id: portsListView
                    anchors.fill: parent
                    anchors.margins: 8
                    clip: true
                    spacing: 6
                    model: backend.selectedEnvironment.portForwards || []
                    delegate: Rectangle {
                        required property var modelData
                        width: portsListView.width
                        height: 48
                        radius: Math.min(3, theme.radius)
                        color: app.surfaceCard
                        border.width: 1
                        border.color: app.borderSubtle

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 12
                            spacing: 10
                            Text {
                                Layout.fillWidth: true
                                text: "127.0.0.1:" + modelData.hostPort + "  →  " + modelData.guestPort
                                color: theme.foreground
                                font.family: app.monoFont
                                font.pixelSize: 12
                            }
                            ActionButton {
                                app: portsOverlay.app
                                minWidth: 78
                                label: qsTr("Open")
                                onClicked: backend.openForwardedPort(modelData.hostPort)
                            }
                            ActionButton {
                                app: portsOverlay.app
                                minWidth: 78
                                label: qsTr("Copy")
                                onClicked: backend.copyText(
                                    "http://127.0.0.1:" + modelData.hostPort,
                                    qsTr("Port URL"))
                            }
                            ActionButton {
                                app: portsOverlay.app
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
    }
}
