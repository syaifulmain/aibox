import QtQuick
import QtQuick.Layouts

// Shared modal surface: dimmed backdrop, centred card and padded content
// column. Each dialog declares its own `app` property and sets `open`,
// `cardWidth` and `cardHeight`.
Rectangle {
    id: root
    property bool open: false
    property color scrimColor: theme.scrim
    property color borderColor: theme.borderSubtle
    property real cardWidth: 520
    property real cardHeight: contentHeight + Style.modalPadding * 2
    property int padding: Style.modalPadding
    property int contentSpacing: Style.space12
    property bool clipCard: false
    property int zOrder: 0
    readonly property alias content: body
    readonly property real contentHeight: body.implicitHeight
    default property alias contentChildren: body.data
    signal dismissed()

    anchors.fill: parent
    visible: open
    color: scrimColor
    z: zOrder

    MouseArea {
        anchors.fill: parent
        onClicked: root.dismissed()
    }

    Rectangle {
        id: card
        anchors.centerIn: parent
        width: root.cardWidth
        height: root.cardHeight
        radius: Math.min(Style.radius8, theme.radius)
        color: theme.surfaceOverlay
        border.width: Style.hairline
        border.color: root.borderColor
        clip: root.clipCard

        MouseArea { anchors.fill: parent }

        ColumnLayout {
            id: body
            anchors.fill: parent
            anchors.margins: root.padding
            spacing: root.contentSpacing
        }
    }
}
