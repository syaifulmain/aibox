import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Dialogs
import QtQuick.Layouts
import "../components"

Rectangle {
    property QtObject app: null
    id: editQuotasOverlay
    anchors.fill: parent
    visible: app.editQuotasVisible
    color: "#c406070a"
    z: 89

    onVisibleChanged: {
        if (!visible)
            return
        quotaCpuField.text = String(app.editQuotaCpu)
        quotaRamField.text = app.editQuotaRam
        quotaDiskField.text = app.editQuotaDisk
        quotaProcField.text = app.editQuotaProc
        quotaIoField.text = app.editQuotaIo
    }

    MouseArea {
        anchors.fill: parent
        onClicked: app.editQuotasVisible = false
    }

    Rectangle {
        anchors.centerIn: parent
        width: Math.min(parent.width - 56, 540)
        height: editQuotaCol.implicitHeight + 48
        radius: Math.min(8, theme.radius)
        color: app.surfaceCard
        border.width: 1
        border.color: theme.accent

        MouseArea { anchors.fill: parent }

        ColumnLayout {
            id: editQuotaCol
            anchors.fill: parent
            anchors.margins: 24
            spacing: 14

            ModalHeader {
                app: editQuotasOverlay.app
                title: qsTr("Edit Resource Quotas · %1").arg(app.editQuotaEnvName)
                subtitle: app.editQuotaIsVm
                          ? qsTr("VMs require a restart for CPU or RAM changes; process limits apply to containers only.")
                          : qsTr("Applies CPU, memory, disk quota, process cap, and I/O limits live.")
                onClosed: app.editQuotasVisible = false
            }

            Rectangle {
                visible: app.editQuotaError.length > 0
                Layout.fillWidth: true
                height: 36
                radius: Math.min(4, theme.radius)
                color: "#261319"
                border.width: 1
                border.color: theme.red
                Text {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    verticalAlignment: Text.AlignVCenter
                    elide: Text.ElideRight
                    text: app.editQuotaError
                    color: theme.red
                    font.family: app.sansFont
                    font.pixelSize: 11
                }
            }

            GridLayout {
                Layout.fillWidth: true
                columns: 2
                columnSpacing: 12
                rowSpacing: 10

                Text { text: qsTr("CPU Cores"); color: theme.foreground; font.family: app.sansFont; font.pixelSize: 12 }
                FlatInput { app: editQuotasOverlay.app; id: quotaCpuField; Layout.fillWidth: true; implicitHeight: 34; placeholder: "4" }

                Text { text: qsTr("RAM Limit"); color: theme.foreground; font.family: app.sansFont; font.pixelSize: 12 }
                FlatInput { app: editQuotasOverlay.app; id: quotaRamField; Layout.fillWidth: true; implicitHeight: 34; placeholder: "8GiB" }

                Text {
                    text: qsTr("Disk Quota (Used: %1)").arg(backend.selectedEnvironment.diskUsage || "0 MB")
                    color: theme.foreground
                    font.family: app.sansFont
                    font.pixelSize: 12
                }
                FlatInput { app: editQuotasOverlay.app; id: quotaDiskField; Layout.fillWidth: true; implicitHeight: 34; placeholder: "40GiB" }

                Text {
                    visible: !app.editQuotaIsVm
                    text: qsTr("Process Limit (pids)")
                    color: theme.foreground
                    font.family: app.sansFont
                    font.pixelSize: 12
                }
                FlatInput {
                    app: editQuotasOverlay.app
                    id: quotaProcField
                    visible: !app.editQuotaIsVm
                    Layout.fillWidth: true
                    implicitHeight: 34
                    placeholder: "4096 or Unlimited"
                }

                Text { text: qsTr("Disk I/O Limit"); color: theme.foreground; font.family: app.sansFont; font.pixelSize: 12 }
                FlatInput { app: editQuotasOverlay.app; id: quotaIoField; Layout.fillWidth: true; implicitHeight: 34; placeholder: "200MB or Unlimited" }
            }

            Rectangle {
                readonly property string poolWarn: app.editQuotasVisible
                                                   ? backend.poolWarningForQuotaEdit(app.editQuotaEnvName, quotaDiskField.text)
                                                   : ""
                visible: poolWarn.length > 0
                Layout.fillWidth: true
                implicitHeight: editPoolWarnTxt.implicitHeight + 16
                radius: Math.min(4, theme.radius)
                color: "#262013"
                border.width: 1
                border.color: "#e5a93c"
                Text {
                    id: editPoolWarnTxt
                    anchors.fill: parent
                    anchors.margins: 8
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    wrapMode: Text.WordWrap
                    text: parent.poolWarn
                    color: "#e5a93c"
                    font.family: app.sansFont
                    font.pixelSize: 11
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: 4
                spacing: 10
                Item { Layout.fillWidth: true }
                ActionButton {
                    app: editQuotasOverlay.app
                    label: qsTr("Cancel")
                    minWidth: 96
                    onClicked: app.editQuotasVisible = false
                }
                ActionButton {
                    app: editQuotasOverlay.app
                    label: qsTr("Apply Quotas")
                    variant: "primary"
                    minWidth: 140
                    disabled: backend.busy
                    onClicked: {
                        const cpu = parseInt(quotaCpuField.text.trim(), 10)
                        const err = backend.updateEnvironmentResources(
                            app.editQuotaEnvName,
                            isNaN(cpu) ? 0 : cpu,
                            quotaRamField.text,
                            quotaDiskField.text,
                            quotaProcField.text,
                            quotaIoField.text)
                        if (err.length > 0) {
                            app.editQuotaError = err
                        } else {
                            app.editQuotasVisible = false
                        }
                    }
                }
            }
        }
    }
}
