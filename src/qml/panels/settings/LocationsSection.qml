import QtQuick
import QtQuick.Layouts
import QtQuick.Dialogs
import "../../components"

// Settings section 0: workspace and OS image storage locations.
ColumnLayout {
    id: root
    property QtObject app: null

    readonly property string workspacePathText: workspacePathField.text
    readonly property string osStoragePathText: osStoragePathField.text
    readonly property int formLabelWidth: Style.buttonMinWidth + Style.padModal

    Layout.fillWidth: true
    spacing: Style.gap

    function setPaths(wsPath, osPath) {
        workspacePathField.text = wsPath
        osStoragePathField.text = osPath
    }

    function reload() {
        workspacePathField.text = backend.workspaceBase
        osStoragePathField.text = backend.osStoragePath
    }

    Card {
        title: qsTr("System root location")
        contentSpacing: Style.gapSm

        Text {
            Layout.fillWidth: true
            text: qsTr("Root: %1  ·  Config: %2").arg(backend.systemPath).arg(backend.configFilePath)
            color: theme.textMuted
            font.family: Style.fontMono
            font.pixelSize: Style.fontCaption
            elide: Text.ElideMiddle
        }
    }

    Card {
        title: qsTr("Workspace location")
        contentSpacing: Style.gapMd

        Text {
            Layout.fillWidth: true
            text: qsTr("New environments share this host folder at %1. Existing environments keep their current shared folder.").arg(backend.containerWorkspacePath)
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
            wrapMode: Text.WordWrap
        }

        FormRow {
            label: qsTr("Workspace path")
            labelWidth: root.formLabelWidth
            hint: qsTr("Default: %1").arg(backend.defaultWorkspaceBase)

            FlatInput {
                id: workspacePathField
                Layout.fillWidth: true
                mono: true
                placeholder: backend.defaultWorkspaceBase
            }

            ActionButton {
                label: qsTr("Browse...")
                onClicked: workspaceFolderDialog.open()
            }

            ActionButton {
                label: qsTr("Default")
                onClicked: workspacePathField.text = backend.defaultWorkspaceBase
            }
        }
    }

    Card {
        title: qsTr("OS image storage")
        contentSpacing: Style.gapMd

        Text {
            Layout.fillWidth: true
            text: qsTr("Downloaded OS images are also exported here. If the local Incus image is gone, AIBox imports the copy from this folder instead of downloading again.")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
            wrapMode: Text.WordWrap
        }

        FormRow {
            label: qsTr("Storage path")
            labelWidth: root.formLabelWidth
            hint: qsTr("Default: %1").arg(backend.defaultOsStoragePath)

            FlatInput {
                id: osStoragePathField
                Layout.fillWidth: true
                mono: true
                placeholder: backend.defaultOsStoragePath
            }

            ActionButton {
                label: qsTr("Browse...")
                onClicked: osStorageFolderDialog.open()
            }

            ActionButton {
                label: qsTr("Default")
                onClicked: osStoragePathField.text = backend.defaultOsStoragePath
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true
        Layout.topMargin: Style.gapXs
        spacing: Style.gap
        Item { Layout.fillWidth: true }

        ActionButton {
            label: qsTr("Save Locations")
            variant: "primary"
            onClicked: root.app.saveGeneralSettings()
        }
    }

    FolderDialog {
        id: workspaceFolderDialog
        title: qsTr("Choose Workspace Location")
        currentFolder: backend.pathToUrl(backend.workspaceBase)
        onAccepted: workspacePathField.text = backend.localPath(selectedFolder)
    }

    FolderDialog {
        id: osStorageFolderDialog
        title: qsTr("Choose OS Image Storage Location")
        currentFolder: backend.pathToUrl(backend.osStoragePath)
        onAccepted: osStoragePathField.text = backend.localPath(selectedFolder)
    }
}
