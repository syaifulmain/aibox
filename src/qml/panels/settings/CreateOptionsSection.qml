import QtQuick
import QtQuick.Layouts
import "../../components"

// Settings section 3: preset cards and quota choices used by Create.
ColumnLayout {
    id: root
    property QtObject app: null

    readonly property int badgeWidth: Style.buttonMinWidth
    readonly property int actionWidth: Style.buttonMinWidth
    readonly property int labelWidth: Style.buttonMinWidth + Style.padSection * 2

    Layout.fillWidth: true
    spacing: Style.gap

    function reload() {
        app.syncCreateChoiceDrafts()
    }

    function swapChoice(value) {
        const text = String(value).trim().replace(/\s+/g, "")
        if (/^\d+$/.test(text))
            return text + "GiB"
        if (text.toLowerCase() === "off" || text.toLowerCase() === "false")
            return "Off"
        if (text.toLowerCase() === "host" || text.toLowerCase() === "default"
                || text.toLowerCase() === "true")
            return "Host"
        return text
    }

    Card {
        title: qsTr("Capability presets")
        contentSpacing: Style.gap

        Text {
            Layout.fillWidth: true
            text: qsTr("These cards appear above the distribution list. Their behavior stays the same; edit the title and subtitle.")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
            wrapMode: Text.WordWrap
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.gap

            Text {
                Layout.preferredWidth: root.badgeWidth
                Layout.alignment: Qt.AlignVCenter
                text: qsTr("Preset")
                color: theme.textMuted
                font.family: Style.fontSans
                font.pixelSize: Style.fontCaption
            }
            Text {
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignVCenter
                text: qsTr("Title")
                color: theme.textMuted
                font.family: Style.fontSans
                font.pixelSize: Style.fontCaption
            }
            Text {
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignVCenter
                text: qsTr("Subtitle")
                color: theme.textMuted
                font.family: Style.fontSans
                font.pixelSize: Style.fontCaption
            }
            Item {
                Layout.preferredWidth: root.actionWidth
            }
        }

        Repeater {
            model: backend.presets

            delegate: ColumnLayout {
                required property var modelData
                required property int index

                Layout.fillWidth: true
                spacing: Style.gap

                Rectangle {
                    Layout.fillWidth: true
                    visible: index > 0
                    height: Style.hairline
                    color: theme.borderSubtle
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Style.gap

                    Rectangle {
                        Layout.preferredWidth: root.badgeWidth
                        Layout.preferredHeight: Style.controlHeightCompact
                        Layout.alignment: Qt.AlignVCenter
                        radius: Math.min(Style.radiusMd, theme.radius)
                        color: theme.surfaceSelected
                        border.width: Style.hairline
                        border.color: theme.borderSubtle

                        Text {
                            anchors.fill: parent
                            anchors.leftMargin: Style.gapMd
                            anchors.rightMargin: Style.gapMd
                            horizontalAlignment: Text.AlignHCenter
                            verticalAlignment: Text.AlignVCenter
                            text: modelData.id
                            color: theme.accentText
                            font.family: Style.fontMono
                            font.pixelSize: Style.fontCaption
                            elide: Text.ElideRight
                        }
                    }

                    FlatInput {
                        id: presetTitleField
                        Layout.fillWidth: true
                        Layout.alignment: Qt.AlignVCenter
                        text: modelData.title
                        placeholder: qsTr("Title")
                    }

                    FlatInput {
                        id: presetSubField
                        Layout.fillWidth: true
                        Layout.alignment: Qt.AlignVCenter
                        text: modelData.sub
                        placeholder: qsTr("Subtitle")
                    }

                    ActionButton {
                        Layout.preferredWidth: root.actionWidth
                        Layout.alignment: Qt.AlignVCenter
                        label: qsTr("Save")
                        onClicked: {
                            const error = backend.savePreset(modelData.id, presetTitleField.text, presetSubField.text)
                            if (error.length > 0)
                                app.showConfigNotice(error, true)
                            else
                                app.showConfigNotice(qsTr("Preset saved."), false)
                        }
                    }
                }
            }
        }
    }

    Card {
        title: qsTr("Quota and core choices")
        contentSpacing: Style.gap

        Text {
            Layout.fillWidth: true
            text: qsTr("Type a number and press Enter to add a choice (up to 4). Disk and RAM units are appended automatically. Swap accepts Off, Host, or a size such as 4GiB; press × on a chip to remove it.")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
            wrapMode: Text.WordWrap
        }

        FormRow {
            label: qsTr("Disk quota")
            labelWidth: root.labelWidth

            ChipInput {
                app: root.app
                Layout.fillWidth: true
                values: app.diskChoiceDraft
                unit: "GiB"
                maxChips: 4
                maxValue: 65536
                placeholder: qsTr("Type a size, then Enter")
                onAddRequested: function (value) {
                    app.updateCreateChoices("disk", app.diskChoiceDraft.concat([value]))
                }
                onRemoveRequested: function (index) {
                    const next = app.diskChoiceDraft.slice()
                    next.splice(index, 1)
                    app.updateCreateChoices("disk", next)
                }
            }
        }

        FormRow {
            label: qsTr("RAM limit")
            labelWidth: root.labelWidth

            ChipInput {
                app: root.app
                Layout.fillWidth: true
                values: app.ramChoiceDraft
                unit: "GiB"
                maxChips: 4
                maxValue: 65536
                placeholder: qsTr("Type a size, then Enter")
                onAddRequested: function (value) {
                    app.updateCreateChoices("ram", app.ramChoiceDraft.concat([value]))
                }
                onRemoveRequested: function (index) {
                    const next = app.ramChoiceDraft.slice()
                    next.splice(index, 1)
                    app.updateCreateChoices("ram", next)
                }
            }
        }

        FormRow {
            label: qsTr("Swap limit")
            labelWidth: root.labelWidth

            ChipInput {
                app: root.app
                Layout.fillWidth: true
                values: app.swapChoiceDraft
                maxChips: 4
                allowText: true
                lockedValues: ["Off", "Host"]
                placeholder: qsTr("Off, Host, or a size")
                onAddRequested: function (value) {
                    app.updateCreateChoices("swap",
                        app.swapChoiceDraft.concat([root.swapChoice(value)]))
                }
                onRemoveRequested: function (index) {
                    const next = app.swapChoiceDraft.slice()
                    next.splice(index, 1)
                    app.updateCreateChoices("swap", next)
                }
            }
        }

        FormRow {
            label: qsTr("CPU cores")
            labelWidth: root.labelWidth

            ChipInput {
                app: root.app
                Layout.fillWidth: true
                values: app.cpuChoiceDraft
                maxChips: 4
                maxValue: 1024
                placeholder: qsTr("Type cores, then Enter")
                onAddRequested: function (value) {
                    app.updateCreateChoices("cpu", app.cpuChoiceDraft.concat([value]))
                }
                onRemoveRequested: function (index) {
                    const next = app.cpuChoiceDraft.slice()
                    next.splice(index, 1)
                    app.updateCreateChoices("cpu", next)
                }
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true
        Layout.topMargin: Style.gapXs
        spacing: Style.gap

        ActionButton {
            label: qsTr("Restore Create Defaults")
            onClicked: {
                const error = backend.resetCreateOptions()
                if (error.length > 0) {
                    app.showConfigNotice(error, true)
                } else {
                    app.syncCreateChoiceDrafts()
                    app.showConfigNotice(qsTr("Create options restored."), false)
                }
            }
        }

        Item { Layout.fillWidth: true }

        ActionButton {
            label: qsTr("Save Options")
            variant: "primary"
            onClicked: app.saveQuotaOptions()
        }
    }
}
