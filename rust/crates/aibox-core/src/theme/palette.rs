use std::path::Path;

use serde::Serialize;

use super::color::{contrast_ratio, ensure_contrast, mix, Rgb};
use super::parse::{parse_colors_file, ThemeColors};

/// Mixing ratios and scrim opacities used to derive a semantic `Palette`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MixRatios {
    pub sidebar_dark: f64,
    pub sidebar_light: f64,
    pub input_light: f64,
    pub raised_dark: f64,
    pub raised_light: f64,
    pub selected_dark: f64,
    pub selected_light: f64,
    pub track_dark: f64,
    pub track_light: f64,
    pub border_subtle_dark: f64,
    pub border_subtle_light: f64,
    pub border_strong: f64,
    pub text_disabled: f64,
    pub status_surface_dark: f64,
    pub status_surface_light: f64,
    pub status_border: f64,
    pub scrim_alpha_dark: f64,
    pub scrim_alpha_light: f64,
    pub scrim_strong_alpha_dark: f64,
    pub scrim_strong_alpha_light: f64,
}

impl MixRatios {
    pub const DEFAULT: Self = Self {
        sidebar_dark: 0.5,
        sidebar_light: 0.3,
        input_light: 0.6,
        raised_dark: 0.06,
        raised_light: 0.05,
        selected_dark: 0.16,
        selected_light: 0.12,
        track_dark: 0.08,
        track_light: 0.10,
        border_subtle_dark: 0.12,
        border_subtle_light: 0.14,
        border_strong: 0.28,
        text_disabled: 0.4,
        status_surface_dark: 0.14,
        status_surface_light: 0.10,
        status_border: 0.40,
        scrim_alpha_dark: 0.77,
        scrim_alpha_light: 0.28,
        scrim_strong_alpha_dark: 0.88,
        scrim_strong_alpha_light: 0.42,
    };
}

impl Default for MixRatios {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Complete contrast-guarded semantic color palette derived from `ThemeColors`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Palette {
    pub is_dark: bool,

    // Surfaces
    pub surface_canvas: String,
    pub surface_card: String,
    pub surface_sidebar: String,
    pub surface_input: String,
    pub surface_raised: String,
    pub surface_selected: String,
    pub surface_track: String,
    pub surface_overlay: String,

    // Borders
    pub border_subtle: String,
    pub border_strong: String,
    pub border_focus: String,

    // Text
    pub text_primary: String,
    pub text_secondary: String,
    pub text_muted: String,
    pub text_disabled: String,
    pub text_on_accent: String,

    // Accent
    pub accent: String,
    pub accent_text: String,
    pub accent_surface: String,
    pub accent_border: String,
    pub on_accent: String,

    // Danger (red)
    pub danger: String,
    pub danger_text: String,
    pub danger_surface: String,
    pub danger_border: String,
    pub on_danger: String,

    // Success (green)
    pub success: String,
    pub success_text: String,
    pub success_surface: String,
    pub success_border: String,
    pub on_success: String,

    // Warning (yellow)
    pub warning: String,
    pub warning_text: String,
    pub warning_surface: String,
    pub warning_border: String,
    pub on_warning: String,

    // Info (cyan / blue)
    pub info: String,
    pub info_text: String,
    pub info_surface: String,
    pub info_border: String,
    pub on_info: String,

