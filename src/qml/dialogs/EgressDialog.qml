import QtQuick
import QtQuick.Layouts
import "../components"

ModalDialog {
    id: egressOverlay
    property QtObject app: null
    open: app.egressVisible
    zOrder: 86
    clipCard: true
    cardWidth: Math.min(parent.width - Style.modalGutter, 780)
    cardHeight: Math.min(parent.height - Style.modalEdge, 560)
    onDismissed: app.egressVisible = false

    ModalHeader {
        title: qsTr("Network Egress Log · %1").arg(backend.egressLogsEnvName)
        subtitle: qsTr("Shows outgoing destinations captured from kernel bridge ACL logs")
        onClosed: app.egressVisible = false

        ActionButton {
            label: qsTr("Refresh Log")
            minWidth: 104
            onClicked: backend.refreshEgressLog(backend.egressLogsEnvName)
        }
    }

    // Table header
    RowLayout {
        Layout.fillWidth: true
        Layout.leftMargin: Style.space10
        Layout.rightMargin: Style.space10
        spacing: Style.space10

        Text {
            Layout.fillWidth: true
            text: qsTr("Destination IP")
            color: theme.muted
            font.family: Style.fontSans
            font.pixelSize: Style.font11
        }
        Text {
            Layout.preferredWidth: 72
            text: qsTr("Proto")
            color: theme.muted
            font.family: Style.fontSans
            font.pixelSize: Style.font11
        }
        Text {
            Layout.preferredWidth: 72
            text: qsTr("Port")
            color: theme.muted
            font.family: Style.fontSans
            font.pixelSize: Style.font11
        }
        Text {
            Layout.preferredWidth: 64
            text: qsTr("Hits")
            color: theme.muted
            font.family: Style.fontSans
            font.pixelSize: Style.font11
        }
        Text {
            Layout.preferredWidth: 88
            text: qsTr("Last Seen")
            color: theme.muted
            font.family: Style.fontSans
            font.pixelSize: Style.font11
        }
        Item { Layout.preferredWidth: 96 }
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
            visible: backend.egressLogs.length === 0
            text: qsTr("No egress entries captured yet. Enable 'Record: ON' on the environment and generate traffic.")
            color: theme.muted
            font.family: Style.fontSans
            font.pixelSize: Style.font12
            horizontalAlignment: Text.AlignHCenter
            width: parent.width - Style.space20 * 2
            wrapMode: Text.WordWrap
        }

        ListView {
            id: egressListView
            anchors.fill: parent
            anchors.margins: Style.space8
            clip: true
            spacing: Style.space6
            model: backend.egressLogs
            delegate: Rectangle {
                required property var modelData
                width: egressListView.width
                height: 36
                radius: Math.min(Style.radius3, theme.radius)
                color: Style.surfaceCard
                border.width: Style.hairline
                border.color: Style.borderSubtle

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Style.space10
                    anchors.rightMargin: Style.space8
                    spacing: Style.space10

                    Text {
                        Layout.fillWidth: true
                        text: modelData.dstIp || ""
                        color: theme.foreground
                        font.family: Style.fontMono
                        font.pixelSize: Style.font12
                    }
                    Text {
                        Layout.preferredWidth: 72
                        text: modelData.proto || ""
                        color: theme.muted
                        font.family: Style.fontMono
                        font.pixelSize: Style.font11
                    }
                    Text {
                        Layout.preferredWidth: 72
                        text: modelData.port || "—"
                        color: theme.foreground
                        font.family: Style.fontMono
                        font.pixelSize: Style.font11
                    }
                    Text {
                        Layout.preferredWidth: 64
                        text: String(modelData.count || 0)
                        color: theme.accent
                        font.family: Style.fontMono
                        font.pixelSize: Style.font11
                    }
                    Text {
                        Layout.preferredWidth: 88
                        text: modelData.lastSeen || "—"
                        color: theme.muted
                        font.family: Style.fontMono
                        font.pixelSize: Style.font11
                    }
                    ActionButton {
                        implicitHeight: Style.controlHeightXxs
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
