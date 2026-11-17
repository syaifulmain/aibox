import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts

Rectangle {
    property QtObject app: null
    id: chipRoot
    property var values: []
    property string unit: ""
    property int maxChips: 4
    property int maxValue: 1024
    property string placeholder: ""
    property string fullHint: ""
    readonly property int count: values ? values.length : 0
    readonly property bool atLimit: count >= maxChips
    signal addRequested(string value)
    signal removeRequested(int index)

    implicitHeight: chipFlow.implicitHeight + 8
    Layout.preferredHeight: implicitHeight
    radius: Math.min(6, theme.radius)
    color: app.surfaceInput
    border.width: 1
    border.color: chipInput.activeFocus ? theme.accent : app.borderSubtle

    HoverHandler { id: chipHover }

    function submit() {
        const raw = chipInput.text.trim()
        if (raw.length === 0)
            return
        const digits = raw.replace(/\s+/g, "")
        if (!/^\d+$/.test(digits) || parseInt(digits, 10) < 1) {
            app.showConfigNotice(qsTr("Enter a whole number greater than zero."), true)
            return
        }
        if (parseInt(digits, 10) > chipRoot.maxValue) {
            app.showConfigNotice(qsTr("The maximum value is %1.").arg(chipRoot.maxValue), true)
            return
        }
        if (chipRoot.atLimit) {
            app.showConfigNotice(qsTr("Limit is %1 choices. Remove one first.").arg(chipRoot.maxChips), true)
            return
        }
        const value = digits + chipRoot.unit
        for (let i = 0; i < chipRoot.count; ++i) {
            if (String(chipRoot.values[i]).toLowerCase() === value.toLowerCase()) {
                app.showConfigNotice(qsTr("%1 is already in the list.").arg(value), true)
                chipInput.text = ""
                return
            }
        }
        chipInput.text = ""
        chipRoot.addRequested(value)
    }

    Flow {
        id: chipFlow
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: 4
        spacing: 6

        Repeater {
            model: chipRoot.values
            delegate: Rectangle {
                required property string modelData
                required property int index
                readonly property string suffix: chipRoot.unit
                readonly property bool removable: chipRoot.count > 1
                readonly property string amount: (chipRoot.unit.length > 0 && modelData.endsWith(chipRoot.unit))
                                                  ? modelData.slice(0, modelData.length - chipRoot.unit.length)
                                                  : modelData

                height: 24
                width: chipRow.implicitWidth + 16
                radius: Math.min(4, theme.radius)
                color: chipMouse.containsMouse && removable ? app.surfaceSelected : app.surfaceCard
                border.width: 1
                border.color: chipMouse.containsMouse && removable ? theme.accent : app.borderSubtle
                opacity: removable ? 1.0 : 0.65

                Row {
                    id: chipRow
                    anchors.centerIn: parent
                    spacing: 5

                    Text {
                        text: "×"
                        color: chipMouse.containsMouse && removable ? theme.accent : theme.muted
                        font.family: app.monoFont
                        font.pixelSize: 12
                    }
                    Text {
                        text: amount
                        color: theme.foreground
                        font.family: app.monoFont
                        font.pixelSize: 12
                    }
                    Text {
                        visible: suffix.length > 0
                        text: suffix
                        color: theme.muted
                        font.family: app.monoFont
                        font.pixelSize: 9
                    }
                }

                MouseArea {
                    id: chipMouse
                    anchors.fill: parent
                    cursorShape: removable ? Qt.PointingHandCursor : Qt.ArrowCursor
                    onClicked: {
                        if (!removable) {
                            app.showConfigNotice(qsTr("Keep at least one choice."), true)
                            return
                        }
                        chipRoot.removeRequested(index)
                    }
                }
            }
        }

        Item {
            visible: !chipRoot.atLimit
            height: 24
            width: Math.min(Math.max(176, chipFlow.width - 220), Math.max(176, chipInput.implicitWidth + 4))

            TextInput {
                id: chipInput
                anchors.fill: parent
                verticalAlignment: TextInput.AlignVCenter
                color: theme.foreground
                selectionColor: theme.accent
                selectedTextColor: theme.accentForeground
                selectByMouse: true
                clip: true
                inputMethodHints: Qt.ImhDigitsOnly
                font.family: app.monoFont
                font.pixelSize: 12
                validator: IntValidator { bottom: 1; top: chipRoot.maxValue }
                onAccepted: chipRoot.submit()
                Keys.onBackPressed: {
                    if (text.length === 0 && chipRoot.count > 1)
                        chipRoot.removeRequested(chipRoot.count - 1)
                }
            }

            Text {
                anchors.fill: parent
                verticalAlignment: Text.AlignVCenter
                visible: chipInput.text.length === 0
                text: chipRoot.placeholder
                color: theme.muted
                font.family: app.sansFont
                font.pixelSize: 11
                elide: Text.ElideRight
            }
        }

        Text {
            visible: chipRoot.atLimit
            height: 24
            verticalAlignment: Text.AlignVCenter
            text: chipHover.hovered ? chipRoot.fullHint : qsTr("%1 / %2").arg(chipRoot.count).arg(chipRoot.maxChips)
            color: theme.muted
            font.family: app.sansFont
            font.pixelSize: 10
        }
    }
}
