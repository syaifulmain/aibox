import QtQuick
import QtQuick.Layouts
import "../../components"

// Settings section 4: auto-snapshots, watchdog, idle stop, stale report and
// session recording.
ColumnLayout {
    id: root
    property QtObject app: null

    Layout.fillWidth: true
    spacing: Style.gap

    function reload() {
        app.autoSnapEnabledDraft = backend.autoSnapshotEnabled
        autoSnapKeepField.text = String(backend.autoSnapshotKeep)
        app.watchdogEnabledDraft = backend.watchdogEnabled
        app.watchdogAutoStopDraft = backend.watchdogAutoStop
        app.idleStopMinutesDraft = backend.idleStopMinutes
        app.staleDaysDraft = backend.staleDays
        app.recordSessionsDraft = backend.recordSessions
        watchdogWarnField.text = String(backend.watchdogDiskWarn)
        watchdogCritField.text = String(backend.watchdogDiskCritical)
    }

    Card {
        title: qsTr("Automatic Btrfs Snapshots")
        contentSpacing: Style.gap

        Text {
            Layout.fillWidth: true
            text: qsTr("Creates an auto-<timestamp> snapshot before Enter OS Terminal and Start OS (at most once every 5 minutes). Manual snapshots are never pruned.")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
            wrapMode: Text.WordWrap
        }

        CheckRow {
            Layout.fillWidth: true
            checked: app.autoSnapEnabledDraft
            text: qsTr("Enable auto-snapshots by default")
            onToggled: app.autoSnapEnabledDraft = !app.autoSnapEnabledDraft
        }

        FormRow {
            label: qsTr("Keep last auto-snapshots")

            FlatInput {
                id: autoSnapKeepField
                Layout.preferredWidth: Style.buttonMinWidth
                mono: true
                placeholder: "5"
            }

            Text {
                Layout.fillWidth: true
                text: qsTr("1 to 50 snapshots per environment")
                color: theme.textMuted
                font.family: Style.fontSans
                font.pixelSize: Style.fontSmall
            }
        }
    }

    Card {
        title: qsTr("Disk & Resource Watchdog")
        contentSpacing: Style.gap

        CheckRow {
            Layout.fillWidth: true
            checked: app.watchdogEnabledDraft
            text: qsTr("Send desktop alerts on high disk, CPU, or RAM")
            onToggled: app.watchdogEnabledDraft = !app.watchdogEnabledDraft
        }

        FormRow {
            label: qsTr("Disk warn / critical (%)")

            FlatInput {
                id: watchdogWarnField
                Layout.preferredWidth: Style.buttonMinWidth
                mono: true
                placeholder: "90"
            }

            FlatInput {
                id: watchdogCritField
                Layout.preferredWidth: Style.buttonMinWidth
                mono: true
                placeholder: "97"
            }

            Text {
                Layout.fillWidth: true
                text: qsTr("50-98 warn, > warn critical (5% hysteresis)")
                color: theme.textMuted
                font.family: Style.fontSans
                font.pixelSize: Style.fontSmall
            }
        }

        CheckRow {
            Layout.fillWidth: true
            checked: app.watchdogAutoStopDraft
            text: qsTr("Auto-stop running containers by default on critical disk")
            danger: true
            onToggled: app.watchdogAutoStopDraft = !app.watchdogAutoStopDraft
        }
    }

    Card {
        title: qsTr("Idle Auto-Stop (Opt-in)")
        contentSpacing: Style.gapMd

        Text {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: qsTr("Automatically stops running non-template containers after sustained <3% CPU with no active incus exec session or port forwards (1 min warning notification beforehand). Works only while the AIBox GUI is running — no background daemon is used.")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.gapMd

            Repeater {
                model: backend.idleStopOptions
                delegate: SelectChip {
                    required property int modelData
                    text: modelData === 0 ? qsTr("Off") : qsTr("%1 min").arg(modelData)
                    selected: app.idleStopMinutesDraft === modelData
                    fillColor: theme.surfaceCard
                    onClicked: app.idleStopMinutesDraft = modelData
                }
            }

            Item { Layout.fillWidth: true }
        }
    }

    Card {
        title: qsTr("Stale Environment Report (U)")
        contentSpacing: Style.gapMd

        actions: [
            ActionButton {
                label: qsTr("Open Report")
                onClicked: {
                    app.settingsVisible = false
                    app.openStaleReportDialog()
                }
            }
        ]

        Text {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: qsTr("Flags non-template environments unused for at least staleDays days (plus unknown-age environments without a usage timestamp) and scans orphaned workspaces, snapshot dirs, and unpaired backup archives.")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.gapMd

            Repeater {
                model: backend.staleDayOptions
                delegate: SelectChip {
                    required property int modelData
                    text: qsTr("%1 d").arg(modelData)
                    selected: app.staleDaysDraft === modelData
                    fillColor: theme.surfaceCard
                    onClicked: app.staleDaysDraft = modelData
                }
            }

            Item { Layout.fillWidth: true }
        }
    }

    Card {
        title: qsTr("Session Recording (Opt-in)")
        contentSpacing: Style.gapMd

        actions: [
            ToggleChip {
                label: qsTr("Session recording")
                checked: app.recordSessionsDraft
                compact: true
                onToggled: app.recordSessionsDraft = !app.recordSessionsDraft
            }
        ]

        Text {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: qsTr("Records interactive Enter shells using script -q -f with timing files, automatically scrubs S3 secrets after exit, and retains the last 20 sessions per environment (200 MB total cap). View or replay sessions under History (H) -> Sessions.")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: Style.gap
        Item { Layout.fillWidth: true }

        ActionButton {
            label: qsTr("Save Safety Settings")
            variant: "primary"
            onClicked: {
                const parsedKeep = parseInt(autoSnapKeepField.text.trim(), 10)
                const keep = isNaN(parsedKeep)
                             ? (app.autoSnapEnabledDraft ? 0 : backend.autoSnapshotKeep)
                             : parsedKeep
                const snapErr = backend.saveAutoSnapshotSettings(app.autoSnapEnabledDraft, keep)
                if (snapErr.length > 0) {
                    app.showConfigNotice(snapErr, true)
                    return
                }
                const parsedWarn = parseInt(watchdogWarnField.text.trim(), 10)
                const parsedCrit = parseInt(watchdogCritField.text.trim(), 10)
                const wdErr = backend.saveWatchdogSettings(
                    app.watchdogEnabledDraft,
                    isNaN(parsedWarn)
                    ? (app.watchdogEnabledDraft ? 0 : backend.watchdogDiskWarn)
                    : parsedWarn,
                    isNaN(parsedCrit)
                    ? (app.watchdogEnabledDraft ? 0 : backend.watchdogDiskCritical)
                    : parsedCrit,
                    app.watchdogAutoStopDraft)
                if (wdErr.length > 0) {
                    app.showConfigNotice(wdErr, true)
                    return
                }
                const idleErr = backend.saveIdleStopSettings(app.idleStopMinutesDraft)
                if (idleErr.length > 0) {
                    app.showConfigNotice(idleErr, true)
                    return
                }
                const staleErr = backend.saveStaleSettings(app.staleDaysDraft)
                if (staleErr.length > 0) {
                    app.showConfigNotice(staleErr, true)
                    return
                }
                const recErr = backend.saveSessionRecordingSettings(app.recordSessionsDraft)
                if (recErr.length > 0) {
                    app.showConfigNotice(recErr, true)
                    return
                }
                autoSnapKeepField.text = String(backend.autoSnapshotKeep)
                watchdogWarnField.text = String(backend.watchdogDiskWarn)
                watchdogCritField.text = String(backend.watchdogDiskCritical)
                app.idleStopMinutesDraft = backend.idleStopMinutes
                app.staleDaysDraft = backend.staleDays
                app.recordSessionsDraft = backend.recordSessions
                app.showConfigNotice(qsTr("Safety settings saved."), false)
            }
        }
    }
}
