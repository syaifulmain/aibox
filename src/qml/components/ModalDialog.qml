import QtQuick
import QtQuick.Layouts

// Shared modal surface: dimmed backdrop, centred card and padded content
// column. Dialogs set `open` and `size` ("sm" | "md" | "lg").
Rectangle {
    id: root
    property bool open: false
    property string size: "sm" // "sm" | "md" | "lg"
    property color scrimColor: theme.scrim
    property color borderColor: theme.borderSubtle
    readonly property real targetWidth: size === "lg" ? Style.modalWidthLg
                                      : (size === "md" ? Style.modalWidthMd : Style.modalWidthSm)
    property real cardWidth: parent ? Math.min(targetWidth, parent.width - Style.modalGutter) : targetWidth
    property real cardHeight: contentHeight + Style.padModal * 2
    property int padding: Style.padModal
    property int contentSpacing: Style.padCard
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
        radius: Math.min(Style.radiusLg, theme.radius)
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
