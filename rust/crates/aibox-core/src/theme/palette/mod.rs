pub mod cache;
pub mod contrast;
pub mod derive;

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::theme::color::{contrast_ratio, Rgba};
use crate::theme::parse::ThemeColors;

pub use cache::PaletteCache;
pub use contrast::{validate_palette_contrast, ContrastThresholds, ContrastViolation};
pub use derive::derive_palette_uncached;

/// Optional user override for theme polarity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModeOverride {
    #[default]
    Auto,
    Light,
    Dark,
}

impl ModeOverride {
    /// Parses `"auto"`, `"light"`, or `"dark"` (case-insensitive), defaulting to `Auto`.
    pub fn from_str_lossy(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::Auto,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

/// Options controlling palette derivation (mode override and high-contrast thresholds).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PaletteOptions {
    pub mode_override: ModeOverride,
    pub high_contrast: bool,
}

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
///
/// Stores `Copy` `Rgba` values internally and formats to `#rrggbb` / `#aarrggbb`
/// only when exported to QML, JSON, or text dumps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Palette {
    pub is_dark: bool,

    // Surfaces
    pub surface_canvas: Rgba,
    pub surface_card: Rgba,
    pub surface_sidebar: Rgba,
    pub surface_input: Rgba,
    pub surface_raised: Rgba,
    pub surface_selected: Rgba,
    pub surface_track: Rgba,
    pub surface_overlay: Rgba,

    // Borders
    pub border_subtle: Rgba,
    pub border_strong: Rgba,
    pub border_focus: Rgba,

    // Text
    pub text_primary: Rgba,
    pub text_secondary: Rgba,
    pub text_muted: Rgba,
    pub text_disabled: Rgba,
    pub text_on_accent: Rgba,

    // Accent
    pub accent: Rgba,
    pub accent_text: Rgba,
    pub accent_surface: Rgba,
    pub accent_border: Rgba,
    pub on_accent: Rgba,

    // Danger (red)
    pub danger: Rgba,
    pub danger_text: Rgba,
    pub danger_surface: Rgba,
    pub danger_border: Rgba,
    pub on_danger: Rgba,

    // Success (green)
    pub success: Rgba,
    pub success_text: Rgba,
    pub success_surface: Rgba,
    pub success_border: Rgba,
    pub on_success: Rgba,

    // Warning (yellow)
    pub warning: Rgba,
    pub warning_text: Rgba,
    pub warning_surface: Rgba,
    pub warning_border: Rgba,
    pub on_warning: Rgba,

    // Info (cyan / blue)
    pub info: Rgba,
    pub info_text: Rgba,
    pub info_surface: Rgba,
    pub info_border: Rgba,
    pub on_info: Rgba,

    // Overlay & selection
    pub scrim: Rgba,
    pub scrim_strong: Rgba,
    pub selection: Rgba,
    pub selection_text: Rgba,

    // Additional chart hues (>= 3:1 vs card)
    pub chart_1: Rgba,
    pub chart_2: Rgba,
    pub chart_3: Rgba,
    pub chart_4: Rgba,
}

/// Derives a complete contrast-guarded `Palette` from parsed `ThemeColors`
/// using default `PaletteOptions`.
pub fn derive_palette(raw: &ThemeColors) -> Palette {
    derive_palette_with_options(raw, PaletteOptions::default())
}

/// Derives a complete contrast-guarded `Palette` from parsed `ThemeColors`
/// with explicit `PaletteOptions` (mode override and high contrast), using the
/// process-wide palette cache.
pub fn derive_palette_with_options(raw: &ThemeColors, opts: PaletteOptions) -> Palette {
    match cache::global_cache().lock() {
        Ok(mut c) => c.get_or_derive(raw, opts),
        Err(_) => derive_palette_uncached(raw, opts),
    }
}

/// Resolves `colors.toml` inside `theme_dir` (checking `theme_dir` directly,
/// `theme/colors.toml`, or `colors.toml`), reads it, and returns `(ThemeColors, Palette)`
/// using the process-wide cache to skip rebuilds when file contents are unchanged.
pub fn load_colors_and_palette(theme_dir: &Path, opts: PaletteOptions) -> (ThemeColors, Palette) {
    let candidate = if theme_dir.is_file() {
        Some(theme_dir.to_path_buf())
    } else if theme_dir.join("theme").join("colors.toml").is_file() {
        Some(theme_dir.join("theme").join("colors.toml"))
    } else if theme_dir.join("colors.toml").is_file() {
        Some(theme_dir.join("colors.toml"))
    } else {
        None
    };

    if let Some(path) = candidate {
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(mut c) = cache::global_cache().lock() {
                return c.get_or_parse_and_derive(&text, opts);
            }
            let colors = crate::theme::parse::parse_colors(&text);
            let palette = derive_palette_uncached(&colors, opts);
            return (colors, palette);
        }
    }

    let colors = ThemeColors::default();
    let palette = derive_palette_with_options(&colors, opts);
    (colors, palette)
}

