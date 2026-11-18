import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import QtQuick.Dialogs
import "../components"

ModalDialog {
    id: createDialogRoot
    property QtObject app: null
    open: app.createVisible
    size: "md"
    cardHeight: Math.min(parent.height - Style.modalGutter, contentHeight + Style.padModal * 2)
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
        implicitHeight: createErrTxt.implicitHeight + Style.padCard * 2
        radius: Math.min(Style.radiusMd, theme.radius)
        color: theme.dangerSurface
        border.width: Style.hairline
        border.color: theme.dangerBorder

        Text {
            id: createErrTxt
            anchors.fill: parent
            anchors.margins: Style.padCard
            wrapMode: Text.WordWrap
            text: backend.errorMessage
            color: theme.dangerText
            font.family: Style.fontSans
            font.pixelSize: Style.fontBody
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
        ScrollBar.vertical.policy: ScrollBar.AlwaysOff

        ColumnLayout {
            id: createFormContent
            width: createFormScroll.availableWidth
            spacing: Style.padCard

            FormRow {
                label: qsTr("Project / Environment Name")
                hint: qsTr("Shared at: %1/%2")
                      .arg(app.newEnvSharedPath.trim().length > 0 ? app.newEnvSharedPath.trim() : backend.workspaceBase)
                      .arg(app.newEnvName.length > 0 ? app.newEnvName : "<name>")

                FlatInput {
                    id: createNameField
                    Layout.fillWidth: true
                    placeholder: qsTr("e.g. pos-desktop, flutter-mobile, web-api")
                    text: app.newEnvName
                    onTextChanged: app.newEnvName = text
                }
            }

            FormRow {
                label: qsTr("Shared Folder Location (optional)")

                FlatInput {
                    id: createSharedPathField
                    Layout.fillWidth: true
                    placeholder: qsTr("Default: %1").arg(backend.workspaceBase)
                    text: app.newEnvSharedPath
                    onTextChanged: app.newEnvSharedPath = text
                }

                ActionButton {
                    label: qsTr("Browse...")
                    onClicked: createSharedFolderDialog.open()
                }

                ActionButton {
                    label: qsTr("Default")
                    onClicked: {
                        app.newEnvSharedPath = ""
                        createSharedPathField.text = ""
                    }
                }
            }

            // Source mode switcher (shown when templates exist)
            RowLayout {
                visible: backend.templates.length > 0
                Layout.fillWidth: true
                spacing: Style.gapMd

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
                        textSize: Style.fontBody
                        mediumWhenSelected: true
                        selectedTextColor: theme.textPrimary
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
                spacing: Style.gapSm

                Text {
                    text: qsTr("Select Template")
                    color: theme.textPrimary
                    font.family: Style.fontSans
                    font.pixelSize: Style.fontBody
                    font.weight: Font.Medium
                }

                GridLayout {
                    Layout.fillWidth: true
                    columns: 2
                    columnSpacing: Style.gapMd
                    rowSpacing: Style.gapMd

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
                spacing: Style.gapSm

                Text {
                    text: qsTr("Environment Capability Preset")
                    color: theme.textPrimary
                    font.family: Style.fontSans
                    font.pixelSize: Style.fontBody
                    font.weight: Font.Medium
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Style.gapMd

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
                spacing: Style.gapSm

                Text {
                    text: qsTr("Linux Distribution")
                    color: theme.textPrimary
                    font.family: Style.fontSans
                    font.pixelSize: Style.fontBody
                    font.weight: Font.Medium
                }

                GridLayout {
                    Layout.fillWidth: true
                    columns: 4
                    columnSpacing: Style.gapMd
                    rowSpacing: Style.gapMd

                    Repeater {
                        model: backend.osCatalog
                        delegate: Rectangle {
                            required property var modelData
                            readonly property bool selected: app.newEnvOsId === modelData.id
                            Layout.fillWidth: true
                            implicitHeight: osCardCol.implicitHeight + Style.gapMd * 2
                            radius: Math.min(Style.radiusMd, theme.radius)
                            color: selected ? theme.surfaceSelected : theme.surfaceInput
                            border.width: Style.hairline
                            border.color: selected ? theme.accentBorder : theme.borderSubtle

                            ColumnLayout {
                                id: osCardCol
                                anchors.centerIn: parent
                                spacing: Style.gapXs

                                Text {
                                    Layout.alignment: Qt.AlignHCenter
                                    text: modelData.id
                                    color: selected ? theme.textPrimary : theme.textSecondary
                                    font.family: Style.fontMono
                                    font.pixelSize: Style.fontBody
                                }
                                Text {
                                    Layout.alignment: Qt.AlignHCenter
                                    text: modelData.downloaded ? qsTr("✓ Cached") : modelData.badge
                                    color: modelData.downloaded ? theme.successText : theme.textMuted
                                    font.family: Style.fontSans
                                    font.pixelSize: Style.fontCaption
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
                spacing: Style.padSection

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Style.gapSm

                    Text {
                        text: qsTr("Disk Quota")
                        color: theme.textPrimary
                        font.family: Style.fontSans
                        font.pixelSize: Style.fontBody
                        font.weight: Font.Medium
                    }

                    RowLayout {
                        spacing: Style.gapSm

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
                    spacing: Style.gapSm

                    Text {
                        text: qsTr("RAM Limit")
                        color: theme.textPrimary
                        font.family: Style.fontSans
                        font.pixelSize: Style.fontBody
                        font.weight: Font.Medium
                    }

                    RowLayout {
                        spacing: Style.gapSm

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
                    spacing: Style.gapSm

                    Text {
                        text: qsTr("CPU Cores")
                        color: theme.textPrimary
                        font.family: Style.fontSans
                        font.pixelSize: Style.fontBody
                        font.weight: Font.Medium
                    }

                    RowLayout {
                        spacing: Style.gapSm

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
                spacing: Style.padSection

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Style.gapSm

                    Text {
                        text: qsTr("Process Limit (Container)")
                        color: theme.textPrimary
                        font.family: Style.fontSans
                        font.pixelSize: Style.fontBody
                        font.weight: Font.Medium
                    }

                    RowLayout {
                        spacing: Style.gapSm

                        Repeater {
                            model: backend.processLimitOptions
                            delegate: SelectChip {
                                required property string modelData
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                text: modelData
                                selected: app.newEnvProcLimit === modelData
                                stretchText: true
                                compact: true
                                onClicked: app.newEnvProcLimit = modelData
                            }
                        }
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Style.gapSm

                    Text {
                        text: qsTr("Disk I/O Limit (Read / Write)")
                        color: theme.textPrimary
                        font.family: Style.fontSans
                        font.pixelSize: Style.fontBody
                        font.weight: Font.Medium
                    }

                    RowLayout {
                        spacing: Style.gapSm

                        Repeater {
                            model: backend.ioLimitOptions
                            delegate: SelectChip {
                                required property string modelData
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                text: modelData
                                selected: app.newEnvIoLimit === modelData
                                stretchText: true
                                compact: true
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
                implicitHeight: createPoolWarnTxt.implicitHeight + Style.padSection
                radius: Math.min(Style.radiusMd, theme.radius)
                color: theme.warningSurface
                border.width: Style.hairline
                border.color: theme.warningBorder

                Text {
                    id: createPoolWarnTxt
                    anchors.fill: parent
                    anchors.margins: Style.gapMd
                    anchors.leftMargin: Style.padCard
                    anchors.rightMargin: Style.padCard
                    wrapMode: Text.WordWrap
                    text: parent.warnMsg
                    color: theme.warningText
                    font.family: Style.fontSans
                    font.pixelSize: Style.fontSmall
                }
            }
        }
    }

    // Dialog footer (creation form)
    DialogFooter {
        visible: !backend.busy

        ActionButton {
            label: app.createFromTemplate ? qsTr("Create from Template") : qsTr("Create Environment")
            variant: "primary"
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
        spacing: Style.padCard

        RowLayout {
            Layout.fillWidth: true

            Text {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: backend.taskStage
                color: theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.fontBody
            }

            Text {
                text: backend.taskProgress + "%"
                color: theme.accentText
                font.family: Style.fontMono
                font.pixelSize: Style.fontBody
            }
        }

        MeterBar {
            Layout.fillWidth: true
            implicitHeight: Style.gapMd
            value: backend.taskProgress
            trackColor: theme.surfaceTrack
            minFillWidth: Style.gapSm
            showAtZero: true
            animationMs: 150
        }

        // Live terminal log output
        Rectangle {
            Layout.fillWidth: true
            height: Style.controlHeight + Style.gap
            radius: Math.min(Style.radiusMd, theme.radius)
            color: theme.surfaceInput
            border.width: Style.hairline
            border.color: theme.borderSubtle

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: Style.padCard
                anchors.rightMargin: Style.padCard
                spacing: Style.gapMd

                Text {
                    text: "$"
                    color: theme.accentText
                    font.family: Style.fontMono
                    font.pixelSize: Style.fontSmall
                }

                Text {
                    Layout.fillWidth: true
                    elide: Text.ElideLeft
                    text: backend.taskLog
                    color: theme.textMuted
                    font.family: Style.fontMono
                    font.pixelSize: Style.fontSmall
                }
            }
        }

        DialogFooter {
            leftActions: Text {
                text: qsTr("Creation is locked while an operation is running.")
                color: theme.textMuted
                font.family: Style.fontSans
                font.pixelSize: Style.fontSmall
            }

            ActionButton {
                label: qsTr("Cancel Creation")
                variant: "danger"
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
