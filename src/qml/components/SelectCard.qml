import QtQuick
import QtQuick.Layouts

// Selectable card with a title, subtitle and selection dot.
Rectangle {
    id: root
    property string title: ""
    property string subtitle: ""
    property bool selected: false
    property bool disabled: false
    property bool titleMono: false
    property bool subtitleMono: false
    property bool mediumWhenSelected: false
    property color selectedTitleColor: theme.accentText
    property color titleColor: theme.textMuted
    property int cardHeight: 0
    signal clicked()

    readonly property bool isHovered: cardMouse.containsMouse && !root.disabled

    implicitHeight: cardCol.implicitHeight + Style.padCard * 2
    height: cardHeight > 0 ? cardHeight : implicitHeight
    radius: Math.min(Style.radiusMd, theme.radius)
    opacity: root.disabled ? Style.opacityDisabled : 1.0
    color: selected ? theme.surfaceSelected : (isHovered ? theme.surfaceRaised : theme.surfaceInput)
    border.width: Style.hairline
    border.color: root.activeFocus ? theme.borderFocus
                : selected ? theme.accentBorder
                : (isHovered ? theme.borderStrong : theme.borderSubtle)

    ColumnLayout {
        id: cardCol
        anchors.fill: parent
        anchors.margins: Style.padCard
        spacing: Style.gapXs

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.gapSm

            Rectangle {
                width: Style.gapSm + Style.gapXs
                height: Style.gapSm + Style.gapXs
                radius: width / 2
                color: root.selected ? theme.accent : theme.textMuted
            }

            Text {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: root.title
                color: root.disabled ? theme.textDisabled
                     : root.selected ? root.selectedTitleColor
                     : (root.isHovered ? theme.textPrimary : root.titleColor)
                font.family: root.titleMono ? Style.fontMono : Style.fontSans
                font.pixelSize: Style.fontBody
                font.weight: root.selected && root.mediumWhenSelected ? Font.Medium : Font.Normal
            }
        }

        Text {
            Layout.fillWidth: true
            elide: Text.ElideRight
            text: root.subtitle
            color: root.disabled ? theme.textDisabled : theme.textMuted
            font.family: root.subtitleMono ? Style.fontMono : Style.fontSans
            font.pixelSize: Style.fontCaption
        }
    }

    MouseArea {
        id: cardMouse
        anchors.fill: parent
        hoverEnabled: !root.disabled
        enabled: !root.disabled
        cursorShape: root.disabled ? Qt.ForbiddenCursor : Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
