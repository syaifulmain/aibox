import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import "../components"

// Per-environment audio bridge: Pulse playback with optional PipeWire/mic.
Card {
    id: root

    readonly property var env: backend.selectedEnvironment
    readonly property bool isVm: (root.env.type || "") === "VIRTUAL-MACHINE"
                                 || (root.env.type || "") === "VM"
    readonly property bool template: root.env.isTemplate || false
    readonly property string audioMode: root.env.audioMode || "off"

    visible: !root.isVm
    title: qsTr("Audio & Microphone")
    borderColor: root.audioMode === "off" ? theme.borderSubtle : theme.accentBorder

    actions: [
        ActionButton {
            label: qsTr("Test Audio")
            disabled: backend.busy || root.template
            onClicked: backend.testEnvironmentAudio(root.env.name || "")
        }
    ]

    RowLayout {
        Layout.fillWidth: true
        spacing: Style.gap

        Repeater {
            model: [
                { id: "off", label: qsTr("Off") },
                { id: "output", label: qsTr("Output") },
                { id: "full", label: qsTr("Output + Mic") }
            ]

            delegate: SelectChip {
                required property var modelData

                text: modelData.label
                selected: root.audioMode === modelData.id
                mono: false
                textSize: Style.fontSmall
                mediumWhenSelected: true
                selectedTextColor: theme.textPrimary
                selectedFill: theme.surfaceSelected
                fillColor: theme.surfaceInput
                selectedBorderColor: theme.accentBorder
                interactive: !backend.busy && !root.template
                opacity: backend.busy ? Style.opacityDisabled : 1.0
                onClicked: backend.setEnvironmentAudio(
                    root.env.name || "",
                    modelData.id)
            }
        }
    }

    Text {
        Layout.fillWidth: true
        text: {
            if (root.audioMode === "full")
                return qsTr("Output and microphone — apps can record audio.")
            if (root.audioMode === "output")
                return qsTr("Output only — apps can play sound.")
            return qsTr("Off — no host audio devices.")
        }
        color: root.audioMode === "off" ? theme.textMuted : theme.textSecondary
        font.family: Style.fontSans
        font.pixelSize: Style.fontSmall
        wrapMode: Text.WordWrap
    }

    Text {
        visible: root.audioMode === "full"
        Layout.fillWidth: true
        text: qsTr("Microphone input is best-effort through the PulseAudio compatibility socket; per-app mic denial needs a future PipeWire policy.")
        color: theme.textMuted
        font.family: Style.fontSans
        font.pixelSize: Style.fontSmall
        wrapMode: Text.WordWrap
    }
}
