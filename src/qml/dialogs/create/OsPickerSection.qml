import QtQuick
import QtQuick.Layouts
import "../../components"

// Capability preset and Linux distribution grid for fresh environments.
ColumnLayout {
    id: root
    property QtObject app: null

    visible: !root.app.createFromTemplate
    Layout.fillWidth: true
    spacing: Style.padCard

    // Capability preset
    ColumnLayout {
        Layout.fillWidth: true
        spacing: Style.gapSm

        Text {
            text: qsTr("Environment Capability Preset")
            color: theme.textPrimary
            font.family: Style.fontSans
            font.pixelSize: Style.fontBody
            font.weight: Font.Medium
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.gapMd

            Repeater {
                model: backend.presets
                delegate: SelectCard {
                    required property var modelData
                    Layout.fillWidth: true
                    selected: root.app.newEnvPreset === modelData.id
                    title: modelData.title
                    subtitle: modelData.sub
                    subtitleMono: true
                    mediumWhenSelected: true
                    onClicked: root.app.newEnvPreset = modelData.id
                }
            }
        }
    }

    // Linux distribution
    ColumnLayout {
        Layout.fillWidth: true
        spacing: Style.gapSm

        Text {
            text: qsTr("Linux Distribution")
            color: theme.textPrimary
            font.family: Style.fontSans
            font.pixelSize: Style.fontBody
            font.weight: Font.Medium
        }

        GridLayout {
            Layout.fillWidth: true
            columns: 4
            columnSpacing: Style.gapMd
            rowSpacing: Style.gapMd

            Repeater {
                model: backend.osCatalog
                delegate: Rectangle {
                    required property var modelData
                    readonly property bool selected: root.app.newEnvOsId === modelData.id
                    Layout.fillWidth: true
                    implicitHeight: osCardCol.implicitHeight + Style.gapMd * 2
                    radius: Math.min(Style.radiusMd, theme.radius)
                    color: selected ? theme.surfaceSelected : theme.surfaceInput
                    border.width: Style.hairline
                    border.color: selected ? theme.accentBorder : theme.borderSubtle

                    ColumnLayout {
                        id: osCardCol
                        anchors.centerIn: parent
                        spacing: Style.gapXs

                        Text {
                            Layout.alignment: Qt.AlignHCenter
                            text: modelData.id
                            color: selected ? theme.textPrimary : theme.textSecondary
                            font.family: Style.fontMono
                            font.pixelSize: Style.fontBody
                        }
                        Text {
                            Layout.alignment: Qt.AlignHCenter
                            text: modelData.downloaded ? qsTr("✓ Cached") : modelData.badge
                            color: modelData.downloaded ? theme.successText : theme.textMuted
                            font.family: Style.fontSans
                            font.pixelSize: Style.fontCaption
                        }
                    }

                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            root.app.newEnvOsId = modelData.id
                            if (!modelData.downloaded)
                                backend.prefetchOsImage(modelData.id)
                        }
                    }
                }
            }
        }
    }
}
