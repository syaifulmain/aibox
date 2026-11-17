import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts

Rectangle {
    property QtObject app: null
    id: root
    property alias text: textInput.text
    property alias echoMode: textInput.echoMode
    property string placeholder: ""
    property bool readOnly: false
    signal accepted()

    implicitHeight: 34
    Layout.preferredHeight: implicitHeight
    radius: Math.min(4, theme.radius)
    color: app.surfaceInput
    border.width: 1
    border.color: textInput.activeFocus ? theme.accent : app.borderSubtle

    Text {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: 12
        anchors.rightMargin: 12
        anchors.verticalCenter: parent.verticalCenter
        visible: textInput.text.length === 0
        text: root.placeholder
        color: theme.muted
        font.family: app.sansFont
        font.pixelSize: 12
        elide: Text.ElideRight
    }

    TextInput {
        id: textInput
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 12
        verticalAlignment: TextInput.AlignVCenter
        color: theme.foreground
        selectionColor: theme.accent
        selectedTextColor: theme.accentForeground
        selectByMouse: true
        clip: true
        readOnly: root.readOnly
        font.family: app.sansFont
        font.pixelSize: 13
        onAccepted: root.accepted()
    }
}
