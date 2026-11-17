import QtQuick
import QtQuick.Layouts
import "../components"

// Secondary environment actions. Primary actions stay on the inspector.
ModalDialog {
    id: actionsDialogRoot
    property QtObject app: null
    open: app.envActionsVisible
    zOrder: 57
    cardWidth: Math.min(parent.width - Style.modalGutter, 560)
    contentSpacing: Style.space10
    onDismissed: app.envActionsVisible = false

    readonly property var env: backend.selectedEnvironment || ({})
    readonly property bool isTemplate: env.isTemplate || false

    function openAction(action) {
        app.envActionsVisible = false
        action()
    }

    ModalHeader {
        title: qsTr("Environment Actions")
        subtitle: env.name || ""
        subtitleMono: true
        onClosed: app.envActionsVisible = false
    }

    MenuRow {
        label: qsTr("Secrets")
        detail: qsTr("Use global secrets and add secrets that only this environment receives")
        disabled: backend.busy
        onClicked: actionsDialogRoot.openAction(function () {
            app.openEnvironmentSecretsDialog(actionsDialogRoot.env.name || "")
        })
    }

    MenuRow {
        label: qsTr("Clone")
        detail: qsTr("Create an isolated copy with its own workspace")
        disabled: backend.busy
        onClicked: actionsDialogRoot.openAction(function () {
            app.openCloneDialog(actionsDialogRoot.env.name || "")
        })
    }

    MenuRow {
        label: qsTr("Quotas & Limits")
        detail: qsTr("Edit CPU, memory, disk, process and I/O limits")
        disabled: backend.busy
        onClicked: actionsDialogRoot.openAction(function () { app.openEditQuotasDialog() })
    }

    MenuRow {
        label: actionsDialogRoot.isTemplate ? qsTr("Unmark Template") : qsTr("Mark as Template")
        detail: qsTr("Reuse this environment as a starting point for new ones")
        disabled: backend.busy
        onClicked: actionsDialogRoot.openAction(function () {
            backend.setEnvironmentTemplate(actionsDialogRoot.env.name || "",
                                           !actionsDialogRoot.isTemplate)
        })
    }

    MenuRow {
        label: qsTr("Copy AI Command")
        detail: qsTr("Copy the command that enters this environment from a terminal")
        disabled: actionsDialogRoot.isTemplate
        onClicked: actionsDialogRoot.openAction(function () {
            backend.copyAiCommand(actionsDialogRoot.env.name || "")
        })
    }

    MenuRow {
        label: qsTr("Paste Image")
        detail: qsTr("Save the clipboard image to the workspace and copy its path")
        disabled: actionsDialogRoot.isTemplate
        onClicked: actionsDialogRoot.openAction(function () {
            backend.pasteImageToWorkspace(actionsDialogRoot.env.name || "")
        })
    }

    // Destructive action stays last so it is never hit by accident.
    MenuRow {
        label: qsTr("Wipe OS")
        detail: qsTr("Destroy this environment and free its workspace")
        danger: true
        disabled: backend.busy
        onClicked: actionsDialogRoot.openAction(function () {
            app.deleteWorkspaceOnWipe = false
            app.wipeVisible = true
        })
    }
}
