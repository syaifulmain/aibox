import QtQuick
import QtQuick.Layouts
import "../components"

ModalDialog {
    id: editQuotasOverlay
    property QtObject app: null
    open: app.editQuotasVisible
    zOrder: 89
    cardWidth: Math.min(parent.width - Style.modalGutter, 540)
    contentSpacing: Style.space14
    onDismissed: app.editQuotasVisible = false

    onVisibleChanged: {
        if (!visible)
            return
        quotaCpuField.text = String(app.editQuotaCpu)
        quotaRamField.text = app.editQuotaRam
        quotaDiskField.text = app.editQuotaDisk
        quotaProcField.text = app.editQuotaProc
        quotaIoField.text = app.editQuotaIo
    }

    ModalHeader {
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
        radius: Math.min(Style.radius4, theme.radius)
        color: theme.dangerSurface
        border.width: Style.hairline
        border.color: theme.dangerBorder
        Text {
            anchors.fill: parent
            anchors.leftMargin: Style.space12
            anchors.rightMargin: Style.space12
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
            text: app.editQuotaError
            color: theme.dangerText
            font.family: Style.fontSans
            font.pixelSize: Style.font11
        }
    }

    GridLayout {
        Layout.fillWidth: true
        columns: 2
        columnSpacing: Style.space12
        rowSpacing: Style.space10

        Text { text: qsTr("CPU Cores"); color: theme.textPrimary; font.family: Style.fontSans; font.pixelSize: Style.font12 }
        FlatInput { id: quotaCpuField; Layout.fillWidth: true; placeholder: "4" }

        Text { text: qsTr("RAM Limit"); color: theme.textPrimary; font.family: Style.fontSans; font.pixelSize: Style.font12 }
        FlatInput { id: quotaRamField; Layout.fillWidth: true; placeholder: "8GiB" }

        Text {
            text: qsTr("Disk Quota (Used: %1)").arg(backend.selectedEnvironment.diskUsage || "0 MB")
            color: theme.textPrimary
            font.family: Style.fontSans
            font.pixelSize: Style.font12
        }
        FlatInput { id: quotaDiskField; Layout.fillWidth: true; placeholder: "40GiB" }

        Text {
            visible: !app.editQuotaIsVm
            text: qsTr("Process Limit (pids)")
            color: theme.textPrimary
            font.family: Style.fontSans
            font.pixelSize: Style.font12
        }
        FlatInput {
            id: quotaProcField
            visible: !app.editQuotaIsVm
            Layout.fillWidth: true
            placeholder: "4096 or Unlimited"
        }

        Text { text: qsTr("Disk I/O Limit"); color: theme.textPrimary; font.family: Style.fontSans; font.pixelSize: Style.font12 }
        FlatInput { id: quotaIoField; Layout.fillWidth: true; placeholder: "200MB or Unlimited" }
    }

    Rectangle {
        readonly property string poolWarn: app.editQuotasVisible
                                           ? backend.poolWarningForQuotaEdit(app.editQuotaEnvName, quotaDiskField.text)
                                           : ""
        visible: poolWarn.length > 0
        Layout.fillWidth: true
        implicitHeight: editPoolWarnTxt.implicitHeight + Style.space16
        radius: Math.min(Style.radius4, theme.radius)
        color: theme.warningSurface
        border.width: Style.hairline
        border.color: theme.warningBorder
        Text {
            id: editPoolWarnTxt
            anchors.fill: parent
            anchors.margins: Style.space8
            anchors.leftMargin: Style.space12
            anchors.rightMargin: Style.space12
            wrapMode: Text.WordWrap
            text: parent.poolWarn
            color: theme.warningText
            font.family: Style.fontSans
            font.pixelSize: Style.font11
        }
    }

    RowLayout {
        Layout.fillWidth: true
        Layout.topMargin: Style.space4
        spacing: Style.space10
        Item { Layout.fillWidth: true }

        ActionButton {
            label: qsTr("Cancel")
            minWidth: 96
            onClicked: app.editQuotasVisible = false
        }

        ActionButton {
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
