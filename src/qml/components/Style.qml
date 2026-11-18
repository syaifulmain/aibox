pragma Singleton

import QtQuick

// Design tokens for the whole interface.
//
// Live Omarchy colours (accent, foreground, red, green, yellow, radius) stay in
// the C++ `theme` object. This singleton holds every value the application
// itself owns: surface colours, spacing, control sizes, type scale and modal
// metrics. Change a value here once and every panel, dialog and control follows.
QtObject {
    // ------------------------------------------------------------------
    // Typography (pixel sizes follow the existing scale)
    // ------------------------------------------------------------------
    readonly property string fontSans: "Adwaita Sans"
    readonly property string fontMono: "JetBrainsMono Nerd Font"

    readonly property int font8: 8
    readonly property int font9: 9
    readonly property int font10: 10
    readonly property int font11: 11
    readonly property int font12: 12
    readonly property int font13: 13
    readonly property int font14: 14
    readonly property int font15: 15
    readonly property int font16: 16
    readonly property int font18: 18

    // ------------------------------------------------------------------
    // Spacing scale
    // ------------------------------------------------------------------
    readonly property int space1: 1
    readonly property int space2: 2
    readonly property int space3: 3
    readonly property int space4: 4
    readonly property int space5: 5
    readonly property int space6: 6
    readonly property int space7: 7
    readonly property int space8: 8
    readonly property int space10: 10
    readonly property int space12: 12
    readonly property int space14: 14
    readonly property int space16: 16
    readonly property int space18: 18
    readonly property int space20: 20
    readonly property int space22: 22
    readonly property int space24: 24
    readonly property int space30: 30

    // ------------------------------------------------------------------
    // Corner radii (the live Hyprland rounding caps each value)
    // ------------------------------------------------------------------
    readonly property int radius2: 2
    readonly property int radius3: 3
    readonly property int radius4: 4
    readonly property int radius5: 5
    readonly property int radius6: 6
    readonly property int radius8: 8

    // ------------------------------------------------------------------
    // Control and badge metrics
    // ------------------------------------------------------------------
    readonly property int controlHeight: 34
    readonly property int controlHeightSm: 28
    readonly property int controlHeightXs: 26
    readonly property int controlHeightXxs: 24

    readonly property int headerHeight: 54
    readonly property int menuRowHeight: 50
    readonly property int listRowHeight: 68
    readonly property int rowHeight: 36
    readonly property int iconTile: 36
    readonly property int badgeHeight: 18
    readonly property int badgeHeightMd: 20
    readonly property int badgeHeightLg: 22
    readonly property int hairline: 1

    // ------------------------------------------------------------------
    // Surfaces and borders (aliased to `theme` until M4–M6 migration)
    // ------------------------------------------------------------------
    readonly property color surfaceCard: theme.surfaceCard
    readonly property color surfaceInput: theme.surfaceInput
    readonly property color surfaceSelected: theme.surfaceSelected
    readonly property color surfaceSidebar: theme.surfaceSidebar
    readonly property color surfaceTrack: theme.surfaceTrack
    readonly property color borderSubtle: theme.borderSubtle

    // Backdrop behind every modal dialog
    readonly property color scrim: theme.scrim
    readonly property color scrimStrong: theme.scrimStrong

    // ------------------------------------------------------------------
    // Feedback surfaces (aliased to `theme` until M4–M6 migration)
    // ------------------------------------------------------------------
    readonly property color dangerSurface: theme.dangerSurface
    readonly property color dangerSurfaceAlt: theme.dangerSurface
    readonly property color dangerSurfaceDeep: theme.dangerSurface
    readonly property color dangerTint: theme.dangerSurface
    readonly property color dangerTintMuted: theme.dangerSurface

    readonly property color successSurface: theme.successSurface
    readonly property color successSurfaceTint: theme.successSurface
    readonly property color successBorder: theme.successBorder

    readonly property color warning: theme.warning
    readonly property color warningSurface: theme.warningSurface
    readonly property color warningSurfaceDeep: theme.warningSurface
    readonly property color warningSurfaceAlt: theme.warningSurface

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
    readonly property int modalGutterWide: 60
    readonly property int modalEdge: 56
    readonly property int modalEdgeTight: 40
    readonly property int modalPadding: 24
    readonly property int modalPaddingTight: 22

    // ------------------------------------------------------------------
    // Card and content insets
    // ------------------------------------------------------------------
    readonly property int cardPadding: 14
    readonly property int cardPaddingSm: 12
    readonly property int cardPaddingXs: 10
}
