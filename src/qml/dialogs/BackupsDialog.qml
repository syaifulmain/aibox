import QtQuick
import QtQuick.Layouts
import QtQuick.Dialogs
import "../components"

ModalDialog {
    id: backupsOverlay
    property QtObject app: null
    open: app.backupsVisible
    zOrder: 62
    cardWidth: Math.min(parent.width - Style.modalGutter, 780)
    cardHeight: Math.min(parent.height - Style.modalEdge, 600)
    onDismissed: app.backupsVisible = false

    ModalHeader {
        title: qsTr("Environment Backups")
        subtitle: backend.backupsDirPath
        subtitleMono: true
        onClosed: app.backupsVisible = false
    }

    // Inline validation error
    Rectangle {
        visible: app.backupError.length > 0
        Layout.fillWidth: true
        implicitHeight: bErrTxt.implicitHeight + Style.space14
        radius: Math.min(Style.radius4, theme.radius)
        color: Style.dangerSurfaceAlt
        border.width: Style.hairline
        border.color: theme.red
        Text {
            id: bErrTxt
            anchors.fill: parent
            anchors.margins: Style.space7
            text: app.backupError
            color: theme.red
            font.family: Style.fontSans
            font.pixelSize: Style.font11
            wrapMode: Text.WordWrap
        }
    }

    // Export selected environment
    Rectangle {
        id: exportCard
        Layout.fillWidth: true
        implicitHeight: exportCol.implicitHeight + Style.space20
        radius: Math.min(Style.radius4, theme.radius)
        color: Style.surfaceInput
        border.width: Style.hairline
        border.color: Style.borderSubtle

        readonly property var spaceInfo: (app.backupsVisible && backend.selectedEnvironment && backend.selectedEnvironment.name)
            ? backend.checkExportSpace(backend.selectedEnvironment.name, app.backupIncludeWorkspace)
            : ({ ok: true, estimatedDisplay: "—", availableDisplay: "—", message: "" })

        ColumnLayout {
            id: exportCol
            anchors.fill: parent
            anchors.margins: Style.space10
            spacing: Style.space8

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space10

                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    spacing: Style.space2

                    Text {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: (backend.selectedEnvironment && backend.selectedEnvironment.name)
                              ? qsTr("Export '%1'").arg(backend.selectedEnvironment.name)
                              : qsTr("No environment selected to export")
                        color: theme.foreground
                        font.family: Style.fontSans
                        font.pixelSize: Style.font13
                        font.weight: Font.Medium
                        elide: Text.ElideMiddle
                    }

                    Text {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: qsTr("Est. size: %1  •  Free: %2")
                              .arg(exportCard.spaceInfo.estimatedDisplay || "—")
                              .arg(exportCard.spaceInfo.availableDisplay || "—")
                        color: exportCard.spaceInfo.ok ? theme.muted : theme.red
                        font.family: Style.fontMono
                        font.pixelSize: Style.font11
                        elide: Text.ElideRight
                    }
                }

                // Spacer keeps the action buttons on the right edge. A nested
                // layout alone does not stretch the row.
                Item {
                    Layout.fillWidth: true
                    Layout.preferredHeight: Style.space1
                }

                ActionButton {
                    label: app.backupIncludeWorkspace ? qsTr("Include Workspace: ON") : qsTr("Include Workspace: OFF")
                    minWidth: 168
                    onClicked: app.backupIncludeWorkspace = !app.backupIncludeWorkspace
                }

                ActionButton {
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

    Text {
        text: qsTr("Saved Backup Archives (%1)").arg(backend.backupArchives.length)
        color: theme.muted
        font.family: Style.fontSans
        font.pixelSize: Style.font11
        font.weight: Font.Medium
    }

    Rectangle {
        Layout.fillWidth: true
        Layout.fillHeight: true
        radius: Math.min(Style.radius4, theme.radius)
        color: Style.surfaceInput
        border.width: Style.hairline
        border.color: Style.borderSubtle

        Text {
            anchors.centerIn: parent
            visible: backend.backupArchives.length === 0
            text: qsTr("No backup archives found in %1").arg(backend.backupsDirPath)
            color: theme.muted
            font.family: Style.fontSans
            font.pixelSize: Style.font12
        }

        ListView {
            anchors.fill: parent
            anchors.margins: Style.space6
            clip: true
            spacing: Style.space6
            model: backend.backupArchives
            delegate: Rectangle {
                required property var modelData
                width: ListView.view.width
                height: 48
                radius: Math.min(Style.radius3, theme.radius)
                color: Style.surfaceCard
                border.width: Style.hairline
                border.color: app.importBackupFile === (modelData.filePath || "") ? theme.accent : Style.borderSubtle

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Style.space12
                    anchors.rightMargin: Style.space12
                    spacing: Style.space10

                    ColumnLayout {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        spacing: Style.space2

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Style.space8

                            Text {
                                Layout.fillWidth: true
                                Layout.minimumWidth: 0
                                text: modelData.fileName || ""
                                color: theme.foreground
                                font.family: Style.fontMono
                                font.pixelSize: Style.font11
                                elide: Text.ElideMiddle
                            }

                            TagBadge {
                                visible: modelData.hasWorkspaceArchive === true
                                text: qsTr("+ Workspace (%1)").arg(modelData.workspaceArchiveSizeDisplay || "")
                                tone: "accent"
                            }
                        }

                        Text {
                            text: "%1  •  %2"
                                  .arg(modelData.sizeDisplay || "")
                                  .arg(modelData.createdAt || "")
                            color: theme.muted
                            font.family: Style.fontMono
                            font.pixelSize: Style.font10
                        }
                    }

                    ActionButton {
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
        implicitHeight: importCol.implicitHeight + Style.space20
        radius: Math.min(Style.radius4, theme.radius)
        color: Style.surfaceInput
        border.width: Style.hairline
        border.color: Style.borderSubtle

        ColumnLayout {
            id: importCol
            anchors.fill: parent
            anchors.margins: Style.space12
            spacing: Style.space10

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space10

                Text {
                    text: qsTr("Import Archive:")
                    color: theme.foreground
                    font.family: Style.fontSans
                    font.pixelSize: Style.font12
                    Layout.preferredWidth: 102
                }

                FlatInput {
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
                    label: qsTr("Browse...")
                    minWidth: 132
                    onClicked: backupBrowseDialog.open()
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space10

                Text {
                    text: qsTr("New Env Name:")
                    color: theme.foreground
                    font.family: Style.fontSans
                    font.pixelSize: Style.font12
                    Layout.preferredWidth: 102
                }

                FlatInput {
                    Layout.fillWidth: true
                    text: app.importNewName
                    placeholder: "restored-env"
                    onTextChanged: {
                        if (text !== app.importNewName)
                            app.importNewName = text
                    }
                }

                ActionButton {
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
