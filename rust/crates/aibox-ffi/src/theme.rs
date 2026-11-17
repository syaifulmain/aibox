use aibox_core::theme;

#[cxx::bridge(namespace = "aibox::ffi")]
mod ffi {
    pub struct ThemeColorsFfi {
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

    pub struct RoundingResultFfi {
        pub valid: bool,
        pub value: i32,
    }

    extern "Rust" {
        fn theme_read_key(path: &str, key: &str, fallback: &str) -> String;
        fn theme_read_accent(path: &str) -> String;
        fn theme_parse_colors_file(path: &str) -> ThemeColorsFfi;
        fn theme_accent_foreground(accent: &str) -> String;
        fn theme_parse_rounding(stdout: &[u8]) -> RoundingResultFfi;
    }
}

fn theme_read_key(path: &str, key: &str, fallback: &str) -> String {
    theme::read_key_file(path, key, fallback)
}

fn theme_read_accent(path: &str) -> String {
    theme::read_accent_file(path)
}

fn theme_parse_colors_file(path: &str) -> ffi::ThemeColorsFfi {
    let c = theme::parse_colors_file(path);
    ffi::ThemeColorsFfi {
        accent: c.accent,
        background: c.background,
        dark_background: c.dark_background,
        lighter_background: c.lighter_background,
        foreground: c.foreground,
        muted: c.muted,
        green: c.green,
        red: c.red,
        yellow: c.yellow,
    }
}

fn theme_accent_foreground(accent: &str) -> String {
    theme::accent_foreground(accent).to_string()
}

fn theme_parse_rounding(stdout: &[u8]) -> ffi::RoundingResultFfi {
    match theme::parse_rounding(stdout) {
        Some(value) => ffi::RoundingResultFfi { valid: true, value },
        None => ffi::RoundingResultFfi {
            valid: false,
            value: 0,
        },
    }
}
