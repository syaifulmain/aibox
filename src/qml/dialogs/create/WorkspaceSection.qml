import QtQuick
import QtQuick.Layouts
import QtQuick.Dialogs
import "../../components"

// Name, shared folder location, and Fresh-OS vs. From-Template selection.
ColumnLayout {
    id: root
    property QtObject app: null

    Layout.fillWidth: true
    spacing: Style.padCard

    function resetSharedPath() {
        createSharedPathField.text = ""
    }

    // Project name and resulting shared path
    ColumnLayout {
        Layout.fillWidth: true
        spacing: Style.gapSm

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.gap

            Text {
                text: qsTr("Project / Environment Name")
                color: theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.fontBody
                font.weight: Font.Medium
            }

            Text {
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                horizontalAlignment: Text.AlignRight
                text: qsTr("Shared at: %1/%2")
                      .arg(root.app.newEnvSharedPath.trim().length > 0 ? root.app.newEnvSharedPath.trim() : backend.workspaceBase)
                      .arg(root.app.newEnvName.length > 0 ? root.app.newEnvName : "<name>")
                color: theme.textMuted
                font.family: Style.fontMono
                font.pixelSize: Style.fontSmall
                elide: Text.ElideMiddle
            }
        }

        FlatInput {
            id: createNameField
            Layout.fillWidth: true
            placeholder: qsTr("e.g. pos-desktop, flutter-mobile, web-api")
            text: root.app.newEnvName
            onTextChanged: root.app.newEnvName = text
        }
    }

    // Optional custom shared folder location (defaults to the configured base)
    ColumnLayout {
        Layout.fillWidth: true
        spacing: Style.gapSm

        Text {
            text: qsTr("Shared Folder Location (optional)")
            color: theme.textPrimary
            font.family: Style.fontSans
            font.pixelSize: Style.fontBody
            font.weight: Font.Medium
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.gapMd

            FlatInput {
                id: createSharedPathField
                Layout.fillWidth: true
                placeholder: qsTr("Default: %1").arg(backend.workspaceBase)
                text: root.app.newEnvSharedPath
                onTextChanged: root.app.newEnvSharedPath = text
            }

            ActionButton {
                label: qsTr("Browse...")
                onClicked: createSharedFolderDialog.open()
            }

            ActionButton {
                label: qsTr("Default")
                onClicked: {
                    root.app.newEnvSharedPath = ""
                    createSharedPathField.text = ""
                }
            }
        }
    }

    // Source mode switcher (shown when templates exist)
    RowLayout {
        visible: backend.templates.length > 0
        Layout.fillWidth: true
        spacing: Style.gapMd

        Repeater {
            model: [
                { fromTpl: false, label: qsTr("Fresh OS") },
                { fromTpl: true, label: qsTr("From Template (%1)").arg(backend.templates.length) }
            ]
            delegate: SelectChip {
                required property var modelData
                Layout.fillWidth: true
                Layout.preferredWidth: 0
                text: modelData.label
                selected: root.app.createFromTemplate === modelData.fromTpl
                mono: false
                textSize: Style.fontBody
                mediumWhenSelected: true
                selectedTextColor: theme.textPrimary
                chipHeight: Style.controlHeight
                onClicked: {
                    root.app.createFromTemplate = modelData.fromTpl
                    if (modelData.fromTpl && root.app.newEnvTemplateName.length === 0 && backend.templates.length > 0)
                        root.app.newEnvTemplateName = backend.templates[0].name || ""
                }
            }
        }
    }

    // Template picker (when From Template is selected)
    ColumnLayout {
        visible: root.app.createFromTemplate && backend.templates.length > 0
        Layout.fillWidth: true
        spacing: Style.gapSm

        Text {
            text: qsTr("Select Template")
            color: theme.textPrimary
            font.family: Style.fontSans
            font.pixelSize: Style.fontBody
            font.weight: Font.Medium
        }

        GridLayout {
            Layout.fillWidth: true
            columns: 2
            columnSpacing: Style.gapMd
            rowSpacing: Style.gapMd

            Repeater {
                model: backend.templates
                delegate: SelectCard {
                    required property var modelData
                    Layout.fillWidth: true
                    selected: root.app.newEnvTemplateName === modelData.name
                    title: modelData.name
                    subtitle: modelData.osName + " · " + modelData.diskLimit + " · " + modelData.memoryLimit
                    titleMono: true
                    onClicked: root.app.newEnvTemplateName = modelData.name
                }
            }
        }
    }

    FolderDialog {
        id: createSharedFolderDialog
        title: qsTr("Choose Custom Shared Folder Location")
        currentFolder: backend.pathToUrl(root.app.newEnvSharedPath.trim().length > 0
                                         ? root.app.newEnvSharedPath.trim() : backend.workspaceBase)
        onAccepted: {
            root.app.newEnvSharedPath = backend.localPath(selectedFolder)
            createSharedPathField.text = root.app.newEnvSharedPath
        }
    }
}
