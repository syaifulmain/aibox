use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::color::{ensure_contrast, from_oklab, mix, to_oklab, Oklab, Rgb};
use super::palette::Palette;
use crate::util::paths::{atomic_write, home_dir};

const SVG_TEMPLATE: &str = include_str!("../../../../../pkgbuild/aibox.svg.tpl");

fn shift_oklab_l(c: Rgb, delta_l: f64) -> Rgb {
    let lab = to_oklab(c);
    from_oklab(Oklab {
        l: (lab.l + delta_l).clamp(0.0, 1.0),
        a: lab.a,
        b: lab.b,
    })
}

/// Renders the adaptive desktop icon SVG for `palette` by substituting
/// contrast-guarded color tokens into `pkgbuild/aibox.svg.tpl`.
pub fn render(palette: &Palette) -> String {
    let bg = palette.surface_canvas.rgb();
    let card = palette.surface_card.rgb();
    let input = palette.surface_input.rgb();
    let fg = palette.text_primary.rgb();
    let accent = palette.accent.rgb();
    let border_strong = palette.border_strong.rgb();
    let border_subtle = palette.border_subtle.rgb();

    let (bg_top, bg_bottom) = if palette.is_dark {
        (mix(card, fg, 0.08), input)
    } else {
        (bg, card)
    };

    let (box_fill, box_shadow) = if palette.is_dark {
        (input, mix(input, Rgb::BLACK, 0.10))
    } else {
        (input, mix(bg, fg, 0.12))
    };

    let frame_b = ensure_contrast(accent, box_fill, 3.0);
    let frame_a = ensure_contrast(shift_oklab_l(frame_b, 0.08), box_fill, 3.0);
    let frame_c = ensure_contrast(shift_oklab_l(frame_b, -0.08), box_fill, 3.0);
    let chevron_a = ensure_contrast(shift_oklab_l(frame_b, 0.10), box_fill, 3.0);
    let chevron_b = frame_b;
    let cursor = ensure_contrast(fg, box_fill, 4.5);

    SVG_TEMPLATE
        .replace("{{bg_top}}", &bg_top.to_hex())
        .replace("{{bg_bottom}}", &bg_bottom.to_hex())
        .replace("{{rim_top}}", &border_strong.to_hex())
        .replace("{{rim_bottom}}", &border_subtle.to_hex())
        .replace("{{frame_a}}", &frame_a.to_hex())
        .replace("{{frame_b}}", &frame_b.to_hex())
        .replace("{{frame_c}}", &frame_c.to_hex())
        .replace("{{chevron_a}}", &chevron_a.to_hex())
        .replace("{{chevron_b}}", &chevron_b.to_hex())
        .replace("{{box_shadow}}", &box_shadow.to_hex())
        .replace("{{box_fill}}", &box_fill.to_hex())
        .replace("{{cursor}}", &cursor.to_hex())
}

/// Returns `$XDG_DATA_HOME/icons/hicolor/scalable/apps/aibox.svg`
/// (defaulting `$XDG_DATA_HOME` to `$HOME/.local/share`).
pub fn default_icon_output_path() -> PathBuf {
    let data_home = match env::var_os("XDG_DATA_HOME") {
        Some(val) if !val.is_empty() => PathBuf::from(val),
        _ => home_dir().join(".local").join("share"),
    };
    data_home
        .join("icons")
        .join("hicolor")
        .join("scalable")
        .join("apps")
        .join("aibox.svg")
}

/// Renders the SVG icon for `palette` and atomically writes it to `output_path`.
///
/// Returns `Ok(true)` if the file was written, or `Ok(false)` if the existing
/// file already had identical contents (avoiding unnecessary watcher/launcher churn).
pub fn write_icon(palette: &Palette, output_path: &Path) -> Result<bool, String> {
    let svg = render(palette);
    if let Ok(existing) = fs::read_to_string(output_path) {
        if existing == svg {
            return Ok(false);
        }
    }

    atomic_write(output_path, svg.as_bytes())?;
    invalidate_hicolor_cache(output_path);
    Ok(true)
}

