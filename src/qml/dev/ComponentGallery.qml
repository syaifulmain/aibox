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

    property string galleryPage: "components"

    ScrollView {
        anchors.fill: parent
        contentWidth: availableWidth
        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
        ScrollBar.vertical.policy: ScrollBar.AlwaysOff

        ColumnLayout {
            width: galleryWin.width - Style.padModal * 2
            x: Style.padModal
            y: Style.padSection
            spacing: Style.padSection

            // Gallery header with page switcher
            RowLayout {
                Layout.fillWidth: true
                spacing: Style.gap

                BrandMark { }
                BrandMark { quiet: true }
                SlidersIcon { color: theme.textPrimary }

                SegmentedTabs {
                    currentValue: galleryWin.galleryPage
                    mediumWhenSelected: true
                    tabs: [
                        { label: "Components", key: "1", value: "components" },
                        { label: "Token Scale", key: "2", value: "tokens" }
                    ]
                    onSelected: function(val) {
                        galleryWin.galleryPage = val
                    }
                }

                Item { Layout.fillWidth: true }

                TagBadge { text: theme.isDark ? "DARK" : "LIGHT"; tone: "accent" }
            }

            // Page 1: Reusable components
            ColumnLayout {
                Layout.fillWidth: true
                visible: galleryWin.galleryPage === "components"
                spacing: Style.padSection

                // Header row: SegmentedTabs preview
                RowLayout {
                    Layout.fillWidth: true
                    spacing: Style.padCard

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
                }

            // ActionButton states
            RowLayout {
                Layout.fillWidth: true
                spacing: Style.gap

                ActionButton { label: "Default" }
                ActionButton { label: "Primary"; variant: "primary" }
                ActionButton { label: "Accent"; variant: "accent" }
                ActionButton { label: "Danger"; variant: "danger" }
                ActionButton { label: "Disabled"; disabled: true }
                ActionButton { label: "Compact"; compact: true }
                ActionButton { label: "Config"; slidersIcon: true }
                ActionButton { label: "?" }
                ActionButton { slidersIcon: true }
            }

            // ToggleChip states
            RowLayout {
                Layout.fillWidth: true
                spacing: Style.gap

                ToggleChip { label: "Auto-snapshot"; checked: false }
                ToggleChip { label: "Record"; checked: true }
                ToggleChip { label: "Include Workspace"; checked: true; compact: true }
                ToggleChip { label: "Disabled"; checked: false; disabled: true }
            }

            // TagBadge tones
            RowLayout {
                Layout.fillWidth: true
                spacing: Style.gapMd

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
                spacing: Style.gap

                SelectChip { text: "2 vCPU"; selected: false }
                SelectChip { text: "4 vCPU"; selected: true; mediumWhenSelected: true }
                SelectChip { text: "8 GB"; selected: false; compact: true }
                SelectChip { text: "Offline"; selected: false; disabled: true }

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
                spacing: Style.padCard

                FlatInput {
                    Layout.fillWidth: true
                    placeholder: "Environment name..."
                }

                FlatInput {
                    Layout.fillWidth: true
                    text: "dev-sandbox"
                }

                FlatInput {
                    Layout.fillWidth: true
                    text: "disabled-env"
                    disabled: true
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
                columnSpacing: Style.padCard
                rowSpacing: Style.gapMd

                CheckRow { text: "Unchecked option"; checked: false }
                CheckRow { text: "Checked option"; checked: true }
                CheckRow { text: "Danger confirmation"; checked: true; danger: true }
                CheckRow { text: "Disabled option"; checked: false; disabled: true }
                MenuRow { label: "Normal action"; detail: "Secondary description" }
                MenuRow { label: "Danger action"; detail: "Irreversible operation"; danger: true }
                MenuRow { label: "Disabled action"; detail: "Currently unavailable"; disabled: true }
            }

            // ResourceMeterCard & MeterBar
            RowLayout {
                Layout.fillWidth: true
                spacing: Style.padCard

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

            // Layout primitives: SectionHeader, Card, FormRow, KeyValue, DialogFooter
            Card {
                title: "Card Primitive"
                subtitle: "Standard surfaceCard container with right-aligned actions"
                actions: [
                    ActionButton { label: "Action"; compact: true }
                ]

                SectionHeader {
                    title: "Section Header"
                    subtitle: "Optional subtitle aligned to 34px control height"
                    ActionButton { label: "Add"; variant: "accent" }
                }

                FormRow {
                    label: "Project Name"
                    hint: "~/aibox/workspaces/dev-sandbox"
                    FlatInput {
                        Layout.fillWidth: true
                        text: "dev-sandbox"
                    }
                }

                KeyValue {
                    label: "Shared Workspace"
                    value: "~/aibox/workspaces/dev-sandbox ↔ /workspace"
                }

                DialogFooter {
                    leftActions: [
                        ActionButton { label: "Reset"; variant: "danger" }
                    ]
                    ActionButton { label: "Secondary" }
                    ActionButton { label: "Save Changes"; variant: "primary" }
                }
            }

            // ModalHeader preview inside a card
            Rectangle {
                Layout.fillWidth: true
                implicitHeight: headerPreview.implicitHeight + Style.gap * 2
                radius: Math.min(Style.radiusLg, theme.radius)
                color: theme.surfaceOverlay
                border.width: Style.hairline
                border.color: theme.borderSubtle

                ModalHeader {
                    id: headerPreview
                    anchors.fill: parent
                    anchors.margins: Style.gap
                    title: "Modal Header Preview"
                    subtitle: "Semantic surfaceOverlay + borderSubtle"
                }
            }
            }

            // Page 2: Semantic token scale (typography, spacing, radius)
            ColumnLayout {
                Layout.fillWidth: true
                visible: galleryWin.galleryPage === "tokens"
                spacing: Style.padSection

                // Typography scale
                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: typeCol.implicitHeight + Style.padCard * 2
                    radius: Math.min(Style.radiusMd, theme.radius)
                    color: theme.surfaceCard
                    border.width: Style.hairline
                    border.color: theme.borderSubtle

                    ColumnLayout {
                        id: typeCol
                        anchors.fill: parent
                        anchors.margins: Style.padCard
                        spacing: Style.gapMd

                        Text {
                            text: "Typography Scale"
                            color: theme.textPrimary
                            font.family: Style.fontSans
                            font.pixelSize: Style.fontHeading
                            font.weight: Font.Medium
                        }

                        Repeater {
                            model: [
                                { name: "fontDisplay (18px)", size: Style.fontDisplay, mono: false, weight: Font.Medium, sample: "dev-sandbox" },
                                { name: "fontHeading (16px)", size: Style.fontHeading, mono: false, weight: Font.Medium, sample: "Modal & Panel Heading" },
                                { name: "fontTitle (14px)", size: Style.fontTitle, mono: false, weight: Font.Medium, sample: "Card & Section Title" },
                                { name: "fontBody (12px)", size: Style.fontBody, mono: false, weight: Font.Normal, sample: "Default UI body, button labels, and input text" },
                                { name: "fontSmall (11px)", size: Style.fontSmall, mono: false, weight: Font.Normal, sample: "Field labels, subtitles, and secondary descriptions" },
                                { name: "fontCaption (10px)", size: Style.fontCaption, mono: true, weight: Font.Normal, sample: "10.143.44.12  |  18.8 / 20.0 GB  |  ~/aibox/workspaces" }
                            ]

                            RowLayout {
                                required property var modelData
                                Layout.fillWidth: true
                                spacing: Style.gap

                                Text {
                                    Layout.preferredWidth: Style.buttonMinWidth * 2
                                    text: modelData.name
                                    color: theme.textMuted
                                    font.family: Style.fontMono
                                    font.pixelSize: Style.fontSmall
                                }

                                Text {
                                    Layout.fillWidth: true
                                    text: modelData.sample
                                    color: theme.textPrimary
                                    font.family: modelData.mono ? Style.fontMono : Style.fontSans
                                    font.pixelSize: modelData.size
                                    font.weight: modelData.weight
                                    elide: Text.ElideRight
                                }
                            }
                        }
                    }
                }

                // Spacing scale
                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: spaceCol.implicitHeight + Style.padCard * 2
                    radius: Math.min(Style.radiusMd, theme.radius)
                    color: theme.surfaceCard
                    border.width: Style.hairline
                    border.color: theme.borderSubtle

                    ColumnLayout {
                        id: spaceCol
                        anchors.fill: parent
                        anchors.margins: Style.padCard
                        spacing: Style.gapMd

                        Text {
                            text: "Spacing Scale"
                            color: theme.textPrimary
                            font.family: Style.fontSans
                            font.pixelSize: Style.fontHeading
                            font.weight: Font.Medium
                        }

                        Repeater {
                            model: [
                                { name: "gapXs (2px)", px: Style.gapXs },
                                { name: "gapSm (4px)", px: Style.gapSm },
                                { name: "gapMd (8px)", px: Style.gapMd },
                                { name: "gap (10px)", px: Style.gap },
                                { name: "padCard (12px)", px: Style.padCard },
                                { name: "padSection (16px)", px: Style.padSection },
                                { name: "padModal (24px)", px: Style.padModal }
                            ]

                            RowLayout {
                                required property var modelData
                                Layout.fillWidth: true
                                spacing: Style.gap

                                Text {
                                    Layout.preferredWidth: Style.buttonMinWidth * 2
                                    text: modelData.name
                                    color: theme.textMuted
                                    font.family: Style.fontMono
                                    font.pixelSize: Style.fontSmall
                                }

                                Rectangle {
                                    implicitWidth: modelData.px
                                    implicitHeight: Style.badgeHeight
                                    radius: Math.min(Style.radiusSm, theme.radius)
                                    color: theme.accent
                                }
                            }
                        }
                    }
                }

                // Radius scale
                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: radiusCol.implicitHeight + Style.padCard * 2
                    radius: Math.min(Style.radiusMd, theme.radius)
                    color: theme.surfaceCard
                    border.width: Style.hairline
                    border.color: theme.borderSubtle

                    ColumnLayout {
                        id: radiusCol
                        anchors.fill: parent
                        anchors.margins: Style.padCard
                        spacing: Style.gapMd

                        Text {
                            text: "Radius Scale"
                            color: theme.textPrimary
                            font.family: Style.fontSans
                            font.pixelSize: Style.fontHeading
                            font.weight: Font.Medium
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Style.padSection

                            Repeater {
                                model: [
                                    { name: "radiusSm (3px)", r: Style.radiusSm },
                                    { name: "radiusMd (4px)", r: Style.radiusMd },
                                    { name: "radiusLg (8px)", r: Style.radiusLg }
                                ]

                                Rectangle {
                                    required property var modelData
                                    Layout.fillWidth: true
                                    implicitHeight: Style.listRowHeight
                                    radius: Math.min(modelData.r, theme.radius)
                                    color: theme.surfaceInput
                                    border.width: Style.hairline
                                    border.color: theme.borderStrong

                                    Text {
                                        anchors.centerIn: parent
                                        text: modelData.name
                                        color: theme.textPrimary
                                        font.family: Style.fontMono
                                        font.pixelSize: Style.fontSmall
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
