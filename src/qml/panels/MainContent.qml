import QtQuick
import QtQuick.Layouts
import "../components"

// Hosts both tabs: the master-detail environment view and the OS catalog.
Item {
    id: mainContentRoot
    property QtObject app: null

    Layout.fillWidth: true
    Layout.fillHeight: true
    readonly property string snapshotNameText: environmentInspector.snapshotNameText

    // --------------------------------------------------------------
    // TAB 0: ENVIRONMENTS (MASTER-DETAIL VIEW)
    // --------------------------------------------------------------
    RowLayout {
        anchors.fill: parent
        anchors.margins: Style.padSection
        spacing: Style.padSection
        visible: mainContentRoot.app.activeTab === 0

        // Empty state
        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: backend.count === 0
            color: theme.surfaceCard
            radius: theme.radius
            border.width: Style.hairline
            border.color: theme.borderSubtle

            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(parent.width - Style.padModal * 3, 480)
                spacing: Style.padCard

                BrandMark {
                    Layout.alignment: Qt.AlignHCenter
                    markSize: 42
                    markRadius: Style.radiusLg
                    textSize: Style.fontHeading
                    quiet: true
                }

                Text {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    text: qsTr("No Isolated Environments Yet")
                    color: theme.textPrimary
                    font.family: Style.fontSans
                    font.pixelSize: Style.fontHeading
                    font.weight: Font.Medium
                }

                Text {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.WordWrap
                    lineHeight: 1.35
                    text: qsTr("Create a clean Linux OS container or VM for your next AI, Desktop, or Mobile project. Give AI full root access inside, share files via %1, and wipe it in 1 second when done.").arg(backend.containerWorkspacePath)
                    color: theme.textSecondary
                    font.family: Style.fontSans
                    font.pixelSize: Style.fontBody
                }

                RowLayout {
                    Layout.alignment: Qt.AlignHCenter
                    Layout.topMargin: Style.gapMd
                    spacing: Style.gap

                    ActionButton {
                        label: qsTr("+ Create First Environment")
                        variant: "primary"
                        disabled: backend.busy
                        onClicked: mainContentRoot.app.openCreateDialog("")
                    }

                    ActionButton {
                        label: qsTr("Browse OS Images")
                        onClicked: mainContentRoot.app.activeTab = 1
                    }
                }
            }
        }

        EnvironmentList {
            app: mainContentRoot.app
            Layout.preferredWidth: 352
            Layout.fillHeight: true
            visible: backend.count > 0
        }

        EnvironmentInspector {
            id: environmentInspector
            app: mainContentRoot.app
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: backend.count > 0
        }
    }

    // --------------------------------------------------------------
    // TAB 1: OS IMAGES & ONE-TAP DOWNLOADS
    // --------------------------------------------------------------
    OsCatalogPanel {
        app: mainContentRoot.app
        visible: mainContentRoot.app.activeTab === 1
    }
}
