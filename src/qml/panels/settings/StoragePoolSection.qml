import QtQuick
import QtQuick.Layouts
import "../../components"

// Settings section 1: Incus storage pool usage and growth.
ColumnLayout {
    id: root
    property QtObject app: null
    readonly property var pool: backend.poolInfo

    Layout.fillWidth: true
    spacing: Style.gap

    function reload() {
        app.poolResizeDraft = ""
        app.poolResizeConfirmVisible = false
        app.poolResizeError = ""
        poolSizeField.text = ""
    }

    // Pool details unavailable
    Card {
        visible: !(root.pool.valid || false)
        title: qsTr("Pool details unavailable")
        contentSpacing: Style.gapMd

        Text {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: qsTr("AIBox could not read the Incus storage pool yet. Make sure Incus is initialized with the default pool, then refresh.")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.gap
            Item { Layout.fillWidth: true }

            ActionButton {
                label: qsTr("Refresh")
                disabled: backend.busy
                onClicked: backend.refresh()
            }
        }
    }

    // Live pool summary card
    Card {
        visible: root.pool.valid || false
        title: qsTr("Incus storage pool")
        contentSpacing: Style.gapMd
        borderColor: root.pool.alertLevel === 2
                     ? theme.dangerBorder
                     : (root.pool.alertLevel === 1 ? theme.warningBorder : theme.borderSubtle)

        actions: [
            Text {
                text: qsTr("%1 / %2 (%3%)")
                          .arg(root.pool.usedText || "0 MiB")
                          .arg(root.pool.totalText || "0 MiB")
                          .arg(root.pool.usedPercent !== undefined ? root.pool.usedPercent : 0)
                color: root.pool.alertLevel === 2
                       ? theme.dangerText
                       : (root.pool.alertLevel === 1 ? theme.warningText : theme.textMuted)
                font.family: Style.fontMono
                font.pixelSize: Style.fontCaption
            }
        ]

        MeterBar {
            Layout.fillWidth: true
            value: root.pool.usedPercent || 0
            trackColor: theme.surfaceTrack
            minFillWidth: Style.gapXs
            showAtZero: true
            animationMs: 0
            fillColor: root.pool.alertLevel === 2
                       ? theme.danger
                       : (root.pool.alertLevel === 1 ? theme.warning : theme.accent)
        }

        Text {
            Layout.fillWidth: true
            elide: Text.ElideMiddle
            text: root.pool.resizable
                  ? qsTr("%1 · %2 · loop file size %3")
                        .arg(root.pool.name || "default")
                        .arg(root.pool.driver || "btrfs")
                        .arg(root.pool.sizeText || "—")
                  : qsTr("%1 · %2 · directory-backed (follows host free space)")
                        .arg(root.pool.name || "default")
                        .arg(root.pool.driver || "btrfs")
            color: theme.textMuted
            font.family: Style.fontMono
            font.pixelSize: Style.fontCaption
        }

        Text {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            visible: (root.pool.allocatedBytes || 0) > 0
            text: qsTr("Allocated quotas: %1%2")
                      .arg(root.pool.allocatedText || "0 MiB")
                      .arg((root.pool.unboundedCount || 0) > 0
                           ? qsTr(" · %1 without quota").arg(root.pool.unboundedCount)
                           : "")
            color: (root.pool.overcommitted || false) ? theme.warningText : theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }
    }

    // Pool resize card
    Card {
        visible: root.pool.valid || false
        title: qsTr("Grow storage pool")
        contentSpacing: Style.gapMd

        Text {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: root.pool.resizable
                  ? qsTr("Grow the loop file in place, for example 60GiB (plain numbers mean GiB). Incus grows the Btrfs filesystem online; pools can never be shrunk.")
                  : qsTr("This pool is backed by a Btrfs subvolume on the host filesystem, so it grows automatically with the host and cannot be resized here.")
            color: theme.textMuted
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }

        FormRow {
            visible: root.pool.resizable || false
            label: qsTr("Target size")
            labelWidth: Style.buttonMinWidth + Style.padModal

            FlatInput {
                id: poolSizeField
                Layout.fillWidth: true
                mono: true
                placeholder: root.pool.sizeText || "60GiB"
                onAccepted: {
                    app.poolResizeDraft = poolSizeField.text
                    app.preparePoolResize()
                }
            }

            ActionButton {
                label: qsTr("Grow pool")
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
            color: theme.dangerText
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }

        // Explicit confirmation before growing the loop file (Esc dismisses)
        Rectangle {
            Layout.fillWidth: true
            visible: app.poolResizeConfirmVisible
            implicitHeight: poolConfirmCol.implicitHeight + Style.padCard * 2
            radius: Math.min(Style.radiusMd, theme.radius)
            color: theme.warningSurface
            border.width: Style.hairline
            border.color: theme.warningBorder

            ColumnLayout {
                id: poolConfirmCol
                anchors.fill: parent
                anchors.margins: Style.padCard
                spacing: Style.gapMd

                Text {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    text: qsTr("Grow pool '%1' from %2 to %3? The loop file is resized in place and this cannot be undone.")
                              .arg(root.pool.name || "default")
                              .arg(root.pool.totalText || "0 MiB")
                              .arg(app.poolResizeDraft.trim())
                    color: theme.warningText
                    font.family: Style.fontSans
                    font.pixelSize: Style.fontSmall
                }

                Text {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    text: {
                        if (!app.poolResizeConfirmVisible)
                            return ""
                        const running = app.runningEnvironmentCount()
                        return running > 0
                               ? qsTr("%1 environment(s) are running. Incus grows Btrfs pools online, but stop them first if you want maximum safety.").arg(running)
                               : qsTr("No environment is running.")
                    }
                    color: theme.textMuted
                    font.family: Style.fontSans
                    font.pixelSize: Style.fontCaption
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Style.gap

                    Text {
                        Layout.fillWidth: true
                        text: qsTr("Press Esc to cancel")
                        color: theme.textMuted
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontCaption
                    }

                    ActionButton {
                        label: qsTr("Confirm growth")
                        variant: "primary"
                        disabled: backend.busy
                        onClicked: app.confirmPoolResize()
                    }
                }
            }
        }
    }
}
