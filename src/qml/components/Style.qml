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

    // Semantic scale (Phase 1+)
    readonly property int fontCaption: 10
    readonly property int fontSmall: 11
    readonly property int fontBody: 12
    readonly property int fontTitle: 14
    readonly property int fontHeading: 16
    readonly property int fontDisplay: 18

    // Numeric scale
    readonly property int font8: 8 // deprecated, remove in phase 7
    readonly property int font9: 9 // deprecated, remove in phase 7
    readonly property int font10: 10 // deprecated, remove in phase 7
    readonly property int font11: 11 // deprecated, remove in phase 7
    readonly property int font12: 12 // deprecated, remove in phase 7
    readonly property int font13: 13 // deprecated, remove in phase 7
    readonly property int font14: 14 // deprecated, remove in phase 7
    readonly property int font15: 15 // deprecated, remove in phase 7
    readonly property int font16: 16 // deprecated, remove in phase 7
    readonly property int font18: 18 // deprecated, remove in phase 7

    // ------------------------------------------------------------------
    // Spacing scale
    // ------------------------------------------------------------------
    // Semantic scale (Phase 1+)
    readonly property int gapXs: 2
    readonly property int gapSm: 4
    readonly property int gapMd: 8
    readonly property int gap: 10
    readonly property int padCard: 12
    readonly property int padSection: 16
    readonly property int padModal: 24

    // Numeric scale
    readonly property int space1: 1 // deprecated, remove in phase 7
    readonly property int space2: 2 // deprecated, remove in phase 7
    readonly property int space3: 3 // deprecated, remove in phase 7
    readonly property int space4: 4 // deprecated, remove in phase 7
    readonly property int space5: 5 // deprecated, remove in phase 7
    readonly property int space6: 6 // deprecated, remove in phase 7
    readonly property int space7: 7 // deprecated, remove in phase 7
    readonly property int space8: 8 // deprecated, remove in phase 7
    readonly property int space10: 10 // deprecated, remove in phase 7
    readonly property int space12: 12 // deprecated, remove in phase 7
    readonly property int space14: 14 // deprecated, remove in phase 7
    readonly property int space16: 16 // deprecated, remove in phase 7
    readonly property int space18: 18 // deprecated, remove in phase 7
    readonly property int space20: 20 // deprecated, remove in phase 7
    readonly property int space22: 22 // deprecated, remove in phase 7
    readonly property int space24: 24 // deprecated, remove in phase 7
    readonly property int space30: 30 // deprecated, remove in phase 7

    // ------------------------------------------------------------------
    // Corner radii (the live Hyprland rounding caps each value)
    // ------------------------------------------------------------------
    // Semantic scale (Phase 1+)
    readonly property int radiusSm: 3
    readonly property int radiusMd: 4
    readonly property int radiusLg: 8

    // Numeric scale
    readonly property int radius2: 2 // deprecated, remove in phase 7
    readonly property int radius3: 3 // deprecated, remove in phase 7
    readonly property int radius4: 4 // deprecated, remove in phase 7
    readonly property int radius5: 5 // deprecated, remove in phase 7
    readonly property int radius6: 6 // deprecated, remove in phase 7
    readonly property int radius8: 8 // deprecated, remove in phase 7

    // ------------------------------------------------------------------
    // Control and badge metrics
    // ------------------------------------------------------------------
    readonly property int controlHeight: 34
    readonly property int controlHeightCompact: 28
    readonly property int controlHeightSm: 28 // deprecated, remove in phase 7
    readonly property int controlHeightXs: 26 // deprecated, remove in phase 7
    readonly property int controlHeightXxs: 24 // deprecated, remove in phase 7

    readonly property int headerHeight: 54
    readonly property int menuRowHeight: 50 // deprecated, remove in phase 7
    readonly property int listRowHeight: 68
    readonly property int rowHeight: 36
    readonly property int iconTile: 36 // target 34 after phase 2-6 migration
    readonly property int badgeHeight: 20
    readonly property int badgeHeightMd: 20 // deprecated, remove in phase 7
    readonly property int badgeHeightLg: 22 // deprecated, remove in phase 7
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
    readonly property int modalGutterWide: 60 // deprecated, remove in phase 7
    readonly property int modalEdge: 56 // deprecated, remove in phase 7
    readonly property int modalEdgeTight: 40 // deprecated, remove in phase 7
    readonly property int modalPadding: 24
    readonly property int modalPaddingTight: 22 // deprecated, remove in phase 7
    readonly property int modalWidthSm: 520
    readonly property int modalWidthMd: 720
    readonly property int modalWidthLg: 960

    // ------------------------------------------------------------------
    // Card and content insets
    // ------------------------------------------------------------------
    readonly property int cardPadding: 14 // deprecated, remove in phase 7
    readonly property int cardPaddingSm: 12 // deprecated, remove in phase 7
    readonly property int cardPaddingXs: 10 // deprecated, remove in phase 7
}
