import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import "../components"
import "settings"

ModalDialog {
    id: settingsOverlay
    property QtObject app: null
    open: app.settingsVisible
    size: "lg"
    zOrder: 80
    padding: 0
    contentSpacing: 0
    borderColor: theme.borderSubtle
    clipCard: true
    cardHeight: Math.min(parent.height - Style.modalGutter, 650)
    onDismissed: app.settingsVisible = false

    // The window functions read and write the location and OS catalog fields,
    // so they go through this small API instead of raw ids.
    readonly property string workspacePathText: locationsSection.workspacePathText
    readonly property string osStoragePathText: locationsSection.osStoragePathText
    readonly property string osFieldId: osCatalogSection.osFieldId
    readonly property string osFieldAlias: osCatalogSection.osFieldAlias
    readonly property string osFieldName: osCatalogSection.osFieldName
    readonly property string osFieldBadge: osCatalogSection.osFieldBadge
    readonly property string osFieldDescription: osCatalogSection.osFieldDescription

    function setPaths(wsPath, osPath) {
        locationsSection.setPaths(wsPath, osPath)
    }

    function clearOsFields() {
        osCatalogSection.clearOsFields()
    }

    function setOsFields(idValue, aliasValue, nameValue, badgeValue, descriptionValue) {
        osCatalogSection.setOsFields(idValue, aliasValue, nameValue, badgeValue, descriptionValue)
    }

    onVisibleChanged: {
        if (!visible)
            return
        app.configNotice = ""
        locationsSection.reload()
        poolSection.reload()
        osCatalogSection.reload()
        createOptionsSection.reload()
        safetySection.reload()
        appearanceSection.reload()
    }

    RowLayout {
        Layout.fillWidth: true
        Layout.fillHeight: true
        spacing: 0

        // ------------------------------------------------------
        // CONFIGURATION SIDEBAR
        // ------------------------------------------------------
        Rectangle {
            id: sidebar
            Layout.preferredWidth: 232
            Layout.fillHeight: true
            color: theme.surfaceInput
            activeFocusOnTab: true

            Keys.onUpPressed: function(event) {
                app.settingsSection = Math.max(0, app.settingsSection - 1)
                event.accepted = true
            }
            Keys.onDownPressed: function(event) {
                app.settingsSection = Math.min(5, app.settingsSection + 1)
                event.accepted = true
            }

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: Style.padSection
                spacing: Style.gapSm

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Style.gap

                    Rectangle {
                        width: Style.controlHeight
                        height: Style.controlHeight
                        radius: Math.min(Style.radiusMd, theme.radius)
                        color: theme.accentSurface
                        border.width: Style.hairline
                        border.color: theme.accentBorder

                        SlidersIcon {
                            anchors.centerIn: parent
                            color: theme.accentText
                        }
                    }

                    Text {
                        text: qsTr("Settings")
                        color: theme.textPrimary
                        font.family: Style.fontSans
                        font.pixelSize: Style.fontTitle
                        font.weight: Font.Medium
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.topMargin: Style.gapMd
                    Layout.bottomMargin: Style.gapSm
                    height: Style.hairline
                    color: theme.borderSubtle
                }

                Repeater {
                    model: [
                        { group: qsTr("System"), title: qsTr("Locations"), detail: qsTr("Workspace & OS images") },
                        { group: "", title: qsTr("Storage Pool"), detail: qsTr("Incus capacity & growth") },
                        { group: qsTr("Environments"), title: qsTr("OS Catalog"), detail: qsTr("Add, edit, remove") },
                        { group: "", title: qsTr("Create Options"), detail: qsTr("Presets & quotas") },
                        { group: "", title: qsTr("Safety"), detail: qsTr("Auto-snapshots") },
                        { group: qsTr("Interface"), title: qsTr("Appearance"), detail: qsTr("Theme mode & contrast") }
                    ]

                    delegate: ColumnLayout {
                        required property var modelData
                        required property int index
                        readonly property bool active: app.settingsSection === index

                        Layout.fillWidth: true
                        spacing: Style.gapSm

                        Text {
                            visible: modelData.group.length > 0
                            Layout.fillWidth: true
                            Layout.topMargin: index === 0 ? Style.gapXs : Style.gapMd
                            Layout.leftMargin: Style.gap
                            text: modelData.group.toUpperCase()
                            color: theme.textMuted
                            font.family: Style.fontSans
                            font.pixelSize: Style.fontCaption
                            font.weight: Font.Medium
                            font.letterSpacing: 0.8
                        }

                        Rectangle {
                            id: navItem
                            Layout.fillWidth: true
                            height: Style.controlHeight + Style.padSection
                            radius: Math.min(Style.radiusMd, theme.radius)
                            color: active ? theme.surfaceSelected
                                 : (navMouse.containsMouse ? theme.surfaceRaised : "transparent")

                            Behavior on color {
                                enabled: !theme.reduceMotion
                                ColorAnimation { duration: 120 }
                            }

                            Rectangle {
                                visible: active
                                anchors.left: parent.left
                                anchors.leftMargin: Style.gapSm
                                anchors.verticalCenter: parent.verticalCenter
                                width: Style.radiusSm
                                height: Style.badgeHeight
                                radius: Math.min(Style.radiusSm, theme.radius)
                                color: theme.accent
                            }

                            ColumnLayout {
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.verticalCenter: parent.verticalCenter
                                anchors.leftMargin: Style.padSection
                                anchors.rightMargin: Style.padCard
                                spacing: Style.gapXs

                                Text {
                                    text: modelData.title
                                    color: active ? theme.accentText
                                         : (navMouse.containsMouse ? theme.textPrimary : theme.textSecondary)
                                    font.family: Style.fontSans
                                    font.pixelSize: Style.fontBody
                                    font.weight: active ? Font.Medium : Font.Normal
                                }
                                Text {
                                    text: modelData.detail
                                    color: theme.textMuted
                                    font.family: Style.fontSans
                                    font.pixelSize: Style.fontCaption
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                            }

                            MouseArea {
                                id: navMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    sidebar.forceActiveFocus()
                                    app.settingsSection = index
                                }
                            }
                        }
                    }
                }

                Item { Layout.fillHeight: true }

                Rectangle {
                    Layout.fillWidth: true
                    height: Style.hairline
                    color: theme.borderSubtle
                }

                RowLayout {
                    Layout.fillWidth: true
                    Layout.topMargin: Style.gapSm
                    spacing: Style.gapMd

                    Text {
                        text: theme.isDark ? qsTr("Dark mode") : qsTr("Light mode")
                        color: theme.textMuted
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontCaption
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }

                    Text {
                        text: "↑/↓ · Esc"
                        color: theme.textMuted
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontCaption
                    }
                }
            }
        }

        Rectangle {
            width: Style.hairline
            Layout.fillHeight: true
            color: theme.borderSubtle
        }

        // ------------------------------------------------------
        // CONFIGURATION CONTENT
        // ------------------------------------------------------
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.margins: Style.padModal
            spacing: Style.padCard

            ModalHeader {
                title: app.settingsSection === 0
                       ? qsTr("Storage Locations")
                       : (app.settingsSection === 1
                          ? qsTr("Incus Storage Pool")
                          : (app.settingsSection === 2
                             ? qsTr("Linux OS Catalog")
                             : (app.settingsSection === 3
                                ? qsTr("Create Environment Options")
                                : (app.settingsSection === 4
                                   ? qsTr("Safety & Auto-Snapshots")
                                   : qsTr("Appearance & Accessibility")))))
                subtitle: app.settingsSection === 0
                          ? qsTr("Choose where shared project files live and where offline OS image copies are saved.")
                          : (app.settingsSection === 1
                             ? qsTr("Monitor pool usage and allocated quotas, and grow the loop file in place when space runs low.")
                             : (app.settingsSection === 2
                                ? qsTr("Add, edit, or remove the distributions shown in the environment creation dialog.")
                                : (app.settingsSection === 3
                                   ? qsTr("Customize the preset cards, disk quota, RAM, and CPU choices shown in the creation dialog.")
                                   : (app.settingsSection === 4
                                      ? qsTr("Configure automatic Btrfs snapshots before entering or starting an environment.")
                                      : qsTr("Override light/dark polarity, enable high contrast, or reduce theme transition motion.")))))
                onClosed: app.settingsVisible = false
            }

            // Save / error notice
            Rectangle {
                visible: app.configNotice.length > 0
                Layout.fillWidth: true
                height: visible ? Style.controlHeight : 0
                radius: Math.min(Style.radiusMd, theme.radius)
                color: app.configNoticeError ? theme.dangerSurface : theme.successSurface
                border.width: Style.hairline
                border.color: app.configNoticeError ? theme.dangerBorder : theme.successBorder

                Text {
                    anchors.fill: parent
                    anchors.leftMargin: Style.padCard
                    anchors.rightMargin: Style.padCard
                    verticalAlignment: Text.AlignVCenter
                    elide: Text.ElideRight
                    text: app.configNotice
                    color: app.configNoticeError ? theme.dangerText : theme.successText
                    font.family: Style.fontSans
                    font.pixelSize: Style.fontBody
                }
            }

            ScrollView {
                id: configScroll
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                ScrollBar.vertical.policy: ScrollBar.AlwaysOff

                ColumnLayout {
                    width: configScroll.availableWidth
                    spacing: Style.padCard

                    LocationsSection {
                        id: locationsSection
                        app: settingsOverlay.app
                        visible: app.settingsSection === 0
                    }

                    StoragePoolSection {
                        id: poolSection
                        app: settingsOverlay.app
                        visible: app.settingsSection === 1
                    }

                    OsCatalogSection {
                        id: osCatalogSection
                        app: settingsOverlay.app
                        visible: app.settingsSection === 2
                    }

                    CreateOptionsSection {
                        id: createOptionsSection
                        app: settingsOverlay.app
                        visible: app.settingsSection === 3
                    }

                    SafetySection {
                        id: safetySection
                        app: settingsOverlay.app
                        visible: app.settingsSection === 4
                    }

                    AppearanceSection {
                        id: appearanceSection
                        app: settingsOverlay.app
                        visible: app.settingsSection === 5
                    }
                }
            }
        }
    }
}
