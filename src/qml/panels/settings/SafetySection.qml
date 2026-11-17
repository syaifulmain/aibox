import QtQuick
import QtQuick.Layouts
import "../../components"

// Settings section 4: auto-snapshots, watchdog, idle stop, stale report and
// session recording.
ColumnLayout {
    id: root
    property QtObject app: null

    Layout.fillWidth: true
    spacing: Style.space12

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

    // Automatic snapshots
    Rectangle {
        Layout.fillWidth: true
        Layout.preferredHeight: safetyCardCol.implicitHeight + Style.cardPadding * 2
        radius: Math.min(Style.radius6, theme.radius)
        color: Style.surfaceInput
        border.width: Style.hairline
        border.color: Style.borderSubtle

        ColumnLayout {
            id: safetyCardCol
            anchors.fill: parent
            anchors.margins: Style.cardPadding
            spacing: Style.space10

            Text {
                text: qsTr("Automatic Btrfs Snapshots")
                color: theme.foreground
                font.family: Style.fontSans
                font.pixelSize: Style.font12
                font.weight: Font.Medium
            }

            Text {
                Layout.fillWidth: true
                text: qsTr("Creates an auto-<timestamp> snapshot before Enter OS Terminal and Start OS (at most once every 5 minutes). Manual snapshots are never pruned.")
                color: theme.muted
                font.family: Style.fontSans
                font.pixelSize: Style.font10
                wrapMode: Text.WordWrap
            }

            CheckRow {
                Layout.fillWidth: true
                checked: app.autoSnapEnabledDraft
                text: qsTr("Enable auto-snapshots by default")
                onToggled: app.autoSnapEnabledDraft = !app.autoSnapEnabledDraft
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space10

                Text {
                    Layout.preferredWidth: 180
                    text: qsTr("Keep last auto-snapshots")
                    color: theme.foreground
                    font.family: Style.fontSans
                    font.pixelSize: Style.font11
                }
                FlatInput {
                    id: autoSnapKeepField
                    Layout.preferredWidth: 100
                    placeholder: "5"
                }
                Text {
                    Layout.fillWidth: true
                    text: qsTr("1 to 50 snapshots per environment")
                    color: theme.muted
                    font.family: Style.fontSans
                    font.pixelSize: Style.font10
                }
            }
        }
    }

    // Disk and resource watchdog
    Rectangle {
        Layout.fillWidth: true
        Layout.preferredHeight: watchdogCardCol.implicitHeight + Style.cardPadding * 2
        radius: Math.min(Style.radius6, theme.radius)
        color: Style.surfaceInput
        border.width: Style.hairline
        border.color: Style.borderSubtle

        ColumnLayout {
            id: watchdogCardCol
            anchors.fill: parent
            anchors.margins: Style.cardPadding
            spacing: Style.space10

            Text {
                text: qsTr("Disk & Resource Watchdog")
                color: theme.foreground
                font.family: Style.fontSans
                font.pixelSize: Style.font12
                font.weight: Font.Medium
            }

            CheckRow {
                Layout.fillWidth: true
                checked: app.watchdogEnabledDraft
                text: qsTr("Send desktop alerts on high disk, CPU, or RAM")
                rowHeight: 36
                onToggled: app.watchdogEnabledDraft = !app.watchdogEnabledDraft
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space10

                Text {
                    Layout.preferredWidth: 180
                    text: qsTr("Disk warn / critical (%)")
                    color: theme.foreground
                    font.family: Style.fontSans
                    font.pixelSize: Style.font11
                }
                FlatInput {
                    id: watchdogWarnField
                    Layout.preferredWidth: 72
                    placeholder: "90"
                }
                FlatInput {
                    id: watchdogCritField
                    Layout.preferredWidth: 72
                    placeholder: "97"
                }
                Text {
                    Layout.fillWidth: true
                    text: qsTr("50-98 warn, > warn critical (5% hysteresis)")
                    color: theme.muted
                    font.family: Style.fontSans
                    font.pixelSize: Style.font10
                }
            }

            CheckRow {
                Layout.fillWidth: true
                checked: app.watchdogAutoStopDraft
                text: qsTr("Auto-stop running containers by default on critical disk")
                rowHeight: 36
                danger: true
                onToggled: app.watchdogAutoStopDraft = !app.watchdogAutoStopDraft
            }
        }
    }

    // Idle auto-stop
    Rectangle {
        Layout.fillWidth: true
        Layout.preferredHeight: idleStopCardCol.implicitHeight + Style.cardPadding * 2
        radius: Math.min(Style.radius6, theme.radius)
        color: Style.surfaceInput
        border.width: Style.hairline
        border.color: Style.borderSubtle

        ColumnLayout {
            id: idleStopCardCol
            anchors.fill: parent
            anchors.margins: Style.cardPadding
            spacing: Style.space8

            Text {
                text: qsTr("Idle Auto-Stop (Opt-in)")
                color: theme.foreground
                font.family: Style.fontSans
                font.pixelSize: Style.font12
                font.weight: Font.Medium
            }

            Text {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: qsTr("Automatically stops running non-template containers after sustained <3% CPU with no active incus exec session or port forwards (1 min warning notification beforehand). Works only while the AIBox GUI is running — no background daemon is used.")
                color: theme.muted
                font.family: Style.fontSans
                font.pixelSize: Style.font11
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space8

                Repeater {
                    model: backend.idleStopOptions
                    delegate: SelectChip {
                        required property int modelData
                        text: modelData === 0 ? qsTr("Off") : qsTr("%1 min").arg(modelData)
                        selected: app.idleStopMinutesDraft === modelData
                        textSize: Style.font11
                        fillColor: Style.surfaceCard
                        chipHeight: Style.controlHeight
                        horizontalPadding: Style.space20
                        onClicked: app.idleStopMinutesDraft = modelData
                    }
                }
                Item { Layout.fillWidth: true }
            }
        }
    }

    // Stale environment report
    Rectangle {
        Layout.fillWidth: true
        Layout.preferredHeight: staleCardCol.implicitHeight + Style.cardPadding * 2
        radius: Math.min(Style.radius6, theme.radius)
        color: Style.surfaceInput
        border.width: Style.hairline
        border.color: Style.borderSubtle

        ColumnLayout {
            id: staleCardCol
            anchors.fill: parent
            anchors.margins: Style.cardPadding
            spacing: Style.space8

            RowLayout {
                Layout.fillWidth: true

                Text {
                    text: qsTr("Stale Environment Report (U)")
                    color: theme.foreground
                    font.family: Style.fontSans
                    font.pixelSize: Style.font12
                    font.weight: Font.Medium
                }
                Item { Layout.fillWidth: true }
                ActionButton {
                    label: qsTr("Open Report")
                    minWidth: 122
                    onClicked: {
                        app.settingsVisible = false
                        app.openStaleReportDialog()
                    }
                }
            }

            Text {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: qsTr("Flags non-template environments unused for at least staleDays days (plus unknown-age environments without a usage timestamp) and scans orphaned workspaces, snapshot dirs, and unpaired backup archives.")
                color: theme.muted
                font.family: Style.fontSans
                font.pixelSize: Style.font11
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space8

                Repeater {
                    model: backend.staleDayOptions
                    delegate: SelectChip {
                        required property int modelData
                        text: qsTr("%1 d").arg(modelData)
                        selected: app.staleDaysDraft === modelData
                        textSize: Style.font11
                        fillColor: Style.surfaceCard
                        chipHeight: Style.controlHeight
                        horizontalPadding: Style.space20
                        onClicked: app.staleDaysDraft = modelData
                    }
                }
                Item { Layout.fillWidth: true }
            }
        }
    }

    // Session recording
    Rectangle {
        Layout.fillWidth: true
        Layout.preferredHeight: recCardCol.implicitHeight + Style.cardPadding * 2
        radius: Math.min(Style.radius6, theme.radius)
        color: Style.surfaceInput
        border.width: Style.hairline
        border.color: Style.borderSubtle

        ColumnLayout {
            id: recCardCol
            anchors.fill: parent
            anchors.margins: Style.cardPadding
            spacing: Style.space8

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space10

                Text {
                    text: qsTr("Session Recording (Opt-in)")
                    color: theme.foreground
                    font.family: Style.fontSans
                    font.pixelSize: Style.font12
                    font.weight: Font.Medium
                }
                Item { Layout.fillWidth: true }

                SelectChip {
                    text: app.recordSessionsDraft ? qsTr("Enabled") : qsTr("Disabled")
                    selected: app.recordSessionsDraft
                    textSize: Style.font11
                    fillColor: Style.surfaceCard
                    chipHeight: Style.controlHeightSm
                    horizontalPadding: Style.space22
                    onClicked: app.recordSessionsDraft = !app.recordSessionsDraft
                }
            }

            Text {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: qsTr("Records interactive Enter shells using script -q -f with timing files, automatically scrubs S3 secrets after exit, and retains the last 20 sessions per environment (200 MB total cap). View or replay sessions under History (H) -> Sessions.")
                color: theme.muted
                font.family: Style.fontSans
                font.pixelSize: Style.font11
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: Style.space10
        Item { Layout.fillWidth: true }

        ActionButton {
            label: qsTr("Save Safety Settings")
            variant: "primary"
            minWidth: 165
            onClicked: {
                const parsedKeep = parseInt(autoSnapKeepField.text.trim(), 10)
                const keep = isNaN(parsedKeep) ? 0 : parsedKeep
                const snapErr = backend.saveAutoSnapshotSettings(app.autoSnapEnabledDraft, keep)
                if (snapErr.length > 0) {
                    app.showConfigNotice(snapErr, true)
                    return
                }
                const parsedWarn = parseInt(watchdogWarnField.text.trim(), 10)
                const parsedCrit = parseInt(watchdogCritField.text.trim(), 10)
                const wdErr = backend.saveWatchdogSettings(
                    app.watchdogEnabledDraft,
                    isNaN(parsedWarn) ? 0 : parsedWarn,
                    isNaN(parsedCrit) ? 0 : parsedCrit,
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
