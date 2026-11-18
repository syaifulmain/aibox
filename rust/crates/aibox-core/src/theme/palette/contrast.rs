use super::{MixRatios, Palette};
use crate::theme::color::{contrast_ratio, ensure_contrast, mix, Rgb};

/// Minimum WCAG contrast thresholds for palette derivation and validation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContrastThresholds {
    pub text_primary: f64,
    pub text: f64,
    pub border: f64,
    pub fill: f64,
    pub chart: f64,
}

impl ContrastThresholds {
    pub const fn for_options(high_contrast: bool) -> Self {
        Self {
            text_primary: 7.0,
            text: if high_contrast { 7.0 } else { 4.5 },
            border: if high_contrast { 3.0 } else { 1.8 },
            fill: 3.0,
            chart: 3.0,
        }
    }
}

pub(crate) struct StatusTokens {
    pub fill: Rgb,
    pub text: Rgb,
    pub surface: Rgb,
    pub border: Rgb,
    pub on_fill: Rgb,
}

pub(crate) fn best_on_color(surface: Rgb, bg: Rgb, fg: Rgb) -> Rgb {
    let bg_c = contrast_ratio(bg, surface);
    let fg_c = contrast_ratio(fg, surface);
    if bg_c >= 4.5 || fg_c >= 4.5 {
        return if bg_c >= fg_c { bg } else { fg };
    }
    let black_c = contrast_ratio(Rgb::BLACK, surface);
    let white_c = contrast_ratio(Rgb::WHITE, surface);
    let mut best = bg;
    let mut best_c = bg_c;
    for (cand, c) in [(fg, fg_c), (Rgb::BLACK, black_c), (Rgb::WHITE, white_c)] {
        if c > best_c {
            best = cand;
            best_c = c;
        }
    }
    best
}

pub(crate) fn derive_status(
    raw: Rgb,
    card: Rgb,
    bg: Rgb,
    fg: Rgb,
    is_dark: bool,
    ratios: &MixRatios,
    high_contrast: bool,
) -> StatusTokens {
    let thresholds = ContrastThresholds::for_options(high_contrast);
    let fill = ensure_contrast(raw, card, thresholds.fill);
    let surface_ratio = if is_dark {
        ratios.status_surface_dark
    } else {
        ratios.status_surface_light
    };
    let surface = mix(card, fill, surface_ratio);
    let text_on_card = ensure_contrast(raw, card, thresholds.text);
    let text = ensure_contrast(text_on_card, surface, thresholds.text);
    let border = ensure_contrast(
        mix(card, fill, ratios.status_border),
        card,
        thresholds.border,
    );
    let on_fill = best_on_color(fill, bg, fg);
    StatusTokens {
        fill,
        text,
        surface,
        border,
        on_fill,
    }
}

/// A single contrast requirement failure discovered by `validate_palette_contrast`.
#[derive(Debug, Clone, PartialEq)]
pub struct ContrastViolation {
    pub pair: &'static str,
    pub actual: f64,
    pub minimum: f64,
    pub fg: Rgb,
    pub bg: Rgb,
}

