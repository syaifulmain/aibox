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
    spacing: Style.space10

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

    RowLayout {
        Layout.fillWidth: true
        spacing: Style.space10

        Text {
            Layout.fillWidth: true
            text: qsTr("%n OS entries", "", backend.osCatalog.length)
            color: theme.muted
            font.family: Style.fontSans
            font.pixelSize: Style.font11
        }

        ActionButton {
            label: qsTr("+ Add OS")
            variant: "primary"
            minWidth: 105
            onClicked: app.beginAddOs()
        }
    }

    // Inline editor for adding or editing an OS entry
    Rectangle {
        visible: app.osEditorVisible
        Layout.fillWidth: true
        Layout.preferredHeight: visible ? osEditorColumn.implicitHeight + Style.cardPadding * 2 : 0
        radius: Math.min(Style.radius6, theme.radius)
        color: Style.surfaceInput
        border.width: Style.hairline
        border.color: theme.accent

        ColumnLayout {
            id: osEditorColumn
            anchors.fill: parent
            anchors.margins: Style.cardPadding
            spacing: Style.space8

            Text {
                text: app.osEditorOriginalId.length > 0 ? qsTr("Edit OS entry") : qsTr("New OS entry")
                color: theme.foreground
                font.family: Style.fontSans
                font.pixelSize: Style.font13
                font.weight: Font.Medium
            }

            GridLayout {
                Layout.fillWidth: true
                columns: 2
                columnSpacing: Style.space10
                rowSpacing: Style.space8

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Style.space4

                    Text {
                        text: qsTr("OS ID")
                        color: theme.muted
                        font.family: Style.fontSans
                        font.pixelSize: Style.font10
                    }
                    FlatInput {
                        id: osIdField
                        Layout.fillWidth: true
                        placeholder: qsTr("e.g. ubuntu-24.04")
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Style.space4

                    Text {
                        text: qsTr("Image alias")
                        color: theme.muted
                        font.family: Style.fontSans
                        font.pixelSize: Style.font10
                    }
                    FlatInput {
                        id: osAliasField
                        Layout.fillWidth: true
                        placeholder: qsTr("e.g. ubuntu/24.04")
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Style.space4

                    Text {
                        text: qsTr("Display name")
                        color: theme.muted
                        font.family: Style.fontSans
                        font.pixelSize: Style.font10
                    }
                    FlatInput {
                        id: osNameField
                        Layout.fillWidth: true
                        placeholder: qsTr("e.g. Ubuntu 24.04 LTS")
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Style.space4

                    Text {
                        text: qsTr("Badge")
                        color: theme.muted
                        font.family: Style.fontSans
                        font.pixelSize: Style.font10
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
                    spacing: Style.space4

                    Text {
                        text: qsTr("Description")
                        color: theme.muted
                        font.family: Style.fontSans
                        font.pixelSize: Style.font10
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
                color: theme.red
                font.family: Style.fontSans
                font.pixelSize: Style.font11
                wrapMode: Text.WordWrap
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space8
                Item { Layout.fillWidth: true }

                ActionButton {
                    label: qsTr("Cancel")
                    minWidth: 88
                    onClicked: app.osEditorVisible = false
                }

                ActionButton {
                    label: qsTr("Save OS")
                    variant: "primary"
                    minWidth: 108
                    onClicked: app.submitOsEditor()
                }
            }
        }
    }

    Repeater {
        model: backend.osCatalog

        delegate: Rectangle {
            required property var modelData
            Layout.fillWidth: true
            height: 64
            radius: Math.min(Style.radius5, theme.radius)
            color: Style.surfaceInput
            border.width: Style.hairline
            border.color: modelData.downloaded ? Style.successBorder : Style.borderSubtle

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: Style.space12
                anchors.rightMargin: Style.space12
                spacing: Style.space10

                Rectangle {
                    width: 30
                    height: 30
                    radius: Style.radius3
                    color: modelData.downloaded ? Style.successSurface : Style.surfaceCard
                    border.width: Style.hairline
                    border.color: modelData.downloaded ? Style.successBorder : Style.borderSubtle

                    Text {
                        anchors.centerIn: parent
                        text: modelData.downloaded ? "✓" : "↓"
                        color: modelData.downloaded ? theme.green : theme.accent
                        font.family: Style.fontMono
                        font.pixelSize: Style.font12
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Style.space1

                    Text {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: modelData.name
                        color: theme.foreground
                        font.family: Style.fontSans
                        font.pixelSize: Style.font12
                        font.weight: Font.Medium
                    }
                    Text {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: modelData.id + "  ·  " + modelData.alias
                        color: theme.muted
                        font.family: Style.fontMono
                        font.pixelSize: Style.font9
                    }
                }

                TagBadge {
                    text: modelData.badge
                    tone: "accentQuiet"
                    fillColor: Style.surfaceCard
                    horizontalPadding: Style.space12
                }

                Text {
                    Layout.preferredWidth: 100
                    horizontalAlignment: Text.AlignRight
                    elide: Text.ElideRight
                    text: modelData.sizeText
                    color: modelData.downloaded ? theme.green : theme.muted
                    font.family: Style.fontMono
                    font.pixelSize: Style.font9
                }

                ActionButton {
                    implicitHeight: Style.controlHeightXs
                    label: qsTr("Edit")
                    minWidth: 58
                    onClicked: app.beginEditOs(modelData)
                }

                ActionButton {
                    implicitHeight: Style.controlHeightXs
                    label: qsTr("Remove")
                    danger: true
                    minWidth: 72
                    onClicked: app.removeOsEntry(modelData)
                }
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: Style.space10

        Text {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: qsTr("Removing an entry only removes it from the catalog. Cached image files are kept until you remove them in OS Images.")
            color: theme.muted
            font.family: Style.fontSans
            font.pixelSize: Style.font10
        }

        ActionButton {
            label: qsTr("Restore Defaults")
            minWidth: 135
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
