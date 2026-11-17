import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts
import "../components"

Rectangle {
    property QtObject app: null
    id: settingsOverlay
    anchors.fill: parent
    visible: app.settingsVisible
    color: "#c406070a"
    z: 80

    // The window functions read and write the location and OS catalog fields
    // that live here, so they go through this small API instead of raw ids.
    readonly property string workspacePathText: workspacePathField.text
    readonly property string osStoragePathText: osStoragePathField.text
    readonly property string osFieldId: osIdField.text
    readonly property string osFieldAlias: osAliasField.text
    readonly property string osFieldName: osNameField.text
    readonly property string osFieldBadge: osBadgeField.text
    readonly property string osFieldDescription: osDescriptionField.text

    function setPaths(wsPath, osPath) {
        workspacePathField.text = wsPath
        osStoragePathField.text = osPath
    }

    function clearOsFields() {
        osIdField.text = ""
        osAliasField.text = ""
        osNameField.text = ""
        osBadgeField.text = ""
        osDescriptionField.text = ""
    }

    function setOsFields(idValue, aliasValue, nameValue, badgeValue, descriptionValue) {
        osIdField.text = idValue
        osAliasField.text = aliasValue
        osNameField.text = nameValue
        osBadgeField.text = badgeValue
        osDescriptionField.text = descriptionValue
    }

    onVisibleChanged: {
        if (!visible)
            return
        app.configNotice = ""
        app.osEditorVisible = false
        app.osEditorError = ""
        workspacePathField.text = backend.workspaceBase
        osStoragePathField.text = backend.osStoragePath
        app.syncCreateChoiceDrafts()
        app.autoSnapEnabledDraft = backend.autoSnapshotEnabled
        autoSnapKeepField.text = String(backend.autoSnapshotKeep)
        app.watchdogEnabledDraft = backend.watchdogEnabled
        app.watchdogAutoStopDraft = backend.watchdogAutoStop
        app.idleStopMinutesDraft = backend.idleStopMinutes
        app.staleDaysDraft = backend.staleDays
        app.recordSessionsDraft = backend.recordSessions
        watchdogWarnField.text = String(backend.watchdogDiskWarn)
        watchdogCritField.text = String(backend.watchdogDiskCritical)
        app.poolResizeDraft = ""
        app.poolResizeConfirmVisible = false
        app.poolResizeError = ""
        poolSizeField.text = ""
    }

    MouseArea {
        anchors.fill: parent
        onClicked: app.settingsVisible = false
    }

    Rectangle {
        anchors.centerIn: parent
        width: Math.min(parent.width - 56, 980)
        height: Math.min(parent.height - 56, 650)
        radius: Math.min(8, theme.radius)
        color: app.surfaceCard
        border.width: 1
        border.color: app.borderSubtle
        clip: true

        MouseArea { anchors.fill: parent }

        RowLayout {
            anchors.fill: parent
            spacing: 0

            // ------------------------------------------------------
            // CONFIGURATION SIDEBAR
            // ------------------------------------------------------
            Rectangle {
                Layout.preferredWidth: 232
                Layout.fillHeight: true
                color: "#0f1018"

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 6

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 10
                        Rectangle {
                            width: 30
                            height: 30
                            radius: Math.min(5, theme.radius)
                            color: app.surfaceSelected
                            border.width: 1
                            border.color: theme.accent
                            SlidersIcon {
                                app: settingsOverlay.app
                                anchors.centerIn: parent
                                width: 16
                                height: 16
                                color: theme.accent
                            }
                        }
                        ColumnLayout {
                            spacing: 1
                            Text {
                                text: qsTr("Configuration")
                                color: theme.foreground
                                font.family: app.sansFont
                                font.pixelSize: 14
                                font.weight: Font.Medium
                            }
                            Text {
                                text: qsTr("AIBox settings")
                                color: theme.muted
                                font.family: app.sansFont
                                font.pixelSize: 10
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.topMargin: 8
                        Layout.bottomMargin: 6
                        height: 1
                        color: app.borderSubtle
                    }

                    Repeater {
                        model: [
                            { title: qsTr("Locations"), detail: qsTr("Workspace & OS images"), step: "01" },
                            { title: qsTr("Storage Pool"), detail: qsTr("Incus capacity & growth"), step: "02" },
                            { title: qsTr("OS Catalog"), detail: qsTr("Add, edit, remove"), step: "03" },
                            { title: qsTr("Create Options"), detail: qsTr("Presets & quotas"), step: "04" },
                            { title: qsTr("Safety"), detail: qsTr("Auto-snapshots"), step: "05" }
                        ]
                        delegate: Rectangle {
                            required property var modelData
                            required property int index
                            Layout.fillWidth: true
                            height: 52
                            radius: Math.min(5, theme.radius)
                            color: app.settingsSection === index ? app.surfaceSelected : "transparent"
                            border.width: app.settingsSection === index ? 1 : 0
                            border.color: theme.accent

                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 10
                                anchors.rightMargin: 10
                                spacing: 10

                                Text {
                                    text: modelData.step
                                    color: app.settingsSection === index ? theme.accent : theme.muted
                                    font.family: app.monoFont
                                    font.pixelSize: 10
                                }

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 1
                                    Text {
                                        text: modelData.title
                                        color: app.settingsSection === index ? theme.foreground : theme.muted
                                        font.family: app.sansFont
                                        font.pixelSize: 12
                                        font.weight: app.settingsSection === index ? Font.Medium : Font.Normal
                                    }
                                    Text {
                                        text: modelData.detail
                                        color: theme.muted
                                        font.family: app.sansFont
                                        font.pixelSize: 10
                                    }
                                }
                            }

                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: app.settingsSection = index
                            }
                        }
                    }

                    Item { Layout.fillHeight: true }

                    Text {
                        Layout.fillWidth: true
                        text: qsTr("Config file")
                        color: theme.muted
                        font.family: app.sansFont
                        font.pixelSize: 10
                    }
                    Text {
                        Layout.fillWidth: true
                        text: backend.configFilePath
                        color: theme.muted
                        font.family: app.monoFont
                        font.pixelSize: 9
                        elide: Text.ElideMiddle
                    }
                }
            }

            Rectangle {
                width: 1
                Layout.fillHeight: true
                color: app.borderSubtle
            }

            // ------------------------------------------------------
            // CONFIGURATION CONTENT
            // ------------------------------------------------------
            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.margins: 24
                spacing: 12

                ModalHeader {
                    app: settingsOverlay.app
                    title: app.settingsSection === 0
                           ? qsTr("Storage Locations")
                           : (app.settingsSection === 1
                              ? qsTr("Incus Storage Pool")
                              : (app.settingsSection === 2
                                 ? qsTr("Linux OS Catalog")
                                 : (app.settingsSection === 3 ? qsTr("Create Environment Options") : qsTr("Safety & Auto-Snapshots"))))
                    subtitle: app.settingsSection === 0
                              ? qsTr("Choose where shared project files live and where offline OS image copies are saved.")
                              : (app.settingsSection === 1
                                 ? qsTr("Monitor pool usage and allocated quotas, and grow the loop file in place when space runs low.")
                                 : (app.settingsSection === 2
                                    ? qsTr("Add, edit, or remove the distributions shown in the environment creation dialog.")
                                    : (app.settingsSection === 3
                                       ? qsTr("Customize the preset cards, disk quota, RAM, and CPU choices shown in the creation dialog.")
                                       : qsTr("Configure automatic Btrfs snapshots before entering or starting an environment."))))
                    onClosed: app.settingsVisible = false
                }

                Rectangle {
                    visible: app.configNotice.length > 0
                    Layout.fillWidth: true
                    height: visible ? 34 : 0
                    radius: Math.min(4, theme.radius)
                    color: app.configNoticeError ? "#261319" : "#132019"
                    border.width: 1
                    border.color: app.configNoticeError ? theme.red : theme.green

                    Text {
                        anchors.fill: parent
                        anchors.leftMargin: 12
                        anchors.rightMargin: 12
                        verticalAlignment: Text.AlignVCenter
                        elide: Text.ElideRight
                        text: app.configNotice
                        color: app.configNoticeError ? theme.red : theme.green
                        font.family: app.sansFont
                        font.pixelSize: 12
                    }
                }

                ScrollView {
                    id: configScroll
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

                    ColumnLayout {
                        width: configScroll.availableWidth
                        spacing: 14

                        // ==================================================
                        // SECTION 0: LOCATIONS
                        // ==================================================
                        ColumnLayout {
                            Layout.fillWidth: true
                            visible: app.settingsSection === 0
                            spacing: 12

                            Rectangle {
                                Layout.fillWidth: true
                                implicitHeight: sysRootInfoCol.implicitHeight + 24
                                radius: Math.min(6, theme.radius)
                                color: app.surfaceInput
                                border.width: 1
                                border.color: app.borderSubtle

                                ColumnLayout {
                                    id: sysRootInfoCol
                                    anchors.fill: parent
                                    anchors.margins: 12
                                    spacing: 4

                                    Text {
                                        text: qsTr("System root location")
                                        color: theme.foreground
                                        font.family: app.sansFont
                                        font.pixelSize: 12
                                        font.weight: Font.Medium
                                    }
                                    Text {
                                        Layout.fillWidth: true
                                        text: qsTr("Root: %1  ·  Config: %2").arg(backend.systemPath).arg(backend.configFilePath)
                                        color: theme.muted
                                        font.family: app.monoFont
                                        font.pixelSize: 10
                                        elide: Text.ElideMiddle
                                    }
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                implicitHeight: wsLocCardCol.implicitHeight + 28
                                radius: Math.min(6, theme.radius)
                                color: app.surfaceInput
                                border.width: 1
                                border.color: app.borderSubtle

                                ColumnLayout {
                                    id: wsLocCardCol
                                    anchors.fill: parent
                                    anchors.margins: 14
                                    spacing: 8

                                    Text {
                                        text: qsTr("Workspace location")
                                        color: theme.foreground
                                        font.family: app.sansFont
                                        font.pixelSize: 12
                                        font.weight: Font.Medium
                                    }
                                    Text {
                                        Layout.fillWidth: true
                                        text: qsTr("New environments share this host folder at %1. Existing environments keep their current shared folder.").arg(backend.containerWorkspacePath)
                                        color: theme.muted
                                        font.family: app.sansFont
                                        font.pixelSize: 10
                                        wrapMode: Text.WordWrap
                                    }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10
                                        FlatInput {
                                            app: settingsOverlay.app
                                            id: workspacePathField
                                            Layout.fillWidth: true
                                            placeholder: backend.defaultWorkspaceBase
                                        }
                                        ActionButton {
                                            app: settingsOverlay.app
                                            label: qsTr("Browse...")
                                            minWidth: 94
                                            onClicked: workspaceFolderDialog.open()
                                        }
                                        ActionButton {
                                            app: settingsOverlay.app
                                            label: qsTr("Default")
                                            minWidth: 84
                                            onClicked: workspacePathField.text = backend.defaultWorkspaceBase
                                        }
                                    }
                                    Text {
                                        text: qsTr("Default: %1").arg(backend.defaultWorkspaceBase)
                                        color: theme.muted
                                        font.family: app.monoFont
                                        font.pixelSize: 9
                                        elide: Text.ElideMiddle
                                        Layout.fillWidth: true
                                    }
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                implicitHeight: osLocCardCol.implicitHeight + 28
                                radius: Math.min(6, theme.radius)
                                color: app.surfaceInput
                                border.width: 1
                                border.color: app.borderSubtle

                                ColumnLayout {
                                    id: osLocCardCol
                                    anchors.fill: parent
                                    anchors.margins: 14
                                    spacing: 8

                                    Text {
                                        text: qsTr("OS image storage")
                                        color: theme.foreground
                                        font.family: app.sansFont
                                        font.pixelSize: 12
                                        font.weight: Font.Medium
                                    }
                                    Text {
                                        Layout.fillWidth: true
                                        text: qsTr("Downloaded OS images are also exported here. If the local Incus image is gone, AIBox imports the copy from this folder instead of downloading again.")
                                        color: theme.muted
                                        font.family: app.sansFont
                                        font.pixelSize: 10
                                        wrapMode: Text.WordWrap
                                    }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10
                                        FlatInput {
                                            app: settingsOverlay.app
                                            id: osStoragePathField
                                            Layout.fillWidth: true
                                            placeholder: backend.defaultOsStoragePath
                                        }
                                        ActionButton {
                                            app: settingsOverlay.app
                                            label: qsTr("Browse...")
                                            minWidth: 94
                                            onClicked: osStorageFolderDialog.open()
                                        }
                                        ActionButton {
                                            app: settingsOverlay.app
                                            label: qsTr("Default")
                                            minWidth: 84
                                            onClicked: osStoragePathField.text = backend.defaultOsStoragePath
                                        }
                                    }
                                    Text {
                                        text: qsTr("Default: %1").arg(backend.defaultOsStoragePath)
                                        color: theme.muted
                                        font.family: app.monoFont
                                        font.pixelSize: 9
                                        elide: Text.ElideMiddle
                                        Layout.fillWidth: true
                                    }
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Layout.topMargin: 2
                                spacing: 10
                                Item { Layout.fillWidth: true }
                                ActionButton {
                                    app: settingsOverlay.app
                                    label: qsTr("Save Locations")
                                    variant: "primary"
                                    minWidth: 150
                                    onClicked: app.saveGeneralSettings()
                                }
                            }
                        }

                        // ==================================================
                        // SECTION 1: STORAGE POOL
                        // ==================================================
                        ColumnLayout {
                            Layout.fillWidth: true
                            visible: app.settingsSection === 1
                            spacing: 12

                            Rectangle {
                                Layout.fillWidth: true
                                visible: !(backend.poolInfo.valid || false)
                                implicitHeight: poolUnavailableCol.implicitHeight + 28
                                radius: Math.min(6, theme.radius)
                                color: app.surfaceInput
                                border.width: 1
                                border.color: app.borderSubtle

                                ColumnLayout {
                                    id: poolUnavailableCol
                                    anchors.fill: parent
                                    anchors.margins: 14
                                    spacing: 8

                                    Text {
                                        text: qsTr("Pool details unavailable")
                                        color: theme.foreground
                                        font.family: app.sansFont
                                        font.pixelSize: 12
                                        font.weight: Font.Medium
                                    }
                                    Text {
                                        Layout.fillWidth: true
                                        wrapMode: Text.WordWrap
                                        text: qsTr("AIBox could not read the Incus storage pool yet. Make sure Incus is initialized with the default pool, then refresh.")
                                        color: theme.muted
                                        font.family: app.sansFont
                                        font.pixelSize: 10
                                    }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10
                                        Item { Layout.fillWidth: true }
                                        ActionButton {
                                            app: settingsOverlay.app
                                            label: qsTr("Refresh")
                                            minWidth: 96
                                            disabled: backend.busy
                                            onClicked: backend.refresh()
                                        }
                                    }
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                visible: backend.poolInfo.valid || false
                                implicitHeight: poolSettingsCol.implicitHeight + 28
                                radius: Math.min(6, theme.radius)
                                color: app.surfaceInput
                                border.width: 1
                                border.color: (backend.poolInfo.alertLevel === 2)
                                              ? theme.red
                                              : ((backend.poolInfo.alertLevel === 1) ? "#e5a93c" : app.borderSubtle)

                                ColumnLayout {
                                    id: poolSettingsCol
                                    anchors.fill: parent
                                    anchors.margins: 14
                                    spacing: 8

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10
                                        Text {
                                            Layout.fillWidth: true
                                            text: qsTr("Incus storage pool")
                                            color: theme.foreground
                                            font.family: app.sansFont
                                            font.pixelSize: 12
                                            font.weight: Font.Medium
                                        }
                                        Text {
                                            text: qsTr("%1 / %2 (%3%)")
                                                      .arg(backend.poolInfo.usedText || "0 MiB")
                                                      .arg(backend.poolInfo.totalText || "0 MiB")
                                                      .arg(backend.poolInfo.usedPercent !== undefined ? backend.poolInfo.usedPercent : 0)
                                            color: (backend.poolInfo.alertLevel === 2)
                                                   ? theme.red
                                                   : ((backend.poolInfo.alertLevel === 1) ? "#e5a93c" : theme.muted)
                                            font.family: app.monoFont
                                            font.pixelSize: 10
                                        }
                                    }

                                    Rectangle {
                                        Layout.fillWidth: true
                                        height: 4
                                        radius: 2
                                        color: "#1a1d29"

                                        Rectangle {
                                            height: parent.height
                                            radius: 2
                                            width: Math.max(2, parent.width * (backend.poolInfo.usedPercent || 0) / 100)
                                            color: (backend.poolInfo.alertLevel === 2)
                                                   ? theme.red
                                                   : ((backend.poolInfo.alertLevel === 1) ? "#e5a93c" : theme.accent)
                                        }
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
                                        font.family: app.monoFont
                                        font.pixelSize: 10
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
                                        color: (backend.poolInfo.overcommitted || false) ? "#e5a93c" : theme.muted
                                        font.family: app.sansFont
                                        font.pixelSize: 10
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        wrapMode: Text.WordWrap
                                        text: backend.poolInfo.resizable
                                              ? qsTr("Grow the loop file in place, for example 60GiB (plain numbers mean GiB). Incus grows the Btrfs filesystem online; pools can never be shrunk.")
                                              : qsTr("This pool is backed by a Btrfs subvolume on the host filesystem, so it grows automatically with the host and cannot be resized here.")
                                        color: theme.muted
                                        font.family: app.sansFont
                                        font.pixelSize: 10
                                    }

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10
                                        visible: backend.poolInfo.resizable || false

                                        FlatInput {
                                            app: settingsOverlay.app
                                            id: poolSizeField
                                            Layout.fillWidth: true
                                            placeholder: backend.poolInfo.sizeText || "60GiB"
                                            onAccepted: {
                                                app.poolResizeDraft = poolSizeField.text
                                                app.preparePoolResize()
                                            }
                                        }
                                        ActionButton {
                                            app: settingsOverlay.app
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
                                        font.family: app.sansFont
                                        font.pixelSize: 10
                                    }

                                    Rectangle {
                                        Layout.fillWidth: true
                                        visible: app.poolResizeConfirmVisible
                                        implicitHeight: poolConfirmCol.implicitHeight + 20
                                        radius: Math.min(4, theme.radius)
                                        color: "#1a1409"
                                        border.width: 1
                                        border.color: "#e5a93c"

                                        ColumnLayout {
                                            id: poolConfirmCol
                                            anchors.fill: parent
                                            anchors.margins: 10
                                            spacing: 6

                                            Text {
                                                Layout.fillWidth: true
                                                wrapMode: Text.WordWrap
                                                text: qsTr("Grow pool '%1' from %2 to %3? The loop file is resized in place and this cannot be undone.")
                                                          .arg(backend.poolInfo.name || "default")
                                                          .arg(backend.poolInfo.totalText || "0 MiB")
                                                          .arg(app.poolResizeDraft.trim())
                                                color: "#e5a93c"
                                                font.family: app.sansFont
                                                font.pixelSize: 11
                                            }
                                            Text {
                                                Layout.fillWidth: true
                                                wrapMode: Text.WordWrap
                                                text: app.runningEnvironmentCount() > 0
                                                      ? qsTr("%1 environment(s) are running. Incus grows Btrfs pools online, but stop them first if you want maximum safety.")
                                                            .arg(app.runningEnvironmentCount())
                                                      : qsTr("No environment is running.")
                                                color: theme.muted
                                                font.family: app.sansFont
                                                font.pixelSize: 10
                                            }
                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 10
                                                Item { Layout.fillWidth: true }
                                                ActionButton {
                                                    app: settingsOverlay.app
                                                    label: qsTr("Cancel")
                                                    minWidth: 84
                                                    onClicked: app.poolResizeConfirmVisible = false
                                                }
                                                ActionButton {
                                                    app: settingsOverlay.app
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

                        // ==================================================
                        // SECTION 2: OS CATALOG
                        // ==================================================
                        ColumnLayout {
                            Layout.fillWidth: true
                            visible: app.settingsSection === 2
                            spacing: 10

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10
                                Text {
                                    Layout.fillWidth: true
                                    text: qsTr("%n OS entries", "", backend.osCatalog.length)
                                    color: theme.muted
                                    font.family: app.sansFont
                                    font.pixelSize: 11
                                }
                                ActionButton {
                                    app: settingsOverlay.app
                                    label: qsTr("+ Add OS")
                                    variant: "primary"
                                    minWidth: 105
                                    onClicked: app.beginAddOs()
                                }
                            }

                            // Inline editor for adding or editing an OS entry.
                            Rectangle {
                                visible: app.osEditorVisible
                                Layout.fillWidth: true
                                Layout.preferredHeight: visible ? osEditorColumn.implicitHeight + 28 : 0
                                radius: Math.min(6, theme.radius)
                                color: app.surfaceInput
                                border.width: 1
                                border.color: theme.accent

                                ColumnLayout {
                                    id: osEditorColumn
                                    anchors.fill: parent
                                    anchors.margins: 14
                                    spacing: 8

                                    Text {
                                        text: app.osEditorOriginalId.length > 0 ? qsTr("Edit OS entry") : qsTr("New OS entry")
                                        color: theme.foreground
                                        font.family: app.sansFont
                                        font.pixelSize: 13
                                        font.weight: Font.Medium
                                    }

                                    GridLayout {
                                        Layout.fillWidth: true
                                        columns: 2
                                        columnSpacing: 10
                                        rowSpacing: 8

                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            spacing: 4
                                            Text {
                                                text: qsTr("OS ID")
                                                color: theme.muted
                                                font.family: app.sansFont
                                                font.pixelSize: 10
                                            }
                                            FlatInput {
                                                app: settingsOverlay.app
                                                id: osIdField
                                                Layout.fillWidth: true
                                                placeholder: qsTr("e.g. ubuntu-24.04")
                                            }
                                        }

                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            spacing: 4
                                            Text {
                                                text: qsTr("Image alias")
                                                color: theme.muted
                                                font.family: app.sansFont
                                                font.pixelSize: 10
                                            }
                                            FlatInput {
                                                app: settingsOverlay.app
                                                id: osAliasField
                                                Layout.fillWidth: true
                                                placeholder: qsTr("e.g. ubuntu/24.04")
                                            }
                                        }

                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            spacing: 4
                                            Text {
                                                text: qsTr("Display name")
                                                color: theme.muted
                                                font.family: app.sansFont
                                                font.pixelSize: 10
                                            }
                                            FlatInput {
                                                app: settingsOverlay.app
                                                id: osNameField
                                                Layout.fillWidth: true
                                                placeholder: qsTr("e.g. Ubuntu 24.04 LTS")
                                            }
                                        }

                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            spacing: 4
                                            Text {
                                                text: qsTr("Badge")
                                                color: theme.muted
                                                font.family: app.sansFont
                                                font.pixelSize: 10
                                            }
                                            FlatInput {
                                                app: settingsOverlay.app
                                                id: osBadgeField
                                                Layout.fillWidth: true
                                                placeholder: qsTr("e.g. Recommended")
                                            }
                                        }

                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            Layout.columnSpan: 2
                                            spacing: 4
                                            Text {
                                                text: qsTr("Description")
                                                color: theme.muted
                                                font.family: app.sansFont
                                                font.pixelSize: 10
                                            }
                                            FlatInput {
                                                app: settingsOverlay.app
                                                id: osDescriptionField
                                                Layout.fillWidth: true
                                                placeholder: qsTr("Short description shown in the OS catalog")
                                            }
                                        }
                                    }

                                    Text {
                                        visible: app.osEditorError.length > 0
                                        Layout.fillWidth: true
                                        text: app.osEditorError
                                        color: theme.red
                                        font.family: app.sansFont
                                        font.pixelSize: 11
                                        wrapMode: Text.WordWrap
                                    }

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 8
                                        Item { Layout.fillWidth: true }
                                        ActionButton {
                                            app: settingsOverlay.app
                                            label: qsTr("Cancel")
                                            minWidth: 88
                                            onClicked: app.osEditorVisible = false
                                        }
                                        ActionButton {
                                            app: settingsOverlay.app
                                            label: qsTr("Save OS")
                                            variant: "primary"
                                            minWidth: 108
                                            onClicked: app.submitOsEditor()
                                        }
                                    }
                                }
                            }

                            Repeater {
                                model: backend.osCatalog
                                delegate: Rectangle {
                                    required property var modelData
                                    Layout.fillWidth: true
                                    height: 64
                                    radius: Math.min(5, theme.radius)
                                    color: app.surfaceInput
                                    border.width: 1
                                    border.color: modelData.downloaded ? "#284d3a" : app.borderSubtle

                                    RowLayout {
                                        anchors.fill: parent
                                        anchors.leftMargin: 12
                                        anchors.rightMargin: 12
                                        spacing: 10

                                        Rectangle {
                                            width: 30
                                            height: 30
                                            radius: 3
                                            color: modelData.downloaded ? "#162920" : app.surfaceCard
                                            border.width: 1
                                            border.color: modelData.downloaded ? "#284d3a" : app.borderSubtle
                                            Text {
                                                anchors.centerIn: parent
                                                text: modelData.downloaded ? "✓" : "↓"
                                                color: modelData.downloaded ? theme.green : theme.accent
                                                font.family: app.monoFont
                                                font.pixelSize: 12
                                            }
                                        }

                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            spacing: 1
                                            Text {
                                                Layout.fillWidth: true
                                                elide: Text.ElideRight
                                                text: modelData.name
                                                color: theme.foreground
                                                font.family: app.sansFont
                                                font.pixelSize: 12
                                                font.weight: Font.Medium
                                            }
                                            Text {
                                                Layout.fillWidth: true
                                                elide: Text.ElideRight
                                                text: modelData.id + "  ·  " + modelData.alias
                                                color: theme.muted
                                                font.family: app.monoFont
                                                font.pixelSize: 9
                                            }
                                        }

                                        Rectangle {
                                            height: 18
                                            width: osBadgeValue.implicitWidth + 12
                                            radius: 2
                                            color: app.surfaceCard
                                            border.width: 1
                                            border.color: app.borderSubtle
                                            Text {
                                                id: osBadgeValue
                                                anchors.centerIn: parent
                                                text: modelData.badge
                                                color: theme.accent
                                                font.family: app.monoFont
                                                font.pixelSize: 9
                                            }
                                        }

                                        Text {
                                            Layout.preferredWidth: 100
                                            horizontalAlignment: Text.AlignRight
                                            elide: Text.ElideRight
                                            text: modelData.sizeText
                                            color: modelData.downloaded ? theme.green : theme.muted
                                            font.family: app.monoFont
                                            font.pixelSize: 9
                                        }

                                        ActionButton {
                                            app: settingsOverlay.app
                                            implicitHeight: 26
                                            label: qsTr("Edit")
                                            minWidth: 58
                                            onClicked: app.beginEditOs(modelData)
                                        }
                                        ActionButton {
                                            app: settingsOverlay.app
                                            implicitHeight: 26
                                            label: qsTr("Remove")
                                            danger: true
                                            minWidth: 72
                                            onClicked: app.removeOsEntry(modelData)
                                        }
                                    }
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10
                                Text {
                                    Layout.fillWidth: true
                                    wrapMode: Text.WordWrap
                                    text: qsTr("Removing an entry only removes it from the catalog. Cached image files are kept until you remove them in OS Images & Downloads.")
                                    color: theme.muted
                                    font.family: app.sansFont
                                    font.pixelSize: 10
                                }
                                ActionButton {
                                    app: settingsOverlay.app
                                    label: qsTr("Restore Defaults")
                                    minWidth: 135
                                    onClicked: {
                                        const error = backend.resetOsCatalog()
                                        if (error.length > 0)
                                            app.showConfigNotice(error, true)
                                        else
                                            app.showConfigNotice(qsTr("Default OS catalog restored."), false)
                                    }
                                }
                            }
                        }

                        // ==================================================
                        // SECTION 3: CREATE OPTIONS
                        // ==================================================
                        ColumnLayout {
                            Layout.fillWidth: true
                            visible: app.settingsSection === 3
                            spacing: 12

                            Text {
                                text: qsTr("Capability presets")
                                color: theme.foreground
                                font.family: app.sansFont
                                font.pixelSize: 12
                                font.weight: Font.Medium
                            }
                            Text {
                                Layout.fillWidth: true
                                text: qsTr("These cards appear above the distribution list. Their behavior stays the same; edit the title and subtitle.")
                                color: theme.muted
                                font.family: app.sansFont
                                font.pixelSize: 10
                                wrapMode: Text.WordWrap
                            }

                            Repeater {
                                model: backend.presets
                                delegate: Rectangle {
                                    required property var modelData
                                    Layout.fillWidth: true
                                    height: 84
                                    radius: Math.min(6, theme.radius)
                                    color: app.surfaceInput
                                    border.width: 1
                                    border.color: app.borderSubtle

                                    RowLayout {
                                        anchors.fill: parent
                                        anchors.margins: 12
                                        spacing: 10

                                        Text {
                                            Layout.preferredWidth: 58
                                            text: modelData.id
                                            color: theme.accent
                                            font.family: app.monoFont
                                            font.pixelSize: 9
                                            elide: Text.ElideRight
                                        }

                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            spacing: 3
                                            Text {
                                                text: qsTr("Title")
                                                color: theme.muted
                                                font.family: app.sansFont
                                                font.pixelSize: 9
                                            }
                                            FlatInput {
                                                app: settingsOverlay.app
                                                id: presetTitleField
                                                Layout.fillWidth: true
                                                text: modelData.title
                                            }
                                        }

                                        ColumnLayout {
                                            Layout.fillWidth: true
                                            spacing: 3
                                            Text {
                                                text: qsTr("Subtitle")
                                                color: theme.muted
                                                font.family: app.sansFont
                                                font.pixelSize: 9
                                            }
                                            FlatInput {
                                                app: settingsOverlay.app
                                                id: presetSubField
                                                Layout.fillWidth: true
                                                text: modelData.sub
                                            }
                                        }

                                        ActionButton {
                                            app: settingsOverlay.app
                                            Layout.alignment: Qt.AlignBottom
                                            label: qsTr("Save")
                                            minWidth: 66
                                            onClicked: {
                                                const error = backend.savePreset(modelData.id, presetTitleField.text, presetSubField.text)
                                                if (error.length > 0)
                                                    app.showConfigNotice(error, true)
                                                else
                                                    app.showConfigNotice(qsTr("Preset saved."), false)
                                            }
                                        }
                                    }
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: quotaCardCol.implicitHeight + 28
                                radius: Math.min(6, theme.radius)
                                color: app.surfaceInput
                                border.width: 1
                                border.color: app.borderSubtle

                                ColumnLayout {
                                    id: quotaCardCol
                                    anchors.fill: parent
                                    anchors.margins: 14
                                    spacing: 6

                                    Text {
                                        text: qsTr("Quota and core choices")
                                        color: theme.foreground
                                        font.family: app.sansFont
                                        font.pixelSize: 12
                                        font.weight: Font.Medium
                                    }
                                    Text {
                                        Layout.fillWidth: true
                                        text: qsTr("Type a number and press Enter to add a choice (up to 4). Disk and RAM units are appended automatically; press × on a chip to remove it.")
                                        color: theme.muted
                                        font.family: app.sansFont
                                        font.pixelSize: 10
                                        wrapMode: Text.WordWrap
                                    }

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10
                                        Text {
                                            Layout.preferredWidth: 120
                                            Layout.alignment: Qt.AlignVCenter
                                            text: qsTr("Disk quota")
                                            color: theme.foreground
                                            font.family: app.sansFont
                                            font.pixelSize: 11
                                        }
                                        ChipInput {
                                            app: settingsOverlay.app
                                            Layout.fillWidth: true
                                            values: app.diskChoiceDraft
                                            unit: "GiB"
                                            maxChips: 4
                                            maxValue: 65536
                                            placeholder: qsTr("Type a size, then Enter")
                                            fullHint: qsTr("Limit 4 — remove one first")
                                            onAddRequested: function (value) {
                                                app.updateCreateChoices("disk", app.diskChoiceDraft.concat([value]))
                                            }
                                            onRemoveRequested: function (index) {
                                                const next = app.diskChoiceDraft.slice()
                                                next.splice(index, 1)
                                                app.updateCreateChoices("disk", next)
                                            }
                                        }
                                    }

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10
                                        Text {
                                            Layout.preferredWidth: 120
                                            Layout.alignment: Qt.AlignVCenter
                                            text: qsTr("RAM limit")
                                            color: theme.foreground
                                            font.family: app.sansFont
                                            font.pixelSize: 11
                                        }
                                        ChipInput {
                                            app: settingsOverlay.app
                                            Layout.fillWidth: true
                                            values: app.ramChoiceDraft
                                            unit: "GiB"
                                            maxChips: 4
                                            maxValue: 65536
                                            placeholder: qsTr("Type a size, then Enter")
                                            fullHint: qsTr("Limit 4 — remove one first")
                                            onAddRequested: function (value) {
                                                app.updateCreateChoices("ram", app.ramChoiceDraft.concat([value]))
                                            }
                                            onRemoveRequested: function (index) {
                                                const next = app.ramChoiceDraft.slice()
                                                next.splice(index, 1)
                                                app.updateCreateChoices("ram", next)
                                            }
                                        }
                                    }

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10
                                        Text {
                                            Layout.preferredWidth: 120
                                            Layout.alignment: Qt.AlignVCenter
                                            text: qsTr("CPU cores")
                                            color: theme.foreground
                                            font.family: app.sansFont
                                            font.pixelSize: 11
                                        }
                                        ChipInput {
                                            app: settingsOverlay.app
                                            Layout.fillWidth: true
                                            values: app.cpuChoiceDraft
                                            maxChips: 4
                                            maxValue: 1024
                                            placeholder: qsTr("Type cores, then Enter")
                                            fullHint: qsTr("Limit 4 — remove one first")
                                            onAddRequested: function (value) {
                                                app.updateCreateChoices("cpu", app.cpuChoiceDraft.concat([value]))
                                            }
                                            onRemoveRequested: function (index) {
                                                const next = app.cpuChoiceDraft.slice()
                                                next.splice(index, 1)
                                                app.updateCreateChoices("cpu", next)
                                            }
                                        }
                                    }

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10
                                        Item { Layout.fillWidth: true }
                                        ActionButton {
                                            app: settingsOverlay.app
                                            label: qsTr("Save Options")
                                            variant: "primary"
                                            minWidth: 130
                                            onClicked: app.saveQuotaOptions()
                                        }
                                    }
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10
                                Item { Layout.fillWidth: true }
                                ActionButton {
                                    app: settingsOverlay.app
                                    label: qsTr("Restore Create Defaults")
                                    minWidth: 185
                                    onClicked: {
                                        const error = backend.resetCreateOptions()
                                        if (error.length > 0) {
                                            app.showConfigNotice(error, true)
                                        } else {
                                            app.syncCreateChoiceDrafts()
                                            app.showConfigNotice(qsTr("Create options restored."), false)
                                        }
                                    }
                                }
                            }
                        }

                        // ==================================================
                        // SECTION 4: SAFETY & AUTO-SNAPSHOTS
                        // ==================================================
                        ColumnLayout {
                            Layout.fillWidth: true
                            visible: app.settingsSection === 4
                            spacing: 12

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: safetyCardCol.implicitHeight + 28
                                radius: Math.min(6, theme.radius)
                                color: app.surfaceInput
                                border.width: 1
                                border.color: app.borderSubtle

                                ColumnLayout {
                                    id: safetyCardCol
                                    anchors.fill: parent
                                    anchors.margins: 14
                                    spacing: 10

                                    Text {
                                        text: qsTr("Automatic Btrfs Snapshots")
                                        color: theme.foreground
                                        font.family: app.sansFont
                                        font.pixelSize: 12
                                        font.weight: Font.Medium
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        text: qsTr("Creates an auto-<timestamp> snapshot before Enter OS Terminal and Start OS (at most once every 5 minutes). Manual snapshots are never pruned.")
                                        color: theme.muted
                                        font.family: app.sansFont
                                        font.pixelSize: 10
                                        wrapMode: Text.WordWrap
                                    }

                                    Rectangle {
                                        Layout.fillWidth: true
                                        height: 38
                                        radius: Math.min(4, theme.radius)
                                        color: app.surfaceCard
                                        border.width: 1
                                        border.color: app.autoSnapEnabledDraft ? theme.accent : app.borderSubtle

                                        RowLayout {
                                            anchors.fill: parent
                                            anchors.leftMargin: 12
                                            anchors.rightMargin: 12
                                            spacing: 10

                                            Rectangle {
                                                width: 16
                                                height: 16
                                                radius: 3
                                                color: app.autoSnapEnabledDraft ? theme.accent : "transparent"
                                                border.width: 1
                                                border.color: app.autoSnapEnabledDraft ? theme.accent : theme.muted
                                                Text {
                                                    anchors.centerIn: parent
                                                    visible: app.autoSnapEnabledDraft
                                                    text: "✓"
                                                    color: theme.accentForeground
                                                    font.family: app.monoFont
                                                    font.pixelSize: 11
                                                }
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                text: qsTr("Enable auto-snapshots by default")
                                                color: theme.foreground
                                                font.family: app.sansFont
                                                font.pixelSize: 12
                                            }
                                        }

                                        MouseArea {
                                            anchors.fill: parent
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: app.autoSnapEnabledDraft = !app.autoSnapEnabledDraft
                                        }
                                    }

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10
                                        Text {
                                            Layout.preferredWidth: 180
                                            text: qsTr("Keep last auto-snapshots")
                                            color: theme.foreground
                                            font.family: app.sansFont
                                            font.pixelSize: 11
                                        }
                                        FlatInput {
                                            app: settingsOverlay.app
                                            id: autoSnapKeepField
                                            Layout.preferredWidth: 100
                                            placeholder: "5"
                                        }
                                        Text {
                                            Layout.fillWidth: true
                                            text: qsTr("1 to 50 snapshots per environment")
                                            color: theme.muted
                                            font.family: app.sansFont
                                            font.pixelSize: 10
                                        }
                                    }
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: watchdogCardCol.implicitHeight + 28
                                radius: Math.min(6, theme.radius)
                                color: app.surfaceInput
                                border.width: 1
                                border.color: app.borderSubtle

                                ColumnLayout {
                                    id: watchdogCardCol
                                    anchors.fill: parent
                                    anchors.margins: 14
                                    spacing: 10

                                    Text {
                                        text: qsTr("Disk & Resource Watchdog")
                                        color: theme.foreground
                                        font.family: app.sansFont
                                        font.pixelSize: 12
                                        font.weight: Font.Medium
                                    }

                                    Rectangle {
                                        Layout.fillWidth: true
                                        height: 36
                                        radius: Math.min(4, theme.radius)
                                        color: app.surfaceCard
                                        border.width: 1
                                        border.color: app.watchdogEnabledDraft ? theme.accent : app.borderSubtle

                                        RowLayout {
                                            anchors.fill: parent
                                            anchors.leftMargin: 12
                                            anchors.rightMargin: 12
                                            spacing: 10

                                            Rectangle {
                                                width: 16
                                                height: 16
                                                radius: 3
                                                color: app.watchdogEnabledDraft ? theme.accent : "transparent"
                                                border.width: 1
                                                border.color: app.watchdogEnabledDraft ? theme.accent : theme.muted
                                                Text {
                                                    anchors.centerIn: parent
                                                    visible: app.watchdogEnabledDraft
                                                    text: "✓"
                                                    color: theme.accentForeground
                                                    font.family: app.monoFont
                                                    font.pixelSize: 11
                                                }
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                text: qsTr("Send desktop alerts on high disk, CPU, or RAM")
                                                color: theme.foreground
                                                font.family: app.sansFont
                                                font.pixelSize: 12
                                            }
                                        }

                                        MouseArea {
                                            anchors.fill: parent
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: app.watchdogEnabledDraft = !app.watchdogEnabledDraft
                                        }
                                    }

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10
                                        Text {
                                            Layout.preferredWidth: 180
                                            text: qsTr("Disk warn / critical (%)")
                                            color: theme.foreground
                                            font.family: app.sansFont
                                            font.pixelSize: 11
                                        }
                                        FlatInput {
                                            app: settingsOverlay.app
                                            id: watchdogWarnField
                                            Layout.preferredWidth: 72
                                            placeholder: "90"
                                        }
                                        FlatInput {
                                            app: settingsOverlay.app
                                            id: watchdogCritField
                                            Layout.preferredWidth: 72
                                            placeholder: "97"
                                        }
                                        Text {
                                            Layout.fillWidth: true
                                            text: qsTr("50-98 warn, > warn critical (5% hysteresis)")
                                            color: theme.muted
                                            font.family: app.sansFont
                                            font.pixelSize: 10
                                        }
                                    }

                                    Rectangle {
                                        Layout.fillWidth: true
                                        height: 36
                                        radius: Math.min(4, theme.radius)
                                        color: app.surfaceCard
                                        border.width: 1
                                        border.color: app.watchdogAutoStopDraft ? theme.red : app.borderSubtle

                                        RowLayout {
                                            anchors.fill: parent
                                            anchors.leftMargin: 12
                                            anchors.rightMargin: 12
                                            spacing: 10

                                            Rectangle {
                                                width: 16
                                                height: 16
                                                radius: 3
                                                color: app.watchdogAutoStopDraft ? theme.red : "transparent"
                                                border.width: 1
                                                border.color: app.watchdogAutoStopDraft ? theme.red : theme.muted
                                                Text {
                                                    anchors.centerIn: parent
                                                    visible: app.watchdogAutoStopDraft
                                                    text: "✓"
                                                    color: theme.accentForeground
                                                    font.family: app.monoFont
                                                    font.pixelSize: 11
                                                }
                                            }

                                            Text {
                                                Layout.fillWidth: true
                                                text: qsTr("Auto-stop running containers by default on critical disk")
                                                color: theme.foreground
                                                font.family: app.sansFont
                                                font.pixelSize: 12
                                            }
                                        }

                                        MouseArea {
                                            anchors.fill: parent
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: app.watchdogAutoStopDraft = !app.watchdogAutoStopDraft
                                        }
                                    }
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: idleStopCardCol.implicitHeight + 28
                                radius: Math.min(6, theme.radius)
                                color: app.surfaceInput
                                border.width: 1
                                border.color: app.borderSubtle

                                ColumnLayout {
                                    id: idleStopCardCol
                                    anchors.fill: parent
                                    anchors.margins: 14
                                    spacing: 8

                                    Text {
                                        text: qsTr("Idle Auto-Stop (Opt-in)")
                                        color: theme.foreground
                                        font.family: app.sansFont
                                        font.pixelSize: 12
                                        font.weight: Font.Medium
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        wrapMode: Text.WordWrap
                                        text: qsTr("Automatically stops running non-template containers after sustained <3% CPU with no active incus exec session or port forwards (1 min warning notification beforehand). Works only while the AIBox GUI is running — no background daemon is used.")
                                        color: theme.muted
                                        font.family: app.sansFont
                                        font.pixelSize: 11
                                    }

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 8

                                        Repeater {
                                            model: backend.idleStopOptions
                                            delegate: Rectangle {
                                                required property int modelData
                                                readonly property bool selected: app.idleStopMinutesDraft === modelData
                                                height: 34
                                                width: idleChipTxt.implicitWidth + 20
                                                radius: Math.min(4, theme.radius)
                                                color: selected ? app.surfaceSelected : app.surfaceCard
                                                border.width: 1
                                                border.color: selected ? theme.accent : app.borderSubtle

                                                Text {
                                                    id: idleChipTxt
                                                    anchors.centerIn: parent
                                                    text: modelData === 0 ? qsTr("Off") : qsTr("%1 min").arg(modelData)
                                                    color: selected ? theme.accent : theme.muted
                                                    font.family: app.monoFont
                                                    font.pixelSize: 11
                                                }

                                                MouseArea {
                                                    anchors.fill: parent
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: app.idleStopMinutesDraft = modelData
                                                }
                                            }
                                        }
                                        Item { Layout.fillWidth: true }
                                    }
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: staleCardCol.implicitHeight + 28
                                radius: Math.min(6, theme.radius)
                                color: app.surfaceInput
                                border.width: 1
                                border.color: app.borderSubtle

                                ColumnLayout {
                                    id: staleCardCol
                                    anchors.fill: parent
                                    anchors.margins: 14
                                    spacing: 8

                                    RowLayout {
                                        Layout.fillWidth: true
                                        Text {
                                            text: qsTr("Stale Environment Report (U)")
                                            color: theme.foreground
                                            font.family: app.sansFont
                                            font.pixelSize: 12
                                            font.weight: Font.Medium
                                        }
                                        Item { Layout.fillWidth: true }
                                        ActionButton {
                                            app: settingsOverlay.app
                                            label: qsTr("Open Report")
                                            shortcutHint: "U"
                                            minWidth: 122
                                            onClicked: {
                                                app.settingsVisible = false
                                                app.openStaleReportDialog()
                                            }
                                        }
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        wrapMode: Text.WordWrap
                                        text: qsTr("Flags non-template environments unused for at least staleDays days (plus unknown-age environments without a usage timestamp) and scans orphaned workspaces, snapshot dirs, and unpaired backup archives.")
                                        color: theme.muted
                                        font.family: app.sansFont
                                        font.pixelSize: 11
                                    }

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 8

                                        Repeater {
                                            model: backend.staleDayOptions
                                            delegate: Rectangle {
                                                required property int modelData
                                                readonly property bool selected: app.staleDaysDraft === modelData
                                                height: 34
                                                width: staleDayTxt.implicitWidth + 20
                                                radius: Math.min(4, theme.radius)
                                                color: selected ? app.surfaceSelected : app.surfaceCard
                                                border.width: 1
                                                border.color: selected ? theme.accent : app.borderSubtle

                                                Text {
                                                    id: staleDayTxt
                                                    anchors.centerIn: parent
                                                    text: qsTr("%1 d").arg(modelData)
                                                    color: selected ? theme.accent : theme.muted
                                                    font.family: app.monoFont
                                                    font.pixelSize: 11
                                                }

                                                MouseArea {
                                                    anchors.fill: parent
                                                    cursorShape: Qt.PointingHandCursor
                                                    onClicked: app.staleDaysDraft = modelData
                                                }
                                            }
                                        }
                                        Item { Layout.fillWidth: true }
                                    }
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: recCardCol.implicitHeight + 28
                                radius: Math.min(6, theme.radius)
                                color: app.surfaceInput
                                border.width: 1
                                border.color: app.borderSubtle

                                ColumnLayout {
                                    id: recCardCol
                                    anchors.fill: parent
                                    anchors.margins: 14
                                    spacing: 8

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 10
                                        Text {
                                            text: qsTr("Session Recording (Opt-in)")
                                            color: theme.foreground
                                            font.family: app.sansFont
                                            font.pixelSize: 12
                                            font.weight: Font.Medium
                                        }
                                        Item { Layout.fillWidth: true }
                                        Rectangle {
                                            height: 28
                                            width: recToggleTxt.implicitWidth + 22
                                            radius: Math.min(4, theme.radius)
                                            color: app.recordSessionsDraft ? app.surfaceSelected : app.surfaceCard
                                            border.width: 1
                                            border.color: app.recordSessionsDraft ? theme.accent : app.borderSubtle

                                            Text {
                                                id: recToggleTxt
                                                anchors.centerIn: parent
                                                text: app.recordSessionsDraft ? qsTr("Enabled") : qsTr("Disabled")
                                                color: app.recordSessionsDraft ? theme.accent : theme.muted
                                                font.family: app.monoFont
                                                font.pixelSize: 11
                                            }

                                            MouseArea {
                                                anchors.fill: parent
                                                cursorShape: Qt.PointingHandCursor
                                                onClicked: app.recordSessionsDraft = !app.recordSessionsDraft
                                            }
                                        }
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        wrapMode: Text.WordWrap
                                        text: qsTr("Records interactive Enter shells using script -q -f with timing files, automatically scrubs S3 secrets after exit, and retains the last 20 sessions per environment (200 MB total cap). View or replay sessions under History (H) -> Sessions.")
                                        color: theme.muted
                                        font.family: app.sansFont
                                        font.pixelSize: 11
                                    }
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10
                                Item { Layout.fillWidth: true }
                                ActionButton {
                                    app: settingsOverlay.app
                                    label: qsTr("Save Safety Settings")
                                    variant: "primary"
                                    minWidth: 165
                                    onClicked: {
                                        const parsedKeep = parseInt(autoSnapKeepField.text.trim(), 10)
                                        const keep = isNaN(parsedKeep) ? 0 : parsedKeep
                                        const snapErr = backend.saveAutoSnapshotSettings(app.autoSnapEnabledDraft, keep)
                                        if (snapErr.length > 0) {
                                            app.showConfigNotice(snapErr, true)
                                            return
                                        }
                                        const parsedWarn = parseInt(watchdogWarnField.text.trim(), 10)
                                        const parsedCrit = parseInt(watchdogCritField.text.trim(), 10)
                                        const wdErr = backend.saveWatchdogSettings(
                                            app.watchdogEnabledDraft,
                                            isNaN(parsedWarn) ? 0 : parsedWarn,
                                            isNaN(parsedCrit) ? 0 : parsedCrit,
                                            app.watchdogAutoStopDraft)
                                        if (wdErr.length > 0) {
                                            app.showConfigNotice(wdErr, true)
                                            return
                                        }
                                        const idleErr = backend.saveIdleStopSettings(app.idleStopMinutesDraft)
                                        if (idleErr.length > 0) {
                                            app.showConfigNotice(idleErr, true)
                                            return
                                        }
                                        const staleErr = backend.saveStaleSettings(app.staleDaysDraft)
                                        if (staleErr.length > 0) {
                                            app.showConfigNotice(staleErr, true)
                                            return
                                        }
                                        const recErr = backend.saveSessionRecordingSettings(app.recordSessionsDraft)
                                        if (recErr.length > 0) {
                                            app.showConfigNotice(recErr, true)
                                            return
                                        }
                                        autoSnapKeepField.text = String(backend.autoSnapshotKeep)
                                        watchdogWarnField.text = String(backend.watchdogDiskWarn)
                                        watchdogCritField.text = String(backend.watchdogDiskCritical)
                                        app.idleStopMinutesDraft = backend.idleStopMinutes
                                        app.staleDaysDraft = backend.staleDays
                                        app.recordSessionsDraft = backend.recordSessions
                                        app.showConfigNotice(qsTr("Safety settings saved."), false)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    FolderDialog {
        id: workspaceFolderDialog
        title: qsTr("Choose Workspace Location")
        currentFolder: backend.pathToUrl(backend.workspaceBase)
        onAccepted: workspacePathField.text = backend.localPath(selectedFolder)
    }

    FolderDialog {
        id: osStorageFolderDialog
        title: qsTr("Choose OS Image Storage Location")
        currentFolder: backend.pathToUrl(backend.osStoragePath)
        onAccepted: osStoragePathField.text = backend.localPath(selectedFolder)
    }
}
