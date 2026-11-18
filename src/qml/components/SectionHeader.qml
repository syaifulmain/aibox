import QtQuick
import QtQuick.Layouts

// Section title with optional subtitle and right-aligned actions, aligned to
// the standard 34 px control height.
Item {
    id: root
    property string title: ""
    property string subtitle: ""
    default property alias actions: actionsRow.data

    Layout.fillWidth: true
    implicitHeight: Math.max(Style.controlHeight, headerRow.implicitHeight)

    RowLayout {
        id: headerRow
        anchors.fill: parent
        spacing: Style.gap

        ColumnLayout {
            Layout.fillWidth: true
            Layout.minimumWidth: 0
            Layout.alignment: Qt.AlignVCenter
            spacing: Style.gapXs

            Text {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: root.title
                color: theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.fontTitle
                font.weight: Font.Medium
            }

            Text {
                visible: root.subtitle.length > 0
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: root.subtitle
                color: theme.textMuted
                font.family: Style.fontSans
                font.pixelSize: Style.fontSmall
            }
        }

        RowLayout {
            id: actionsRow
            spacing: Style.gap
            Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
        }
    }
}
