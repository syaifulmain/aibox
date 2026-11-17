import QtQuick
import QtQuick.Layouts
import "../../components"

// Settings section 1: Incus storage pool usage and growth.
ColumnLayout {
    id: root
    property QtObject app: null

    Layout.fillWidth: true
    spacing: Style.space12

    function reload() {
        app.poolResizeDraft = ""
        app.poolResizeConfirmVisible = false
        app.poolResizeError = ""
        poolSizeField.text = ""
    }

    // Pool details unavailable
    Rectangle {
        Layout.fillWidth: true
        visible: !(backend.poolInfo.valid || false)
        implicitHeight: poolUnavailableCol.implicitHeight + Style.cardPadding * 2
        radius: Math.min(Style.radius6, theme.radius)
        color: Style.surfaceInput
        border.width: Style.hairline
        border.color: Style.borderSubtle

        ColumnLayout {
            id: poolUnavailableCol
            anchors.fill: parent
            anchors.margins: Style.cardPadding
            spacing: Style.space8

            Text {
                text: qsTr("Pool details unavailable")
                color: theme.foreground
                font.family: Style.fontSans
                font.pixelSize: Style.font12
                font.weight: Font.Medium
            }
            Text {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: qsTr("AIBox could not read the Incus storage pool yet. Make sure Incus is initialized with the default pool, then refresh.")
                color: theme.muted
                font.family: Style.fontSans
                font.pixelSize: Style.font10
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space10
                Item { Layout.fillWidth: true }
                ActionButton {
                    label: qsTr("Refresh")
                    minWidth: 96
                    disabled: backend.busy
                    onClicked: backend.refresh()
                }
            }
        }
    }

    // Live pool card
    Rectangle {
        Layout.fillWidth: true
        visible: backend.poolInfo.valid || false
        implicitHeight: poolSettingsCol.implicitHeight + Style.cardPadding * 2
        radius: Math.min(Style.radius6, theme.radius)
        color: Style.surfaceInput
        border.width: Style.hairline
        border.color: backend.poolInfo.alertLevel === 2
                      ? theme.red
                      : (backend.poolInfo.alertLevel === 1 ? Style.warning : Style.borderSubtle)

        ColumnLayout {
            id: poolSettingsCol
            anchors.fill: parent
            anchors.margins: Style.cardPadding
            spacing: Style.space8

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space10

                Text {
                    Layout.fillWidth: true
                    text: qsTr("Incus storage pool")
                    color: theme.foreground
                    font.family: Style.fontSans
                    font.pixelSize: Style.font12
                    font.weight: Font.Medium
                }

                Text {
                    text: qsTr("%1 / %2 (%3%)")
                              .arg(backend.poolInfo.usedText || "0 MiB")
                              .arg(backend.poolInfo.totalText || "0 MiB")
                              .arg(backend.poolInfo.usedPercent !== undefined ? backend.poolInfo.usedPercent : 0)
                    color: backend.poolInfo.alertLevel === 2
                           ? theme.red
                           : (backend.poolInfo.alertLevel === 1 ? Style.warning : theme.muted)
                    font.family: Style.fontMono
                    font.pixelSize: Style.font10
                }
            }

            MeterBar {
                Layout.fillWidth: true
                implicitHeight: 4
                value: backend.poolInfo.usedPercent || 0
                trackColor: Style.surfaceTrack
                minFillWidth: Style.space2
                showAtZero: true
                animationMs: 0
                fillColor: backend.poolInfo.alertLevel === 2
                           ? theme.red
                           : (backend.poolInfo.alertLevel === 1 ? Style.warning : theme.accent)
            }

            Text {
                Layout.fillWidth: true
                elide: Text.ElideMiddle
                text: backend.poolInfo.resizable
                      ? qsTr("%1 · %2 · loop file size %3")
                            .arg(backend.poolInfo.name || "default")
                            .arg(backend.poolInfo.driver || "btrfs")
                            .arg(backend.poolInfo.sizeText || "—")
                      : qsTr("%1 · %2 · directory-backed (follows host free space)")
                            .arg(backend.poolInfo.name || "default")
                            .arg(backend.poolInfo.driver || "btrfs")
                color: theme.muted
                font.family: Style.fontMono
                font.pixelSize: Style.font10
            }

            Text {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                visible: (backend.poolInfo.allocatedBytes || 0) > 0
                text: qsTr("Allocated quotas: %1%2")
                          .arg(backend.poolInfo.allocatedText || "0 MiB")
                          .arg((backend.poolInfo.unboundedCount || 0) > 0
                               ? qsTr(" · %1 without quota").arg(backend.poolInfo.unboundedCount)
                               : "")
                color: (backend.poolInfo.overcommitted || false) ? Style.warning : theme.muted
                font.family: Style.fontSans
                font.pixelSize: Style.font10
            }

            Text {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: backend.poolInfo.resizable
                      ? qsTr("Grow the loop file in place, for example 60GiB (plain numbers mean GiB). Incus grows the Btrfs filesystem online; pools can never be shrunk.")
                      : qsTr("This pool is backed by a Btrfs subvolume on the host filesystem, so it grows automatically with the host and cannot be resized here.")
                color: theme.muted
                font.family: Style.fontSans
                font.pixelSize: Style.font10
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space10
                visible: backend.poolInfo.resizable || false

                FlatInput {
                    id: poolSizeField
                    Layout.fillWidth: true
                    placeholder: backend.poolInfo.sizeText || "60GiB"
                    onAccepted: {
                        app.poolResizeDraft = poolSizeField.text
                        app.preparePoolResize()
                    }
                }

                ActionButton {
                    label: qsTr("Grow pool")
                    minWidth: 118
                    disabled: backend.busy
                    onClicked: {
                        app.poolResizeDraft = poolSizeField.text
                        app.preparePoolResize()
                    }
                }
            }

            Text {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                visible: app.poolResizeError.length > 0
                text: app.poolResizeError
                color: theme.red
                font.family: Style.fontSans
                font.pixelSize: Style.font10
            }

            // Explicit confirmation before growing the loop file
            Rectangle {
                Layout.fillWidth: true
                visible: app.poolResizeConfirmVisible
                implicitHeight: poolConfirmCol.implicitHeight + Style.space20
                radius: Math.min(Style.radius4, theme.radius)
                color: Style.warningSurfaceDeep
                border.width: Style.hairline
                border.color: Style.warning

                ColumnLayout {
                    id: poolConfirmCol
                    anchors.fill: parent
                    anchors.margins: Style.space10
                    spacing: Style.space6

                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        text: qsTr("Grow pool '%1' from %2 to %3? The loop file is resized in place and this cannot be undone.")
                                  .arg(backend.poolInfo.name || "default")
                                  .arg(backend.poolInfo.totalText || "0 MiB")
                                  .arg(app.poolResizeDraft.trim())
                        color: Style.warning
                        font.family: Style.fontSans
                        font.pixelSize: Style.font11
                    }

                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        text: app.runningEnvironmentCount() > 0
                              ? qsTr("%1 environment(s) are running. Incus grows Btrfs pools online, but stop them first if you want maximum safety.")
                                    .arg(app.runningEnvironmentCount())
                              : qsTr("No environment is running.")
                        color: theme.muted
                        font.family: Style.fontSans
                        font.pixelSize: Style.font10
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Style.space10
                        Item { Layout.fillWidth: true }

                        ActionButton {
                            label: qsTr("Cancel")
                            minWidth: 84
                            onClicked: app.poolResizeConfirmVisible = false
                        }

                        ActionButton {
                            label: qsTr("Confirm growth")
                            variant: "primary"
                            minWidth: 148
                            disabled: backend.busy
                            onClicked: app.confirmPoolResize()
                        }
                    }
                }
            }
        }
    }
}
