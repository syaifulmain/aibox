import QtQuick
import QtQuick.Layouts
import "../../components"

// Settings section 5: theme polarity override, high contrast, and motion.
ColumnLayout {
    id: root
    property QtObject app: null
    property string modeDraft: backend.themeModeOverride
    property bool highContrastDraft: backend.highContrast
    property bool reduceMotionDraft: backend.reduceMotion

    Layout.fillWidth: true
    spacing: Style.gap

    function reload() {
        modeDraft = backend.themeModeOverride
        highContrastDraft = backend.highContrast
        reduceMotionDraft = backend.reduceMotion
    }

    Card {
        title: qsTr("Theme Mode")
        contentSpacing: Style.gap

        Text {
            Layout.fillWidth: true
            text: qsTr("Follow the active Omarchy theme automatically, or force a light or dark palette inside AIBox.")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
            wrapMode: Text.WordWrap
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.gapMd

            Repeater {
                model: [
                    { id: "auto", label: qsTr("Follow Omarchy") },
                    { id: "light", label: qsTr("Always light") },
                    { id: "dark", label: qsTr("Always dark") }
                ]

                delegate: SelectChip {
                    required property var modelData
                    text: modelData.label
                    selected: root.modeDraft === modelData.id
                    onClicked: root.modeDraft = modelData.id
                }
            }

            Item { Layout.fillWidth: true }
        }
    }

    Card {
        title: qsTr("Accessibility & Motion")
        contentSpacing: Style.gap

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.gap

            ToggleChip {
                label: qsTr("High contrast")
                checked: root.highContrastDraft
                onToggled: root.highContrastDraft = !root.highContrastDraft
            }

            Text {
                Layout.fillWidth: true
                text: qsTr("7:1 text, 3:1 borders")
                color: theme.textMuted
                font.family: Style.fontSans
                font.pixelSize: Style.fontSmall
                wrapMode: Text.WordWrap
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.gap

            ToggleChip {
                label: qsTr("Reduce motion")
                checked: root.reduceMotionDraft
                onToggled: root.reduceMotionDraft = !root.reduceMotionDraft
            }

            Text {
                Layout.fillWidth: true
                text: qsTr("Disable theme transition animations")
                color: theme.textMuted
                font.family: Style.fontSans
                font.pixelSize: Style.fontSmall
                wrapMode: Text.WordWrap
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true

        Item { Layout.fillWidth: true }

        ActionButton {
            label: qsTr("Save Appearance")
            variant: "primary"
            onClicked: {
                const err = backend.saveAppearanceSettings(
                    root.modeDraft,
                    root.highContrastDraft,
                    root.reduceMotionDraft
                )
                if (err.length > 0)
                    app.showConfigNotice(err, true)
                else
                    app.showConfigNotice(qsTr("Appearance preferences saved."), false)
            }
        }
    }
}
