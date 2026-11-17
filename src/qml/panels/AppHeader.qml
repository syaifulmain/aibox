import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts
import "../components"

Rectangle {
    property QtObject app: null
    id: appHeaderRoot
    Layout.fillWidth: true
    Layout.preferredHeight: 54
    color: theme.darkBackground

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 20
        anchors.rightMargin: 20
        spacing: 16

        // Minimalist App Brand
        RowLayout {
            spacing: 10
            Rectangle {
                width: 28
                height: 28
                radius: Math.min(4, theme.radius)
                color: app.surfaceSelected
                border.width: 1
                border.color: theme.accent
                Text {
                    anchors.centerIn: parent
                    text: ">_"
                    color: theme.accent
                    font.family: app.monoFont
                    font.pixelSize: 12
                }
            }
            Text {
                text: qsTr("AIBox")
                color: theme.foreground
                font.family: app.sansFont
                font.pixelSize: 15
                font.weight: Font.Medium
            }
            Text {
                text: "·"
                color: theme.muted
                font.pixelSize: 14
            }
            Text {
                text: qsTr("Isolated Linux OS Manager")
                color: theme.muted
                font.family: app.sansFont
                font.pixelSize: 12
            }
        }

        // Segmented Tab Switcher
        Rectangle {
            Layout.leftMargin: 12
            Layout.preferredHeight: 34
            Layout.preferredWidth: segRow.implicitWidth + 8
            Layout.alignment: Qt.AlignVCenter
            radius: Math.min(4, theme.radius)
            color: app.surfaceInput
            border.width: 1
            border.color: app.borderSubtle

            RowLayout {
                id: segRow
                anchors.centerIn: parent
                spacing: 4

                Repeater {
                    model: [
                        { label: qsTr("%n Environments", "", backend.environments.length), key: "1", tab: 0 },
                        { label: qsTr("OS Images & Downloads"), key: "2", tab: 1 }
                    ]
                    delegate: Rectangle {
                        required property var modelData
                        height: 28
                        width: tabItemRow.implicitWidth + 18
                        radius: Math.min(3, theme.radius)
                        color: app.activeTab === modelData.tab ? app.surfaceSelected : "transparent"
                        border.width: app.activeTab === modelData.tab ? 1 : 0
                        border.color: theme.accent

                        RowLayout {
                            id: tabItemRow
                            anchors.centerIn: parent
                            spacing: 8
                            Text {
                                text: modelData.label
                                color: app.activeTab === modelData.tab ? theme.foreground : theme.muted
                                font.family: app.sansFont
                                font.pixelSize: 12
                                font.weight: app.activeTab === modelData.tab ? Font.Medium : Font.Normal
                            }
                            Text {
                                text: modelData.key
                                color: theme.muted
                                font.family: app.monoFont
                                font.pixelSize: 10
                            }
                        }

                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: app.activeTab = modelData.tab
                        }
                    }
                }
            }
        }

        Item { Layout.fillWidth: true }

        RowLayout {
            spacing: 10
            Layout.alignment: Qt.AlignVCenter

            ActionButton {
                app: appHeaderRoot.app
                label: qsTr("Secrets")
                shortcutHint: "A"
                minWidth: 96
                disabled: backend.firstRun
                Layout.alignment: Qt.AlignVCenter
                onClicked: app.openSecretsDialog()
            }

            ActionButton {
                app: appHeaderRoot.app
                label: qsTr("Backups")
                shortcutHint: "B"
                minWidth: 96
                disabled: backend.firstRun
                Layout.alignment: Qt.AlignVCenter
                onClicked: app.openBackupsDialog()
            }

            ActionButton {
                app: appHeaderRoot.app
                label: backend.recentFailedHistoryCount > 0
                       ? qsTr("History (%1!)").arg(backend.recentFailedHistoryCount)
                       : qsTr("History")
                shortcutHint: "H"
                variant: backend.recentFailedHistoryCount > 0 ? "danger" : ""
                minWidth: 96
                disabled: backend.firstRun
                Layout.alignment: Qt.AlignVCenter
                onClicked: app.openHistoryDialog()
            }

            ActionButton {
                app: appHeaderRoot.app
                slidersIcon: true
                minWidth: 34
                disabled: backend.firstRun || backend.busy
                Layout.alignment: Qt.AlignVCenter
                onClicked: app.openSettings(0)
                ToolTip.visible: hovered
                ToolTip.text: qsTr("Configuration (3)")
            }

            ActionButton {
                app: appHeaderRoot.app
                label: qsTr("Refresh")
                shortcutHint: "R"
                minWidth: 96
                disabled: backend.busy
                Layout.alignment: Qt.AlignVCenter
                onClicked: backend.refresh()
            }

            ActionButton {
                app: appHeaderRoot.app
                label: "?"
                minWidth: 34
                Layout.alignment: Qt.AlignVCenter
                onClicked: app.helpVisible = !app.helpVisible
            }

            ActionButton {
                app: appHeaderRoot.app
                label: qsTr("+ New Environment")
                shortcutHint: "N"
                variant: "primary"
                minWidth: 165
                disabled: backend.busy
                Layout.alignment: Qt.AlignVCenter
                onClicked: app.openCreateDialog("")
            }
        }
    }

    Rectangle {
        anchors.bottom: parent.bottom
        width: parent.width
        height: 1
        color: app.borderSubtle
    }
}
