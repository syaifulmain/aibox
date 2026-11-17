import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import "../components"

// Left pane of the Environments tab: the environment list, pool summary and
// the stale-report entry point.
Rectangle {
    id: listRoot
    property QtObject app: null

    color: Style.surfaceCard
    radius: theme.radius
    border.width: Style.hairline
    border.color: Style.borderSubtle

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Style.space12
        spacing: Style.space10

        RowLayout {
            Layout.fillWidth: true

            Text {
                text: qsTr("Environments")
                color: theme.muted
                font.family: Style.fontSans
                font.pixelSize: Style.font11
                font.weight: Font.Medium
            }

            Item { Layout.fillWidth: true }

            Text {
                text: qsTr("↑/↓ select · Space enter")
                color: theme.muted
                font.family: Style.fontMono
                font.pixelSize: Style.font10
            }
        }

        ListView {
            id: envListView
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: Style.space6
            model: backend.environments

            delegate: Item {
                id: envRow
                required property var modelData
                required property int index
                width: envListView.width
                height: 68

                readonly property bool selected: backend.selectedIndex === index
                readonly property real diskRatio: (modelData.diskQuotaBytes || 0) > 0
                                                 ? (modelData.diskUsageBytes || 0) / modelData.diskQuotaBytes
                                                 : 0
                readonly property string networkLabel: modelData.networkMode === "offline"
                                                       ? qsTr("NET OFF")
                                                       : (modelData.networkMode === "allowlist"
                                                          ? qsTr("ACL") + " · " + (modelData.ipv4 || "")
                                                          : (modelData.ipv4 || ""))

                Rectangle {
                    anchors.fill: parent
                    radius: Math.min(Style.radius4, theme.radius)
                    color: envRow.selected ? Style.surfaceSelected : Style.surfaceInput
                    border.width: Style.hairline
                    border.color: envRow.selected ? theme.accent : Style.borderSubtle

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: Style.space12
                        spacing: Style.space7

                        // State dot, name, badges
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Style.space8

                            Rectangle {
                                width: 8
                                height: 8
                                radius: 4
                                color: modelData.running ? theme.green : theme.muted
                            }

                            Text {
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                                text: modelData.name
                                color: theme.foreground
                                font.family: Style.fontMono
                                font.pixelSize: Style.font13
                                font.weight: Font.Medium
                            }

                            TagBadge {
                                visible: modelData.isTemplate || false
                                text: qsTr("TEMPLATE")
                                tone: "accent"
                            }

                            TagBadge {
                                text: modelData.status
                                tone: modelData.running ? "success" : "neutral"
                            }
                        }

                        // Disk and memory usage, network endpoint
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Style.space12

                            RowLayout {
                                spacing: Style.space5

                                Text {
                                    text: qsTr("DISK")
                                    color: theme.muted
                                    font.family: Style.fontMono
                                    font.pixelSize: Style.font8
                                    font.letterSpacing: 1
                                    Layout.alignment: Qt.AlignVCenter
                                }

                                Text {
                                    text: app.sizePair(modelData.diskUsage, modelData.diskLimit)
                                    color: envRow.diskRatio >= 0.9 ? theme.red
                                         : (envRow.diskRatio >= 0.75 ? theme.yellow : theme.foreground)
                                    opacity: envRow.diskRatio >= 0.75 ? 1 : Style.opacityDeemphasized
                                    font.family: Style.fontMono
                                    font.pixelSize: Style.font10
                                    Layout.alignment: Qt.AlignVCenter
                                }
                            }

                            RowLayout {
                                spacing: Style.space5

                                Text {
                                    text: qsTr("RAM")
                                    color: theme.muted
                                    font.family: Style.fontMono
                                    font.pixelSize: Style.font8
                                    font.letterSpacing: 1
                                    Layout.alignment: Qt.AlignVCenter
                                }

                                Text {
                                    text: app.sizePair(modelData.memoryUsage, modelData.memoryLimit)
                                    color: theme.foreground
                                    opacity: Style.opacityDeemphasized
                                    font.family: Style.fontMono
                                    font.pixelSize: Style.font10
                                    Layout.alignment: Qt.AlignVCenter
                                }
                            }

                            Text {
                                Layout.fillWidth: true
                                Layout.minimumWidth: 0
                                horizontalAlignment: Text.AlignRight
                                elide: Text.ElideRight
                                text: envRow.networkLabel
                                color: modelData.networkMode === "offline" ? theme.red : theme.accent
                                font.family: Style.fontMono
                                font.pixelSize: Style.font10
                                Layout.alignment: Qt.AlignVCenter
                            }
                        }
                    }

                    MouseArea {
                        id: envRowMouse
                        anchors.fill: parent
                        preventStealing: true
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: backend.selectedIndex = index
                        onDoubleClicked: backend.enterEnvironment(modelData.name)
                        ToolTip.delay: 700
                        ToolTip.visible: envRowMouse.containsMouse
                        ToolTip.text: qsTr("Disk %1 / %2 · RAM %3 / %4 · %5")
                                          .arg(modelData.diskUsage || "—")
                                          .arg(modelData.diskLimit || "—")
                                          .arg(modelData.memoryUsage || "—")
                                          .arg(modelData.memoryLimit || "—")
                                          .arg(envRow.networkLabel)
                    }
                }
            }
        }

        // Storage pool summary
        Rectangle {
            visible: backend.poolInfo.valid || false
            Layout.fillWidth: true
            implicitHeight: poolSidebarRow.implicitHeight + Style.space12
            radius: Math.min(Style.radius4, theme.radius)
            color: Style.surfaceInput
            border.width: Style.hairline
            border.color: backend.poolInfo.alertLevel === 2
                          ? theme.red
                          : (backend.poolInfo.alertLevel === 1 ? Style.warning : Style.borderSubtle)

            RowLayout {
                id: poolSidebarRow
                anchors.fill: parent
                anchors.margins: Style.space6
                anchors.leftMargin: Style.space10
                anchors.rightMargin: Style.space10
                spacing: Style.space6

                Text {
                    Layout.fillWidth: true
                    elide: Text.ElideRight
                    text: qsTr("Pool %1: %2 / %3 (%4%)")
                              .arg(backend.poolInfo.name || "default")
                              .arg(backend.poolInfo.usedText || "0 MiB")
                              .arg(backend.poolInfo.totalText || "0 MiB")
                              .arg(backend.poolInfo.usedPercent !== undefined ? backend.poolInfo.usedPercent : 0)
                    color: backend.poolInfo.alertLevel === 2
                           ? theme.red
                           : (backend.poolInfo.alertLevel === 1 ? Style.warning : theme.muted)
                    font.family: Style.fontMono
                    font.pixelSize: Style.font10
                }

                Text {
                    visible: (backend.poolInfo.allocatedBytes || 0) > 0
                    text: qsTr("Alloc %1").arg(backend.poolInfo.allocatedText || "0 MiB")
                    color: (backend.poolInfo.overcommitted || false) ? Style.warning : theme.muted
                    font.family: Style.fontMono
                    font.pixelSize: Style.font10
                }

                Text {
                    text: qsTr("Manage")
                    color: theme.accent
                    font.family: Style.fontSans
                    font.pixelSize: Style.font10
                }
            }

            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: app.openPoolSettings()
            }
        }

        ActionButton {
            Layout.fillWidth: true
            label: qsTr("Stale & Orphans Report")
            shortcutHint: "U"
            onClicked: app.openStaleReportDialog()
        }
    }
}
