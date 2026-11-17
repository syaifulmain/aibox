import QtQuick

// The ">_" tile that identifies AIBox in headers and empty states.
Rectangle {
    id: root
    property int markSize: 28
    property int markRadius: Style.radius4
    property int textSize: Style.font12
    property bool quiet: false

    width: markSize
    height: markSize
    radius: Math.min(markRadius, theme.radius)
    color: root.quiet ? Style.surfaceInput : Style.surfaceSelected
    border.width: Style.hairline
    border.color: root.quiet ? Style.borderSubtle : theme.accent

    Text {
        anchors.centerIn: parent
        text: ">_"
        color: theme.accent
        font.family: Style.fontMono
        font.pixelSize: root.textSize
    }
}
