import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Layouts

ApplicationWindow {
    id: win
    width: 1080
    height: 720
    minimumWidth: 860
    minimumHeight: 560
    visible: true
    title: qsTr("AIBox")
    color: theme.background

    Material.theme: Material.Dark
    Material.accent: theme.accent
    Material.roundedScale: theme.radius

    readonly property string sansFont: "Adwaita Sans"
    readonly property string monoFont: "JetBrainsMono Nerd Font"
    readonly property color borderSubtle: "#232738"
    readonly property color surfaceCard: "#12131c"
    readonly property color surfaceInput: "#0b0c12"
    readonly property color surfaceSelected: "#192136"
    readonly property color accentColor: theme.accent
    readonly property color redColor: theme.red
    readonly property color greenColor: theme.green

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

    readonly property bool anyModalOpen: helpVisible || createVisible || wipeVisible

    // Reusable clean flat input box (replaces Material TextField completely)
    component FlatInput: Rectangle {
        id: root
        property alias text: textInput.text
        property string placeholder: ""
        property bool readOnly: false
        signal accepted()

        implicitHeight: 36
        radius: Math.min(4, theme.radius)
        color: win.surfaceInput
        border.width: 1
        border.color: textInput.activeFocus ? theme.accent : win.borderSubtle

        Text {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            visible: textInput.text.length === 0
            text: root.placeholder
            color: theme.muted
            font.family: win.sansFont
            font.pixelSize: 12
            elide: Text.ElideRight
        }

        TextInput {
            id: textInput
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            verticalAlignment: TextInput.AlignVCenter
            color: theme.foreground
            selectionColor: theme.accent
            selectedTextColor: theme.accentForeground
            selectByMouse: true
            clip: true
            readOnly: root.readOnly
            font.family: win.sansFont
            font.pixelSize: 13
            onAccepted: root.accepted()
        }
    }

    // Reusable sleek action button
    component ActionButton: Rectangle {
        id: btn
        property string label: ""
        property string shortcutHint: ""
        property string variant: "" // "primary" | "danger" | "success" | "accent" | ""
        property bool primary: false
        property bool danger: false
        property bool success: false
        property bool accent: false
        property bool disabled: false
        property int minWidth: 0
        signal clicked()

        readonly property bool isPrimary: primary || variant === "primary"
        readonly property bool isDanger: danger || variant === "danger"
        readonly property bool isSuccess: success || variant === "success"
        readonly property bool isAccent: accent || variant === "accent" || variant === "theme"

        implicitHeight: 34
        implicitWidth: minWidth > 0 ? Math.max(minWidth, btnRow.implicitWidth + 24)
                                    : (label === "?" ? 34 : btnRow.implicitWidth + 24)
        Layout.preferredHeight: implicitHeight
        Layout.preferredWidth: implicitWidth
        radius: Math.min(4, theme.radius)
        opacity: btn.disabled ? 0.4 : (btnMouse.containsMouse ? 0.9 : 1.0)
        color: btn.isPrimary
               ? theme.accent
               : (btn.isDanger
                  ? (btnMouse.containsMouse
                     ? Qt.rgba(win.redColor.r, win.redColor.g, win.redColor.b, 0.22)
                     : Qt.rgba(win.redColor.r, win.redColor.g, win.redColor.b, 0.12))
                  : (btn.isAccent
                     ? (btnMouse.containsMouse
                        ? Qt.rgba(win.accentColor.r, win.accentColor.g, win.accentColor.b, 0.22)
                        : Qt.rgba(win.accentColor.r, win.accentColor.g, win.accentColor.b, 0.12))
                     : (btn.isSuccess
                        ? (btnMouse.containsMouse
                           ? Qt.rgba(win.greenColor.r, win.greenColor.g, win.greenColor.b, 0.22)
                           : Qt.rgba(win.greenColor.r, win.greenColor.g, win.greenColor.b, 0.12))
                        : (btnMouse.containsMouse ? theme.lighterBackground : win.surfaceCard))))

        border.width: btn.isPrimary ? 0 : 1
        border.color: btn.isDanger
                      ? theme.red
                      : (btn.isAccent
                         ? theme.accent
                         : (btn.isSuccess
                            ? theme.green
                            : (btnMouse.containsMouse ? theme.muted : win.borderSubtle)))

        RowLayout {
            id: btnRow
            anchors.centerIn: parent
            spacing: 8

            Text {
                text: btn.label
                color: btn.isPrimary
                       ? theme.accentForeground
                       : (btn.isDanger
                          ? theme.red
                          : (btn.isAccent
                             ? theme.accent
                             : (btn.isSuccess ? theme.green : theme.foreground)))
                font.family: win.sansFont
                font.pixelSize: 12
                font.weight: btn.isPrimary ? Font.Medium : Font.Normal
            }

            Rectangle {
                visible: btn.shortcutHint.length > 0
                height: 18
                width: hintText.implicitWidth + 10
                radius: 2
                color: btn.isPrimary
                       ? Qt.rgba(0, 0, 0, 0.18)
                       : (btn.isDanger
                          ? Qt.rgba(win.redColor.r, win.redColor.g, win.redColor.b, 0.18)
                          : (btn.isAccent
                             ? Qt.rgba(win.accentColor.r, win.accentColor.g, win.accentColor.b, 0.18)
                             : (btn.isSuccess
                                ? Qt.rgba(win.greenColor.r, win.greenColor.g, win.greenColor.b, 0.18)
                                : win.surfaceInput)))
                border.width: btn.isPrimary ? 0 : 1
                border.color: btn.isDanger
                              ? Qt.rgba(win.redColor.r, win.redColor.g, win.redColor.b, 0.35)
                              : (btn.isAccent
                                 ? Qt.rgba(win.accentColor.r, win.accentColor.g, win.accentColor.b, 0.35)
                                 : (btn.isSuccess
                                    ? Qt.rgba(win.greenColor.r, win.greenColor.g, win.greenColor.b, 0.35)
                                    : win.borderSubtle))

                Text {
                    id: hintText
                    anchors.centerIn: parent
                    text: btn.shortcutHint
                    color: btn.isPrimary
                           ? theme.accentForeground
                           : (btn.isDanger
                              ? theme.red
                              : (btn.isAccent
                                 ? theme.accent
                                 : (btn.isSuccess ? theme.green : theme.muted)))
                    font.family: win.monoFont
                    font.pixelSize: 10
                }
            }
        }

        MouseArea {
            id: btnMouse
            anchors.fill: parent
            hoverEnabled: true
            enabled: !btn.disabled
            cursorShape: btn.disabled ? Qt.ForbiddenCursor : Qt.PointingHandCursor
            onClicked: btn.clicked()
        }
    }

    Shortcut {
        sequence: "?"
        context: Qt.ApplicationShortcut
        onActivated: win.helpVisible = !win.helpVisible
    }
    Shortcut {
        sequence: "Escape"
        context: Qt.ApplicationShortcut
        onActivated: {
            if (win.helpVisible) {
                win.helpVisible = false
            } else if (win.createVisible) {
                win.createVisible = false
            } else if (win.wipeVisible) {
                win.wipeVisible = false
            } else {
                backend.clearError()
            }
        }
    }
    Shortcut {
        sequence: "Q"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && !backend.busy
        onActivated: Qt.quit()
    }
    Shortcut {
        sequence: "N"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && !backend.busy
        onActivated: {
            win.newEnvName = ""
            backend.clearError()
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
        enabled: !win.anyModalOpen && win.activeTab === 0 && !backend.busy
        onActivated: backend.primaryAction()
    }
    Shortcut {
        sequence: "Return"
        context: Qt.ApplicationShortcut
        enabled: !win.anyModalOpen && win.activeTab === 0 && !backend.busy && backend.environments.length > 0
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
        onActivated: backend.createSnapshot(backend.selectedEnvironment.name || "", snapNameInput.text)
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

    Component.onCompleted: {
        if (backend.busy && backend.pendingEnvName.length > 0) {
            win.newEnvName = backend.pendingEnvName
            win.createVisible = true
        }
    }

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
                Layout.preferredHeight: 54
                color: theme.darkBackground

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 20
                    anchors.rightMargin: 20
                    spacing: 16

                    // Minimalist App Brand
                    RowLayout {
                        spacing: 10
                        Rectangle {
                            width: 28
                            height: 28
                            radius: Math.min(4, theme.radius)
                            color: win.surfaceSelected
                            border.width: 1
                            border.color: theme.accent
                            Text {
                                anchors.centerIn: parent
                                text: ">_"
                                color: theme.accent
                                font.family: win.monoFont
                                font.pixelSize: 12
                            }
                        }
                        Text {
                            text: qsTr("AIBox")
                            color: theme.foreground
                            font.family: win.sansFont
                            font.pixelSize: 15
                            font.weight: Font.Medium
                        }
                        Text {
                            text: "·"
                            color: theme.muted
                            font.pixelSize: 14
                        }
                        Text {
                            text: qsTr("Isolated Linux OS Manager")
                            color: theme.muted
                            font.family: win.sansFont
                            font.pixelSize: 12
                        }
                    }

                    // Segmented Tab Switcher
                    Rectangle {
                        Layout.leftMargin: 12
                        Layout.preferredHeight: 34
                        Layout.preferredWidth: segRow.implicitWidth + 8
                        Layout.alignment: Qt.AlignVCenter
                        radius: Math.min(4, theme.radius)
                        color: win.surfaceInput
                        border.width: 1
                        border.color: win.borderSubtle

                        RowLayout {
                            id: segRow
                            anchors.centerIn: parent
                            spacing: 4

                            Repeater {
                                model: [
                                    { label: qsTr("%n Environments", "", backend.environments.length), key: "1", tab: 0 },
                                    { label: qsTr("OS Images & Downloads"), key: "2", tab: 1 }
                                ]
                                delegate: Rectangle {
                                    required property var modelData
                                    height: 28
                                    width: tabItemRow.implicitWidth + 18
                                    radius: Math.min(3, theme.radius)
                                    color: win.activeTab === modelData.tab ? win.surfaceSelected : "transparent"
                                    border.width: win.activeTab === modelData.tab ? 1 : 0
                                    border.color: theme.accent

                                    RowLayout {
                                        id: tabItemRow
                                        anchors.centerIn: parent
                                        spacing: 8
                                        Text {
                                            text: modelData.label
                                            color: win.activeTab === modelData.tab ? theme.foreground : theme.muted
                                            font.family: win.sansFont
                                            font.pixelSize: 12
                                            font.weight: win.activeTab === modelData.tab ? Font.Medium : Font.Normal
                                        }
                                        Text {
                                            text: modelData.key
                                            color: theme.muted
                                            font.family: win.monoFont
                                            font.pixelSize: 10
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
                    }

                    Item { Layout.fillWidth: true }

                    RowLayout {
                        spacing: 10
                        Layout.alignment: Qt.AlignVCenter

                        ActionButton {
                            label: qsTr("Refresh")
                            shortcutHint: "R"
                            minWidth: 96
                            disabled: backend.busy
                            Layout.alignment: Qt.AlignVCenter
                            onClicked: backend.refresh()
                        }

                        ActionButton {
                            label: "?"
                            minWidth: 34
                            Layout.alignment: Qt.AlignVCenter
                            onClicked: win.helpVisible = !win.helpVisible
                        }

                        ActionButton {
                            label: qsTr("+ New Environment")
                            shortcutHint: "N"
                            variant: "primary"
                            minWidth: 165
                            disabled: backend.busy
                            Layout.alignment: Qt.AlignVCenter
                            onClicked: {
                                win.newEnvName = ""
                                backend.clearError()
                                win.createVisible = true
                            }
                        }
                    }
                }

                Rectangle {
                    anchors.bottom: parent.bottom
                    width: parent.width
                    height: 1
                    color: win.borderSubtle
                }
            }

            // ==================================================================
            // LIVE TASK PROGRESS BAR / ERROR BANNER (WHEN BUSY OUTSIDE MODAL)
            // ==================================================================
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: visible ? (backend.busy ? 68 : 42) : 0
                visible: (!win.createVisible && backend.busy) || backend.errorMessage.length > 0 || (!backend.busy && backend.statusMessage.length > 0)
                color: backend.errorMessage.length > 0 ? "#241217" : win.surfaceCard

                ColumnLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 20
                    anchors.rightMargin: 20
                    anchors.topMargin: 8
                    anchors.bottomMargin: 8
                    spacing: 6

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12

                        Text {
                            visible: backend.busy
                            text: backend.taskProgress + "%"
                            color: theme.accent
                            font.family: win.monoFont
                            font.pixelSize: 12
                        }

                        Text {
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                            text: backend.errorMessage.length > 0
                                  ? backend.errorMessage
                                  : (backend.busy ? backend.taskStage : backend.statusMessage)
                            color: backend.errorMessage.length > 0 ? theme.red : theme.foreground
                            font.family: win.sansFont
                            font.pixelSize: 12
                        }

                        Text {
                            visible: backend.busy && backend.taskLog.length > 0
                            Layout.maximumWidth: 340
                            elide: Text.ElideLeft
                            text: backend.taskLog
                            color: theme.muted
                            font.family: win.monoFont
                            font.pixelSize: 11
                        }

                        ActionButton {
                            visible: backend.busy
                            implicitHeight: 26
                            label: qsTr("Cancel")
                            danger: true
                            onClicked: backend.cancelTask()
                        }

                        ActionButton {
                            visible: backend.errorMessage.length > 0
                            implicitHeight: 26
                            label: qsTr("Dismiss")
                            onClicked: backend.clearError()
                        }
                    }

                    // Progress track when busy
                    Rectangle {
                        visible: backend.busy
                        Layout.fillWidth: true
                        height: 4
                        radius: 2
                        color: win.surfaceInput

                        Rectangle {
                            height: parent.height
                            width: parent.width * (Math.max(4, backend.taskProgress) / 100.0)
                            radius: 2
                            color: theme.accent
                            Behavior on width { NumberAnimation { duration: 150 } }
                        }
                    }
                }

                Rectangle {
                    anchors.bottom: parent.bottom
                    width: parent.width
                    height: 1
                    color: backend.errorMessage.length > 0 ? theme.red : win.borderSubtle
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
                    spacing: 16
                    visible: win.activeTab === 0

                    // EMPTY STATE WHEN NO ENVIRONMENTS EXIST
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        visible: backend.environments.length === 0
                        color: win.surfaceCard
                        radius: theme.radius
                        border.width: 1
                        border.color: win.borderSubtle

                        ColumnLayout {
                            anchors.centerIn: parent
                            width: Math.min(parent.width - 80, 480)
                            spacing: 12

                            Rectangle {
                                Layout.alignment: Qt.AlignHCenter
                                width: 42
                                height: 42
                                radius: Math.min(6, theme.radius)
                                color: win.surfaceInput
                                border.width: 1
                                border.color: win.borderSubtle
                                Text {
                                    anchors.centerIn: parent
                                    text: ">_"
                                    color: theme.accent
                                    font.family: win.monoFont
                                    font.pixelSize: 16
                                }
                            }

                            Text {
                                Layout.fillWidth: true
                                horizontalAlignment: Text.AlignHCenter
                                text: qsTr("No Isolated Environments Yet")
                                color: theme.foreground
                                font.family: win.sansFont
                                font.pixelSize: 16
                                font.weight: Font.Medium
                            }

                            Text {
                                Layout.fillWidth: true
                                horizontalAlignment: Text.AlignHCenter
                                wrapMode: Text.WordWrap
                                lineHeight: 1.35
                                text: qsTr("Create a clean Linux OS container or VM for your next AI, Desktop, or Mobile project. Give AI full root access inside, share files via /workspace, and wipe it in 1 second when done.")
                                color: theme.muted
                                font.family: win.sansFont
                                font.pixelSize: 12
                            }

                            RowLayout {
                                Layout.alignment: Qt.AlignHCenter
                                Layout.topMargin: 8
                                spacing: 10

                                ActionButton {
                                    label: qsTr("+ Create First Environment")
                                    shortcutHint: "N"
                                    primary: true
                                    disabled: backend.busy
                                    onClicked: {
                                        win.newEnvName = ""
                                        backend.clearError()
                                        win.createVisible = true
                                    }
                                }

                                ActionButton {
                                    label: qsTr("Browse OS Images")
                                    shortcutHint: "2"
                                    onClicked: win.activeTab = 1
                                }
                            }
                        }
                    }

                    // LEFT PANE: ENVIRONMENT LIST
                    Rectangle {
                        Layout.preferredWidth: 330
                        Layout.fillHeight: true
                        visible: backend.environments.length > 0
                        color: win.surfaceCard
                        radius: theme.radius
                        border.width: 1
                        border.color: win.borderSubtle

                        ColumnLayout {
                            anchors.fill: parent
                            anchors.margins: 12
                            spacing: 10

                            RowLayout {
                                Layout.fillWidth: true
                                Text {
                                    text: qsTr("Environments")
                                    color: theme.muted
                                    font.family: win.sansFont
                                    font.pixelSize: 11
                                    font.weight: Font.Medium
                                }
                                Item { Layout.fillWidth: true }
                                Text {
                                    text: qsTr("↑/↓ select · Space enter")
                                    color: theme.muted
                                    font.family: win.monoFont
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
                                    required property var modelData
                                    required property int index
                                    width: envListView.width
                                    height: 78

                                    Rectangle {
                                        anchors.fill: parent
                                        radius: Math.min(4, theme.radius)
                                        color: backend.selectedIndex === index ? win.surfaceSelected : win.surfaceInput
                                        border.width: 1
                                        border.color: backend.selectedIndex === index ? theme.accent : win.borderSubtle

                                        ColumnLayout {
                                            anchors.fill: parent
                                            anchors.margins: 12
                                            spacing: 4

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
                                                    font.family: win.monoFont
                                                    font.pixelSize: 13
                                                    font.weight: Font.Medium
                                                }

                                                Rectangle {
                                                    height: 18
                                                    width: statusBadgeText.implicitWidth + 10
                                                    radius: 2
                                                    color: modelData.running ? "#162920" : win.surfaceCard
                                                    border.width: 1
                                                    border.color: modelData.running ? "#284d3a" : win.borderSubtle
                                                    Text {
                                                        id: statusBadgeText
                                                        anchors.centerIn: parent
                                                        text: modelData.status
                                                        color: modelData.running ? theme.green : theme.muted
                                                        font.family: win.monoFont
                                                        font.pixelSize: 9
                                                    }
                                                }
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                elide: Text.ElideRight
                                                text: modelData.osName + " · " + modelData.preset
                                                color: theme.muted
                                                font.family: win.sansFont
                                                font.pixelSize: 11
                                            }

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 10
                                                Text {
                                                    text: modelData.diskLimit + " disk"
                                                    color: theme.muted
                                                    font.family: win.monoFont
                                                    font.pixelSize: 10
                                                }
                                                Text {
                                                    text: modelData.memoryUsage + " / " + modelData.memoryLimit
                                                    color: theme.muted
                                                    font.family: win.monoFont
                                                    font.pixelSize: 10
                                                }
                                                Item { Layout.fillWidth: true }
                                                Text {
                                                    text: modelData.ipv4
                                                    color: theme.accent
                                                    font.family: win.monoFont
                                                    font.pixelSize: 10
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
                        color: win.surfaceCard
                        radius: theme.radius
                        border.width: 1
                        border.color: win.borderSubtle

                        ColumnLayout {
                            anchors.fill: parent
                            anchors.margins: 20
                            spacing: 16

                            // Environment Header & Primary Action
                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 16

                                ColumnLayout {
                                    spacing: 4
                                    Layout.alignment: Qt.AlignVCenter

                                    RowLayout {
                                        spacing: 12
                                        Layout.alignment: Qt.AlignVCenter

                                        Text {
                                            id: selectedEnvTitle
                                            text: backend.selectedEnvironment.name || ""
                                            color: theme.foreground
                                            font.family: win.monoFont
                                            font.pixelSize: 18
                                            font.weight: Font.Medium
                                            elide: Text.ElideRight
                                            Layout.maximumWidth: 380
                                            Layout.alignment: Qt.AlignVCenter
                                        }

                                        Rectangle {
                                            implicitHeight: 22
                                            implicitWidth: statusBadgeRow.implicitWidth + 16
                                            Layout.preferredHeight: 22
                                            Layout.preferredWidth: implicitWidth
                                            Layout.alignment: Qt.AlignVCenter
                                            radius: 3
                                            color: (backend.selectedEnvironment.running || false) ? "#162920" : win.surfaceInput
                                            border.width: 1
                                            border.color: (backend.selectedEnvironment.running || false) ? "#284d3a" : win.borderSubtle

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
                                                    font.family: win.monoFont
                                                    font.pixelSize: 10
                                                    font.weight: Font.Medium
                                                    Layout.alignment: Qt.AlignVCenter
                                                }
                                            }
                                        }
                                    }

                                    Text {
                                        text: (backend.selectedEnvironment.osName || "") + "  ·  " + (backend.selectedEnvironment.preset || "")
                                        color: theme.muted
                                        font.family: win.sansFont
                                        font.pixelSize: 12
                                    }
                                }

                                Item { Layout.fillWidth: true }

                                ActionButton {
                                    label: qsTr("Enter OS Terminal")
                                    shortcutHint: "Space"
                                    variant: "primary"
                                    minWidth: 165
                                    disabled: backend.busy
                                    Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                                    onClicked: backend.enterEnvironment(backend.selectedEnvironment.name || "")
                                }
                            }

                            // Secondary Action Bar
                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10

                                ActionButton {
                                    label: (backend.selectedEnvironment.running || false) ? qsTr("Stop OS") : qsTr("Start OS")
                                    shortcutHint: "S"
                                    variant: (backend.selectedEnvironment.running || false) ? "danger" : "accent"
                                    minWidth: 110
                                    disabled: backend.busy
                                    Layout.alignment: Qt.AlignVCenter
                                    onClicked: backend.toggleStartStop(backend.selectedEnvironment.name || "")
                                }

                                ActionButton {
                                    label: qsTr("Open Shared Folder")
                                    shortcutHint: "F"
                                    minWidth: 165
                                    Layout.alignment: Qt.AlignVCenter
                                    onClicked: backend.openWorkspaceFolder(backend.selectedEnvironment.name || "")
                                }

                                ActionButton {
                                    label: qsTr("Copy AI Command")
                                    shortcutHint: "C"
                                    minWidth: 155
                                    Layout.alignment: Qt.AlignVCenter
                                    onClicked: backend.copyAiCommand(backend.selectedEnvironment.name || "")
                                }

                                Item { Layout.fillWidth: true }

                                ActionButton {
                                    label: qsTr("Wipe OS")
                                    shortcutHint: "Del"
                                    variant: "danger"
                                    minWidth: 110
                                    disabled: backend.busy
                                    Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                                    onClicked: {
                                        win.deleteWorkspaceOnWipe = false
                                        win.wipeVisible = true
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
                                        { title: qsTr("Disk Quota"), val: backend.selectedEnvironment.diskLimit || "—" },
                                        { title: qsTr("Memory Usage"), val: (backend.selectedEnvironment.memoryUsage || "0 MB") + " / " + (backend.selectedEnvironment.memoryLimit || "—") },
                                        { title: qsTr("CPU Allocation"), val: backend.selectedEnvironment.cpuLimit || "—" },
                                        { title: qsTr("IPv4 Address"), val: backend.selectedEnvironment.ipv4 || "—" }
                                    ]
                                    delegate: Rectangle {
                                        required property var modelData
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: 58
                                        radius: Math.min(4, theme.radius)
                                        color: win.surfaceInput
                                        border.width: 1
                                        border.color: win.borderSubtle

                                        ColumnLayout {
                                            anchors.fill: parent
                                            anchors.margins: 10
                                            spacing: 3
                                            Text {
                                                text: modelData.title
                                                color: theme.muted
                                                font.family: win.sansFont
                                                font.pixelSize: 11
                                            }
                                            Text {
                                                Layout.fillWidth: true
                                                elide: Text.ElideRight
                                                text: modelData.val
                                                color: theme.foreground
                                                font.family: win.monoFont
                                                font.pixelSize: 13
                                            }
                                        }
                                    }
                                }
                            }

                            // Shared Folder Info Box
                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 40
                                radius: Math.min(4, theme.radius)
                                color: win.surfaceInput
                                border.width: 1
                                border.color: win.borderSubtle

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 12
                                    anchors.rightMargin: 12
                                    spacing: 10

                                    Text {
                                        text: qsTr("Shared Workspace")
                                        color: theme.muted
                                        font.family: win.sansFont
                                        font.pixelSize: 11
                                    }
                                    Text {
                                        Layout.fillWidth: true
                                        elide: Text.ElideMiddle
                                        text: (backend.selectedEnvironment.workspacePath || "") + "  ⟷  /workspace"
                                        color: theme.accent
                                        font.family: win.monoFont
                                        font.pixelSize: 12
                                    }
                                }
                            }

                            // SNAPSHOTS (TIME MACHINE) SECTION
                            Rectangle {
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                radius: Math.min(4, theme.radius)
                                color: win.surfaceInput
                                border.width: 1
                                border.color: win.borderSubtle

                                ColumnLayout {
                                    anchors.fill: parent
                                    anchors.margins: 14
                                    spacing: 10

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10

                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            spacing: 2
                                            Text {
                                                text: qsTr("Btrfs Snapshots (Instant Rollback)")
                                                color: theme.foreground
                                                font.family: win.sansFont
                                                font.pixelSize: 12
                                                font.weight: Font.Medium
                                            }
                                            Text {
                                                text: qsTr("Save OS state in 1 second before major AI changes")
                                                color: theme.muted
                                                font.family: win.sansFont
                                                font.pixelSize: 11
                                            }
                                        }

                                        FlatInput {
                                            id: snapNameInput
                                            Layout.preferredWidth: 200
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
                                            implicitHeight: 34
                                            minWidth: 145
                                            label: qsTr("+ Save Snapshot")
                                            shortcutHint: "P"
                                            disabled: backend.busy
                                            onClicked: {
                                                backend.createSnapshot(backend.selectedEnvironment.name || "", snapNameInput.text)
                                                snapNameInput.text = ""
                                            }
                                        }
                                    }

                                    Rectangle {
                                        Layout.fillWidth: true
                                        height: 1
                                        color: win.borderSubtle
                                    }

                                    Text {
                                        visible: !backend.selectedEnvironment.snapshots || backend.selectedEnvironment.snapshots.length === 0
                                        text: qsTr("No snapshots saved for this environment yet.")
                                        color: theme.muted
                                        font.family: win.sansFont
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
                                            height: 36

                                            Rectangle {
                                                anchors.fill: parent
                                                radius: Math.min(3, theme.radius)
                                                color: win.surfaceCard
                                                border.width: 1
                                                border.color: win.borderSubtle

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

                                                    Text {
                                                        Layout.fillWidth: true
                                                        text: modelData
                                                        color: theme.foreground
                                                        font.family: win.monoFont
                                                        font.pixelSize: 12
                                                    }

                                                    ActionButton {
                                                        implicitHeight: 24
                                                        label: qsTr("Restore")
                                                        disabled: backend.busy
                                                        onClicked: backend.restoreSnapshot(backend.selectedEnvironment.name || "", modelData)
                                                    }

                                                    ActionButton {
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
                                color: win.surfaceInput
                                border.width: 1
                                border.color: win.borderSubtle

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 12
                                    anchors.rightMargin: 12
                                    spacing: 10

                                    Text {
                                        text: qsTr("Virtual Display:")
                                        color: theme.muted
                                        font.family: win.sansFont
                                        font.pixelSize: 12
                                    }

                                    FlatInput {
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
                                font.family: win.sansFont
                                font.pixelSize: 16
                                font.weight: Font.Medium
                            }
                            Text {
                                text: qsTr("Pre-download OS images to local storage so new environments launch in under 1 second, or click 'Use This OS' to start immediately.")
                                color: theme.muted
                                font.family: win.sansFont
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
                                color: win.surfaceCard
                                border.width: 1
                                border.color: modelData.downloaded ? "#284d3a" : win.borderSubtle

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 16
                                    anchors.rightMargin: 16
                                    spacing: 14

                                    Rectangle {
                                        width: 36
                                        height: 36
                                        radius: Math.min(4, theme.radius)
                                        color: modelData.downloaded ? "#162920" : win.surfaceInput
                                        border.width: 1
                                        border.color: modelData.downloaded ? "#284d3a" : win.borderSubtle
                                        Text {
                                            anchors.centerIn: parent
                                            text: modelData.downloaded ? "✓" : "↓"
                                            color: modelData.downloaded ? theme.green : theme.accent
                                            font.family: win.monoFont
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
                                                font.family: win.sansFont
                                                font.pixelSize: 14
                                                font.weight: Font.Medium
                                            }
                                            Rectangle {
                                                height: 18
                                                width: osBadgeText.implicitWidth + 10
                                                radius: 2
                                                color: win.surfaceInput
                                                border.width: 1
                                                border.color: win.borderSubtle
                                                Text {
                                                    id: osBadgeText
                                                    anchors.centerIn: parent
                                                    text: modelData.badge
                                                    color: theme.accent
                                                    font.family: win.monoFont
                                                    font.pixelSize: 10
                                                }
                                            }
                                            Rectangle {
                                                height: 18
                                                width: cacheStateText.implicitWidth + 10
                                                radius: 2
                                                color: modelData.downloaded ? "#162920" : win.surfaceInput
                                                border.width: 1
                                                border.color: modelData.downloaded ? "#284d3a" : win.borderSubtle
                                                Text {
                                                    id: cacheStateText
                                                    anchors.centerIn: parent
                                                    text: modelData.downloading ? qsTr("Downloading...") : modelData.sizeText
                                                    color: modelData.downloaded ? theme.green : theme.muted
                                                    font.family: win.monoFont
                                                    font.pixelSize: 10
                                                }
                                            }
                                        }

                                        Text {
                                            Layout.fillWidth: true
                                            elide: Text.ElideRight
                                            text: modelData.description
                                            color: theme.muted
                                            font.family: win.sansFont
                                            font.pixelSize: 12
                                        }
                                    }

                                    ActionButton {
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
                                        label: qsTr("Use This OS")
                                        variant: "primary"
                                        minWidth: 125
                                        disabled: backend.busy
                                        onClicked: {
                                            win.newEnvOsId = modelData.id
                                            win.newEnvName = ""
                                            backend.clearError()
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

        // ======================================================================
        // MODAL 1: CREATE NEW ENVIRONMENT DIALOG (WITH LIVE PROGRESS & CANCEL)
        // ======================================================================
        Rectangle {
            anchors.fill: parent
            visible: win.createVisible
            color: "#b806070a"

            MouseArea {
                anchors.fill: parent
                onClicked: {
                    if (!backend.busy)
                        win.createVisible = false
                }
            }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(parent.width - 60, 680)
                height: createCol.implicitHeight + 48
                radius: Math.min(6, theme.radius)
                color: win.surfaceCard
                border.width: 1
                border.color: win.borderSubtle

                MouseArea { anchors.fill: parent }

                ColumnLayout {
                    id: createCol
                    anchors.fill: parent
                    anchors.margins: 24
                    spacing: 16

                        RowLayout {
                            Layout.fillWidth: true
                            ColumnLayout {
                                spacing: 2
                                Text {
                                    text: backend.busy
                                          ? qsTr("Creating Environment '%1'...").arg(backend.pendingEnvName.length > 0 ? backend.pendingEnvName : win.newEnvName)
                                          : qsTr("Create Isolated Linux Environment")
                                    color: theme.foreground
                                    font.family: win.sansFont
                                    font.pixelSize: 16
                                    font.weight: Font.Medium
                                }
                                Text {
                                    text: backend.busy
                                          ? qsTr("Runs in background — safe to close window or press Esc to minimize.")
                                          : qsTr("Configure OS distribution, hardware limits, and shared folder.")
                                    color: theme.muted
                                    font.family: win.sansFont
                                    font.pixelSize: 12
                                }
                            }
                            Item { Layout.fillWidth: true }
                            Rectangle {
                                height: 22
                                width: 34
                                radius: 3
                                color: win.surfaceInput
                                border.width: 1
                                border.color: win.borderSubtle
                                Text {
                                    anchors.centerIn: parent
                                    text: "Esc"
                                    color: theme.muted
                                    font.family: win.monoFont
                                    font.pixelSize: 10
                                }
                                MouseArea {
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: win.createVisible = false
                                }
                            }
                        }

                    Rectangle {
                        Layout.fillWidth: true
                        height: 1
                        color: win.borderSubtle
                    }

                    // Error box inside modal if validation fails
                    Rectangle {
                        visible: backend.errorMessage.length > 0 && !backend.busy
                        Layout.fillWidth: true
                        height: 36
                        radius: Math.min(4, theme.radius)
                        color: "#261319"
                        border.width: 1
                        border.color: theme.red

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 12
                            Text {
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                                text: backend.errorMessage
                                color: theme.red
                                font.family: win.sansFont
                                font.pixelSize: 12
                            }
                        }
                    }

                    // ----------------------------------------------------------
                    // VIEW A: CREATION FORM (SHOWN WHEN NOT BUSY)
                    // ----------------------------------------------------------
                    ColumnLayout {
                        Layout.fillWidth: true
                        visible: !backend.busy
                        spacing: 16

                        // Project Name Field (Clean FlatInput, NO floating label!)
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 6

                            RowLayout {
                                Layout.fillWidth: true
                                Text {
                                    text: qsTr("Project / Environment Name")
                                    color: theme.foreground
                                    font.family: win.sansFont
                                    font.pixelSize: 12
                                    font.weight: Font.Medium
                                }
                                Item { Layout.fillWidth: true }
                                Text {
                                    text: qsTr("Shared at: %1/%2").arg(backend.workspaceBase).arg(win.newEnvName.length > 0 ? win.newEnvName : "<name>")
                                    color: theme.muted
                                    font.family: win.monoFont
                                    font.pixelSize: 11
                                }
                            }

                            FlatInput {
                                id: createNameField
                                Layout.fillWidth: true
                                implicitHeight: 38
                                placeholder: qsTr("e.g. pos-desktop, flutter-mobile, web-api")
                                text: win.newEnvName
                                onTextChanged: win.newEnvName = text
                            }
                        }

                        // 1. Capability Preset
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 6
                            Text {
                                text: qsTr("Environment Capability Preset")
                                color: theme.foreground
                                font.family: win.sansFont
                                font.pixelSize: 12
                                font.weight: Font.Medium
                            }
                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 8
                                Repeater {
                                    model: [
                                        { id: "full", title: qsTr("Desktop & Mobile"), sub: qsTr("GPU + KVM + GUI Bridge") },
                                        { id: "standard", title: qsTr("Standard AI / Web"), sub: qsTr("Docker + Isolated OS") },
                                        { id: "vm", title: qsTr("Virtual Machine"), sub: qsTr("Full KVM Kernel (--vm)") }
                                    ]
                                    delegate: Rectangle {
                                        required property var modelData
                                        readonly property bool selected: win.newEnvPreset === modelData.id
                                        Layout.fillWidth: true
                                        height: 50
                                        radius: Math.min(4, theme.radius)
                                        color: selected ? win.surfaceSelected : win.surfaceInput
                                        border.width: 1
                                        border.color: selected ? theme.accent : win.borderSubtle

                                        ColumnLayout {
                                            anchors.fill: parent
                                            anchors.leftMargin: 12
                                            anchors.rightMargin: 12
                                            anchors.topMargin: 7
                                            anchors.bottomMargin: 7
                                            spacing: 2

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 6
                                                Rectangle {
                                                    width: 6
                                                    height: 6
                                                    radius: 3
                                                    color: parent.parent.parent.selected ? theme.accent : theme.muted
                                                }
                                                Text {
                                                    Layout.fillWidth: true
                                                    elide: Text.ElideRight
                                                    text: modelData.title
                                                    color: parent.parent.parent.selected ? theme.foreground : theme.muted
                                                    font.family: win.sansFont
                                                    font.pixelSize: 12
                                                    font.weight: parent.parent.parent.selected ? Font.Medium : Font.Normal
                                                }
                                            }
                                            Text {
                                                Layout.fillWidth: true
                                                elide: Text.ElideRight
                                                text: modelData.sub
                                                color: theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 10
                                            }
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

                        // 2. Operating System (Distro)
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 6
                            Text {
                                text: qsTr("Linux Distribution")
                                color: theme.foreground
                                font.family: win.sansFont
                                font.pixelSize: 12
                                font.weight: Font.Medium
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
                                        readonly property bool selected: win.newEnvOsId === modelData.id
                                        Layout.fillWidth: true
                                        height: 46
                                        radius: Math.min(4, theme.radius)
                                        color: selected ? win.surfaceSelected : win.surfaceInput
                                        border.width: 1
                                        border.color: selected ? theme.accent : win.borderSubtle

                                        ColumnLayout {
                                            anchors.centerIn: parent
                                            spacing: 2
                                            Text {
                                                Layout.alignment: Qt.AlignHCenter
                                                text: modelData.id
                                                color: selected ? theme.foreground : theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 12
                                            }
                                            Text {
                                                Layout.alignment: Qt.AlignHCenter
                                                text: modelData.downloaded ? qsTr("✓ Cached") : modelData.badge
                                                color: modelData.downloaded ? theme.green : theme.muted
                                                font.family: win.sansFont
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

                        // 3. Disk, RAM, and CPU Quotas
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 16

                            // Disk Size
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 6
                                Text {
                                    text: qsTr("Disk Quota")
                                    color: theme.foreground
                                    font.family: win.sansFont
                                    font.pixelSize: 12
                                    font.weight: Font.Medium
                                }
                                RowLayout {
                                    spacing: 6
                                    Repeater {
                                        model: ["20GiB", "35GiB", "60GiB", "100GiB"]
                                        delegate: Rectangle {
                                            required property string modelData
                                            readonly property bool selected: win.newEnvDisk === modelData
                                            height: 32
                                            width: 54
                                            radius: Math.min(4, theme.radius)
                                            color: selected ? win.surfaceSelected : win.surfaceInput
                                            border.width: 1
                                            border.color: selected ? theme.accent : win.borderSubtle
                                            Text {
                                                anchors.centerIn: parent
                                                text: modelData
                                                color: selected ? theme.accent : theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 11
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
                                    text: qsTr("RAM Limit")
                                    color: theme.foreground
                                    font.family: win.sansFont
                                    font.pixelSize: 12
                                    font.weight: Font.Medium
                                }
                                RowLayout {
                                    spacing: 6
                                    Repeater {
                                        model: ["4GiB", "6GiB", "8GiB", "12GiB"]
                                        delegate: Rectangle {
                                            required property string modelData
                                            readonly property bool selected: win.newEnvRam === modelData
                                            height: 32
                                            width: 48
                                            radius: Math.min(4, theme.radius)
                                            color: selected ? win.surfaceSelected : win.surfaceInput
                                            border.width: 1
                                            border.color: selected ? theme.accent : win.borderSubtle
                                            Text {
                                                anchors.centerIn: parent
                                                text: modelData
                                                color: selected ? theme.accent : theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 11
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
                                    text: qsTr("CPU Cores")
                                    color: theme.foreground
                                    font.family: win.sansFont
                                    font.pixelSize: 12
                                    font.weight: Font.Medium
                                }
                                RowLayout {
                                    spacing: 6
                                    Repeater {
                                        model: [2, 4, 6, 8]
                                        delegate: Rectangle {
                                            required property int modelData
                                            readonly property bool selected: win.newEnvCpu === modelData
                                            height: 32
                                            width: 36
                                            radius: Math.min(4, theme.radius)
                                            color: selected ? win.surfaceSelected : win.surfaceInput
                                            border.width: 1
                                            border.color: selected ? theme.accent : win.borderSubtle
                                            Text {
                                                anchors.centerIn: parent
                                                text: modelData
                                                color: selected ? theme.accent : theme.muted
                                                font.family: win.monoFont
                                                font.pixelSize: 11
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

                        // Dialog Footer Buttons
                        RowLayout {
                            Layout.fillWidth: true
                            Layout.topMargin: 4
                            spacing: 10
                            Item { Layout.fillWidth: true }

                            ActionButton {
                                label: qsTr("Cancel")
                                minWidth: 96
                                onClicked: win.createVisible = false
                            }

                            ActionButton {
                                label: qsTr("Create Environment")
                                variant: "primary"
                                minWidth: 165
                                disabled: backend.busy || win.newEnvName.trim().length === 0
                                onClicked: {
                                    backend.createEnvironment(win.newEnvName, win.newEnvOsId, win.newEnvPreset,
                                                              win.newEnvDisk, win.newEnvRam, win.newEnvCpu)
                                }
                            }
                        }
                    }

                    // ----------------------------------------------------------
                    // VIEW B: LIVE PROGRESS & CANCELLATION PANEL (WHEN BUSY)
                    // ----------------------------------------------------------
                    ColumnLayout {
                        Layout.fillWidth: true
                        visible: backend.busy
                        spacing: 14

                        RowLayout {
                            Layout.fillWidth: true
                            Text {
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                                text: backend.taskStage
                                color: theme.foreground
                                font.family: win.sansFont
                                font.pixelSize: 13
                            }
                            Text {
                                text: backend.taskProgress + "%"
                                color: theme.accent
                                font.family: win.monoFont
                                font.pixelSize: 13
                            }
                        }

                        // Smooth Progress Bar
                        Rectangle {
                            Layout.fillWidth: true
                            height: 8
                            radius: 4
                            color: win.surfaceInput
                            border.width: 1
                            border.color: win.borderSubtle

                            Rectangle {
                                height: parent.height
                                width: parent.width * (Math.max(4, backend.taskProgress) / 100.0)
                                radius: 4
                                color: theme.accent
                                Behavior on width { NumberAnimation { duration: 150 } }
                            }
                        }

                        // Live Terminal Log Output Box
                        Rectangle {
                            Layout.fillWidth: true
                            height: 44
                            radius: Math.min(4, theme.radius)
                            color: win.surfaceInput
                            border.width: 1
                            border.color: win.borderSubtle

                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 12
                                anchors.rightMargin: 12
                                spacing: 8
                                Text {
                                    text: "$"
                                    color: theme.accent
                                    font.family: win.monoFont
                                    font.pixelSize: 11
                                }
                                Text {
                                    Layout.fillWidth: true
                                    elide: Text.ElideLeft
                                    text: backend.taskLog
                                    color: theme.muted
                                    font.family: win.monoFont
                                    font.pixelSize: 11
                                }
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            Text {
                                text: qsTr("Creation is locked while an operation is running.")
                                color: theme.muted
                                font.family: win.sansFont
                                font.pixelSize: 11
                            }
                            Item { Layout.fillWidth: true }
                            ActionButton {
                                label: qsTr("Cancel Creation")
                                variant: "danger"
                                minWidth: 140
                                onClicked: backend.cancelTask()
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
            color: "#b806070a"

            MouseArea {
                anchors.fill: parent
                onClicked: win.wipeVisible = false
            }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(parent.width - 60, 460)
                height: wipeCol.implicitHeight + 48
                radius: Math.min(6, theme.radius)
                color: win.surfaceCard
                border.width: 1
                border.color: theme.red

                MouseArea { anchors.fill: parent }

                ColumnLayout {
                    id: wipeCol
                    anchors.fill: parent
                    anchors.margins: 24
                    spacing: 14

                    Text {
                        text: qsTr("Wipe Environment '%1'?").arg(backend.selectedEnvironment.name || "")
                        color: theme.foreground
                        font.family: win.sansFont
                        font.pixelSize: 16
                        font.weight: Font.Medium
                    }

                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        lineHeight: 1.35
                        text: qsTr("This will permanently destroy the entire Linux OS container, installed packages, databases, and snapshots inside '%1' in 1 second. Your host OS remains untouched.").arg(backend.selectedEnvironment.name || "")
                        color: theme.muted
                        font.family: win.sansFont
                        font.pixelSize: 12
                    }

                    // Custom clean checkbox row
                    Rectangle {
                        Layout.fillWidth: true
                        height: 38
                        radius: Math.min(4, theme.radius)
                        color: win.surfaceInput
                        border.width: 1
                        border.color: win.deleteWorkspaceOnWipe ? theme.red : win.borderSubtle

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 12
                            spacing: 10

                            Rectangle {
                                width: 16
                                height: 16
                                radius: 3
                                color: win.deleteWorkspaceOnWipe ? theme.red : "transparent"
                                border.width: 1
                                border.color: win.deleteWorkspaceOnWipe ? theme.red : theme.muted
                                Text {
                                    anchors.centerIn: parent
                                    visible: win.deleteWorkspaceOnWipe
                                    text: "✓"
                                    color: "black"
                                    font.family: win.monoFont
                                    font.pixelSize: 11
                                }
                            }

                            Text {
                                Layout.fillWidth: true
                                elide: Text.ElideMiddle
                                text: qsTr("Also delete shared project files in %1").arg(backend.selectedEnvironment.workspacePath || "")
                                color: theme.foreground
                                font.family: win.sansFont
                                font.pixelSize: 12
                            }
                        }

                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: win.deleteWorkspaceOnWipe = !win.deleteWorkspaceOnWipe
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.topMargin: 4
                        spacing: 10
                        Item { Layout.fillWidth: true }

                        ActionButton {
                            label: qsTr("Cancel")
                            minWidth: 96
                            onClicked: win.wipeVisible = false
                        }

                        ActionButton {
                            label: qsTr("Wipe Environment Now")
                            variant: "danger"
                            minWidth: 185
                            onClicked: {
                                backend.wipeEnvironment(backend.selectedEnvironment.name || "", win.deleteWorkspaceOnWipe)
                                win.wipeVisible = false
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
            color: "#b806070a"

            MouseArea {
                anchors.fill: parent
                onClicked: win.helpVisible = false
            }

            Rectangle {
                anchors.centerIn: parent
                width: 440
                height: helpCol.implicitHeight + 48
                radius: Math.min(6, theme.radius)
                color: win.surfaceCard
                border.width: 1
                border.color: win.borderSubtle

                MouseArea { anchors.fill: parent }

                ColumnLayout {
                    id: helpCol
                    anchors.fill: parent
                    anchors.margins: 24
                    spacing: 10

                    Text {
                        text: qsTr("Keyboard Shortcuts")
                        color: theme.foreground
                        font.family: win.sansFont
                        font.pixelSize: 15
                        font.weight: Font.Medium
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        height: 1
                        color: win.borderSubtle
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
                            spacing: 12
                            Rectangle {
                                width: 104
                                height: 24
                                radius: 3
                                color: win.surfaceInput
                                border.width: 1
                                border.color: win.borderSubtle
                                Text {
                                    anchors.centerIn: parent
                                    text: modelData.k
                                    color: theme.accent
                                    font.family: win.monoFont
                                    font.pixelSize: 11
                                }
                            }
                            Text {
                                Layout.fillWidth: true
                                text: modelData.d
                                color: theme.muted
                                font.family: win.sansFont
                                font.pixelSize: 12
                            }
                        }
                    }
                }
            }
        }
    }
}
