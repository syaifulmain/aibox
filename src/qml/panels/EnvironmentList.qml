import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import "../components"

// Left pane of the Environments tab: the environment list, pool summary and
// the stale-report entry point.
Rectangle {
    id: listRoot
    property QtObject app: null

    color: theme.surfaceInput
    radius: theme.radius
    border.width: Style.hairline
    border.color: theme.borderSubtle

    Behavior on color {
        enabled: !theme.reduceMotion
        ColorAnimation { duration: theme.reduceMotion ? 0 : 150 }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Style.padCard
        spacing: Style.gap

        SectionHeader {
            title: qsTr("Environments")
        }

        ListView {
            id: envListView
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            reuseItems: true
            spacing: Style.gap
            model: backend.environments

            delegate: Item {
                id: envRow
                required property var modelData
                required property int index
                width: envListView.width
                height: Style.listRowHeight

                readonly property bool selected: backend.selectedIndex === index
                readonly property real diskRatio: (modelData.diskQuotaBytes || 0) > 0
                                                 ? (modelData.diskUsageBytes || 0) / modelData.diskQuotaBytes
                                                 : 0
                readonly property string usageSummary: app.sizePair(modelData.diskUsage, modelData.diskLimit)
                                                      + "  ·  "
                                                      + app.sizePair(modelData.memoryUsage, modelData.memoryLimit)
                readonly property string networkLabel: modelData.networkMode === "offline"
                                                       ? qsTr("NET OFF")
                                                       : (modelData.networkMode === "allowlist"
                                                          ? qsTr("ACL") + " · " + (modelData.ipv4 || "")
                                                          : (modelData.ipv4 || ""))

                Rectangle {
                    anchors.fill: parent
                    radius: Math.min(Style.radiusMd, theme.radius)
                    color: envRow.selected
                           ? theme.surfaceSelected
                           : (envRowMouse.containsMouse ? theme.surfaceRaised : theme.surfaceInput)
                    border.width: Style.hairline
                    border.color: envRow.selected
                                  ? theme.accentBorder
                                  : (envRowMouse.containsMouse ? theme.borderStrong : theme.borderSubtle)

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: Style.padCard
                        spacing: Style.gapMd

                        // State dot, name, badges
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Style.gapMd

                            Rectangle {
                                width: Style.gapMd
                                height: Style.gapMd
                                radius: width / 2
                                color: modelData.running ? theme.success : theme.textMuted
                            }

                            Text {
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                                text: modelData.name
                                color: theme.textPrimary
                                font.family: Style.fontMono
                                font.pixelSize: Style.fontBody
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
                            spacing: Style.gap

                            Text {
                                text: envRow.usageSummary
                                color: envRow.diskRatio >= 0.9 ? theme.dangerText
                                     : (envRow.diskRatio >= 0.75 ? theme.warningText : theme.textMuted)
                                font.family: Style.fontMono
                                font.pixelSize: Style.fontCaption
                                Layout.alignment: Qt.AlignVCenter
                            }

                            Text {
                                Layout.fillWidth: true
                                Layout.minimumWidth: 0
                                horizontalAlignment: Text.AlignRight
                                elide: Text.ElideRight
                                text: envRow.networkLabel
                                color: modelData.networkMode === "offline" ? theme.dangerText : theme.accentText
                                font.family: Style.fontMono
                                font.pixelSize: Style.fontCaption
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
                    }
                }
            }
        }

        // Storage pool summary
        Item {
            visible: backend.poolInfo.valid || false
            Layout.fillWidth: true
            implicitHeight: poolCard.implicitHeight

            Card {
                id: poolCard
                anchors.fill: parent
                showHeader: false
                fillColor: theme.surfaceInput
                borderColor: backend.poolInfo.alertLevel === 2
                             ? theme.dangerBorder
                             : (backend.poolInfo.alertLevel === 1 ? theme.warningBorder : theme.borderSubtle)
                cardPadding: Style.padCard
                contentSpacing: Style.gapSm

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Style.gapMd

                    Text {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: qsTr("Storage Pool")
                        color: theme.textSecondary
                        font.family: Style.fontSans
                        font.pixelSize: Style.fontSmall
                    }

                    Text {
                        text: (backend.poolInfo.usedPercent !== undefined
                               ? backend.poolInfo.usedPercent
                               : 0) + "%"
                        color: backend.poolInfo.alertLevel === 2
                               ? theme.dangerText
                               : (backend.poolInfo.alertLevel === 1 ? theme.warningText : theme.accentText)
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontCaption
                        font.weight: Font.Medium
                    }

                    Text {
                        text: qsTr("Manage")
                        color: theme.accentText
                        font.family: Style.fontSans
                        font.pixelSize: Style.fontSmall
                    }
                }

                MeterBar {
                    Layout.fillWidth: true
                    value: backend.poolInfo.usedPercent !== undefined ? backend.poolInfo.usedPercent : 0
                    minFillWidth: Style.gapSm
                    fillColor: backend.poolInfo.alertLevel === 2
                               ? theme.danger
                               : (backend.poolInfo.alertLevel === 1 ? theme.warning : theme.accent)
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Style.gapSm

                    Text {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: qsTr("%1 / %2 used")
                                  .arg(backend.poolInfo.usedText || "0 MiB")
                                  .arg(backend.poolInfo.totalText || "0 MiB")
                        color: theme.textMuted
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontCaption
                    }

                    Text {
                        visible: (backend.poolInfo.allocatedBytes || 0) > 0
                        elide: Text.ElideRight
                        text: qsTr("Allocated %1").arg(backend.poolInfo.allocatedText || "0 MiB")
                        color: (backend.poolInfo.overcommitted || false) ? theme.warningText : theme.textMuted
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontCaption
                    }
                }
            }

            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: app.openPoolSettings()
            }
        }
    }
}
