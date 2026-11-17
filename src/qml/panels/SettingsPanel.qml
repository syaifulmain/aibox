import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import "../components"
import "settings"

ModalDialog {
    id: settingsOverlay
    property QtObject app: null
    open: app.settingsVisible
    zOrder: 80
    padding: 0
    contentSpacing: 0
    borderColor: Style.borderSubtle
    clipCard: true
    cardWidth: Math.min(parent.width - Style.modalGutter, 980)
    cardHeight: Math.min(parent.height - Style.modalEdge, 650)
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
    }

    RowLayout {
        Layout.fillWidth: true
        Layout.fillHeight: true
        spacing: 0

        // ------------------------------------------------------
        // CONFIGURATION SIDEBAR
        // ------------------------------------------------------
        Rectangle {
            Layout.preferredWidth: 232
            Layout.fillHeight: true
            color: Style.surfaceSidebar

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: Style.space16
                spacing: Style.space6

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Style.space10

                    Rectangle {
                        width: 30
                        height: 30
                        radius: Math.min(Style.radius5, theme.radius)
                        color: Style.surfaceSelected
                        border.width: Style.hairline
                        border.color: theme.accent

                        SlidersIcon {
                            anchors.centerIn: parent
                            width: 16
                            height: 16
                            color: theme.accent
                        }
                    }

                    ColumnLayout {
                        spacing: Style.space1

                        Text {
                            text: qsTr("Configuration")
                            color: theme.foreground
                            font.family: Style.fontSans
                            font.pixelSize: Style.font14
                            font.weight: Font.Medium
                        }
                        Text {
                            text: qsTr("AIBox settings")
                            color: theme.muted
                            font.family: Style.fontSans
                            font.pixelSize: Style.font10
                        }
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.topMargin: Style.space8
                    Layout.bottomMargin: Style.space6
                    height: Style.hairline
                    color: Style.borderSubtle
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
                        radius: Math.min(Style.radius5, theme.radius)
                        color: app.settingsSection === index ? Style.surfaceSelected : "transparent"
                        border.width: app.settingsSection === index ? Style.hairline : 0
                        border.color: theme.accent

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: Style.space10
                            anchors.rightMargin: Style.space10
                            spacing: Style.space10

                            Text {
                                text: modelData.step
                                color: app.settingsSection === index ? theme.accent : theme.muted
                                font.family: Style.fontMono
                                font.pixelSize: Style.font10
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: Style.space1

                                Text {
                                    text: modelData.title
                                    color: app.settingsSection === index ? theme.foreground : theme.muted
                                    font.family: Style.fontSans
                                    font.pixelSize: Style.font12
                                    font.weight: app.settingsSection === index ? Font.Medium : Font.Normal
                                }
                                Text {
                                    text: modelData.detail
                                    color: theme.muted
                                    font.family: Style.fontSans
                                    font.pixelSize: Style.font10
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
                    font.family: Style.fontSans
                    font.pixelSize: Style.font10
                }
                Text {
                    Layout.fillWidth: true
                    text: backend.configFilePath
                    color: theme.muted
                    font.family: Style.fontMono
                    font.pixelSize: Style.font9
                    elide: Text.ElideMiddle
                }
            }
        }

        Rectangle {
            width: Style.hairline
            Layout.fillHeight: true
            color: Style.borderSubtle
        }

        // ------------------------------------------------------
        // CONFIGURATION CONTENT
        // ------------------------------------------------------
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.margins: Style.space24
            spacing: Style.space12

            ModalHeader {
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

            // Save / error notice
            Rectangle {
                visible: app.configNotice.length > 0
                Layout.fillWidth: true
                height: visible ? Style.controlHeight : 0
                radius: Math.min(Style.radius4, theme.radius)
                color: app.configNoticeError ? Style.dangerSurface : Style.successSurfaceTint
                border.width: Style.hairline
                border.color: app.configNoticeError ? theme.red : theme.green

                Text {
                    anchors.fill: parent
                    anchors.leftMargin: Style.space12
                    anchors.rightMargin: Style.space12
                    verticalAlignment: Text.AlignVCenter
                    elide: Text.ElideRight
                    text: app.configNotice
                    color: app.configNoticeError ? theme.red : theme.green
                    font.family: Style.fontSans
                    font.pixelSize: Style.font12
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
                    spacing: Style.space14

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
                }
            }
        }
    }
}
