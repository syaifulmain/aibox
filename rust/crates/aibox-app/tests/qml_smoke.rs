use std::ffi::CString;
use std::fs;
use std::os::raw::{c_char, c_int};
use std::path::{Path, PathBuf};

use aibox_app as _;

extern "C" {
    fn aibox_test_init_gui_app();
    fn aibox_test_shutdown_gui_app();
    fn aibox_test_qml_smoke_with_theme(
        theme_dir: *const c_char,
        err_buf: *mut c_char,
        err_buf_len: c_int,
    ) -> c_int;
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(prefix: &str) -> Self {
        let mut p = std::env::temp_dir();
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        p.push(format!("aibox-qml-smoke-{prefix}-{pid}-{nanos}"));
        fs::create_dir_all(&p).expect("create temp dir");
        Self(p)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run_smoke_with_fixture(fixture_name: &str) {
    let fixtures_dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../aibox-core/tests/fixtures/themes");
    let tmp = TempDir::new(fixture_name);
    let theme_subdir = tmp.0.join("theme");
    fs::create_dir_all(&theme_subdir).unwrap();
    fs::copy(
        fixtures_dir.join(format!("{fixture_name}.toml")),
        theme_subdir.join("colors.toml"),
    )
    .unwrap();

    let c_dir = CString::new(tmp.0.to_string_lossy().as_ref()).unwrap();
    let mut buf = vec![0u8; 16384];
    let rc = unsafe {
        aibox_test_qml_smoke_with_theme(
            c_dir.as_ptr(),
            buf.as_mut_ptr() as *mut c_char,
            buf.len() as c_int,
        )
    };
    if rc != 0 {
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        let msg = String::from_utf8_lossy(&buf[..end]);
        eprintln!("QML smoke test ({fixture_name}) failed ({rc} warnings/errors):\n{msg}");
        std::process::exit(1);
    }
}

fn main() {
    let _ = aibox_app::ensure_cxx_qt_linked();
    unsafe { aibox_test_init_gui_app() };
    run_smoke_with_fixture("tokyo-night");
    run_smoke_with_fixture("catppuccin-latte");
    unsafe { aibox_test_shutdown_gui_app() };
    println!("running 2 tests");
    println!("test qml_smoke_dark_tokyo_night_zero_warnings ... ok");
    println!("test qml_smoke_light_catppuccin_latte_zero_warnings ... ok");
    println!("test result: ok. 2 passed; 0 failed");
}
