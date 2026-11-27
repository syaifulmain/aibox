import QtQuick
import QtQuick.Layouts
import "../components"

ModalDialog {
    id: egressOverlay
    property QtObject app: null
    open: app.egressVisible
    zOrder: 86
    clipCard: true
    size: "md"
    cardHeight: Math.min(parent.height - Style.modalGutter, Style.windowMinHeight)
    onDismissed: app.egressVisible = false

    ModalHeader {
        title: qsTr("Network Egress Log · %1").arg(backend.egressLogsEnvName)
        subtitle: qsTr("Shows outgoing destinations captured from kernel bridge ACL logs")
        onClosed: app.egressVisible = false

        ActionButton {
            label: qsTr("Refresh Log")
            onClicked: backend.refreshEgressLog(backend.egressLogsEnvName)
        }
    }

    // Table header
    RowLayout {
        Layout.fillWidth: true
        Layout.leftMargin: Style.gap
        Layout.rightMargin: Style.gap
        spacing: Style.gap

        Text {
            Layout.fillWidth: true
            text: qsTr("Destination IP")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }
        Text {
            Layout.preferredWidth: 72
            text: qsTr("Proto")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }
        Text {
            Layout.preferredWidth: 72
            text: qsTr("Port")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }
        Text {
            Layout.preferredWidth: 64
            text: qsTr("Hits")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }
        Text {
            Layout.preferredWidth: 88
            text: qsTr("Last Seen")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }
        Item { Layout.preferredWidth: 96 }
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
            visible: backend.egressLogs.length === 0
            text: qsTr("No egress entries captured yet. Enable 'Record: ON' on the environment and generate traffic.")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontBody
            horizontalAlignment: Text.AlignHCenter
            width: parent.width - Style.padModal * 2
            wrapMode: Text.WordWrap
        }

        ListView {
            id: egressListView
            anchors.fill: parent
            anchors.margins: Style.gapMd
            clip: true
            reuseItems: true
            spacing: Style.gapSm
            model: backend.egressLogs
            delegate: Rectangle {
                required property var modelData
                width: egressListView.width
                height: Style.rowHeight
                radius: Math.min(Style.radiusSm, theme.radius)
                color: theme.surfaceCard
                border.width: Style.hairline
                border.color: theme.borderSubtle

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Style.gap
                    anchors.rightMargin: Style.gapMd
                    spacing: Style.gap

                    Text {
                        Layout.fillWidth: true
                        text: modelData.dstIp || ""
                        color: theme.textPrimary
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontBody
                    }
                    Text {
                        Layout.preferredWidth: 72
                        text: modelData.proto || ""
                        color: theme.textMuted
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontSmall
                    }
                    Text {
                        Layout.preferredWidth: 72
                        text: modelData.port || "—"
                        color: theme.textPrimary
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontSmall
                    }
                    Text {
                        Layout.preferredWidth: 64
                        text: String(modelData.count || 0)
                        color: theme.accentText
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontSmall
                    }
                    Text {
                        Layout.preferredWidth: 88
                        text: modelData.lastSeen || "—"
                        color: theme.textMuted
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontSmall
                    }
                    ActionButton {
                        compact: true
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
