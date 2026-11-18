import QtQuick
import QtQuick.Layouts
import QtQuick.Dialogs
import "../components"

ModalDialog {
    id: mountsOverlay
    property QtObject app: null
    open: app.mountsVisible
    zOrder: 88
    clipCard: true
    cardWidth: Math.min(parent.width - Style.modalGutter, 760)
    cardHeight: Math.min(parent.height - Style.modalEdge, 560)
    onDismissed: app.mountsVisible = false

    ModalHeader {
        title: qsTr("Extra Host Mounts · %1").arg(backend.selectedEnvironment.name || "")
        subtitle: qsTr("Mounts additional folders under /home/workspace/<name> (Read-Only by default; system/sensitive paths blocked)")
        onClosed: app.mountsVisible = false
    }

    Rectangle {
        visible: app.extraMountError.length > 0
        Layout.fillWidth: true
        height: 36
        radius: Math.min(Style.radius4, theme.radius)
        color: theme.dangerSurface
        border.width: Style.hairline
        border.color: theme.dangerBorder
        Text {
            anchors.fill: parent
            anchors.leftMargin: Style.space12
            anchors.rightMargin: Style.space12
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
            text: app.extraMountError
            color: theme.dangerText
            font.family: Style.fontSans
            font.pixelSize: Style.font11
        }
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: Style.space10

        FlatInput {
            id: extraMountPathInput
            Layout.fillWidth: true
            placeholder: qsTr("Host folder (e.g. ~/datasets or ~/repos/ref)")
        }

        ActionButton {
            label: qsTr("Browse...")
            minWidth: 94
            onClicked: extraMountFolderDialog.open()
        }

        FlatInput {
            id: extraMountNameInput
            Layout.preferredWidth: 145
            placeholder: qsTr("Mount name (e.g. data)")
        }

        ActionButton {
            label: qsTr("+ Attach")
            variant: "primary"
            minWidth: 102
            disabled: backend.busy
            onClicked: {
                const err = backend.addExtraMount(
                    backend.selectedEnvironment.name || "",
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
        }
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: Style.space12

        Rectangle {
            height: Style.controlHeightSm
            width: roModeTxt.implicitWidth + Style.space24
            radius: Math.min(Style.radius4, theme.radius)
            color: app.mountReadOnlyDraft ? theme.surfaceSelected : theme.surfaceInput
            border.width: Style.hairline
            border.color: app.mountReadOnlyDraft ? theme.accentBorder : theme.borderSubtle
            Text {
                id: roModeTxt
                anchors.centerIn: parent
                text: qsTr("Read-Only (Safe)")
                color: app.mountReadOnlyDraft ? theme.textPrimary : theme.textMuted
                font.family: Style.fontSans
                font.pixelSize: Style.font11
            }
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: {
                    app.mountReadOnlyDraft = true
                    app.mountConfirmRwDraft = false
                }
            }
        }

        Rectangle {
            height: Style.controlHeightSm
            width: rwModeTxt.implicitWidth + Style.space24
            radius: Math.min(Style.radius4, theme.radius)
            color: !app.mountReadOnlyDraft ? theme.dangerSurface : theme.surfaceInput
            border.width: Style.hairline
            border.color: !app.mountReadOnlyDraft ? theme.dangerBorder : theme.borderSubtle
            Text {
                id: rwModeTxt
                anchors.centerIn: parent
                text: qsTr("Read-Write")
                color: !app.mountReadOnlyDraft ? theme.dangerText : theme.textMuted
                font.family: Style.fontSans
                font.pixelSize: Style.font11
            }
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: app.mountReadOnlyDraft = false
            }
        }

        Rectangle {
            visible: !app.mountReadOnlyDraft
            Layout.fillWidth: true
            height: Style.controlHeightSm
            radius: Math.min(Style.radius4, theme.radius)
            color: theme.surfaceInput
            border.width: Style.hairline
            border.color: app.mountConfirmRwDraft ? theme.dangerBorder : theme.borderSubtle
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: Style.space10
                anchors.rightMargin: Style.space10
                spacing: Style.space8
                Rectangle {
                    width: 14
                    height: 14
                    radius: Style.radius3
                    color: app.mountConfirmRwDraft ? theme.danger : "transparent"
                    border.width: Style.hairline
                    border.color: app.mountConfirmRwDraft ? theme.danger : theme.textMuted
                    Text {
                        anchors.centerIn: parent
                        visible: app.mountConfirmRwDraft
                        text: "✓"
                        color: theme.onDanger
                        font.family: Style.fontMono
                        font.pixelSize: Style.font10
                    }
                }
                Text {
                    Layout.fillWidth: true
                    elide: Text.ElideRight
                    text: qsTr("Confirm: allow the container to modify files inside this host folder")
                    color: app.mountConfirmRwDraft ? theme.dangerText : theme.textMuted
                    font.family: Style.fontSans
                    font.pixelSize: Style.font11
                }
            }
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: app.mountConfirmRwDraft = !app.mountConfirmRwDraft
            }
        }
    }

    Rectangle {
        Layout.fillWidth: true
        Layout.fillHeight: true
        radius: Math.min(Style.radius4, theme.radius)
        color: theme.surfaceInput
        border.width: Style.hairline
        border.color: theme.borderSubtle

        Text {
            anchors.centerIn: parent
            visible: !backend.selectedEnvironment.extraMounts || backend.selectedEnvironment.extraMounts.length === 0
            text: qsTr("No extra host mounts attached.")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.font12
        }

        ListView {
            id: mountsListView
            anchors.fill: parent
            anchors.margins: Style.space8
            clip: true
            spacing: Style.space6
            model: backend.selectedEnvironment.extraMounts || []
            delegate: Rectangle {
                required property var modelData
                width: mountsListView.width
                height: 48
                radius: Math.min(Style.radius3, theme.radius)
                color: theme.surfaceCard
                border.width: Style.hairline
                border.color: theme.borderSubtle

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Style.space12
                    anchors.rightMargin: Style.space12
                    spacing: Style.space10

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
                        font.pixelSize: Style.font12
                    }

                    ActionButton {
                        minWidth: 84
                        label: qsTr("Remove")
                        danger: true
                        disabled: backend.busy
                        onClicked: backend.removeExtraMount(
                            backend.selectedEnvironment.name || "", modelData.deviceName || "")
                    }
                }
            }
        }
    }

    FolderDialog {
        id: extraMountFolderDialog
        title: qsTr("Choose Extra Host Folder to Mount")
        currentFolder: backend.pathToUrl(backend.workspaceBase)
        onAccepted: extraMountPathInput.text = backend.localPath(selectedFolder)
    }
}
