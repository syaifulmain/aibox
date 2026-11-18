import QtQuick
import QtQuick.Layouts

// Selectable card with a title, subtitle and selection dot.
Rectangle {
    id: root
    property string title: ""
    property string subtitle: ""
    property bool selected: false
    property bool titleMono: false
    property bool subtitleMono: false
    property bool mediumWhenSelected: false
    property color selectedTitleColor: theme.accentText
    property color titleColor: theme.textMuted
    property int cardHeight: 50
    signal clicked()

    height: cardHeight
    radius: Math.min(Style.radius4, theme.radius)
    color: selected ? theme.surfaceSelected : (cardMouse.containsMouse ? theme.surfaceRaised : theme.surfaceInput)
    border.width: Style.hairline
    border.color: selected ? theme.accentBorder : (cardMouse.containsMouse ? theme.borderStrong : theme.borderSubtle)

    ColumnLayout {
        anchors.fill: parent
        anchors.leftMargin: Style.space12
        anchors.rightMargin: Style.space12
        anchors.topMargin: Style.space7
        anchors.bottomMargin: Style.space7
        spacing: Style.space2

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.space6

            Rectangle {
                width: 6
                height: 6
                radius: 3
                color: root.selected ? theme.accent : theme.textMuted
            }

            Text {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: root.title
                color: root.selected ? root.selectedTitleColor : (cardMouse.containsMouse ? theme.textPrimary : root.titleColor)
                font.family: root.titleMono ? Style.fontMono : Style.fontSans
                font.pixelSize: Style.font12
                font.weight: root.selected && root.mediumWhenSelected ? Font.Medium : Font.Normal
            }
        }

        Text {
            Layout.fillWidth: true
            elide: Text.ElideRight
            text: root.subtitle
            color: theme.textMuted
            font.family: root.subtitleMono ? Style.fontMono : Style.fontSans
            font.pixelSize: Style.font10
        }
    }

    MouseArea {
        id: cardMouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
