import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import "../components"

ModalDialog {
    id: historyOverlay
    property QtObject app: null
    open: app.historyVisible
    zOrder: 85
    clipCard: true
    size: "lg"
    cardHeight: Math.min(parent.height - Style.modalGutter, 620)
    contentSpacing: Style.padCard
    onDismissed: app.historyVisible = false

    ModalHeader {
        title: qsTr("History")
        subtitle: qsTr("Operation logs and recorded terminal sessions")
        onClosed: app.historyVisible = false

        // Segmented switcher: Tasks / Sessions
        SegmentedTabs {
            Layout.alignment: Qt.AlignVCenter
            tabs: [
                { label: qsTr("Tasks (%1)").arg(backend.operationHistory.length), value: 0 },
                { label: qsTr("Sessions (%1)").arg(backend.recordedSessions.length), value: 1 }
            ]
            currentValue: app.historyTab
            textSize: Style.fontSmall
            itemPadding: Style.padCard
            onSelected: function (value) {
                app.historyTab = value
                if (value === 1) {
                    backend.refreshRecordedSessions()
                    if (app.selectedSessionId.length === 0 && backend.recordedSessions.length > 0)
                        app.selectRecordedSession(backend.recordedSessions[0])
                }
            }
        }
    }

    // Tasks tab
    RowLayout {
        visible: app.historyTab === 0
        Layout.fillWidth: true
        Layout.fillHeight: true
        spacing: Style.padCard

        // Left pane: history entries
        Rectangle {
            Layout.preferredWidth: 360
            Layout.fillHeight: true
            radius: Math.min(Style.radiusMd, theme.radius)
            color: theme.surfaceInput
            border.width: Style.hairline
            border.color: theme.borderSubtle

            Text {
                anchors.centerIn: parent
                visible: backend.operationHistory.length === 0
                text: qsTr("No recorded operations yet.")
                color: theme.textMuted
                font.family: Style.fontSans
                font.pixelSize: Style.fontBody
            }

            ListView {
                id: historyListView
                anchors.fill: parent
                anchors.margins: Style.gapMd
                clip: true
                spacing: Style.gapSm
                model: backend.operationHistory

                delegate: Rectangle {
                    required property var modelData
                    readonly property bool selected: app.selectedHistoryId === (modelData.id || "")
                    width: historyListView.width
                    height: histItemCol.implicitHeight + Style.padSection
                    radius: Math.min(Style.radiusMd, theme.radius)
                    color: selected ? theme.surfaceSelected : theme.surfaceInput
                    border.width: Style.hairline
                    border.color: selected ? theme.accentBorder : theme.borderSubtle

                    ColumnLayout {
                        id: histItemCol
                        anchors.fill: parent
                        anchors.margins: Style.gapMd
                        spacing: Style.gapSm

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Style.gapSm

                            TagBadge {
                                text: modelData.status === "failed"
                                      ? qsTr("FAILED")
                                      : (modelData.status === "cancelled" ? qsTr("CANCELLED") : qsTr("OK"))
                                tone: modelData.status === "failed"
                                      ? "danger"
                                      : (modelData.status === "cancelled" ? "warning" : "success")
                                borderColor: modelData.status === "failed"
                                             ? theme.dangerBorder
                                             : (modelData.status === "cancelled" ? theme.warningBorder : theme.successBorder)
                                textWeight: Font.Medium
                            }

                            TagBadge {
                                visible: (modelData.reason || "") === "idle"
                                text: qsTr("IDLE")
                                tone: "warning"
                                textWeight: Font.Medium
                            }

                            Text {
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                                text: modelData.envName || ""
                                color: theme.textMuted
                                font.family: Style.fontMono
                                font.pixelSize: Style.fontCaption
                            }

                            Text {
                                text: modelData.durationMs !== undefined
                                      ? (modelData.durationMs >= 1000
                                         ? (Math.round(modelData.durationMs / 100) / 10) + "s"
                                         : modelData.durationMs + "ms")
                                      : ""
                                color: theme.textMuted
                                font.family: Style.fontMono
                                font.pixelSize: Style.fontCaption
                            }
                        }

                        Text {
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                            text: modelData.description || ""
                            color: theme.textPrimary
                            font.family: Style.fontSans
                            font.pixelSize: Style.fontBody
                        }

                        Text {
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                            text: modelData.timestamp || (modelData.startedAt || "").replace("T", " ")
                            color: theme.textMuted
                            font.family: Style.fontMono
                            font.pixelSize: Style.fontCaption
                        }
                    }

                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            app.selectedHistoryId = modelData.id || ""
                            app.selectedHistoryLog = backend.readOperationLog(app.selectedHistoryId)
                        }
                    }
                }
            }
        }

        // Right pane: full task log
        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            radius: Math.min(Style.radiusMd, theme.radius)
            color: theme.surfaceInput
            border.width: Style.hairline
            border.color: theme.borderSubtle

            ScrollView {
                anchors.fill: parent
                anchors.margins: Style.gap
                clip: true
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                ScrollBar.vertical.policy: ScrollBar.AlwaysOff

                TextArea {
                    readOnly: true
                    selectByMouse: true
                    wrapMode: TextEdit.WrapAnywhere
                    text: app.selectedHistoryLog.length > 0
                          ? app.selectedHistoryLog
                          : qsTr("Select an operation to inspect its log output.")
                    color: app.selectedHistoryLog.length > 0 ? theme.textPrimary : theme.textMuted
                    font.family: Style.fontMono
                    font.pixelSize: Style.fontSmall
                    background: null
                }
            }
        }
    }

    // Sessions tab
    RowLayout {
        visible: app.historyTab === 1
        Layout.fillWidth: true
        Layout.fillHeight: true
        spacing: Style.padCard

        Rectangle {
            Layout.preferredWidth: 340
            Layout.fillHeight: true
            radius: Math.min(Style.radiusMd, theme.radius)
            color: theme.surfaceInput
            border.width: Style.hairline
            border.color: theme.borderSubtle

            Text {
                anchors.centerIn: parent
                width: parent.width - Style.padModal
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
                visible: backend.recordedSessions.length === 0
                text: qsTr("No recorded interactive sessions yet. Enable Session Recording in Settings -> Safety or per environment.")
                color: theme.textMuted
                font.family: Style.fontSans
                font.pixelSize: Style.fontBody
            }

            ListView {
                id: sessionsListView
                anchors.fill: parent
                anchors.margins: Style.gapMd
                clip: true
                spacing: Style.gapSm
                model: backend.recordedSessions

                delegate: Rectangle {
                    required property var modelData
                    readonly property bool selected: app.selectedSessionId === (modelData.id || "")
                    width: sessionsListView.width
                    height: sessItemCol.implicitHeight + Style.padSection
                    radius: Math.min(Style.radiusMd, theme.radius)
                    color: selected ? theme.surfaceSelected : theme.surfaceInput
                    border.width: Style.hairline
                    border.color: selected ? theme.accentBorder : theme.borderSubtle

                    ColumnLayout {
                        id: sessItemCol
                        anchors.fill: parent
                        anchors.margins: Style.gapMd
                        spacing: Style.gapSm

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Style.gapSm

                            Text {
                                text: modelData.envName || ""
                                color: theme.textPrimary
                                font.family: Style.fontMono
                                font.pixelSize: Style.fontBody
                                font.weight: Font.Medium
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }

                            Text {
                                text: modelData.sizeDisplay || ""
                                color: theme.textMuted
                                font.family: Style.fontMono
                                font.pixelSize: Style.fontCaption
                            }

                            Rectangle {
                                height: Style.badgeHeight
                                width: Style.badgeHeight
                                radius: Math.min(Style.radiusSm, theme.radius)
                                color: delSessMouse.containsMouse ? theme.dangerSurface : theme.surfaceInput
                                border.width: Style.hairline
                                border.color: delSessMouse.containsMouse ? theme.dangerBorder : theme.borderSubtle

                                Text {
                                    anchors.centerIn: parent
                                    text: "x"
                                    color: delSessMouse.containsMouse ? theme.dangerText : theme.textMuted
                                    font.family: Style.fontMono
                                    font.pixelSize: Style.fontCaption
                                }

                                MouseArea {
                                    id: delSessMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: {
                                        const wasSelected = (app.selectedSessionId === (modelData.id || ""))
                                        backend.deleteRecordedSession(modelData.logPath || "", modelData.timingPath || "")
                                        if (wasSelected) {
                                            if (backend.recordedSessions.length > 0)
                                                app.selectRecordedSession(backend.recordedSessions[0])
                                            else
                                                app.selectRecordedSession(null)
                                        }
                                    }
                                }
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Style.gapSm

                            Text {
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                                text: modelData.timestamp || ""
                                color: theme.textMuted
                                font.family: Style.fontMono
                                font.pixelSize: Style.fontCaption
                            }

                            Text {
                                visible: modelData.hasTiming || false
                                text: qsTr("TIMED")
                                color: theme.accentText
                                font.family: Style.fontMono
                                font.pixelSize: Style.fontCaption
                            }
                        }
                    }

                    MouseArea {
                        anchors.fill: parent
                        z: -1
                        cursorShape: Qt.PointingHandCursor
                        onClicked: app.selectRecordedSession(modelData)
                    }
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            radius: Math.min(Style.radiusMd, theme.radius)
            color: theme.surfaceInput
            border.width: Style.hairline
            border.color: theme.borderSubtle

            ScrollView {
                anchors.fill: parent
                anchors.margins: Style.gap
                clip: true
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                ScrollBar.vertical.policy: ScrollBar.AlwaysOff

                TextArea {
                    readOnly: true
                    selectByMouse: true
                    wrapMode: TextEdit.WrapAnywhere
                    text: app.selectedSessionText.length > 0
                          ? app.selectedSessionText
                          : qsTr("Select a recorded session to view ANSI-stripped terminal output (200 KB cap).")
                    color: app.selectedSessionText.length > 0 ? theme.textPrimary : theme.textMuted
                    font.family: Style.fontMono
                    font.pixelSize: Style.fontSmall
                    background: null
                }
            }
        }
    }

    DialogFooter {
        ActionButton {
            visible: app.historyTab === 0
            label: qsTr("Clear")
            variant: "danger"
            disabled: backend.operationHistory.length === 0
            onClicked: {
                backend.clearOperationHistory()
                app.selectedHistoryId = ""
                app.selectedHistoryLog = ""
            }
        }

        ActionButton {
            visible: app.historyTab === 1
            label: qsTr("Replay")
            variant: "primary"
            disabled: !app.selectedSessionHasTiming
            onClicked: backend.replayRecordedSession(app.selectedSessionLogPath, app.selectedSessionTimingPath)
        }
    }
}
