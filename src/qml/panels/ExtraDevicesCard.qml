import QtQuick
import QtQuick.Layouts
import "../components"

// Opt-in host hardware: camera, USB/input event devices, and the CUPS socket.
Card {
    id: root

    property var env: backend.selectedEnvironment
    readonly property bool isVm: (root.env.type || "") === "VIRTUAL-MACHINE"
                                 || (root.env.type || "") === "VM"
    readonly property bool template: root.env.isTemplate || false
    readonly property bool cameraOn: root.env.cameraEnabled || false
    readonly property bool usbOn: root.env.usbEnabled || false
    readonly property bool printerOn: root.env.printerEnabled || false
    readonly property bool anyOn: root.cameraOn || root.usbOn || root.printerOn
    readonly property bool locked: backend.busy || root.template

    visible: !root.isVm
    title: qsTr("Extra Devices")
    borderColor: root.anyOn ? theme.accentBorder : theme.borderSubtle

    RowLayout {
        Layout.fillWidth: true
        spacing: Style.gap

        ToggleChip {
            label: qsTr("Camera")
            checked: root.cameraOn
            disabled: root.locked
            onToggled: backend.setEnvironmentExtraDevice(
                root.env.name || "",
                "camera",
                !root.cameraOn)
        }

        ToggleChip {
            label: qsTr("USB / Gamepad")
            checked: root.usbOn
            disabled: root.locked
            onToggled: backend.setEnvironmentExtraDevice(
                root.env.name || "",
                "usb",
                !root.usbOn)
        }

        ToggleChip {
            label: qsTr("Printer (CUPS)")
            checked: root.printerOn
            disabled: root.locked
            onToggled: backend.setEnvironmentExtraDevice(
                root.env.name || "",
                "printer",
                !root.printerOn)
        }

        Item { Layout.fillWidth: true }
    }

    Text {
        Layout.fillWidth: true
        text: qsTr("Camera and input devices grant real host hardware access; enable only for trusted environments. Printer exposes the host CUPS socket.")
        color: root.anyOn ? theme.warningText : theme.textMuted
        font.family: Style.fontSans
        font.pixelSize: Style.fontSmall
        wrapMode: Text.WordWrap
    }

    Text {
        Layout.fillWidth: true
        text: qsTr("USB/input exposes keyboard, mouse and gamepad event devices to the environment.")
        color: theme.textMuted
        font.family: Style.fontSans
        font.pixelSize: Style.fontSmall
        wrapMode: Text.WordWrap
    }
}
