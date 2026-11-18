import QtQuick
import QtQuick.Layouts

// Full-width checkbox row used by the safety settings.
Rectangle {
    id: root
    property bool checked: false
    property string text: ""
    property int rowHeight: 38
    property bool danger: false
    signal toggled()

    Layout.fillWidth: true
    height: rowHeight
    radius: Math.min(Style.radius4, theme.radius)
    color: rowMouse.containsMouse ? theme.surfaceRaised : theme.surfaceCard
    border.width: Style.hairline
    border.color: checked ? (root.danger ? theme.dangerBorder : theme.accentBorder)
                          : (rowMouse.containsMouse ? theme.borderStrong : theme.borderSubtle)

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Style.space12
        anchors.rightMargin: Style.space12
        spacing: Style.space10

        Rectangle {
            width: 16
            height: 16
            radius: Style.radius3
            color: root.checked ? (root.danger ? theme.danger : theme.accent) : "transparent"
            border.width: Style.hairline
            border.color: root.checked ? (root.danger ? theme.danger : theme.accent) : theme.borderStrong

            Text {
                anchors.centerIn: parent
                visible: root.checked
                text: "✓"
                color: root.danger ? theme.onDanger : theme.textOnAccent
                font.family: Style.fontMono
                font.pixelSize: Style.font11
            }
        }

        Text {
            Layout.fillWidth: true
            text: root.text
            color: theme.textPrimary
            font.family: Style.fontSans
            font.pixelSize: Style.font12
        }
    }

    MouseArea {
        id: rowMouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.toggled()
    }
}
