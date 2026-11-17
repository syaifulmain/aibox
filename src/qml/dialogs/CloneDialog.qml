import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts
import "../components"

Rectangle {
    id: cloneDialogRoot
    property QtObject app: null
    anchors.fill: parent
    visible: app.cloneVisible
    color: "#c406070a"

    MouseArea {
        anchors.fill: parent
        onClicked: {
            if (!backend.busy)
                app.cloneVisible = false
        }
    }

    Rectangle {
        anchors.centerIn: parent
        width: Math.min(parent.width - 60, 520)
        height: cloneCol.implicitHeight + 48
        radius: Math.min(8, theme.radius)
        color: app.surfaceCard
        border.width: 1
        border.color: theme.accent

        MouseArea { anchors.fill: parent }

        ColumnLayout {
            id: cloneCol
            anchors.fill: parent
            anchors.margins: 24
            spacing: 14

            ModalHeader {
                app: cloneDialogRoot.app
                title: qsTr("Clone Environment '%1'").arg(app.cloneSourceName)
                subtitle: qsTr("Duplicates the OS container with Btrfs copy-on-write and creates a separate shared workspace folder.")
                onClosed: app.cloneVisible = false
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 6

                Text {
                    Layout.fillWidth: true
                    text: qsTr("New Environment Name")
                    color: theme.foreground
                    font.family: app.sansFont
                    font.pixelSize: 12
                    font.weight: Font.Medium
                }

                FlatInput {
                    app: cloneDialogRoot.app
                    id: cloneNameField
                    Layout.fillWidth: true
                    placeholder: qsTr("e.g. agent-sandbox-2")
                    text: app.cloneNewName
                    onTextChanged: app.cloneNewName = text
                    onAccepted: {
                        if (!backend.busy && app.cloneNewName.trim().length > 0) {
                            backend.cloneEnvironment(app.cloneSourceName, app.cloneNewName)
                            app.cloneVisible = false
                        }
                    }
                }

                Text {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    text: qsTr("Shared at: %1/%2").arg(backend.workspaceBase).arg(app.cloneNewName.length > 0 ? app.cloneNewName : "<name>")
                    color: theme.muted
                    font.family: app.monoFont
                    font.pixelSize: 10
                    elide: Text.ElideMiddle
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: 4
                spacing: 10
                Item { Layout.fillWidth: true }

                ActionButton {
                    app: cloneDialogRoot.app
                    label: qsTr("Cancel")
                    minWidth: 96
                    onClicked: app.cloneVisible = false
                }

                ActionButton {
                    app: cloneDialogRoot.app
                    label: qsTr("Clone Environment")
                    variant: "primary"
                    minWidth: 165
                    disabled: backend.busy || app.cloneNewName.trim().length === 0
                    onClicked: {
                        backend.cloneEnvironment(app.cloneSourceName, app.cloneNewName)
                        app.cloneVisible = false
                    }
                }
            }
        }
    }
}
