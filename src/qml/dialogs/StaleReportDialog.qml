import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import "../components"

ModalDialog {
    id: staleReportDialogRoot
    property QtObject app: null
    open: app.staleReportVisible
    zOrder: 28
    size: "lg"
    cardHeight: Math.min(parent.height - Style.modalGutter, 640)
    onDismissed: {
        app.staleConfirmAction = ""
        app.staleReportVisible = false
    }

    ModalHeader {
        title: qsTr("Stale Environments & Disk Orphans")
        subtitle: qsTr("Unused >= %1d · Reclaimable: %2 · Stale: %3 · Unknown: %4 · Orphans: %5")
                  .arg(backend.staleReport.staleDays !== undefined ? backend.staleReport.staleDays : backend.staleDays)
                  .arg(backend.staleReport.totalReclaimableDisplay || "0 KB")
                  .arg(backend.staleReport.staleCount !== undefined ? backend.staleReport.staleCount : 0)
                  .arg(backend.staleReport.unknownCount !== undefined ? backend.staleReport.unknownCount : 0)
                  .arg(backend.staleReport.orphanCount !== undefined ? backend.staleReport.orphanCount : 0)
        subtitleMono: true
        onClosed: {
            app.staleConfirmAction = ""
            app.staleReportVisible = false
        }

        ActionButton {
            label: qsTr("Rescan")
            disabled: backend.busy
            onClicked: {
                app.staleActionError = ""
                backend.refreshStaleReport()
            }
        }
    }

    // Inline action error banner
    Rectangle {
        visible: app.staleActionError.length > 0
        Layout.fillWidth: true
        implicitHeight: staleErrTxt.implicitHeight + Style.padSection
        radius: Math.min(Style.radiusMd, theme.radius)
        color: theme.dangerSurface
        border.width: Style.hairline
        border.color: theme.dangerBorder
        Text {
            id: staleErrTxt
            anchors.fill: parent
            anchors.margins: Style.gapMd
            text: app.staleActionError
            color: theme.dangerText
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
            wrapMode: Text.WordWrap
        }
    }

    // Inline confirmation bar for Wipe / Export & Wipe / Delete orphan
    Rectangle {
        visible: app.staleConfirmAction.length > 0
        Layout.fillWidth: true
        implicitHeight: staleConfirmCol.implicitHeight + Style.padCard * 2
        radius: Math.min(Style.radiusMd, theme.radius)
        color: theme.surfaceInput
        border.width: Style.hairline
        border.color: app.staleConfirmAction === "export-wipe" ? theme.accentBorder : theme.dangerBorder

        ColumnLayout {
            id: staleConfirmCol
            anchors.fill: parent
            anchors.margins: Style.gap
            spacing: Style.gapMd

            Text {
                Layout.fillWidth: true
                text: app.staleConfirmTitle
                color: theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.fontBody
                font.weight: Font.Medium
                wrapMode: Text.WordWrap
            }
            Text {
                Layout.fillWidth: true
                text: app.staleConfirmDetail
                color: theme.textMuted
                font.family: Style.fontSans
                font.pixelSize: Style.fontSmall
                wrapMode: Text.WordWrap
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.gap

                CheckRow {
                    visible: app.staleConfirmAction === "wipe" || app.staleConfirmAction === "export-wipe"
                    checked: app.staleConfirmDeleteWs
                    rowHeight: Style.controlHeightCompact
                    text: qsTr("Also delete host %1 files & snapshots").arg(backend.containerWorkspacePath)
                    onToggled: app.staleConfirmDeleteWs = !app.staleConfirmDeleteWs
                }

                Item { Layout.fillWidth: true }

                ActionButton {
                    visible: app.staleConfirmAction === "delete-orphan"
                    label: qsTr("Delete as admin")
                    disabled: backend.busy
                    onClicked: {
                        app.staleActionError = ""
                        const err = backend.deleteOrphanAsAdmin(
                            app.staleConfirmKind, app.staleConfirmTarget)
                        if (err.length > 0)
                            app.staleActionError = err
                        else
                            app.staleConfirmAction = ""
                    }
                }

                ActionButton {
                    label: app.staleConfirmAction === "export-wipe"
                           ? qsTr("Export & Wipe")
                           : (app.staleConfirmAction === "delete-orphan" ? qsTr("Delete Orphan") : qsTr("Confirm Wipe"))
                    variant: app.staleConfirmAction === "export-wipe" ? "primary" : "danger"
                    disabled: backend.busy
                    onClicked: {
                        app.staleActionError = ""
                        if (app.staleConfirmAction === "export-wipe") {
                            const err = backend.exportAndWipeEnvironment(
                                app.staleConfirmTarget,
                                true,
                                app.staleConfirmDeleteWs)
                            if (err.length > 0) {
                                app.staleActionError = err
                            } else {
                                app.staleConfirmAction = ""
                                app.staleReportVisible = false
                            }
                        } else if (app.staleConfirmAction === "wipe") {
                            backend.wipeEnvironment(app.staleConfirmTarget, app.staleConfirmDeleteWs)
                            app.staleConfirmAction = ""
                            app.staleReportVisible = false
                        } else if (app.staleConfirmAction === "delete-orphan") {
                            const err = backend.deleteOrphan(app.staleConfirmKind, app.staleConfirmTarget)
                            if (err.length > 0) {
                                app.staleActionError = err
                            } else {
                                app.staleConfirmAction = ""
                            }
                        }
                    }
                }
            }
        }
    }

    ScrollView {
        id: staleReportScroll
        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
        ScrollBar.vertical.policy: ScrollBar.AlwaysOff

        ColumnLayout {
            width: staleReportScroll.availableWidth
            spacing: Style.padCard

            Text {
                text: qsTr("Stale & Unknown Environments (%1)").arg((backend.staleReport.entries || []).length)
                color: theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.fontBody
                font.weight: Font.Medium
            }

            Text {
                visible: (backend.staleReport.entries || []).length === 0
                text: qsTr("No environments have been unused for %1+ days (templates excluded).").arg(backend.staleReport.staleDays || backend.staleDays)
                color: theme.textMuted
                font.family: Style.fontSans
                font.pixelSize: Style.fontBody
            }

            Repeater {
                model: backend.staleReport.entries || []
                delegate: Rectangle {
                    required property var modelData
                    Layout.fillWidth: true
                    implicitHeight: staleEnvRow.implicitHeight + Style.padCard * 2
                    radius: Math.min(Style.radiusMd, theme.radius)
                    color: theme.surfaceInput
                    border.width: Style.hairline
                    border.color: modelData.status === "stale" ? theme.warningBorder : theme.borderSubtle

                    RowLayout {
                        id: staleEnvRow
                        anchors.fill: parent
                        anchors.margins: Style.gap
                        spacing: Style.padCard

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: Style.gapSm

                            RowLayout {
                                spacing: Style.gapMd

                                Rectangle {
                                    width: Style.gapMd
                                    height: Style.gapMd
                                    radius: width / 2
                                    color: modelData.running ? theme.success : theme.textMuted
                                }

                                Text {
                                    text: modelData.name || ""
                                    color: theme.textPrimary
                                    font.family: Style.fontMono
                                    font.pixelSize: Style.fontBody
                                    font.weight: Font.Medium
                                }

                                TagBadge {
                                    text: modelData.status === "stale" ? qsTr("STALE") : qsTr("UNKNOWN")
                                    tone: modelData.status === "stale" ? "warning" : "neutral"
                                    fillColor: theme.surfaceCard
                                    badgeHeight: Style.badgeHeight
                                    badgeRadius: Style.radiusSm
                                    textSize: Style.fontCaption
                                    horizontalPadding: Style.padCard
                                }

                                Text {
                                    text: (modelData.osName || "") + " · " + (modelData.lastUsedDisplay || "")
                                    color: theme.textMuted
                                    font.family: Style.fontSans
                                    font.pixelSize: Style.fontSmall
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                            }

                            Text {
                                Layout.fillWidth: true
                                text: qsTr("Disk: %1 · Snapshots: %2 (%3) · Workspace: %4 · Total: %5")
                                          .arg(modelData.diskUsageDisplay || "0 KB")
                                          .arg(modelData.snapshotCount !== undefined ? modelData.snapshotCount : 0)
                                          .arg(modelData.snapshotTotalDisplay || "—")
                                          .arg(modelData.workspaceSizeDisplay || "0 KB")
                                          .arg(modelData.totalFootprintDisplay || "0 KB")
                                color: theme.textMuted
                                font.family: Style.fontMono
                                font.pixelSize: Style.fontCaption
                                elide: Text.ElideRight
                            }
                        }

                        ActionButton {
                            label: qsTr("Export & Wipe")
                            variant: "primary"
                            disabled: backend.busy
                            onClicked: {
                                app.staleActionError = ""
                                app.staleConfirmAction = "export-wipe"
                                app.staleConfirmTarget = modelData.name || ""
                                app.staleConfirmKind = ""
                                app.staleConfirmDeleteWs = true
                                app.staleConfirmTitle = qsTr("Export & Wipe '%1'?").arg(modelData.name || "")
                                app.staleConfirmDetail = qsTr("Exports an instance & workspace backup archive to %1 first. Wipe proceeds ONLY if export succeeds and the archive size is > 0 bytes. Footprint: %2.")
                                                             .arg(backend.backupsDirPath)
                                                             .arg(modelData.totalFootprintDisplay || "0 KB")
                            }
                        }

                        ActionButton {
                            label: qsTr("Wipe")
                            variant: "danger"
                            disabled: backend.busy
                            onClicked: {
                                app.staleActionError = ""
                                app.staleConfirmAction = "wipe"
                                app.staleConfirmTarget = modelData.name || ""
                                app.staleConfirmKind = ""
                                app.staleConfirmDeleteWs = true
                                app.staleConfirmTitle = qsTr("Wipe environment '%1' without export?").arg(modelData.name || "")
                                app.staleConfirmDetail = qsTr("Permanently destroys the container/VM and its snapshots (%1 disk, %2 snapshots, %3 workspace).")
                                                             .arg(modelData.diskUsageDisplay || "0 KB")
                                                             .arg(modelData.snapshotCount || 0)
                                                             .arg(modelData.workspaceSizeDisplay || "0 KB")
                            }
                        }
                    }
                }
            }

            Rectangle {
                Layout.fillWidth: true
                height: Style.hairline
                color: theme.borderSubtle
            }

            Text {
                text: qsTr("Orphaned Folders, Snapshots & Backups (%1)").arg((backend.staleReport.orphans || []).length)
                color: theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.fontBody
                font.weight: Font.Medium
            }

            Text {
                visible: (backend.staleReport.orphans || []).length === 0
                text: qsTr("No orphaned workspace folders, workspace snapshot directories, or unpaired backup workspace archives found.")
                color: theme.textMuted
                font.family: Style.fontSans
                font.pixelSize: Style.fontBody
            }

            Repeater {
                model: backend.staleReport.orphans || []
                delegate: Rectangle {
                    required property var modelData
                    Layout.fillWidth: true
                    implicitHeight: orphanRow.implicitHeight + Style.padCard * 2
                    radius: Math.min(Style.radiusMd, theme.radius)
                    color: theme.surfaceInput
                    border.width: Style.hairline
                    border.color: theme.borderSubtle

                    RowLayout {
                        id: orphanRow
                        anchors.fill: parent
                        anchors.margins: Style.gap
                        spacing: Style.gap

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: Style.gapXs

                            RowLayout {
                                spacing: Style.gapMd

                                Text {
                                    text: modelData.name || ""
                                    color: theme.textPrimary
                                    font.family: Style.fontMono
                                    font.pixelSize: Style.fontBody
                                    font.weight: Font.Medium
                                }

                                TagBadge {
                                    text: modelData.kindLabel || modelData.kind || ""
                                    tone: "neutral"
                                    fillColor: theme.surfaceCard
                                    badgeHeight: Style.badgeHeight
                                    badgeRadius: Style.radiusSm
                                    textSize: Style.fontCaption
                                }

                                TagBadge {
                                    text: modelData.sizeDisplay || "0 KB"
                                    tone: "accent"
                                    fillColor: theme.surfaceCard
                                    badgeHeight: Style.badgeHeight
                                    badgeRadius: Style.radiusSm
                                    textSize: Style.fontCaption
                                }
                            }

                            Text {
                                Layout.fillWidth: true
                                text: modelData.path || ""
                                color: theme.textMuted
                                font.family: Style.fontMono
                                font.pixelSize: Style.fontCaption
                                elide: Text.ElideMiddle
                            }
                        }

                        ActionButton {
                            label: qsTr("Open folder")
                            onClicked: backend.openOrphanFolder(modelData.openPath || modelData.path || "")
                        }

                        ActionButton {
                            label: qsTr("Delete")
                            variant: "danger"
                            disabled: backend.busy
                            onClicked: {
                                app.staleActionError = ""
                                app.staleConfirmAction = "delete-orphan"
                                app.staleConfirmKind = modelData.kind || ""
                                app.staleConfirmTarget = modelData.path || ""
                                app.staleConfirmTitle = qsTr("Delete orphan '%1' (%2)?")
                                                            .arg(modelData.name || "")
                                                            .arg(modelData.sizeDisplay || "0 KB")
                                app.staleConfirmDetail = qsTr("Permanently deletes %1 (%2 · %3). This cannot be undone. Root-owned files need 'Delete as admin'.")
                                                             .arg(modelData.path || "")
                                                             .arg(modelData.kindLabel || "")
                                                             .arg(modelData.sizeDisplay || "0 KB")
                            }
                        }
                    }
                }
            }
        }
    }
}