/// Loads `colors.toml` from `theme_dir` (checking `colors.toml` directly,
/// `theme/colors.toml`, or a direct file path) and derives its `Palette`.
pub fn load_palette(theme_dir: &Path) -> Palette {
    load_palette_with_options(theme_dir, PaletteOptions::default())
}

/// Loads `colors.toml` from `theme_dir` and derives its `Palette` with `PaletteOptions`.
pub fn load_palette_with_options(theme_dir: &Path, opts: PaletteOptions) -> Palette {
    load_colors_and_palette(theme_dir, opts).1
}

impl Palette {
    /// Returns every `(token_name, Rgba)` pair in deterministic order without allocations.
    pub fn color_tokens(&self) -> [(&'static str, Rgba); 49] {
        [
            ("surfaceCanvas", self.surface_canvas),
            ("surfaceCard", self.surface_card),
            ("surfaceSidebar", self.surface_sidebar),
            ("surfaceInput", self.surface_input),
            ("surfaceRaised", self.surface_raised),
            ("surfaceSelected", self.surface_selected),
            ("surfaceTrack", self.surface_track),
            ("surfaceOverlay", self.surface_overlay),
            ("borderSubtle", self.border_subtle),
            ("borderStrong", self.border_strong),
            ("borderFocus", self.border_focus),
            ("textPrimary", self.text_primary),
            ("textSecondary", self.text_secondary),
            ("textMuted", self.text_muted),
            ("textDisabled", self.text_disabled),
            ("textOnAccent", self.text_on_accent),
            ("accent", self.accent),
            ("accentText", self.accent_text),
            ("accentSurface", self.accent_surface),
            ("accentBorder", self.accent_border),
            ("onAccent", self.on_accent),
            ("danger", self.danger),
            ("dangerText", self.danger_text),
            ("dangerSurface", self.danger_surface),
            ("dangerBorder", self.danger_border),
            ("onDanger", self.on_danger),
            ("success", self.success),
            ("successText", self.success_text),
            ("successSurface", self.success_surface),
            ("successBorder", self.success_border),
            ("onSuccess", self.on_success),
            ("warning", self.warning),
            ("warningText", self.warning_text),
            ("warningSurface", self.warning_surface),
            ("warningBorder", self.warning_border),
            ("onWarning", self.on_warning),
            ("info", self.info),
            ("infoText", self.info_text),
            ("infoSurface", self.info_surface),
            ("infoBorder", self.info_border),
            ("onInfo", self.on_info),
            ("scrim", self.scrim),
            ("scrimStrong", self.scrim_strong),
            ("selection", self.selection),
            ("selectionText", self.selection_text),
            ("chart1", self.chart_1),
            ("chart2", self.chart_2),
            ("chart3", self.chart_3),
            ("chart4", self.chart_4),
        ]
    }

    /// Returns every `(token_name, hex_value)` pair in deterministic order.
    pub fn tokens(&self) -> Vec<(&'static str, String)> {
        self.color_tokens()
            .into_iter()
            .map(|(name, rgba)| (name, rgba.to_qml_hex()))
            .collect()
    }

    /// Formats a human-readable dump of all tokens and their WCAG contrast ratios.
    pub fn format_dump(&self) -> String {
        let card = self.surface_card.rgb();
        let mut out = format!("mode: {}\n", if self.is_dark { "dark" } else { "light" });
        for (name, rgba) in self.color_tokens() {
            let hex = rgba.to_qml_hex();
            if rgba.3 == 255 {
                let ratio = contrast_ratio(rgba.rgb(), card);
                out.push_str(&format!("{name:<18} {hex}  (vs card: {ratio:.2}:1)\n"));
            } else {
                out.push_str(&format!("{name:<18} {hex}\n"));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::color::{oklab_delta_e, Rgb};
    use crate::theme::parse::parse_colors;
    use std::path::PathBuf;
    use std::time::Instant;

    fn rgb(hex: &str) -> Rgb {
        Rgb::from_hex(hex).expect("valid hex")
    }

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("themes")
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
    fn all_23_omarchy_fixtures_satisfy_contrast_contracts_across_all_modes() {
        let dir = fixtures_dir();
        let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
            .expect("fixtures dir")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("toml"))
            .collect();
        paths.sort();
        assert_eq!(paths.len(), 23, "expected all 23 Omarchy theme fixtures");

        let modes = [
            PaletteOptions {
                mode_override: ModeOverride::Auto,
                high_contrast: false,
            },
            PaletteOptions {
                mode_override: ModeOverride::Auto,
                high_contrast: true,
            },
            PaletteOptions {
                mode_override: ModeOverride::Light,
                high_contrast: false,
            },
            PaletteOptions {
                mode_override: ModeOverride::Dark,
                high_contrast: false,
            },
        ];

        for path in &paths {
            let name = path.file_name().unwrap().to_string_lossy();
            let text = fs::read_to_string(path).expect("read fixture");
            let colors = parse_colors(&text);
            for opts in modes {
                let p = derive_palette_with_options(&colors, opts);
                let violations = validate_palette_contrast(&p, opts.high_contrast);
                assert!(
                    violations.is_empty(),
                    "{name} ({opts:?}) failed contrast validation: {violations:?}"
                );
            }
        }
    }

    #[test]
    fn tokyo_night_parity_with_current_style_qml() {
        let tokyo_path = fixtures_dir().join("tokyo-night.toml");
        let p = load_palette(&tokyo_path);
        assert!(p.is_dark);

        let pairs = [
            ("surfaceCard", p.surface_card, "#12131c"),
            ("surfaceInput", p.surface_input, "#0b0c12"),
            ("surfaceSidebar", p.surface_sidebar, "#0f1018"),
            ("surfaceSelected", p.surface_selected, "#192136"),
            ("borderSubtle", p.border_subtle, "#232738"),
        ];
        for (name, derived, legacy) in pairs {
            let de = oklab_delta_e(derived.rgb(), rgb(legacy));
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
        assert_eq!(p.surface_canvas.to_hex(), "#0e0e10");
        assert!(!p.tokens().is_empty());
    }

    #[test]
    fn mode_override_flips_polarity_with_valid_contrast() {
        let dir = fixtures_dir();

        let tokyo_forced_light = load_palette_with_options(
            &dir.join("tokyo-night.toml"),
            PaletteOptions {
                mode_override: ModeOverride::Light,
                high_contrast: false,
            },
        );
        assert!(!tokyo_forced_light.is_dark);
        let card = tokyo_forced_light.surface_card.rgb();
        assert_contrast(
            "tokyo-night->light",
            "textPrimary",
            tokyo_forced_light.text_primary.rgb(),
            card,
            7.0,
        );
        assert_contrast(
            "tokyo-night->light",
            "textMuted",
            tokyo_forced_light.text_muted.rgb(),
            card,
            4.5,
        );

        let latte_forced_dark = load_palette_with_options(
            &dir.join("catppuccin-latte.toml"),
            PaletteOptions {
                mode_override: ModeOverride::Dark,
                high_contrast: false,
            },
        );
        assert!(latte_forced_dark.is_dark);
        let card = latte_forced_dark.surface_card.rgb();
        assert_contrast(
            "latte->dark",
            "textPrimary",
            latte_forced_dark.text_primary.rgb(),
            card,
            7.0,
        );
        assert_contrast(
            "latte->dark",
            "textMuted",
            latte_forced_dark.text_muted.rgb(),
            card,
            4.5,
        );
    }

    #[test]
    fn high_contrast_enforces_aaa_text_and_3_to_1_borders() {
        let dir = fixtures_dir();

        for fixture in ["tokyo-night.toml", "catppuccin-latte.toml", "white.toml"] {
            let p = load_palette_with_options(
                &dir.join(fixture),
                PaletteOptions {
                    mode_override: ModeOverride::Auto,
                    high_contrast: true,
                },
            );
            let card = p.surface_card.rgb();
            assert_contrast(fixture, "textPrimary", p.text_primary.rgb(), card, 7.0);
            assert_contrast(fixture, "textSecondary", p.text_secondary.rgb(), card, 7.0);
            assert_contrast(fixture, "textMuted", p.text_muted.rgb(), card, 7.0);
            assert_contrast(fixture, "accentText", p.accent_text.rgb(), card, 7.0);
            assert_contrast(fixture, "dangerText", p.danger_text.rgb(), card, 7.0);
            assert_contrast(fixture, "borderSubtle", p.border_subtle.rgb(), card, 3.0);
            assert_contrast(fixture, "borderStrong", p.border_strong.rgb(), card, 3.0);
        }
    }

    #[test]
    fn chart_hues_meet_contrast_against_card() {
        let dir = fixtures_dir();
        for fixture in ["tokyo-night.toml", "catppuccin-latte.toml"] {
            let p = load_palette(&dir.join(fixture));
            let card = p.surface_card.rgb();
            assert_contrast(fixture, "chart1", p.chart_1.rgb(), card, 3.0);
            assert_contrast(fixture, "chart2", p.chart_2.rgb(), card, 3.0);
            assert_contrast(fixture, "chart3", p.chart_3.rgb(), card, 3.0);
            assert_contrast(fixture, "chart4", p.chart_4.rgb(), card, 3.0);
        }
    }

    #[test]
    fn text_muted_meets_contrast_against_card_and_sidebar_across_all_fixtures() {
        let dir = fixtures_dir();
        let mut count = 0;
        for entry in fs::read_dir(&dir).expect("fixtures dir") {
            let path = entry.expect("entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            count += 1;
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let p = load_palette(&path);
            let muted = p.text_muted.rgb();

            assert_contrast(
                &name,
                "textMuted vs surfaceCard",
                muted,
                p.surface_card.rgb(),
                4.5,
            );
            assert_contrast(
                &name,
                "textMuted vs surfaceSidebar",
                muted,
                p.surface_sidebar.rgb(),
                4.5,
            );
        }
        assert_eq!(count, 23, "expected all 23 Omarchy theme fixtures");
    }

    #[test]
    fn palette_cache_hits_for_same_theme_and_skips_unchanged_content_rebuild() {
        let tokyo_text =
            fs::read_to_string(fixtures_dir().join("tokyo-night.toml")).expect("read tokyo-night");
        let colors = parse_colors(&tokyo_text);
        let mut cache = PaletteCache::new();

        let opts_auto = PaletteOptions::default();
        let opts_hc = PaletteOptions {
            mode_override: ModeOverride::Auto,
            high_contrast: true,
        };
        let opts_light = PaletteOptions {
            mode_override: ModeOverride::Light,
            high_contrast: false,
        };

        let p1 = cache.get_or_derive(&colors, opts_auto);
        assert_eq!(cache.misses(), 1);
        assert_eq!(cache.hits(), 0);

        let p2 = cache.get_or_derive(&colors, opts_auto);
        assert_eq!(p1, p2);
        assert_eq!(cache.misses(), 1);
        assert_eq!(cache.hits(), 1);

        let p_hc = cache.get_or_derive(&colors, opts_hc);
        assert_ne!(p_hc, p1);
        assert_eq!(cache.misses(), 2);

        let p_light = cache.get_or_derive(&colors, opts_light);
        assert!(!p_light.is_dark);
        assert_eq!(cache.misses(), 3);

        // Content cache hits when the file content is identical (e.g., rewritten without changes)
        let (c1, cp1) = cache.get_or_parse_and_derive(&tokyo_text, opts_auto);
        assert_eq!(cp1, p1);
        assert_eq!(c1, colors);
        // Misses stay 3 because derive_entries already had (colors, opts_auto), and hits incremented
        assert_eq!(cache.misses(), 3);
        let hits_before = cache.hits();
        let (c2, cp2) = cache.get_or_parse_and_derive(&tokyo_text, opts_auto);
        assert_eq!(c2, c1);
        assert_eq!(cp2, cp1);
        assert_eq!(cache.hits(), hits_before + 1);
    }

    #[test]
    fn derive_palette_completes_under_one_millisecond() {
        let dir = fixtures_dir();
        let themes: Vec<ThemeColors> = fs::read_dir(&dir)
            .expect("fixtures dir")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("toml"))
            .map(|p| parse_colors(&fs::read_to_string(p).expect("read fixture")))
            .collect();
        assert_eq!(themes.len(), 23);

        let opts = PaletteOptions::default();
        let iterations = 10;
        let start = Instant::now();
        for _ in 0..iterations {
            for colors in &themes {
                let p = std::hint::black_box(derive_palette_uncached(colors, opts));
                std::hint::black_box(p);
            }
        }
        let elapsed = start.elapsed();
        let per_palette = elapsed / (themes.len() * iterations) as u32;
        assert!(
            per_palette.as_micros() < 1000,
            "palette build took {} us (>= 1 ms)",
            per_palette.as_micros()
        );
    }
}
