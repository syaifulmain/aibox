use std::fs;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::color::{relative_luminance, Rgb};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeColors {
    pub is_dark: bool,
    pub mode: String,
    pub accent: String,
    pub background: String,
    pub dark_background: String,
    pub darker_background: String,
    pub lighter_background: String,
    pub foreground: String,
    pub base_foreground: String,
    pub light_foreground: String,
    pub bright_foreground: String,
    pub dark_foreground: String,
    pub muted: String,
    pub raw_muted: String,
    pub selection: String,
    pub green: String,
    pub red: String,
    pub yellow: String,
    pub orange: String,
    pub cyan: String,
    pub blue: String,
    pub magenta: String,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            is_dark: true,
            mode: "dark".to_string(),
            accent: "#FFD60A".to_string(),
            background: "#0e0e10".to_string(),
            dark_background: "#13141c".to_string(),
            darker_background: "#0e0e14".to_string(),
            lighter_background: "#1e202e".to_string(),
            foreground: "#c0caf5".to_string(),
            base_foreground: "#a9b1d6".to_string(),
            light_foreground: "#b4bee6".to_string(),
            bright_foreground: "#c0caf5".to_string(),
            dark_foreground: "#565f89".to_string(),
            muted: "#565f89".to_string(),
            raw_muted: "#414868".to_string(),
            selection: "#292e42".to_string(),
            green: "#9ece6a".to_string(),
            red: "#f7768e".to_string(),
            yellow: "#e0af68".to_string(),
            orange: "#eb927b".to_string(),
            cyan: "#449dab".to_string(),
            blue: "#7aa2f7".to_string(),
            magenta: "#ad8ee6".to_string(),
        }
    }
}

impl ThemeColors {
    /// Default light-mode fallback palette (used when `mode = "light"` or
    /// luminance indicates a light theme and individual keys are missing).
    pub fn default_light() -> Self {
        Self {
            is_dark: false,
            mode: "light".to_string(),
            accent: "#1e66f5".to_string(),
            background: "#eff1f5".to_string(),
            dark_background: "#e3e4e8".to_string(),
            darker_background: "#d7d8dc".to_string(),
            lighter_background: "#dce0e8".to_string(),
            foreground: "#4c4f69".to_string(),
            base_foreground: "#4c4f69".to_string(),
            light_foreground: "#5c5f77".to_string(),
            bright_foreground: "#4c4f69".to_string(),
            dark_foreground: "#9ca0b0".to_string(),
            muted: "#9ca0b0".to_string(),
            raw_muted: "#acb0be".to_string(),
            selection: "#ccd0da".to_string(),
            green: "#40a02b".to_string(),
            red: "#d20f39".to_string(),
            yellow: "#df8e1d".to_string(),
            orange: "#d84e2b".to_string(),
            cyan: "#179299".to_string(),
            blue: "#1e66f5".to_string(),
            magenta: "#ea76cb".to_string(),
        }
    }
}

fn parse_toml_kv_line(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('[') {
        return None;
    }
    let (lhs, rhs) = trimmed.split_once('=')?;
    let k = lhs.trim();
    if k.is_empty() {
        return None;
    }
    Some((k, rhs.trim_start()))
}

fn parse_quoted_hex6(rhs: &str) -> Option<&str> {
    let bytes = rhs.as_bytes();
    if bytes.len() < 9 {
        return None;
    }
    let quote = bytes[0];
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    if bytes[1] != b'#' {
        return None;
    }
    if !bytes[2..8].iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    if bytes[8] != b'"' && bytes[8] != b'\'' {
        return None;
    }
    Some(&rhs[1..8])
}

fn parse_quoted_string(rhs: &str) -> Option<&str> {
    let bytes = rhs.as_bytes();
    let first = *bytes.first()?;
    if first != b'"' && first != b'\'' {
        return None;
    }
    let rest = &rhs[1..];
    let end = rest.find(['"', '\'', '\r', '\n'])?;
    let val = rest[..end].trim();
    if val.is_empty() {
        None
    } else {
        Some(val)
    }
}

/// Reads a `#RRGGBB` hex color for `key` from Omarchy `colors.toml` text,
/// falling back to `fallback` when missing or malformed.
pub fn read_key(toml_text: &str, key: &str, fallback: &str) -> String {
    for line in toml_text.lines() {
        let Some((k, rhs)) = parse_toml_kv_line(line) else {
            continue;
        };
        if k == key {
            if let Some(hex) = parse_quoted_hex6(rhs) {
                return hex.to_string();
            }
        }
    }
    fallback.to_string()
}

/// Reads a quoted string value for `key` from `colors.toml` text (e.g. `mode = "light"`).
pub fn read_string_key(toml_text: &str, key: &str) -> Option<String> {
    for line in toml_text.lines() {
        let Some((k, rhs)) = parse_toml_kv_line(line) else {
            continue;
        };
        if k == key {
            if let Some(val) = parse_quoted_string(rhs) {
                return Some(val.to_string());
            }
        }
    }
    None
}

