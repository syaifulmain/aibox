import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts
import "../components"

Rectangle {
    id: staleReportDialogRoot
    property QtObject app: null
    visible: app.staleReportVisible
    anchors.fill: parent
    color: "#c406070a"
    z: 28

    MouseArea {
        anchors.fill: parent
        onClicked: app.staleReportVisible = false
    }

    Rectangle {
        width: Math.min(860, parent.width - 56)
        height: Math.min(640, parent.height - 56)
        anchors.centerIn: parent
        radius: Math.min(8, theme.radius)
        color: app.surfaceCard
        border.width: 1
        border.color: theme.accent

        MouseArea { anchors.fill: parent }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 24
            spacing: 12

            ModalHeader {
                app: staleReportDialogRoot.app
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
                    app: staleReportDialogRoot.app
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
                implicitHeight: staleErrTxt.implicitHeight + 14
                radius: Math.min(4, theme.radius)
                color: "#291418"
                border.width: 1
                border.color: theme.red
                Text {
                    id: staleErrTxt
                    anchors.fill: parent
                    anchors.margins: 7
                    text: app.staleActionError
                    color: theme.red
                    font.family: app.sansFont
                    font.pixelSize: 11
                    wrapMode: Text.WordWrap
                }
            }

            // Inline confirmation bar for Wipe / Export & Wipe / Delete orphan
            Rectangle {
                visible: app.staleConfirmAction.length > 0
                Layout.fillWidth: true
                implicitHeight: staleConfirmCol.implicitHeight + 20
                radius: Math.min(4, theme.radius)
                color: app.surfaceInput
                border.width: 1
                border.color: app.staleConfirmAction === "export-wipe" ? theme.accent : theme.red

                ColumnLayout {
                    id: staleConfirmCol
                    anchors.fill: parent
                    anchors.margins: 10
                    spacing: 8

                    Text {
                        Layout.fillWidth: true
                        text: app.staleConfirmTitle
                        color: theme.foreground
                        font.family: app.sansFont
                        font.pixelSize: 12
                        font.weight: Font.Medium
                        wrapMode: Text.WordWrap
                    }
                    Text {
                        Layout.fillWidth: true
                        text: app.staleConfirmDetail
                        color: theme.muted
                        font.family: app.sansFont
                        font.pixelSize: 11
                        wrapMode: Text.WordWrap
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 10

                        Rectangle {
                            visible: app.staleConfirmAction === "wipe" || app.staleConfirmAction === "export-wipe"
                            height: 28
                            width: staleWsDelRow.implicitWidth + 16
                            radius: Math.min(4, theme.radius)
                            color: app.staleConfirmDeleteWs ? app.surfaceSelected : app.surfaceCard
                            border.width: 1
                            border.color: app.staleConfirmDeleteWs ? theme.accent : app.borderSubtle

                            RowLayout {
                                id: staleWsDelRow
                                anchors.centerIn: parent
                                spacing: 6
                                Text {
                                    text: app.staleConfirmDeleteWs ? "[x]" : "[ ]"
                                    color: app.staleConfirmDeleteWs ? theme.accent : theme.muted
                                    font.family: app.monoFont
                                    font.pixelSize: 11
                                }
                                Text {
                                    text: qsTr("Also delete host %1 files & snapshots").arg(backend.containerWorkspacePath)
                                    color: theme.foreground
                                    font.family: app.sansFont
                                    font.pixelSize: 11
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
                            app: staleReportDialogRoot.app
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
                            app: staleReportDialogRoot.app
                            label: qsTr("Cancel")
                            minWidth: 84
                            onClicked: app.staleConfirmAction = ""
                        }

                        ActionButton {
                            app: staleReportDialogRoot.app
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
                    spacing: 14

                    Text {
                        text: qsTr("Stale & Unknown Environments (%1)").arg((backend.staleReport.entries || []).length)
                        color: theme.foreground
                        font.family: app.sansFont
                        font.pixelSize: 13
                        font.weight: Font.Medium
                    }

                    Text {
                        visible: (backend.staleReport.entries || []).length === 0
                        text: qsTr("No environments have been unused for %1+ days (templates excluded).").arg(backend.staleReport.staleDays || backend.staleDays)
                        color: theme.muted
                        font.family: app.sansFont
                        font.pixelSize: 12
                    }

                    Repeater {
                        model: backend.staleReport.entries || []
                        delegate: Rectangle {
                            required property var modelData
                            Layout.fillWidth: true
                            implicitHeight: staleEnvRow.implicitHeight + 20
                            radius: Math.min(4, theme.radius)
                            color: app.surfaceInput
                            border.width: 1
                            border.color: modelData.status === "stale" ? "#e5a93c" : app.borderSubtle

                            RowLayout {
                                id: staleEnvRow
                                anchors.fill: parent
                                anchors.margins: 10
                                spacing: 12

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 4

                                    RowLayout {
                                        spacing: 8
                                        Rectangle {
                                            width: 8
                                            height: 8
                                            radius: 4
                                            color: modelData.running ? theme.green : theme.muted
                                        }
                                        Text {
                                            text: modelData.name || ""
                                            color: theme.foreground
                                            font.family: app.monoFont
                                            font.pixelSize: 13
                                            font.weight: Font.Medium
                                        }
                                        Rectangle {
                                            height: 20
                                            width: staleBadgeTxt.implicitWidth + 12
                                            radius: 3
                                            color: app.surfaceCard
                                            border.width: 1
                                            border.color: modelData.status === "stale" ? "#e5a93c" : app.borderSubtle
                                            Text {
                                                id: staleBadgeTxt
                                                anchors.centerIn: parent
                                                text: modelData.status === "stale" ? qsTr("STALE") : qsTr("UNKNOWN")
                                                color: modelData.status === "stale" ? "#e5a93c" : theme.muted
                                                font.family: app.monoFont
                                                font.pixelSize: 10
                                            }
                                        }
                                        Text {
                                            text: (modelData.osName || "") + " · " + (modelData.lastUsedDisplay || "")
                                            color: theme.muted
                                            font.family: app.sansFont
                                            font.pixelSize: 11
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
                                        font.family: app.monoFont
                                        font.pixelSize: 10
                                        elide: Text.ElideRight
                                    }
                                }

                                ActionButton {
                                    app: staleReportDialogRoot.app
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
                                    app: staleReportDialogRoot.app
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

                    Rectangle { Layout.fillWidth: true; height: 1; color: app.borderSubtle }

                    Text {
                        text: qsTr("Orphaned Folders, Snapshots & Backups (%1)").arg((backend.staleReport.orphans || []).length)
                        color: theme.foreground
                        font.family: app.sansFont
                        font.pixelSize: 13
                        font.weight: Font.Medium
                    }

                    Text {
                        visible: (backend.staleReport.orphans || []).length === 0
                        text: qsTr("No orphaned workspace folders, workspace snapshot directories, or unpaired backup workspace archives found.")
                        color: theme.muted
                        font.family: app.sansFont
                        font.pixelSize: 12
                    }

                    Repeater {
                        model: backend.staleReport.orphans || []
                        delegate: Rectangle {
                            required property var modelData
                            Layout.fillWidth: true
                            implicitHeight: orphanRow.implicitHeight + 20
                            radius: Math.min(4, theme.radius)
                            color: app.surfaceInput
                            border.width: 1
                            border.color: app.borderSubtle

                            RowLayout {
                                id: orphanRow
                                anchors.fill: parent
                                anchors.margins: 10
                                spacing: 10

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 3

                                    RowLayout {
                                        spacing: 8
                                        Text {
                                            text: modelData.name || ""
                                            color: theme.foreground
                                            font.family: app.monoFont
                                            font.pixelSize: 12
                                            font.weight: Font.Medium
                                        }
                                        Rectangle {
                                            height: 20
                                            width: orphanKindTxt.implicitWidth + 10
                                            radius: 3
                                            color: app.surfaceCard
                                            border.width: 1
                                            border.color: app.borderSubtle
                                            Text {
                                                id: orphanKindTxt
                                                anchors.centerIn: parent
                                                text: modelData.kindLabel || modelData.kind || ""
                                                color: theme.muted
                                                font.family: app.monoFont
                                                font.pixelSize: 10
                                            }
                                        }
                                        Rectangle {
                                            height: 20
                                            width: orphanSizeTxt.implicitWidth + 10
                                            radius: 3
                                            color: app.surfaceCard
                                            border.width: 1
                                            border.color: theme.accent
                                            Text {
                                                id: orphanSizeTxt
                                                anchors.centerIn: parent
                                                text: modelData.sizeDisplay || "0 KB"
                                                color: theme.accent
                                                font.family: app.monoFont
                                                font.pixelSize: 10
                                            }
                                        }
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        text: modelData.path || ""
                                        color: theme.muted
                                        font.family: app.monoFont
                                        font.pixelSize: 10
                                        elide: Text.ElideMiddle
                                    }
                                }

                                ActionButton {
                                    app: staleReportDialogRoot.app
                                    label: qsTr("Open folder")
                                    minWidth: 104
                                    onClicked: backend.openOrphanFolder(modelData.openPath || modelData.path || "")
                                }

                                ActionButton {
                                    app: staleReportDialogRoot.app
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
    }
}
