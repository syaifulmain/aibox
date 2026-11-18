use super::contrast::{derive_status, ContrastThresholds};
use super::{MixRatios, ModeOverride, Palette, PaletteOptions};
use crate::theme::color::{ensure_contrast, mix, Rgb, Rgba};
use crate::theme::parse::ThemeColors;

fn parse_rgb_or(hex: &str, fallback: Rgb) -> Rgb {
    Rgb::from_hex(hex).unwrap_or(fallback)
}

/// Derives a complete contrast-guarded `Palette` from `ThemeColors` without caching.
pub fn derive_palette_uncached(raw: &ThemeColors, opts: PaletteOptions) -> Palette {
    let ratios = MixRatios::DEFAULT;
    let thresholds = ContrastThresholds::for_options(opts.high_contrast);
    let is_dark = match opts.mode_override {
        ModeOverride::Auto => raw.is_dark,
        ModeOverride::Light => false,
        ModeOverride::Dark => true,
    };
    let polarity_flipped = is_dark != raw.is_dark;
    let fallback = if is_dark {
        ThemeColors::default()
    } else {
        ThemeColors::default_light()
    };

    let bg_raw = parse_rgb_or(
        &raw.background,
        Rgb::from_hex(&fallback.background).unwrap_or(Rgb::BLACK),
    );
    let dark_bg_raw = parse_rgb_or(
        &raw.dark_background,
        Rgb::from_hex(&fallback.dark_background).unwrap_or(bg_raw),
    );
    let darker_bg_raw = parse_rgb_or(
        &raw.darker_background,
        Rgb::from_hex(&fallback.darker_background).unwrap_or(dark_bg_raw),
    );
    let fg_raw = parse_rgb_or(
        &raw.bright_foreground,
        Rgb::from_hex(&fallback.bright_foreground).unwrap_or(Rgb::WHITE),
    );
    let fg_secondary_raw = parse_rgb_or(&raw.base_foreground, fg_raw);
    let fg_muted_raw = parse_rgb_or(&raw.dark_foreground, fg_secondary_raw);

    let accent_raw = parse_rgb_or(
        &raw.accent,
        Rgb::from_hex(&fallback.accent).unwrap_or(Rgb::WHITE),
    );
    let red_raw = parse_rgb_or(&raw.red, Rgb::from_hex(&fallback.red).unwrap_or(accent_raw));
    let green_raw = parse_rgb_or(
        &raw.green,
        Rgb::from_hex(&fallback.green).unwrap_or(accent_raw),
    );
    let yellow_raw = parse_rgb_or(
        &raw.yellow,
        Rgb::from_hex(&fallback.yellow).unwrap_or(accent_raw),
    );
    let orange_raw = parse_rgb_or(
        &raw.orange,
        Rgb::from_hex(&fallback.orange).unwrap_or(accent_raw),
    );
    let magenta_raw = parse_rgb_or(
        &raw.magenta,
        Rgb::from_hex(&fallback.magenta).unwrap_or(accent_raw),
    );
    let cyan_raw = parse_rgb_or(
        &raw.cyan,
        Rgb::from_hex(&fallback.cyan).unwrap_or(accent_raw),
    );

    // When the user forces the opposite polarity, derive surfaces from the opposite pole
    // while keeping the theme's foreground tint, accent, and status hues.
    let (bg, dark_bg, darker_bg) = if polarity_flipped {
        if is_dark {
            let b = mix(fg_raw, Rgb::BLACK, 0.90);
            let d = mix(b, Rgb::WHITE, 0.04);
            let dd = mix(b, Rgb::BLACK, 0.25);
            (b, d, dd)
        } else {
            let b = mix(fg_raw, Rgb::WHITE, 0.90);
            let d = mix(b, fg_raw, 0.05);
            let dd = mix(b, fg_raw, 0.10);
            (b, d, dd)
        }
    } else {
        (bg_raw, dark_bg_raw, darker_bg_raw)
    };

    let selection_raw = if polarity_flipped {
        mix(dark_bg, accent_raw, if is_dark { 0.22 } else { 0.18 })
    } else {
        parse_rgb_or(&raw.selection, dark_bg)
    };

    // Surfaces
    let surface_canvas = bg;
    let card = dark_bg;
    let fg_pole = if polarity_flipped {
        ensure_contrast(fg_raw, card, thresholds.text_primary)
    } else {
        fg_raw
    };
    let surface_sidebar = mix(
        dark_bg,
        darker_bg,
        if is_dark {
            ratios.sidebar_dark
        } else {
            ratios.sidebar_light
        },
    );
    let surface_input = if is_dark {
        darker_bg
    } else {
        mix(bg, Rgb::WHITE, ratios.input_light)
    };
    let surface_raised = mix(
        card,
        fg_pole,
        if is_dark {
            ratios.raised_dark
        } else {
            ratios.raised_light
        },
    );
    let surface_selected = mix(
        card,
        accent_raw,
        if is_dark {
            ratios.selected_dark
        } else {
            ratios.selected_light
        },
    );
    let surface_track = mix(
        card,
        fg_pole,
        if is_dark {
            ratios.track_dark
        } else {
            ratios.track_light
        },
    );
    let surface_overlay = if is_dark { card } else { bg };

    // Text
    let text_primary = ensure_contrast(fg_raw, card, thresholds.text_primary);
    let text_secondary = ensure_contrast(fg_secondary_raw, card, thresholds.text);
    let text_muted = ensure_contrast(
        ensure_contrast(fg_muted_raw, card, thresholds.text),
        surface_sidebar,
        thresholds.text,
    );
    let text_disabled = mix(text_muted, card, ratios.text_disabled);

    // Accent & Status
    let accent = derive_status(
        accent_raw,
        card,
        bg,
        fg_pole,
        is_dark,
        &ratios,
        opts.high_contrast,
    );
    let danger = derive_status(
        red_raw,
        card,
        bg,
        fg_pole,
        is_dark,
        &ratios,
        opts.high_contrast,
    );
    let success = derive_status(
        green_raw,
        card,
        bg,
        fg_pole,
        is_dark,
        &ratios,
        opts.high_contrast,
    );
    let warning = derive_status(
        yellow_raw,
        card,
        bg,
        fg_pole,
        is_dark,
        &ratios,
        opts.high_contrast,
    );
    let info = derive_status(
        cyan_raw,
        card,
        bg,
        fg_pole,
        is_dark,
        &ratios,
        opts.high_contrast,
    );

    // Chart hues (>= 3:1 vs card)
    let chart_1 = ensure_contrast(accent_raw, card, thresholds.chart);
    let chart_2 = ensure_contrast(orange_raw, card, thresholds.chart);
    let chart_3 = ensure_contrast(magenta_raw, card, thresholds.chart);
    let chart_4 = ensure_contrast(cyan_raw, card, thresholds.chart);

    // Borders
    let border_subtle_raw = mix(
        card,
        fg_pole,
        if is_dark {
            ratios.border_subtle_dark
        } else {
            ratios.border_subtle_light
        },
    );
    let border_subtle = if opts.high_contrast {
        ensure_contrast(border_subtle_raw, card, thresholds.border)
    } else {
        border_subtle_raw
    };
    let border_strong = ensure_contrast(
        mix(card, fg_pole, ratios.border_strong),
        card,
        thresholds.border,
    );
    let border_focus = accent.fill;

    // Overlay & selection
    let scrim = if is_dark {
        Rgba::with_alpha(darker_bg, ratios.scrim_alpha_dark)
    } else {
        Rgba::with_alpha(Rgb::BLACK, ratios.scrim_alpha_light)
    };
    let scrim_strong = if is_dark {
        Rgba::with_alpha(darker_bg, ratios.scrim_strong_alpha_dark)
    } else {
        Rgba::with_alpha(Rgb::BLACK, ratios.scrim_strong_alpha_light)
    };

    Palette {
        is_dark,
        surface_canvas: Rgba::from_rgb(surface_canvas),
        surface_card: Rgba::from_rgb(card),
        surface_sidebar: Rgba::from_rgb(surface_sidebar),
        surface_input: Rgba::from_rgb(surface_input),
        surface_raised: Rgba::from_rgb(surface_raised),
        surface_selected: Rgba::from_rgb(surface_selected),
        surface_track: Rgba::from_rgb(surface_track),
        surface_overlay: Rgba::from_rgb(surface_overlay),
        border_subtle: Rgba::from_rgb(border_subtle),
        border_strong: Rgba::from_rgb(border_strong),
        border_focus: Rgba::from_rgb(border_focus),
        text_primary: Rgba::from_rgb(text_primary),
        text_secondary: Rgba::from_rgb(text_secondary),
        text_muted: Rgba::from_rgb(text_muted),
        text_disabled: Rgba::from_rgb(text_disabled),
        text_on_accent: Rgba::from_rgb(accent.on_fill),
        accent: Rgba::from_rgb(accent.fill),
        accent_text: Rgba::from_rgb(accent.text),
        accent_surface: Rgba::from_rgb(accent.surface),
        accent_border: Rgba::from_rgb(accent.border),
        on_accent: Rgba::from_rgb(accent.on_fill),
        danger: Rgba::from_rgb(danger.fill),
        danger_text: Rgba::from_rgb(danger.text),
        danger_surface: Rgba::from_rgb(danger.surface),
        danger_border: Rgba::from_rgb(danger.border),
        on_danger: Rgba::from_rgb(danger.on_fill),
        success: Rgba::from_rgb(success.fill),
        success_text: Rgba::from_rgb(success.text),
        success_surface: Rgba::from_rgb(success.surface),
        success_border: Rgba::from_rgb(success.border),
        on_success: Rgba::from_rgb(success.on_fill),
        warning: Rgba::from_rgb(warning.fill),
        warning_text: Rgba::from_rgb(warning.text),
        warning_surface: Rgba::from_rgb(warning.surface),
        warning_border: Rgba::from_rgb(warning.border),
        on_warning: Rgba::from_rgb(warning.on_fill),
        info: Rgba::from_rgb(info.fill),
        info_text: Rgba::from_rgb(info.text),
        info_surface: Rgba::from_rgb(info.surface),
        info_border: Rgba::from_rgb(info.border),
        on_info: Rgba::from_rgb(info.on_fill),
        scrim,
        scrim_strong,
        selection: Rgba::from_rgb(selection_raw),
        selection_text: Rgba::from_rgb(text_primary),
        chart_1: Rgba::from_rgb(chart_1),
        chart_2: Rgba::from_rgb(chart_2),
        chart_3: Rgba::from_rgb(chart_3),
        chart_4: Rgba::from_rgb(chart_4),
    }
}