/// Reads `accent` from `colors.toml` text with `#FFD60A` fallback.
pub fn read_accent(toml_text: &str) -> String {
    read_key(toml_text, "accent", "#FFD60A")
}

/// Parses all theme colors from `colors.toml` text in a single pass.
pub fn parse_colors(toml_text: &str) -> ThemeColors {
    let dark_defaults = ThemeColors::default();
    let light_defaults = ThemeColors::default_light();

    let mut mode_raw: Option<String> = None;
    let mut accent: Option<&str> = None;
    let mut background: Option<&str> = None;
    let mut dark_background: Option<&str> = None;
    let mut darker_background: Option<&str> = None;
    let mut lighter_background: Option<&str> = None;
    let mut foreground: Option<&str> = None;
    let mut bright_foreground: Option<&str> = None;
    let mut dark_foreground: Option<&str> = None;
    let mut light_foreground: Option<&str> = None;
    let mut raw_muted: Option<&str> = None;
    let mut selection: Option<&str> = None;
    let mut green: Option<&str> = None;
    let mut red: Option<&str> = None;
    let mut yellow: Option<&str> = None;
    let mut orange: Option<&str> = None;
    let mut cyan: Option<&str> = None;
    let mut blue: Option<&str> = None;
    let mut magenta: Option<&str> = None;

    for line in toml_text.lines() {
        let Some((k, rhs)) = parse_toml_kv_line(line) else {
            continue;
        };
        if k == "mode" {
            if mode_raw.is_none() {
                if let Some(val) = parse_quoted_string(rhs) {
                    mode_raw = Some(val.to_ascii_lowercase());
                }
            }
            continue;
        }
        let slot = match k {
            "accent" => &mut accent,
            "background" => &mut background,
            "dark_background" => &mut dark_background,
            "darker_background" => &mut darker_background,
            "lighter_background" => &mut lighter_background,
            "foreground" => &mut foreground,
            "bright_foreground" => &mut bright_foreground,
            "dark_foreground" => &mut dark_foreground,
            "light_foreground" => &mut light_foreground,
            "muted" => &mut raw_muted,
            "selection" => &mut selection,
            "green" => &mut green,
            "red" => &mut red,
            "yellow" => &mut yellow,
            "orange" => &mut orange,
            "cyan" => &mut cyan,
            "blue" => &mut blue,
            "magenta" => &mut magenta,
            _ => continue,
        };
        if slot.is_none() {
            if let Some(hex) = parse_quoted_hex6(rhs) {
                *slot = Some(hex);
            }
        }
    }

    let is_dark = match mode_raw.as_deref() {
        Some("light") => false,
        Some("dark") => true,
        _ => {
            let bg_hex = background.unwrap_or(&dark_defaults.background);
            let bg_rgb = Rgb::from_hex(bg_hex).unwrap_or(Rgb::new(0x0e, 0x0e, 0x10));
            relative_luminance(bg_rgb) < 0.4
        }
    };

    let defaults = if is_dark {
        &dark_defaults
    } else {
        &light_defaults
    };

    let bg = background.unwrap_or(&defaults.background).to_string();
    let base_fg = foreground.unwrap_or(&defaults.base_foreground).to_string();
    let fg_compat = foreground.unwrap_or(&defaults.foreground);
    let bright_fg = bright_foreground.unwrap_or(fg_compat).to_string();
    let dark_fg = dark_foreground
        .unwrap_or(&defaults.dark_foreground)
        .to_string();
    let blue_val = blue.unwrap_or(&defaults.blue).to_string();
    let cyan_fallback = blue.unwrap_or(&defaults.cyan);

    ThemeColors {
        is_dark,
        mode: if is_dark {
            "dark".to_string()
        } else {
            "light".to_string()
        },
        accent: accent.unwrap_or(&defaults.accent).to_string(),
        background: bg,
        dark_background: dark_background
            .unwrap_or(&defaults.dark_background)
            .to_string(),
        darker_background: darker_background
            .unwrap_or(&defaults.darker_background)
            .to_string(),
        lighter_background: lighter_background
            .unwrap_or(&defaults.lighter_background)
            .to_string(),
        foreground: bright_fg.clone(),
        base_foreground: base_fg,
        light_foreground: light_foreground
            .unwrap_or(&defaults.light_foreground)
            .to_string(),
        bright_foreground: bright_fg,
        dark_foreground: dark_fg.clone(),
        muted: dark_fg,
        raw_muted: raw_muted.unwrap_or(&defaults.raw_muted).to_string(),
        selection: selection.unwrap_or(&defaults.selection).to_string(),
        green: green.unwrap_or(&defaults.green).to_string(),
        red: red.unwrap_or(&defaults.red).to_string(),
        yellow: yellow.unwrap_or(&defaults.yellow).to_string(),
        orange: orange.unwrap_or(&defaults.orange).to_string(),
        cyan: cyan.unwrap_or(cyan_fallback).to_string(),
        blue: blue_val,
        magenta: magenta.unwrap_or(&defaults.magenta).to_string(),
    }
}

