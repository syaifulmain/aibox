import QtQuick
import QtQuick.Layouts

// Full-width checkbox row used by the safety settings.
Rectangle {
    id: root
    property bool checked: false
    property bool disabled: false
    property int rowHeight: Style.rowHeight
    property string text: ""
    property bool danger: false
    signal toggled()

    readonly property bool isHovered: rowMouse.containsMouse && !root.disabled

    Layout.fillWidth: true
    implicitHeight: Math.max(root.rowHeight, checkText.implicitHeight + Style.padCard)
    Layout.preferredHeight: implicitHeight
    radius: Math.min(Style.radiusMd, theme.radius)
    opacity: root.disabled ? Style.opacityDisabled : 1.0
    color: isHovered ? theme.surfaceRaised : theme.surfaceCard
    border.width: Style.hairline
    border.color: root.activeFocus ? theme.borderFocus
                : checked ? (root.danger ? theme.dangerBorder : theme.accentBorder)
                : (isHovered ? theme.borderStrong : theme.borderSubtle)

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Style.padCard
        anchors.rightMargin: Style.padCard
        spacing: Style.gapMd

        Rectangle {
            width: Style.padSection
            height: Style.padSection
            radius: Math.min(Style.radiusSm, theme.radius)
            color: root.checked ? (root.danger ? theme.danger : theme.accent) : "transparent"
            border.width: Style.hairline
            border.color: root.checked ? (root.danger ? theme.danger : theme.accent) : theme.borderStrong

            Text {
                anchors.centerIn: parent
                visible: root.checked
                text: "✓"
                color: root.danger ? theme.onDanger : theme.textOnAccent
                font.family: Style.fontMono
                font.pixelSize: Style.fontSmall
            }
        }

        Text {
            id: checkText
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: root.text
            color: root.disabled ? theme.textDisabled : theme.textPrimary
            font.family: Style.fontSans
            font.pixelSize: Style.fontBody
        }
    }

    MouseArea {
        id: rowMouse
        anchors.fill: parent
        hoverEnabled: !root.disabled
        enabled: !root.disabled
        cursorShape: root.disabled ? Qt.ForbiddenCursor : Qt.PointingHandCursor
        onClicked: root.toggled()
    }
}
