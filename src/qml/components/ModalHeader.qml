import QtQuick
import QtQuick.Layouts

// Title, optional subtitle, extra actions and the shared Close button.
ColumnLayout {
    id: mh
    property string title: ""
    property string subtitle: ""
    property bool subtitleMono: false
    property bool showClose: true
    property bool closeDisabled: false
    default property alias extraActions: extraRow.data
    signal closed()

    Layout.fillWidth: true
    spacing: Style.space12

    RowLayout {
        Layout.fillWidth: true
        spacing: Style.space10

        ColumnLayout {
            Layout.fillWidth: true
            Layout.minimumWidth: 0
            spacing: Style.space2

            Text {
                Layout.fillWidth: true
                text: mh.title
                color: theme.foreground
                font.family: Style.fontSans
                font.pixelSize: Style.font16
                font.weight: Font.Medium
                elide: Text.ElideRight
            }

            Text {
                visible: mh.subtitle.length > 0
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                text: mh.subtitle
                color: theme.muted
                font.family: mh.subtitleMono ? Style.fontMono : Style.fontSans
                font.pixelSize: mh.subtitleMono ? Style.font10 : Style.font11
                wrapMode: mh.subtitleMono ? Text.NoWrap : Text.WordWrap
                elide: mh.subtitleMono ? Text.ElideMiddle : Text.ElideNone
            }
        }

        RowLayout {
            id: extraRow
            spacing: Style.space10
            Layout.alignment: Qt.AlignTop | Qt.AlignRight
        }

        ActionButton {
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
        height: Style.hairline
        color: Style.borderSubtle
    }
}
