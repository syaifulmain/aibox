import QtQuick
import QtQuick.Layouts
import "../components"

// Single entry point for maintenance and inspection surfaces that used to sit
// in the top header. Shortcuts still open each tool directly.
ModalDialog {
    id: toolsDialogRoot
    property QtObject app: null
    open: app.toolsVisible
    zOrder: 58
    cardWidth: Math.min(parent.width - Style.modalGutter, 560)
    contentSpacing: Style.space10
    onDismissed: app.toolsVisible = false

    function openTool(action) {
        app.toolsVisible = false
        action()
    }

    ModalHeader {
        title: qsTr("Tools")
        subtitle: qsTr("Maintenance, safety and inspection")
        onClosed: app.toolsVisible = false
    }

    MenuRow {
        label: qsTr("Secrets")
        detail: qsTr("Store API keys and choose which environments receive them")
        disabled: backend.firstRun
        onClicked: toolsDialogRoot.openTool(function () { app.openSecretsDialog() })
    }

    MenuRow {
        label: qsTr("Backups")
        detail: qsTr("Export an environment or import an archive")
        disabled: backend.firstRun
        onClicked: toolsDialogRoot.openTool(function () { app.openBackupsDialog() })
    }

    MenuRow {
        label: qsTr("History")
        detail: qsTr("Operation logs and recorded terminal sessions")
        disabled: backend.firstRun
        onClicked: toolsDialogRoot.openTool(function () { app.openHistoryDialog() })
    }

    MenuRow {
        label: qsTr("Stale & Orphans")
        detail: qsTr("Review unused environments and reclaimable disk artifacts")
        disabled: backend.firstRun
        onClicked: toolsDialogRoot.openTool(function () { app.openStaleReportDialog() })
    }
}
