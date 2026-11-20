import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import "../components"
import "create"

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
        workspaceSection.resetSharedPath()
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

            WorkspaceSection {
                id: workspaceSection
                app: createDialogRoot.app
            }

            OsPickerSection {
                app: createDialogRoot.app
            }

            ResourceSection {
                app: createDialogRoot.app
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
                                             app.newEnvSwap, app.newEnvSharedPath)
                } else {
                    backend.createEnvironment(app.newEnvName, app.newEnvOsId, app.newEnvPreset,
                                              app.newEnvDisk, app.newEnvRam, app.newEnvCpu,
                                              app.newEnvProcLimit, app.newEnvIoLimit,
                                              app.newEnvSwap, app.newEnvSharedPath)
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
}
