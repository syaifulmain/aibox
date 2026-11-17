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
        }

        // Main tab switcher
        SegmentedTabs {
            Layout.leftMargin: Style.space12
            Layout.alignment: Qt.AlignVCenter
            tabs: [
                { label: qsTr("%n Environments", "", backend.environments.length), key: "1", value: 0 },
                { label: qsTr("OS Images"), key: "2", value: 1 }
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
                label: qsTr("Tools")
                minWidth: 92
                disabled: backend.firstRun
                Layout.alignment: Qt.AlignVCenter
                onClicked: app.toolsVisible = true
                ToolTip.visible: hovered
                ToolTip.text: qsTr("Secrets, backups, history and cleanup")
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
