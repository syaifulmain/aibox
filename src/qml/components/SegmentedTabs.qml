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
    color: Style.surfaceInput
    border.width: Style.hairline
    border.color: Style.borderSubtle

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
                color: active ? Style.surfaceSelected : "transparent"
                border.width: active ? Style.hairline : 0
                border.color: theme.accent

                RowLayout {
                    id: itemRow
                    anchors.centerIn: parent
                    spacing: Style.space8

                    Text {
                        text: modelData.label || ""
                        color: parent.parent.active ? theme.foreground : theme.muted
                        font.family: Style.fontSans
                        font.pixelSize: root.textSize
                        font.weight: parent.parent.active && root.mediumWhenSelected ? Font.Medium : Font.Normal
                    }

                    Text {
                        visible: (modelData.key || "").length > 0
                        text: modelData.key || ""
                        color: theme.muted
                        font.family: Style.fontMono
                        font.pixelSize: Style.font10
                    }
                }

                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.selected(modelData.value)
                }
            }
        }
    }
}
