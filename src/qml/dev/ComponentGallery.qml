import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import "../components"

// Development-only gallery displaying every reusable component and its states.
// Loaded only when AIBOX_GALLERY=1 (used for visual theme matrix checks).
Window {
    id: galleryWin
    width: 1080
    height: 780
    visible: true
    title: "AIBox Component Gallery"
    color: theme.surfaceCanvas
    Material.theme: theme.isDark ? Material.Dark : Material.Light
    Material.accent: theme.accent

    ScrollView {
        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            width: galleryWin.width - Style.space30 * 2
            x: Style.space30
            y: Style.space20
            spacing: Style.space16

            // Header row: BrandMark, SlidersIcon, SegmentedTabs
            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space12

                BrandMark { }
                BrandMark { quiet: true }
                SlidersIcon { color: theme.textPrimary }

                SegmentedTabs {
                    currentValue: "envs"
                    mediumWhenSelected: true
                    tabs: [
                        { label: "Environments", key: "1", value: "envs" },
                        { label: "OS Catalog", key: "2", value: "catalog" },
                        { label: "Settings", key: "3", value: "settings" }
                    ]
                }

                Item { Layout.fillWidth: true }

                TagBadge { text: theme.isDark ? "DARK" : "LIGHT"; tone: "accent" }
            }

            // ActionButton states
            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space10

                ActionButton { label: "Default" }
                ActionButton { label: "Primary"; variant: "primary" }
                ActionButton { label: "Accent"; variant: "accent" }
                ActionButton { label: "Success"; variant: "success" }
                ActionButton { label: "Danger"; variant: "danger" }
                ActionButton { label: "Disabled"; disabled: true }
                ActionButton { label: "Config"; slidersIcon: true }
            }

            // TagBadge tones
            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space8

                TagBadge { text: "NEUTRAL"; tone: "neutral" }
                TagBadge { text: "QUIET"; tone: "quiet" }
                TagBadge { text: "ACCENT"; tone: "accent" }
                TagBadge { text: "ACCENT QUIET"; tone: "accentQuiet" }
                TagBadge { text: "RUNNING"; tone: "success"; dotVisible: true }
                TagBadge { text: "WARNING"; tone: "warning"; dotVisible: true }
                TagBadge { text: "STOPPED"; tone: "danger"; dotVisible: true }
            }

            // SelectChip & SelectCard
            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space10

                SelectChip { text: "2 vCPU"; selected: false }
                SelectChip { text: "4 vCPU"; selected: true; mediumWhenSelected: true }
                SelectChip { text: "8 GB"; selected: false }

                SelectCard {
                    Layout.fillWidth: true
                    title: "Container"
                    subtitle: "Fast shared-kernel isolation"
                    selected: true
                    mediumWhenSelected: true
                }

                SelectCard {
                    Layout.fillWidth: true
                    title: "Virtual Machine"
                    subtitle: "Full KVM kernel isolation"
                    selected: false
                }
            }

            // Inputs: FlatInput & ChipInput
            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space12

                FlatInput {
                    Layout.fillWidth: true
                    placeholder: "Environment name..."
                }

                FlatInput {
                    Layout.fillWidth: true
                    text: "dev-sandbox"
                }

                ChipInput {
                    Layout.fillWidth: true
                    values: ["2", "4", "8"]
                    unit: "GB"
                    placeholder: "Add..."
                }
            }

            // CheckRow & MenuRow
            GridLayout {
                Layout.fillWidth: true
                columns: 2
                columnSpacing: Style.space12
                rowSpacing: Style.space8

                CheckRow { text: "Unchecked option"; checked: false }
                CheckRow { text: "Checked option"; checked: true }
                CheckRow { text: "Danger confirmation"; checked: true; danger: true }
                MenuRow { label: "Normal action"; detail: "Secondary description" }
                MenuRow { label: "Danger action"; detail: "Irreversible operation"; danger: true }
                MenuRow { label: "Disabled action"; detail: "Currently unavailable"; disabled: true }
            }

            // ResourceMeterCard & MeterBar
            RowLayout {
                Layout.fillWidth: true
                spacing: Style.space12

                ResourceMeterCard {
                    Layout.fillWidth: true
                    title: "CPU"
                    value: "2 / 4 cores"
                    badge: "32%"
                    percent: 32
                    hasBar: true
                    active: true
                }

                ResourceMeterCard {
                    Layout.fillWidth: true
                    title: "Memory"
                    value: "6.2 / 8.0 GB"
                    badge: "78%"
                    percent: 78
                    hasBar: true
                    active: true
                    alertLevel: 1
                }

                ResourceMeterCard {
                    Layout.fillWidth: true
                    title: "Disk"
                    value: "18.8 / 20.0 GB"
                    badge: "94%"
                    percent: 94
                    hasBar: true
                    active: true
                    alertLevel: 2
                }
            }

            // ModalHeader preview inside a card
            Rectangle {
                Layout.fillWidth: true
                implicitHeight: headerPreview.implicitHeight + Style.space20
                radius: Math.min(Style.radius8, theme.radius)
                color: theme.surfaceOverlay
                border.width: Style.hairline
                border.color: theme.borderSubtle

                ModalHeader {
                    id: headerPreview
                    anchors.fill: parent
                    anchors.margins: Style.space10
                    title: "Modal Header Preview"
                    subtitle: "Semantic surfaceOverlay + borderSubtle"
                }
            }
        }
    }
}
