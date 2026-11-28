import QtQuick
import QtQuick.Layouts
import QtQuick.Dialogs
import "../components"

ModalDialog {
    id: mountsOverlay
    property QtObject app: null
    readonly property var env: backend.selectedEnvironment
    open: app.mountsVisible
    zOrder: 88
    clipCard: true
    size: "md"
    cardHeight: Math.min(parent.height - Style.modalGutter, Style.windowMinHeight)
    onDismissed: app.mountsVisible = false

    function submitMount() {
        const err = backend.addExtraMount(
            mountsOverlay.env.name || "",
            extraMountPathInput.text,
            extraMountNameInput.text,
            app.mountReadOnlyDraft,
            app.mountConfirmRwDraft)
        if (err.length > 0) {
            app.extraMountError = err
        } else {
            app.extraMountError = ""
            extraMountPathInput.text = ""
            extraMountNameInput.text = ""
            app.mountReadOnlyDraft = true
            app.mountConfirmRwDraft = false
        }
    }

    ModalHeader {
        title: qsTr("Extra Host Mounts · %1").arg(mountsOverlay.env.name || "")
        subtitle: qsTr("Mounts additional folders under /home/workspace/<name> (Read-Only by default; system/sensitive paths blocked)")
        onClosed: app.mountsVisible = false
    }

    Rectangle {
        visible: app.extraMountError.length > 0
        Layout.fillWidth: true
        implicitHeight: mountErrTxt.implicitHeight + Style.padCard * 2
        radius: Math.min(Style.radiusMd, theme.radius)
        color: theme.dangerSurface
        border.width: Style.hairline
        border.color: theme.dangerBorder
        Text {
            id: mountErrTxt
            anchors.fill: parent
            anchors.margins: Style.padCard
            wrapMode: Text.WordWrap
            text: app.extraMountError
            color: theme.dangerText
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }
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
            visible: mountsListView.count === 0
            text: qsTr("No extra host mounts attached.")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontBody
        }

        ListView {
            id: mountsListView
            anchors.fill: parent
            anchors.margins: Style.gapMd
            clip: true
            spacing: Style.gapSm
            model: mountsOverlay.env.extraMounts || []
            delegate: Rectangle {
                required property var modelData
                width: mountsListView.width
                implicitHeight: mountRow.implicitHeight + Style.padCard * 2
                radius: Math.min(Style.radiusSm, theme.radius)
                color: theme.surfaceCard
                border.width: Style.hairline
                border.color: theme.borderSubtle

                RowLayout {
                    id: mountRow
                    anchors.fill: parent
                    anchors.leftMargin: Style.padCard
                    anchors.rightMargin: Style.padCard
                    spacing: Style.gap

                    TagBadge {
                        text: modelData.readonly ? "RO" : "RW"
                        tone: modelData.readonly ? "success" : "danger"
                        borderColor: modelData.readonly ? theme.successBorder : theme.dangerBorder
                        textWeight: Font.Medium
                    }

                    Text {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        elide: Text.ElideMiddle
                        text: (modelData.source || "") + "  →  " + (modelData.path || "")
                        color: theme.textPrimary
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontBody
                    }

                    ActionButton {
                        label: qsTr("Remove")
                        variant: "danger"
                        disabled: backend.busy
                        onClicked: backend.removeExtraMount(
                            mountsOverlay.env.name || "", modelData.deviceName || "")
                    }
                }
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: Style.gap

        FlatInput {
            id: extraMountPathInput
            Layout.fillWidth: true
            placeholder: qsTr("Host folder (e.g. ~/datasets or ~/repos/ref)")
        }

        ActionButton {
            label: qsTr("Browse...")
            onClicked: extraMountFolderDialog.open()
        }

        FlatInput {
            id: extraMountNameInput
            Layout.preferredWidth: 145
            placeholder: qsTr("Mount name (e.g. data)")
        }
    }

    CheckRow {
        visible: !app.mountReadOnlyDraft
        checked: app.mountConfirmRwDraft
        danger: true
        rowHeight: Style.controlHeightCompact
        text: qsTr("Confirm: allow the container to modify files inside this host folder")
        onToggled: app.mountConfirmRwDraft = !app.mountConfirmRwDraft
    }

    DialogFooter {
        leftActions: [
            SelectChip {
                text: qsTr("Read-Only (Safe)")
                selected: app.mountReadOnlyDraft
                mono: false
                selectedTextColor: theme.textPrimary
                onClicked: {
                    app.mountReadOnlyDraft = true
                    app.mountConfirmRwDraft = false
                }
            },
            SelectChip {
                text: qsTr("Read-Write")
                selected: !app.mountReadOnlyDraft
                mono: false
                selectedFill: theme.dangerSurface
                selectedBorderColor: theme.dangerBorder
                selectedTextColor: theme.dangerText
                onClicked: app.mountReadOnlyDraft = false
            }
        ]

        ActionButton {
            label: qsTr("Add Mount")
            variant: "primary"
            disabled: backend.busy
            onClicked: mountsOverlay.submitMount()
        }
    }

    FolderDialog {
        id: extraMountFolderDialog
        title: qsTr("Choose Extra Host Folder to Mount")
        currentFolder: backend.pathToUrl(backend.workspaceBase)
        onAccepted: extraMountPathInput.text = backend.localPath(selectedFolder)
    }
}
