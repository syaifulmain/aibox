import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import "../components"

Rectangle {
    id: appHeaderRoot
    property QtObject app: null
    Layout.fillWidth: true
    Layout.preferredHeight: Style.headerHeight
    color: theme.darkBackground

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Style.space20
        anchors.rightMargin: Style.space20
        spacing: Style.space16

        // App brand
        RowLayout {
            spacing: Style.space10

            BrandMark { markSize: 28 }

            Text {
                text: qsTr("AIBox")
                color: theme.foreground
                font.family: Style.fontSans
                font.pixelSize: Style.font15
                font.weight: Font.Medium
            }
            Text {
                text: "·"
                color: theme.muted
                font.pixelSize: Style.font14
            }
            Text {
                text: qsTr("Isolated Linux OS Manager")
                color: theme.muted
                font.family: Style.fontSans
                font.pixelSize: Style.font12
            }
        }

        // Main tab switcher
        SegmentedTabs {
            Layout.leftMargin: Style.space12
            Layout.alignment: Qt.AlignVCenter
            tabs: [
                { label: qsTr("%n Environments", "", backend.environments.length), key: "1", value: 0 },
                { label: qsTr("OS Images & Downloads"), key: "2", value: 1 }
            ]
            currentValue: app.activeTab
            mediumWhenSelected: true
            onSelected: function (value) { app.activeTab = value }
        }

        Item { Layout.fillWidth: true }

        RowLayout {
            spacing: Style.space10
            Layout.alignment: Qt.AlignVCenter

            ActionButton {
                label: qsTr("Secrets")
                shortcutHint: "A"
                minWidth: 96
                disabled: backend.firstRun
                Layout.alignment: Qt.AlignVCenter
                onClicked: app.openSecretsDialog()
            }

            ActionButton {
                label: qsTr("Backups")
                shortcutHint: "B"
                minWidth: 96
                disabled: backend.firstRun
                Layout.alignment: Qt.AlignVCenter
                onClicked: app.openBackupsDialog()
            }

            ActionButton {
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
                slidersIcon: true
                minWidth: Style.controlHeight
                disabled: backend.firstRun || backend.busy
                Layout.alignment: Qt.AlignVCenter
                onClicked: app.openSettings(0)
                ToolTip.visible: hovered
                ToolTip.text: qsTr("Configuration (3)")
            }

            ActionButton {
                label: qsTr("Refresh")
                shortcutHint: "R"
                minWidth: 96
                disabled: backend.busy
                Layout.alignment: Qt.AlignVCenter
                onClicked: backend.refresh()
            }

            ActionButton {
                label: "?"
                minWidth: Style.controlHeight
                Layout.alignment: Qt.AlignVCenter
                onClicked: app.helpVisible = !app.helpVisible
            }

            ActionButton {
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
        height: Style.hairline
        color: Style.borderSubtle
    }
}
