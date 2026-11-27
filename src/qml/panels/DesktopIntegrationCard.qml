import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import "../components"

// Per-environment desktop bridge toggle: native Wayland with an X11 fallback.
Card {
    id: root

    readonly property var env: backend.selectedEnvironment
    readonly property bool isVm: (root.env.type || "") === "VIRTUAL-MACHINE"
                                 || (root.env.type || "") === "VM"
    readonly property bool desktopOn: root.env.desktopEnabled || false
    readonly property bool template: root.env.isTemplate || false

    visible: !root.isVm
    title: qsTr("Desktop Integration")
    borderColor: root.desktopOn ? theme.accentBorder : theme.borderSubtle

    actions: [
        ToggleChip {
            label: qsTr("Desktop")
            checked: root.desktopOn
            disabled: backend.busy || root.template || root.isVm
            onToggled: backend.setEnvironmentDesktop(
                root.env.name || "",
                !root.desktopOn)
        }
    ]

    Text {
        Layout.fillWidth: true
        text: root.desktopOn
              ? qsTr("Wayland apps (with X11 fallback), clipboard and drag-and-drop use your host desktop.")
              : qsTr("Off — headless server mode. Enable before running GUI programs.")
        color: root.desktopOn ? theme.textSecondary : theme.textMuted
        font.family: Style.fontSans
        font.pixelSize: Style.fontSmall
        wrapMode: Text.WordWrap
    }
}
