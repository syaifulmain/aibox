import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts

ColumnLayout {
    property QtObject app: null
    id: mh
    property string title: ""
    property string subtitle: ""
    property bool subtitleMono: false
    property bool showClose: true
    property bool closeDisabled: false
    default property alias extraActions: extraRow.data
    signal closed()

    Layout.fillWidth: true
    spacing: 12

    RowLayout {
        Layout.fillWidth: true
        spacing: 10

        ColumnLayout {
            Layout.fillWidth: true
            Layout.minimumWidth: 0
            spacing: 2

            Text {
                Layout.fillWidth: true
                text: mh.title
                color: theme.foreground
                font.family: app.sansFont
                font.pixelSize: 16
                font.weight: Font.Medium
                elide: Text.ElideRight
            }

            Text {
                visible: mh.subtitle.length > 0
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                text: mh.subtitle
                color: theme.muted
                font.family: mh.subtitleMono ? app.monoFont : app.sansFont
                font.pixelSize: mh.subtitleMono ? 10 : 11
                wrapMode: mh.subtitleMono ? Text.NoWrap : Text.WordWrap
                elide: mh.subtitleMono ? Text.ElideMiddle : Text.ElideNone
            }
        }

        RowLayout {
            id: extraRow
            spacing: 10
            Layout.alignment: Qt.AlignTop | Qt.AlignRight
        }

        ActionButton {
            app: mh.app
            visible: mh.showClose
            label: qsTr("Close")
            shortcutHint: "Esc"
            minWidth: 88
            disabled: mh.closeDisabled
            Layout.alignment: Qt.AlignTop | Qt.AlignRight
            onClicked: mh.closed()
        }
    }

    Rectangle {
        Layout.fillWidth: true
        height: 1
        color: app.borderSubtle
    }
}
