import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import "../components"

// Network mode, address and per-environment network actions.
Rectangle {
    id: root
    property QtObject app: null

    readonly property string ipAddress: backend.selectedEnvironment.ipv4 || ""
    readonly property bool offline: backend.selectedEnvironment.networkMode === "offline"
    readonly property bool template: backend.selectedEnvironment.isTemplate || false

    Layout.fillWidth: true
    Layout.preferredHeight: netCol.implicitHeight + Style.space16
    radius: Math.min(Style.radius4, theme.radius)
    color: theme.surfaceInput
    border.width: Style.hairline
    border.color: root.offline
                  ? theme.warningBorder
                  : (backend.selectedEnvironment.networkMode === "allowlist" ? theme.accentBorder : theme.borderSubtle)

    ColumnLayout {
        id: netCol
        anchors.fill: parent
        anchors.leftMargin: Style.space12
        anchors.rightMargin: Style.space12
        anchors.topMargin: Style.space10
        anchors.bottomMargin: Style.space10
        spacing: Style.space8

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.space8

            Text {
                text: qsTr("Network")
                color: theme.textSecondary
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
                    selectedTextColor: modelData.id === "offline" ? theme.warningText : theme.textPrimary
                    selectedFill: modelData.id === "offline" ? theme.warningSurface : theme.surfaceSelected
                    fillColor: theme.surfaceCard
                    chipHeight: Style.controlHeightSm
                    horizontalPadding: Style.space16
                    selectedBorderColor: modelData.id === "offline" ? theme.warningBorder : theme.accentBorder
                    interactive: !backend.busy && !root.template
                    opacity: backend.busy ? Style.opacityDisabled : 1.0
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

            Text {
                visible: root.ipAddress.length > 0 && root.ipAddress !== "—" && !root.offline
                text: root.ipAddress
                color: theme.accentText
                font.family: Style.fontMono
                font.pixelSize: Style.font11
                Layout.alignment: Qt.AlignVCenter
                ToolTip.visible: ipHover.hovered
                ToolTip.delay: 500
                ToolTip.text: qsTr("Click to copy this address")
                HoverHandler {
                    id: ipHover
                    cursorShape: Qt.PointingHandCursor
                }
                TapHandler {
                    onTapped: backend.copyText(root.ipAddress, qsTr("IP address"))
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Style.space8

            ActionButton {
                label: (backend.selectedEnvironment.egressRecording || false)
                       ? qsTr("Record: ON")
                       : qsTr("Record: OFF")
                variant: (backend.selectedEnvironment.egressRecording || false) ? "accent" : ""
                minWidth: 104
                disabled: backend.busy || root.template || root.offline
                onClicked: backend.setEgressRecording(
                    backend.selectedEnvironment.name || "",
                    !(backend.selectedEnvironment.egressRecording || false))
            }

            ActionButton {
                label: (backend.selectedEnvironment.portForwardCount || 0) > 0
                       ? qsTr("Ports (%1)").arg(backend.selectedEnvironment.portForwardCount)
                       : qsTr("Ports")
                minWidth: 86
                disabled: root.template
                onClicked: app.openPortsDialog()
            }

            ActionButton {
                label: qsTr("Egress Log")
                minWidth: 96
                disabled: root.template
                onClicked: app.openEgressDialog(backend.selectedEnvironment.name || "")
            }

            Item { Layout.fillWidth: true }
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
                placeholder: qsTr("Allowed domains or CIDRs (e.g. github.com, 1.1.1.1)")
                onAccepted: {
                    if (!backend.busy)
                        backend.setNetworkMode(backend.selectedEnvironment.name || "", "allowlist", app.netAllowlistDraft)
                }
            }

            ActionButton {
                label: qsTr("Apply ACL")
                variant: "primary"
                minWidth: 96
                disabled: backend.busy || app.netAllowlistDraft.trim().length === 0 || root.template
                onClicked: backend.setNetworkMode(backend.selectedEnvironment.name || "", "allowlist", app.netAllowlistDraft)
            }

            ActionButton {
                visible: backend.selectedEnvironment.networkMode === "allowlist"
                label: qsTr("Refresh IPs")
                minWidth: 96
                disabled: backend.busy || root.template
                onClicked: backend.refreshNetworkAllowlist(backend.selectedEnvironment.name || "")
            }
        }
    }
}
