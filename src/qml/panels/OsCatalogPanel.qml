import QtQuick
import QtQuick.Layouts
import "../components"

// Tab 1: OS image catalog and one-tap downloads.
ColumnLayout {
    id: osCatalogRoot
    property QtObject app: null

    anchors.fill: parent
    anchors.margins: Style.padSection
    spacing: Style.padCard

    RowLayout {
        Layout.fillWidth: true

        ColumnLayout {
            spacing: Style.gapXs

            Text {
                text: qsTr("Linux OS Image Catalog")
                color: theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.fontHeading
                font.weight: Font.Medium
            }
            Text {
                text: qsTr("Pre-download OS images to local storage so new environments launch in under 1 second, or click 'Use This OS' to start immediately.")
                color: theme.textSecondary
                font.family: Style.fontSans
                font.pixelSize: Style.fontBody
            }
        }
    }

    ListView {
        id: osListView
        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        spacing: Style.gapMd
        model: backend.osCatalog

        delegate: Rectangle {
            required property var modelData
            width: osListView.width
            height: Style.listRowHeight
            radius: Math.min(Style.radiusMd, theme.radius)
            color: theme.surfaceInput
            border.width: Style.hairline
            border.color: modelData.downloaded ? theme.successBorder : theme.borderSubtle

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: Style.padSection
                anchors.rightMargin: Style.padSection
                spacing: Style.padCard

                // Cached / download state icon
                Rectangle {
                    width: Style.controlHeight
                    height: Style.controlHeight
                    radius: Math.min(Style.radiusMd, theme.radius)
                    color: modelData.downloaded ? theme.successSurface : theme.surfaceCard
                    border.width: Style.hairline
                    border.color: modelData.downloaded ? theme.successBorder : theme.borderSubtle

                    Text {
                        anchors.centerIn: parent
                        text: modelData.downloaded ? "✓" : "↓"
                        color: modelData.downloaded ? theme.successText : theme.accentText
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontTitle
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    spacing: Style.gapXs

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Style.gapMd

                        Text {
                            elide: Text.ElideRight
                            text: modelData.name
                            color: theme.textPrimary
                            font.family: Style.fontSans
                            font.pixelSize: Style.fontBody
                            font.weight: Font.Medium
                        }

                        Text {
                            visible: (modelData.id || "").length > 0
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                            text: modelData.id || ""
                            color: theme.textMuted
                            font.family: Style.fontMono
                            font.pixelSize: Style.fontCaption
                        }
                    }

                    Text {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: modelData.description
                        color: theme.textSecondary
                        font.family: Style.fontSans
                        font.pixelSize: Style.fontBody
                    }
                }

                TagBadge {
                    text: modelData.badge
                    tone: "accentQuiet"
                }

                Text {
                    Layout.preferredWidth: Style.buttonMinWidth + Style.padSection
                    horizontalAlignment: Text.AlignRight
                    elide: Text.ElideRight
                    text: modelData.downloading ? qsTr("Downloading...") : modelData.sizeText
                    color: modelData.downloaded ? theme.successText : theme.textMuted
                    font.family: Style.fontMono
                    font.pixelSize: Style.fontCaption
                }

                ActionButton {
                    compact: true
                    minWidth: Style.buttonMinWidth + Style.padModal + Style.gapMd
                    label: modelData.downloading
                           ? qsTr("Downloading...")
                           : (modelData.downloaded ? qsTr("Remove Cache") : qsTr("Download OS"))
                    variant: modelData.downloaded ? "danger" : "default"
                    disabled: backend.busy
                    onClicked: {
                        if (modelData.downloaded)
                            backend.removeOsImage(modelData.id)
                        else
                            backend.downloadOsImage(modelData.id)
                    }
                }

                ActionButton {
                    compact: true
                    label: qsTr("Use This OS")
                    variant: "primary"
                    disabled: backend.busy
                    onClicked: osCatalogRoot.app.openCreateDialog(modelData.id)
                }
            }
        }
    }
}