    // Overlay & selection
    pub scrim: String,
    pub scrim_strong: String,
    pub selection: String,
    pub selection_text: String,
}

struct StatusTokens {
    fill: Rgb,
    text: Rgb,
    surface: Rgb,
    border: Rgb,
    on_fill: Rgb,
}

fn parse_rgb_or(hex: &str, fallback: Rgb) -> Rgb {
    Rgb::from_hex(hex).unwrap_or(fallback)
}

fn best_on_color(surface: Rgb, bg: Rgb, fg: Rgb) -> Rgb {
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

fn derive_status(
    raw: Rgb,
    card: Rgb,
    bg: Rgb,
    fg: Rgb,
    is_dark: bool,
    ratios: &MixRatios,
) -> StatusTokens {
    let fill = ensure_contrast(raw, card, 3.0);
    let surface_ratio = if is_dark {
        ratios.status_surface_dark
    } else {
        ratios.status_surface_light
    };
    let surface = mix(card, fill, surface_ratio);
    let text_on_card = ensure_contrast(raw, card, 4.5);
    let text = ensure_contrast(text_on_card, surface, 4.5);
    let border = ensure_contrast(mix(card, fill, ratios.status_border), card, 1.8);
    let on_fill = best_on_color(fill, bg, fg);
    StatusTokens {
        fill,
        text,
        surface,
        border,
        on_fill,
    }
}

/// Derives a complete contrast-guarded `Palette` from parsed `ThemeColors`.
pub fn derive_palette(raw: &ThemeColors) -> Palette {
    let ratios = MixRatios::DEFAULT;
    let is_dark = raw.is_dark;
    let fallback = if is_dark {
        ThemeColors::default()
    } else {
        ThemeColors::default_light()
    };

    let bg = parse_rgb_or(
        &raw.background,
        Rgb::from_hex(&fallback.background).unwrap_or(Rgb::BLACK),
    );
    let dark_bg = parse_rgb_or(
        &raw.dark_background,
        Rgb::from_hex(&fallback.dark_background).unwrap_or(bg),
    );
    let darker_bg = parse_rgb_or(
        &raw.darker_background,
        Rgb::from_hex(&fallback.darker_background).unwrap_or(dark_bg),
    );
    let fg_raw = parse_rgb_or(
        &raw.bright_foreground,
        Rgb::from_hex(&fallback.bright_foreground).unwrap_or(Rgb::WHITE),
    );
    let fg_secondary_raw = parse_rgb_or(&raw.base_foreground, fg_raw);
    let fg_muted_raw = parse_rgb_or(&raw.dark_foreground, fg_secondary_raw);
    let selection_raw = parse_rgb_or(&raw.selection, dark_bg);

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
    let cyan_raw = parse_rgb_or(
        &raw.cyan,
        Rgb::from_hex(&fallback.cyan).unwrap_or(accent_raw),
    );

    // Surfaces
    let surface_canvas = bg;
    let card = dark_bg;
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
        fg_raw,
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
        fg_raw,
        if is_dark {
            ratios.track_dark
        } else {
            ratios.track_light
        },
    );
    let surface_overlay = if is_dark { card } else { bg };

    // Text
    let text_primary = ensure_contrast(fg_raw, card, 7.0);
    let text_secondary = ensure_contrast(fg_secondary_raw, card, 4.5);
    let text_muted = ensure_contrast(fg_muted_raw, card, 4.5);
    let text_disabled = mix(text_muted, card, ratios.text_disabled);

    // Accent & Status
    let accent = derive_status(accent_raw, card, bg, fg_raw, is_dark, &ratios);
    let danger = derive_status(red_raw, card, bg, fg_raw, is_dark, &ratios);
    let success = derive_status(green_raw, card, bg, fg_raw, is_dark, &ratios);
    let warning = derive_status(yellow_raw, card, bg, fg_raw, is_dark, &ratios);
    let info = derive_status(cyan_raw, card, bg, fg_raw, is_dark, &ratios);

    // Borders
    let border_subtle = mix(
        card,
        fg_raw,
        if is_dark {
            ratios.border_subtle_dark
        } else {
            ratios.border_subtle_light
        },
    );
    let border_strong = ensure_contrast(mix(card, fg_raw, ratios.border_strong), card, 1.8);
    let border_focus = accent.fill;

    // Overlay & selection
    let scrim = if is_dark {
        darker_bg.to_qml_argb(ratios.scrim_alpha_dark)
    } else {
        Rgb::BLACK.to_qml_argb(ratios.scrim_alpha_light)
    };
    let scrim_strong = if is_dark {
        darker_bg.to_qml_argb(ratios.scrim_strong_alpha_dark)
    } else {
        Rgb::BLACK.to_qml_argb(ratios.scrim_strong_alpha_light)
    };

    Palette {
        is_dark,
        surface_canvas: surface_canvas.to_hex(),
        surface_card: card.to_hex(),
        surface_sidebar: surface_sidebar.to_hex(),
        surface_input: surface_input.to_hex(),
        surface_raised: surface_raised.to_hex(),
        surface_selected: surface_selected.to_hex(),
        surface_track: surface_track.to_hex(),
        surface_overlay: surface_overlay.to_hex(),
        border_subtle: border_subtle.to_hex(),
        border_strong: border_strong.to_hex(),
        border_focus: border_focus.to_hex(),
        text_primary: text_primary.to_hex(),
        text_secondary: text_secondary.to_hex(),
        text_muted: text_muted.to_hex(),
        text_disabled: text_disabled.to_hex(),
        text_on_accent: accent.on_fill.to_hex(),
        accent: accent.fill.to_hex(),
        accent_text: accent.text.to_hex(),
        accent_surface: accent.surface.to_hex(),
        accent_border: accent.border.to_hex(),
        on_accent: accent.on_fill.to_hex(),
        danger: danger.fill.to_hex(),
        danger_text: danger.text.to_hex(),
        danger_surface: danger.surface.to_hex(),
        danger_border: danger.border.to_hex(),
        on_danger: danger.on_fill.to_hex(),
        success: success.fill.to_hex(),
        success_text: success.text.to_hex(),
        success_surface: success.surface.to_hex(),
        success_border: success.border.to_hex(),
        on_success: success.on_fill.to_hex(),
        warning: warning.fill.to_hex(),
        warning_text: warning.text.to_hex(),
        warning_surface: warning.surface.to_hex(),
        warning_border: warning.border.to_hex(),
        on_warning: warning.on_fill.to_hex(),
        info: info.fill.to_hex(),
        info_text: info.text.to_hex(),
        info_surface: info.surface.to_hex(),
        info_border: info.border.to_hex(),
        on_info: info.on_fill.to_hex(),
        scrim,
        scrim_strong,
        selection: selection_raw.to_hex(),
        selection_text: text_primary.to_hex(),
    }
}

/// Loads `colors.toml` from `theme_dir` (checking `colors.toml` directly,
/// `theme/colors.toml`, or a direct file path) and derives its `Palette`.
pub fn load_palette(theme_dir: &Path) -> Palette {
    let candidate = if theme_dir.is_file() {
        Some(theme_dir.to_path_buf())
    } else if theme_dir.join("theme").join("colors.toml").is_file() {
        Some(theme_dir.join("theme").join("colors.toml"))
    } else if theme_dir.join("colors.toml").is_file() {
        Some(theme_dir.join("colors.toml"))
    } else {
        None
    };

    let colors = match candidate {
        Some(path) => match path.to_str() {
            Some(s) => parse_colors_file(s),
            None => ThemeColors::default(),
        },
        None => ThemeColors::default(),
    };
    derive_palette(&colors)
}

impl Palette {
    /// Returns every `(token_name, hex_value)` pair in deterministic order.
    pub fn tokens(&self) -> Vec<(&'static str, String)> {
        vec![
            ("surfaceCanvas", self.surface_canvas.clone()),
            ("surfaceCard", self.surface_card.clone()),
            ("surfaceSidebar", self.surface_sidebar.clone()),
            ("surfaceInput", self.surface_input.clone()),
            ("surfaceRaised", self.surface_raised.clone()),
            ("surfaceSelected", self.surface_selected.clone()),
            ("surfaceTrack", self.surface_track.clone()),
            ("surfaceOverlay", self.surface_overlay.clone()),
            ("borderSubtle", self.border_subtle.clone()),
            ("borderStrong", self.border_strong.clone()),
            ("borderFocus", self.border_focus.clone()),
            ("textPrimary", self.text_primary.clone()),
            ("textSecondary", self.text_secondary.clone()),
            ("textMuted", self.text_muted.clone()),
            ("textDisabled", self.text_disabled.clone()),
            ("textOnAccent", self.text_on_accent.clone()),
            ("accent", self.accent.clone()),
            ("accentText", self.accent_text.clone()),
            ("accentSurface", self.accent_surface.clone()),
            ("accentBorder", self.accent_border.clone()),
            ("onAccent", self.on_accent.clone()),
            ("danger", self.danger.clone()),
            ("dangerText", self.danger_text.clone()),
            ("dangerSurface", self.danger_surface.clone()),
            ("dangerBorder", self.danger_border.clone()),
            ("onDanger", self.on_danger.clone()),
            ("success", self.success.clone()),
            ("successText", self.success_text.clone()),
            ("successSurface", self.success_surface.clone()),
            ("successBorder", self.success_border.clone()),
            ("onSuccess", self.on_success.clone()),
            ("warning", self.warning.clone()),
            ("warningText", self.warning_text.clone()),
            ("warningSurface", self.warning_surface.clone()),
            ("warningBorder", self.warning_border.clone()),
            ("onWarning", self.on_warning.clone()),
            ("info", self.info.clone()),
            ("infoText", self.info_text.clone()),
            ("infoSurface", self.info_surface.clone()),
            ("infoBorder", self.info_border.clone()),
            ("onInfo", self.on_info.clone()),
            ("scrim", self.scrim.clone()),
            ("scrimStrong", self.scrim_strong.clone()),
            ("selection", self.selection.clone()),
            ("selectionText", self.selection_text.clone()),
        ]
    }

