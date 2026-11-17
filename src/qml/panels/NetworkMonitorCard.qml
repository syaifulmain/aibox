import QtQuick
import QtQuick.Layouts
import "../components"

// Live ping, throughput and totals for the selected environment.
Rectangle {
    id: netMonitorCard
    property QtObject app: null

    Layout.fillWidth: true
    Layout.preferredHeight: netMonitorCol.implicitHeight + Style.space20
    radius: Math.min(Style.radius4, theme.radius)
    color: Style.surfaceInput
    border.width: Style.hairline
    border.color: Style.borderSubtle

    readonly property var stats: backend.networkStats
    readonly property bool live: stats.active === true
    readonly property real pingMs: stats.pingMs !== undefined ? stats.pingMs : -1
    readonly property real lossPct: stats.packetLossPercent !== undefined ? stats.packetLossPercent : -1

    function pingText() {
        if (!live || pingMs < 0)
            return "—"
        return (pingMs < 10 ? pingMs.toFixed(1) : Math.round(pingMs).toString()) + " ms"
    }

    function lossText() {
        if (!live || lossPct < 0)
            return "—"
        return (lossPct % 1 === 0 ? lossPct.toFixed(0) : lossPct.toFixed(1)) + "%"
    }

    ColumnLayout {
        id: netMonitorCol
        anchors.fill: parent
        anchors.margins: Style.space14
        spacing: Style.space10

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.space8

            Text {
                text: qsTr("Network Monitor")
                color: theme.foreground
                font.family: Style.fontSans
                font.pixelSize: Style.font12
                font.weight: Font.Medium
            }

            Rectangle {
                Layout.alignment: Qt.AlignVCenter
                width: 6
                height: 6
                radius: 3
                color: netMonitorCard.live ? theme.green : theme.muted
            }

            Text {
                text: netMonitorCard.live
                      ? qsTr("LIVE")
                      : (backend.selectedEnvironment.networkMode === "offline"
                         ? qsTr("NET OFF")
                         : qsTr("IDLE"))
                color: netMonitorCard.live ? theme.green : theme.muted
                font.family: Style.fontMono
                font.pixelSize: Style.font10
                font.weight: Font.Medium
                Layout.alignment: Qt.AlignVCenter
            }

            Item { Layout.fillWidth: true }

            ActionButton {
                label: qsTr("Probe")
                minWidth: 72
                implicitHeight: Style.controlHeightXs
                disabled: backend.busy || !(backend.selectedEnvironment.running || false)
                Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                onClicked: backend.refreshNetworkStats()
            }
        }

        // Two equal-width columns so labels and values line up
        RowLayout {
            Layout.fillWidth: true
            spacing: Style.space30

            ColumnLayout {
                Layout.fillWidth: true
                Layout.preferredWidth: 0
                spacing: Style.space8

                CopyValueRow {
                    label: qsTr("Ping")
                    value: netMonitorCard.pingText()
                }
                CopyValueRow {
                    label: qsTr("Receiving")
                    value: netMonitorCard.stats.rxRateText || "0 KB/s"
                    tone: "down"
                }
                CopyValueRow {
                    label: qsTr("Downloaded")
                    value: netMonitorCard.stats.rxTotalText || "0 B"
                }
                CopyValueRow {
                    label: qsTr("IP Address")
                    value: netMonitorCard.stats.ip || "—"
                    copyValue: netMonitorCard.stats.ip || ""
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.preferredWidth: 0
                spacing: Style.space8

                CopyValueRow {
                    label: qsTr("Packet Loss")
                    value: netMonitorCard.lossText()
                    tone: netMonitorCard.lossPct > 0 ? "warn" : "normal"
                }
                CopyValueRow {
                    label: qsTr("Sending")
                    value: netMonitorCard.stats.txRateText || "0 KB/s"
                    tone: "up"
                }
                CopyValueRow {
                    label: qsTr("Uploaded")
                    value: netMonitorCard.stats.txTotalText || "0 B"
                }
                CopyValueRow {
                    label: qsTr("Gateway")
                    value: netMonitorCard.stats.gateway || "—"
                    copyValue: netMonitorCard.stats.gateway || ""
                }
            }
        }
    }
}
