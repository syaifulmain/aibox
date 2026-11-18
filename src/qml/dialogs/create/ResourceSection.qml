import QtQuick
import QtQuick.Layouts
import "../../components"

// Disk, RAM, CPU, process, and I/O limits plus storage pool overcommit warning.
ColumnLayout {
    id: root
    property QtObject app: null

    Layout.fillWidth: true
    spacing: Style.padCard

    // Disk, RAM and CPU quotas
    RowLayout {
        Layout.fillWidth: true
        spacing: Style.padSection

        ColumnLayout {
            Layout.fillWidth: true
            spacing: Style.gapSm

            Text {
                text: qsTr("Disk Quota")
                color: theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.fontBody
                font.weight: Font.Medium
            }

            RowLayout {
                spacing: Style.gapSm

                Repeater {
                    model: backend.diskOptions
                    delegate: SelectChip {
                        required property string modelData
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        text: modelData
                        selected: root.app.newEnvDisk === modelData
                        stretchText: true
                        onClicked: root.app.newEnvDisk = modelData
                    }
                }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: Style.gapSm

            Text {
                text: qsTr("RAM Limit")
                color: theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.fontBody
                font.weight: Font.Medium
            }

            RowLayout {
                spacing: Style.gapSm

                Repeater {
                    model: backend.ramOptions
                    delegate: SelectChip {
                        required property string modelData
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        text: modelData
                        selected: root.app.newEnvRam === modelData
                        stretchText: true
                        onClicked: root.app.newEnvRam = modelData
                    }
                }
            }
        }

        ColumnLayout {
            spacing: Style.gapSm

            Text {
                text: qsTr("CPU Cores")
                color: theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.fontBody
                font.weight: Font.Medium
            }

            RowLayout {
                spacing: Style.gapSm

                Repeater {
                    model: backend.cpuOptions
                    delegate: SelectChip {
                        required property int modelData
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        text: modelData
                        selected: root.app.newEnvCpu === modelData
                        stretchText: true
                        onClicked: root.app.newEnvCpu = modelData
                    }
                }
            }
        }
    }

    // Runaway limits (processes and disk I/O)
    RowLayout {
        Layout.fillWidth: true
        spacing: Style.padSection

        ColumnLayout {
            Layout.fillWidth: true
            spacing: Style.gapSm

            Text {
                text: qsTr("Process Limit (Container)")
                color: theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.fontBody
                font.weight: Font.Medium
            }

            RowLayout {
                spacing: Style.gapSm

                Repeater {
                    model: backend.processLimitOptions
                    delegate: SelectChip {
                        required property string modelData
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        text: modelData
                        selected: root.app.newEnvProcLimit === modelData
                        stretchText: true
                        compact: true
                        onClicked: root.app.newEnvProcLimit = modelData
                    }
                }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: Style.gapSm

            Text {
                text: qsTr("Disk I/O Limit (Read / Write)")
                color: theme.textPrimary
                font.family: Style.fontSans
                font.pixelSize: Style.fontBody
                font.weight: Font.Medium
            }

            RowLayout {
                spacing: Style.gapSm

                Repeater {
                    model: backend.ioLimitOptions
                    delegate: SelectChip {
                        required property string modelData
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        text: modelData
                        selected: root.app.newEnvIoLimit === modelData
                        stretchText: true
                        compact: true
                        onClicked: root.app.newEnvIoLimit = modelData
                    }
                }
            }
        }
    }

    // Storage pool overcommit warning (non-blocking)
    Rectangle {
        readonly property string warnMsg: root.app.createVisible ? backend.poolWarningForCreate(root.app.newEnvDisk) : ""
        visible: warnMsg.length > 0
        Layout.fillWidth: true
        implicitHeight: createPoolWarnTxt.implicitHeight + Style.padSection
        radius: Math.min(Style.radiusMd, theme.radius)
        color: theme.warningSurface
        border.width: Style.hairline
        border.color: theme.warningBorder

        Text {
            id: createPoolWarnTxt
            anchors.fill: parent
            anchors.margins: Style.gapMd
            anchors.leftMargin: Style.padCard
            anchors.rightMargin: Style.padCard
            wrapMode: Text.WordWrap
            text: parent.warnMsg
            color: theme.warningText
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }
    }
}