    /// Formats a human-readable dump of all tokens and their WCAG contrast ratios.
    pub fn format_dump(&self) -> String {
        let card = Rgb::from_hex(&self.surface_card).unwrap_or(Rgb::BLACK);
        let mut out = format!("mode: {}\n", if self.is_dark { "dark" } else { "light" });
        for (name, value) in self.tokens() {
            if let Some(rgb) = Rgb::from_hex(&value) {
                let ratio = contrast_ratio(rgb, card);
                out.push_str(&format!("{name:<18} {value}  (vs card: {ratio:.2}:1)\n"));
            } else {
                out.push_str(&format!("{name:<18} {value}\n"));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::color::oklab_delta_e;
    use crate::theme::parse::parse_colors;
    use std::fs;
    use std::path::PathBuf;

    fn rgb(hex: &str) -> Rgb {
        Rgb::from_hex(hex).expect("valid hex")
    }

    fn assert_contrast(theme: &str, pair: &str, fg: Rgb, bg: Rgb, min: f64) {
        let ratio = contrast_ratio(fg, bg);
        assert!(
            ratio >= min,
            "{theme}: {pair} contrast {ratio:.2}:1 < min {min:.2}:1 (fg={}, bg={})",
            fg.to_hex(),
            bg.to_hex()
        );
    }

    #[test]
    fn all_23_omarchy_fixtures_satisfy_contrast_contracts() {
        let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("themes");

        let mut count = 0;
        for entry in fs::read_dir(&fixtures_dir).expect("fixtures dir") {
            let path = entry.expect("entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            count += 1;
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let text = fs::read_to_string(&path).expect("read fixture");
            let colors = parse_colors(&text);
            let p = derive_palette(&colors);
            let card = rgb(&p.surface_card);

            // Borders
            assert_contrast(
                &name,
                "borderStrong vs card",
                rgb(&p.border_strong),
                card,
                1.8,
            );
            assert_contrast(
                &name,
                "borderFocus vs card",
                rgb(&p.border_focus),
                card,
                3.0,
            );

            // Text
            assert_contrast(
                &name,
                "textPrimary vs card",
                rgb(&p.text_primary),
                card,
                7.0,
            );
            assert_contrast(
                &name,
                "textSecondary vs card",
                rgb(&p.text_secondary),
                card,
                4.5,
            );
            assert_contrast(&name, "textMuted vs card", rgb(&p.text_muted), card, 4.5);

            // Accent & Statuses
            for (label, fill, text_tok, surf, border) in [
                (
                    "accent",
                    &p.accent,
                    &p.accent_text,
                    &p.accent_surface,
                    &p.accent_border,
                ),
                (
                    "danger",
                    &p.danger,
                    &p.danger_text,
                    &p.danger_surface,
                    &p.danger_border,
                ),
                (
                    "success",
                    &p.success,
                    &p.success_text,
                    &p.success_surface,
                    &p.success_border,
                ),
                (
                    "warning",
                    &p.warning,
                    &p.warning_text,
                    &p.warning_surface,
                    &p.warning_border,
                ),
                (
                    "info",
                    &p.info,
                    &p.info_text,
                    &p.info_surface,
                    &p.info_border,
                ),
            ] {
                assert_contrast(&name, &format!("{label} vs card"), rgb(fill), card, 3.0);
                assert_contrast(
                    &name,
                    &format!("{label}Text vs card"),
                    rgb(text_tok),
                    card,
                    4.5,
                );
                assert_contrast(
                    &name,
                    &format!("{label}Text vs {label}Surface"),
                    rgb(text_tok),
                    rgb(surf),
                    4.5,
                );
                assert_contrast(
                    &name,
                    &format!("{label}Border vs card"),
                    rgb(border),
                    card,
                    1.8,
                );
            }
        }
        assert_eq!(count, 23, "expected all 23 Omarchy theme fixtures");
    }

    #[test]
    fn tokyo_night_parity_with_current_style_qml() {
        let tokyo_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("themes")
            .join("tokyo-night.toml");
        let p = load_palette(&tokyo_path);
        assert!(p.is_dark);

        let pairs = [
            ("surfaceCard", &p.surface_card, "#12131c"),
            ("surfaceInput", &p.surface_input, "#0b0c12"),
            ("surfaceSidebar", &p.surface_sidebar, "#0f1018"),
            ("surfaceSelected", &p.surface_selected, "#192136"),
            ("borderSubtle", &p.border_subtle, "#232738"),
        ];
        for (name, derived, legacy) in pairs {
            let de = oklab_delta_e(rgb(derived), rgb(legacy));
            assert!(
                de < 0.03,
                "Tokyo Night parity for {name}: derived={derived}, legacy={legacy}, deltaE={de}"
            );
        }
    }

    #[test]
    fn load_palette_falls_back_on_missing_dir() {
        let p = load_palette(Path::new("/nonexistent/aibox-theme-dir"));
        assert!(p.is_dark);
        assert_eq!(p.surface_canvas, "#0e0e10");
        assert!(!p.tokens().is_empty());
    }
}
