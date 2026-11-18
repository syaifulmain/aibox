use std::sync::OnceLock;

use regex::Regex;

/// 8-bit sRGB color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
    };

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parses `#RRGGBB` into an `Rgb` value.
    pub fn from_hex(hex: &str) -> Option<Self> {
        let caps = hex_re().captures(hex.trim())?;
        let digits = caps.get(1)?.as_str();
        let r = u8::from_str_radix(&digits[0..2], 16).ok()?;
        let g = u8::from_str_radix(&digits[2..4], 16).ok()?;
        let b = u8::from_str_radix(&digits[4..6], 16).ok()?;
        Some(Self { r, g, b })
    }

    /// Formats the color as lowercase `#rrggbb`.
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Formats the color as `#aarrggbb` for QML `color` properties.
    pub fn to_qml_argb(self, alpha: f64) -> String {
        let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!("#{:02x}{:02x}{:02x}{:02x}", a, self.r, self.g, self.b)
    }
}

/// Perceptual OKLab color coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklab {
    pub l: f64,
    pub a: f64,
    pub b: f64,
}

fn hex_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^#([0-9a-fA-F]{6})$").expect("valid hex regex"))
}

fn srgb_to_linear(v: u8) -> f64 {
    let s = v as f64 / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f64) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let v = if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (v * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Computes WCAG 2.x relative luminance in `[0.0, 1.0]`.
pub fn relative_luminance(c: Rgb) -> f64 {
    0.2126 * srgb_to_linear(c.r) + 0.7152 * srgb_to_linear(c.g) + 0.0722 * srgb_to_linear(c.b)
}

/// Computes WCAG 2.x contrast ratio between two colors in `[1.0, 21.0]`.
pub fn contrast_ratio(a: Rgb, b: Rgb) -> f64 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Converts sRGB to perceptual OKLab.
pub fn to_oklab(c: Rgb) -> Oklab {
    let r = srgb_to_linear(c.r);
    let g = srgb_to_linear(c.g);
    let b = srgb_to_linear(c.b);

    let l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
    let m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
    let s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;

    let l_ = l.cbrt();
    let m_ = m.cbrt();
    let s_ = s.cbrt();

    Oklab {
        l: 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
        a: 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
        b: 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
    }
}

/// Converts OKLab back to sRGB, clamping out-of-gamut channels.
pub fn from_oklab(lab: Oklab) -> Rgb {
    let l_ = lab.l + 0.3963377774 * lab.a + 0.2158037573 * lab.b;
    let m_ = lab.l - 0.1055613458 * lab.a - 0.0638541728 * lab.b;
    let s_ = lab.l - 0.0894841775 * lab.a - 1.2914855480 * lab.b;

    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;

    let r = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
    let g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
    let b = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s;

    Rgb {
        r: linear_to_srgb(r),
        g: linear_to_srgb(g),
        b: linear_to_srgb(b),
    }
}

/// Linearly interpolates between `a` and `b` in OKLab space.
pub fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let la = to_oklab(a);
    let lb = to_oklab(b);
    from_oklab(Oklab {
        l: la.l + (lb.l - la.l) * t,
        a: la.a + (lb.a - la.a) * t,
        b: la.b + (lb.b - la.b) * t,
    })
}

/// Euclidean distance between two colors in OKLab space.
pub fn oklab_delta_e(a: Rgb, b: Rgb) -> f64 {
    let la = to_oklab(a);
    let lb = to_oklab(b);
    let dl = la.l - lb.l;
    let da = la.a - lb.a;
    let db = la.b - lb.b;
    (dl * dl + da * da + db * db).sqrt()
}

/// Returns `true` when the color's OKLab chroma is below `0.02` (achromatic / gray).
pub fn is_near_gray(c: Rgb) -> bool {
    let lab = to_oklab(c);
    (lab.a * lab.a + lab.b * lab.b).sqrt() < 0.02
}

