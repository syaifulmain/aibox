import QtQuick
import QtQuick.Layouts

// On/off toggle chip with a label and state indicator ("ON" / "OFF").
Rectangle {
    id: root
    property string label: ""
    property bool checked: false
    property bool disabled: false
    property bool compact: false
    property string onText: qsTr("ON")
    property string offText: qsTr("OFF")
    signal toggled()

    readonly property bool isHovered: toggleMouse.containsMouse && !root.disabled

    implicitHeight: root.compact ? Style.controlHeightCompact : Style.controlHeight
    implicitWidth: chipRow.implicitWidth + Style.padCard * 2
    Layout.preferredHeight: implicitHeight
    Layout.preferredWidth: implicitWidth
    radius: Math.min(Style.radiusMd, theme.radius)
    opacity: root.disabled ? Style.opacityDisabled : 1.0
    color: root.checked ? theme.surfaceSelected
         : (root.isHovered ? theme.surfaceRaised : theme.surfaceInput)
    border.width: Style.hairline
    border.color: root.activeFocus ? theme.borderFocus
                : root.checked ? theme.accentBorder
                : (root.isHovered ? theme.borderStrong : theme.borderSubtle)

    RowLayout {
        id: chipRow
        anchors.centerIn: parent
        spacing: Style.gapMd

        Text {
            text: root.label
            color: root.disabled ? theme.textDisabled
                 : (root.checked ? theme.accentText : theme.textPrimary)
            font.family: Style.fontSans
            font.pixelSize: Style.fontBody
        }

        Text {
            text: root.checked ? root.onText : root.offText
            color: root.disabled ? theme.textDisabled
                 : (root.checked ? theme.accentText : theme.textMuted)
            font.family: Style.fontMono
            font.pixelSize: Style.fontCaption
            font.weight: Font.Medium
        }
    }

    MouseArea {
        id: toggleMouse
        anchors.fill: parent
        hoverEnabled: !root.disabled
        enabled: !root.disabled
        cursorShape: root.disabled ? Qt.ForbiddenCursor : Qt.PointingHandCursor
        onClicked: root.toggled()
    }
}
