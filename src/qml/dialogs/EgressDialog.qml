import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts
import "../components"

Rectangle {
    property QtObject app: null
    id: egressOverlay
    anchors.fill: parent
    visible: app.egressVisible
    color: "#c406070a"
    z: 86

    MouseArea {
        anchors.fill: parent
        onClicked: app.egressVisible = false
    }

    Rectangle {
        anchors.centerIn: parent
        width: Math.min(parent.width - 56, 780)
        height: Math.min(parent.height - 56, 560)
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
                app: egressOverlay.app
                title: qsTr("Network Egress Log · %1").arg(backend.egressLogsEnvName)
                subtitle: qsTr("Shows outgoing destinations captured from kernel bridge ACL logs")
                onClosed: app.egressVisible = false

                ActionButton {
                    app: egressOverlay.app
                    label: qsTr("Refresh Log")
                    minWidth: 104
                    onClicked: backend.refreshEgressLog(backend.egressLogsEnvName)
                }
            }

            // Table header
            RowLayout {
                Layout.fillWidth: true
                Layout.leftMargin: 10
                Layout.rightMargin: 10
                spacing: 10
                Text {
                    Layout.fillWidth: true
                    text: qsTr("Destination IP")
                    color: theme.muted
                    font.family: app.sansFont
                    font.pixelSize: 11
                }
                Text {
                    Layout.preferredWidth: 72
                    text: qsTr("Proto")
                    color: theme.muted
                    font.family: app.sansFont
                    font.pixelSize: 11
                }
                Text {
                    Layout.preferredWidth: 72
                    text: qsTr("Port")
                    color: theme.muted
                    font.family: app.sansFont
                    font.pixelSize: 11
                }
                Text {
                    Layout.preferredWidth: 64
                    text: qsTr("Hits")
                    color: theme.muted
                    font.family: app.sansFont
                    font.pixelSize: 11
                }
                Text {
                    Layout.preferredWidth: 88
                    text: qsTr("Last Seen")
                    color: theme.muted
                    font.family: app.sansFont
                    font.pixelSize: 11
                }
                Item { Layout.preferredWidth: 96 }
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
                    visible: backend.egressLogs.length === 0
                    text: qsTr("No egress entries captured yet. Enable 'Record: ON' on the environment and generate traffic.")
                    color: theme.muted
                    font.family: app.sansFont
                    font.pixelSize: 12
                    horizontalAlignment: Text.AlignHCenter
                    width: parent.width - 40
                    wrapMode: Text.WordWrap
                }

                ListView {
                    id: egressListView
                    anchors.fill: parent
                    anchors.margins: 8
                    clip: true
                    spacing: 6
                    model: backend.egressLogs
                    delegate: Rectangle {
                        required property var modelData
                        width: egressListView.width
                        height: 36
                        radius: Math.min(3, theme.radius)
                        color: app.surfaceCard
                        border.width: 1
                        border.color: app.borderSubtle

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 10
                            anchors.rightMargin: 8
                            spacing: 10

                            Text {
                                Layout.fillWidth: true
                                text: modelData.dstIp || ""
                                color: theme.foreground
                                font.family: app.monoFont
                                font.pixelSize: 12
                            }
                            Text {
                                Layout.preferredWidth: 72
                                text: modelData.proto || ""
                                color: theme.muted
                                font.family: app.monoFont
                                font.pixelSize: 11
                            }
                            Text {
                                Layout.preferredWidth: 72
                                text: modelData.port || "—"
                                color: theme.foreground
                                font.family: app.monoFont
                                font.pixelSize: 11
                            }
                            Text {
                                Layout.preferredWidth: 64
                                text: String(modelData.count || 0)
                                color: theme.accent
                                font.family: app.monoFont
                                font.pixelSize: 11
                            }
                            Text {
                                Layout.preferredWidth: 88
                                text: modelData.lastSeen || "—"
                                color: theme.muted
                                font.family: app.monoFont
                                font.pixelSize: 11
                            }
                            ActionButton {
                                app: egressOverlay.app
                                implicitHeight: 24
                                minWidth: 92
                                label: qsTr("+ Allow")
                                disabled: backend.busy
                                onClicked: backend.addEgressDestinationToAllowlist(
                                    backend.egressLogsEnvName, modelData.dstIp || "")
                            }
                        }
                    }
                }
            }
        }
    }
}
