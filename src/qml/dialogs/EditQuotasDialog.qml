import QtQuick
import QtQuick.Layouts
import "../components"

ModalDialog {
    id: editQuotasOverlay
    property QtObject app: null
    property string ramUnit: "GiB"
    property string diskUnit: "GiB"
    property string ioUnit: "MB"
    property string swapValue: "Host"
    readonly property var swapChoices: {
        const base = backend.swapOptions || []
        const out = []
        for (let i = 0; i < base.length; ++i)
            out.push(base[i])
        if (out.indexOf(swapValue) < 0)
            out.unshift(swapValue)
        return out
    }
    readonly property var ramUnits: unitOptions(["GiB", "MiB", "TiB"], ramUnit)
    readonly property var diskUnits: unitOptions(["GiB", "MiB", "TiB"], diskUnit)
    readonly property var ioUnits: unitOptions(["MB", "MiB", "GB", "GiB"], ioUnit)
    open: app.editQuotasVisible
    zOrder: 89
    size: "md"
    onDismissed: app.editQuotasVisible = false

    // The loader creates this dialog only when it is already open, so load on completion.
    Component.onCompleted: if (open) loadValues()

    function unitOptions(base, current) {
        return base.indexOf(current) >= 0 ? base : [current].concat(base)
    }

    function unitOf(value, fallback) {
        const match = String(value).trim().match(/(GiB|MiB|TiB|GB|MB|TB)$/i)
        return match ? match[1] : fallback
    }

    function numberPart(value) {
        const match = String(value).trim().match(/^\d+(?:\.\d+)?/)
        return match ? match[0] : String(value).trim()
    }

    function composeSize(value, unit) {
        const raw = String(value).trim()
        if (raw.length === 0)
            return ""
        if (raw.toLowerCase() === "unlimited")
            return "Unlimited"
        const match = raw.match(/^\d+(?:\.\d+)?/)
        return match ? match[0] + unit : raw
    }

    function loadValues() {
        quotaCpuField.text = String(app.editQuotaCpu)
        ramUnit = unitOf(app.editQuotaRam, "GiB")
        quotaRamField.text = numberPart(app.editQuotaRam)
        diskUnit = unitOf(app.editQuotaDisk, "GiB")
        quotaDiskField.text = numberPart(app.editQuotaDisk)
        quotaProcField.text = app.editQuotaProc
        ioUnit = unitOf(app.editQuotaIo, "MB")
        quotaIoField.text = numberPart(app.editQuotaIo)
        swapValue = app.editQuotaSwap.length > 0 ? app.editQuotaSwap : "Host"
    }

    ModalHeader {
        title: qsTr("Edit Resource Quotas · %1").arg(app.editQuotaEnvName)
        subtitle: app.editQuotaIsVm
                  ? qsTr("VMs require a restart for CPU or RAM changes; process limits apply to containers only.")
                  : qsTr("Applies CPU, memory, swap, disk quota, process cap, and I/O limits live.")
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

        FlatInput { id: quotaRamField; Layout.fillWidth: true; placeholder: "8" }

        SizeUnitChips {
            units: editQuotasOverlay.ramUnits
            current: editQuotasOverlay.ramUnit
            onSelected: function (unit) { editQuotasOverlay.ramUnit = unit }
        }
    }

    FormRow {
        visible: !app.editQuotaIsVm
        label: qsTr("Swap Limit")
        hint: qsTr("Host follows the Incus default; Off disables swap.")

        Repeater {
            model: editQuotasOverlay.swapChoices

            delegate: SelectChip {
                required property string modelData
                Layout.fillWidth: true
                Layout.preferredWidth: 0
                text: modelData
                selected: editQuotasOverlay.swapValue === modelData
                stretchText: true
                compact: true
                onClicked: editQuotasOverlay.swapValue = modelData
            }
        }
    }

    Text {
        visible: !app.editQuotaIsVm && !backend.hostSwapAvailable
        Layout.fillWidth: true
        text: qsTr("Host has no active swap. Swap limits will not be used.")
        color: theme.warningText
        wrapMode: Text.WordWrap
        font.family: Style.fontSans
        font.pixelSize: Style.fontSmall
    }

    Text {
        visible: app.editQuotaIsVm
        Layout.fillWidth: true
        text: qsTr("VM swap is managed inside the guest.")
        color: theme.textMuted
        wrapMode: Text.WordWrap
        font.family: Style.fontSans
        font.pixelSize: Style.fontSmall
    }

    FormRow {
        label: qsTr("Disk Quota (Used: %1)").arg(backend.selectedEnvironment.diskUsage || "0 MB")

        FlatInput { id: quotaDiskField; Layout.fillWidth: true; placeholder: "40" }

        SizeUnitChips {
            units: editQuotasOverlay.diskUnits
            current: editQuotasOverlay.diskUnit
            onSelected: function (unit) { editQuotasOverlay.diskUnit = unit }
        }
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
        hint: qsTr("Type Unlimited to remove the cap.")

        FlatInput { id: quotaIoField; Layout.fillWidth: true; placeholder: "200" }

        SizeUnitChips {
            units: editQuotasOverlay.ioUnits
            current: editQuotasOverlay.ioUnit
            onSelected: function (unit) { editQuotasOverlay.ioUnit = unit }
        }
    }

    Rectangle {
        readonly property string poolWarn: app.editQuotasVisible
                                           ? backend.poolWarningForQuotaEdit(app.editQuotaEnvName,
                                               editQuotasOverlay.composeSize(quotaDiskField.text, editQuotasOverlay.diskUnit))
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
                const swapLimit = editQuotasOverlay.swapValue.length > 0
                                 ? editQuotasOverlay.swapValue
                                 : "Host"
                const err = backend.updateEnvironmentResources(
                    app.editQuotaEnvName,
                    isNaN(cpu) ? 0 : cpu,
                    editQuotasOverlay.composeSize(quotaRamField.text, editQuotasOverlay.ramUnit),
                    editQuotasOverlay.composeSize(quotaDiskField.text, editQuotasOverlay.diskUnit),
                    quotaProcField.text,
                    editQuotasOverlay.composeSize(quotaIoField.text, editQuotasOverlay.ioUnit),
                    swapLimit)
                if (err.length > 0) {
                    app.editQuotaError = err
                } else {
                    app.editQuotasVisible = false
                }
            }
        }
    }
}
