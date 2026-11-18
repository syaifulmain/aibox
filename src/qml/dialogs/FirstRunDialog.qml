import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import "../components"

// First-run modal: lets the user choose the AIBox system storage location.
Item {
    id: root
    property QtObject app: null
    anchors.fill: parent

    function submit() {
        const error = backend.completeFirstRun(firstRunPathField.text)
        if (error.length > 0) {
            root.app.firstRunError = error
        } else {
            root.app.firstRunError = ""
            firstRunPathField.text = backend.systemPath
            root.app.onFirstRunCompleted()
        }
    }

    ModalDialog {
        id: firstRunOverlay
        open: backend.firstRun
        scrimColor: theme.scrimStrong
        zOrder: 100
        cardWidth: Math.min(parent.width - Style.modalGutter, Style.modalWidthMd)
        contentSpacing: Style.padCard

        readonly property string resolvedPreview: {
            const raw = firstRunPathField.text.trim()
            const base = raw.length > 0 ? raw : backend.defaultSystemPath
            return base.length > 1 && base.endsWith("/") ? base.slice(0, -1) : base
        }

        onVisibleChanged: {
            if (visible) {
                root.app.firstRunError = ""
                firstRunPathField.text = backend.systemPath.length > 0 ? backend.systemPath : backend.defaultSystemPath
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.padCard

            BrandMark {
                markSize: Style.controlHeight
                markRadius: Style.radiusMd
                textSize: Style.fontTitle
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                spacing: Style.gapXs

                Text {
                    Layout.fillWidth: true
                    text: qsTr("Welcome to AIBox")
                    color: theme.textPrimary
                    font.family: Style.fontSans
                    font.pixelSize: Style.fontDisplay
                    font.weight: Font.Medium
                }

                Text {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    text: qsTr("Choose where AIBox stores its system data, shared workspaces, OS images, and backups. You can keep the default or pick a custom location.")
                    color: theme.textSecondary
                    font.family: Style.fontSans
                    font.pixelSize: Style.fontSmall
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            height: Style.hairline
            color: theme.borderSubtle
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: Style.gapMd

            Text {
                Layout.fillWidth: true
                text: qsTr("System Storage Location")
                color: theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.fontBody
                font.weight: Font.Medium
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.gap

                FlatInput {
                    id: firstRunPathField
                    Layout.fillWidth: true
                    text: backend.systemPath.length > 0 ? backend.systemPath : backend.defaultSystemPath
                    placeholder: backend.defaultSystemPath
                    onAccepted: root.submit()
                }

                ActionButton {
                    label: qsTr("Browse...")
                    onClicked: firstRunFolderDialog.open()
                }

                ActionButton {
                    label: qsTr("Default")
                    onClicked: firstRunPathField.text = backend.defaultSystemPath
                }
            }

            Rectangle {
                Layout.fillWidth: true
                implicitHeight: previewGrid.implicitHeight + Style.padCard * 2
                radius: Math.min(Style.radiusMd, theme.radius)
                color: theme.surfaceInput
                border.width: Style.hairline
                border.color: theme.borderSubtle

                ColumnLayout {
                    id: previewGrid
                    anchors.fill: parent
                    anchors.margins: Style.padCard
                    spacing: Style.gapSm

                    Text {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        elide: Text.ElideMiddle
                        text: qsTr("Workspaces:  %1/workspaces").arg(firstRunOverlay.resolvedPreview)
                        color: theme.textMuted
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontCaption
                    }

                    Text {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        elide: Text.ElideMiddle
                        text: qsTr("OS Images:   %1/os-images").arg(firstRunOverlay.resolvedPreview)
                        color: theme.textMuted
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontCaption
                    }

                    Text {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        elide: Text.ElideMiddle
                        text: qsTr("State & Cfg: %1/state, config.json").arg(firstRunOverlay.resolvedPreview)
                        color: theme.textMuted
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontCaption
                    }
                }
            }
        }

        Rectangle {
            visible: root.app && root.app.firstRunError.length > 0
            Layout.fillWidth: true
            implicitHeight: firstRunErrTxt.implicitHeight + Style.padCard * 2
            radius: Math.min(Style.radiusMd, theme.radius)
            color: theme.dangerSurface
            border.width: Style.hairline
            border.color: theme.dangerBorder

            Text {
                id: firstRunErrTxt
                anchors.fill: parent
                anchors.margins: Style.padCard
                wrapMode: Text.WordWrap
                text: root.app ? root.app.firstRunError : ""
                color: theme.dangerText
                font.family: Style.fontSans
                font.pixelSize: Style.fontSmall
            }
        }

        DialogFooter {
            ActionButton {
                label: qsTr("Continue")
                variant: "primary"
                onClicked: root.submit()
            }
        }
    }

    FolderDialog {
        id: firstRunFolderDialog
        title: qsTr("Choose AIBox System Storage Location")
        currentFolder: backend.pathToUrl(firstRunPathField.text.trim().length > 0 ? firstRunPathField.text.trim() : backend.defaultSystemPath)
        onAccepted: firstRunPathField.text = backend.localPath(selectedFolder)
    }
}
