use std::fs;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeColors {
    pub accent: String,
    pub background: String,
    pub dark_background: String,
    pub lighter_background: String,
    pub foreground: String,
    pub muted: String,
    pub green: String,
    pub red: String,
    pub yellow: String,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            accent: "#FFD60A".to_string(),
            background: "#0e0e10".to_string(),
            dark_background: "#13141c".to_string(),
            lighter_background: "#1e202e".to_string(),
            foreground: "#c0caf5".to_string(),
            muted: "#565f89".to_string(),
            green: "#9ece6a".to_string(),
            red: "#f7768e".to_string(),
            yellow: "#e0af68".to_string(),
        }
    }
}

/// Reads a `#RRGGBB` hex color for `key` from Omarchy `colors.toml` text,
/// falling back to `fallback` when missing or malformed.
pub fn read_key(toml_text: &str, key: &str, fallback: &str) -> String {
    let pattern = format!(
        r#"(?m)^\s*{}\s*=\s*["'](#[0-9a-fA-F]{{6}})["']"#,
        regex::escape(key)
    );
    if let Ok(re) = Regex::new(&pattern) {
        if let Some(caps) = re.captures(toml_text) {
            if let Some(m) = caps.get(1) {
                return m.as_str().to_string();
            }
        }
    }
    fallback.to_string()
}

/// Reads `accent` from `colors.toml` text with `#FFD60A` fallback.
pub fn read_accent(toml_text: &str) -> String {
    read_key(toml_text, "accent", "#FFD60A")
}

/// Parses all theme colors from `colors.toml` text.
pub fn parse_colors(toml_text: &str) -> ThemeColors {
    let defaults = ThemeColors::default();
    let fg_fallback = read_key(toml_text, "foreground", &defaults.foreground);
    ThemeColors {
        accent: read_accent(toml_text),
        background: read_key(toml_text, "background", &defaults.background),
        dark_background: read_key(toml_text, "dark_background", &defaults.dark_background),
        lighter_background: read_key(
            toml_text,
            "lighter_background",
            &defaults.lighter_background,
        ),
        foreground: read_key(toml_text, "bright_foreground", &fg_fallback),
        muted: read_key(toml_text, "dark_foreground", &defaults.muted),
        green: read_key(toml_text, "green", &defaults.green),
        red: read_key(toml_text, "red", &defaults.red),
        yellow: read_key(toml_text, "yellow", &defaults.yellow),
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

fn hex_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^#([0-9a-fA-F]{6})$").expect("valid hex regex"))
}

fn parse_rgb_f64(hex: &str) -> Option<(f64, f64, f64)> {
    let caps = hex_re().captures(hex.trim())?;
    let digits = caps.get(1)?.as_str();
    let r = u8::from_str_radix(&digits[0..2], 16).ok()? as f64 / 255.0;
    let g = u8::from_str_radix(&digits[2..4], 16).ok()? as f64 / 255.0;
    let b = u8::from_str_radix(&digits[4..6], 16).ok()? as f64 / 255.0;
    Some((r, g, b))
}

/// Returns `"black"` or `"white"`, whichever has better contrast on `accent`
/// according to WCAG relative luminance.
pub fn accent_foreground(accent_hex: &str) -> &'static str {
    let Some((r, g, b)) = parse_rgb_f64(accent_hex) else {
        return "white";
    };
    let linear = |v: f64| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance = 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b);
    if luminance > 0.179 {
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
