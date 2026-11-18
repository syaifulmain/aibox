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
    spacing: Style.padCard

    RowLayout {
        Layout.fillWidth: true
        spacing: Style.gap

        ColumnLayout {
            Layout.fillWidth: true
            Layout.minimumWidth: 0
            spacing: Style.gapXs

            Text {
                Layout.fillWidth: true
                text: mh.title
                color: theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.fontHeading
                font.weight: Font.Medium
                elide: Text.ElideRight
            }

            Text {
                visible: mh.subtitle.length > 0
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                text: mh.subtitle
                color: theme.textMuted
                font.family: mh.subtitleMono ? Style.fontMono : Style.fontSans
                font.pixelSize: mh.subtitleMono ? Style.fontCaption : Style.fontSmall
                wrapMode: mh.subtitleMono ? Text.NoWrap : Text.WordWrap
                elide: mh.subtitleMono ? Text.ElideMiddle : Text.ElideNone
            }
        }

        RowLayout {
            id: extraRow
            spacing: Style.gap
            Layout.alignment: Qt.AlignTop | Qt.AlignRight
        }

        ActionButton {
            objectName: "modalCloseButton"
            visible: mh.showClose
            label: qsTr("Close")
            minWidth: Style.buttonMinWidth
            disabled: mh.closeDisabled
            Layout.alignment: Qt.AlignTop | Qt.AlignRight
            onClicked: mh.closed()
        }
    }

    Rectangle {
        Layout.fillWidth: true
        height: Style.hairline
        color: theme.borderSubtle
    }
}
