import QtQuick
import QtQuick.Layouts

// Compact unit selector placed beside a numeric size input.
RowLayout {
    id: root
    property var units: []
    property string current: ""
    signal selected(string unit)

    Layout.alignment: Qt.AlignVCenter
    spacing: Style.gapXs

    Repeater {
        model: root.units
        delegate: SelectChip {
            required property string modelData
            text: modelData
            compact: true
            selected: root.current === modelData
            onClicked: root.selected(modelData)
        }
    }
}
