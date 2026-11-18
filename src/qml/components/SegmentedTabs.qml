import QtQuick
import QtQuick.Layouts

// Track with evenly styled segment buttons, used for the main tabs and the
// history switcher.
Rectangle {
    id: root
    property var tabs: []
    property var currentValue
    property int textSize: Style.fontBody
    property int itemPadding: Style.padSection
    property bool mediumWhenSelected: false
    signal selected(var value)

    height: Style.controlHeight
    width: tabRow.implicitWidth + Style.gapMd
    radius: Math.min(Style.radiusMd, theme.radius)
    color: theme.surfaceInput
    border.width: Style.hairline
    border.color: theme.borderSubtle

    RowLayout {
        id: tabRow
        anchors.centerIn: parent
        spacing: Style.gapSm

        Repeater {
            model: root.tabs
            delegate: Rectangle {
                required property var modelData
                readonly property bool active: root.currentValue === modelData.value

                height: Style.controlHeightCompact
                width: itemRow.implicitWidth + root.itemPadding
                radius: Math.min(Style.radiusSm, theme.radius)
                color: active ? theme.surfaceSelected : (tabMouse.containsMouse ? theme.surfaceRaised : "transparent")
                border.width: active ? Style.hairline : 0
                border.color: theme.accentBorder

                RowLayout {
                    id: itemRow
                    anchors.centerIn: parent
                    spacing: Style.gapMd

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
                        font.pixelSize: Style.fontCaption
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