/// Reads and parses a `colors.toml` file, returning defaults if unreadable.
pub fn parse_colors_file(path: &str) -> ThemeColors {
    match fs::read_to_string(path) {
        Ok(text) => parse_colors(&text),
        Err(_) => ThemeColors::default(),
    }
}

/// Reads a single key from a `colors.toml` file, returning `fallback` if missing.
pub fn read_key_file(path: &str, key: &str, fallback: &str) -> String {
    match fs::read_to_string(path) {
        Ok(text) => read_key(&text, key, fallback),
        Err(_) => fallback.to_string(),
    }
}

/// Reads `accent` from a `colors.toml` file, returning `#FFD60A` if missing.
pub fn read_accent_file(path: &str) -> String {
    read_key_file(path, "accent", "#FFD60A")
}

/// Returns `"black"` or `"white"`, whichever has better contrast on `accent`
/// according to WCAG relative luminance.
pub fn accent_foreground(accent_hex: &str) -> &'static str {
    let Some(rgb) = Rgb::from_hex(accent_hex) else {
        return "white";
    };
    if relative_luminance(rgb) > 0.179 {
        "black"
    } else {
        "white"
    }
}

/// Parses `hyprctl -j getoption decoration:rounding` stdout into a non-negative
/// corner radius.
pub fn parse_rounding(hyprctl_stdout: &[u8]) -> Option<i32> {
    let val: Value = serde_json::from_slice(hyprctl_stdout).ok()?;
    let int_val = val.get("int")?;
    let n = if let Some(i) = int_val.as_i64() {
        i
    } else {
        int_val.as_f64()?.round() as i64
    };
    if n < 0 || n > i32::MAX as i64 {
        return None;
    }
    Some(n as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn defaults_match_qt_theme_falls_back() {
        let colors = parse_colors("");
        assert!(colors.is_dark);
        assert_eq!(colors.accent, "#FFD60A");
        assert_eq!(accent_foreground(&colors.accent), "black");
        assert_eq!(colors.background, "#0e0e10");
        assert_eq!(colors.dark_background, "#13141c");
        assert_eq!(colors.lighter_background, "#1e202e");
        assert_eq!(colors.foreground, "#c0caf5");
        assert_eq!(colors.muted, "#565f89");
        assert_eq!(colors.green, "#9ece6a");
        assert_eq!(colors.red, "#f7768e");
        assert_eq!(colors.yellow, "#e0af68");
    }

    #[test]
    fn detects_light_mode_from_key_or_luminance() {
        let from_key = parse_colors("mode = \"light\"\n");
        assert!(!from_key.is_dark);
        assert_eq!(from_key.mode, "light");
        assert_eq!(from_key.background, "#eff1f5");

        let from_lum = parse_colors("background = \"#ffffff\"\n");
        assert!(!from_lum.is_dark);
        assert_eq!(from_lum.mode, "light");

        let from_dark_lum = parse_colors("background = \"#101218\"\n");
        assert!(from_dark_lum.is_dark);
        assert_eq!(from_dark_lum.mode, "dark");
    }

    #[test]
    fn reads_accent_and_contrast() {
        let colors = parse_colors("accent = \"#112233\"\n");
        assert_eq!(colors.accent, "#112233");
        assert_eq!(accent_foreground(&colors.accent), "white");
    }

    #[test]
    fn parses_hyprctl_rounding_json() {
        assert_eq!(parse_rounding(br#"{"int":0}"#), Some(0));
        assert_eq!(parse_rounding(br#"{"int":6}"#), Some(6));
        assert_eq!(parse_rounding(br#"{"int":-1}"#), None);
        assert_eq!(parse_rounding(br#"{}"#), None);
        assert_eq!(parse_rounding(br#"{"int":"bad"}"#), None);
        assert_eq!(parse_rounding(b"not json"), None);
    }

    #[test]
    fn parses_fixture_themes() {
        let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("themes");
        for entry in fs::read_dir(&fixtures_dir).expect("fixtures dir") {
            let path = entry.expect("entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let text = fs::read_to_string(&path).expect("read fixture");
            let colors = parse_colors(&text);
            assert!(colors.accent.starts_with('#'));
            assert_eq!(colors.accent.len(), 7);
            assert!(matches!(
                accent_foreground(&colors.accent),
                "black" | "white"
            ));
        }
    }
}
