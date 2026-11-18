import QtQuick
import QtQuick.Layouts
import "../components"

ModalDialog {
    id: editQuotasOverlay
    property QtObject app: null
    open: app.editQuotasVisible
    zOrder: 89
    size: "md"
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
        implicitHeight: editQuotaErrTxt.implicitHeight + Style.padCard * 2
        radius: Math.min(Style.radiusMd, theme.radius)
        color: theme.dangerSurface
        border.width: Style.hairline
        border.color: theme.dangerBorder

        Text {
            id: editQuotaErrTxt
            anchors.fill: parent
            anchors.margins: Style.padCard
            wrapMode: Text.WordWrap
            text: app.editQuotaError
            color: theme.dangerText
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }
    }

    FormRow {
        label: qsTr("CPU Cores")
        FlatInput { id: quotaCpuField; Layout.fillWidth: true; placeholder: "4" }
    }

    FormRow {
        label: qsTr("RAM Limit")
        FlatInput { id: quotaRamField; Layout.fillWidth: true; placeholder: "8GiB" }
    }

    FormRow {
        label: qsTr("Disk Quota (Used: %1)").arg(backend.selectedEnvironment.diskUsage || "0 MB")
        FlatInput { id: quotaDiskField; Layout.fillWidth: true; placeholder: "40GiB" }
    }

    FormRow {
        visible: !app.editQuotaIsVm
        label: qsTr("Process Limit (pids)")
        FlatInput {
            id: quotaProcField
            Layout.fillWidth: true
            placeholder: "4096 or Unlimited"
        }
    }

    FormRow {
        label: qsTr("Disk I/O Limit")
        FlatInput { id: quotaIoField; Layout.fillWidth: true; placeholder: "200MB or Unlimited" }
    }

    Rectangle {
        readonly property string poolWarn: app.editQuotasVisible
                                           ? backend.poolWarningForQuotaEdit(app.editQuotaEnvName, quotaDiskField.text)
                                           : ""
        visible: poolWarn.length > 0
        Layout.fillWidth: true
        implicitHeight: editPoolWarnTxt.implicitHeight + Style.padSection
        radius: Math.min(Style.radiusMd, theme.radius)
        color: theme.warningSurface
        border.width: Style.hairline
        border.color: theme.warningBorder

        Text {
            id: editPoolWarnTxt
            anchors.fill: parent
            anchors.margins: Style.gapMd
            anchors.leftMargin: Style.padCard
            anchors.rightMargin: Style.padCard
            wrapMode: Text.WordWrap
            text: parent.poolWarn
            color: theme.warningText
            font.family: Style.fontSans
            font.pixelSize: Style.fontSmall
        }
    }

    DialogFooter {
        ActionButton {
            label: qsTr("Save")
            variant: "primary"
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
