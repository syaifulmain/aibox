import QtQuick
import QtQuick.Layouts

// Compact resource card with a primary value, optional usage meter and alert
// colouring. Disk, memory and CPU all use the same layout.
Rectangle {
    id: root
    property string title: ""
    property string value: ""
    property string badge: ""
    property int alertLevel: 0
    property real percent: 0
    property bool hasBar: false
    property bool active: false

    Layout.preferredHeight: 88
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
                             ? (root.percent >= 85 ? theme.red : theme.accent)
                             : theme.muted))
                font.family: Style.fontMono
                font.pixelSize: Style.font10
                font.weight: Font.Medium
            }
        }

        Text {
            Layout.fillWidth: true
            elide: Text.ElideRight
            text: root.value
            color: theme.foreground
            font.family: Style.fontMono
            font.pixelSize: Style.font13
        }

        Item { Layout.fillHeight: true }

        MeterBar {
            visible: root.hasBar
            Layout.fillWidth: true
            implicitHeight: 4
            value: root.percent
            minFillWidth: Style.space4
            fillColor: root.alertLevel === 2
                       ? theme.red
                       : (root.alertLevel === 1
                          ? Style.warning
                          : (root.percent >= 85 ? theme.red : theme.accent))
        }
    }
}