/// Shifts the OKLab lightness of `fg` away from `bg` while preserving hue/chroma
/// until `contrast_ratio(result, bg) >= min`, clamping at black or white.
pub fn ensure_contrast(fg: Rgb, bg: Rgb, min: f64) -> Rgb {
    if contrast_ratio(fg, bg) >= min {
        return fg;
    }

    let fg_lab = to_oklab(fg);
    let bg_lab = to_oklab(bg);
    let go_lighter = contrast_ratio(Rgb::WHITE, bg) >= contrast_ratio(Rgb::BLACK, bg);

    if go_lighter {
        if contrast_ratio(Rgb::WHITE, bg) < min {
            return Rgb::WHITE;
        }
        let mut lo = fg_lab.l.max(bg_lab.l).clamp(0.0, 1.0);
        let mut hi = 1.0;
        let mut best = Rgb::WHITE;
        for _ in 0..16 {
            let mid = (lo + hi) * 0.5;
            let cand = from_oklab(Oklab {
                l: mid,
                a: fg_lab.a,
                b: fg_lab.b,
            });
            if contrast_ratio(cand, bg) >= min {
                best = cand;
                hi = mid;
            } else {
                lo = mid;
            }
        }
        if contrast_ratio(best, bg) >= min {
            best
        } else {
            Rgb::WHITE
        }
    } else {
        if contrast_ratio(Rgb::BLACK, bg) < min {
            return Rgb::BLACK;
        }
        let mut lo = 0.0;
        let mut hi = fg_lab.l.min(bg_lab.l).clamp(0.0, 1.0);
        let mut best = Rgb::BLACK;
        for _ in 0..16 {
            let mid = (lo + hi) * 0.5;
            let cand = from_oklab(Oklab {
                l: mid,
                a: fg_lab.a,
                b: fg_lab.b,
            });
            if contrast_ratio(cand, bg) >= min {
                best = cand;
                lo = mid;
            } else {
                hi = mid;
            }
        }
        if contrast_ratio(best, bg) >= min {
            best
        } else {
            Rgb::BLACK
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wcag_black_white_reference() {
        assert!((relative_luminance(Rgb::BLACK) - 0.0).abs() < 1e-6);
        assert!((relative_luminance(Rgb::WHITE) - 1.0).abs() < 1e-6);
        assert!((contrast_ratio(Rgb::BLACK, Rgb::WHITE) - 21.0).abs() < 1e-6);
        assert!((contrast_ratio(Rgb::WHITE, Rgb::BLACK) - 21.0).abs() < 1e-6);
        assert!((contrast_ratio(Rgb::BLACK, Rgb::BLACK) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn hex_and_qml_argb_roundtrip() {
        let c = Rgb::from_hex("#7aa2f7").expect("valid hex");
        assert_eq!(c, Rgb::new(0x7a, 0xa2, 0xf7));
        assert_eq!(c.to_hex(), "#7aa2f7");
        assert_eq!(Rgb::BLACK.to_qml_argb(0.28), "#47000000");
        assert_eq!(Rgb::from_hex("#bad"), None);
    }

    #[test]
    fn oklab_roundtrip_and_gray_detection() {
        for hex in ["#1a1b26", "#7aa2f7", "#eff1f5", "#f7768e", "#6e6e6e"] {
            let c = Rgb::from_hex(hex).expect("hex");
            let back = from_oklab(to_oklab(c));
            assert!(oklab_delta_e(c, back) < 0.005, "roundtrip for {hex}");
        }
        assert!(is_near_gray(Rgb::from_hex("#6e6e6e").expect("gray")));
        assert!(is_near_gray(Rgb::from_hex("#2a2a2a").expect("gray")));
        assert!(!is_near_gray(Rgb::from_hex("#7aa2f7").expect("blue")));
    }

    #[test]
    fn ensure_contrast_fixes_low_contrast_pairs() {
        // Light mode muted gray on white
        let muted = Rgb::from_hex("#c0c0c0").expect("muted");
        let fixed = ensure_contrast(muted, Rgb::WHITE, 4.5);
        assert!(contrast_ratio(fixed, Rgb::WHITE) >= 4.5);
        assert!(is_near_gray(fixed));

        // Yellow warning on catppuccin-latte card
        let yellow = Rgb::from_hex("#df8e1d").expect("yellow");
        let latte_card = Rgb::from_hex("#e3e4e8").expect("card");
        let fixed_yellow = ensure_contrast(yellow, latte_card, 4.5);
        assert!(contrast_ratio(fixed_yellow, latte_card) >= 4.5);

        // Already passing color is returned unchanged
        let bright = Rgb::from_hex("#c0caf5").expect("bright");
        let dark_card = Rgb::from_hex("#13141c").expect("dark card");
        assert_eq!(ensure_contrast(bright, dark_card, 7.0), bright);
    }
}
