import QtQuick
import "components"

// Lazy-loads modal dialogs and overlays on demand so startup only parses and
// instantiates the main window until a dialog is opened.
Item {
    id: host
    property QtObject app: null
    anchors.fill: parent

    readonly property Item createDialog: createLoader.item
    readonly property Item settingsPanel: settingsLoader.item

    component DialogLoader: Loader {
        property bool dialogOpen: false
        property string dialogSource: ""
        anchors.fill: parent
        active: dialogOpen && host.app !== null
        z: item ? item.z : 0

        function ensureSource() {
            if (active && host.app !== null && status === Loader.Null)
                setSource(dialogSource, { "app": host.app })
        }

        onActiveChanged: ensureSource()
        Component.onCompleted: ensureSource()
    }

    Connections {
        target: host
        function onAppChanged() {
            for (let i = 0; i < host.children.length; ++i) {
                const child = host.children[i]
                if (child && typeof child.ensureSource === "function")
                    child.ensureSource()
            }
        }
    }

    DialogLoader {
        id: createLoader
        dialogOpen: host.app ? host.app.createVisible : false
        dialogSource: "dialogs/CreateDialog.qml"
    }

    DialogLoader {
        dialogOpen: host.app ? host.app.cloneVisible : false
        dialogSource: "dialogs/CloneDialog.qml"
    }

    DialogLoader {
        dialogOpen: host.app ? host.app.envActionsVisible : false
        dialogSource: "dialogs/EnvironmentActionsDialog.qml"
    }

    DialogLoader {
        dialogOpen: host.app ? host.app.toolsVisible : false
        dialogSource: "dialogs/ToolsDialog.qml"
    }

    DialogLoader {
        dialogOpen: host.app ? host.app.wipeVisible : false
        dialogSource: "dialogs/WipeDialog.qml"
    }

    DialogLoader {
        dialogOpen: host.app ? host.app.stopConfirmVisible : false
        dialogSource: "dialogs/StopDialog.qml"
    }

    DialogLoader {
        dialogOpen: host.app ? host.app.restoreDialogVisible : false
        dialogSource: "dialogs/RestoreDialog.qml"
    }

    DialogLoader {
        dialogOpen: host.app ? host.app.changeReviewVisible : false
        dialogSource: "dialogs/ChangeReviewDialog.qml"
    }

    DialogLoader {
        dialogOpen: host.app ? host.app.helpVisible : false
        dialogSource: "dialogs/HelpOverlay.qml"
    }

    DialogLoader {
        id: settingsLoader
        dialogOpen: host.app ? host.app.settingsVisible : false
        dialogSource: "panels/SettingsPanel.qml"
    }

    DialogLoader {
        dialogOpen: backend.firstRun
        dialogSource: "dialogs/FirstRunDialog.qml"
        z: 100
    }

    DialogLoader {
        dialogOpen: host.app ? host.app.historyVisible : false
        dialogSource: "dialogs/HistoryDialog.qml"
    }

    DialogLoader {
        dialogOpen: host.app ? host.app.egressVisible : false
        dialogSource: "dialogs/EgressDialog.qml"
    }

    DialogLoader {
        dialogOpen: host.app ? host.app.portsVisible : false
        dialogSource: "dialogs/PortsDialog.qml"
    }

    DialogLoader {
        dialogOpen: host.app ? host.app.mountsVisible : false
        dialogSource: "dialogs/MountsDialog.qml"
    }

    DialogLoader {
        dialogOpen: host.app ? host.app.editQuotasVisible : false
        dialogSource: "dialogs/EditQuotasDialog.qml"
    }

    DialogLoader {
        dialogOpen: host.app ? host.app.backupsVisible : false
        dialogSource: "dialogs/BackupsDialog.qml"
    }

    DialogLoader {
        dialogOpen: host.app ? host.app.secretsVisible : false
        dialogSource: "dialogs/SecretsDialog.qml"
    }

    DialogLoader {
        dialogOpen: host.app ? host.app.staleReportVisible : false
        dialogSource: "dialogs/StaleReportDialog.qml"
    }

    ThemeDebugOverlay {
        app: host.app
        targetRoot: host.app ? host.app.contentItem : null
        active: host.app ? host.app.themeDebugVisible : false
    }
}
