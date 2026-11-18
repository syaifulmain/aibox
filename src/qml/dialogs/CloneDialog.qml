import QtQuick
import QtQuick.Layouts
import "../components"

ModalDialog {
    id: cloneDialogRoot
    property QtObject app: null
    open: app.cloneVisible
    size: "sm"
    onDismissed: {
        if (!backend.busy)
            app.cloneVisible = false
    }

    ModalHeader {
        title: qsTr("Clone Environment '%1'").arg(app.cloneSourceName)
        subtitle: qsTr("Duplicates the OS container with Btrfs copy-on-write and creates a separate shared workspace folder.")
        onClosed: app.cloneVisible = false
    }

    FormRow {
        label: qsTr("New Environment Name")
        hint: qsTr("Shared at: %1/%2").arg(backend.workspaceBase).arg(app.cloneNewName.length > 0 ? app.cloneNewName : "<name>")

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
    }

    DialogFooter {
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
