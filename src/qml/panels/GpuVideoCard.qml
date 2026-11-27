import QtQuick
import QtQuick.Layouts
import "../components"

// Per-environment GPU passthrough: the ai-gui profile exposes host /dev/dri.
Card {
    id: root

    readonly property var env: backend.selectedEnvironment
    readonly property bool isVm: (root.env.type || "") === "VIRTUAL-MACHINE"
                                 || (root.env.type || "") === "VM"
    readonly property bool template: root.env.isTemplate || false
    readonly property string preset: root.env.preset || ""
    // The GUI preset string carries GPU; custom presets without it stay hidden.
    readonly property bool gpuPreset: !root.isVm
                                      && root.preset.toUpperCase().indexOf("GPU") >= 0

    visible: root.gpuPreset
    title: qsTr("GPU & Video")
    borderColor: root.gpuPreset ? theme.accentBorder : theme.borderSubtle

    actions: [
        ActionButton {
            label: qsTr("Test GPU")
            disabled: backend.busy || root.template
            onClicked: backend.testEnvironmentGpu(root.env.name || "")
        }
    ]

    Text {
        Layout.fillWidth: true
        text: qsTr("GPU, WebGL and VA-API video decode use the host /dev/dri. Install mesa, intel-media-driver, libva-utils and mesa-utils inside the environment for the full stack.")
        color: theme.textSecondary
        font.family: Style.fontSans
        font.pixelSize: Style.fontSmall
        wrapMode: Text.WordWrap
    }

    Text {
        Layout.fillWidth: true
        text: qsTr("The ai-gui profile exposes /dev/dri with mode=0666, so container root can use it; non-root container users need render and video GIDs that match the host.")
        color: theme.textMuted
        font.family: Style.fontSans
        font.pixelSize: Style.fontSmall
        wrapMode: Text.WordWrap
    }
}
