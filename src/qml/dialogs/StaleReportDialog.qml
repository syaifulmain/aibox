import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import "../components"

ModalDialog {
    id: staleReportDialogRoot
    property QtObject app: null
    open: app.staleReportVisible
    zOrder: 28
    cardWidth: Math.min(860, parent.width - Style.modalGutter)
    cardHeight: Math.min(640, parent.height - Style.modalEdge)
    onDismissed: app.staleReportVisible = false

    ModalHeader {
        title: qsTr("Stale Environments & Disk Orphans")
        subtitle: qsTr("Unused >= %1d · Reclaimable: %2 · Stale: %3 · Unknown: %4 · Orphans: %5")
                  .arg(backend.staleReport.staleDays !== undefined ? backend.staleReport.staleDays : backend.staleDays)
                  .arg(backend.staleReport.totalReclaimableDisplay || "0 KB")
                  .arg(backend.staleReport.staleCount !== undefined ? backend.staleReport.staleCount : 0)
                  .arg(backend.staleReport.unknownCount !== undefined ? backend.staleReport.unknownCount : 0)
                  .arg(backend.staleReport.orphanCount !== undefined ? backend.staleReport.orphanCount : 0)
        subtitleMono: true
        onClosed: app.staleReportVisible = false

        ActionButton {
            label: qsTr("Rescan")
            minWidth: 88
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
        implicitHeight: staleErrTxt.implicitHeight + Style.space14
        radius: Math.min(Style.radius4, theme.radius)
        color: Style.dangerSurfaceAlt
        border.width: Style.hairline
        border.color: theme.red
        Text {
            id: staleErrTxt
            anchors.fill: parent
            anchors.margins: Style.space7
            text: app.staleActionError
            color: theme.red
            font.family: Style.fontSans
            font.pixelSize: Style.font11
            wrapMode: Text.WordWrap
        }
    }

    // Inline confirmation bar for Wipe / Export & Wipe / Delete orphan
    Rectangle {
        visible: app.staleConfirmAction.length > 0
        Layout.fillWidth: true
        implicitHeight: staleConfirmCol.implicitHeight + Style.space20
        radius: Math.min(Style.radius4, theme.radius)
        color: Style.surfaceInput
        border.width: Style.hairline
        border.color: app.staleConfirmAction === "export-wipe" ? theme.accent : theme.red

        ColumnLayout {
            id: staleConfirmCol
            anchors.fill: parent
            anchors.margins: Style.space10
            spacing: Style.space8

            Text {
                Layout.fillWidth: true
                text: app.staleConfirmTitle
                color: theme.foreground
                font.family: Style.fontSans
                font.pixelSize: Style.font12
                font.weight: Font.Medium
                wrapMode: Text.WordWrap
            }
            Text {
                Layout.fillWidth: true
                text: app.staleConfirmDetail
                color: theme.muted
                font.family: Style.fontSans
                font.pixelSize: Style.font11
                wrapMode: Text.WordWrap
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space10

                Rectangle {
                    visible: app.staleConfirmAction === "wipe" || app.staleConfirmAction === "export-wipe"
                    height: Style.controlHeightSm
                    width: staleWsDelRow.implicitWidth + Style.space16
                    radius: Math.min(Style.radius4, theme.radius)
                    color: app.staleConfirmDeleteWs ? Style.surfaceSelected : Style.surfaceCard
                    border.width: Style.hairline
                    border.color: app.staleConfirmDeleteWs ? theme.accent : Style.borderSubtle

                    RowLayout {
                        id: staleWsDelRow
                        anchors.centerIn: parent
                        spacing: Style.space6
                        Text {
                            text: app.staleConfirmDeleteWs ? "[x]" : "[ ]"
                            color: app.staleConfirmDeleteWs ? theme.accent : theme.muted
                            font.family: Style.fontMono
                            font.pixelSize: Style.font11
                        }
                        Text {
                            text: qsTr("Also delete host %1 files & snapshots").arg(backend.containerWorkspacePath)
                            color: theme.foreground
                            font.family: Style.fontSans
                            font.pixelSize: Style.font11
                        }
                    }

                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: app.staleConfirmDeleteWs = !app.staleConfirmDeleteWs
                    }
                }

                Item { Layout.fillWidth: true }

                ActionButton {
                    visible: app.staleConfirmAction === "delete-orphan"
                    label: qsTr("Delete as admin")
                    minWidth: 132
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
                    label: qsTr("Cancel")
                    minWidth: 84
                    onClicked: app.staleConfirmAction = ""
                }

                ActionButton {
                    label: app.staleConfirmAction === "export-wipe"
                           ? qsTr("Export & Wipe")
                           : (app.staleConfirmAction === "delete-orphan" ? qsTr("Delete Orphan") : qsTr("Confirm Wipe"))
                    variant: app.staleConfirmAction === "export-wipe" ? "primary" : ""
                    danger: app.staleConfirmAction !== "export-wipe"
                    minWidth: 126
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

        ColumnLayout {
            width: staleReportScroll.availableWidth
            spacing: Style.space14

            Text {
                text: qsTr("Stale & Unknown Environments (%1)").arg((backend.staleReport.entries || []).length)
                color: theme.foreground
                font.family: Style.fontSans
                font.pixelSize: Style.font13
                font.weight: Font.Medium
            }

            Text {
                visible: (backend.staleReport.entries || []).length === 0
                text: qsTr("No environments have been unused for %1+ days (templates excluded).").arg(backend.staleReport.staleDays || backend.staleDays)
                color: theme.muted
                font.family: Style.fontSans
                font.pixelSize: Style.font12
            }

            Repeater {
                model: backend.staleReport.entries || []
                delegate: Rectangle {
                    required property var modelData
                    Layout.fillWidth: true
                    implicitHeight: staleEnvRow.implicitHeight + Style.space20
                    radius: Math.min(Style.radius4, theme.radius)
                    color: Style.surfaceInput
                    border.width: Style.hairline
                    border.color: modelData.status === "stale" ? Style.warning : Style.borderSubtle

                    RowLayout {
                        id: staleEnvRow
                        anchors.fill: parent
                        anchors.margins: Style.space10
                        spacing: Style.space12

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: Style.space4

                            RowLayout {
                                spacing: Style.space8

                                Rectangle {
                                    width: 8
                                    height: 8
                                    radius: 4
                                    color: modelData.running ? theme.green : theme.muted
                                }

                                Text {
                                    text: modelData.name || ""
                                    color: theme.foreground
                                    font.family: Style.fontMono
                                    font.pixelSize: Style.font13
                                    font.weight: Font.Medium
                                }

                                TagBadge {
                                    text: modelData.status === "stale" ? qsTr("STALE") : qsTr("UNKNOWN")
                                    tone: modelData.status === "stale" ? "warning" : "neutral"
                                    fillColor: Style.surfaceCard
                                    badgeHeight: Style.badgeHeightMd
                                    badgeRadius: Style.radius3
                                    textSize: Style.font10
                                    horizontalPadding: Style.space12
                                }

                                Text {
                                    text: (modelData.osName || "") + " · " + (modelData.lastUsedDisplay || "")
                                    color: theme.muted
                                    font.family: Style.fontSans
                                    font.pixelSize: Style.font11
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
                                color: theme.muted
                                font.family: Style.fontMono
                                font.pixelSize: Style.font10
                                elide: Text.ElideRight
                            }
                        }

                        ActionButton {
                            label: qsTr("Export & Wipe")
                            variant: "primary"
                            minWidth: 118
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
                            danger: true
                            minWidth: 74
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
                color: Style.borderSubtle
            }

            Text {
                text: qsTr("Orphaned Folders, Snapshots & Backups (%1)").arg((backend.staleReport.orphans || []).length)
                color: theme.foreground
                font.family: Style.fontSans
                font.pixelSize: Style.font13
                font.weight: Font.Medium
            }

            Text {
                visible: (backend.staleReport.orphans || []).length === 0
                text: qsTr("No orphaned workspace folders, workspace snapshot directories, or unpaired backup workspace archives found.")
                color: theme.muted
                font.family: Style.fontSans
                font.pixelSize: Style.font12
            }

            Repeater {
                model: backend.staleReport.orphans || []
                delegate: Rectangle {
                    required property var modelData
                    Layout.fillWidth: true
                    implicitHeight: orphanRow.implicitHeight + Style.space20
                    radius: Math.min(Style.radius4, theme.radius)
                    color: Style.surfaceInput
                    border.width: Style.hairline
                    border.color: Style.borderSubtle

                    RowLayout {
                        id: orphanRow
                        anchors.fill: parent
                        anchors.margins: Style.space10
                        spacing: Style.space10

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: Style.space3

                            RowLayout {
                                spacing: Style.space8

                                Text {
                                    text: modelData.name || ""
                                    color: theme.foreground
                                    font.family: Style.fontMono
                                    font.pixelSize: Style.font12
                                    font.weight: Font.Medium
                                }

                                TagBadge {
                                    text: modelData.kindLabel || modelData.kind || ""
                                    tone: "neutral"
                                    fillColor: Style.surfaceCard
                                    badgeHeight: Style.badgeHeightMd
                                    badgeRadius: Style.radius3
                                    textSize: Style.font10
                                }

                                TagBadge {
                                    text: modelData.sizeDisplay || "0 KB"
                                    tone: "accent"
                                    fillColor: Style.surfaceCard
                                    badgeHeight: Style.badgeHeightMd
                                    badgeRadius: Style.radius3
                                    textSize: Style.font10
                                }
                            }

                            Text {
                                Layout.fillWidth: true
                                text: modelData.path || ""
                                color: theme.muted
                                font.family: Style.fontMono
                                font.pixelSize: Style.font10
                                elide: Text.ElideMiddle
                            }
                        }

                        ActionButton {
                            label: qsTr("Open folder")
                            minWidth: 104
                            onClicked: backend.openOrphanFolder(modelData.openPath || modelData.path || "")
                        }

                        ActionButton {
                            label: qsTr("Delete")
                            danger: true
                            minWidth: 80
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
