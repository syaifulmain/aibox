import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import "../components"

// Network mode, address and per-environment network actions.
Card {
    id: root
    property QtObject app: null

    readonly property string ipAddress: backend.selectedEnvironment.ipv4 || ""
    readonly property bool offline: backend.selectedEnvironment.networkMode === "offline"
    readonly property bool template: backend.selectedEnvironment.isTemplate || false

    title: qsTr("Network")
    borderColor: root.offline
                 ? theme.warningBorder
                 : (backend.selectedEnvironment.networkMode === "allowlist" ? theme.accentBorder : theme.borderSubtle)

    // Controls on the left (50%), live network telemetry grid on the right (50%)
    RowLayout {
        Layout.fillWidth: true
        spacing: Style.padSection

        ColumnLayout {
            Layout.fillWidth: true
            Layout.preferredWidth: 1
            Layout.alignment: Qt.AlignVCenter
            spacing: Style.gap

            // Row 1: Network mode chips (34 px)
            RowLayout {
                spacing: Style.gap

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
                        textSize: Style.fontSmall
                        mediumWhenSelected: true
                        selectedTextColor: modelData.id === "offline" ? theme.warningText : theme.textPrimary
                        selectedFill: modelData.id === "offline" ? theme.warningSurface : theme.surfaceSelected
                        fillColor: theme.surfaceInput
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
            }

            // Row 2: Recording toggle and network dialogs (34 px)
            RowLayout {
                spacing: Style.gap

                ToggleChip {
                    label: qsTr("Record")
                    checked: backend.selectedEnvironment.egressRecording || false
                    disabled: backend.busy || root.template || root.offline
                    onToggled: backend.setEgressRecording(
                        backend.selectedEnvironment.name || "",
                        !(backend.selectedEnvironment.egressRecording || false))
                }

                ActionButton {
                    label: (backend.selectedEnvironment.portForwardCount || 0) > 0
                           ? qsTr("Ports (%1)").arg(backend.selectedEnvironment.portForwardCount)
                           : qsTr("Ports")
                    disabled: root.template
                    onClicked: app.openPortsDialog()
                }

                ActionButton {
                    label: qsTr("Egress Log")
                    disabled: root.template
                    onClicked: app.openEgressDialog(backend.selectedEnvironment.name || "")
                }
            }
        }

        Rectangle {
            width: Style.hairline
            Layout.fillHeight: true
            color: theme.borderSubtle
        }

        GridLayout {
            Layout.fillWidth: true
            Layout.preferredWidth: 1
            Layout.alignment: Qt.AlignVCenter
            columns: 2
            columnSpacing: Style.padModal
            rowSpacing: Style.gapSm

            Repeater {
                model: [
                    { label: qsTr("Receiving"), value: backend.selectedEnvironment.netRxRateText || "—", accent: false },
                    { label: qsTr("Sending"), value: backend.selectedEnvironment.netTxRateText || "—", accent: false },
                    { label: qsTr("Downloaded"), value: backend.selectedEnvironment.netRxTotalText || "—", accent: false },
                    { label: qsTr("Uploaded"), value: backend.selectedEnvironment.netTxTotalText || "—", accent: false },
                    { label: qsTr("IP Address"), value: (root.ipAddress.length > 0 && !root.offline) ? root.ipAddress : "—", accent: true },
                    { label: qsTr("Interface"), value: backend.selectedEnvironment.hostInterface || "—", accent: false }
                ]

                delegate: RowLayout {
                    required property var modelData
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    spacing: Style.gap

                    Text {
                        text: modelData.label
                        color: theme.textMuted
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontSmall
                    }

                    Item { Layout.fillWidth: true }

                    Text {
                        readonly property bool canCopy: modelData.accent && modelData.value !== "—"
                        text: modelData.value
                        horizontalAlignment: Text.AlignRight
                        color: modelData.accent && modelData.value !== "—" ? theme.accentText : theme.textPrimary
                        font.family: Style.fontMono
                        font.pixelSize: Style.fontSmall
                        ToolTip.visible: canCopy && metricHover.hovered
                        ToolTip.delay: 500
                        ToolTip.text: qsTr("Click to copy this address")

                        HoverHandler {
                            id: metricHover
                            cursorShape: parent.canCopy ? Qt.PointingHandCursor : Qt.ArrowCursor
                        }

                        TapHandler {
                            enabled: parent.canCopy
                            onTapped: backend.copyText(root.ipAddress, qsTr("IP address"))
                        }
                    }
                }
            }
        }
    }

    // Row 3: Allowlist ACL editor
    RowLayout {
        visible: app.netAllowlistEditing || backend.selectedEnvironment.networkMode === "allowlist"
        Layout.fillWidth: true
        spacing: Style.gap

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
            disabled: backend.busy || app.netAllowlistDraft.trim().length === 0 || root.template
            onClicked: backend.setNetworkMode(backend.selectedEnvironment.name || "", "allowlist", app.netAllowlistDraft)
        }

        ActionButton {
            visible: backend.selectedEnvironment.networkMode === "allowlist"
            label: qsTr("Refresh IPs")
            disabled: backend.busy || root.template
            onClicked: backend.refreshNetworkAllowlist(backend.selectedEnvironment.name || "")
        }
    }
}
