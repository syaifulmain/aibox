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
    spacing: Style.space12

    function reload() {
        modeDraft = backend.themeModeOverride
        highContrastDraft = backend.highContrast
        reduceMotionDraft = backend.reduceMotion
    }

    Rectangle {
        Layout.fillWidth: true
        Layout.preferredHeight: modeCardCol.implicitHeight + Style.cardPadding * 2
        radius: Math.min(Style.radius6, theme.radius)
        color: theme.surfaceInput
        border.width: Style.hairline
        border.color: theme.borderSubtle

        ColumnLayout {
            id: modeCardCol
            anchors.fill: parent
            anchors.margins: Style.cardPadding
            spacing: Style.space10

            Text {
                text: qsTr("Theme Mode")
                color: theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.font12
                font.weight: Font.Medium
            }

            Text {
                Layout.fillWidth: true
                text: qsTr("Follow the active Omarchy theme automatically, or force a light or dark palette inside AIBox.")
                color: theme.textMuted
                font.family: Style.fontSans
                font.pixelSize: Style.font10
                wrapMode: Text.WordWrap
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space8

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
    }

    Rectangle {
        Layout.fillWidth: true
        Layout.preferredHeight: accessCardCol.implicitHeight + Style.cardPadding * 2
        radius: Math.min(Style.radius6, theme.radius)
        color: theme.surfaceInput
        border.width: Style.hairline
        border.color: theme.borderSubtle

        ColumnLayout {
            id: accessCardCol
            anchors.fill: parent
            anchors.margins: Style.cardPadding
            spacing: Style.space10

            Text {
                text: qsTr("Accessibility & Motion")
                color: theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.font12
                font.weight: Font.Medium
            }

            CheckRow {
                Layout.fillWidth: true
                checked: root.highContrastDraft
                text: qsTr("High contrast (7:1 text, 3:1 borders)")
                onToggled: root.highContrastDraft = !root.highContrastDraft
            }

            CheckRow {
                Layout.fillWidth: true
                checked: root.reduceMotionDraft
                text: qsTr("Reduce motion (disable theme transition animations)")
                onToggled: root.reduceMotionDraft = !root.reduceMotionDraft
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
                if (err.length > 0) {
                    app.configNoticeError = true
                    app.configNotice = err
                } else {
                    app.configNoticeError = false
                    app.configNotice = qsTr("Appearance preferences saved.")
                }
            }
        }
    }
}
