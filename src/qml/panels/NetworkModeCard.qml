import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import "../components"

// Network mode, address and per-environment network actions.
Card {
    id: root
    property QtObject app: null

    readonly property var env: backend.selectedEnvironment
    readonly property string ipAddress: root.env.ipv4 || ""
    readonly property bool offline: root.env.networkMode === "offline"
    readonly property bool template: root.env.isTemplate || false

    title: qsTr("Network")
    borderColor: root.offline
                 ? theme.warningBorder
                 : (root.env.networkMode === "allowlist" ? theme.accentBorder : theme.borderSubtle)

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
                        readonly property string activeMode: root.env.networkMode || "online"
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
                            const name = root.env.name || ""
                            if (modelData.id === "allowlist") {
                                app.netAllowlistEditing = true
                                if (app.netAllowlistDraft.length === 0)
                                    app.netAllowlistDraft = root.env.networkAllowlist || ""
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
                    checked: root.env.egressRecording || false
                    disabled: backend.busy || root.template || root.offline
                    onToggled: backend.setEgressRecording(
                        root.env.name || "",
                        !(root.env.egressRecording || false))
                }

                ActionButton {
                    label: (root.env.portForwardCount || 0) > 0
                           ? qsTr("Ports (%1)").arg(root.env.portForwardCount)
                           : qsTr("Ports")
                    disabled: root.template
                    onClicked: app.openPortsDialog()
                }

                ActionButton {
                    label: qsTr("Egress Log")
                    disabled: root.template
                    onClicked: app.openEgressDialog(root.env.name || "")
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
                    { label: qsTr("Receiving"), value: root.env.netRxRateText || "—", accent: false },
                    { label: qsTr("Sending"), value: root.env.netTxRateText || "—", accent: false },
                    { label: qsTr("Downloaded"), value: root.env.netRxTotalText || "—", accent: false },
                    { label: qsTr("Uploaded"), value: root.env.netTxTotalText || "—", accent: false },
                    { label: qsTr("IP Address"), value: (root.ipAddress.length > 0 && !root.offline) ? root.ipAddress : "—", accent: true },
                    { label: qsTr("Interface"), value: root.env.hostInterface || "—", accent: false }
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
        visible: app.netAllowlistEditing || root.env.networkMode === "allowlist"
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
                    backend.setNetworkMode(root.env.name || "", "allowlist", app.netAllowlistDraft)
            }
        }

        ActionButton {
            label: qsTr("Apply ACL")
            variant: "primary"
            disabled: backend.busy || app.netAllowlistDraft.trim().length === 0 || root.template
            onClicked: backend.setNetworkMode(root.env.name || "", "allowlist", app.netAllowlistDraft)
        }

        ActionButton {
            visible: root.env.networkMode === "allowlist"
            label: qsTr("Refresh IPs")
            disabled: backend.busy || root.template
            onClicked: backend.refreshNetworkAllowlist(root.env.name || "")
        }
    }
}
