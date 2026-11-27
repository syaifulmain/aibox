import QtQuick
import QtQuick.Layouts
import "../components"

// Per-environment filtered host session bus: notifications, portals and tray.
Card {
    id: root

    readonly property var env: backend.selectedEnvironment
    readonly property bool isVm: (root.env.type || "") === "VIRTUAL-MACHINE"
                                 || (root.env.type || "") === "VM"
    readonly property bool template: root.env.isTemplate || false
    readonly property bool dbusOn: root.env.dbusEnabled || false

    visible: !root.isVm
    title: qsTr("Host Services (D-Bus)")
    borderColor: root.dbusOn ? theme.accentBorder : theme.borderSubtle

    actions: [
        ToggleChip {
            label: qsTr("Filtered bus")
            checked: root.dbusOn
            disabled: backend.busy || root.template
            onToggled: backend.setEnvironmentDbus(
                root.env.name || "",
                !root.dbusOn)
        },
        ActionButton {
            label: qsTr("Test Host Services")
            disabled: backend.busy || root.template
            onClicked: backend.testEnvironmentDbus(root.env.name || "")
        }
    ]

    Text {
        Layout.fillWidth: true
        text: qsTr("Notifications, file pickers, screen sharing and links use a filtered host session bus. The bus is filtered to notifications, portals and the tray watcher; secrets are not exposed.")
        color: root.dbusOn ? theme.textSecondary : theme.textMuted
        font.family: Style.fontSans
        font.pixelSize: Style.fontSmall
        wrapMode: Text.WordWrap
    }

    Text {
        Layout.fillWidth: true
        text: qsTr("Tray item names are best-effort because the proxy cannot allow dynamic StatusNotifierItem names.")
        color: theme.textMuted
        font.family: Style.fontSans
        font.pixelSize: Style.fontSmall
        wrapMode: Text.WordWrap
    }
}
