import QtQuick
import QtQuick.Layouts

// Single-line flat text field with a static placeholder.
Rectangle {
    id: root
    property alias text: textInput.text
    property alias echoMode: textInput.echoMode
    property string placeholder: ""
    property bool readOnly: false
    signal accepted()

    implicitHeight: Style.controlHeight
    Layout.preferredHeight: implicitHeight
    radius: Math.min(Style.radius4, theme.radius)
    color: theme.surfaceInput
    border.width: Style.hairline
    border.color: textInput.activeFocus ? theme.borderFocus : theme.borderSubtle

    Text {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: Style.space12
        anchors.rightMargin: Style.space12
        anchors.verticalCenter: parent.verticalCenter
        visible: textInput.text.length === 0
        text: root.placeholder
        color: theme.textMuted
        font.family: Style.fontSans
        font.pixelSize: Style.font12
        elide: Text.ElideRight
    }

    TextInput {
        id: textInput
        anchors.fill: parent
        anchors.leftMargin: Style.space12
        anchors.rightMargin: Style.space12
        verticalAlignment: TextInput.AlignVCenter
        color: theme.textPrimary
        selectionColor: theme.selection
        selectedTextColor: theme.selectionText
        selectByMouse: true
        clip: true
        readOnly: root.readOnly
        font.family: Style.fontSans
        font.pixelSize: Style.font13
        onAccepted: root.accepted()
    }
}
