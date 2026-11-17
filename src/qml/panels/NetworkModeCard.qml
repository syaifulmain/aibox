import QtQuick
import QtQuick.Layouts
import "../components"

// Network mode selector and allowlist editor.
Rectangle {
    id: root
    property QtObject app: null

    Layout.fillWidth: true
    Layout.preferredHeight: netCol.implicitHeight + Style.space16
    radius: Math.min(Style.radius4, theme.radius)
    color: Style.surfaceInput
    border.width: Style.hairline
    border.color: backend.selectedEnvironment.networkMode === "offline"
                  ? theme.red
                  : (backend.selectedEnvironment.networkMode === "allowlist" ? theme.accent : Style.borderSubtle)

    ColumnLayout {
        id: netCol
        anchors.fill: parent
        anchors.leftMargin: Style.space14
        anchors.rightMargin: Style.space14
        anchors.topMargin: Style.space8
        anchors.bottomMargin: Style.space8
        spacing: Style.space10

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.space10

            Text {
                text: qsTr("Network Mode")
                color: theme.muted
                font.family: Style.fontSans
                font.pixelSize: Style.font11
            }

            Repeater {
                model: [
                    { id: "online", label: qsTr("Online") },
                    { id: "allowlist", label: qsTr("Allowlist") },
                    { id: "offline", label: qsTr("Offline") }
                ]
                delegate: SelectChip {
                    required property var modelData
                    readonly property string activeMode: backend.selectedEnvironment.networkMode || "online"
                    text: modelData.label
                    selected: (modelData.id === "allowlist" && app.netAllowlistEditing)
                              || (!app.netAllowlistEditing && activeMode === modelData.id)
                    mono: false
                    textSize: Style.font11
                    mediumWhenSelected: true
                    selectedTextColor: theme.foreground
                    fillColor: Style.surfaceCard
                    chipHeight: Style.controlHeightSm
                    horizontalPadding: Style.space20
                    selectedBorderColor: modelData.id === "offline" ? theme.red : theme.accent
                    interactive: !backend.busy && !(backend.selectedEnvironment.isTemplate || false)
                    opacity: backend.busy ? 0.5 : 1.0
                    onClicked: {
                        const name = backend.selectedEnvironment.name || ""
                        if (modelData.id === "allowlist") {
                            app.netAllowlistEditing = true
                            if (app.netAllowlistDraft.length === 0)
                                app.netAllowlistDraft = backend.selectedEnvironment.networkAllowlist || ""
                        } else {
                            app.netAllowlistEditing = false
                            backend.setNetworkMode(name, modelData.id, "")
                        }
                    }
                }
            }

            Item { Layout.fillWidth: true }

            ActionButton {
                label: (backend.selectedEnvironment.egressRecording || false)
                       ? qsTr("Record: ON")
                       : qsTr("Record: OFF")
                variant: (backend.selectedEnvironment.egressRecording || false) ? "accent" : ""
                minWidth: 104
                disabled: backend.busy || (backend.selectedEnvironment.isTemplate || false)
                          || backend.selectedEnvironment.networkMode === "offline"
                onClicked: backend.setEgressRecording(
                    backend.selectedEnvironment.name || "",
                    !(backend.selectedEnvironment.egressRecording || false))
            }

            ActionButton {
                label: (backend.selectedEnvironment.portForwardCount || 0) > 0
                       ? qsTr("Ports (%1)").arg(backend.selectedEnvironment.portForwardCount)
                       : qsTr("Ports")
                minWidth: 86
                disabled: backend.selectedEnvironment.isTemplate || false
                onClicked: app.openPortsDialog()
            }

            ActionButton {
                label: qsTr("Egress Log")
                minWidth: 96
                disabled: backend.selectedEnvironment.isTemplate || false
                onClicked: app.openEgressDialog(backend.selectedEnvironment.name || "")
            }

            ActionButton {
                label: backend.selectedEnvironment.networkMode === "offline"
                       ? qsTr("Restore Online")
                       : qsTr("Kill Network")
                shortcutHint: "K"
                variant: backend.selectedEnvironment.networkMode === "offline" ? "accent" : "danger"
                minWidth: 132
                disabled: backend.busy || (backend.selectedEnvironment.isTemplate || false)
                onClicked: {
                    const name = backend.selectedEnvironment.name || ""
                    const nextMode = backend.selectedEnvironment.networkMode === "offline" ? "online" : "offline"
                    app.netAllowlistEditing = false
                    backend.setNetworkMode(name, nextMode, "")
                }
            }
        }

        RowLayout {
            visible: app.netAllowlistEditing || backend.selectedEnvironment.networkMode === "allowlist"
            Layout.fillWidth: true
            spacing: Style.space8

            FlatInput {
                id: allowlistInput
                Layout.fillWidth: true
                text: app.netAllowlistDraft
                onTextChanged: app.netAllowlistDraft = text
                placeholder: qsTr("Allowed domains or CIDRs (e.g. github.com, api.anthropic.com, 1.1.1.1)")
                onAccepted: {
                    if (!backend.busy)
                        backend.setNetworkMode(backend.selectedEnvironment.name || "", "allowlist", app.netAllowlistDraft)
                }
            }

            ActionButton {
                label: qsTr("Apply ACL")
                variant: "primary"
                minWidth: 96
                disabled: backend.busy || app.netAllowlistDraft.trim().length === 0 || (backend.selectedEnvironment.isTemplate || false)
                onClicked: backend.setNetworkMode(backend.selectedEnvironment.name || "", "allowlist", app.netAllowlistDraft)
            }

            ActionButton {
                visible: backend.selectedEnvironment.networkMode === "allowlist"
                label: qsTr("Refresh IPs")
                minWidth: 96
                disabled: backend.busy || (backend.selectedEnvironment.isTemplate || false)
                onClicked: backend.refreshNetworkAllowlist(backend.selectedEnvironment.name || "")
            }
        }
    }
}
