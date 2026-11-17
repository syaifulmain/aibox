import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts
import "../components"

Rectangle {
    id: createDialogRoot
    property QtObject app: null
    anchors.fill: parent
    visible: app.createVisible
    color: "#c406070a"

    // The window resets this field when it opens the dialog.
    function resetSharedPath() {
        createSharedPathField.text = ""
    }

    MouseArea {
        anchors.fill: parent
        onClicked: {
            if (!backend.busy)
                app.createVisible = false
        }
    }

    Rectangle {
        anchors.centerIn: parent
        width: Math.min(parent.width - 60, 680)
        // The dialog follows its content, so the form stays compact in
        // template mode instead of leaving a large empty area.
        height: Math.min(parent.height - 40, createCol.implicitHeight + 44)
        radius: Math.min(8, theme.radius)
        color: app.surfaceCard
        border.width: 1
        border.color: theme.accent

        MouseArea { anchors.fill: parent }

        ColumnLayout {
            id: createCol
            anchors.fill: parent
            anchors.margins: 22
            spacing: 12

            ModalHeader {
                app: createDialogRoot.app
                title: backend.busy
                       ? qsTr("Creating Environment '%1'...").arg(backend.pendingEnvName.length > 0 ? backend.pendingEnvName : app.newEnvName)
                       : qsTr("Create Isolated Linux Environment")
                subtitle: backend.busy
                          ? qsTr("Runs in background — safe to close window or press Esc to minimize.")
                          : qsTr("Configure OS distribution, hardware limits, and shared folder.")
                onClosed: app.createVisible = false
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
                        font.family: app.sansFont
                        font.pixelSize: 12
                    }
                }
            }

            // ----------------------------------------------------------
            // VIEW A: CREATION FORM (SHOWN WHEN NOT BUSY)
            // ----------------------------------------------------------
            ScrollView {
                id: createFormScroll
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.preferredHeight: createFormContent.implicitHeight
                Layout.minimumHeight: 140
                visible: !backend.busy
                clip: true
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

                ColumnLayout {
                    id: createFormContent
                    width: createFormScroll.availableWidth
                    spacing: 12

                // Project Name Field (Clean FlatInput, NO floating label!)
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 6

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 10
                        Text {
                            text: qsTr("Project / Environment Name")
                            color: theme.foreground
                            font.family: app.sansFont
                            font.pixelSize: 12
                            font.weight: Font.Medium
                        }
                        Text {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            horizontalAlignment: Text.AlignRight
                            text: qsTr("Shared at: %1/%2")
                                      .arg(app.newEnvSharedPath.trim().length > 0 ? app.newEnvSharedPath.trim() : backend.workspaceBase)
                                      .arg(app.newEnvName.length > 0 ? app.newEnvName : "<name>")
                            color: theme.muted
                            font.family: app.monoFont
                            font.pixelSize: 11
                            elide: Text.ElideMiddle
                        }
                    }

                    FlatInput {
                        app: createDialogRoot.app
                        id: createNameField
                        Layout.fillWidth: true
                        placeholder: qsTr("e.g. pos-desktop, flutter-mobile, web-api")
                        text: app.newEnvName
                        onTextChanged: app.newEnvName = text
                    }
                }

                // Optional custom shared folder location (defaults to Settings base)
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 6

                    Text {
                        text: qsTr("Shared Folder Location (optional)")
                        color: theme.foreground
                        font.family: app.sansFont
                        font.pixelSize: 12
                        font.weight: Font.Medium
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        FlatInput {
                            app: createDialogRoot.app
                            id: createSharedPathField
                            Layout.fillWidth: true
                            placeholder: qsTr("Default: %1").arg(backend.workspaceBase)
                            text: app.newEnvSharedPath
                            onTextChanged: app.newEnvSharedPath = text
                        }

                        ActionButton {
                            app: createDialogRoot.app
                            label: qsTr("Browse...")
                            minWidth: 94
                            onClicked: createSharedFolderDialog.open()
                        }

                        ActionButton {
                            app: createDialogRoot.app
                            label: qsTr("Default")
                            minWidth: 84
                            onClicked: {
                                app.newEnvSharedPath = ""
                                createSharedPathField.text = ""
                            }
                        }
                    }
                }

                // Source mode switcher (shown when templates exist)
                RowLayout {
                    visible: backend.templates.length > 0
                    Layout.fillWidth: true
                    spacing: 8

                    Repeater {
                        model: [
                            { fromTpl: false, label: qsTr("Fresh OS") },
                            { fromTpl: true, label: qsTr("From Template (%1)").arg(backend.templates.length) }
                        ]
                        delegate: Rectangle {
                            required property var modelData
                            readonly property bool selected: app.createFromTemplate === modelData.fromTpl
                            Layout.fillWidth: true
                            height: 34
                            radius: Math.min(4, theme.radius)
                            color: selected ? app.surfaceSelected : app.surfaceInput
                            border.width: 1
                            border.color: selected ? theme.accent : app.borderSubtle
                            Text {
                                anchors.centerIn: parent
                                text: modelData.label
                                color: selected ? theme.foreground : theme.muted
                                font.family: app.sansFont
                                font.pixelSize: 12
                                font.weight: selected ? Font.Medium : Font.Normal
                            }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    app.createFromTemplate = modelData.fromTpl
                                    if (modelData.fromTpl && app.newEnvTemplateName.length === 0 && backend.templates.length > 0)
                                        app.newEnvTemplateName = backend.templates[0].name || ""
                                }
                            }
                        }
                    }
                }

                // Template Picker (when From Template is selected)
                ColumnLayout {
                    visible: app.createFromTemplate && backend.templates.length > 0
                    Layout.fillWidth: true
                    spacing: 6
                    Text {
                        text: qsTr("Select Template")
                        color: theme.foreground
                        font.family: app.sansFont
                        font.pixelSize: 12
                        font.weight: Font.Medium
                    }
                    GridLayout {
                        Layout.fillWidth: true
                        columns: 2
                        columnSpacing: 8
                        rowSpacing: 8
                        Repeater {
                            model: backend.templates
                            delegate: Rectangle {
                                required property var modelData
                                readonly property bool selected: app.newEnvTemplateName === modelData.name
                                Layout.fillWidth: true
                                height: 50
                                radius: Math.min(4, theme.radius)
                                color: selected ? app.surfaceSelected : app.surfaceInput
                                border.width: 1
                                border.color: selected ? theme.accent : app.borderSubtle

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
                                            text: modelData.name
                                            color: parent.parent.parent.selected ? theme.foreground : theme.muted
                                            font.family: app.monoFont
                                            font.pixelSize: 12
                                        }
                                    }
                                    Text {
                                        Layout.fillWidth: true
                                        elide: Text.ElideRight
                                        text: modelData.osName + " · " + modelData.diskLimit + " · " + modelData.memoryLimit
                                        color: theme.muted
                                        font.family: app.sansFont
                                        font.pixelSize: 10
                                    }
                                }

                                MouseArea {
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: app.newEnvTemplateName = modelData.name
                                }
                            }
                        }
                    }
                }

                // 1. Capability Preset
                ColumnLayout {
                    visible: !app.createFromTemplate
                    Layout.fillWidth: true
                    spacing: 6
                    Text {
                        text: qsTr("Environment Capability Preset")
                        color: theme.foreground
                        font.family: app.sansFont
                        font.pixelSize: 12
                        font.weight: Font.Medium
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8
                        Repeater {
                            model: backend.presets
                            delegate: Rectangle {
                                required property var modelData
                                readonly property bool selected: app.newEnvPreset === modelData.id
                                Layout.fillWidth: true
                                height: 50
                                radius: Math.min(4, theme.radius)
                                color: selected ? app.surfaceSelected : app.surfaceInput
                                border.width: 1
                                border.color: selected ? theme.accent : app.borderSubtle

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
                                            font.family: app.sansFont
                                            font.pixelSize: 12
                                            font.weight: parent.parent.parent.selected ? Font.Medium : Font.Normal
                                        }
                                    }
                                    Text {
                                        Layout.fillWidth: true
                                        elide: Text.ElideRight
                                        text: modelData.sub
                                        color: theme.muted
                                        font.family: app.monoFont
                                        font.pixelSize: 10
                                    }
                                }
                                MouseArea {
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: app.newEnvPreset = modelData.id
                                }
                            }
                        }
                    }
                }

                // 2. Operating System (Distro)
                ColumnLayout {
                    visible: !app.createFromTemplate
                    Layout.fillWidth: true
                    spacing: 6
                    Text {
                        text: qsTr("Linux Distribution")
                        color: theme.foreground
                        font.family: app.sansFont
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
                                readonly property bool selected: app.newEnvOsId === modelData.id
                                Layout.fillWidth: true
                                height: 46
                                radius: Math.min(4, theme.radius)
                                color: selected ? app.surfaceSelected : app.surfaceInput
                                border.width: 1
                                border.color: selected ? theme.accent : app.borderSubtle

                                ColumnLayout {
                                    anchors.centerIn: parent
                                    spacing: 2
                                    Text {
                                        Layout.alignment: Qt.AlignHCenter
                                        text: modelData.id
                                        color: selected ? theme.foreground : theme.muted
                                        font.family: app.monoFont
                                        font.pixelSize: 12
                                    }
                                    Text {
                                        Layout.alignment: Qt.AlignHCenter
                                        text: modelData.downloaded ? qsTr("✓ Cached") : modelData.badge
                                        color: modelData.downloaded ? theme.green : theme.muted
                                        font.family: app.sansFont
                                        font.pixelSize: 10
                                    }
                                }
                                MouseArea {
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: app.newEnvOsId = modelData.id
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
                            font.family: app.sansFont
                            font.pixelSize: 12
                            font.weight: Font.Medium
                        }
                        RowLayout {
                            spacing: 6
                            Repeater {
                                model: backend.diskOptions
                                delegate: Rectangle {
                                    required property string modelData
                                    readonly property bool selected: app.newEnvDisk === modelData
                                    Layout.fillWidth: true
                                    Layout.preferredWidth: 0
                                    height: 32
                                    radius: Math.min(4, theme.radius)
                                    color: selected ? app.surfaceSelected : app.surfaceInput
                                    border.width: 1
                                    border.color: selected ? theme.accent : app.borderSubtle
                                    Text {
                                        anchors.centerIn: parent
                                        width: parent.width - 4
                                        horizontalAlignment: Text.AlignHCenter
                                        elide: Text.ElideRight
                                        text: modelData
                                        color: selected ? theme.accent : theme.muted
                                        font.family: app.monoFont
                                        font.pixelSize: 11
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: app.newEnvDisk = modelData
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
                            font.family: app.sansFont
                            font.pixelSize: 12
                            font.weight: Font.Medium
                        }
                        RowLayout {
                            spacing: 6
                            Repeater {
                                model: backend.ramOptions
                                delegate: Rectangle {
                                    required property string modelData
                                    readonly property bool selected: app.newEnvRam === modelData
                                    Layout.fillWidth: true
                                    Layout.preferredWidth: 0
                                    height: 32
                                    radius: Math.min(4, theme.radius)
                                    color: selected ? app.surfaceSelected : app.surfaceInput
                                    border.width: 1
                                    border.color: selected ? theme.accent : app.borderSubtle
                                    Text {
                                        anchors.centerIn: parent
                                        width: parent.width - 4
                                        horizontalAlignment: Text.AlignHCenter
                                        elide: Text.ElideRight
                                        text: modelData
                                        color: selected ? theme.accent : theme.muted
                                        font.family: app.monoFont
                                        font.pixelSize: 11
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: app.newEnvRam = modelData
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
                            font.family: app.sansFont
                            font.pixelSize: 12
                            font.weight: Font.Medium
                        }
                        RowLayout {
                            spacing: 6
                            Repeater {
                                model: backend.cpuOptions
                                delegate: Rectangle {
                                    required property int modelData
                                    readonly property bool selected: app.newEnvCpu === modelData
                                    Layout.fillWidth: true
                                    Layout.preferredWidth: 0
                                    height: 32
                                    radius: Math.min(4, theme.radius)
                                    color: selected ? app.surfaceSelected : app.surfaceInput
                                    border.width: 1
                                    border.color: selected ? theme.accent : app.borderSubtle
                                    Text {
                                        anchors.centerIn: parent
                                        width: parent.width - 4
                                        horizontalAlignment: Text.AlignHCenter
                                        elide: Text.ElideRight
                                        text: modelData
                                        color: selected ? theme.accent : theme.muted
                                        font.family: app.monoFont
                                        font.pixelSize: 11
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: app.newEnvCpu = modelData
                                    }
                                }
                            }
                        }
                    }
                }

                // 4. Runaway Limits (Process Limit & Disk I/O Rate)
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 16

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 6
                        Text {
                            text: qsTr("Process Limit (Container)")
                            color: theme.foreground
                            font.family: app.sansFont
                            font.pixelSize: 12
                            font.weight: Font.Medium
                        }
                        RowLayout {
                            spacing: 6
                            Repeater {
                                model: backend.processLimitOptions
                                delegate: Rectangle {
                                    required property string modelData
                                    readonly property bool selected: app.newEnvProcLimit === modelData
                                    Layout.fillWidth: true
                                    Layout.preferredWidth: 0
                                    height: 30
                                    radius: Math.min(4, theme.radius)
                                    color: selected ? app.surfaceSelected : app.surfaceInput
                                    border.width: 1
                                    border.color: selected ? theme.accent : app.borderSubtle
                                    Text {
                                        id: procChipTxt
                                        anchors.centerIn: parent
                                        width: parent.width - 4
                                        horizontalAlignment: Text.AlignHCenter
                                        elide: Text.ElideRight
                                        text: modelData
                                        color: selected ? theme.accent : theme.muted
                                        font.family: app.monoFont
                                        font.pixelSize: 11
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: app.newEnvProcLimit = modelData
                                    }
                                }
                            }
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 6
                        Text {
                            text: qsTr("Disk I/O Limit (Read / Write)")
                            color: theme.foreground
                            font.family: app.sansFont
                            font.pixelSize: 12
                            font.weight: Font.Medium
                        }
                        RowLayout {
                            spacing: 6
                            Repeater {
                                model: backend.ioLimitOptions
                                delegate: Rectangle {
                                    required property string modelData
                                    readonly property bool selected: app.newEnvIoLimit === modelData
                                    Layout.fillWidth: true
                                    Layout.preferredWidth: 0
                                    height: 30
                                    radius: Math.min(4, theme.radius)
                                    color: selected ? app.surfaceSelected : app.surfaceInput
                                    border.width: 1
                                    border.color: selected ? theme.accent : app.borderSubtle
                                    Text {
                                        id: ioChipTxt
                                        anchors.centerIn: parent
                                        width: parent.width - 4
                                        horizontalAlignment: Text.AlignHCenter
                                        elide: Text.ElideRight
                                        text: modelData
                                        color: selected ? theme.accent : theme.muted
                                        font.family: app.monoFont
                                        font.pixelSize: 11
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: app.newEnvIoLimit = modelData
                                    }
                                }
                            }
                        }
                    }
                }

                // 5. Provisioning Presets (Ubuntu / Debian / Arch)
                ColumnLayout {
                    visible: !app.createFromTemplate
                             && (app.newEnvOsId.indexOf("ubuntu") >= 0
                                 || app.newEnvOsId.indexOf("debian") >= 0
                                 || app.newEnvOsId.indexOf("arch") >= 0)
                    Layout.fillWidth: true
                    spacing: 6

                    RowLayout {
                        Layout.fillWidth: true
                        Text {
                            text: qsTr("Provisioning Presets (Optional)")
                            color: theme.foreground
                            font.family: app.sansFont
                            font.pixelSize: 12
                            font.weight: Font.Medium
                        }
                        Item { Layout.fillWidth: true }
                        Text {
                            text: qsTr("Runs via incus exec after boot (non-fatal)")
                            color: theme.muted
                            font.family: app.sansFont
                            font.pixelSize: 10
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8
                        Repeater {
                            model: backend.provisionBundles
                            delegate: Rectangle {
                                required property var modelData
                                readonly property bool selected: app.newEnvProvisionBundles.indexOf(modelData.id) >= 0
                                Layout.fillWidth: true
                                height: 32
                                radius: Math.min(4, theme.radius)
                                color: selected ? app.surfaceSelected : app.surfaceInput
                                border.width: 1
                                border.color: selected ? theme.accent : app.borderSubtle
                                Text {
                                    anchors.centerIn: parent
                                    text: (selected ? "✓ " : "") + modelData.title
                                    color: selected ? theme.foreground : theme.muted
                                    font.family: app.sansFont
                                    font.pixelSize: 11
                                    font.weight: selected ? Font.Medium : Font.Normal
                                }
                                MouseArea {
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: app.toggleProvisionBundle(modelData.id)
                                }
                            }
                        }
                    }
                }

                // Storage pool overcommit warning (non-blocking)
                Rectangle {
                    readonly property string warnMsg: app.createVisible ? backend.poolWarningForCreate(app.newEnvDisk) : ""
                    visible: warnMsg.length > 0
                    Layout.fillWidth: true
                    implicitHeight: createPoolWarnTxt.implicitHeight + 16
                    radius: Math.min(4, theme.radius)
                    color: "#262013"
                    border.width: 1
                    border.color: "#e5a93c"
                    Text {
                        id: createPoolWarnTxt
                        anchors.fill: parent
                        anchors.margins: 8
                        anchors.leftMargin: 12
                        anchors.rightMargin: 12
                        wrapMode: Text.WordWrap
                        text: parent.warnMsg
                        color: "#e5a93c"
                        font.family: app.sansFont
                        font.pixelSize: 11
                    }
                }
                }
                }

                // Dialog Footer Buttons
                RowLayout {
                    Layout.fillWidth: true
                    Layout.topMargin: 4
                    visible: !backend.busy
                    spacing: 10
                    Item { Layout.fillWidth: true }

                    ActionButton {
                        app: createDialogRoot.app
                        label: qsTr("Cancel")
                        minWidth: 96
                        onClicked: app.createVisible = false
                    }

                    ActionButton {
                        app: createDialogRoot.app
                        label: app.createFromTemplate ? qsTr("Create from Template") : qsTr("Create Environment")
                        variant: "primary"
                        minWidth: 165
                        disabled: backend.busy || app.newEnvName.trim().length === 0 || (app.createFromTemplate && app.newEnvTemplateName.length === 0)
                        onClicked: {
                            if (app.createFromTemplate) {
                                backend.cloneEnvironment(app.newEnvTemplateName, app.newEnvName,
                                                         app.newEnvDisk, app.newEnvRam, app.newEnvCpu, true,
                                                         app.newEnvProcLimit, app.newEnvIoLimit,
                                                         app.newEnvSharedPath)
                            } else {
                                backend.createEnvironment(app.newEnvName, app.newEnvOsId, app.newEnvPreset,
                                                          app.newEnvDisk, app.newEnvRam, app.newEnvCpu,
                                                          app.newEnvProcLimit, app.newEnvIoLimit,
                                                          app.newEnvProvisionBundles, app.newEnvSharedPath)
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
                        font.family: app.sansFont
                        font.pixelSize: 13
                    }
                    Text {
                        text: backend.taskProgress + "%"
                        color: theme.accent
                        font.family: app.monoFont
                        font.pixelSize: 13
                    }
                }

                // Smooth Progress Bar
                Rectangle {
                    Layout.fillWidth: true
                    height: 8
                    radius: 4
                    color: app.surfaceInput
                    border.width: 1
                    border.color: app.borderSubtle

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
                    color: app.surfaceInput
                    border.width: 1
                    border.color: app.borderSubtle

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 12
                        anchors.rightMargin: 12
                        spacing: 8
                        Text {
                            text: "$"
                            color: theme.accent
                            font.family: app.monoFont
                            font.pixelSize: 11
                        }
                        Text {
                            Layout.fillWidth: true
                            elide: Text.ElideLeft
                            text: backend.taskLog
                            color: theme.muted
                            font.family: app.monoFont
                            font.pixelSize: 11
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    Text {
                        text: qsTr("Creation is locked while an operation is running.")
                        color: theme.muted
                        font.family: app.sansFont
                        font.pixelSize: 11
                    }
                    Item { Layout.fillWidth: true }
                    ActionButton {
                        app: createDialogRoot.app
                        label: qsTr("Cancel Creation")
                        variant: "danger"
                        minWidth: 140
                        onClicked: backend.cancelTask()
                    }
                }
            }
        }
    }

    FolderDialog {
        id: createSharedFolderDialog
        title: qsTr("Choose Custom Shared Folder Location")
        currentFolder: backend.pathToUrl(app.newEnvSharedPath.trim().length > 0
                                         ? app.newEnvSharedPath.trim() : backend.workspaceBase)
        onAccepted: {
            app.newEnvSharedPath = backend.localPath(selectedFolder)
            createSharedPathField.text = app.newEnvSharedPath
        }
    }
}
