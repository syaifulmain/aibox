import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts
import "../components"

Rectangle {
    property QtObject app: null
    id: historyOverlay
    anchors.fill: parent
    visible: app.historyVisible
    color: "#c406070a"
    z: 85

    MouseArea {
        anchors.fill: parent
        onClicked: app.historyVisible = false
    }

    Rectangle {
        anchors.centerIn: parent
        width: Math.min(parent.width - 56, 980)
        height: Math.min(parent.height - 56, 620)
        radius: Math.min(8, theme.radius)
        color: app.surfaceCard
        border.width: 1
        border.color: theme.accent
        clip: true

        MouseArea { anchors.fill: parent }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 24
            spacing: 14

            RowLayout {
                Layout.fillWidth: true
                spacing: 10

                Text {
                    text: app.historyTab === 0 ? qsTr("Operation History") : qsTr("Recorded Sessions")
                    color: theme.foreground
                    font.family: app.sansFont
                    font.pixelSize: 16
                    font.weight: Font.Medium
                }

                // Segmented switcher: Tasks / Sessions (H3)
                Rectangle {
                    height: 34
                    width: histTabRow.implicitWidth + 8
                    radius: Math.min(4, theme.radius)
                    color: app.surfaceInput
                    border.width: 1
                    border.color: app.borderSubtle

                    RowLayout {
                        id: histTabRow
                        anchors.centerIn: parent
                        spacing: 4
                        Repeater {
                            model: [
                                { tab: 0, label: qsTr("Tasks (%1)").arg(backend.operationHistory.length) },
                                { tab: 1, label: qsTr("Sessions (%1)").arg(backend.recordedSessions.length) }
                            ]
                            delegate: Rectangle {
                                required property var modelData
                                height: 28
                                width: hTabTxt.implicitWidth + 14
                                radius: Math.min(3, theme.radius)
                                color: app.historyTab === modelData.tab ? app.surfaceSelected : "transparent"
                                border.width: app.historyTab === modelData.tab ? 1 : 0
                                border.color: theme.accent
                                Text {
                                    id: hTabTxt
                                    anchors.centerIn: parent
                                    text: modelData.label
                                    color: app.historyTab === modelData.tab ? theme.foreground : theme.muted
                                    font.family: app.sansFont
                                    font.pixelSize: 11
                                }
                                MouseArea {
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: {
                                        app.historyTab = modelData.tab
                                        if (modelData.tab === 1) {
                                            backend.refreshRecordedSessions()
                                            if (app.selectedSessionId.length === 0 && backend.recordedSessions.length > 0)
                                                app.selectRecordedSession(backend.recordedSessions[0])
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                Item { Layout.fillWidth: true }

                ActionButton {
                    app: historyOverlay.app
                    visible: app.historyTab === 0
                    label: qsTr("Clear History")
                    danger: true
                    minWidth: 112
                    disabled: backend.operationHistory.length === 0
                    onClicked: {
                        backend.clearOperationHistory()
                        app.selectedHistoryId = ""
                        app.selectedHistoryLog = ""
                    }
                }

                ActionButton {
                    app: historyOverlay.app
                    visible: app.historyTab === 1
                    label: qsTr("Replay in terminal")
                    variant: "primary"
                    minWidth: 146
                    disabled: !app.selectedSessionHasTiming
                    onClicked: backend.replayRecordedSession(app.selectedSessionLogPath, app.selectedSessionTimingPath)
                }

                ActionButton {
                    app: historyOverlay.app
                    label: qsTr("Close")
                    shortcutHint: "Esc"
                    minWidth: 88
                    onClicked: app.historyVisible = false
                }
            }

            Rectangle {
                Layout.fillWidth: true
                height: 1
                color: app.borderSubtle
            }

            RowLayout {
                visible: app.historyTab === 0
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 14

                // Left pane: History Entries
                Rectangle {
                    Layout.preferredWidth: 360
                    Layout.fillHeight: true
                    radius: Math.min(4, theme.radius)
                    color: app.surfaceInput
                    border.width: 1
                    border.color: app.borderSubtle

                    Text {
                        anchors.centerIn: parent
                        visible: backend.operationHistory.length === 0
                        text: qsTr("No recorded operations yet.")
                        color: theme.muted
                        font.family: app.sansFont
                        font.pixelSize: 12
                    }

                    ListView {
                        id: historyListView
                        anchors.fill: parent
                        anchors.margins: 8
                        clip: true
                        spacing: 6
                        model: backend.operationHistory
                        delegate: Rectangle {
                            required property var modelData
                            readonly property bool selected: (app.selectedHistoryId === (modelData.id || ""))
                            width: historyListView.width
                            height: histItemCol.implicitHeight + 16
                            radius: Math.min(4, theme.radius)
                            color: selected ? app.surfaceSelected : app.surfaceCard
                            border.width: 1
                            border.color: selected ? theme.accent : app.borderSubtle

                            ColumnLayout {
                                id: histItemCol
                                anchors.fill: parent
                                anchors.margins: 8
                                spacing: 4

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 6

                                    Rectangle {
                                        height: 18
                                        width: histBadgeTxt.implicitWidth + 10
                                        radius: 2
                                        color: modelData.status === "failed"
                                               ? "#2c1419"
                                               : (modelData.status === "cancelled" ? "#262014" : "#162920")
                                        border.width: 1
                                        border.color: modelData.status === "failed"
                                                      ? theme.red
                                                      : (modelData.status === "cancelled" ? "#e5a93c" : theme.green)
                                        Text {
                                            id: histBadgeTxt
                                            anchors.centerIn: parent
                                            text: modelData.status === "failed"
                                                  ? qsTr("FAILED")
                                                  : (modelData.status === "cancelled" ? qsTr("CANCELLED") : qsTr("OK"))
                                            color: modelData.status === "failed"
                                                   ? theme.red
                                                   : (modelData.status === "cancelled" ? "#e5a93c" : theme.green)
                                            font.family: app.monoFont
                                            font.pixelSize: 9
                                            font.weight: Font.Medium
                                        }
                                    }

                                    Rectangle {
                                        visible: (modelData.reason || "") === "idle"
                                        height: 18
                                        width: idleReasonTxt.implicitWidth + 10
                                        radius: 2
                                        color: "#262014"
                                        border.width: 1
                                        border.color: "#e5a93c"
                                        Text {
                                            id: idleReasonTxt
                                            anchors.centerIn: parent
                                            text: qsTr("IDLE")
                                            color: "#e5a93c"
                                            font.family: app.monoFont
                                            font.pixelSize: 9
                                            font.weight: Font.Medium
                                        }
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        elide: Text.ElideRight
                                        text: modelData.envName || ""
                                        color: theme.muted
                                        font.family: app.monoFont
                                        font.pixelSize: 10
                                    }

                                    Text {
                                        text: modelData.durationMs !== undefined
                                              ? (modelData.durationMs >= 1000
                                                 ? (Math.round(modelData.durationMs / 100) / 10) + "s"
                                                 : modelData.durationMs + "ms")
                                              : ""
                                        color: theme.muted
                                        font.family: app.monoFont
                                        font.pixelSize: 10
                                    }
                                }

                                Text {
                                    Layout.fillWidth: true
                                    elide: Text.ElideRight
                                    text: modelData.description || ""
                                    color: theme.foreground
                                    font.family: app.sansFont
                                    font.pixelSize: 12
                                }

                                Text {
                                    Layout.fillWidth: true
                                    elide: Text.ElideRight
                                    text: modelData.timestamp || (modelData.startedAt || "").replace("T", " ")
                                    color: theme.muted
                                    font.family: app.monoFont
                                    font.pixelSize: 10
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

                // Right pane: Full Task Log
                Rectangle {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    radius: Math.min(4, theme.radius)
                    color: app.surfaceInput
                    border.width: 1
                    border.color: app.borderSubtle

                    ScrollView {
                        anchors.fill: parent
                        anchors.margins: 10
                        clip: true

                        TextArea {
                            readOnly: true
                            selectByMouse: true
                            wrapMode: TextEdit.WrapAnywhere
                            text: app.selectedHistoryLog.length > 0
                                  ? app.selectedHistoryLog
                                  : qsTr("Select an operation to inspect its log output.")
                            color: app.selectedHistoryLog.length > 0 ? theme.foreground : theme.muted
                            font.family: app.monoFont
                            font.pixelSize: 11
                            background: null
                        }
                    }
                }
            }

            // Sessions Tab Content (H3)
            RowLayout {
                visible: app.historyTab === 1
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 14

                Rectangle {
                    Layout.preferredWidth: 340
                    Layout.fillHeight: true
                    radius: Math.min(4, theme.radius)
                    color: app.surfaceInput
                    border.width: 1
                    border.color: app.borderSubtle

                    Text {
                        anchors.centerIn: parent
                        width: parent.width - 24
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.WordWrap
                        visible: backend.recordedSessions.length === 0
                        text: qsTr("No recorded interactive sessions yet. Enable Session Recording in Settings -> Safety or per environment.")
                        color: theme.muted
                        font.family: app.sansFont
                        font.pixelSize: 12
                    }

                    ListView {
                        id: sessionsListView
                        anchors.fill: parent
                        anchors.margins: 8
                        clip: true
                        spacing: 6
                        model: backend.recordedSessions
                        delegate: Rectangle {
                            required property var modelData
                            readonly property bool selected: (app.selectedSessionId === (modelData.id || ""))
                            width: sessionsListView.width
                            height: sessItemCol.implicitHeight + 16
                            radius: Math.min(4, theme.radius)
                            color: selected ? app.surfaceSelected : app.surfaceCard
                            border.width: 1
                            border.color: selected ? theme.accent : app.borderSubtle

                            ColumnLayout {
                                id: sessItemCol
                                anchors.fill: parent
                                anchors.margins: 8
                                spacing: 4

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 6
                                    Text {
                                        text: modelData.envName || ""
                                        color: theme.foreground
                                        font.family: app.monoFont
                                        font.pixelSize: 12
                                        font.weight: Font.Medium
                                        elide: Text.ElideRight
                                        Layout.fillWidth: true
                                    }
                                    Text {
                                        text: modelData.sizeDisplay || ""
                                        color: theme.muted
                                        font.family: app.monoFont
                                        font.pixelSize: 10
                                    }
                                    Rectangle {
                                        height: 20
                                        width: 22
                                        radius: 3
                                        color: delSessMouse.containsMouse ? "#2c1419" : app.surfaceInput
                                        border.width: 1
                                        border.color: delSessMouse.containsMouse ? theme.red : app.borderSubtle
                                        Text {
                                            anchors.centerIn: parent
                                            text: "x"
                                            color: delSessMouse.containsMouse ? theme.red : theme.muted
                                            font.family: app.monoFont
                                            font.pixelSize: 10
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
                                    spacing: 6
                                    Text {
                                        Layout.fillWidth: true
                                        elide: Text.ElideRight
                                        text: modelData.timestamp || ""
                                        color: theme.muted
                                        font.family: app.monoFont
                                        font.pixelSize: 10
                                    }
                                    Text {
                                        visible: modelData.hasTiming || false
                                        text: qsTr("TIMED")
                                        color: theme.accent
                                        font.family: app.monoFont
                                        font.pixelSize: 9
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
                    radius: Math.min(4, theme.radius)
                    color: app.surfaceInput
                    border.width: 1
                    border.color: app.borderSubtle

                    ScrollView {
                        anchors.fill: parent
                        anchors.margins: 10
                        clip: true

                        TextArea {
                            readOnly: true
                            selectByMouse: true
                            wrapMode: TextEdit.WrapAnywhere
                            text: app.selectedSessionText.length > 0
                                  ? app.selectedSessionText
                                  : qsTr("Select a recorded session to view ANSI-stripped terminal output (200 KB cap).")
                            color: app.selectedSessionText.length > 0 ? theme.foreground : theme.muted
                            font.family: app.monoFont
                            font.pixelSize: 11
                            background: null
                        }
                    }
                }
            }
        }
    }
}
