import QtQuick
import QtQuick.Layouts

// Track with evenly styled segment buttons, used for the main tabs and the
// history switcher.
Rectangle {
    id: root
    property var tabs: []
    property var currentValue
    property int textSize: Style.font12
    property int itemPadding: Style.space18
    property bool mediumWhenSelected: false
    signal selected(var value)

    height: Style.controlHeight
    width: tabRow.implicitWidth + Style.space8
    radius: Math.min(Style.radius4, theme.radius)
    color: theme.surfaceInput
    border.width: Style.hairline
    border.color: theme.borderSubtle

    RowLayout {
        id: tabRow
        anchors.centerIn: parent
        spacing: Style.space4

        Repeater {
            model: root.tabs
            delegate: Rectangle {
                required property var modelData
                readonly property bool active: root.currentValue === modelData.value

                height: Style.controlHeightSm
                width: itemRow.implicitWidth + root.itemPadding
                radius: Math.min(Style.radius3, theme.radius)
                color: active ? theme.surfaceSelected : (tabMouse.containsMouse ? theme.surfaceRaised : "transparent")
                border.width: active ? Style.hairline : 0
                border.color: theme.accentBorder

                RowLayout {
                    id: itemRow
                    anchors.centerIn: parent
                    spacing: Style.space8

                    Text {
                        text: modelData.label || ""
                        color: parent.parent.active ? theme.accentText : (tabMouse.containsMouse ? theme.textPrimary : theme.textMuted)
                        font.family: Style.fontSans
                        font.pixelSize: root.textSize
                        font.weight: parent.parent.active && root.mediumWhenSelected ? Font.Medium : Font.Normal
                    }

                    Text {
                        visible: (modelData.key || "").length > 0
                        text: modelData.key || ""
                        color: theme.textMuted
                        font.family: Style.fontMono
                        font.pixelSize: Style.font10
                    }
                }

                MouseArea {
                    id: tabMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.selected(modelData.value)
                }
            }
        }
    }
}
