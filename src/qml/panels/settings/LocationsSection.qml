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

    Layout.fillWidth: true
    spacing: Style.space12

    function setPaths(wsPath, osPath) {
        workspacePathField.text = wsPath
        osStoragePathField.text = osPath
    }

    function reload() {
        workspacePathField.text = backend.workspaceBase
        osStoragePathField.text = backend.osStoragePath
    }

    // System root summary
    Rectangle {
        Layout.fillWidth: true
        implicitHeight: sysRootInfoCol.implicitHeight + Style.cardPaddingSm * 2
        radius: Math.min(Style.radius6, theme.radius)
        color: theme.surfaceInput
        border.width: Style.hairline
        border.color: theme.borderSubtle

        ColumnLayout {
            id: sysRootInfoCol
            anchors.fill: parent
            anchors.margins: Style.cardPaddingSm
            spacing: Style.space4

            Text {
                text: qsTr("System root location")
                color: theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.font12
                font.weight: Font.Medium
            }
            Text {
                Layout.fillWidth: true
                text: qsTr("Root: %1  ·  Config: %2").arg(backend.systemPath).arg(backend.configFilePath)
                color: theme.textMuted
                font.family: Style.fontMono
                font.pixelSize: Style.font10
                elide: Text.ElideMiddle
            }
        }
    }

    // Workspace location
    Rectangle {
        Layout.fillWidth: true
        implicitHeight: wsLocCardCol.implicitHeight + Style.cardPadding * 2
        radius: Math.min(Style.radius6, theme.radius)
        color: theme.surfaceInput
        border.width: Style.hairline
        border.color: theme.borderSubtle

        ColumnLayout {
            id: wsLocCardCol
            anchors.fill: parent
            anchors.margins: Style.cardPadding
            spacing: Style.space8

            Text {
                text: qsTr("Workspace location")
                color: theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.font12
                font.weight: Font.Medium
            }
            Text {
                Layout.fillWidth: true
                text: qsTr("New environments share this host folder at %1. Existing environments keep their current shared folder.").arg(backend.containerWorkspacePath)
                color: theme.textMuted
                font.family: Style.fontSans
                font.pixelSize: Style.font10
                wrapMode: Text.WordWrap
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space10

                FlatInput {
                    id: workspacePathField
                    Layout.fillWidth: true
                    placeholder: backend.defaultWorkspaceBase
                }

                ActionButton {
                    label: qsTr("Browse...")
                    minWidth: 94
                    onClicked: workspaceFolderDialog.open()
                }

                ActionButton {
                    label: qsTr("Default")
                    minWidth: 84
                    onClicked: workspacePathField.text = backend.defaultWorkspaceBase
                }
            }

            Text {
                Layout.fillWidth: true
                text: qsTr("Default: %1").arg(backend.defaultWorkspaceBase)
                color: theme.textMuted
                font.family: Style.fontMono
                font.pixelSize: Style.font9
                elide: Text.ElideMiddle
            }
        }
    }

    // OS image storage
    Rectangle {
        Layout.fillWidth: true
        implicitHeight: osLocCardCol.implicitHeight + Style.cardPadding * 2
        radius: Math.min(Style.radius6, theme.radius)
        color: theme.surfaceInput
        border.width: Style.hairline
        border.color: theme.borderSubtle

        ColumnLayout {
            id: osLocCardCol
            anchors.fill: parent
            anchors.margins: Style.cardPadding
            spacing: Style.space8

            Text {
                text: qsTr("OS image storage")
                color: theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.font12
                font.weight: Font.Medium
            }
            Text {
                Layout.fillWidth: true
                text: qsTr("Downloaded OS images are also exported here. If the local Incus image is gone, AIBox imports the copy from this folder instead of downloading again.")
                color: theme.textMuted
                font.family: Style.fontSans
                font.pixelSize: Style.font10
                wrapMode: Text.WordWrap
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space10

                FlatInput {
                    id: osStoragePathField
                    Layout.fillWidth: true
                    placeholder: backend.defaultOsStoragePath
                }

                ActionButton {
                    label: qsTr("Browse...")
                    minWidth: 94
                    onClicked: osStorageFolderDialog.open()
                }

                ActionButton {
                    label: qsTr("Default")
                    minWidth: 84
                    onClicked: osStoragePathField.text = backend.defaultOsStoragePath
                }
            }

            Text {
                Layout.fillWidth: true
                text: qsTr("Default: %1").arg(backend.defaultOsStoragePath)
                color: theme.textMuted
                font.family: Style.fontMono
                font.pixelSize: Style.font9
                elide: Text.ElideMiddle
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true
        Layout.topMargin: Style.space2
        spacing: Style.space10
        Item { Layout.fillWidth: true }

        ActionButton {
            label: qsTr("Save Locations")
            variant: "primary"
            minWidth: 150
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
