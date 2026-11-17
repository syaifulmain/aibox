import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts
import "../components"

Item {
    property QtObject app: null
    id: mainContentRoot
    Layout.fillWidth: true
    Layout.fillHeight: true

    // --------------------------------------------------------------
    // TAB 0: ENVIRONMENTS (MASTER-DETAIL VIEW)
    // --------------------------------------------------------------
    RowLayout {
        anchors.fill: parent
        anchors.margins: 20
        spacing: 16
        visible: app.activeTab === 0

        // EMPTY STATE WHEN NO ENVIRONMENTS EXIST
        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: backend.environments.length === 0
            color: app.surfaceCard
            radius: theme.radius
            border.width: 1
            border.color: app.borderSubtle

            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(parent.width - 80, 480)
                spacing: 12

                Rectangle {
                    Layout.alignment: Qt.AlignHCenter
                    width: 42
                    height: 42
                    radius: Math.min(6, theme.radius)
                    color: app.surfaceInput
                    border.width: 1
                    border.color: app.borderSubtle
                    Text {
                        anchors.centerIn: parent
                        text: ">_"
                        color: theme.accent
                        font.family: app.monoFont
                        font.pixelSize: 16
                    }
                }

                Text {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    text: qsTr("No Isolated Environments Yet")
                    color: theme.foreground
                    font.family: app.sansFont
                    font.pixelSize: 16
                    font.weight: Font.Medium
                }

                Text {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.WordWrap
                    lineHeight: 1.35
                    text: qsTr("Create a clean Linux OS container or VM for your next AI, Desktop, or Mobile project. Give AI full root access inside, share files via %1, and wipe it in 1 second when done.").arg(backend.containerWorkspacePath)
                    color: theme.muted
                    font.family: app.sansFont
                    font.pixelSize: 12
                }

                RowLayout {
                    Layout.alignment: Qt.AlignHCenter
                    Layout.topMargin: 8
                    spacing: 10

                    ActionButton {
                        app: mainContentRoot.app
                        label: qsTr("+ Create First Environment")
                        shortcutHint: "N"
                        primary: true
                        disabled: backend.busy
                        onClicked: app.openCreateDialog("")
                    }

                    ActionButton {
                        app: mainContentRoot.app
                        label: qsTr("Browse OS Images")
                        shortcutHint: "2"
                        onClicked: app.activeTab = 1
                    }

                    ActionButton {
                        app: mainContentRoot.app
                        label: qsTr("Stale & Orphans")
                        shortcutHint: "U"
                        onClicked: app.openStaleReportDialog()
                    }
                }
            }
        }

        // LEFT PANE: ENVIRONMENT LIST
        Rectangle {
            Layout.preferredWidth: 352
            Layout.fillHeight: true
            visible: backend.environments.length > 0
            color: app.surfaceCard
            radius: theme.radius
            border.width: 1
            border.color: app.borderSubtle

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 12
                spacing: 10

                RowLayout {
                    Layout.fillWidth: true
                    Text {
                        text: qsTr("Environments")
                        color: theme.muted
                        font.family: app.sansFont
                        font.pixelSize: 11
                        font.weight: Font.Medium
                    }
                    Item { Layout.fillWidth: true }
                    Text {
                        text: qsTr("↑/↓ select · Space enter")
                        color: theme.muted
                        font.family: app.monoFont
                        font.pixelSize: 10
                    }
                }

                ListView {
                    id: envListView
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 6
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
                            radius: Math.min(4, theme.radius)
                            color: envRow.selected ? app.surfaceSelected : app.surfaceInput
                            border.width: 1
                            border.color: envRow.selected ? theme.accent : app.borderSubtle

                            ColumnLayout {
                                anchors.fill: parent
                                anchors.margins: 12
                                spacing: 7

                                // Row 1: state dot, name and badges
                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 8

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
                                        font.family: app.monoFont
                                        font.pixelSize: 13
                                        font.weight: Font.Medium
                                    }

                                    Rectangle {
                                        visible: modelData.isTemplate || false
                                        height: 18
                                        width: tplListBadgeText.implicitWidth + 10
                                        radius: 2
                                        color: app.surfaceSelected
                                        border.width: 1
                                        border.color: theme.accent
                                        Text {
                                            id: tplListBadgeText
                                            anchors.centerIn: parent
                                            text: qsTr("TEMPLATE")
                                            color: theme.accent
                                            font.family: app.monoFont
                                            font.pixelSize: 9
                                        }
                                    }

                                    Rectangle {
                                        height: 18
                                        width: statusBadgeText.implicitWidth + 10
                                        radius: 2
                                        color: modelData.running ? "#162920" : app.surfaceCard
                                        border.width: 1
                                        border.color: modelData.running ? "#284d3a" : app.borderSubtle
                                        Text {
                                            id: statusBadgeText
                                            anchors.centerIn: parent
                                            text: modelData.status
                                            color: modelData.running ? theme.green : theme.muted
                                            font.family: app.monoFont
                                            font.pixelSize: 9
                                        }
                                    }
                                }

                                // Row 2: disk / memory usage and network endpoint
                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 12

                                    RowLayout {
                                        spacing: 5

                                        Text {
                                            text: qsTr("DISK")
                                            color: theme.muted
                                            font.family: app.monoFont
                                            font.pixelSize: 8
                                            font.letterSpacing: 1
                                            Layout.alignment: Qt.AlignVCenter
                                        }

                                        Text {
                                            text: app.sizePair(modelData.diskUsage, modelData.diskLimit)
                                            color: envRow.diskRatio >= 0.9 ? theme.red
                                                 : (envRow.diskRatio >= 0.75 ? theme.yellow : theme.foreground)
                                            opacity: envRow.diskRatio >= 0.75 ? 1 : 0.72
                                            font.family: app.monoFont
                                            font.pixelSize: 10
                                            Layout.alignment: Qt.AlignVCenter
                                        }
                                    }

                                    RowLayout {
                                        spacing: 5

                                        Text {
                                            text: qsTr("RAM")
                                            color: theme.muted
                                            font.family: app.monoFont
                                            font.pixelSize: 8
                                            font.letterSpacing: 1
                                            Layout.alignment: Qt.AlignVCenter
                                        }

                                        Text {
                                            text: app.sizePair(modelData.memoryUsage, modelData.memoryLimit)
                                            color: theme.foreground
                                            opacity: 0.72
                                            font.family: app.monoFont
                                            font.pixelSize: 10
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
                                        font.family: app.monoFont
                                        font.pixelSize: 10
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

                Rectangle {
                    visible: backend.poolInfo.valid || false
                    Layout.fillWidth: true
                    implicitHeight: poolSidebarRow.implicitHeight + 12
                    radius: Math.min(4, theme.radius)
                    color: app.surfaceInput
                    border.width: 1
                    border.color: (backend.poolInfo.alertLevel === 2)
                                  ? theme.red
                                  : (backend.poolInfo.alertLevel === 1 ? "#e5a93c" : app.borderSubtle)

                    RowLayout {
                        id: poolSidebarRow
                        anchors.fill: parent
                        anchors.margins: 6
                        anchors.leftMargin: 10
                        anchors.rightMargin: 10
                        spacing: 6

                        Text {
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                            text: qsTr("Pool %1: %2 / %3 (%4%)")
                                      .arg(backend.poolInfo.name || "default")
                                      .arg(backend.poolInfo.usedText || "0 MiB")
                                      .arg(backend.poolInfo.totalText || "0 MiB")
                                      .arg(backend.poolInfo.usedPercent !== undefined ? backend.poolInfo.usedPercent : 0)
                            color: (backend.poolInfo.alertLevel === 2)
                                   ? theme.red
                                   : (backend.poolInfo.alertLevel === 1 ? "#e5a93c" : theme.muted)
                            font.family: app.monoFont
                            font.pixelSize: 10
                        }

                        Text {
                            visible: (backend.poolInfo.allocatedBytes || 0) > 0
                            text: qsTr("Alloc %1").arg(backend.poolInfo.allocatedText || "0 MiB")
                            color: (backend.poolInfo.overcommitted || false) ? "#e5a93c" : theme.muted
                            font.family: app.monoFont
                            font.pixelSize: 10
                        }

                        Text {
                            text: qsTr("Manage")
                            color: theme.accent
                            font.family: app.sansFont
                            font.pixelSize: 10
                        }
                    }

                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: app.openPoolSettings()
                    }
                }

                ActionButton {
                    app: mainContentRoot.app
                    Layout.fillWidth: true
                    label: qsTr("Stale & Orphans Report")
                    shortcutHint: "U"
                    onClicked: app.openStaleReportDialog()
                }
            }
        }

        // RIGHT PANE: SELECTED ENVIRONMENT INSPECTOR & CONTROLS
        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: backend.environments.length > 0
            color: app.surfaceCard
            radius: theme.radius
            border.width: 1
            border.color: app.borderSubtle

            ScrollView {
                id: inspectorScroll
                anchors.fill: parent
                anchors.margins: 20
                clip: true
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

            ColumnLayout {
                width: inspectorScroll.availableWidth
                height: Math.max(inspectorScroll.availableHeight, implicitHeight)
                spacing: 12

                // Environment Header & Primary Action
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 16

                    ColumnLayout {
                        spacing: 4
                        Layout.alignment: Qt.AlignVCenter

                        RowLayout {
                            spacing: 10
                            Layout.alignment: Qt.AlignVCenter

                            Text {
                                id: selectedEnvTitle
                                text: backend.selectedEnvironment.name || ""
                                color: theme.foreground
                                font.family: app.monoFont
                                font.pixelSize: 18
                                font.weight: Font.Medium
                                elide: Text.ElideRight
                                Layout.maximumWidth: 340
                                Layout.alignment: Qt.AlignVCenter
                            }

                            Rectangle {
                                visible: backend.selectedEnvironment.isTemplate || false
                                implicitHeight: 22
                                implicitWidth: tplHeaderBadgeText.implicitWidth + 14
                                Layout.preferredHeight: 22
                                Layout.preferredWidth: implicitWidth
                                Layout.alignment: Qt.AlignVCenter
                                radius: 3
                                color: app.surfaceSelected
                                border.width: 1
                                border.color: theme.accent

                                Text {
                                    id: tplHeaderBadgeText
                                    anchors.centerIn: parent
                                    text: qsTr("TEMPLATE")
                                    color: theme.accent
                                    font.family: app.monoFont
                                    font.pixelSize: 10
                                    font.weight: Font.Medium
                                }
                            }

                            Rectangle {
                                implicitHeight: 22
                                implicitWidth: statusBadgeRow.implicitWidth + 16
                                Layout.preferredHeight: 22
                                Layout.preferredWidth: implicitWidth
                                Layout.alignment: Qt.AlignVCenter
                                radius: 3
                                color: (backend.selectedEnvironment.running || false) ? "#162920" : app.surfaceInput
                                border.width: 1
                                border.color: (backend.selectedEnvironment.running || false) ? "#284d3a" : app.borderSubtle

                                RowLayout {
                                    id: statusBadgeRow
                                    anchors.centerIn: parent
                                    spacing: 6

                                    Rectangle {
                                        width: 6
                                        height: 6
                                        radius: 3
                                        color: (backend.selectedEnvironment.running || false) ? theme.green : theme.muted
                                        Layout.alignment: Qt.AlignVCenter
                                    }

                                    Text {
                                        id: detailStatusText
                                        text: backend.selectedEnvironment.status || ""
                                        color: (backend.selectedEnvironment.running || false) ? theme.green : theme.muted
                                        font.family: app.monoFont
                                        font.pixelSize: 10
                                        font.weight: Font.Medium
                                        Layout.alignment: Qt.AlignVCenter
                                    }
                                }
                            }

                            Rectangle {
                                readonly property int secCount: (backend.selectedEnvironment.secretNames || []).length
                                visible: secCount > 0
                                implicitHeight: 22
                                implicitWidth: secHeaderBadgeText.implicitWidth + 14
                                Layout.preferredHeight: 22
                                Layout.preferredWidth: implicitWidth
                                Layout.alignment: Qt.AlignVCenter
                                radius: 3
                                color: app.surfaceInput
                                border.width: 1
                                border.color: theme.accent

                                Text {
                                    id: secHeaderBadgeText
                                    anchors.centerIn: parent
                                    text: qsTr("SECRETS: %1").arg((backend.selectedEnvironment.secretNames || []).length)
                                    color: theme.accent
                                    font.family: app.monoFont
                                    font.pixelSize: 10
                                    font.weight: Font.Medium
                                }

                                MouseArea {
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: app.openSecretsDialog()
                                }
                            }
                        }

                        Text {
                            readonly property string pidStr: backend.selectedEnvironment.processLimit || ""
                            readonly property string ioStr: backend.selectedEnvironment.diskWriteLimit || backend.selectedEnvironment.diskReadLimit || ""
                            readonly property string limitSuffix: (pidStr.length > 0 ? "  ·  Pids " + pidStr : "")
                                                                  + (ioStr.length > 0 ? "  ·  I/O " + ioStr : "")
                            text: (backend.selectedEnvironment.osName || "") + "  ·  " + (backend.selectedEnvironment.preset || "") + limitSuffix
                            color: theme.muted
                            font.family: app.sansFont
                            font.pixelSize: 12
                        }
                    }

                    Item { Layout.fillWidth: true }

                    ActionButton {
                        app: mainContentRoot.app
                        label: (backend.selectedEnvironment.isTemplate || false)
                               ? qsTr("New from Template")
                               : qsTr("Enter OS Terminal")
                        shortcutHint: (backend.selectedEnvironment.isTemplate || false) ? "" : "Space"
                        variant: "primary"
                        minWidth: 165
                        disabled: backend.busy
                        Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                        onClicked: {
                            const env = backend.selectedEnvironment
                            if (env.isTemplate)
                                app.openCreateDialog("", env.name || "")
                            else
                                backend.enterEnvironment(env.name || "")
                        }
                    }
                }

                // Secondary Action Bar
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 10

                    ActionButton {
                        app: mainContentRoot.app
                        label: (backend.selectedEnvironment.running || false) ? qsTr("Stop OS") : qsTr("Start OS")
                        shortcutHint: "S"
                        variant: (backend.selectedEnvironment.running || false) ? "danger" : "accent"
                        minWidth: 98
                        disabled: backend.busy || (backend.selectedEnvironment.isTemplate || false)
                        Layout.alignment: Qt.AlignVCenter
                        onClicked: backend.toggleStartStop(backend.selectedEnvironment.name || "")
                    }

                    ActionButton {
                        app: mainContentRoot.app
                        label: qsTr("Clone")
                        shortcutHint: "D"
                        minWidth: 86
                        disabled: backend.busy
                        Layout.alignment: Qt.AlignVCenter
                        onClicked: app.openCloneDialog(backend.selectedEnvironment.name || "")
                    }

                    ActionButton {
                        app: mainContentRoot.app
                        label: qsTr("Quotas")
                        shortcutHint: "E"
                        minWidth: 92
                        disabled: backend.busy
                        Layout.alignment: Qt.AlignVCenter
                        onClicked: app.openEditQuotasDialog()
                    }

                    ActionButton {
                        app: mainContentRoot.app
                        label: (backend.selectedEnvironment.isTemplate || false) ? qsTr("Unmark Template") : qsTr("Template")
                        shortcutHint: "T"
                        variant: (backend.selectedEnvironment.isTemplate || false) ? "accent" : ""
                        minWidth: 105
                        disabled: backend.busy
                        Layout.alignment: Qt.AlignVCenter
                        onClicked: backend.setEnvironmentTemplate(backend.selectedEnvironment.name || "", !(backend.selectedEnvironment.isTemplate || false))
                    }

                    ActionButton {
                        app: mainContentRoot.app
                        label: qsTr("Copy AI Command")
                        shortcutHint: "C"
                        minWidth: 142
                        disabled: backend.selectedEnvironment.isTemplate || false
                        Layout.alignment: Qt.AlignVCenter
                        onClicked: backend.copyAiCommand(backend.selectedEnvironment.name || "")
                    }

                    ActionButton {
                        app: mainContentRoot.app
                        label: qsTr("Paste Image")
                        shortcutHint: "I"
                        minWidth: 112
                        disabled: backend.selectedEnvironment.isTemplate || false
                        Layout.alignment: Qt.AlignVCenter
                        onClicked: backend.pasteImageToWorkspace(backend.selectedEnvironment.name || "")
                    }

                    Item { Layout.fillWidth: true }

                    ActionButton {
                        app: mainContentRoot.app
                        label: qsTr("Wipe OS")
                        shortcutHint: "Del"
                        variant: "danger"
                        minWidth: 96
                        disabled: backend.busy
                        Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                        onClicked: {
                            app.deleteWorkspaceOnWipe = false
                            app.wipeVisible = true
                        }
                    }
                }

                // 3 Hardware Resource Monitor Cards (Task Manager Style)
                GridLayout {
                    Layout.fillWidth: true
                    columns: 3
                    columnSpacing: 10
                    rowSpacing: 10

                    Repeater {
                        model: [
                            {
                                title: qsTr("Disk Usage"),
                                val: (backend.selectedEnvironment.diskUsage || "0 MB") + " / " + (backend.selectedEnvironment.diskLimit || "—"),
                                subtext: "",
                                subAlert: 0,
                                pct: backend.selectedEnvironment.diskPercent !== undefined ? backend.selectedEnvironment.diskPercent : 0,
                                badge: (backend.selectedEnvironment.diskAlertLevel === 2
                                        ? qsTr("CRIT ")
                                        : (backend.selectedEnvironment.diskAlertLevel === 1 ? qsTr("WARN ") : ""))
                                       + (backend.selectedEnvironment.diskPercent !== undefined ? backend.selectedEnvironment.diskPercent : 0) + "%",
                                hasBar: true,
                                isCpu: false,
                                isNetwork: false,
                                alertLevel: backend.selectedEnvironment.diskAlertLevel !== undefined ? backend.selectedEnvironment.diskAlertLevel : 0,
                                cores: [],
                                active: true
                            },
                            {
                                title: qsTr("Memory Usage"),
                                val: (backend.selectedEnvironment.memoryUsage || "0 MB") + " / " + (backend.selectedEnvironment.memoryLimit || "—"),
                                subtext: "",
                                subAlert: 0,
                                pct: backend.selectedEnvironment.memoryPercent !== undefined ? backend.selectedEnvironment.memoryPercent : 0,
                                badge: (backend.selectedEnvironment.memoryPercent !== undefined ? backend.selectedEnvironment.memoryPercent : 0) + "%",
                                hasBar: true,
                                isCpu: false,
                                isNetwork: false,
                                alertLevel: 0,
                                cores: [],
                                active: backend.selectedEnvironment.running || false
                            },
                            {
                                title: qsTr("CPU Usage") + " · " + (backend.selectedEnvironment.cpuLimit || "—"),
                                val: (backend.selectedEnvironment.cpuUsage || "0%") + " · " + (backend.selectedEnvironment.cpuLimit || "—"),
                                subtext: "",
                                subAlert: 0,
                                pct: backend.selectedEnvironment.cpuPercent !== undefined ? backend.selectedEnvironment.cpuPercent : 0,
                                badge: backend.selectedEnvironment.cpuUsage || "0%",
                                hasBar: false,
                                isCpu: true,
                                isNetwork: false,
                                alertLevel: 0,
                                cores: backend.selectedEnvironment.cpuCorePercents || [0, 0, 0, 0],
                                active: backend.selectedEnvironment.running || false
                            }
                        ]
                        delegate: Rectangle {
                            id: statCard
                            required property var modelData
                            Layout.fillWidth: true
                            Layout.preferredHeight: 94
                            radius: Math.min(4, theme.radius)
                            color: app.surfaceInput
                            border.width: 1
                            border.color: statCard.modelData.alertLevel === 2
                                          ? theme.red
                                          : (statCard.modelData.alertLevel === 1 ? "#e5a93c" : app.borderSubtle)

                            ColumnLayout {
                                anchors.fill: parent
                                anchors.margins: 10
                                spacing: 4

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 6

                                    Text {
                                        Layout.fillWidth: true
                                        elide: Text.ElideRight
                                        text: statCard.modelData.title
                                        color: theme.muted
                                        font.family: app.sansFont
                                        font.pixelSize: 11
                                    }

                                    Text {
                                        text: statCard.modelData.badge
                                        color: statCard.modelData.alertLevel === 2
                                               ? theme.red
                                               : (statCard.modelData.alertLevel === 1
                                                  ? "#e5a93c"
                                                  : (statCard.modelData.active
                                                     ? (statCard.modelData.isNetwork ? theme.green : (statCard.modelData.pct >= 85 ? theme.red : theme.accent))
                                                     : theme.muted))
                                        font.family: app.monoFont
                                        font.pixelSize: 10
                                        font.weight: Font.Medium
                                    }
                                }

                                // Standard single-value row (Disk, Memory, IPv4)
                                Text {
                                    visible: !statCard.modelData.isCpu
                                    Layout.fillWidth: true
                                    elide: Text.ElideRight
                                    text: statCard.modelData.val
                                    color: theme.foreground
                                    font.family: app.monoFont
                                    font.pixelSize: 13
                                }

                                Text {
                                    visible: !statCard.modelData.isCpu && (statCard.modelData.subtext || "").length > 0
                                    Layout.fillWidth: true
                                    elide: Text.ElideRight
                                    text: statCard.modelData.subtext || ""
                                    color: statCard.modelData.subAlert === 2
                                           ? theme.red
                                           : (statCard.modelData.subAlert === 1 ? "#e5a93c" : theme.muted)
                                    font.family: app.monoFont
                                    font.pixelSize: 10
                                }

                                Item {
                                    visible: !statCard.modelData.isCpu
                                    Layout.fillHeight: true
                                }

                                Rectangle {
                                    visible: statCard.modelData.hasBar && !statCard.modelData.isCpu
                                    Layout.fillWidth: true
                                    height: 4
                                    radius: 2
                                    color: app.surfaceCard

                                    Rectangle {
                                        height: parent.height
                                        width: statCard.modelData.pct > 0
                                               ? Math.max(4, parent.width * (Math.min(100, statCard.modelData.pct) / 100.0))
                                               : 0
                                        radius: 2
                                        color: statCard.modelData.isNetwork
                                               ? (statCard.modelData.active ? theme.green : theme.muted)
                                               : (statCard.modelData.alertLevel === 2
                                                  ? theme.red
                                                  : (statCard.modelData.alertLevel === 1
                                                     ? "#e5a93c"
                                                     : (statCard.modelData.pct >= 85 ? theme.red : theme.accent)))
                                        Behavior on width { NumberAnimation { duration: 180 } }
                                    }
                                }

                                // Per-core CPU Task Manager Grid
                                GridLayout {
                                    visible: statCard.modelData.isCpu
                                    Layout.fillWidth: true
                                    Layout.fillHeight: true
                                    columns: statCard.modelData.cores.length > 4 ? 3 : 2
                                    columnSpacing: 8
                                    rowSpacing: 4

                                    Repeater {
                                        model: statCard.modelData.cores
                                        delegate: ColumnLayout {
                                            required property var modelData
                                            required property int index
                                            Layout.fillWidth: true
                                            spacing: 2

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 4
                                                Text {
                                                    text: "C" + (index + 1)
                                                    color: theme.muted
                                                    font.family: app.monoFont
                                                    font.pixelSize: 9
                                                }
                                                Item { Layout.fillWidth: true }
                                                Text {
                                                    text: modelData + "%"
                                                    color: statCard.modelData.active ? theme.foreground : theme.muted
                                                    font.family: app.monoFont
                                                    font.pixelSize: 9
                                                }
                                            }

                                            Rectangle {
                                                Layout.fillWidth: true
                                                height: 3
                                                radius: 1.5
                                                color: app.surfaceCard

                                                Rectangle {
                                                    height: parent.height
                                                    width: modelData > 0
                                                           ? Math.max(3, parent.width * (Math.min(100, modelData) / 100.0))
                                                           : 0
                                                    radius: 1.5
                                                    color: modelData >= 85 ? theme.red : theme.accent
                                                    Behavior on width { NumberAnimation { duration: 180 } }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Shared Folder Info Box
                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 50
                    radius: Math.min(4, theme.radius)
                    color: app.surfaceInput
                    border.width: 1
                    border.color: app.borderSubtle

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 14
                        anchors.rightMargin: 14
                        spacing: 10

                        Text {
                            text: qsTr("Shared Workspace")
                            color: theme.muted
                            font.family: app.sansFont
                            font.pixelSize: 11
                        }
                        Text {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            elide: Text.ElideMiddle
                            text: (backend.selectedEnvironment.workspacePath || "") + "  ⟷  " + backend.containerWorkspacePath
                            color: theme.accent
                            font.family: app.monoFont
                            font.pixelSize: 12
                        }

                        ActionButton {
                            app: mainContentRoot.app
                            label: (backend.selectedEnvironment.extraMountCount || 0) > 0
                                   ? qsTr("Mounts (%1)").arg(backend.selectedEnvironment.extraMountCount)
                                   : qsTr("Extra Mounts")
                            minWidth: 112
                            Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                            onClicked: app.openMountsDialog()
                        }

                        ActionButton {
                            app: mainContentRoot.app
                            label: qsTr("Open Shared Folder")
                            shortcutHint: "F"
                            minWidth: 165
                            Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                            onClicked: backend.openWorkspaceFolder(backend.selectedEnvironment.name || "")
                        }
                    }
                }

                // Root File Access (localhost-only SFTP) Info Box
                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 50
                    radius: Math.min(4, theme.radius)
                    color: app.surfaceInput
                    border.width: 1
                    border.color: (backend.selectedEnvironment.fileAccessActive || false)
                                  ? theme.accent
                                  : app.borderSubtle

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 14
                        anchors.rightMargin: 14
                        spacing: 10

                        Text {
                            text: qsTr("Root Files")
                            color: theme.muted
                            font.family: app.sansFont
                            font.pixelSize: 11
                            ToolTip.visible: rootLabelHover.hovered
                            ToolTip.delay: 500
                            ToolTip.text: qsTr("Browse this environment's root filesystem in your file manager over a localhost-only, passwordless SFTP connection. Anyone on this machine can read and write files as root while it is enabled.")
                            HoverHandler { id: rootLabelHover }
                        }

                        Text {
                            id: rootAccessText
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            elide: Text.ElideMiddle
                            text: (backend.selectedEnvironment.fileAccessActive || false)
                                  ? (backend.selectedEnvironment.fileAccessUrl || "")
                                  : (backend.selectedEnvironment.running
                                     ? qsTr("Off — browse the container root in your file manager")
                                     : qsTr("Off — start the environment to enable"))
                            color: (backend.selectedEnvironment.fileAccessActive || false)
                                   ? theme.accent
                                   : theme.muted
                            font.family: app.monoFont
                            font.pixelSize: 12
                            ToolTip.visible: rootUrlHover.hovered && (backend.selectedEnvironment.fileAccessActive || false)
                            ToolTip.delay: 400
                            ToolTip.text: qsTr("Click to copy this SFTP URL")
                            HoverHandler {
                                id: rootUrlHover
                                cursorShape: (backend.selectedEnvironment.fileAccessActive || false)
                                             ? Qt.PointingHandCursor : Qt.ArrowCursor
                            }
                            TapHandler {
                                enabled: backend.selectedEnvironment.fileAccessActive || false
                                onTapped: backend.copyText(
                                    backend.selectedEnvironment.fileAccessUrl || "",
                                    qsTr("SFTP URL"))
                            }
                        }

                        ActionButton {
                            app: mainContentRoot.app
                            visible: backend.selectedEnvironment.fileAccessActive || false
                            label: qsTr("Open")
                            minWidth: 78
                            onClicked: backend.openFileAccess(backend.selectedEnvironment.name || "")
                        }

                        ActionButton {
                            app: mainContentRoot.app
                            label: (backend.selectedEnvironment.fileAccessActive || false)
                                   ? qsTr("Disable")
                                   : qsTr("Enable")
                            variant: (backend.selectedEnvironment.fileAccessActive || false)
                                     ? "danger"
                                     : "accent"
                            minWidth: 92
                            disabled: backend.busy
                                      || (backend.selectedEnvironment.isTemplate || false)
                                      || (!(backend.selectedEnvironment.fileAccessActive || false)
                                          && !(backend.selectedEnvironment.running || false))
                            onClicked: {
                                const name = backend.selectedEnvironment.name || ""
                                if (backend.selectedEnvironment.fileAccessActive || false)
                                    backend.disableFileAccess(name)
                                else
                                    backend.enableFileAccess(name)
                            }
                        }
                    }
                }

                // Network Control Bar
                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: netCol.implicitHeight + 16
                    radius: Math.min(4, theme.radius)
                    color: app.surfaceInput
                    border.width: 1
                    border.color: backend.selectedEnvironment.networkMode === "offline"
                                  ? theme.red
                                  : (backend.selectedEnvironment.networkMode === "allowlist" ? theme.accent : app.borderSubtle)

                    ColumnLayout {
                        id: netCol
                        anchors.fill: parent
                        anchors.leftMargin: 14
                        anchors.rightMargin: 14
                        anchors.topMargin: 8
                        anchors.bottomMargin: 8
                        spacing: 10

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 10

                            Text {
                                text: qsTr("Network Mode")
                                color: theme.muted
                                font.family: app.sansFont
                                font.pixelSize: 11
                            }

                            Repeater {
                                model: [
                                    { id: "online", label: qsTr("Online") },
                                    { id: "allowlist", label: qsTr("Allowlist") },
                                    { id: "offline", label: qsTr("Offline") }
                                ]
                                delegate: Rectangle {
                                    required property var modelData
                                    readonly property string activeMode: backend.selectedEnvironment.networkMode || "online"
                                    readonly property bool selected: (modelData.id === "allowlist" && app.netAllowlistEditing)
                                                                     || (!app.netAllowlistEditing && activeMode === modelData.id)
                                    height: 28
                                    width: netChipText.implicitWidth + 20
                                    radius: Math.min(4, theme.radius)
                                    color: selected ? app.surfaceSelected : app.surfaceCard
                                    border.width: 1
                                    border.color: selected
                                                  ? (modelData.id === "offline" ? theme.red : theme.accent)
                                                  : app.borderSubtle
                                    opacity: backend.busy ? 0.5 : 1.0

                                    Text {
                                        id: netChipText
                                        anchors.centerIn: parent
                                        text: modelData.label
                                        color: selected ? theme.foreground : theme.muted
                                        font.family: app.sansFont
                                        font.pixelSize: 11
                                        font.weight: selected ? Font.Medium : Font.Normal
                                    }

                                    MouseArea {
                                        anchors.fill: parent
                                        enabled: !backend.busy && !(backend.selectedEnvironment.isTemplate || false)
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: {
                                            const name = backend.selectedEnvironment.name || ""
                                            if (modelData.id === "allowlist") {
                                                app.netAllowlistEditing = true
                                                if (app.netAllowlistDraft.length === 0)
                                                    app.netAllowlistDraft = backend.selectedEnvironment.networkAllowlist || ""
                                            } else {
                                                app.netAllowlistEditing = false
                                                backend.setNetworkMode(name, modelData.id, "")
                                            }
                                        }
                                    }
                                }
                            }

                            Item { Layout.fillWidth: true }

                            ActionButton {
                                app: mainContentRoot.app
                                label: (backend.selectedEnvironment.egressRecording || false)
                                       ? qsTr("Record: ON")
                                       : qsTr("Record: OFF")
                                variant: (backend.selectedEnvironment.egressRecording || false) ? "accent" : ""
                                minWidth: 104
                                disabled: backend.busy || (backend.selectedEnvironment.isTemplate || false)
                                          || backend.selectedEnvironment.networkMode === "offline"
                                onClicked: backend.setEgressRecording(
                                    backend.selectedEnvironment.name || "",
                                    !(backend.selectedEnvironment.egressRecording || false))
                            }

                            ActionButton {
                                app: mainContentRoot.app
                                label: (backend.selectedEnvironment.portForwardCount || 0) > 0
                                       ? qsTr("Ports (%1)").arg(backend.selectedEnvironment.portForwardCount)
                                       : qsTr("Ports")
                                minWidth: 86
                                disabled: backend.selectedEnvironment.isTemplate || false
                                onClicked: app.openPortsDialog()
                            }

                            ActionButton {
                                app: mainContentRoot.app
                                label: qsTr("Egress Log")
                                minWidth: 96
                                disabled: backend.selectedEnvironment.isTemplate || false
                                onClicked: app.openEgressDialog(backend.selectedEnvironment.name || "")
                            }

                            ActionButton {
                                app: mainContentRoot.app
                                label: backend.selectedEnvironment.networkMode === "offline"
                                       ? qsTr("Restore Online")
                                       : qsTr("Kill Network")
                                shortcutHint: "K"
                                variant: backend.selectedEnvironment.networkMode === "offline" ? "accent" : "danger"
                                minWidth: 132
                                disabled: backend.busy || (backend.selectedEnvironment.isTemplate || false)
                                onClicked: {
                                    const name = backend.selectedEnvironment.name || ""
                                    const nextMode = backend.selectedEnvironment.networkMode === "offline" ? "online" : "offline"
                                    app.netAllowlistEditing = false
                                    backend.setNetworkMode(name, nextMode, "")
                                }
                            }
                        }

                        RowLayout {
                            visible: app.netAllowlistEditing || backend.selectedEnvironment.networkMode === "allowlist"
                            Layout.fillWidth: true
                            spacing: 8

                            FlatInput {
                                app: mainContentRoot.app
                                id: allowlistInput
                                Layout.fillWidth: true
                                implicitHeight: 34
                                text: app.netAllowlistDraft
                                onTextChanged: app.netAllowlistDraft = text
                                placeholder: qsTr("Allowed domains or CIDRs (e.g. github.com, api.anthropic.com, 1.1.1.1)")
                                onAccepted: {
                                    if (!backend.busy)
                                        backend.setNetworkMode(backend.selectedEnvironment.name || "", "allowlist", app.netAllowlistDraft)
                                }
                            }

                            ActionButton {
                                app: mainContentRoot.app
                                label: qsTr("Apply ACL")
                                variant: "primary"
                                minWidth: 96
                                disabled: backend.busy || app.netAllowlistDraft.trim().length === 0 || (backend.selectedEnvironment.isTemplate || false)
                                onClicked: backend.setNetworkMode(backend.selectedEnvironment.name || "", "allowlist", app.netAllowlistDraft)
                            }

                            ActionButton {
                                app: mainContentRoot.app
                                visible: backend.selectedEnvironment.networkMode === "allowlist"
                                label: qsTr("Refresh IPs")
                                minWidth: 96
                                disabled: backend.busy || (backend.selectedEnvironment.isTemplate || false)
                                onClicked: backend.refreshNetworkAllowlist(backend.selectedEnvironment.name || "")
                            }
                        }
                    }
                }

                // Network Monitor (live ping, throughput & totals)
                Rectangle {
                    id: netMonitorCard
                    Layout.fillWidth: true
                    Layout.preferredHeight: netMonitorCol.implicitHeight + 20
                    radius: Math.min(4, theme.radius)
                    color: app.surfaceInput
                    border.width: 1
                    border.color: app.borderSubtle

                    readonly property var stats: backend.networkStats
                    readonly property bool live: stats.active === true
                    readonly property real pingMs: stats.pingMs !== undefined ? stats.pingMs : -1
                    readonly property real lossPct: stats.packetLossPercent !== undefined ? stats.packetLossPercent : -1

                    function pingText() {
                        if (!live || pingMs < 0)
                            return "—"
                        return (pingMs < 10 ? pingMs.toFixed(1) : Math.round(pingMs).toString()) + " ms"
                    }

                    function lossText() {
                        if (!live || lossPct < 0)
                            return "—"
                        return (lossPct % 1 === 0 ? lossPct.toFixed(0) : lossPct.toFixed(1)) + "%"
                    }

                    ColumnLayout {
                        id: netMonitorCol
                        anchors.fill: parent
                        anchors.margins: 14
                        spacing: 10

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8

                            Text {
                                text: qsTr("Network Monitor")
                                color: theme.foreground
                                font.family: app.sansFont
                                font.pixelSize: 12
                                font.weight: Font.Medium
                            }

                            Rectangle {
                                Layout.alignment: Qt.AlignVCenter
                                width: 6
                                height: 6
                                radius: 3
                                color: netMonitorCard.live ? theme.green : theme.muted
                            }

                            Text {
                                text: netMonitorCard.live
                                      ? qsTr("LIVE")
                                      : (backend.selectedEnvironment.networkMode === "offline"
                                         ? qsTr("NET OFF")
                                         : qsTr("IDLE"))
                                color: netMonitorCard.live ? theme.green : theme.muted
                                font.family: app.monoFont
                                font.pixelSize: 10
                                font.weight: Font.Medium
                                Layout.alignment: Qt.AlignVCenter
                            }

                            Item { Layout.fillWidth: true }

                            ActionButton {
                                app: mainContentRoot.app
                                label: qsTr("Probe")
                                minWidth: 72
                                implicitHeight: 26
                                disabled: backend.busy || !(backend.selectedEnvironment.running || false)
                                Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                                onClicked: backend.refreshNetworkStats()
                            }
                        }

                        // Two equal-width columns so labels and values line up
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 30

                            Repeater {
                                model: [
                                    [
                                        { label: qsTr("Ping"), value: netMonitorCard.pingText(), tone: "normal", copyValue: "" },
                                        { label: qsTr("Receiving"), value: netMonitorCard.stats.rxRateText || "0 KB/s", tone: "down", copyValue: "" },
                                        { label: qsTr("Downloaded"), value: netMonitorCard.stats.rxTotalText || "0 B", tone: "normal", copyValue: "" },
                                        { label: qsTr("IP Address"), value: netMonitorCard.stats.ip || "—", tone: "normal", copyValue: netMonitorCard.stats.ip || "" }
                                    ],
                                    [
                                        { label: qsTr("Packet Loss"), value: netMonitorCard.lossText(), tone: netMonitorCard.lossPct > 0 ? "warn" : "normal", copyValue: "" },
                                        { label: qsTr("Sending"), value: netMonitorCard.stats.txRateText || "0 KB/s", tone: "up", copyValue: "" },
                                        { label: qsTr("Uploaded"), value: netMonitorCard.stats.txTotalText || "0 B", tone: "normal", copyValue: "" },
                                        { label: qsTr("Gateway"), value: netMonitorCard.stats.gateway || "—", tone: "normal", copyValue: netMonitorCard.stats.gateway || "" }
                                    ]
                                ]

                                delegate: ColumnLayout {
                                    required property var modelData
                                    Layout.fillWidth: true
                                    Layout.preferredWidth: 0
                                    spacing: 8

                                    Repeater {
                                        model: modelData

                                        delegate: RowLayout {
                                            id: netRow
                                            required property var modelData

                                            readonly property bool copyable: (modelData.copyValue || "").length > 0
                                            property bool justCopied: false

                                            Layout.fillWidth: true
                                            Layout.minimumWidth: 0
                                            spacing: 10

                                            Text {
                                                text: modelData.label
                                                color: netRow.copyable && netHover.hovered ? theme.foreground : theme.muted
                                                font.family: app.sansFont
                                                font.pixelSize: 11
                                            }

                                            Item { Layout.fillWidth: true }

                                            Text {
                                                Layout.minimumWidth: 0
                                                elide: Text.ElideRight
                                                text: netRow.justCopied ? qsTr("Copied") : modelData.value
                                                color: netRow.justCopied
                                                       ? theme.accent
                                                       : (modelData.value === "—"
                                                          ? theme.muted
                                                          : (modelData.tone === "down"
                                                             ? theme.green
                                                             : (modelData.tone === "up"
                                                                ? theme.accent
                                                                : (modelData.tone === "warn" ? "#e5a93c" : theme.foreground))))
                                                font.family: app.monoFont
                                                font.pixelSize: 12
                                            }

                                            // Monochrome copy glyph: hover hint for copyable values
                                            Item {
                                                visible: netRow.copyable
                                                Layout.alignment: Qt.AlignVCenter
                                                Layout.preferredWidth: 10
                                                Layout.preferredHeight: 10
                                                opacity: netRow.justCopied ? 1.0 : (netHover.hovered ? 0.95 : 0.4)

                                                Rectangle {
                                                    x: 0
                                                    y: 3
                                                    width: 7
                                                    height: 7
                                                    radius: 1
                                                    color: "transparent"
                                                    border.width: 1
                                                    border.color: netRow.justCopied ? theme.accent : (netHover.hovered ? theme.accent : theme.muted)
                                                }
                                                Rectangle {
                                                    x: 3
                                                    y: 0
                                                    width: 7
                                                    height: 7
                                                    radius: 1
                                                    color: app.surfaceInput
                                                    border.width: 1
                                                    border.color: netRow.justCopied ? theme.accent : (netHover.hovered ? theme.accent : theme.muted)
                                                }
                                            }

                                            HoverHandler {
                                                id: netHover
                                                enabled: netRow.copyable
                                                cursorShape: Qt.PointingHandCursor
                                            }

                                            TapHandler {
                                                enabled: netRow.copyable
                                                onTapped: {
                                                    if (backend.copyText(netRow.modelData.copyValue, netRow.modelData.label)) {
                                                        netRow.justCopied = true
                                                        copyResetTimer.restart()
                                                    }
                                                }
                                            }

                                            Timer {
                                                id: copyResetTimer
                                                interval: 1200
                                                onTriggered: netRow.justCopied = false
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // SNAPSHOTS (TIME MACHINE) SECTION
                Rectangle {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.minimumHeight: 150
                    radius: Math.min(4, theme.radius)
                    color: app.surfaceInput
                    border.width: 1
                    border.color: app.borderSubtle

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 14
                        spacing: 10

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 10

                            ColumnLayout {
                                spacing: 2
                                Text {
                                    text: qsTr("Btrfs Snapshots (Instant Rollback)")
                                    color: theme.foreground
                                    font.family: app.sansFont
                                    font.pixelSize: 12
                                    font.weight: Font.Medium
                                }
                                Text {
                                    text: qsTr("Auto-snapshot runs before Enter & Start (keeps last %1)").arg(backend.autoSnapshotKeep)
                                    color: theme.muted
                                    font.family: app.sansFont
                                    font.pixelSize: 11
                                }
                            }

                            Item { Layout.fillWidth: true }

                            ActionButton {
                                app: mainContentRoot.app
                                implicitHeight: 34
                                minWidth: 112
                                label: (backend.selectedEnvironment.autoStopOnCritical || false)
                                       ? qsTr("Auto-stop: ON")
                                       : qsTr("Auto-stop: OFF")
                                variant: (backend.selectedEnvironment.autoStopOnCritical || false) ? "danger" : ""
                                disabled: backend.busy
                                Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                                onClicked: backend.setEnvironmentAutoStop(
                                    backend.selectedEnvironment.name || "",
                                    !(backend.selectedEnvironment.autoStopOnCritical || false))
                            }

                            ActionButton {
                                app: mainContentRoot.app
                                implicitHeight: 34
                                minWidth: 86
                                visible: backend.selectedEnvironment.workspaceIsSubvolume || false
                                label: (backend.selectedEnvironment.workspaceSnapshotEnabled !== false)
                                       ? qsTr("WS: ON")
                                       : qsTr("WS: OFF")
                                variant: (backend.selectedEnvironment.workspaceSnapshotEnabled !== false) ? "accent" : ""
                                disabled: backend.busy
                                Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                                onClicked: backend.setEnvironmentWorkspaceSnapshot(
                                    backend.selectedEnvironment.name || "",
                                    !(backend.selectedEnvironment.workspaceSnapshotEnabled !== false))
                            }

                            ActionButton {
                                app: mainContentRoot.app
                                implicitHeight: 34
                                minWidth: 92
                                label: (backend.selectedEnvironment.autoSnapshotEnabled !== false)
                                       ? qsTr("Auto: ON")
                                       : qsTr("Auto: OFF")
                                variant: (backend.selectedEnvironment.autoSnapshotEnabled !== false) ? "accent" : ""
                                disabled: backend.busy
                                Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                                onClicked: backend.setEnvironmentAutoSnapshot(
                                    backend.selectedEnvironment.name || "",
                                    !(backend.selectedEnvironment.autoSnapshotEnabled !== false))
                            }

                            FlatInput {
                                app: mainContentRoot.app
                                id: snapNameInput
                                Layout.preferredWidth: 165
                                Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                                implicitHeight: 34
                                placeholder: qsTr("Snapshot label (optional)")
                                onAccepted: {
                                    if (!backend.busy) {
                                        backend.createSnapshot(backend.selectedEnvironment.name || "", snapNameInput.text)
                                        snapNameInput.text = ""
                                    }
                                }
                            }

                            ActionButton {
                                app: mainContentRoot.app
                                implicitHeight: 34
                                minWidth: 138
                                label: qsTr("+ Save Snapshot")
                                shortcutHint: "P"
                                disabled: backend.busy
                                Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                                onClicked: {
                                    backend.createSnapshot(backend.selectedEnvironment.name || "", snapNameInput.text)
                                    snapNameInput.text = ""
                                }
                            }
                        }

                        // Per-environment Idle Auto-Stop policy (H1)
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8
                            readonly property string curIdleOv: backend.selectedEnvironment.idleStopOverride || "inherit"
                            readonly property bool curIgnorePorts: backend.selectedEnvironment.idleStopIgnorePorts || false
                            readonly property int globalIdleMin: backend.idleStopMinutes

                            Text {
                                text: qsTr("Idle Auto-Stop:")
                                color: theme.muted
                                font.family: app.sansFont
                                font.pixelSize: 11
                            }

                            Repeater {
                                model: [
                                    { val: "inherit", lbl: parent.globalIdleMin > 0 ? qsTr("Inherit (%1m)").arg(parent.globalIdleMin) : qsTr("Inherit (Off)") },
                                    { val: "off", lbl: qsTr("Off") },
                                    { val: "15", lbl: "15m" },
                                    { val: "30", lbl: "30m" },
                                    { val: "60", lbl: "60m" },
                                    { val: "120", lbl: "120m" }
                                ]
                                delegate: Rectangle {
                                    required property var modelData
                                    readonly property string normOv: (backend.selectedEnvironment.idleStopOverride || "inherit")
                                    readonly property bool selected: normOv === modelData.val || (modelData.val === "inherit" && normOv.length === 0)
                                    height: 26
                                    width: idleOvTxt.implicitWidth + 14
                                    radius: Math.min(4, theme.radius)
                                    color: selected ? app.surfaceSelected : app.surfaceCard
                                    border.width: 1
                                    border.color: selected ? theme.accent : app.borderSubtle
                                    Text {
                                        id: idleOvTxt
                                        anchors.centerIn: parent
                                        text: modelData.lbl
                                        color: selected ? theme.accent : theme.muted
                                        font.family: app.monoFont
                                        font.pixelSize: 10
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        enabled: !backend.busy
                                        onClicked: backend.setEnvironmentIdleStop(
                                            backend.selectedEnvironment.name || "",
                                            modelData.val,
                                            backend.selectedEnvironment.idleStopIgnorePorts || false)
                                    }
                                }
                            }

                            Rectangle {
                                id: allowPortsPill
                                readonly property bool allowPorts: backend.selectedEnvironment.idleStopIgnorePorts || false
                                height: 26
                                width: allowPortsTxt.implicitWidth + 14
                                radius: Math.min(4, theme.radius)
                                color: allowPorts ? app.surfaceSelected : app.surfaceCard
                                border.width: 1
                                border.color: allowPorts ? theme.accent : app.borderSubtle
                                Text {
                                    id: allowPortsTxt
                                    anchors.centerIn: parent
                                    text: allowPortsPill.allowPorts ? qsTr("Ignore ports: ON") : qsTr("Ignore ports: OFF")
                                    color: allowPortsPill.allowPorts ? theme.accent : theme.muted
                                    font.family: app.monoFont
                                    font.pixelSize: 10
                                }
                                MouseArea {
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    enabled: !backend.busy
                                    onClicked: backend.setEnvironmentIdleStop(
                                        backend.selectedEnvironment.name || "",
                                        backend.selectedEnvironment.idleStopOverride || "inherit",
                                        !(backend.selectedEnvironment.idleStopIgnorePorts || false))
                                }
                            }

                            Item { Layout.fillWidth: true }
                        }

                        // Per-environment Session Recording policy (H3)
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8
                            visible: !(backend.selectedEnvironment.isTemplate || false)

                            Text {
                                text: qsTr("Session Recording:")
                                color: theme.muted
                                font.family: app.sansFont
                                font.pixelSize: 11
                            }

                            Repeater {
                                model: [
                                    { val: "inherit", label: backend.recordSessions ? qsTr("Inherit (On)") : qsTr("Inherit (Off)") },
                                    { val: "true", label: qsTr("On") },
                                    { val: "false", label: qsTr("Off") }
                                ]
                                delegate: Rectangle {
                                    required property var modelData
                                    readonly property string curOv: (backend.selectedEnvironment.recordOverride || "").toLowerCase()
                                    readonly property bool selected: (modelData.val === "inherit" && (curOv === "" || curOv === "inherit")) || (curOv === modelData.val)
                                    height: 26
                                    width: recOvTxt.implicitWidth + 14
                                    radius: Math.min(4, theme.radius)
                                    color: selected ? app.surfaceSelected : app.surfaceCard
                                    border.width: 1
                                    border.color: selected ? theme.accent : app.borderSubtle
                                    Text {
                                        id: recOvTxt
                                        anchors.centerIn: parent
                                        text: modelData.label
                                        color: selected ? theme.accent : theme.muted
                                        font.family: app.monoFont
                                        font.pixelSize: 10
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        enabled: !backend.busy
                                        onClicked: backend.setEnvironmentRecordSessions(
                                            backend.selectedEnvironment.name || "",
                                            modelData.val)
                                    }
                                }
                            }

                            Item { Layout.fillWidth: true }
                        }

                        Rectangle {
                            Layout.fillWidth: true
                            height: 1
                            color: app.borderSubtle
                        }

                        // One-time conversion / cleanup banner for plain-directory workspaces on Btrfs
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 10
                            visible: ((backend.selectedEnvironment.workspaceOnBtrfs || false) && !(backend.selectedEnvironment.workspaceIsSubvolume || false)) || (backend.selectedEnvironment.hasPreSubvolBackup || false)

                            Text {
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                                text: (backend.selectedEnvironment.hasPreSubvolBackup || false)
                                      ? qsTr("Pre-subvolume backup (.pre-subvol) is kept on disk.")
                                      : qsTr("Workspace is a plain folder. Convert to Btrfs subvolume to enable paired %1 snapshots.").arg(backend.containerWorkspacePath)
                                color: theme.muted
                                font.family: app.sansFont
                                font.pixelSize: 11
                            }

                            ActionButton {
                                app: mainContentRoot.app
                                implicitHeight: 28
                                visible: (backend.selectedEnvironment.workspaceOnBtrfs || false) && !(backend.selectedEnvironment.workspaceIsSubvolume || false)
                                label: qsTr("Convert to Subvolume")
                                disabled: backend.busy || (backend.selectedEnvironment.running || false)
                                onClicked: backend.convertWorkspaceToSubvolume(backend.selectedEnvironment.name || "")
                            }

                            ActionButton {
                                app: mainContentRoot.app
                                implicitHeight: 28
                                visible: backend.selectedEnvironment.hasPreSubvolBackup || false
                                label: qsTr("Delete .pre-subvol")
                                danger: true
                                disabled: backend.busy
                                onClicked: backend.deletePreSubvolBackup(backend.selectedEnvironment.name || "")
                            }
                        }

                        Text {
                            visible: (!backend.selectedEnvironment.snapshots || backend.selectedEnvironment.snapshots.length === 0) && (!backend.selectedEnvironment.orphanWorkspaceSnapshots || backend.selectedEnvironment.orphanWorkspaceSnapshots.length === 0)
                            text: qsTr("No snapshots saved for this environment yet.")
                            color: theme.muted
                            font.family: app.sansFont
                            font.pixelSize: 12
                        }

                        ListView {
                            id: snapListView
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            clip: true
                            spacing: 6
                            model: backend.selectedEnvironment.snapshots || []

                            delegate: Item {
                                required property string modelData
                                readonly property bool hasWsHalf: (backend.selectedEnvironment.workspaceSnapshots || []).indexOf(modelData) >= 0
                                width: snapListView.width
                                height: 36

                                Rectangle {
                                    anchors.fill: parent
                                    radius: Math.min(3, theme.radius)
                                    color: app.surfaceCard
                                    border.width: 1
                                    border.color: app.borderSubtle

                                    RowLayout {
                                        anchors.fill: parent
                                        anchors.leftMargin: 12
                                        anchors.rightMargin: 10
                                        spacing: 10

                                        Text {
                                            text: "●"
                                            color: theme.accent
                                            font.pixelSize: 10
                                        }

                                        Rectangle {
                                            visible: modelData.startsWith("auto-")
                                            height: 18
                                            width: autoTagText.implicitWidth + 10
                                            radius: 2
                                            color: app.surfaceInput
                                            border.width: 1
                                            border.color: app.borderSubtle
                                            Text {
                                                id: autoTagText
                                                anchors.centerIn: parent
                                                text: qsTr("AUTO")
                                                color: theme.muted
                                                font.family: app.monoFont
                                                font.pixelSize: 9
                                            }
                                        }

                                        Rectangle {
                                            visible: hasWsHalf
                                            height: 18
                                            width: wsTagText.implicitWidth + 10
                                            radius: 2
                                            color: app.surfaceInput
                                            border.width: 1
                                            border.color: theme.accent
                                            Text {
                                                id: wsTagText
                                                anchors.centerIn: parent
                                                text: qsTr("W")
                                                color: theme.accent
                                                font.family: app.monoFont
                                                font.pixelSize: 9
                                                font.weight: Font.Bold
                                            }
                                        }

                                        Text {
                                            Layout.fillWidth: true
                                            text: modelData
                                            color: theme.foreground
                                            font.family: app.monoFont
                                            font.pixelSize: 12
                                        }

                                        Text {
                                            readonly property string snapSize: (backend.selectedEnvironment.snapshotSizes
                                                                                && backend.selectedEnvironment.snapshotSizes[modelData])
                                                                               ? backend.selectedEnvironment.snapshotSizes[modelData]
                                                                               : ""
                                            readonly property string wsSize: (backend.selectedEnvironment.workspaceSnapshotSizes
                                                                              && backend.selectedEnvironment.workspaceSnapshotSizes[modelData])
                                                                             ? backend.selectedEnvironment.workspaceSnapshotSizes[modelData]
                                                                             : ""
                                            readonly property string combinedSize: {
                                                if (snapSize.length > 0 && wsSize.length > 0)
                                                    return snapSize + " · W: " + wsSize
                                                if (snapSize.length > 0)
                                                    return snapSize
                                                if (wsSize.length > 0)
                                                    return "W: " + wsSize
                                                return ""
                                            }
                                            visible: combinedSize.length > 0
                                            text: combinedSize
                                            color: theme.muted
                                            font.family: app.monoFont
                                            font.pixelSize: 11
                                        }

                                        ActionButton {
                                            app: mainContentRoot.app
                                            implicitHeight: 24
                                            visible: hasWsHalf
                                            label: qsTr("Changes")
                                            onClicked: app.openChangeReviewDialog(modelData)
                                        }

                                        ActionButton {
                                            app: mainContentRoot.app
                                            implicitHeight: 24
                                            label: qsTr("Restore")
                                            disabled: backend.busy
                                            onClicked: app.openRestoreDialog(modelData, true, hasWsHalf)
                                        }

                                        ActionButton {
                                            app: mainContentRoot.app
                                            implicitHeight: 24
                                            label: qsTr("Delete")
                                            danger: true
                                            disabled: backend.busy
                                            onClicked: backend.deleteSnapshot(backend.selectedEnvironment.name || "", modelData)
                                        }
                                    }
                                }
                            }
                        }

                        // Workspace-only snapshots (e.g. pre-restore safety snapshots or orphans)
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 4
                            visible: (backend.selectedEnvironment.orphanWorkspaceSnapshots || []).length > 0

                            Text {
                                text: qsTr("Workspace-only snapshots")
                                color: theme.muted
                                font.family: app.sansFont
                                font.pixelSize: 11
                            }

                            Repeater {
                                model: backend.selectedEnvironment.orphanWorkspaceSnapshots || []
                                delegate: Rectangle {
                                    required property string modelData
                                    Layout.fillWidth: true
                                    height: 32
                                    radius: Math.min(3, theme.radius)
                                    color: app.surfaceCard
                                    border.width: 1
                                    border.color: app.borderSubtle

                                    RowLayout {
                                        anchors.fill: parent
                                        anchors.leftMargin: 12
                                        anchors.rightMargin: 10
                                        spacing: 10

                                        Rectangle {
                                            height: 18
                                            width: wsOnlyTag.implicitWidth + 10
                                            radius: 2
                                            color: app.surfaceInput
                                            border.width: 1
                                            border.color: app.borderSubtle
                                            Text {
                                                id: wsOnlyTag
                                                anchors.centerIn: parent
                                                text: qsTr("WS ONLY")
                                                color: theme.muted
                                                font.family: app.monoFont
                                                font.pixelSize: 9
                                            }
                                        }

                                        Text {
                                            Layout.fillWidth: true
                                            text: modelData
                                            color: theme.foreground
                                            font.family: app.monoFont
                                            font.pixelSize: 11
                                        }

                                        Text {
                                            readonly property string wsOnlySize: (backend.selectedEnvironment.workspaceSnapshotSizes
                                                                                  && backend.selectedEnvironment.workspaceSnapshotSizes[modelData])
                                                                                 ? backend.selectedEnvironment.workspaceSnapshotSizes[modelData]
                                                                                 : ""
                                            visible: wsOnlySize.length > 0
                                            text: wsOnlySize
                                            color: theme.muted
                                            font.family: app.monoFont
                                            font.pixelSize: 11
                                        }

                                        ActionButton {
                                            app: mainContentRoot.app
                                            implicitHeight: 24
                                            label: qsTr("Changes")
                                            onClicked: app.openChangeReviewDialog(modelData)
                                        }

                                        ActionButton {
                                            app: mainContentRoot.app
                                            implicitHeight: 24
                                            label: qsTr("Restore")
                                            disabled: backend.busy
                                            onClicked: app.openRestoreDialog(modelData, false, true)
                                        }

                                        ActionButton {
                                            app: mainContentRoot.app
                                            implicitHeight: 24
                                            label: qsTr("Delete")
                                            danger: true
                                            disabled: backend.busy
                                            onClicked: backend.deleteSnapshot(backend.selectedEnvironment.name || "", modelData)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // HEADLESS GUI TEST RUNNER
                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 54
                    radius: Math.min(4, theme.radius)
                    color: app.surfaceInput
                    border.width: 1
                    border.color: app.borderSubtle

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 12
                        anchors.rightMargin: 12
                        spacing: 10

                        Text {
                            text: qsTr("Virtual Display:")
                            color: theme.muted
                            font.family: app.sansFont
                            font.pixelSize: 12
                        }

                        FlatInput {
                            app: mainContentRoot.app
                            id: headlessInput
                            Layout.fillWidth: true
                            implicitHeight: 34
                            placeholder: qsTr("Command for 1920x1080 Xvfb screen (e.g. flutter test, npm test, pytest)")
                            onAccepted: {
                                if (!backend.busy && headlessInput.text.length > 0)
                                    backend.runHeadlessCommand(backend.selectedEnvironment.name || "", headlessInput.text)
                            }
                        }

                        ActionButton {
                            app: mainContentRoot.app
                            implicitHeight: 34
                            minWidth: 120
                            label: qsTr("Run Headless")
                            disabled: backend.busy
                            onClicked: {
                                if (headlessInput.text.length > 0)
                                    backend.runHeadlessCommand(backend.selectedEnvironment.name || "", headlessInput.text)
                            }
                        }
                    }
                }
            }
            }
        }
    }

    // --------------------------------------------------------------
    // TAB 1: OS IMAGES & ONE-TAP DOWNLOADS
    // --------------------------------------------------------------
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 20
        spacing: 14
        visible: app.activeTab === 1

        RowLayout {
            Layout.fillWidth: true
            ColumnLayout {
                spacing: 2
                Text {
                    text: qsTr("Linux OS Image Catalog")
                    color: theme.foreground
                    font.family: app.sansFont
                    font.pixelSize: 16
                    font.weight: Font.Medium
                }
                Text {
                    text: qsTr("Pre-download OS images to local storage so new environments launch in under 1 second, or click 'Use This OS' to start immediately.")
                    color: theme.muted
                    font.family: app.sansFont
                    font.pixelSize: 12
                }
            }
        }

        ListView {
            id: osListView
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 8
            model: backend.osCatalog

            delegate: Item {
                required property var modelData
                width: osListView.width
                height: 74

                Rectangle {
                    anchors.fill: parent
                    radius: Math.min(4, theme.radius)
                    color: app.surfaceCard
                    border.width: 1
                    border.color: modelData.downloaded ? "#284d3a" : app.borderSubtle

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 16
                        anchors.rightMargin: 16
                        spacing: 14

                        Rectangle {
                            width: 36
                            height: 36
                            radius: Math.min(4, theme.radius)
                            color: modelData.downloaded ? "#162920" : app.surfaceInput
                            border.width: 1
                            border.color: modelData.downloaded ? "#284d3a" : app.borderSubtle
                            Text {
                                anchors.centerIn: parent
                                text: modelData.downloaded ? "✓" : "↓"
                                color: modelData.downloaded ? theme.green : theme.accent
                                font.family: app.monoFont
                                font.pixelSize: 14
                            }
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 3

                            RowLayout {
                                spacing: 8
                                Text {
                                    text: modelData.name
                                    color: theme.foreground
                                    font.family: app.sansFont
                                    font.pixelSize: 14
                                    font.weight: Font.Medium
                                }
                                Rectangle {
                                    height: 18
                                    width: osBadgeText.implicitWidth + 10
                                    radius: 2
                                    color: app.surfaceInput
                                    border.width: 1
                                    border.color: app.borderSubtle
                                    Text {
                                        id: osBadgeText
                                        anchors.centerIn: parent
                                        text: modelData.badge
                                        color: theme.accent
                                        font.family: app.monoFont
                                        font.pixelSize: 10
                                    }
                                }
                                Rectangle {
                                    height: 18
                                    width: cacheStateText.implicitWidth + 10
                                    radius: 2
                                    color: modelData.downloaded ? "#162920" : app.surfaceInput
                                    border.width: 1
                                    border.color: modelData.downloaded ? "#284d3a" : app.borderSubtle
                                    Text {
                                        id: cacheStateText
                                        anchors.centerIn: parent
                                        text: modelData.downloading ? qsTr("Downloading...") : modelData.sizeText
                                        color: modelData.downloaded ? theme.green : theme.muted
                                        font.family: app.monoFont
                                        font.pixelSize: 10
                                    }
                                }
                            }

                            Text {
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                                text: modelData.description
                                color: theme.muted
                                font.family: app.sansFont
                                font.pixelSize: 12
                            }
                        }

                        ActionButton {
                            app: mainContentRoot.app
                            label: modelData.downloading
                                   ? qsTr("Downloading...")
                                   : (modelData.downloaded ? qsTr("Remove Cache") : qsTr("Download OS"))
                            variant: modelData.downloaded ? "danger" : ""
                            minWidth: 135
                            disabled: backend.busy
                            onClicked: {
                                if (modelData.downloaded)
                                    backend.removeOsImage(modelData.id)
                                else
                                    backend.downloadOsImage(modelData.id)
                            }
                        }

                        ActionButton {
                            app: mainContentRoot.app
                            label: qsTr("Use This OS")
                            variant: "primary"
                            minWidth: 125
                            disabled: backend.busy
                            onClicked: app.openCreateDialog(modelData.id)
                        }
                    }
                }
            }
        }
    }
    // The window shortcuts create a snapshot with the name typed in this pane.
    readonly property string snapshotNameText: snapNameInput.text

}
