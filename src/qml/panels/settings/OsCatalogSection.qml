import QtQuick
import QtQuick.Layouts
import "../../components"

// Settings section 2: editable Linux OS catalog.
ColumnLayout {
    id: root
    property QtObject app: null

    readonly property string osFieldId: osIdField.text
    readonly property string osFieldAlias: osAliasField.text
    readonly property string osFieldName: osNameField.text
    readonly property string osFieldBadge: osBadgeField.text
    readonly property string osFieldDescription: osDescriptionField.text

    Layout.fillWidth: true
    spacing: Style.gap

    function clearOsFields() {
        osIdField.text = ""
        osAliasField.text = ""
        osNameField.text = ""
        osBadgeField.text = ""
        osDescriptionField.text = ""
    }

    function setOsFields(idValue, aliasValue, nameValue, badgeValue, descriptionValue) {
        osIdField.text = idValue
        osAliasField.text = aliasValue
        osNameField.text = nameValue
        osBadgeField.text = badgeValue
        osDescriptionField.text = descriptionValue
    }

    function reload() {
        app.osEditorVisible = false
        app.osEditorError = ""
    }

    SectionHeader {
        title: qsTr("%n OS entries", "", backend.osCatalog.length)

        ActionButton {
            label: qsTr("+ Add OS")
            variant: "primary"
            onClicked: app.beginAddOs()
        }
    }

    // Inline editor for adding or editing an OS entry (Esc dismisses)
    Card {
        visible: app.osEditorVisible
        title: app.osEditorOriginalId.length > 0 ? qsTr("Edit OS entry") : qsTr("New OS entry")
        borderColor: theme.accentBorder
        contentSpacing: Style.gapMd

        GridLayout {
            Layout.fillWidth: true
            columns: 2
            columnSpacing: Style.gap
            rowSpacing: Style.gapMd

            ColumnLayout {
                Layout.fillWidth: true
                spacing: Style.gapSm

                Text {
                    text: qsTr("OS ID")
                    color: theme.textMuted
                    font.family: Style.fontSans
                    font.pixelSize: Style.fontCaption
                }
                FlatInput {
                    id: osIdField
                    Layout.fillWidth: true
                    mono: true
                    placeholder: qsTr("e.g. ubuntu-24.04")
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: Style.gapSm

                Text {
                    text: qsTr("Image alias")
                    color: theme.textMuted
                    font.family: Style.fontSans
                    font.pixelSize: Style.fontCaption
                }
                FlatInput {
                    id: osAliasField
                    Layout.fillWidth: true
                    mono: true
                    placeholder: qsTr("e.g. ubuntu/24.04")
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: Style.gapSm

                Text {
                    text: qsTr("Display name")
                    color: theme.textMuted
                    font.family: Style.fontSans
                    font.pixelSize: Style.fontCaption
                }
                FlatInput {
                    id: osNameField
                    Layout.fillWidth: true
                    placeholder: qsTr("e.g. Ubuntu 24.04 LTS")
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: Style.gapSm

                Text {
                    text: qsTr("Badge")
                    color: theme.textMuted
                    font.family: Style.fontSans
                    font.pixelSize: Style.fontCaption
                }
                FlatInput {
                    id: osBadgeField
                    Layout.fillWidth: true
                    placeholder: qsTr("e.g. Recommended")
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.columnSpan: 2
                spacing: Style.gapSm

                Text {
                    text: qsTr("Description")
                    color: theme.textMuted
                    font.family: Style.fontSans
                    font.pixelSize: Style.fontCaption
                }
                FlatInput {
                    id: osDescriptionField
                    Layout.fillWidth: true
                    placeholder: qsTr("Short description shown in the OS catalog")
                }
            }
        }

        Text {
            visible: app.osEditorError.length > 0
            Layout.fillWidth: true
            text: app.osEditorError
            color: theme.dangerText
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
            wrapMode: Text.WordWrap
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.gap

            Text {
                Layout.fillWidth: true
                text: qsTr("Press Esc to cancel")
                color: theme.textMuted
                font.family: Style.fontMono
                font.pixelSize: Style.fontCaption
            }

            ActionButton {
                label: qsTr("Save OS")
                variant: "primary"
                onClicked: app.submitOsEditor()
            }
        }
    }

    Repeater {
        model: backend.osCatalog

        delegate: Rectangle {
            required property var modelData
            Layout.fillWidth: true
            height: Style.listRowHeight
            radius: Math.min(Style.radiusMd, theme.radius)
            color: theme.surfaceInput
            border.width: Style.hairline
            border.color: modelData.downloaded ? theme.successBorder : theme.borderSubtle

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: Style.padCard
                anchors.rightMargin: Style.padCard
                spacing: Style.gap

                Rectangle {
                    width: Style.controlHeight
                    height: Style.controlHeight
                    radius: Math.min(Style.radiusMd, theme.radius)
                    color: modelData.downloaded ? theme.successSurface : theme.surfaceCard
                    border.width: Style.hairline
                    border.color: modelData.downloaded ? theme.successBorder : theme.borderSubtle

                    Text {
                        anchors.centerIn: parent
                        text: modelData.downloaded ? "✓" : "↓"
                        color: modelData.downloaded ? theme.successText : theme.accentText
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontTitle
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    spacing: Style.gapXs

                    Text {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: modelData.name
                        color: theme.textPrimary
                        font.family: Style.fontSans
                        font.pixelSize: Style.fontBody
                        font.weight: Font.Medium
                    }
                    Text {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: modelData.id + "  ·  " + modelData.alias
                        color: theme.textMuted
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontCaption
                    }
                }

                TagBadge {
                    text: modelData.badge
                    tone: "accentQuiet"
                }

                Text {
                    Layout.preferredWidth: Style.buttonMinWidth + Style.gap
                    horizontalAlignment: Text.AlignRight
                    elide: Text.ElideRight
                    text: modelData.sizeText
                    color: modelData.downloaded ? theme.successText : theme.textMuted
                    font.family: Style.fontMono
                    font.pixelSize: Style.fontCaption
                }

                ActionButton {
                    compact: true
                    label: qsTr("Edit")
                    onClicked: app.beginEditOs(modelData)
                }

                ActionButton {
                    compact: true
                    label: qsTr("Remove")
                    variant: "danger"
                    onClicked: app.removeOsEntry(modelData)
                }
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: Style.gap

        Text {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: qsTr("Removing an entry only removes it from the catalog. Cached image files are kept until you remove them in OS Images.")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }

        ActionButton {
            label: qsTr("Restore Defaults")
            onClicked: {
                const error = backend.resetOsCatalog()
                if (error.length > 0)
                    app.showConfigNotice(error, true)
                else
                    app.showConfigNotice(qsTr("Default OS catalog restored."), false)
            }
        }
    }
}
