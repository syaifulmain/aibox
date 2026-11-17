import QtQuick

// Compact selectable chip used for quota and network choices.
Rectangle {
    id: root
    property string text: ""
    property bool selected: false
    property bool mono: true
    property int textSize: Style.font11
    property bool mediumWhenSelected: false
    property color selectedTextColor: theme.accent
    property color textColor: theme.muted
    property color selectedFill: Style.surfaceSelected
    property color fillColor: Style.surfaceInput
    property color borderColor: Style.borderSubtle
    property color selectedBorderColor: theme.accent
    property bool interactive: true
    property int chipHeight: 32
    property int chipRadius: Style.radius4
    property int horizontalPadding: Style.space16
    property bool stretchText: false
    signal clicked()

    height: chipHeight
    width: label.implicitWidth + horizontalPadding
    radius: Math.min(chipRadius, theme.radius)
    color: selected ? selectedFill : fillColor
    border.width: Style.hairline
    border.color: selected ? selectedBorderColor : borderColor

    Text {
        id: label
        anchors.centerIn: parent
        width: root.stretchText ? parent.width - Style.space4 : implicitWidth
        horizontalAlignment: root.stretchText ? Text.AlignHCenter : Text.AlignLeft
        elide: root.stretchText ? Text.ElideRight : Text.ElideNone
        text: root.text
        color: root.selected ? root.selectedTextColor : root.textColor
        font.family: root.mono ? Style.fontMono : Style.fontSans
        font.pixelSize: root.textSize
        font.weight: root.selected && root.mediumWhenSelected ? Font.Medium : Font.Normal
    }

    MouseArea {
        anchors.fill: parent
        enabled: root.interactive
        cursorShape: root.interactive ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: root.clicked()
    }
}
