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
                readonly property string usageSummary: (listRoot.app
                                                       ? listRoot.app.sizePair(modelData.diskUsage, modelData.diskLimit)
                                                         + "  ·  "
                                                         + listRoot.app.sizePair(modelData.memoryUsage, modelData.memoryLimit)
                                                       : "")
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
                                visible: (modelData.bootAutostart || false) && !(modelData.isTemplate || false)
                                text: qsTr("BOOT")
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
                        onDoubleClicked: {
                            if (backend.busy)
                                return
                            if (modelData.isTemplate) {
                                if (listRoot.app)
                                    listRoot.app.openCreateDialog("", modelData.name)
                            } else {
                                backend.enterEnvironment(modelData.name)
                            }
                        }
                    }
                }
            }
        }

        // Storage pool summary
        Item {
            id: poolSummary
            readonly property var pool: backend.poolInfo
            visible: pool.valid || false
            Layout.fillWidth: true
            implicitHeight: poolCard.implicitHeight

            Card {
                id: poolCard
                anchors.fill: parent
                showHeader: false
                fillColor: theme.surfaceInput
                borderColor: poolSummary.pool.alertLevel === 2
                             ? theme.dangerBorder
                             : (poolSummary.pool.alertLevel === 1 ? theme.warningBorder : theme.borderSubtle)
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
                        text: (poolSummary.pool.usedPercent !== undefined
                               ? poolSummary.pool.usedPercent
                               : 0) + "%"
                        color: poolSummary.pool.alertLevel === 2
                               ? theme.dangerText
                               : (poolSummary.pool.alertLevel === 1 ? theme.warningText : theme.accentText)
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
                    value: poolSummary.pool.usedPercent !== undefined ? poolSummary.pool.usedPercent : 0
                    minFillWidth: Style.gapSm
                    fillColor: poolSummary.pool.alertLevel === 2
                               ? theme.danger
                               : (poolSummary.pool.alertLevel === 1 ? theme.warning : theme.accent)
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Style.gapSm

                    Text {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: qsTr("%1 / %2 used")
                                  .arg(poolSummary.pool.usedText || "0 MiB")
                                  .arg(poolSummary.pool.totalText || "0 MiB")
                        color: theme.textMuted
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontCaption
                    }

                    Text {
                        visible: (poolSummary.pool.allocatedBytes || 0) > 0
                        elide: Text.ElideRight
                        text: qsTr("Allocated %1").arg(poolSummary.pool.allocatedText || "0 MiB")
                        color: (poolSummary.pool.overcommitted || false) ? theme.warningText : theme.textMuted
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontCaption
                    }
                }
            }

            MouseArea {
                anchors.fill: parent
                enabled: !backend.busy && !backend.firstRun
                cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                onClicked: app.openPoolSettings()
            }
        }
    }
}
