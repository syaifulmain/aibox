import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts
import "../components"

Rectangle {
    property QtObject app: null
    id: backupsOverlay
    visible: app.backupsVisible
    anchors.fill: parent
    color: "#c406070a"
    z: 62

    MouseArea {
        anchors.fill: parent
        onClicked: app.backupsVisible = false
    }

    Rectangle {
        width: Math.min(780, parent.width - 56)
        height: Math.min(600, parent.height - 56)
        anchors.centerIn: parent
        radius: Math.min(8, theme.radius)
        color: app.surfaceCard
        border.width: 1
        border.color: theme.accent

        MouseArea { anchors.fill: parent }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 24
            spacing: 12

            ModalHeader {
                app: backupsOverlay.app
                title: qsTr("Environment Backups")
                subtitle: backend.backupsDirPath
                subtitleMono: true
                onClosed: app.backupsVisible = false
            }

            // Inline validation error
            Rectangle {
                visible: app.backupError.length > 0
                Layout.fillWidth: true
                implicitHeight: bErrTxt.implicitHeight + 14
                radius: Math.min(4, theme.radius)
                color: "#291418"
                border.width: 1
                border.color: app.redColor
                Text {
                    id: bErrTxt
                    anchors.fill: parent
                    anchors.margins: 7
                    text: app.backupError
                    color: app.redColor
                    font.family: app.sansFont
                    font.pixelSize: 11
                    wrapMode: Text.WordWrap
                }
            }

            // Export selected environment
            Rectangle {
                id: exportCard
                Layout.fillWidth: true
                implicitHeight: exportCol.implicitHeight + 20
                radius: Math.min(4, theme.radius)
                color: app.surfaceInput
                border.width: 1
                border.color: app.borderSubtle

                readonly property var spaceInfo: (app.backupsVisible && backend.selectedEnvironment && backend.selectedEnvironment.name)
                    ? backend.checkExportSpace(backend.selectedEnvironment.name, app.backupIncludeWorkspace)
                    : ({ ok: true, estimatedDisplay: "—", availableDisplay: "—", message: "" })

                ColumnLayout {
                    id: exportCol
                    anchors.fill: parent
                    anchors.margins: 10
                    spacing: 8

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 10

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 2
                            Text {
                                text: (backend.selectedEnvironment && backend.selectedEnvironment.name)
                                      ? qsTr("Export '%1'").arg(backend.selectedEnvironment.name)
                                      : qsTr("No environment selected to export")
                                color: theme.foreground
                                font.family: app.sansFont
                                font.pixelSize: 13
                                font.weight: Font.Medium
                            }
                            Text {
                                text: qsTr("Est. size: %1  •  Free: %2")
                                      .arg(exportCard.spaceInfo.estimatedDisplay || "—")
                                      .arg(exportCard.spaceInfo.availableDisplay || "—")
                                color: exportCard.spaceInfo.ok ? theme.muted : app.redColor
                                font.family: app.monoFont
                                font.pixelSize: 11
                            }
                        }

                        ActionButton {
                            app: backupsOverlay.app
                            label: app.backupIncludeWorkspace ? qsTr("Include Workspace: ON") : qsTr("Include Workspace: OFF")
                            minWidth: 168
                            onClicked: app.backupIncludeWorkspace = !app.backupIncludeWorkspace
                        }

                        ActionButton {
                            app: backupsOverlay.app
                            label: qsTr("Export Backup")
                            variant: "primary"
                            minWidth: 130
                            disabled: backend.busy || !(backend.selectedEnvironment && backend.selectedEnvironment.name) || !exportCard.spaceInfo.ok
                            onClicked: {
                                const err = backend.exportEnvironment(backend.selectedEnvironment.name, app.backupIncludeWorkspace)
                                if (err.length > 0) {
                                    app.backupError = err
                                } else {
                                    app.backupsVisible = false
                                }
                            }
                        }
                    }
                }
            }

            // Saved backups list
            Text {
                text: qsTr("Saved Backup Archives (%1)").arg(backend.backupArchives.length)
                color: theme.muted
                font.family: app.sansFont
                font.pixelSize: 11
                font.weight: Font.Medium
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                radius: Math.min(4, theme.radius)
                color: app.surfaceInput
                border.width: 1
                border.color: app.borderSubtle

                Text {
                    anchors.centerIn: parent
                    visible: backend.backupArchives.length === 0
                    text: qsTr("No backup archives found in %1").arg(backend.backupsDirPath)
                    color: theme.muted
                    font.family: app.sansFont
                    font.pixelSize: 12
                }

                ListView {
                    anchors.fill: parent
                    anchors.margins: 6
                    clip: true
                    spacing: 6
                    model: backend.backupArchives
                    delegate: Rectangle {
                        required property var modelData
                        width: ListView.view.width
                        height: 48
                        radius: Math.min(3, theme.radius)
                        color: app.surfaceCard
                        border.width: 1
                        border.color: app.importBackupFile === (modelData.filePath || "") ? theme.accent : app.borderSubtle

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 12
                            spacing: 10

                            ColumnLayout {
                                Layout.fillWidth: true
                                Layout.minimumWidth: 0
                                spacing: 2
                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 8
                                    Text {
                                        Layout.fillWidth: true
                                        Layout.minimumWidth: 0
                                        text: modelData.fileName || ""
                                        color: theme.foreground
                                        font.family: app.monoFont
                                        font.pixelSize: 11
                                        elide: Text.ElideMiddle
                                    }
                                    Rectangle {
                                        visible: modelData.hasWorkspaceArchive === true
                                        width: wsTag.implicitWidth + 10
                                        height: 18
                                        radius: 2
                                        color: app.surfaceSelected
                                        border.width: 1
                                        border.color: theme.accent
                                        Text {
                                            id: wsTag
                                            anchors.centerIn: parent
                                            text: qsTr("+ Workspace (%1)").arg(modelData.workspaceArchiveSizeDisplay || "")
                                            color: theme.accent
                                            font.family: app.monoFont
                                            font.pixelSize: 9
                                        }
                                    }
                                }
                                Text {
                                    text: "%1  •  %2"
                                          .arg(modelData.sizeDisplay || "")
                                          .arg(modelData.createdAt || "")
                                    color: theme.muted
                                    font.family: app.monoFont
                                    font.pixelSize: 10
                                }
                            }

                            ActionButton {
                                app: backupsOverlay.app
                                label: qsTr("Select")
                                minWidth: 74
                                onClicked: {
                                    app.importBackupFile = modelData.filePath || ""
                                    app.importWsFile = modelData.workspaceArchivePath || ""
                                    app.importNewName = (modelData.envName || "restored") + "-restored"
                                    app.backupError = ""
                                }
                            }

                            ActionButton {
                                app: backupsOverlay.app
                                label: qsTr("Delete")
                                variant: "danger"
                                minWidth: 74
                                disabled: backend.busy
                                onClicked: backend.deleteBackup(modelData.filePath || "", modelData.workspaceArchivePath || "")
                            }
                        }
                    }
                }
            }

            // Import backup form
            Rectangle {
                Layout.fillWidth: true
                implicitHeight: importCol.implicitHeight + 20
                radius: Math.min(4, theme.radius)
                color: app.surfaceInput
                border.width: 1
                border.color: app.borderSubtle

                ColumnLayout {
                    id: importCol
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 10

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 10
                        Text {
                            text: qsTr("Import Archive:")
                            color: theme.foreground
                            font.family: app.sansFont
                            font.pixelSize: 12
                            Layout.preferredWidth: 102
                        }
                        FlatInput {
                            app: backupsOverlay.app
                            Layout.fillWidth: true
                            text: app.importBackupFile
                            placeholder: "/path/to/env-backup.tar.gz"
                            onTextChanged: {
                                if (text !== app.importBackupFile) {
                                    app.importBackupFile = text
                                    app.importWsFile = ""
                                }
                            }
                        }
                        ActionButton {
                            app: backupsOverlay.app
                            label: qsTr("Browse...")
                            minWidth: 132
                            onClicked: backupBrowseDialog.open()
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 10
                        Text {
                            text: qsTr("New Env Name:")
                            color: theme.foreground
                            font.family: app.sansFont
                            font.pixelSize: 12
                            Layout.preferredWidth: 102
                        }
                        FlatInput {
                            app: backupsOverlay.app
                            Layout.fillWidth: true
                            text: app.importNewName
                            placeholder: "restored-env"
                            onTextChanged: {
                                if (text !== app.importNewName)
                                    app.importNewName = text
                            }
                        }
                        ActionButton {
                            app: backupsOverlay.app
                            label: qsTr("Import Backup")
                            variant: "primary"
                            minWidth: 132
                            disabled: backend.busy || app.importBackupFile.trim().length === 0 || app.importNewName.trim().length === 0
                            onClicked: {
                                const err = backend.importEnvironment(app.importBackupFile, app.importNewName, app.importWsFile)
                                if (err.length > 0) {
                                    app.backupError = err
                                } else {
                                    app.backupsVisible = false
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    FileDialog {
        id: backupBrowseDialog
        title: qsTr("Choose Environment Backup Archive")
        currentFolder: backend.pathToUrl(backend.backupsDirPath)
        nameFilters: [qsTr("Backup archives (*.tar.gz)"), qsTr("All files (*)")]
        onAccepted: {
            app.importBackupFile = backend.localPath(selectedFile)
            app.importWsFile = ""
            if (app.importNewName.trim().length === 0)
                app.importNewName = "restored-env"
        }
    }
}
