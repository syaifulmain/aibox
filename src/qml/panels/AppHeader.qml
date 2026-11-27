import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import "../components"

Rectangle {
    id: appHeaderRoot
    property QtObject app: null
    readonly property int envCount: backend.count
    Layout.fillWidth: true
    Layout.preferredHeight: Style.headerHeight
    color: theme.surfaceCanvas

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Style.padSection
        anchors.rightMargin: Style.padSection
        spacing: Style.padSection

        // App brand
        RowLayout {
            spacing: Style.gap

            BrandMark { markSize: Style.controlHeight }

            Text {
                text: qsTr("AIBox")
                color: theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.fontTitle
                font.weight: Font.Medium
            }
        }

        // Main tab switcher
        SegmentedTabs {
            Layout.leftMargin: Style.padCard
            Layout.alignment: Qt.AlignVCenter
            tabs: [
                { label: qsTr("%n Environments", "", appHeaderRoot.envCount), key: "1", value: 0 },
                { label: qsTr("OS Images"), key: "2", value: 1 }
            ]
            currentValue: app.activeTab
            mediumWhenSelected: true
            onSelected: function (value) { app.activeTab = value }
        }

        Item { Layout.fillWidth: true }

        RowLayout {
            spacing: Style.gap
            Layout.alignment: Qt.AlignVCenter

            ActionButton {
                label: qsTr("Tools")
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
        color: theme.borderSubtle
    }
}