/// Validates all text/surface, status, border, and chart contrast pairs in `palette`.
pub fn validate_palette_contrast(palette: &Palette, high_contrast: bool) -> Vec<ContrastViolation> {
    let thresholds = ContrastThresholds::for_options(high_contrast);
    let card = palette.surface_card.rgb();
    let sidebar = palette.surface_sidebar.rgb();
    let mut violations = Vec::new();

    let mut check = |pair: &'static str, fg: Rgb, bg: Rgb, minimum: f64| {
        let actual = contrast_ratio(fg, bg);
        if actual < minimum {
            violations.push(ContrastViolation {
                pair,
                actual,
                minimum,
                fg,
                bg,
            });
        }
    };

    // Borders
    if high_contrast {
        check(
            "borderSubtle vs surfaceCard",
            palette.border_subtle.rgb(),
            card,
            thresholds.border,
        );
    }
    check(
        "borderStrong vs surfaceCard",
        palette.border_strong.rgb(),
        card,
        thresholds.border,
    );
    check(
        "borderFocus vs surfaceCard",
        palette.border_focus.rgb(),
        card,
        thresholds.fill,
    );

    // Primary, secondary, and muted text
    check(
        "textPrimary vs surfaceCard",
        palette.text_primary.rgb(),
        card,
        thresholds.text_primary,
    );
    check(
        "textPrimary vs surfaceCanvas",
        palette.text_primary.rgb(),
        palette.surface_canvas.rgb(),
        thresholds.text,
    );
    check(
        "textSecondary vs surfaceCard",
        palette.text_secondary.rgb(),
        card,
        thresholds.text,
    );
    check(
        "textMuted vs surfaceCard",
        palette.text_muted.rgb(),
        card,
        thresholds.text,
    );
    check(
        "textMuted vs surfaceSidebar",
        palette.text_muted.rgb(),
        sidebar,
        thresholds.text,
    );

    // Accent & statuses
    let statuses = [
        (
            "accent vs surfaceCard",
            "accentText vs surfaceCard",
            "accentText vs accentSurface",
            "accentBorder vs surfaceCard",
            "onAccent vs accent",
            palette.accent.rgb(),
            palette.accent_text.rgb(),
            palette.accent_surface.rgb(),
            palette.accent_border.rgb(),
            palette.on_accent.rgb(),
        ),
        (
            "danger vs surfaceCard",
            "dangerText vs surfaceCard",
            "dangerText vs dangerSurface",
            "dangerBorder vs surfaceCard",
            "onDanger vs danger",
            palette.danger.rgb(),
            palette.danger_text.rgb(),
            palette.danger_surface.rgb(),
            palette.danger_border.rgb(),
            palette.on_danger.rgb(),
        ),
        (
            "success vs surfaceCard",
            "successText vs surfaceCard",
            "successText vs successSurface",
            "successBorder vs surfaceCard",
            "onSuccess vs success",
            palette.success.rgb(),
            palette.success_text.rgb(),
            palette.success_surface.rgb(),
            palette.success_border.rgb(),
            palette.on_success.rgb(),
        ),
        (
            "warning vs surfaceCard",
            "warningText vs surfaceCard",
            "warningText vs warningSurface",
            "warningBorder vs surfaceCard",
            "onWarning vs warning",
            palette.warning.rgb(),
            palette.warning_text.rgb(),
            palette.warning_surface.rgb(),
            palette.warning_border.rgb(),
            palette.on_warning.rgb(),
        ),
        (
            "info vs surfaceCard",
            "infoText vs surfaceCard",
            "infoText vs infoSurface",
            "infoBorder vs surfaceCard",
            "onInfo vs info",
            palette.info.rgb(),
            palette.info_text.rgb(),
            palette.info_surface.rgb(),
            palette.info_border.rgb(),
            palette.on_info.rgb(),
        ),
    ];

    for (fill_lbl, text_lbl, surf_lbl, border_lbl, on_lbl, fill, text, surf, border, on_fill) in
        statuses
    {
        check(fill_lbl, fill, card, thresholds.fill);
        check(text_lbl, text, card, thresholds.text);
        check(surf_lbl, text, surf, thresholds.text);
        check(border_lbl, border, card, thresholds.border);
        check(on_lbl, on_fill, fill, 4.5);
    }

    // Charts
    check(
        "chart1 vs surfaceCard",
        palette.chart_1.rgb(),
        card,
        thresholds.chart,
    );
    check(
        "chart2 vs surfaceCard",
        palette.chart_2.rgb(),
        card,
        thresholds.chart,
    );
    check(
        "chart3 vs surfaceCard",
        palette.chart_3.rgb(),
        card,
        thresholds.chart,
    );
    check(
        "chart4 vs surfaceCard",
        palette.chart_4.rgb(),
        card,
        thresholds.chart,
    );

    violations
}
