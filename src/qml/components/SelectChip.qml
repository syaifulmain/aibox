import QtQuick

// Selectable chip used for quota and network choices.
Rectangle {
    id: root
    property string text: ""
    property bool selected: false
    property bool compact: false
    property bool disabled: false
    property bool mono: true
    property int textSize: Style.fontSmall
    property bool mediumWhenSelected: false
    property color selectedTextColor: theme.accentText
    property color textColor: theme.textMuted
    property color selectedFill: theme.surfaceSelected
    property color fillColor: theme.surfaceInput
    property color borderColor: theme.borderSubtle
    property color selectedBorderColor: theme.accentBorder
    property bool interactive: true
    property int chipHeight: compact ? Style.controlHeightCompact : Style.controlHeight
    property int chipRadius: Style.radiusMd
    property int horizontalPadding: Style.padSection
    property bool stretchText: false
    signal clicked()

    readonly property bool isHovered: chipMouse.containsMouse && root.interactive && !root.disabled

    height: chipHeight
    width: label.implicitWidth + horizontalPadding
    radius: Math.min(chipRadius, theme.radius)
    opacity: root.disabled ? Style.opacityDisabled : 1.0
    color: selected ? selectedFill : (isHovered ? theme.surfaceRaised : fillColor)
    border.width: Style.hairline
    border.color: root.activeFocus ? theme.borderFocus
                : selected ? selectedBorderColor
                : (isHovered ? theme.borderStrong : borderColor)

    Text {
        id: label
        anchors.centerIn: parent
        width: root.stretchText ? parent.width - Style.gapSm : implicitWidth
        horizontalAlignment: root.stretchText ? Text.AlignHCenter : Text.AlignLeft
        elide: root.stretchText ? Text.ElideRight : Text.ElideNone
        text: root.text
        color: root.disabled ? theme.textDisabled
             : root.selected ? root.selectedTextColor
             : (root.isHovered ? theme.textPrimary : root.textColor)
        font.family: root.mono ? Style.fontMono : Style.fontSans
        font.pixelSize: root.textSize
        font.weight: root.selected && root.mediumWhenSelected ? Font.Medium : Font.Normal
    }

    MouseArea {
        id: chipMouse
        anchors.fill: parent
        hoverEnabled: root.interactive && !root.disabled
        enabled: root.interactive && !root.disabled
        cursorShape: root.disabled ? Qt.ForbiddenCursor : (root.interactive ? Qt.PointingHandCursor : Qt.ArrowCursor)
        onClicked: root.clicked()
    }
}
