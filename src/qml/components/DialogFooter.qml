import QtQuick
import QtQuick.Layouts

// Right-aligned dialog action bar with an optional left slot for secondary actions.
// Place the single primary action last (rightmost).
Item {
    id: root
    property alias leftActions: leftRow.data
    default property alias rightActions: rightRow.data

    Layout.fillWidth: true
    implicitHeight: Math.max(Style.controlHeight, footerRow.implicitHeight)

    RowLayout {
        id: footerRow
        anchors.fill: parent
        spacing: Style.gap

        RowLayout {
            id: leftRow
            spacing: Style.gap
            Layout.alignment: Qt.AlignVCenter
        }

        Item { Layout.fillWidth: true }

        RowLayout {
            id: rightRow
            spacing: Style.gap
            Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
        }
    }
}