fn invalidate_hicolor_cache(output_path: &Path) {
    let hicolor_dir = output_path
        .ancestors()
        .find(|p| p.file_name().and_then(|n| n.to_str()) == Some("hicolor"));
    let Some(hicolor_dir) = hicolor_dir else {
        return;
    };
    if !hicolor_dir.is_dir() {
        return;
    }

    let _ = Command::new("touch")
        .arg(hicolor_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    if hicolor_dir.join("index.theme").is_file() || hicolor_dir.join("icon-theme.cache").is_file() {
        let _ = Command::new("gtk-update-icon-cache")
            .arg("-q")
            .arg(hicolor_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{contrast_ratio, is_near_gray, load_palette, oklab_delta_e};

    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("themes")
    }

    fn extract_svg_colors(svg: &str) -> Vec<Rgb> {
        let mut colors = Vec::new();
        for part in svg.split('#').skip(1) {
            if part.len() >= 6 {
                let candidate = format!("#{}", &part[..6]);
                if let Some(rgb) = Rgb::from_hex(&candidate) {
                    colors.push(rgb);
                }
            }
        }
        colors
    }

    #[test]
    fn tokyo_night_icon_matches_reference_design() {
        let palette = load_palette(&fixture_dir().join("tokyo-night.toml"));
        let svg = render(&palette);

        assert!(!svg.contains("{{"));
        assert!(!svg.contains("}}"));
        assert!(svg.starts_with("<svg "));
        assert!(svg.contains("viewBox=\"0 0 512 512\""));

        // Reference Tokyo Night SVG uses #252837 -> #13151d bg, #7aa2f7 accent, #10121a box.
        let colors = extract_svg_colors(&svg);
        // bg_top, bg_bottom, rim_top, rim_bottom, frame_a, frame_b, frame_c, chevron_a, chevron_b, box_shadow, box_fill, cursor
        assert_eq!(colors.len(), 12);

        let bg_top = colors[0];
        let frame_b = colors[5];
        let box_fill = colors[10];
        let cursor = colors[11];

        let ref_bg_top = Rgb::from_hex("#252837").expect("hex");
        let ref_accent = Rgb::from_hex("#7aa2f7").expect("hex");
        let ref_box = Rgb::from_hex("#10121a").expect("hex");

        assert!(oklab_delta_e(bg_top, ref_bg_top) < 0.08);
        assert_eq!(frame_b, ref_accent);
        assert!(oklab_delta_e(box_fill, ref_box) < 0.05);
        assert!(contrast_ratio(frame_b, box_fill) >= 3.0);
        assert!(contrast_ratio(cursor, box_fill) >= 4.5);
    }

    #[test]
    fn all_23_omarchy_themes_render_valid_contrast_icons() {
        let dir = fixture_dir();
        let mut count = 0;
        for entry in fs::read_dir(&dir).expect("read themes dir") {
            let path = entry.expect("entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            count += 1;
            let palette = load_palette(&path);
            let svg = render(&palette);
            assert!(!svg.contains("{{"), "unresolved placeholder in {:?}", path);
            let colors = extract_svg_colors(&svg);
            assert_eq!(colors.len(), 12, "expected 12 hex colors in {:?}", path);

            let box_fill = colors[10];
            let frame_a = colors[4];
            let frame_b = colors[5];
            let frame_c = colors[6];
            let chevron_a = colors[7];
            let chevron_b = colors[8];
            let cursor = colors[11];

            for (label, fg) in [
                ("frame_a", frame_a),
                ("frame_b", frame_b),
                ("frame_c", frame_c),
                ("chevron_a", chevron_a),
                ("chevron_b", chevron_b),
            ] {
                assert!(
                    contrast_ratio(fg, box_fill) >= 3.0,
                    "{:?} {} contrast too low: {}",
                    path,
                    label,
                    contrast_ratio(fg, box_fill)
                );
            }
            assert!(
                contrast_ratio(cursor, box_fill) >= 4.5,
                "{:?} cursor contrast too low",
                path
            );
        }
        assert!(count >= 23, "expected at least 23 theme fixtures");
    }

    #[test]
    fn white_monochrome_theme_stays_achromatic() {
        let palette = load_palette(&fixture_dir().join("white.toml"));
        let svg = render(&palette);
        let colors = extract_svg_colors(&svg);
        for c in colors {
            assert!(is_near_gray(c), "expected monochrome color, got {:?}", c);
        }
    }

    #[test]
    fn write_icon_is_atomic_and_skips_identical_content() {
        let tmp = env::temp_dir().join(format!(
            "aibox-icon-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let out_path = tmp.join("aibox.svg");

        let dark = load_palette(&fixture_dir().join("tokyo-night.toml"));
        let light = load_palette(&fixture_dir().join("catppuccin-latte.toml"));

        assert_eq!(write_icon(&dark, &out_path), Ok(true));
        assert!(out_path.is_file());
        assert_eq!(write_icon(&dark, &out_path), Ok(false));
        assert_eq!(write_icon(&light, &out_path), Ok(true));
        assert_eq!(write_icon(&light, &out_path), Ok(false));

        let _ = fs::remove_dir_all(&tmp);
    }
}
