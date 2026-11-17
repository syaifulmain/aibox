import QtQuick
import QtQuick.Layouts
import "../components"

ModalDialog {
    id: cloneDialogRoot
    property QtObject app: null
    open: app.cloneVisible
    cardWidth: Math.min(parent.width - Style.modalGutterWide, 520)
    contentSpacing: Style.space14
    onDismissed: {
        if (!backend.busy)
            app.cloneVisible = false
    }

    ModalHeader {
        title: qsTr("Clone Environment '%1'").arg(app.cloneSourceName)
        subtitle: qsTr("Duplicates the OS container with Btrfs copy-on-write and creates a separate shared workspace folder.")
        onClosed: app.cloneVisible = false
    }

    ColumnLayout {
        Layout.fillWidth: true
        spacing: Style.space6

        Text {
            Layout.fillWidth: true
            text: qsTr("New Environment Name")
            color: theme.foreground
            font.family: Style.fontSans
            font.pixelSize: Style.font12
            font.weight: Font.Medium
        }

        FlatInput {
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
            font.family: Style.fontMono
            font.pixelSize: Style.font10
            elide: Text.ElideMiddle
        }
    }

    RowLayout {
        Layout.fillWidth: true
        Layout.topMargin: Style.space4
        spacing: Style.space10
        Item { Layout.fillWidth: true }

        ActionButton {
            label: qsTr("Cancel")
            minWidth: 96
            onClicked: app.cloneVisible = false
        }

        ActionButton {
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
