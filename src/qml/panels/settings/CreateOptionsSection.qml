import QtQuick
import QtQuick.Layouts
import "../../components"

// Settings section 3: preset cards and quota choices used by Create.
ColumnLayout {
    id: root
    property QtObject app: null

    Layout.fillWidth: true
    spacing: Style.space12

    function reload() {
        app.syncCreateChoiceDrafts()
    }

    Text {
        text: qsTr("Capability presets")
        color: theme.foreground
        font.family: Style.fontSans
        font.pixelSize: Style.font12
        font.weight: Font.Medium
    }

    Text {
        Layout.fillWidth: true
        text: qsTr("These cards appear above the distribution list. Their behavior stays the same; edit the title and subtitle.")
        color: theme.muted
        font.family: Style.fontSans
        font.pixelSize: Style.font10
        wrapMode: Text.WordWrap
    }

    Repeater {
        model: backend.presets

        delegate: Rectangle {
            required property var modelData
            Layout.fillWidth: true
            height: 84
            radius: Math.min(Style.radius6, theme.radius)
            color: Style.surfaceInput
            border.width: Style.hairline
            border.color: Style.borderSubtle

            RowLayout {
                anchors.fill: parent
                anchors.margins: Style.space12
                spacing: Style.space10

                Text {
                    Layout.preferredWidth: 58
                    text: modelData.id
                    color: theme.accent
                    font.family: Style.fontMono
                    font.pixelSize: Style.font9
                    elide: Text.ElideRight
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Style.space3

                    Text {
                        text: qsTr("Title")
                        color: theme.muted
                        font.family: Style.fontSans
                        font.pixelSize: Style.font9
                    }
                    FlatInput {
                        id: presetTitleField
                        Layout.fillWidth: true
                        text: modelData.title
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Style.space3

                    Text {
                        text: qsTr("Subtitle")
                        color: theme.muted
                        font.family: Style.fontSans
                        font.pixelSize: Style.font9
                    }
                    FlatInput {
                        id: presetSubField
                        Layout.fillWidth: true
                        text: modelData.sub
                    }
                }

                ActionButton {
                    Layout.alignment: Qt.AlignBottom
                    label: qsTr("Save")
                    minWidth: 66
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

    // Quota and core choices
    Rectangle {
        Layout.fillWidth: true
        Layout.preferredHeight: quotaCardCol.implicitHeight + Style.cardPadding * 2
        radius: Math.min(Style.radius6, theme.radius)
        color: Style.surfaceInput
        border.width: Style.hairline
        border.color: Style.borderSubtle

        ColumnLayout {
            id: quotaCardCol
            anchors.fill: parent
            anchors.margins: Style.cardPadding
            spacing: Style.space6

            Text {
                text: qsTr("Quota and core choices")
                color: theme.foreground
                font.family: Style.fontSans
                font.pixelSize: Style.font12
                font.weight: Font.Medium
            }
            Text {
                Layout.fillWidth: true
                text: qsTr("Type a number and press Enter to add a choice (up to 4). Disk and RAM units are appended automatically; press × on a chip to remove it.")
                color: theme.muted
                font.family: Style.fontSans
                font.pixelSize: Style.font10
                wrapMode: Text.WordWrap
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space10

                Text {
                    Layout.preferredWidth: 120
                    Layout.alignment: Qt.AlignVCenter
                    text: qsTr("Disk quota")
                    color: theme.foreground
                    font.family: Style.fontSans
                    font.pixelSize: Style.font11
                }

                ChipInput {
                    app: root.app
                    Layout.fillWidth: true
                    values: app.diskChoiceDraft
                    unit: "GiB"
                    maxChips: 4
                    maxValue: 65536
                    placeholder: qsTr("Type a size, then Enter")
                    fullHint: qsTr("Limit 4 — remove one first")
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

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space10

                Text {
                    Layout.preferredWidth: 120
                    Layout.alignment: Qt.AlignVCenter
                    text: qsTr("RAM limit")
                    color: theme.foreground
                    font.family: Style.fontSans
                    font.pixelSize: Style.font11
                }

                ChipInput {
                    app: root.app
                    Layout.fillWidth: true
                    values: app.ramChoiceDraft
                    unit: "GiB"
                    maxChips: 4
                    maxValue: 65536
                    placeholder: qsTr("Type a size, then Enter")
                    fullHint: qsTr("Limit 4 — remove one first")
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

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space10

                Text {
                    Layout.preferredWidth: 120
                    Layout.alignment: Qt.AlignVCenter
                    text: qsTr("CPU cores")
                    color: theme.foreground
                    font.family: Style.fontSans
                    font.pixelSize: Style.font11
                }

                ChipInput {
                    app: root.app
                    Layout.fillWidth: true
                    values: app.cpuChoiceDraft
                    maxChips: 4
                    maxValue: 1024
                    placeholder: qsTr("Type cores, then Enter")
                    fullHint: qsTr("Limit 4 — remove one first")
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

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space10
                Item { Layout.fillWidth: true }

                ActionButton {
                    label: qsTr("Save Options")
                    variant: "primary"
                    minWidth: 130
                    onClicked: app.saveQuotaOptions()
                }
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: Style.space10
        Item { Layout.fillWidth: true }

        ActionButton {
            label: qsTr("Restore Create Defaults")
            minWidth: 185
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
    }
}
