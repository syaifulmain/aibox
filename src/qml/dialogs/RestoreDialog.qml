import QtQuick
import QtQuick.Layouts
import "../components"

ModalDialog {
    id: restoreDialogRoot
    property QtObject app: null
    readonly property var env: backend.selectedEnvironment
    open: app.restoreDialogVisible
    size: "sm"
    onDismissed: app.restoreDialogVisible = false

    ModalHeader {
        title: qsTr("Restore Snapshot '%1'").arg(app.restoreSnapName)
        subtitle: qsTr("Choose what to restore for environment '%1':").arg(restoreDialogRoot.env.name || "")
        onClosed: app.restoreDialogVisible = false
    }

    ColumnLayout {
        Layout.fillWidth: true
        spacing: Style.gapMd

        Repeater {
            model: [
                { id: "both", title: qsTr("Both (System + Workspace)"), desc: qsTr("Restore OS container rootfs and %1 together.").arg(backend.containerWorkspacePath), enabled: app.restoreHasSystem && app.restoreHasWorkspace },
                { id: "system", title: qsTr("System only"), desc: qsTr("Restore OS container rootfs; leave %1 untouched.").arg(backend.containerWorkspacePath), enabled: app.restoreHasSystem },
                { id: "workspace", title: qsTr("Workspace only"), desc: qsTr("Restore %1 from Btrfs snapshot; leave OS packages untouched.").arg(backend.containerWorkspacePath), enabled: app.restoreHasWorkspace }
            ]
            delegate: Rectangle {
                required property var modelData
                Layout.fillWidth: true
                implicitHeight: restoreOptionRow.implicitHeight + Style.padCard * 2
                radius: Math.min(Style.radiusMd, theme.radius)
                opacity: modelData.enabled ? 1.0 : Style.opacityDisabled
                color: app.restoreMode === modelData.id ? theme.surfaceSelected : theme.surfaceInput
                border.width: Style.hairline
                border.color: app.restoreMode === modelData.id ? theme.accentBorder : theme.borderSubtle

                RowLayout {
                    id: restoreOptionRow
                    anchors.fill: parent
                    anchors.leftMargin: Style.padCard
                    anchors.rightMargin: Style.padCard
                    spacing: Style.gap

                    // Radio indicator
                    Rectangle {
                        width: Style.padSection
                        height: Style.padSection
                        radius: width / 2
                        color: "transparent"
                        border.width: Style.hairline
                        border.color: app.restoreMode === modelData.id ? theme.accent : theme.textMuted
                        Rectangle {
                            anchors.centerIn: parent
                            width: Style.gapMd
                            height: Style.gapMd
                            radius: width / 2
                            visible: app.restoreMode === modelData.id
                            color: theme.accent
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: Style.gapXs
                        Text {
                            text: modelData.title
                            color: theme.textPrimary
                            font.family: Style.fontSans
                            font.pixelSize: Style.fontBody
                            font.weight: Font.Medium
                        }
                        Text {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            text: modelData.desc
                            color: theme.textMuted
                            font.family: Style.fontSans
                            font.pixelSize: Style.fontSmall
                        }
                    }
                }

                MouseArea {
                    anchors.fill: parent
                    enabled: modelData.enabled
                    cursorShape: modelData.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                    onClicked: app.restoreMode = modelData.id
                }
            }
        }
    }

    Rectangle {
        Layout.fillWidth: true
        visible: app.restoreMode !== "system"
        implicitHeight: restoreNoteText.implicitHeight + Style.padSection
        radius: Math.min(Style.radiusMd, theme.radius)
        color: theme.surfaceInput
        border.width: Style.hairline
        border.color: theme.borderSubtle

        Text {
            id: restoreNoteText
            anchors.fill: parent
            anchors.margins: Style.gapMd
            wrapMode: Text.WordWrap
            lineHeight: 1.3
            text: (restoreDialogRoot.env.running || false)
                  ? qsTr("A safety snapshot (pre-restore) of your current %1 will be created first, and '%2' will be stopped automatically before swapping %1.").arg(backend.containerWorkspacePath).arg(restoreDialogRoot.env.name || "")
                  : qsTr("A safety snapshot (pre-restore) of your current %1 will be created before swapping %1.").arg(backend.containerWorkspacePath)
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }
    }

    DialogFooter {
        ActionButton {
            label: qsTr("Restore Snapshot")
            variant: "primary"
            disabled: backend.busy
            onClicked: {
                backend.restoreSnapshot(restoreDialogRoot.env.name || "", app.restoreSnapName, app.restoreMode)
                app.restoreDialogVisible = false
            }
        }
    }
}
