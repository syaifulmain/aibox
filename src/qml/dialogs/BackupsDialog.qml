import QtQuick
import QtQuick.Layouts
import QtQuick.Dialogs
import "../components"

ModalDialog {
    id: backupsOverlay
    property QtObject app: null
    open: app.backupsVisible
    zOrder: 62
    size: "md"
    cardHeight: Math.min(parent.height - Style.modalGutter, 600)
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
        implicitHeight: bErrTxt.implicitHeight + Style.padSection
        radius: Math.min(Style.radiusMd, theme.radius)
        color: theme.dangerSurface
        border.width: Style.hairline
        border.color: theme.dangerBorder
        Text {
            id: bErrTxt
            anchors.fill: parent
            anchors.margins: Style.gapMd
            text: app.backupError
            color: theme.dangerText
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
            wrapMode: Text.WordWrap
        }
    }

    // Export selected environment summary
    Rectangle {
        id: exportCard
        Layout.fillWidth: true
        implicitHeight: exportCol.implicitHeight + Style.padCard * 2
        radius: Math.min(Style.radiusMd, theme.radius)
        color: theme.surfaceInput
        border.width: Style.hairline
        border.color: theme.borderSubtle

        readonly property var spaceInfo: (app.backupsVisible && backend.selectedEnvironment && backend.selectedEnvironment.name)
            ? backend.checkExportSpace(backend.selectedEnvironment.name, app.backupIncludeWorkspace)
            : ({ ok: true, estimatedDisplay: "—", availableDisplay: "—", message: "" })

        ColumnLayout {
            id: exportCol
            anchors.fill: parent
            anchors.margins: Style.padCard
            spacing: Style.gapXs

            Text {
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                text: (backend.selectedEnvironment && backend.selectedEnvironment.name)
                      ? qsTr("Export '%1'").arg(backend.selectedEnvironment.name)
                      : qsTr("No environment selected to export")
                color: theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.fontBody
                font.weight: Font.Medium
                elide: Text.ElideMiddle
            }

            Text {
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                text: qsTr("Est. size: %1  •  Free: %2")
                      .arg(exportCard.spaceInfo.estimatedDisplay || "—")
                      .arg(exportCard.spaceInfo.availableDisplay || "—")
                color: exportCard.spaceInfo.ok ? theme.textMuted : theme.dangerText
                font.family: Style.fontMono
                font.pixelSize: Style.fontSmall
                elide: Text.ElideRight
            }
        }
    }

    Text {
        text: qsTr("Saved Backup Archives (%1)").arg(backend.backupArchives.length)
        color: theme.textMuted
        font.family: Style.fontSans
        font.pixelSize: Style.fontSmall
        font.weight: Font.Medium
    }

    Rectangle {
        Layout.fillWidth: true
        Layout.fillHeight: true
        radius: Math.min(Style.radiusMd, theme.radius)
        color: theme.surfaceInput
        border.width: Style.hairline
        border.color: theme.borderSubtle

        Text {
            anchors.centerIn: parent
            visible: backend.backupArchives.length === 0
            text: qsTr("No backup archives found in %1").arg(backend.backupsDirPath)
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontBody
        }

        ListView {
            anchors.fill: parent
            anchors.margins: Style.gapSm
            clip: true
            spacing: Style.gapSm
            model: backend.backupArchives
            delegate: Rectangle {
                required property var modelData
                width: ListView.view.width
                implicitHeight: backupRow.implicitHeight + Style.padCard * 2
                radius: Math.min(Style.radiusSm, theme.radius)
                color: theme.surfaceCard
                border.width: Style.hairline
                border.color: app.importBackupFile === (modelData.filePath || "") ? theme.accentBorder : theme.borderSubtle

                RowLayout {
                    id: backupRow
                    anchors.fill: parent
                    anchors.leftMargin: Style.padCard
                    anchors.rightMargin: Style.padCard
                    spacing: Style.gap

                    ColumnLayout {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        spacing: Style.gapXs

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Style.gapMd

                            Text {
                                Layout.fillWidth: true
                                Layout.minimumWidth: 0
                                text: modelData.fileName || ""
                                color: theme.textPrimary
                                font.family: Style.fontMono
                                font.pixelSize: Style.fontSmall
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
                            color: theme.textMuted
                            font.family: Style.fontMono
                            font.pixelSize: Style.fontCaption
                        }
                    }

                    ActionButton {
                        label: qsTr("Select")
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
        implicitHeight: importCol.implicitHeight + Style.padCard * 2
        radius: Math.min(Style.radiusMd, theme.radius)
        color: theme.surfaceInput
        border.width: Style.hairline
        border.color: theme.borderSubtle

        ColumnLayout {
            id: importCol
            anchors.fill: parent
            anchors.margins: Style.padCard
            spacing: Style.gap

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.gap

                Text {
                    text: qsTr("Import Archive:")
                    color: theme.textPrimary
                    font.family: Style.fontSans
                    font.pixelSize: Style.fontBody
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
                    onClicked: backupBrowseDialog.open()
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.gap

                Text {
                    text: qsTr("New Env Name:")
                    color: theme.textPrimary
                    font.family: Style.fontSans
                    font.pixelSize: Style.fontBody
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

    DialogFooter {
        leftActions: ToggleChip {
            label: qsTr("Include Workspace")
            checked: app.backupIncludeWorkspace
            onToggled: app.backupIncludeWorkspace = !app.backupIncludeWorkspace
        }

        ActionButton {
            label: qsTr("Export Backup")
            variant: "primary"
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
