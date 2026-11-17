import QtQuick
import QtQuick.Layouts

// Full-width action row used by the Tools and environment action dialogs.
Rectangle {
    id: root
    property string label: ""
    property string detail: ""
    property bool danger: false
    property bool disabled: false
    readonly property bool hovered: rowMouse.containsMouse
    signal clicked()

    Layout.fillWidth: true
    implicitHeight: Style.menuRowHeight
    radius: Math.min(Style.radius4, theme.radius)
    color: root.hovered && !root.disabled
           ? (root.danger ? Style.dangerSurface : Style.surfaceSelected)
           : Style.surfaceInput
    border.width: Style.hairline
    border.color: root.hovered && !root.disabled
                  ? (root.danger ? theme.red : theme.accent)
                  : Style.borderSubtle
    opacity: root.disabled ? Style.opacityDisabled : 1.0

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Style.space12
        anchors.rightMargin: Style.space12
        spacing: Style.space10

        ColumnLayout {
            Layout.fillWidth: true
            Layout.minimumWidth: 0
            spacing: Style.space1

            Text {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: root.label
                color: root.danger ? theme.red : theme.foreground
                font.family: Style.fontSans
                font.pixelSize: Style.font12
                font.weight: Font.Medium
            }

            Text {
                Layout.fillWidth: true
                visible: root.detail.length > 0
                elide: Text.ElideRight
                text: root.detail
                color: theme.muted
                font.family: Style.fontSans
                font.pixelSize: Style.font10
            }
        }
    }

    MouseArea {
        id: rowMouse
        anchors.fill: parent
        hoverEnabled: true
        enabled: !root.disabled
        cursorShape: root.disabled ? Qt.ForbiddenCursor : Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
