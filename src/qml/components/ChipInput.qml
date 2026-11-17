import QtQuick
import QtQuick.Layouts

// Editable list of numeric chips, used for the create-option choices.
Rectangle {
    id: chipRoot
    property QtObject app: null
    property var values: []
    property string unit: ""
    property int maxChips: 4
    property int maxValue: 1024
    property string placeholder: ""
    readonly property int count: values ? values.length : 0
    readonly property bool atLimit: count >= maxChips
    signal addRequested(string value)
    signal removeRequested(int index)

    implicitHeight: chipFlow.implicitHeight + Style.space8
    Layout.preferredHeight: implicitHeight
    radius: Math.min(Style.radius6, theme.radius)
    color: Style.surfaceInput
    border.width: Style.hairline
    border.color: chipInput.activeFocus ? theme.accent : Style.borderSubtle

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
        anchors.margins: Style.space4
        spacing: Style.space6

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

                height: Style.controlHeightXs
                width: chipRow.implicitWidth + Style.space16
                radius: Math.min(Style.radius4, theme.radius)
                color: chipMouse.containsMouse && removable ? Style.surfaceSelected : Style.surfaceCard
                border.width: Style.hairline
                border.color: chipMouse.containsMouse && removable ? theme.accent : Style.borderSubtle
                opacity: removable ? 1.0 : Style.opacityInactive

                Row {
                    id: chipRow
                    anchors.centerIn: parent
                    spacing: Style.space5

                    Text {
                        text: amount
                        color: theme.foreground
                        font.family: Style.fontMono
                        font.pixelSize: Style.font12
                    }
                    Text {
                        visible: suffix.length > 0
                        text: suffix
                        color: theme.muted
                        font.family: Style.fontMono
                        font.pixelSize: Style.font9
                    }
                    Text {
                        text: "×"
                        leftPadding: Style.space2
                        color: chipMouse.containsMouse && removable ? theme.accent : theme.muted
                        font.family: Style.fontMono
                        font.pixelSize: Style.font10
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
            height: Style.controlHeightXxs
            width: Math.min(Math.max(176, chipFlow.width - 220), Math.max(176, chipInput.implicitWidth + Style.space4))

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
                font.family: Style.fontMono
                font.pixelSize: Style.font12
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
                font.family: Style.fontSans
                font.pixelSize: Style.font11
                elide: Text.ElideRight
            }
        }

        TagBadge {
            visible: chipRoot.atLimit
            text: qsTr("%1 / %2").arg(chipRoot.count).arg(chipRoot.maxChips)
            tone: "quiet"
            badgeHeight: Style.controlHeightXxs
        }
    }
}
