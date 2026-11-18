import QtQuick
import QtQuick.Layouts

// Sans label paired with a monospace value in accentText.
Item {
    id: root
    property string label: ""
    property string value: ""
    property bool interactive: false
    property bool bodyValue: false
    property color valueColor: theme.accentText
    signal clicked()

    Layout.fillWidth: true
    implicitHeight: kvRow.implicitHeight

    RowLayout {
        id: kvRow
        anchors.fill: parent
        spacing: Style.gap

        Text {
            text: root.label
            color: theme.textSecondary
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }

        Text {
            Layout.fillWidth: true
            elide: Text.ElideMiddle
            text: root.value
            color: root.valueColor
            font.family: Style.fontMono
            font.pixelSize: root.bodyValue ? Style.fontBody : Style.fontCaption
        }
    }

    MouseArea {
        anchors.fill: parent
        enabled: root.interactive
        cursorShape: root.interactive ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: root.clicked()
    }
}
