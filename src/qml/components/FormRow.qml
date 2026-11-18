import QtQuick
import QtQuick.Layouts

// Fixed-width label paired with a flexible control slot and optional mono hint.
ColumnLayout {
    id: root
    property string label: ""
    property string hint: ""
    property int labelWidth: Style.buttonMinWidth * 2
    default property alias control: controlSlot.data

    Layout.fillWidth: true
    spacing: Style.gapXs

    RowLayout {
        Layout.fillWidth: true
        spacing: Style.gap

        Text {
            Layout.preferredWidth: root.labelWidth
            Layout.alignment: Qt.AlignVCenter
            elide: Text.ElideRight
            text: root.label
            color: theme.textSecondary
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }

        RowLayout {
            id: controlSlot
            Layout.fillWidth: true
            spacing: Style.gap
        }
    }

    Text {
        visible: root.hint.length > 0
        Layout.fillWidth: true
        Layout.leftMargin: root.labelWidth + Style.gap
        wrapMode: Text.WordWrap
        text: root.hint
        color: theme.textMuted
        font.family: Style.fontMono
        font.pixelSize: Style.fontCaption
    }
}
