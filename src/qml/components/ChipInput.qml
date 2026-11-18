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

    implicitHeight: Math.max(Style.controlHeight, chipFlow.implicitHeight + Style.gapMd)
    Layout.preferredHeight: implicitHeight
    radius: Math.min(Style.radiusMd, theme.radius)
    color: theme.surfaceInput
    border.width: Style.hairline
    border.color: chipInput.activeFocus ? theme.borderFocus : (chipHover.hovered ? theme.borderStrong : theme.borderSubtle)

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
        anchors.margins: Style.gapSm
        spacing: Style.gapSm

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

                height: Style.controlHeightCompact
                width: chipRow.implicitWidth + Style.padSection
                radius: Math.min(Style.radiusSm, theme.radius)
                color: chipMouse.containsMouse && removable ? theme.surfaceSelected : theme.surfaceCard
                border.width: Style.hairline
                border.color: chipMouse.containsMouse && removable ? theme.accentBorder : theme.borderSubtle
                opacity: removable ? 1.0 : Style.opacityInactive

                Row {
                    id: chipRow
                    anchors.centerIn: parent
                    spacing: Style.gapSm

                    Text {
                        text: amount
                        color: theme.textPrimary
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontBody
                    }
                    Text {
                        visible: suffix.length > 0
                        text: suffix
                        color: theme.textMuted
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontCaption
                    }
                    Text {
                        text: "×"
                        leftPadding: Style.gapXs
                        color: chipMouse.containsMouse && removable ? theme.accentText : theme.textMuted
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontCaption
                    }
                }

                MouseArea {
                    id: chipMouse
                    anchors.fill: parent
                    hoverEnabled: true
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
            height: Style.controlHeightCompact
            width: Math.min(Math.max(Style.buttonMinWidth * 2, chipFlow.width - Style.buttonMinWidth * 2),
                            Math.max(Style.buttonMinWidth * 2, chipInput.implicitWidth + Style.gapSm))

            TextInput {
                id: chipInput
                anchors.fill: parent
                verticalAlignment: TextInput.AlignVCenter
                color: theme.textPrimary
                selectionColor: theme.selection
                selectedTextColor: theme.selectionText
                selectByMouse: true
                clip: true
                inputMethodHints: Qt.ImhDigitsOnly
                font.family: Style.fontMono
                font.pixelSize: Style.fontBody
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
                color: theme.textMuted
                font.family: Style.fontSans
                font.pixelSize: Style.fontSmall
                elide: Text.ElideRight
            }
        }

        TagBadge {
            visible: chipRoot.atLimit
            text: qsTr("%1 / %2").arg(chipRoot.count).arg(chipRoot.maxChips)
            tone: "quiet"
            badgeHeight: Style.controlHeightCompact
        }
    }
}
