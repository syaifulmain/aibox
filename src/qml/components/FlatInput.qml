import QtQuick
import QtQuick.Layouts

// Single-line flat text field with a static placeholder.
Rectangle {
    id: root
    property alias text: textInput.text
    property alias echoMode: textInput.echoMode
    property string placeholder: ""
    property bool readOnly: false
    property bool disabled: false
    property bool mono: false
    signal accepted()

    implicitHeight: Style.controlHeight
    Layout.preferredHeight: implicitHeight
    radius: Math.min(Style.radiusMd, theme.radius)
    opacity: root.disabled ? Style.opacityDisabled : 1.0
    color: theme.surfaceInput
    border.width: Style.hairline
    border.color: textInput.activeFocus ? theme.borderFocus
                : (inputHover.hovered && !root.disabled ? theme.borderStrong : theme.borderSubtle)

    HoverHandler {
        id: inputHover
        enabled: !root.disabled
    }

    Text {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: Style.gapMd
        anchors.rightMargin: Style.gapMd
        anchors.verticalCenter: parent.verticalCenter
        visible: textInput.text.length === 0
        text: root.placeholder
        color: theme.textMuted
        font.family: root.mono ? Style.fontMono : Style.fontSans
        font.pixelSize: Style.fontBody
        elide: Text.ElideRight
    }

    TextInput {
        id: textInput
        anchors.fill: parent
        anchors.leftMargin: Style.gapMd
        anchors.rightMargin: Style.gapMd
        verticalAlignment: TextInput.AlignVCenter
        enabled: !root.disabled
        color: root.disabled ? theme.textDisabled : theme.textPrimary
        selectionColor: theme.selection
        selectedTextColor: theme.selectionText
        selectByMouse: true
        clip: true
        readOnly: root.readOnly
        font.family: root.mono ? Style.fontMono : Style.fontSans
        font.pixelSize: Style.fontBody
        onAccepted: root.accepted()
    }
}
