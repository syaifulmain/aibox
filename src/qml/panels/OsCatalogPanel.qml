import QtQuick
import QtQuick.Layouts
import "../components"

// Tab 1: OS image catalog and one-tap downloads.
ColumnLayout {
    id: osCatalogRoot
    property QtObject app: null

    anchors.fill: parent
    anchors.margins: Style.space20
    spacing: Style.space14

    RowLayout {
        Layout.fillWidth: true

        ColumnLayout {
            spacing: Style.space2

            Text {
                text: qsTr("Linux OS Image Catalog")
                color: theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.font16
                font.weight: Font.Medium
            }
            Text {
                text: qsTr("Pre-download OS images to local storage so new environments launch in under 1 second, or click 'Use This OS' to start immediately.")
                color: theme.textSecondary
                font.family: Style.fontSans
                font.pixelSize: Style.font12
            }
        }
    }

    ListView {
        id: osListView
        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        spacing: Style.space8
        model: backend.osCatalog

        delegate: Rectangle {
            required property var modelData
            width: osListView.width
            height: Style.listRowHeight
            radius: Math.min(Style.radius4, theme.radius)
            color: theme.surfaceCard
            border.width: Style.hairline
            border.color: modelData.downloaded ? theme.successBorder : theme.borderSubtle

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: Style.space16
                anchors.rightMargin: Style.space16
                spacing: Style.space14

                // Cached / download state icon
                Rectangle {
                    width: Style.iconTile
                    height: Style.iconTile
                    radius: Math.min(Style.radius4, theme.radius)
                    color: modelData.downloaded ? theme.successSurface : theme.surfaceInput
                    border.width: Style.hairline
                    border.color: modelData.downloaded ? theme.successBorder : theme.borderSubtle

                    Text {
                        anchors.centerIn: parent
                        text: modelData.downloaded ? "✓" : "↓"
                        color: modelData.downloaded ? theme.successText : theme.accentText
                        font.family: Style.fontMono
                        font.pixelSize: Style.font14
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Style.space3

                    RowLayout {
                        spacing: Style.space8

                        Text {
                            text: modelData.name
                            color: theme.textPrimary
                            font.family: Style.fontSans
                            font.pixelSize: Style.font14
                            font.weight: Font.Medium
                        }

                        TagBadge {
                            text: modelData.badge
                            tone: "accentQuiet"
                            textSize: Style.font10
                        }

                        TagBadge {
                            text: modelData.downloading ? qsTr("Downloading...") : modelData.sizeText
                            tone: modelData.downloaded ? "success" : "quiet"
                            textSize: Style.font10
                        }
                    }

                    Text {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: modelData.description
                        color: theme.textSecondary
                        font.family: Style.fontSans
                        font.pixelSize: Style.font12
                    }
                }

                ActionButton {
                    label: modelData.downloading
                           ? qsTr("Downloading...")
                           : (modelData.downloaded ? qsTr("Remove Cache") : qsTr("Download OS"))
                    variant: modelData.downloaded ? "danger" : ""
                    minWidth: 135
                    disabled: backend.busy
                    onClicked: {
                        if (modelData.downloaded)
                            backend.removeOsImage(modelData.id)
                        else
                            backend.downloadOsImage(modelData.id)
                    }
                }

                ActionButton {
                    label: qsTr("Use This OS")
                    variant: "primary"
                    minWidth: 125
                    disabled: backend.busy
                    onClicked: osCatalogRoot.app.openCreateDialog(modelData.id)
                }
            }
        }
    }
}
