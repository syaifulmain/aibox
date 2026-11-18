pragma Singleton

import QtQuick

// Design metrics for the whole interface.
//
// All live colours (surfaces, borders, text, accent, status tones, scrims)
// live in the `theme` QObject (`ThemeRust`). This singleton holds non-colour
// design tokens: typography, spacing, control sizes, radii and modal metrics.
// Change a value here once and every panel, dialog and control follows.
QtObject {
    // ------------------------------------------------------------------
    // Typography
    // ------------------------------------------------------------------
    readonly property string fontSans: "Adwaita Sans"
    readonly property string fontMono: "JetBrainsMono Nerd Font"

    readonly property int fontCaption: 10
    readonly property int fontSmall: 11
    readonly property int fontBody: 12
    readonly property int fontTitle: 14
    readonly property int fontHeading: 16
    readonly property int fontDisplay: 18

    // ------------------------------------------------------------------
    // Spacing scale
    // ------------------------------------------------------------------
    readonly property int gapXs: 2
    readonly property int gapSm: 4
    readonly property int gapMd: 8
    readonly property int gap: 10
    readonly property int padCard: 12
    readonly property int padSection: 16
    readonly property int padModal: 24

    // ------------------------------------------------------------------
    // Corner radii (the live Hyprland rounding caps each value)
    // ------------------------------------------------------------------
    readonly property int radiusSm: 3
    readonly property int radiusMd: 4
    readonly property int radiusLg: 8

    // ------------------------------------------------------------------
    // Control and badge metrics
    // ------------------------------------------------------------------
    readonly property int controlHeight: 34
    readonly property int controlHeightCompact: 28

    readonly property int headerHeight: 54
    readonly property int listRowHeight: 68
    readonly property int rowHeight: 36
    readonly property int iconTile: 36
    readonly property int badgeHeight: 20
    readonly property int buttonMinWidth: 88
    readonly property int hairline: 1

    // ------------------------------------------------------------------
    // Interaction feedback
    // ------------------------------------------------------------------
    readonly property real opacityDisabled: 0.4
    readonly property real opacityHover: 0.9
    readonly property real opacityDeemphasized: 0.72
    readonly property real opacityInactive: 0.65

    // ------------------------------------------------------------------
    // Modal metrics
    // ------------------------------------------------------------------
    readonly property int modalGutter: 56
    readonly property int modalPadding: 24
    readonly property int modalWidthSm: 520
    readonly property int modalWidthMd: 720
    readonly property int modalWidthLg: 960

    // ------------------------------------------------------------------
    // Window defaults
    // ------------------------------------------------------------------
    readonly property int windowWidth: 1080
    readonly property int windowHeight: 720
    readonly property int windowMinWidth: 960
    readonly property int windowMinHeight: 560
}
