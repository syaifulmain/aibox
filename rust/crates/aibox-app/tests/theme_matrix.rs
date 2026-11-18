use std::ffi::CString;
use std::fs;
use std::os::raw::{c_char, c_int};
use std::path::{Path, PathBuf};

use aibox_app as _;
use aibox_core::theme::{derive_palette, icon, parse_colors};

extern "C" {
    fn aibox_test_init_gui_app();
    fn aibox_test_shutdown_gui_app();
    fn aibox_test_render_theme_matrix(
        theme_dir: *const c_char,
        out_dir: *const c_char,
        err_buf: *mut c_char,
        err_buf_len: c_int,
    ) -> c_int;
    fn aibox_test_gallery_theme_matrix(
        theme_dirs_newline_sep: *const c_char,
        err_buf: *mut c_char,
        err_buf_len: c_int,
    ) -> c_int;
}

const MATRIX_THEMES: &[&str] = &[
    "tokyo-night",
    "catppuccin-latte",
    "flexoki-light",
    "white",
    "rose-pine",
    "vantablack",
    "gruvbox",
];

const SCREENS: &[&str] = &[
    "main.png",
    "settings.png",
    "create.png",
    "wipe.png",
    "gallery.png",
];

struct TempDir(PathBuf);

impl TempDir {
    fn new(prefix: &str) -> Self {
        let mut p = std::env::temp_dir();
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        p.push(format!("aibox-theme-matrix-{prefix}-{pid}-{nanos}"));
        fs::create_dir_all(&p).expect("create temp dir");
        Self(p)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn render_theme(fixtures_dir: &Path, matrix_root: &Path, theme_name: &str) -> bool {
    let fixture_path = fixtures_dir.join(format!("{theme_name}.toml"));
    let toml_text = fs::read_to_string(&fixture_path)
        .unwrap_or_else(|e| panic!("read fixture {}: {e}", fixture_path.display()));
    let palette = derive_palette(&parse_colors(&toml_text));

    let tmp = TempDir::new(theme_name);
    let theme_subdir = tmp.0.join("theme");
    fs::create_dir_all(&theme_subdir).expect("create theme subdir");
    fs::write(theme_subdir.join("colors.toml"), &toml_text).expect("write colors.toml");

    let theme_out = matrix_root.join(theme_name);
    fs::create_dir_all(&theme_out).expect("create theme output dir");

    let svg = icon::render(&palette);
    fs::write(theme_out.join("icon.svg"), svg).expect("write icon.svg");

    let c_theme = CString::new(tmp.0.to_string_lossy().as_ref()).expect("theme CString");
    let c_out = CString::new(theme_out.to_string_lossy().as_ref()).expect("out CString");
    let mut buf = vec![0u8; 8192];
    let rc = unsafe {
        aibox_test_render_theme_matrix(
            c_theme.as_ptr(),
            c_out.as_ptr(),
            buf.as_mut_ptr() as *mut c_char,
            buf.len() as c_int,
        )
    };
    if rc != 0 {
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        let msg = String::from_utf8_lossy(&buf[..end]);
        panic!("theme_matrix render failed for {theme_name}: {msg}");
    }

    for screen in SCREENS {
        let p = theme_out.join(screen);
        assert!(p.exists(), "missing rendered screenshot {}", p.display());
    }
    palette.is_dark
}

fn write_index_html(matrix_root: &Path, rendered: &[(&str, bool)]) {
    let mut html = String::from(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <title>AIBox Adaptive Theme Matrix</title>\n<style>\n\
         body { font-family: system-ui, sans-serif; background: #14151a; color: #e6e8ef; margin: 24px; }\n\
         h1 { margin-bottom: 8px; }\n\
         p.sub { color: #9aa0b4; margin-top: 0; margin-bottom: 24px; }\n\
         .theme { margin-bottom: 32px; padding: 16px; border: 1px solid #2a2d3a; border-radius: 10px; background: #1a1c24; }\n\
         .theme-head { display: flex; align-items: center; gap: 12px; margin-bottom: 12px; }\n\
         .theme-head img.icon { width: 32px; height: 32px; }\n\
         .badge { font-size: 12px; padding: 2px 8px; border-radius: 999px; background: #2a2d3a; }\n\
         .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 12px; }\n\
         figure { margin: 0; background: #101117; border: 1px solid #262936; border-radius: 8px; overflow: hidden; }\n\
         figure img { width: 100%; height: auto; display: block; }\n\
         figcaption { padding: 6px 10px; font-size: 12px; color: #b4bacb; }\n\
         </style>\n</head>\n<body>\n<h1>AIBox Adaptive Theme Matrix</h1>\n\
         <p class=\"sub\">Generated offscreen via <code>cargo test --test theme_matrix -- --ignored</code>.</p>\n",
    );

    for (name, is_dark) in rendered {
        let mode = if *is_dark { "dark" } else { "light" };
        html.push_str(&format!(
            "<section class=\"theme\">\n<div class=\"theme-head\">\
             <img class=\"icon\" src=\"{name}/icon.svg\" alt=\"{name} icon\">\
             <strong>{name}</strong><span class=\"badge\">{mode}</span></div>\n<div class=\"grid\">\n"
        ));
        for screen in SCREENS {
            html.push_str(&format!(
                "<figure><a href=\"{name}/{screen}\"><img src=\"{name}/{screen}\" alt=\"{name} {screen}\"></a>\
                 <figcaption>{screen}</figcaption></figure>\n"
            ));
        }
        html.push_str("</div>\n</section>\n");
    }

    html.push_str("</body>\n</html>\n");
    fs::write(matrix_root.join("index.html"), html).expect("write index.html");
}

#[test]
#[ignore = "slow offscreen screenshot matrix; run explicitly with --ignored"]
fn generate_theme_screenshot_matrix() {
    let _ = aibox_app::ensure_cxx_qt_linked();
    unsafe { aibox_test_init_gui_app() };

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixtures_dir = manifest_dir.join("../aibox-core/tests/fixtures/themes");
    let matrix_root = manifest_dir.join("../../target/theme-matrix");
    fs::create_dir_all(&matrix_root).expect("create target/theme-matrix");

    let mut rendered = Vec::with_capacity(MATRIX_THEMES.len());
    for &theme_name in MATRIX_THEMES {
        let is_dark = render_theme(&fixtures_dir, &matrix_root, theme_name);
        rendered.push((theme_name, is_dark));
    }
    write_index_html(&matrix_root, &rendered);

    unsafe { aibox_test_shutdown_gui_app() };
}

#[test]
fn gallery_renders_all_23_themes_light_dark_high_contrast_without_warnings() {
    let _ = aibox_app::ensure_cxx_qt_linked();
    unsafe { aibox_test_init_gui_app() };

    let fixtures_dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../aibox-core/tests/fixtures/themes");
    let mut toml_files: Vec<_> = fs::read_dir(&fixtures_dir)
        .expect("read theme fixtures dir")
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("toml"))
        .collect();
    toml_files.sort();
    assert_eq!(
        toml_files.len(),
        23,
        "expected 23 Omarchy theme fixtures in {}",
        fixtures_dir.display()
    );

    let tmp_root = TempDir::new("gallery-23");
    let mut theme_dirs = Vec::with_capacity(toml_files.len());
    for (idx, path) in toml_files.iter().enumerate() {
        let dir = tmp_root.0.join(format!("t{idx:02}"));
        let theme_subdir = dir.join("theme");
        fs::create_dir_all(&theme_subdir).expect("create theme subdir");
        fs::copy(path, theme_subdir.join("colors.toml")).expect("copy colors.toml");
        theme_dirs.push(dir.to_string_lossy().into_owned());
    }

    let joined = CString::new(theme_dirs.join("\n")).expect("dirs CString");
    let mut buf = vec![0u8; 16384];
    let rc = unsafe {
        aibox_test_gallery_theme_matrix(
            joined.as_ptr(),
            buf.as_mut_ptr() as *mut c_char,
            buf.len() as c_int,
        )
    };
    unsafe { aibox_test_shutdown_gui_app() };

    if rc != 0 {
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        let msg = String::from_utf8_lossy(&buf[..end]);
        panic!("gallery theme matrix failed ({rc} warnings/errors):\n{msg}");
    }
}
