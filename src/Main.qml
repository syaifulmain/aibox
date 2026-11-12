import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Layouts

ApplicationWindow {
    id: win
    width: 1080
    height: 720
    minimumWidth: 820
    minimumHeight: 540
    visible: true
    title: qsTr("AIBox")
    color: theme.background

    Material.theme: Material.Dark
    Material.accent: theme.accent
    Material.roundedScale: theme.radius

    property int activeTab: 0
    property bool helpVisible: false
    property bool createVisible: false
    property bool wipeVisible: false
    property bool deleteWorkspaceOnWipe: false

    // Form state for New Environment dialog
    property string newEnvName: ""
    property string newEnvOsId: "ubuntu-24.04"
    property string newEnvPreset: "full"
    property string newEnvDisk: "35GiB"
    property string newEnvRam: "6GiB"
    property int newEnvCpu: 4

    // Form state for Snapshot & Headless command
    property string customSnapshotName: ""
    property string headlessCmd: ""

    readonly property bool anyModalOpen: helpVisible || createVisible || wipeVisible

    Shortcut {
        sequence: "?"
        context: Qt.ApplicationShortcut
        onActivated: win.helpVisible = !win.helpVisible
    }
    Shortcut {
        sequence: "Escape"
        context: Qt.ApplicationShortcut
        onActivated: {
            if (win.helpVisible) win.helpVisible = false
            else if (win.createVisible) win.createVisible = false
            else if (win.wipeVisible) win.wipeVisible = false
            else backend.clearError()
        }
    }
    Shortcut {
        sequence: "Q"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen
        onActivated: Qt.quit()
    }
    Shortcut {
        sequence: "N"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && !backend.busy
        onActivated: {
            win.newEnvName = ""
            win.createVisible = true
        }
    }
    Shortcut {
        sequence: "1"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen
        onActivated: win.activeTab = 0
    }
    Shortcut {
        sequence: "2"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen
        onActivated: win.activeTab = 1
    }
    Shortcut {
        sequence: "R"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && !backend.busy
        onActivated: backend.refresh()
    }
    Shortcut {
        sequence: "Space"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0
        onActivated: backend.primaryAction()
    }
    Shortcut {
        sequence: "Return"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && backend.environments.length > 0
        onActivated: backend.enterEnvironment(backend.selectedEnvironment.name || "")
    }
    Shortcut {
        sequence: "F"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && backend.environments.length > 0
        onActivated: backend.openWorkspaceFolder(backend.selectedEnvironment.name || "")
    }
    Shortcut {
        sequence: "S"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && !backend.busy && backend.environments.length > 0
        onActivated: backend.toggleStartStop(backend.selectedEnvironment.name || "")
    }
    Shortcut {
        sequence: "P"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && !backend.busy && backend.environments.length > 0
        onActivated: backend.createSnapshot(backend.selectedEnvironment.name || "", win.customSnapshotName)
    }
    Shortcut {
        sequence: "C"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && backend.environments.length > 0
        onActivated: backend.copyAiCommand(backend.selectedEnvironment.name || "")
    }
    Shortcut {
        sequence: "Delete"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && !backend.busy && backend.environments.length > 0
        onActivated: {
            win.deleteWorkspaceOnWipe = false
            win.wipeVisible = true
        }
    }
    Shortcut {
        sequence: "Up"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && backend.environments.length > 0
        onActivated: backend.selectedIndex = Math.max(0, backend.selectedIndex - 1)
    }
    Shortcut {
        sequence: "Down"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && backend.environments.length > 0
        onActivated: backend.selectedIndex = Math.min(backend.environments.length - 1, backend.selectedIndex + 1)
    }

    Connections {
        target: backend
        function onEnvironmentCreated(name) {
            win.createVisible = false
            win.activeTab = 0
        }
    }

    // Right-to-left languages mirror the window; the Translator sets the
    // application's layout direction.
    Item {
        anchors.fill: parent
        LayoutMirroring.enabled: Qt.application.layoutDirection === Qt.RightToLeft
        LayoutMirroring.childrenInherit: true

        ColumnLayout {
            anchors.fill: parent
            spacing: 0

            // ==================================================================
            // TOP HEADER BAR
            // ==================================================================
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 60
                color: theme.darkBackground

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 20
                    anchors.rightMargin: 20
                    spacing: 16

                    // App title & badge
                    RowLayout {
                        spacing: 10
                        Rectangle {
                            width: 34
                            height: 34
                            radius: Math.min(6, theme.radius)
                            color: theme.accent
                            Text {
                                anchors.centerIn: parent
                                text: "AI"
                                color: theme.accentForeground
                                font.pixelSize: 14
                                font.bold: true
                            }
                        }
                        ColumnLayout {
                            spacing: 1
                            Text {
                                text: qsTr("AIBox")
                                color: theme.foreground
                                font.pixelSize: 16
                                font.bold: true
                            }
                            Text {
                                text: qsTr("Isolated Linux OS Manager")
                                color: theme.muted
                                font.pixelSize: 11
                            }
                        }
                    }

                    // Navigation Tabs
                    RowLayout {
                        Layout.leftMargin: 16
                        spacing: 8

                        Repeater {
                            model: [
                                { label: qsTr("%n Environments", "", backend.environments.length), key: "1", tab: 0 },
                                { label: qsTr("OS Images & Downloads"), key: "2", tab: 1 }
                            ]
                            delegate: Rectangle {
                                required property var modelData
                                height: 36
                                width: tabRow.implicitWidth + 24
                                radius: theme.radius
                                color: win.activeTab === modelData.tab ? theme.lighterBackground : "transparent"
                                border.width: win.activeTab === modelData.tab ? 1 : 0
                                border.color: theme.accent

                                RowLayout {
                                    id: tabRow
                                    anchors.centerIn: parent
                                    spacing: 8
                                    Text {
                                        text: modelData.label
                                        color: win.activeTab === modelData.tab ? theme.foreground : theme.muted
                                        font.pixelSize: 13
                                        font.bold: win.activeTab === modelData.tab
                                    }
                                    Rectangle {
                                        width: 18
                                        height: 18
                                        radius: Math.min(4, theme.radius)
                                        color: theme.darkBackground
                                        border.width: 1
                                        border.color: theme.muted
                                        Text {
                                            anchors.centerIn: parent
                                            text: modelData.key
                                            color: theme.muted
                                            font.pixelSize: 10
                                        }
                                    }
                                }
                                MouseArea {
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: win.activeTab = modelData.tab
                                }
                            }
                        }
                    }

                    Item { Layout.fillWidth: true }

                    // Refresh button
                    Rectangle {
                        height: 36
                        width: refreshLabel.implicitWidth + 24
                        radius: theme.radius
                        color: theme.lighterBackground
                        opacity: backend.busy ? 0.5 : 1.0
                        Text {
                            id: refreshLabel
                            anchors.centerIn: parent
                            text: qsTr("Refresh (R)")
                            color: theme.foreground
                            font.pixelSize: 12
                        }
                        MouseArea {
                            anchors.fill: parent
                            enabled: !backend.busy
                            cursorShape: Qt.PointingHandCursor
                            onClicked: backend.refresh()
                        }
                    }

                    // Help button
                    Rectangle {
                        width: 36
                        height: 36
                        radius: theme.radius
                        color: win.helpVisible ? theme.accent : theme.lighterBackground
                        Text {
                            anchors.centerIn: parent
                            text: "?"
                            color: win.helpVisible ? theme.accentForeground : theme.foreground
                            font.pixelSize: 14
                            font.bold: true
                        }
                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: win.helpVisible = !win.helpVisible
                        }
                    }

                    // Primary + New Environment Button
                    Rectangle {
                        height: 36
                        width: newBtnLabel.implicitWidth + 28
                        radius: theme.radius
                        color: theme.accent
                        opacity: backend.busy ? 0.5 : 1.0
                        Text {
                            id: newBtnLabel
                            anchors.centerIn: parent
                            text: qsTr("+ New Environment (N)")
                            color: theme.accentForeground
                            font.pixelSize: 13
                            font.bold: true
                        }
                        MouseArea {
                            anchors.fill: parent
                            enabled: !backend.busy
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                win.newEnvName = ""
                                win.createVisible = true
                            }
                        }
                    }
                }

                Rectangle {
                    anchors.bottom: parent.bottom
                    width: parent.width
                    height: 1
                    color: theme.lighterBackground
                }
            }

            // ==================================================================
            // STATUS / PROGRESS / ERROR BANNER
            // ==================================================================
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: visible ? 40 : 0
                visible: backend.busy || backend.statusMessage.length > 0 || backend.errorMessage.length > 0
                color: backend.errorMessage.length > 0 ? "#3a1820" : theme.lighterBackground

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 20
                    anchors.rightMargin: 20
                    spacing: 12

                    BusyIndicator {
                        running: backend.busy
                        visible: backend.busy
                        Layout.preferredWidth: 22
                        Layout.preferredHeight: 22
                    }

                    Text {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: backend.errorMessage.length > 0 ? backend.errorMessage : backend.statusMessage
                        color: backend.errorMessage.length > 0 ? theme.red : theme.foreground
                        font.pixelSize: 12
                        font.bold: backend.errorMessage.length > 0
                    }

                    Rectangle {
                        visible: backend.errorMessage.length > 0
                        height: 24
                        width: dismissText.implicitWidth + 16
                        radius: Math.min(4, theme.radius)
                        color: theme.darkBackground
                        Text {
                            id: dismissText
                            anchors.centerIn: parent
                            text: qsTr("Dismiss")
                            color: theme.foreground
                            font.pixelSize: 11
                        }
                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: backend.clearError()
                        }
                    }
                }
            }

            // ==================================================================
            // MAIN CONTENT AREA
            // ==================================================================
            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true

                // --------------------------------------------------------------
                // TAB 0: ENVIRONMENTS (MASTER-DETAIL VIEW)
                // --------------------------------------------------------------
                RowLayout {
                    anchors.fill: parent
                    anchors.margins: 20
                    spacing: 20
                    visible: win.activeTab === 0

                    // EMPTY STATE WHEN NO ENVIRONMENTS EXIST
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        visible: backend.environments.length === 0
                        color: theme.darkBackground
                        radius: theme.radius
                        border.width: 1
                        border.color: theme.lighterBackground

                        ColumnLayout {
                            anchors.centerIn: parent
                            width: Math.min(parent.width - 60, 520)
                            spacing: 14

                            Rectangle {
                                Layout.alignment: Qt.AlignHCenter
                                width: 56
                                height: 56
                                radius: theme.radius
                                color: theme.lighterBackground
                                border.width: 1
                                border.color: theme.accent
                                Text {
                                    anchors.centerIn: parent
                                    text: "OS"
                                    color: theme.accent
                                    font.pixelSize: 20
                                    font.bold: true
                                }
                            }

                            Text {
                                Layout.fillWidth: true
                                horizontalAlignment: Text.AlignHCenter
                                text: qsTr("No Isolated Environments Yet")
                                color: theme.foreground
                                font.pixelSize: 20
                                font.bold: true
                            }

                            Text {
                                Layout.fillWidth: true
                                horizontalAlignment: Text.AlignHCenter
                                wrapMode: Text.WordWrap
                                text: qsTr("Create a fresh Linux OS environment for your next big AI, Desktop, or Mobile project. Give AI full root control inside, share files seamlessly via /workspace, and wipe it clean in 1 second when finished.")
                                color: theme.muted
                                font.pixelSize: 13
                            }

                            RowLayout {
                                Layout.alignment: Qt.AlignHCenter
                                Layout.topMargin: 8
                                spacing: 12

                                Rectangle {
                                    height: 40
                                    width: emptyCreateText.implicitWidth + 32
                                    radius: theme.radius
                                    color: theme.accent
                                    Text {
                                        id: emptyCreateText
                                        anchors.centerIn: parent
                                        text: qsTr("+ Create First Environment (N)")
                                        color: theme.accentForeground
                                        font.pixelSize: 13
                                        font.bold: true
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: win.createVisible = true
                                    }
                                }

                                Rectangle {
                                    height: 40
                                    width: emptyCatalogText.implicitWidth + 28
                                    radius: theme.radius
                                    color: theme.lighterBackground
                                    Text {
                                        id: emptyCatalogText
                                        anchors.centerIn: parent
                                        text: qsTr("Browse OS Images (2)")
                                        color: theme.foreground
                                        font.pixelSize: 13
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: win.activeTab = 1
                                    }
                                }
                            }
                        }
                    }

                    // LEFT PANE: ENVIRONMENT LIST
                    Rectangle {
                        Layout.preferredWidth: 360
                        Layout.fillHeight: true
                        visible: backend.environments.length > 0
                        color: theme.darkBackground
                        radius: theme.radius
                        border.width: 1
                        border.color: theme.lighterBackground

                        ColumnLayout {
                            anchors.fill: parent
                            anchors.margins: 12
                            spacing: 10

                            RowLayout {
                                Layout.fillWidth: true
                                Text {
                                    text: qsTr("ENVIRONMENTS")
                                    color: theme.muted
                                    font.pixelSize: 11
                                    font.bold: true
                                }
                                Item { Layout.fillWidth: true }
                                Text {
                                    text: qsTr("↑/↓ to select • Space to enter")
                                    color: theme.muted
                                    font.pixelSize: 11
                                }
                            }

                            ListView {
                                id: envListView
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                clip: true
                                spacing: 8
                                model: backend.environments

                                delegate: Item {
                                    required property var modelData
                                    required property int index
                                    width: envListView.width
                                    height: 86

                                    Rectangle {
                                        anchors.fill: parent
                                        radius: theme.radius
                                        color: backend.selectedIndex === index ? theme.lighterBackground : theme.background
                                        border.width: backend.selectedIndex === index ? 2 : 1
                                        border.color: backend.selectedIndex === index ? theme.accent : theme.lighterBackground

                                        ColumnLayout {
                                            anchors.fill: parent
                                            anchors.margins: 12
                                            spacing: 6

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 8

                                                Rectangle {
                                                    width: 10
                                                    height: 10
                                                    radius: 5
                                                    color: modelData.running ? theme.green : theme.muted
                                                }

                                                Text {
                                                    Layout.fillWidth: true
                                                    elide: Text.ElideRight
                                                    text: modelData.name
                                                    color: theme.foreground
                                                    font.pixelSize: 15
                                                    font.bold: true
                                                }

                                                Rectangle {
                                                    height: 20
                                                    width: statusBadgeText.implicitWidth + 12
                                                    radius: Math.min(4, theme.radius)
                                                    color: modelData.running ? "#1d3528" : theme.darkBackground
                                                    Text {
                                                        id: statusBadgeText
                                                        anchors.centerIn: parent
                                                        text: modelData.status
                                                        color: modelData.running ? theme.green : theme.muted
                                                        font.pixelSize: 10
                                                        font.bold: true
                                                    }
                                                }
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                elide: Text.ElideRight
                                                text: modelData.osName + " • " + modelData.preset
                                                color: theme.muted
                                                font.pixelSize: 12
                                            }

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 10
                                                Text {
                                                    text: qsTr("Disk: %1").arg(modelData.diskLimit)
                                                    color: theme.muted
                                                    font.pixelSize: 11
                                                }
                                                Text {
                                                    text: qsTr("RAM: %1 / %2").arg(modelData.memoryUsage).arg(modelData.memoryLimit)
                                                    color: theme.muted
                                                    font.pixelSize: 11
                                                }
                                                Item { Layout.fillWidth: true }
                                                Text {
                                                    text: modelData.ipv4
                                                    color: theme.accent
                                                    font.pixelSize: 11
                                                }
                                            }
                                        }

                                        MouseArea {
                                            anchors.fill: parent
                                            preventStealing: true
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: backend.selectedIndex = index
                                            onDoubleClicked: backend.enterEnvironment(modelData.name)
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // RIGHT PANE: SELECTED ENVIRONMENT INSPECTOR & CONTROLS
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        visible: backend.environments.length > 0
                        color: theme.darkBackground
                        radius: theme.radius
                        border.width: 1
                        border.color: theme.lighterBackground

                        ColumnLayout {
                            anchors.fill: parent
                            anchors.margins: 20
                            spacing: 16

                            // Environment Header & Primary Actions
                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 12

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 4

                                    RowLayout {
                                        spacing: 10
                                        Text {
                                            text: backend.selectedEnvironment.name || ""
                                            color: theme.foreground
                                            font.pixelSize: 22
                                            font.bold: true
                                        }
                                        Rectangle {
                                            height: 22
                                            width: detailStatusText.implicitWidth + 14
                                            radius: Math.min(4, theme.radius)
                                            color: (backend.selectedEnvironment.running || false) ? "#1d3528" : theme.lighterBackground
                                            Text {
                                                id: detailStatusText
                                                anchors.centerIn: parent
                                                text: backend.selectedEnvironment.status || ""
                                                color: (backend.selectedEnvironment.running || false) ? theme.green : theme.muted
                                                font.pixelSize: 11
                                                font.bold: true
                                            }
                                        }
                                    }

                                    Text {
                                        text: (backend.selectedEnvironment.osName || "") + "  •  " + (backend.selectedEnvironment.preset || "")
                                        color: theme.muted
                                        font.pixelSize: 13
                                    }
                                }

                                // Enter Terminal Button
                                Rectangle {
                                    height: 38
                                    width: enterBtnText.implicitWidth + 28
                                    radius: theme.radius
                                    color: theme.accent
                                    Text {
                                        id: enterBtnText
                                        anchors.centerIn: parent
                                        text: qsTr("Enter OS Terminal (Space)")
                                        color: theme.accentForeground
                                        font.pixelSize: 13
                                        font.bold: true
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: backend.enterEnvironment( backend.selectedEnvironment.name || "")
                                    }
                                }
                            }

                            // Secondary Action Bar
                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10

                                // Open Shared Folder
                                Rectangle {
                                    height: 34
                                    width: folderBtnText.implicitWidth + 24
                                    radius: theme.radius
                                    color: theme.lighterBackground
                                    Text {
                                        id: folderBtnText
                                        anchors.centerIn: parent
                                        text: qsTr("Open Shared Folder (F)")
                                        color: theme.foreground
                                        font.pixelSize: 12
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: backend.openWorkspaceFolder(backend.selectedEnvironment.name || "")
                                    }
                                }

                                // Copy AI Command
                                Rectangle {
                                    height: 34
                                    width: copyAiBtnText.implicitWidth + 24
                                    radius: theme.radius
                                    color: theme.lighterBackground
                                    Text {
                                        id: copyAiBtnText
                                        anchors.centerIn: parent
                                        text: qsTr("Copy AI Command (C)")
                                        color: theme.foreground
                                        font.pixelSize: 12
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: backend.copyAiCommand(backend.selectedEnvironment.name || "")
                                    }
                                }

                                // Start / Stop Toggle
                                Rectangle {
                                    height: 34
                                    width: powerBtnText.implicitWidth + 24
                                    radius: theme.radius
                                    color: theme.lighterBackground
                                    opacity: backend.busy ? 0.5 : 1.0
                                    Text {
                                        id: powerBtnText
                                        anchors.centerIn: parent
                                        text: (backend.selectedEnvironment.running || false) ? qsTr("Stop OS (S)") : qsTr("Start OS (S)")
                                        color: (backend.selectedEnvironment.running || false) ? theme.yellow : theme.green
                                        font.pixelSize: 12
                                        font.bold: true
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        enabled: !backend.busy
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: backend.toggleStartStop(backend.selectedEnvironment.name || "")
                                    }
                                }

                                Item { Layout.fillWidth: true }

                                // Wipe Environment Button
                                Rectangle {
                                    height: 34
                                    width: wipeBtnText.implicitWidth + 24
                                    radius: theme.radius
                                    color: "#3a1820"
                                    border.width: 1
                                    border.color: theme.red
                                    opacity: backend.busy ? 0.5 : 1.0
                                    Text {
                                        id: wipeBtnText
                                        anchors.centerIn: parent
                                        text: qsTr("Wipe OS (Del)")
                                        color: theme.red
                                        font.pixelSize: 12
                                        font.bold: true
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        enabled: !backend.busy
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: {
                                            win.deleteWorkspaceOnWipe = false
                                            win.wipeVisible = true
                                        }
                                    }
                                }
                            }

                            // 4 Hardware & Shared Path Stat Cards
                            GridLayout {
                                Layout.fillWidth: true
                                columns: 4
                                columnSpacing: 10
                                rowSpacing: 10

                                Repeater {
                                    model: [
                                        { title: qsTr("DISK QUOTA"), val: backend.selectedEnvironment.diskLimit || "—" },
                                        { title: qsTr("MEMORY USAGE"), val: (backend.selectedEnvironment.memoryUsage || "0 MB") + " / " + (backend.selectedEnvironment.memoryLimit || "—") },
                                        { title: qsTr("CPU ALLOCATION"), val: backend.selectedEnvironment.cpuLimit || "—" },
                                        { title: qsTr("IPV4 ADDRESS"), val: backend.selectedEnvironment.ipv4 || "—" }
                                    ]
                                    delegate: Rectangle {
                                        required property var modelData
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: 62
                                        radius: theme.radius
                                        color: theme.background
                                        border.width: 1
                                        border.color: theme.lighterBackground

                                        ColumnLayout {
                                            anchors.fill: parent
                                            anchors.margins: 10
                                            spacing: 4
                                            Text {
                                                text: modelData.title
                                                color: theme.muted
                                                font.pixelSize: 10
                                                font.bold: true
                                            }
                                            Text {
                                                Layout.fillWidth: true
                                                elide: Text.ElideRight
                                                text: modelData.val
                                                color: theme.foreground
                                                font.pixelSize: 14
                                                font.bold: true
                                            }
                                        }
                                    }
                                }
                            }

                            // Shared Folder Info Box
                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 46
                                radius: theme.radius
                                color: theme.background
                                border.width: 1
                                border.color: theme.lighterBackground

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 12
                                    anchors.rightMargin: 12
                                    spacing: 10

                                    Text {
                                        text: qsTr("SHARED WORKSPACE:")
                                        color: theme.muted
                                        font.pixelSize: 11
                                        font.bold: true
                                    }
                                    Text {
                                        Layout.fillWidth: true
                                        elide: Text.ElideMiddle
                                        text: (backend.selectedEnvironment.workspacePath || "") + "  ⟷  /workspace (inside OS)"
                                        color: theme.accent
                                        font.pixelSize: 12
                                    }
                                }
                            }

                            // SNAPSHOTS (TIME MACHINE) SECTION
                            Rectangle {
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                radius: theme.radius
                                color: theme.background
                                border.width: 1
                                border.color: theme.lighterBackground

                                ColumnLayout {
                                    anchors.fill: parent
                                    anchors.margins: 12
                                    spacing: 10

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10

                                        Text {
                                            Layout.fillWidth: true
                                            elide: Text.ElideRight
                                            text: qsTr("INSTANT BTRFS SNAPSHOTS (TIME MACHINE)")
                                            color: theme.muted
                                            font.pixelSize: 11
                                            font.bold: true
                                        }

                                        TextField {
                                            id: snapNameInput
                                            Layout.preferredWidth: 165
                                            placeholderText: qsTr("Snapshot label (optional)")
                                            font.pixelSize: 12
                                            onTextChanged: win.customSnapshotName = text
                                        }

                                        Rectangle {
                                            height: 32
                                            width: snapCreateLabel.implicitWidth + 20
                                            radius: Math.min(4, theme.radius)
                                            color: theme.accent
                                            opacity: backend.busy ? 0.5 : 1.0
                                            Text {
                                                id: snapCreateLabel
                                                anchors.centerIn: parent
                                                text: qsTr("+ Save Snapshot (P)")
                                                color: theme.accentForeground
                                                font.pixelSize: 12
                                                font.bold: true
                                            }
                                            MouseArea {
                                                anchors.fill: parent
                                                enabled: !backend.busy
                                                cursorShape: Qt.PointingHandCursor
                                                onClicked: {
                                                    backend.createSnapshot(backend.selectedEnvironment.name || "", snapNameInput.text)
                                                    snapNameInput.text = ""
                                                }
                                            }
                                        }
                                    }

                                    Text {
                                        visible: !backend.selectedEnvironment.snapshots || backend.selectedEnvironment.snapshots.length === 0
                                        text: qsTr("No snapshots yet. Save a snapshot in 1 second before letting AI perform major system changes.")
                                        color: theme.muted
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
                                            width: snapListView.width
                                            height: 38

                                            Rectangle {
                                                anchors.fill: parent
                                                radius: Math.min(4, theme.radius)
                                                color: theme.darkBackground

                                                RowLayout {
                                                    anchors.fill: parent
                                                    anchors.leftMargin: 12
                                                    anchors.rightMargin: 12
                                                    spacing: 10

                                                    Text {
                                                        Layout.fillWidth: true
                                                        text: "📸  " + modelData
                                                        color: theme.foreground
                                                        font.pixelSize: 13
                                                        font.bold: true
                                                    }

                                                    Rectangle {
                                                        height: 26
                                                        width: restoreSnapText.implicitWidth + 16
                                                        radius: Math.min(4, theme.radius)
                                                        color: theme.lighterBackground
                                                        Text {
                                                            id: restoreSnapText
                                                            anchors.centerIn: parent
                                                            text: qsTr("Restore")
                                                            color: theme.green
                                                            font.pixelSize: 11
                                                            font.bold: true
                                                        }
                                                        MouseArea {
                                                            anchors.fill: parent
                                                            enabled: !backend.busy
                                                            cursorShape: Qt.PointingHandCursor
                                                            onClicked: backend.restoreSnapshot(backend.selectedEnvironment.name || "", modelData)
                                                        }
                                                    }

                                                    Rectangle {
                                                        height: 26
                                                        width: delSnapText.implicitWidth + 16
                                                        radius: Math.min(4, theme.radius)
                                                        color: theme.lighterBackground
                                                        Text {
                                                            id: delSnapText
                                                            anchors.centerIn: parent
                                                            text: qsTr("Delete")
                                                            color: theme.red
                                                            font.pixelSize: 11
                                                        }
                                                        MouseArea {
                                                            anchors.fill: parent
                                                            enabled: !backend.busy
                                                            cursorShape: Qt.PointingHandCursor
                                                            onClicked: backend.deleteSnapshot(backend.selectedEnvironment.name || "", modelData)
                                                        }
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
                                Layout.preferredHeight: 62
                                radius: theme.radius
                                color: theme.background
                                border.width: 1
                                border.color: theme.lighterBackground

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 12
                                    anchors.rightMargin: 12
                                    spacing: 10

                                    TextField {
                                        id: headlessInput
                                        Layout.fillWidth: true
                                        placeholderText: qsTr("Headless GUI test command (e.g. flutter test, npm test, pytest)")
                                        font.pixelSize: 12
                                    }

                                    Rectangle {
                                        height: 34
                                        width: runHeadlessText.implicitWidth + 20
                                        radius: Math.min(4, theme.radius)
                                        color: theme.lighterBackground
                                        border.width: 1
                                        border.color: theme.accent
                                        opacity: backend.busy ? 0.5 : 1.0
                                        Text {
                                            id: runHeadlessText
                                            anchors.centerIn: parent
                                            text: qsTr("Run in Virtual Display")
                                            color: theme.accent
                                            font.pixelSize: 12
                                            font.bold: true
                                        }
                                        MouseArea {
                                            anchors.fill: parent
                                            enabled: !backend.busy
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: {
                                                if (headlessInput.text.length > 0) {
                                                    backend.runHeadlessCommand(backend.selectedEnvironment.name || "", headlessInput.text)
                                                }
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
                    visible: win.activeTab === 1

                    RowLayout {
                        Layout.fillWidth: true
                        ColumnLayout {
                            spacing: 2
                            Text {
                                text: qsTr("Linux OS Image Catalog")
                                color: theme.foreground
                                font.pixelSize: 18
                                font.bold: true
                            }
                            Text {
                                text: qsTr("Tap 'Download' to pre-cache an OS image locally so new environments launch in under 1 second, or tap 'Use OS' to start a project immediately.")
                                color: theme.muted
                                font.pixelSize: 12
                            }
                        }
                    }

                    ListView {
                        id: osListView
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        clip: true
                        spacing: 10
                        model: backend.osCatalog

                        delegate: Item {
                            required property var modelData
                            width: osListView.width
                            height: 82

                            Rectangle {
                                anchors.fill: parent
                                radius: theme.radius
                                color: theme.darkBackground
                                border.width: 1
                                border.color: modelData.downloaded ? theme.green : theme.lighterBackground

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 18
                                    anchors.rightMargin: 18
                                    spacing: 16

                                    // OS Status Indicator Icon
                                    Rectangle {
                                        width: 44
                                        height: 44
                                        radius: theme.radius
                                        color: modelData.downloaded ? "#1d3528" : theme.lighterBackground
                                        Text {
                                            anchors.centerIn: parent
                                            text: modelData.downloaded ? "✓" : "⬇"
                                            color: modelData.downloaded ? theme.green : theme.accent
                                            font.pixelSize: 18
                                            font.bold: true
                                        }
                                    }

                                    // OS Info
                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: 4

                                        RowLayout {
                                            spacing: 10
                                            Text {
                                                text: modelData.name
                                                color: theme.foreground
                                                font.pixelSize: 15
                                                font.bold: true
                                            }
                                            Rectangle {
                                                height: 20
                                                width: osBadgeText.implicitWidth + 12
                                                radius: Math.min(4, theme.radius)
                                                color: theme.lighterBackground
                                                Text {
                                                    id: osBadgeText
                                                    anchors.centerIn: parent
                                                    text: modelData.badge
                                                    color: theme.accent
                                                    font.pixelSize: 10
                                                    font.bold: true
                                                }
                                            }
                                            Rectangle {
                                                height: 20
                                                width: cacheStateText.implicitWidth + 12
                                                radius: Math.min(4, theme.radius)
                                                color: modelData.downloaded ? "#1d3528" : theme.background
                                                Text {
                                                    id: cacheStateText
                                                    anchors.centerIn: parent
                                                    text: modelData.downloading ? qsTr("Downloading...") : modelData.sizeText
                                                    color: modelData.downloaded ? theme.green : theme.muted
                                                    font.pixelSize: 10
                                                    font.bold: modelData.downloaded
                                                }
                                            }
                                        }

                                        Text {
                                            Layout.fillWidth: true
                                            elide: Text.ElideRight
                                            text: modelData.description
                                            color: theme.muted
                                            font.pixelSize: 12
                                        }
                                    }

                                    // Download or Remove Cache button
                                    Rectangle {
                                        height: 36
                                        width: dlBtnText.implicitWidth + 24
                                        radius: theme.radius
                                        color: modelData.downloaded ? theme.background : theme.lighterBackground
                                        border.width: 1
                                        border.color: modelData.downloaded ? theme.muted : theme.accent
                                        opacity: backend.busy ? 0.5 : 1.0

                                        Text {
                                            id: dlBtnText
                                            anchors.centerIn: parent
                                            text: modelData.downloading
                                                  ? qsTr("Downloading...")
                                                  : (modelData.downloaded ? qsTr("Remove Cache") : qsTr("Download OS"))
                                            color: modelData.downloaded ? theme.muted : theme.accent
                                            font.pixelSize: 12
                                            font.bold: !modelData.downloaded
                                        }
                                        MouseArea {
                                            anchors.fill: parent
                                            enabled: !backend.busy
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: {
                                                if (modelData.downloaded)
                                                    backend.removeOsImage(modelData.id)
                                                else
                                                    backend.downloadOsImage(modelData.id)
                                            }
                                        }
                                    }

                                    // Use OS to Create Environment button
                                    Rectangle {
                                        height: 36
                                        width: useOsBtnText.implicitWidth + 24
                                        radius: theme.radius
                                        color: theme.accent
                                        opacity: backend.busy ? 0.5 : 1.0
                                        Text {
                                            id: useOsBtnText
                                            anchors.centerIn: parent
                                            text: qsTr("Use This OS")
                                            color: theme.accentForeground
                                            font.pixelSize: 12
                                            font.bold: true
                                        }
                                        MouseArea {
                                            anchors.fill: parent
                                            enabled: !backend.busy
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: {
                                                win.newEnvOsId = modelData.id
                                                win.newEnvName = ""
                                                win.createVisible = true
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

        // ======================================================================
        // MODAL 1: CREATE NEW ENVIRONMENT DIALOG
        // ======================================================================
        Rectangle {
            anchors.fill: parent
            visible: win.createVisible
            color: "#b0000000"

            MouseArea {
                anchors.fill: parent
                onClicked: win.createVisible = false
            }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(parent.width - 60, 680)
                height: createCol.implicitHeight + 48
                radius: theme.radius
                color: theme.darkBackground
                border.width: 2
                border.color: theme.accent

                MouseArea { anchors.fill: parent }

                ColumnLayout {
                    id: createCol
                    anchors.fill: parent
                    anchors.margins: 24
                    spacing: 16

                    RowLayout {
                        Layout.fillWidth: true
                        Text {
                            text: qsTr("Create Isolated Linux Environment")
                            color: theme.foreground
                            font.pixelSize: 18
                            font.bold: true
                        }
                        Item { Layout.fillWidth: true }
                        Text {
                            text: "Esc"
                            color: theme.muted
                            font.pixelSize: 12
                        }
                    }

                    TextField {
                        id: createNameField
                        Layout.fillWidth: true
                        placeholderText: qsTr("Project / Environment Name (e.g. pos-desktop, flutter-app)")
                        font.pixelSize: 14
                        text: win.newEnvName
                        onTextChanged: win.newEnvName = text
                    }

                    // 1. Preset Selection
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 6
                        Text {
                            text: qsTr("1. ENVIRONMENT CAPABILITY PRESET")
                            color: theme.muted
                            font.pixelSize: 11
                            font.bold: true
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8
                            Repeater {
                                model: [
                                    { id: "full", label: qsTr("Desktop & Mobile (GPU + KVM + GUI)") },
                                    { id: "standard", label: qsTr("Standard AI / Web (Docker)") },
                                    { id: "vm", label: qsTr("Full KVM Virtual Machine") }
                                ]
                                delegate: Rectangle {
                                    required property var modelData
                                    Layout.fillWidth: true
                                    height: 36
                                    radius: theme.radius
                                    color: win.newEnvPreset === modelData.id ? theme.accent : theme.background
                                    border.width: 1
                                    border.color: win.newEnvPreset === modelData.id ? theme.accent : theme.lighterBackground
                                    Text {
                                        anchors.centerIn: parent
                                        text: modelData.label
                                        color: win.newEnvPreset === modelData.id ? theme.accentForeground : theme.foreground
                                        font.pixelSize: 12
                                        font.bold: win.newEnvPreset === modelData.id
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: win.newEnvPreset = modelData.id
                                    }
                                }
                            }
                        }
                    }

                    // 2. OS Selection Grid
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 6
                        Text {
                            text: qsTr("2. OPERATING SYSTEM (DISTRO)")
                            color: theme.muted
                            font.pixelSize: 11
                            font.bold: true
                        }
                        GridLayout {
                            Layout.fillWidth: true
                            columns: 4
                            columnSpacing: 8
                            rowSpacing: 8
                            Repeater {
                                model: backend.osCatalog
                                delegate: Rectangle {
                                    required property var modelData
                                    Layout.fillWidth: true
                                    height: 44
                                    radius: theme.radius
                                    color: win.newEnvOsId === modelData.id ? theme.accent : theme.background
                                    border.width: 1
                                    border.color: win.newEnvOsId === modelData.id ? theme.accent : theme.lighterBackground

                                    ColumnLayout {
                                        anchors.centerIn: parent
                                        spacing: 1
                                        Text {
                                            Layout.alignment: Qt.AlignHCenter
                                            text: modelData.id
                                            color: win.newEnvOsId === modelData.id ? theme.accentForeground : theme.foreground
                                            font.pixelSize: 12
                                            font.bold: true
                                        }
                                        Text {
                                            Layout.alignment: Qt.AlignHCenter
                                            text: modelData.downloaded ? qsTr("✓ Cached") : modelData.badge
                                            color: win.newEnvOsId === modelData.id ? theme.accentForeground : (modelData.downloaded ? theme.green : theme.muted)
                                            font.pixelSize: 10
                                        }
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: win.newEnvOsId = modelData.id
                                    }
                                }
                            }
                        }
                    }

                    // 3. Disk Size, RAM, and CPU selectors
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 16

                        // Disk Size
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 6
                            Text {
                                text: qsTr("3. MAX DISK SIZE")
                                color: theme.muted
                                font.pixelSize: 11
                                font.bold: true
                            }
                            RowLayout {
                                spacing: 6
                                Repeater {
                                    model: ["20GiB", "35GiB", "60GiB", "100GiB"]
                                    delegate: Rectangle {
                                        required property string modelData
                                        height: 32
                                        width: 52
                                        radius: Math.min(4, theme.radius)
                                        color: win.newEnvDisk === modelData ? theme.accent : theme.background
                                        Text {
                                            anchors.centerIn: parent
                                            text: modelData
                                            color: win.newEnvDisk === modelData ? theme.accentForeground : theme.foreground
                                            font.pixelSize: 11
                                            font.bold: win.newEnvDisk === modelData
                                        }
                                        MouseArea {
                                            anchors.fill: parent
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: win.newEnvDisk = modelData
                                        }
                                    }
                                }
                            }
                        }

                        // RAM Limit
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 6
                            Text {
                                text: qsTr("4. RAM LIMIT")
                                color: theme.muted
                                font.pixelSize: 11
                                font.bold: true
                            }
                            RowLayout {
                                spacing: 6
                                Repeater {
                                    model: ["4GiB", "6GiB", "8GiB", "12GiB"]
                                    delegate: Rectangle {
                                        required property string modelData
                                        height: 32
                                        width: 48
                                        radius: Math.min(4, theme.radius)
                                        color: win.newEnvRam === modelData ? theme.accent : theme.background
                                        Text {
                                            anchors.centerIn: parent
                                            text: modelData
                                            color: win.newEnvRam === modelData ? theme.accentForeground : theme.foreground
                                            font.pixelSize: 11
                                            font.bold: win.newEnvRam === modelData
                                        }
                                        MouseArea {
                                            anchors.fill: parent
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: win.newEnvRam = modelData
                                        }
                                    }
                                }
                            }
                        }

                        // CPU Cores
                        ColumnLayout {
                            spacing: 6
                            Text {
                                text: qsTr("5. CPU CORES")
                                color: theme.muted
                                font.pixelSize: 11
                                font.bold: true
                            }
                            RowLayout {
                                spacing: 6
                                Repeater {
                                    model: [2, 4, 6, 8]
                                    delegate: Rectangle {
                                        required property int modelData
                                        height: 32
                                        width: 36
                                        radius: Math.min(4, theme.radius)
                                        color: win.newEnvCpu === modelData ? theme.accent : theme.background
                                        Text {
                                            anchors.centerIn: parent
                                            text: modelData
                                            color: win.newEnvCpu === modelData ? theme.accentForeground : theme.foreground
                                            font.pixelSize: 11
                                            font.bold: win.newEnvCpu === modelData
                                        }
                                        MouseArea {
                                            anchors.fill: parent
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: win.newEnvCpu = modelData
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Shared folder preview
                    Rectangle {
                        Layout.fillWidth: true
                        height: 36
                        radius: Math.min(4, theme.radius)
                        color: theme.background
                        Text {
                            anchors.centerIn: parent
                            text: qsTr("Shared Folder: %1/%2  ⟷  /workspace").arg(backend.workspaceBase).arg(win.newEnvName.length > 0 ? win.newEnvName : "<project-name>")
                            color: theme.muted
                            font.pixelSize: 12
                        }
                    }

                    // Dialog Buttons
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12
                        Item { Layout.fillWidth: true }

                        Rectangle {
                            height: 38
                            width: cancelCreateText.implicitWidth + 28
                            radius: theme.radius
                            color: theme.lighterBackground
                            Text {
                                id: cancelCreateText
                                anchors.centerIn: parent
                                text: qsTr("Cancel")
                                color: theme.foreground
                                font.pixelSize: 13
                            }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: win.createVisible = false
                            }
                        }

                        Rectangle {
                            height: 38
                            width: confirmCreateText.implicitWidth + 32
                            radius: theme.radius
                            color: theme.accent
                            Text {
                                id: confirmCreateText
                                anchors.centerIn: parent
                                text: qsTr("Create Environment")
                                color: theme.accentForeground
                                font.pixelSize: 13
                                font.bold: true
                            }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    backend.createEnvironment(win.newEnvName, win.newEnvOsId, win.newEnvPreset,
                                                              win.newEnvDisk, win.newEnvRam, win.newEnvCpu)
                                    win.createVisible = false
                                }
                            }
                        }
                    }
                }
            }
        }

        // ======================================================================
        // MODAL 2: WIPE / DELETE CONFIRMATION DIALOG
        // ======================================================================
        Rectangle {
            anchors.fill: parent
            visible: win.wipeVisible
            color: "#b0000000"

            MouseArea {
                anchors.fill: parent
                onClicked: win.wipeVisible = false
            }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(parent.width - 60, 480)
                height: wipeCol.implicitHeight + 48
                radius: theme.radius
                color: theme.darkBackground
                border.width: 2
                border.color: theme.red

                MouseArea { anchors.fill: parent }

                ColumnLayout {
                    id: wipeCol
                    anchors.fill: parent
                    anchors.margins: 24
                    spacing: 16

                    Text {
                        text: qsTr("Wipe Environment '%1'?").arg(backend.selectedEnvironment.name || "")
                        color: theme.foreground
                        font.pixelSize: 18
                        font.bold: true
                    }

                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        text: qsTr("This will permanently destroy the entire Linux OS container, installed packages, databases, and snapshots inside '%1' in 1 second. Your host OS remains untouched.").arg(backend.selectedEnvironment.name || "")
                        color: theme.muted
                        font.pixelSize: 13
                    }

                    CheckBox {
                        id: wipeFilesCheck
                        checked: win.deleteWorkspaceOnWipe
                        onCheckedChanged: win.deleteWorkspaceOnWipe = checked
                        text: qsTr("Also delete shared project files in %1").arg(backend.selectedEnvironment.workspacePath || "")
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12
                        Item { Layout.fillWidth: true }

                        Rectangle {
                            height: 36
                            width: cancelWipeText.implicitWidth + 24
                            radius: theme.radius
                            color: theme.lighterBackground
                            Text {
                                id: cancelWipeText
                                anchors.centerIn: parent
                                text: qsTr("Cancel")
                                color: theme.foreground
                                font.pixelSize: 13
                            }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: win.wipeVisible = false
                            }
                        }

                        Rectangle {
                            height: 36
                            width: confirmWipeText.implicitWidth + 28
                            radius: theme.radius
                            color: theme.red
                            Text {
                                id: confirmWipeText
                                anchors.centerIn: parent
                                text: qsTr("Wipe Environment Now")
                                color: "black"
                                font.pixelSize: 13
                                font.bold: true
                            }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    backend.wipeEnvironment(backend.selectedEnvironment.name || "", win.deleteWorkspaceOnWipe)
                                    win.wipeVisible = false
                                }
                            }
                        }
                    }
                }
            }
        }

        // ======================================================================
        // MODAL 3: KEYBOARD SHORTCUTS OVERLAY (?)
        // ======================================================================
        Rectangle {
            anchors.fill: parent
            visible: win.helpVisible
            color: "#b0000000"

            MouseArea {
                anchors.fill: parent
                onClicked: win.helpVisible = false
            }

            Rectangle {
                anchors.centerIn: parent
                width: 460
                height: helpCol.implicitHeight + 48
                radius: theme.radius
                color: theme.darkBackground
                border.width: 2
                border.color: theme.accent

                MouseArea { anchors.fill: parent }

                ColumnLayout {
                    id: helpCol
                    anchors.fill: parent
                    anchors.margins: 24
                    spacing: 12

                    Text {
                        text: qsTr("Keyboard Shortcuts")
                        color: theme.foreground
                        font.pixelSize: 18
                        font.bold: true
                    }

                    Repeater {
                        model: [
                            { k: "Space / Enter", d: qsTr("Enter selected OS terminal in /workspace") },
                            { k: "N", d: qsTr("Create a new isolated OS environment") },
                            { k: "F", d: qsTr("Open shared workspace folder on Host") },
                            { k: "S", d: qsTr("Start or stop selected environment") },
                            { k: "P", d: qsTr("Create instant 1-second Btrfs snapshot") },
                            { k: "C", d: qsTr("Copy AI agent execution command") },
                            { k: "Delete", d: qsTr("Wipe / destroy selected environment") },
                            { k: "1 / 2", d: qsTr("Switch between Environments and OS Catalog") },
                            { k: "R", d: qsTr("Refresh status and disk usage") },
                            { k: "?", d: qsTr("Toggle this shortcuts overlay") },
                            { k: "Q", d: qsTr("Quit AIBox") }
                        ]
                        delegate: RowLayout {
                            required property var modelData
                            Layout.fillWidth: true
                            Rectangle {
                                width: 110
                                height: 26
                                radius: Math.min(4, theme.radius)
                                color: theme.lighterBackground
                                Text {
                                    anchors.centerIn: parent
                                    text: modelData.k
                                    color: theme.accent
                                    font.pixelSize: 12
                                    font.bold: true
                                }
                            }
                            Text {
                                Layout.fillWidth: true
                                text: modelData.d
                                color: theme.foreground
                                font.pixelSize: 13
                            }
                        }
                    }
                }
            }
        }
    }
}
