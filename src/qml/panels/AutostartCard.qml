import QtQuick
import QtQuick.Layouts
import "../components"

// Per-environment boot autostart configuration (boot.autostart, priority, delay).
Card {
    id: root

    readonly property var env: backend.selectedEnvironment
    readonly property string envName: root.env.name || ""
    readonly property bool autostartOn: root.env.bootAutostart || false
    readonly property int savedPriority: root.env.bootAutostartPriority || 0
    readonly property int savedDelay: root.env.bootAutostartDelay || 0
    readonly property bool template: root.env.isTemplate || false
    readonly property bool locked: backend.busy || root.template

    property string errorText: ""
    readonly property bool dirty: priorityInput.text.trim() !== String(root.savedPriority)
                                  || delayInput.text.trim() !== String(root.savedDelay)

    function syncFields() {
        priorityInput.text = String(root.savedPriority)
        delayInput.text = String(root.savedDelay)
        root.errorText = ""
    }

    function saveOptions() {
        const pRaw = priorityInput.text.trim()
        const dRaw = delayInput.text.trim()
        const pVal = pRaw.length === 0 ? 0 : Number(pRaw)
        const dVal = dRaw.length === 0 ? 0 : Number(dRaw)
        if (!Number.isInteger(pVal) || pVal < 0) {
            root.errorText = qsTr("Priority must be a non-negative whole number.")
            return
        }
        if (!Number.isInteger(dVal) || dVal < 0) {
            root.errorText = qsTr("Delay must be a non-negative whole number of seconds.")
            return
        }
        const err = backend.setEnvironmentAutostart(root.envName, true, pVal, dVal)
        if (err && err.length > 0) {
            root.errorText = err
            return
        }
        root.errorText = ""
    }

    onEnvNameChanged: root.syncFields()
    onSavedPriorityChanged: root.syncFields()
    onSavedDelayChanged: root.syncFields()
    Component.onCompleted: root.syncFields()

    title: qsTr("Start on Boot")
    borderColor: root.autostartOn ? theme.accentBorder : theme.borderSubtle

    actions: [
        ToggleChip {
            label: qsTr("Autostart")
            checked: root.autostartOn
            disabled: root.locked
            onToggled: backend.setEnvironmentAutostart(
                root.envName,
                !root.autostartOn,
                root.savedPriority,
                root.savedDelay)
        }
    ]

    Text {
        Layout.fillWidth: true
        text: root.autostartOn
              ? qsTr("Starts automatically when Incus boots on the host.")
              : qsTr("Off — starts only when you launch it manually.")
        color: root.autostartOn ? theme.textSecondary : theme.textMuted
        font.family: Style.fontSans
        font.pixelSize: Style.fontSmall
        wrapMode: Text.WordWrap
    }

    RowLayout {
        visible: root.autostartOn
        Layout.fillWidth: true
        spacing: Style.gap

        Text {
            text: qsTr("Priority")
            color: theme.textSecondary
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }

        FlatInput {
            id: priorityInput
            Layout.preferredWidth: Style.buttonMinWidth
            mono: true
            placeholder: "0"
            disabled: root.locked
            onAccepted: root.saveOptions()
        }

        Text {
            text: qsTr("Delay (s)")
            color: theme.textSecondary
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }

        FlatInput {
            id: delayInput
            Layout.preferredWidth: Style.buttonMinWidth
            mono: true
            placeholder: "0"
            disabled: root.locked
            onAccepted: root.saveOptions()
        }

        Item { Layout.fillWidth: true }

        ActionButton {
            label: qsTr("Save")
            variant: "accent"
            disabled: root.locked || !root.dirty
            onClicked: root.saveOptions()
        }
    }

    Text {
        visible: root.autostartOn && root.errorText.length > 0
        Layout.fillWidth: true
        text: root.errorText
        color: theme.dangerText
        font.family: Style.fontSans
        font.pixelSize: Style.fontSmall
        wrapMode: Text.WordWrap
    }
}
