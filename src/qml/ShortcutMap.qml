import QtQuick

// Global application keyboard shortcuts bound to the window state (`app`).
Item {
    id: shortcutMap
    property QtObject app: null
    property Item mainContent: null
    readonly property bool selectedIsTemplate: backend.selectedEnvironment.isTemplate || false
    readonly property bool textInputFocused: shortcutMap.app && (shortcutMap.app.activeFocusItem instanceof TextInput)

    Shortcut {
        sequence: "?"
        context: Qt.ApplicationShortcut
        onActivated: shortcutMap.app.helpVisible = !shortcutMap.app.helpVisible
    }
    Shortcut {
        sequence: "Ctrl+Shift+T"
        context: Qt.ApplicationShortcut
        onActivated: shortcutMap.app.themeDebugVisible = !shortcutMap.app.themeDebugVisible
    }

    function handleEscape() {
        const app = shortcutMap.app
        if (backend.firstRun) {
            return
        } else if (app.helpVisible) {
            app.helpVisible = false
        } else if (app.toolsVisible) {
            app.toolsVisible = false
        } else if (app.envActionsVisible) {
            app.envActionsVisible = false
        } else if (app.portsVisible) {
            app.portsVisible = false
        } else if (app.mountsVisible) {
            app.mountsVisible = false
        } else if (app.editQuotasVisible) {
            app.editQuotasVisible = false
        } else if (app.backupsVisible) {
            app.backupsVisible = false
        } else if (app.egressVisible) {
            app.egressVisible = false
        } else if (app.historyVisible) {
            app.historyVisible = false
        } else if (app.settingsVisible) {
            if (app.settingsSection === 2 && app.osEditorVisible)
                app.osEditorVisible = false
            else if (app.settingsSection === 1 && app.poolResizeConfirmVisible)
                app.poolResizeConfirmVisible = false
            else
                app.settingsVisible = false
        } else if (app.createVisible) {
            app.createVisible = false
        } else if (app.cloneVisible) {
            app.cloneVisible = false
        } else if (app.wipeVisible) {
            app.wipeVisible = false
        } else if (app.stopConfirmVisible) {
            app.stopConfirmVisible = false
        } else if (app.restoreDialogVisible) {
            app.restoreDialogVisible = false
        } else if (app.changeReviewVisible) {
            app.changeReviewVisible = false
        } else if (app.secretsVisible) {
            app.secretsVisible = false
        } else if (app.staleReportVisible) {
            if (app.staleConfirmAction.length > 0)
                app.staleConfirmAction = ""
            else
                app.staleReportVisible = false
        } else {
            backend.clearError()
        }
    }

    Shortcut {
        sequence: "Escape"
        context: Qt.ApplicationShortcut
        onActivated: shortcutMap.handleEscape()
    }
    Shortcut {
        sequence: "A"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && !backend.firstRun && !shortcutMap.textInputFocused
        onActivated: shortcutMap.app.openSecretsDialog()
    }
    Shortcut {
        sequence: "U"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && !backend.firstRun && !shortcutMap.textInputFocused
        onActivated: shortcutMap.app.openStaleReportDialog()
    }
    Shortcut {
        sequence: "H"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && !shortcutMap.textInputFocused
        onActivated: shortcutMap.app.openHistoryDialog()
    }
    Shortcut {
        sequence: "B"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && !backend.firstRun && !shortcutMap.textInputFocused
        onActivated: shortcutMap.app.openBackupsDialog()
    }
    Shortcut {
        sequence: "V"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && shortcutMap.app.activeTab === 0 && backend.count > 0 && !shortcutMap.textInputFocused
        onActivated: shortcutMap.app.openChangeReviewDialog("")
    }
    Shortcut {
        sequence: "E"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && shortcutMap.app.activeTab === 0 && !backend.busy && backend.count > 0 && !shortcutMap.textInputFocused
        onActivated: shortcutMap.app.openEditQuotasDialog()
    }
    Shortcut {
        sequence: "3"
        context: Qt.ApplicationShortcut
        enabled: !backend.firstRun && !backend.busy && !shortcutMap.textInputFocused && (!shortcutMap.app.anyModalOpen || shortcutMap.app.settingsVisible)
        onActivated: {
            if (shortcutMap.app.settingsVisible)
                shortcutMap.app.settingsVisible = false
            else
                shortcutMap.app.openSettings(0)
        }
    }
    Shortcut {
        sequence: "Q"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && !backend.busy && !shortcutMap.textInputFocused
        onActivated: Qt.quit()
    }
    Shortcut {
        sequence: "N"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && !backend.busy && !shortcutMap.textInputFocused
        onActivated: shortcutMap.app.openCreateDialog("")
    }
    Shortcut {
        sequence: "1"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && !shortcutMap.textInputFocused
        onActivated: shortcutMap.app.activeTab = 0
    }
    Shortcut {
        sequence: "2"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && !shortcutMap.textInputFocused
        onActivated: shortcutMap.app.activeTab = 1
    }
    Shortcut {
        sequence: "R"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && !backend.busy && !shortcutMap.textInputFocused
        onActivated: backend.refresh()
    }
    Shortcut {
        sequence: "Space"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && shortcutMap.app.activeTab === 0 && !backend.busy && backend.count > 0 && !shortcutMap.textInputFocused
        onActivated: {
            const env = backend.selectedEnvironment
            if (env.isTemplate)
                shortcutMap.app.openCreateDialog("", env.name || "")
            else
                backend.enterEnvironment(env.name || "")
        }
    }
    Shortcut {
        sequence: "Return"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && shortcutMap.app.activeTab === 0 && !backend.busy && backend.count > 0 && !shortcutMap.textInputFocused
        onActivated: {
            const env = backend.selectedEnvironment
            if (env.isTemplate)
                shortcutMap.app.openCreateDialog("", env.name || "")
            else
                backend.enterEnvironment(env.name || "")
        }
    }
    Shortcut {
        sequence: "F"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && shortcutMap.app.activeTab === 0 && backend.count > 0 && !shortcutMap.textInputFocused
        onActivated: backend.openWorkspaceFolder(backend.selectedEnvironment.name || "")
    }
    Shortcut {
        sequence: "S"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && shortcutMap.app.activeTab === 0 && !backend.busy && backend.count > 0 && !shortcutMap.selectedIsTemplate && !shortcutMap.textInputFocused
        onActivated: shortcutMap.app.requestStartStop()
    }
    Shortcut {
        sequence: "P"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && shortcutMap.app.activeTab === 0 && !backend.busy && backend.count > 0 && !shortcutMap.textInputFocused
        onActivated: {
            backend.createSnapshot(backend.selectedEnvironment.name || "", shortcutMap.mainContent.snapshotNameText)
            shortcutMap.mainContent.clearSnapshotName()
        }
    }
    Shortcut {
        sequence: "C"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && shortcutMap.app.activeTab === 0 && backend.count > 0 && !shortcutMap.selectedIsTemplate && !shortcutMap.textInputFocused
        onActivated: backend.copyAiCommand(backend.selectedEnvironment.name || "")
    }
    Shortcut {
        sequence: "I"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && shortcutMap.app.activeTab === 0 && backend.count > 0 && !shortcutMap.selectedIsTemplate && !shortcutMap.textInputFocused
        onActivated: backend.pasteImageToWorkspace(backend.selectedEnvironment.name || "")
    }
    Shortcut {
        sequence: "D"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && shortcutMap.app.activeTab === 0 && !backend.busy && backend.count > 0 && !shortcutMap.textInputFocused
        onActivated: shortcutMap.app.openCloneDialog(backend.selectedEnvironment.name || "")
    }
    Shortcut {
        sequence: "T"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && shortcutMap.app.activeTab === 0 && !backend.busy && backend.count > 0 && !shortcutMap.textInputFocused
        onActivated: {
            const env = backend.selectedEnvironment
            backend.setEnvironmentTemplate(env.name || "", !(env.isTemplate || false))
        }
    }
    Shortcut {
        sequence: "K"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && shortcutMap.app.activeTab === 0 && !backend.busy && backend.count > 0 && !shortcutMap.selectedIsTemplate && !shortcutMap.textInputFocused
        onActivated: {
            const env = backend.selectedEnvironment
            const nextMode = (env.networkMode === "offline") ? "online" : "offline"
            backend.setNetworkMode(env.name || "", nextMode, "")
        }
    }
    Shortcut {
        sequence: "Delete"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && shortcutMap.app.activeTab === 0 && !backend.busy && backend.count > 0 && !shortcutMap.textInputFocused
        onActivated: {
            shortcutMap.app.deleteWorkspaceOnWipe = false
            shortcutMap.app.wipeVisible = true
        }
    }
    Shortcut {
        sequence: "Up"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && shortcutMap.app.activeTab === 0 && backend.count > 0 && !shortcutMap.textInputFocused
        onActivated: backend.selectedIndex = Math.max(0, backend.selectedIndex - 1)
    }
    Shortcut {
        sequence: "Down"
        context: Qt.ApplicationShortcut
        enabled: !shortcutMap.app.anyModalOpen && shortcutMap.app.activeTab === 0 && backend.count > 0 && !shortcutMap.textInputFocused
        onActivated: backend.selectedIndex = Math.min(backend.count - 1, backend.selectedIndex + 1)
    }
}
