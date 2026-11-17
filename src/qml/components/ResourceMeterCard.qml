import QtQuick
import QtQuick.Layouts

// Task-manager style card with a primary value, optional per-core bars and an
// optional usage meter.
Rectangle {
    id: root
    property string title: ""
    property string value: ""
    property string badge: ""
    property string subtext: ""
    property int alertLevel: 0
    property int subAlert: 0
    property real percent: 0
    property bool hasBar: false
    property bool isCpu: false
    property bool isNetwork: false
    property bool active: false
    property var cores: []

    Layout.preferredHeight: 94
    radius: Math.min(Style.radius4, theme.radius)
    color: Style.surfaceInput
    border.width: Style.hairline
    border.color: root.alertLevel === 2
                  ? theme.red
                  : (root.alertLevel === 1 ? Style.warning : Style.borderSubtle)

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Style.space10
        spacing: Style.space4

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.space6

            Text {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: root.title
                color: theme.muted
                font.family: Style.fontSans
                font.pixelSize: Style.font11
            }

            Text {
                text: root.badge
                color: root.alertLevel === 2
                       ? theme.red
                       : (root.alertLevel === 1
                          ? Style.warning
                          : (root.active
                             ? (root.isNetwork ? theme.green : (root.percent >= 85 ? theme.red : theme.accent))
                             : theme.muted))
                font.family: Style.fontMono
                font.pixelSize: Style.font10
                font.weight: Font.Medium
            }
        }

        // Standard single-value row (disk, memory, network)
        Text {
            visible: !root.isCpu
            Layout.fillWidth: true
            elide: Text.ElideRight
            text: root.value
            color: theme.foreground
            font.family: Style.fontMono
            font.pixelSize: Style.font13
        }

        Text {
            visible: !root.isCpu && root.subtext.length > 0
            Layout.fillWidth: true
            elide: Text.ElideRight
            text: root.subtext
            color: root.subAlert === 2
                   ? theme.red
                   : (root.subAlert === 1 ? Style.warning : theme.muted)
            font.family: Style.fontMono
            font.pixelSize: Style.font10
        }

        Item {
            visible: !root.isCpu
            Layout.fillHeight: true
        }

        MeterBar {
            visible: root.hasBar && !root.isCpu
            Layout.fillWidth: true
            implicitHeight: 4
            value: root.percent
            minFillWidth: Style.space4
            fillColor: root.isNetwork
                       ? (root.active ? theme.green : theme.muted)
                       : (root.alertLevel === 2
                          ? theme.red
                          : (root.alertLevel === 1
                             ? Style.warning
                             : (root.percent >= 85 ? theme.red : theme.accent)))
        }

        // Per-core CPU grid
        GridLayout {
            visible: root.isCpu
            Layout.fillWidth: true
            Layout.fillHeight: true
            columns: root.cores.length > 4 ? 3 : 2
            columnSpacing: Style.space8
            rowSpacing: Style.space4

            Repeater {
                model: root.cores
                delegate: ColumnLayout {
                    required property var modelData
                    required property int index
                    Layout.fillWidth: true
                    spacing: Style.space2

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Style.space4

                        Text {
                            text: "C" + (index + 1)
                            color: theme.muted
                            font.family: Style.fontMono
                            font.pixelSize: Style.font9
                        }

                        Item { Layout.fillWidth: true }

                        Text {
                            text: modelData + "%"
                            color: root.active ? theme.foreground : theme.muted
                            font.family: Style.fontMono
                            font.pixelSize: Style.font9
                        }
                    }

                    MeterBar {
                        Layout.fillWidth: true
                        implicitHeight: 3
                        value: modelData
                        minFillWidth: 3
                        fillColor: modelData >= 85 ? theme.red : theme.accent
                    }
                }
            }
        }
    }
}
