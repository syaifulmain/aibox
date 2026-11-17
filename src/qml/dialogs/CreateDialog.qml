import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import QtQuick.Dialogs
import "../components"

ModalDialog {
    id: createDialogRoot
    property QtObject app: null
    open: app.createVisible
    cardWidth: Math.min(parent.width - Style.modalGutter, 680)
    cardHeight: Math.min(parent.height - Style.modalEdge, contentHeight + Style.modalPadding * 2)
    padding: Style.modalPadding
    onDismissed: {
        if (!backend.busy)
            app.createVisible = false
    }

    // The window resets this field when it opens the dialog.
    function resetSharedPath() {
        createSharedPathField.text = ""
    }

    ModalHeader {
        title: backend.busy
               ? qsTr("Creating Environment '%1'...").arg(backend.pendingEnvName.length > 0 ? backend.pendingEnvName : app.newEnvName)
               : qsTr("Create Isolated Linux Environment")
        subtitle: backend.busy
                  ? qsTr("Runs in background — safe to close window or press Esc to minimize.")
                  : qsTr("Configure OS distribution, hardware limits, and shared folder.")
        onClosed: app.createVisible = false
    }

    // Inline validation error
    Rectangle {
        visible: backend.errorMessage.length > 0 && !backend.busy
        Layout.fillWidth: true
        height: 36
        radius: Math.min(Style.radius4, theme.radius)
        color: Style.dangerSurface
        border.width: Style.hairline
        border.color: theme.red

        Text {
            anchors.fill: parent
            anchors.leftMargin: Style.space12
            anchors.rightMargin: Style.space12
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
            text: backend.errorMessage
            color: theme.red
            font.family: Style.fontSans
            font.pixelSize: Style.font12
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
            spacing: Style.space12

            // Project name and resulting shared path
            ColumnLayout {
                Layout.fillWidth: true
                spacing: Style.space6

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Style.space10

                    Text {
                        text: qsTr("Project / Environment Name")
                        color: theme.foreground
                        font.family: Style.fontSans
                        font.pixelSize: Style.font12
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
                        font.family: Style.fontMono
                        font.pixelSize: Style.font11
                        elide: Text.ElideMiddle
                    }
                }

                FlatInput {
                    id: createNameField
                    Layout.fillWidth: true
                    placeholder: qsTr("e.g. pos-desktop, flutter-mobile, web-api")
                    text: app.newEnvName
                    onTextChanged: app.newEnvName = text
                }
            }

            // Optional custom shared folder location (defaults to the configured base)
            ColumnLayout {
                Layout.fillWidth: true
                spacing: Style.space6

                Text {
                    text: qsTr("Shared Folder Location (optional)")
                    color: theme.foreground
                    font.family: Style.fontSans
                    font.pixelSize: Style.font12
                    font.weight: Font.Medium
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Style.space8

                    FlatInput {
                        id: createSharedPathField
                        Layout.fillWidth: true
                        placeholder: qsTr("Default: %1").arg(backend.workspaceBase)
                        text: app.newEnvSharedPath
                        onTextChanged: app.newEnvSharedPath = text
                    }

                    ActionButton {
                        label: qsTr("Browse...")
                        minWidth: 94
                        onClicked: createSharedFolderDialog.open()
                    }

                    ActionButton {
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
                spacing: Style.space8

                Repeater {
                    model: [
                        { fromTpl: false, label: qsTr("Fresh OS") },
                        { fromTpl: true, label: qsTr("From Template (%1)").arg(backend.templates.length) }
                    ]
                    delegate: SelectChip {
                        required property var modelData
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        text: modelData.label
                        selected: app.createFromTemplate === modelData.fromTpl
                        mono: false
                        textSize: Style.font12
                        mediumWhenSelected: true
                        selectedTextColor: theme.foreground
                        chipHeight: Style.controlHeight
                        onClicked: {
                            app.createFromTemplate = modelData.fromTpl
                            if (modelData.fromTpl && app.newEnvTemplateName.length === 0 && backend.templates.length > 0)
                                app.newEnvTemplateName = backend.templates[0].name || ""
                        }
                    }
                }
            }

            // Template picker (when From Template is selected)
            ColumnLayout {
                visible: app.createFromTemplate && backend.templates.length > 0
                Layout.fillWidth: true
                spacing: Style.space6

                Text {
                    text: qsTr("Select Template")
                    color: theme.foreground
                    font.family: Style.fontSans
                    font.pixelSize: Style.font12
                    font.weight: Font.Medium
                }

                GridLayout {
                    Layout.fillWidth: true
                    columns: 2
                    columnSpacing: Style.space8
                    rowSpacing: Style.space8

                    Repeater {
                        model: backend.templates
                        delegate: SelectCard {
                            required property var modelData
                            Layout.fillWidth: true
                            selected: app.newEnvTemplateName === modelData.name
                            title: modelData.name
                            subtitle: modelData.osName + " · " + modelData.diskLimit + " · " + modelData.memoryLimit
                            titleMono: true
                            onClicked: app.newEnvTemplateName = modelData.name
                        }
                    }
                }
            }

            // Capability preset
            ColumnLayout {
                visible: !app.createFromTemplate
                Layout.fillWidth: true
                spacing: Style.space6

                Text {
                    text: qsTr("Environment Capability Preset")
                    color: theme.foreground
                    font.family: Style.fontSans
                    font.pixelSize: Style.font12
                    font.weight: Font.Medium
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Style.space8

                    Repeater {
                        model: backend.presets
                        delegate: SelectCard {
                            required property var modelData
                            Layout.fillWidth: true
                            selected: app.newEnvPreset === modelData.id
                            title: modelData.title
                            subtitle: modelData.sub
                            subtitleMono: true
                            mediumWhenSelected: true
                            onClicked: app.newEnvPreset = modelData.id
                        }
                    }
                }
            }

            // Linux distribution
            ColumnLayout {
                visible: !app.createFromTemplate
                Layout.fillWidth: true
                spacing: Style.space6

                Text {
                    text: qsTr("Linux Distribution")
                    color: theme.foreground
                    font.family: Style.fontSans
                    font.pixelSize: Style.font12
                    font.weight: Font.Medium
                }

                GridLayout {
                    Layout.fillWidth: true
                    columns: 4
                    columnSpacing: Style.space8
                    rowSpacing: Style.space8

                    Repeater {
                        model: backend.osCatalog
                        delegate: Rectangle {
                            required property var modelData
                            readonly property bool selected: app.newEnvOsId === modelData.id
                            Layout.fillWidth: true
                            height: 46
                            radius: Math.min(Style.radius4, theme.radius)
                            color: selected ? Style.surfaceSelected : Style.surfaceInput
                            border.width: Style.hairline
                            border.color: selected ? theme.accent : Style.borderSubtle

                            ColumnLayout {
                                anchors.centerIn: parent
                                spacing: Style.space2

                                Text {
                                    Layout.alignment: Qt.AlignHCenter
                                    text: modelData.id
                                    color: selected ? theme.foreground : theme.muted
                                    font.family: Style.fontMono
                                    font.pixelSize: Style.font12
                                }
                                Text {
                                    Layout.alignment: Qt.AlignHCenter
                                    text: modelData.downloaded ? qsTr("✓ Cached") : modelData.badge
                                    color: modelData.downloaded ? theme.green : theme.muted
                                    font.family: Style.fontSans
                                    font.pixelSize: Style.font10
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

            // Disk, RAM and CPU quotas
            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space16

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Style.space6

                    Text {
                        text: qsTr("Disk Quota")
                        color: theme.foreground
                        font.family: Style.fontSans
                        font.pixelSize: Style.font12
                        font.weight: Font.Medium
                    }

                    RowLayout {
                        spacing: Style.space6

                        Repeater {
                            model: backend.diskOptions
                            delegate: SelectChip {
                                required property string modelData
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                text: modelData
                                selected: app.newEnvDisk === modelData
                                stretchText: true
                                onClicked: app.newEnvDisk = modelData
                            }
                        }
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Style.space6

                    Text {
                        text: qsTr("RAM Limit")
                        color: theme.foreground
                        font.family: Style.fontSans
                        font.pixelSize: Style.font12
                        font.weight: Font.Medium
                    }

                    RowLayout {
                        spacing: Style.space6

                        Repeater {
                            model: backend.ramOptions
                            delegate: SelectChip {
                                required property string modelData
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                text: modelData
                                selected: app.newEnvRam === modelData
                                stretchText: true
                                onClicked: app.newEnvRam = modelData
                            }
                        }
                    }
                }

                ColumnLayout {
                    spacing: Style.space6

                    Text {
                        text: qsTr("CPU Cores")
                        color: theme.foreground
                        font.family: Style.fontSans
                        font.pixelSize: Style.font12
                        font.weight: Font.Medium
                    }

                    RowLayout {
                        spacing: Style.space6

                        Repeater {
                            model: backend.cpuOptions
                            delegate: SelectChip {
                                required property int modelData
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                text: modelData
                                selected: app.newEnvCpu === modelData
                                stretchText: true
                                onClicked: app.newEnvCpu = modelData
                            }
                        }
                    }
                }
            }

            // Runaway limits (processes and disk I/O)
            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space16

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Style.space6

                    Text {
                        text: qsTr("Process Limit (Container)")
                        color: theme.foreground
                        font.family: Style.fontSans
                        font.pixelSize: Style.font12
                        font.weight: Font.Medium
                    }

                    RowLayout {
                        spacing: Style.space6

                        Repeater {
                            model: backend.processLimitOptions
                            delegate: SelectChip {
                                required property string modelData
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                text: modelData
                                selected: app.newEnvProcLimit === modelData
                                stretchText: true
                                chipHeight: 30
                                onClicked: app.newEnvProcLimit = modelData
                            }
                        }
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Style.space6

                    Text {
                        text: qsTr("Disk I/O Limit (Read / Write)")
                        color: theme.foreground
                        font.family: Style.fontSans
                        font.pixelSize: Style.font12
                        font.weight: Font.Medium
                    }

                    RowLayout {
                        spacing: Style.space6

                        Repeater {
                            model: backend.ioLimitOptions
                            delegate: SelectChip {
                                required property string modelData
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                text: modelData
                                selected: app.newEnvIoLimit === modelData
                                stretchText: true
                                chipHeight: 30
                                onClicked: app.newEnvIoLimit = modelData
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
                implicitHeight: createPoolWarnTxt.implicitHeight + Style.space16
                radius: Math.min(Style.radius4, theme.radius)
                color: Style.warningSurface
                border.width: Style.hairline
                border.color: Style.warning

                Text {
                    id: createPoolWarnTxt
                    anchors.fill: parent
                    anchors.margins: Style.space8
                    anchors.leftMargin: Style.space12
                    anchors.rightMargin: Style.space12
                    wrapMode: Text.WordWrap
                    text: parent.warnMsg
                    color: Style.warning
                    font.family: Style.fontSans
                    font.pixelSize: Style.font11
                }
            }
        }
    }

    // Dialog footer
    RowLayout {
        Layout.fillWidth: true
        Layout.topMargin: Style.space4
        visible: !backend.busy
        spacing: Style.space10
        Item { Layout.fillWidth: true }

        ActionButton {
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
                                              app.newEnvSharedPath)
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
        spacing: Style.space14

        RowLayout {
            Layout.fillWidth: true

            Text {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: backend.taskStage
                color: theme.foreground
                font.family: Style.fontSans
                font.pixelSize: Style.font13
            }

            Text {
                text: backend.taskProgress + "%"
                color: theme.accent
                font.family: Style.fontMono
                font.pixelSize: Style.font13
            }
        }

        MeterBar {
            Layout.fillWidth: true
            implicitHeight: 8
            value: backend.taskProgress
            trackColor: Style.surfaceInput
            minFillWidth: Style.space4
            showAtZero: true
            animationMs: 150
        }

        // Live terminal log output
        Rectangle {
            Layout.fillWidth: true
            height: 44
            radius: Math.min(Style.radius4, theme.radius)
            color: Style.surfaceInput
            border.width: Style.hairline
            border.color: Style.borderSubtle

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: Style.space12
                anchors.rightMargin: Style.space12
                spacing: Style.space8

                Text {
                    text: "$"
                    color: theme.accent
                    font.family: Style.fontMono
                    font.pixelSize: Style.font11
                }

                Text {
                    Layout.fillWidth: true
                    elide: Text.ElideLeft
                    text: backend.taskLog
                    color: theme.muted
                    font.family: Style.fontMono
                    font.pixelSize: Style.font11
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true

            Text {
                text: qsTr("Creation is locked while an operation is running.")
                color: theme.muted
                font.family: Style.fontSans
                font.pixelSize: Style.font11
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
