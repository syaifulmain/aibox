import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts
import "../components"

Rectangle {
    property QtObject app: null
    id: mountsOverlay
    anchors.fill: parent
    visible: app.mountsVisible
    color: "#c406070a"
    z: 88

    MouseArea {
        anchors.fill: parent
        onClicked: app.mountsVisible = false
    }

    Rectangle {
        anchors.centerIn: parent
        width: Math.min(parent.width - 56, 760)
        height: Math.min(parent.height - 56, 560)
        radius: Math.min(8, theme.radius)
        color: app.surfaceCard
        border.width: 1
        border.color: theme.accent
        clip: true

        MouseArea { anchors.fill: parent }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 24
            spacing: 12

            ModalHeader {
                app: mountsOverlay.app
                title: qsTr("Extra Host Mounts · %1").arg(backend.selectedEnvironment.name || "")
                subtitle: qsTr("Mounts additional folders under /home/workspace/<name> (Read-Only by default; system/sensitive paths blocked)")
                onClosed: app.mountsVisible = false
            }

            Rectangle {
                visible: app.extraMountError.length > 0
                Layout.fillWidth: true
                height: 36
                radius: Math.min(4, theme.radius)
                color: "#261319"
                border.width: 1
                border.color: theme.red
                Text {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    verticalAlignment: Text.AlignVCenter
                    elide: Text.ElideRight
                    text: app.extraMountError
                    color: theme.red
                    font.family: app.sansFont
                    font.pixelSize: 11
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 10
                FlatInput {
                    app: mountsOverlay.app
                    id: extraMountPathInput
                    Layout.fillWidth: true
                    placeholder: qsTr("Host folder (e.g. ~/datasets or ~/repos/ref)")
                }
                ActionButton {
                    app: mountsOverlay.app
                    label: qsTr("Browse...")
                    minWidth: 94
                    onClicked: extraMountFolderDialog.open()
                }
                FlatInput {
                    app: mountsOverlay.app
                    id: extraMountNameInput
                    Layout.preferredWidth: 145
                    placeholder: qsTr("Mount name (e.g. data)")
                }
                ActionButton {
                    app: mountsOverlay.app
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
                spacing: 12

                Rectangle {
                    height: 28
                    width: roModeTxt.implicitWidth + 24
                    radius: Math.min(4, theme.radius)
                    color: app.mountReadOnlyDraft ? app.surfaceSelected : app.surfaceInput
                    border.width: 1
                    border.color: app.mountReadOnlyDraft ? theme.accent : app.borderSubtle
                    Text {
                        id: roModeTxt
                        anchors.centerIn: parent
                        text: qsTr("Read-Only (Safe)")
                        color: app.mountReadOnlyDraft ? theme.foreground : theme.muted
                        font.family: app.sansFont
                        font.pixelSize: 11
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
                    height: 28
                    width: rwModeTxt.implicitWidth + 24
                    radius: Math.min(4, theme.radius)
                    color: !app.mountReadOnlyDraft ? "#26181c" : app.surfaceInput
                    border.width: 1
                    border.color: !app.mountReadOnlyDraft ? theme.red : app.borderSubtle
                    Text {
                        id: rwModeTxt
                        anchors.centerIn: parent
                        text: qsTr("Read-Write")
                        color: !app.mountReadOnlyDraft ? theme.red : theme.muted
                        font.family: app.sansFont
                        font.pixelSize: 11
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
                    height: 28
                    radius: Math.min(4, theme.radius)
                    color: app.surfaceInput
                    border.width: 1
                    border.color: app.mountConfirmRwDraft ? theme.red : app.borderSubtle
                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 10
                        anchors.rightMargin: 10
                        spacing: 8
                        Rectangle {
                            width: 14
                            height: 14
                            radius: 3
                            color: app.mountConfirmRwDraft ? theme.red : "transparent"
                            border.width: 1
                            border.color: app.mountConfirmRwDraft ? theme.red : theme.muted
                            Text {
                                anchors.centerIn: parent
                                visible: app.mountConfirmRwDraft
                                text: "✓"
                                color: theme.accentForeground
                                font.family: app.monoFont
                                font.pixelSize: 10
                            }
                        }
                        Text {
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                            text: qsTr("Confirm: allow the container to modify files inside this host folder")
                            color: app.mountConfirmRwDraft ? theme.red : theme.muted
                            font.family: app.sansFont
                            font.pixelSize: 11
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
                radius: Math.min(4, theme.radius)
                color: app.surfaceInput
                border.width: 1
                border.color: app.borderSubtle

                Text {
                    anchors.centerIn: parent
                    visible: !backend.selectedEnvironment.extraMounts || backend.selectedEnvironment.extraMounts.length === 0
                    text: qsTr("No extra host mounts attached.")
                    color: theme.muted
                    font.family: app.sansFont
                    font.pixelSize: 12
                }

                ListView {
                    id: mountsListView
                    anchors.fill: parent
                    anchors.margins: 8
                    clip: true
                    spacing: 6
                    model: backend.selectedEnvironment.extraMounts || []
                    delegate: Rectangle {
                        required property var modelData
                        width: mountsListView.width
                        height: 48
                        radius: Math.min(3, theme.radius)
                        color: app.surfaceCard
                        border.width: 1
                        border.color: app.borderSubtle

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 12
                            spacing: 10

                            Rectangle {
                                height: 18
                                width: roBadgeTxt.implicitWidth + 10
                                radius: 2
                                color: modelData.readonly ? "#162920" : "#2c1419"
                                border.width: 1
                                border.color: modelData.readonly ? theme.green : theme.red
                                Text {
                                    id: roBadgeTxt
                                    anchors.centerIn: parent
                                    text: modelData.readonly ? "RO" : "RW"
                                    color: modelData.readonly ? theme.green : theme.red
                                    font.family: app.monoFont
                                    font.pixelSize: 9
                                    font.weight: Font.Medium
                                }
                            }

                            Text {
                                Layout.fillWidth: true
                                Layout.minimumWidth: 0
                                elide: Text.ElideMiddle
                                text: (modelData.source || "") + "  →  " + (modelData.path || "")
                                color: theme.foreground
                                font.family: app.monoFont
                                font.pixelSize: 12
                            }

                            ActionButton {
                                app: mountsOverlay.app
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
        }
    }

    FolderDialog {
        id: extraMountFolderDialog
        title: qsTr("Choose Extra Host Folder to Mount")
        currentFolder: backend.pathToUrl(backend.workspaceBase)
        onAccepted: extraMountPathInput.text = backend.localPath(selectedFolder)
    }
}
