import QtQuick
import QtQuick.Layouts
import "panels"

AppState {
    id: win
    dialogHostRef: dialogHost

    ShortcutMap {
        app: win
        mainContent: mainContent
    }

    Connections {
        target: backend
        function onEnvironmentCreated(name) {
            win.createVisible = false
            win.cloneVisible = false
            win.activeTab = 0
        }
        function onSelectedIndexChanged() {
            win.syncSelectedNetworkDraft()
        }
        function onEnvironmentsChanged() {
            if (!win.netAllowlistEditing || win.netAllowlistDraft.length === 0)
                win.syncSelectedNetworkDraft()
        }
        function onAppearanceSettingsChanged() {
            theme.setPreferences(
                backend.themeModeOverride,
                backend.highContrast,
                backend.reduceMotion
            )
        }
    }

    Component.onCompleted: {
        win.syncSelectedNetworkDraft()
        if (backend.busy && backend.pendingEnvName.length > 0) {
            win.newEnvName = backend.pendingEnvName
            win.createVisible = true
        }
        theme.setPreferences(
            backend.themeModeOverride,
            backend.highContrast,
            backend.reduceMotion
        )
    }

    Item {
        anchors.fill: parent
        LayoutMirroring.enabled: Qt.application.layoutDirection === Qt.RightToLeft
        LayoutMirroring.childrenInherit: true

        ColumnLayout {
            anchors.fill: parent
            spacing: 0

            AppHeader {
                app: win
            }

            StatusBanner {
                app: win
            }

            MainContent {
                id: mainContent
                app: win
            }
        }

        DialogHost {
            id: dialogHost
            app: win
        }
    }
}
