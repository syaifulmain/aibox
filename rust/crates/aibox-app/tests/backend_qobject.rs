use std::ffi::CString;
use std::fs;
use std::os::raw::{c_char, c_int, c_void};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use aibox_app as _;

extern "C" {
    fn aibox_test_init_gui_app();
    fn aibox_test_shutdown_gui_app();
    fn aibox_test_process_events(max_ms: c_int);

    fn aibox_test_backend_new() -> *mut c_void;
    fn aibox_test_backend_free(ptr: *mut c_void);
    fn aibox_test_backend_configure(ptr: *mut c_void, path: *const c_char, project_mode: bool);
    fn aibox_test_backend_count(ptr: *mut c_void) -> c_int;
    fn aibox_test_backend_primary_action_with_spy(ptr: *mut c_void) -> c_int;
    fn aibox_test_backend_enable_file_access(
        ptr: *mut c_void,
        name: *const c_char,
        buf: *mut c_char,
        buf_len: c_int,
    ) -> c_int;
    fn aibox_test_backend_disable_file_access(ptr: *mut c_void, name: *const c_char);
    fn aibox_test_backend_open_file_access(ptr: *mut c_void, name: *const c_char);
    fn aibox_test_backend_busy(ptr: *mut c_void) -> bool;
    fn aibox_test_backend_selected_env_empty(ptr: *mut c_void) -> bool;
    fn aibox_test_backend_env_count(ptr: *mut c_void) -> c_int;
    fn aibox_test_backend_enter_environment(ptr: *mut c_void, name: *const c_char);

    fn aibox_test_theme_new() -> *mut c_void;
    fn aibox_test_theme_free(ptr: *mut c_void);
    fn aibox_test_theme_configure(ptr: *mut c_void, dir: *const c_char);
    fn aibox_test_theme_is_dark(ptr: *mut c_void) -> bool;
    fn aibox_test_theme_string_prop(
        ptr: *mut c_void,
        prop_name: *const c_char,
        buf: *mut c_char,
        buf_len: c_int,
    ) -> c_int;
    fn aibox_test_theme_spy_new(ptr: *mut c_void) -> *mut c_void;
    fn aibox_test_theme_spy_count(spy: *mut c_void) -> c_int;
    fn aibox_test_theme_spy_free(spy: *mut c_void);
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
        p.push(format!("aibox-test-{prefix}-{pid}-{nanos}"));
        fs::create_dir_all(&p).expect("create temp dir");
        Self(p)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct BackendHandle(*mut c_void);

impl BackendHandle {
    fn new() -> Self {
        let ptr = unsafe { aibox_test_backend_new() };
        assert!(!ptr.is_null());
        Self(ptr)
    }

    fn configure(&self, path: &Path, project_mode: bool) {
        let c_path = CString::new(path.to_string_lossy().as_ref()).unwrap();
        unsafe { aibox_test_backend_configure(self.0, c_path.as_ptr(), project_mode) };
    }

    fn count(&self) -> i32 {
        unsafe { aibox_test_backend_count(self.0) }
    }

    fn primary_action_with_spy(&self) -> i32 {
        unsafe { aibox_test_backend_primary_action_with_spy(self.0) }
    }

    fn enable_file_access(&self, name: &str) -> String {
        let c_name = CString::new(name).unwrap();
        let mut buf = vec![0u8; 4096];
        let len = unsafe {
            aibox_test_backend_enable_file_access(
                self.0,
                c_name.as_ptr(),
                buf.as_mut_ptr() as *mut c_char,
                buf.len() as c_int,
            )
        };
        let n = (len as usize).min(buf.len().saturating_sub(1));
        String::from_utf8_lossy(&buf[..n]).to_string()
    }

    fn disable_file_access(&self, name: &str) {
        let c_name = CString::new(name).unwrap();
        unsafe { aibox_test_backend_disable_file_access(self.0, c_name.as_ptr()) };
    }

    fn open_file_access(&self, name: &str) {
        let c_name = CString::new(name).unwrap();
        unsafe { aibox_test_backend_open_file_access(self.0, c_name.as_ptr()) };
    }

    fn busy(&self) -> bool {
        unsafe { aibox_test_backend_busy(self.0) }
    }

    fn selected_env_empty(&self) -> bool {
        unsafe { aibox_test_backend_selected_env_empty(self.0) }
    }

    fn env_count(&self) -> i32 {
        unsafe { aibox_test_backend_env_count(self.0) }
    }

    fn enter_environment(&self, name: &str) {
        let c_name = CString::new(name).unwrap();
        unsafe { aibox_test_backend_enter_environment(self.0, c_name.as_ptr()) };
    }
}

impl Drop for BackendHandle {
    fn drop(&mut self) {
        unsafe { aibox_test_backend_free(self.0) };
    }
}

fn write_executable(path: &Path, content: &str) {
    fs::write(path, content).expect("write executable");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod +x");
}

fn test_primary_action_counts() {
    let dir = TempDir::new("primary-action");
    let backend = BackendHandle::new();
    backend.configure(dir.path(), false);
    assert_eq!(backend.count(), 0);
    let fired = backend.primary_action_with_spy();
    assert_eq!(backend.count(), 1);
    assert_eq!(fired, 1);
}

fn test_enable_file_access_requires_known_running_environment() {
    let dir = TempDir::new("file-access");
    let backend = BackendHandle::new();
    backend.configure(dir.path(), false);
    let err = backend.enable_file_access("missing-box");
    assert!(!err.is_empty(), "expected error for unknown environment");
    assert!(!backend.busy(), "busy should remain false on error");
    assert!(backend.selected_env_empty());

    backend.disable_file_access("missing-box");
    backend.open_file_access("missing-box");
    assert!(!backend.busy());
}

fn test_enter_environment_returns_without_blocking() {
    let dir = TempDir::new("enter-nonblock");
    let stub_dir = dir.path().join("bin");
    let proj_dir = dir.path().join("proj");
    let ws_base = proj_dir.join(".aibox/workspaces");
    let state_dir = proj_dir.join(".aibox/state");
    fs::create_dir_all(&stub_dir).unwrap();
    fs::create_dir_all(ws_base.join("dev")).unwrap();
    fs::create_dir_all(&state_dir).unwrap();
    fs::write(
        proj_dir.join(".aibox/config.json"),
        r#"{"firstRunCompleted":true}"#,
    )
    .unwrap();

    let list_json_file = dir.path().join("list.json");
    let list_json = format!(
        r#"[
  {{
    "name": "dev",
    "type": "container",
    "status": "Stopped",
    "config": {{}},
    "expanded_devices": {{ "workspace": {{ "source": "{}/dev", "path": "/home/workspace" }} }},
    "snapshots": []
  }}
]"#,
        ws_base.display()
    );
    fs::write(&list_json_file, list_json).unwrap();

    // Slow incus info keeps the preparation step running for 3 seconds.
    let incus_stub = format!(
        r#"#!/bin/sh
if [ "$1" = "list" ]; then
    cat "{}"
    exit 0
fi
if [ "$1" = "info" ]; then
    sleep 3
    echo "Status: RUNNING"
    exit 0
fi
exit 0
"#,
        list_json_file.display()
    );
    write_executable(&stub_dir.join("incus"), &incus_stub);

    let marker_path = dir.path().join("terminal-opened");
    let term_stub = format!("#!/bin/sh\necho \"$@\" > '{}'\n", marker_path.display());
    write_executable(&stub_dir.join("xdg-terminal-exec"), &term_stub);

    let old_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{old_path}", stub_dir.display());
    std::env::set_var("PATH", &new_path);

    let backend = BackendHandle::new();
    backend.configure(&proj_dir, true);

    let deadline = Instant::now() + Duration::from_secs(5);
    while backend.env_count() == 0 && Instant::now() < deadline {
        unsafe { aibox_test_process_events(50) };
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        backend.env_count() > 0,
        "expected refresh to populate 'dev' environment"
    );

    // Also verify enableFileAccess rejects a stopped environment with busy == false.
    let stopped_err = backend.enable_file_access("dev");
    assert!(
        !stopped_err.is_empty(),
        "expected error when enabling file access on stopped environment"
    );
    assert!(!backend.busy());

    let start = Instant::now();
    backend.enter_environment("dev");
    let call_elapsed = start.elapsed();
    assert!(
        call_elapsed < Duration::from_millis(100),
        "enter_environment blocked the calling thread for {:?}",
        call_elapsed
    );

    let launch_deadline = Instant::now() + Duration::from_secs(15);
    while !marker_path.exists() && Instant::now() < launch_deadline {
        unsafe { aibox_test_process_events(50) };
        std::thread::sleep(Duration::from_millis(25));
    }
    std::env::set_var("PATH", old_path);

    assert!(
        marker_path.exists(),
        "terminal launch marker was not created within timeout"
    );
    let launched = fs::read_to_string(&marker_path).unwrap();
    assert!(
        launched.contains("incus exec"),
        "expected 'incus exec' in launched command, got: {launched}"
    );
}

struct ThemeHandle(*mut c_void);

impl ThemeHandle {
    fn new() -> Self {
        let ptr = unsafe { aibox_test_theme_new() };
        assert!(!ptr.is_null());
        Self(ptr)
    }

    fn configure(&self, dir: &Path) {
        let c_dir = CString::new(dir.to_string_lossy().as_ref()).unwrap();
        unsafe { aibox_test_theme_configure(self.0, c_dir.as_ptr()) };
    }

    fn is_dark(&self) -> bool {
        unsafe { aibox_test_theme_is_dark(self.0) }
    }

    fn string_prop(&self, name: &str) -> String {
        let c_name = CString::new(name).unwrap();
        let mut buf = vec![0u8; 256];
        let len = unsafe {
            aibox_test_theme_string_prop(
                self.0,
                c_name.as_ptr(),
                buf.as_mut_ptr() as *mut c_char,
                buf.len() as c_int,
            )
        };
        let n = (len as usize).min(buf.len().saturating_sub(1));
        String::from_utf8_lossy(&buf[..n]).to_string()
    }
}

impl Drop for ThemeHandle {
    fn drop(&mut self) {
        unsafe { aibox_test_theme_free(self.0) };
    }
}

struct ThemeSpyHandle(*mut c_void);

impl ThemeSpyHandle {
    fn new(theme: &ThemeHandle) -> Self {
        let ptr = unsafe { aibox_test_theme_spy_new(theme.0) };
        assert!(!ptr.is_null());
        Self(ptr)
    }

    fn count(&self) -> i32 {
        unsafe { aibox_test_theme_spy_count(self.0) }
    }
}

impl Drop for ThemeSpyHandle {
    fn drop(&mut self) {
        unsafe { aibox_test_theme_spy_free(self.0) };
    }
}

fn test_theme_qobject_loads_light_fixture_and_emits_changed_once() {
    let fixtures_dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../aibox-core/tests/fixtures/themes");
    let latte = fs::read(fixtures_dir.join("catppuccin-latte.toml")).unwrap();
    let tokyo = fs::read(fixtures_dir.join("tokyo-night.toml")).unwrap();

    let tmp = TempDir::new("theme-qobject");
    let theme_subdir = tmp.path().join("theme");
    fs::create_dir_all(&theme_subdir).unwrap();
    let colors_path = theme_subdir.join("colors.toml");
    fs::write(&colors_path, &latte).unwrap();

    let prev_env = std::env::var_os("AIBOX_THEME_DIR");
    std::env::set_var("AIBOX_THEME_DIR", tmp.path());
    let theme = ThemeHandle::new();
    match prev_env {
        Some(v) => std::env::set_var("AIBOX_THEME_DIR", v),
        None => std::env::remove_var("AIBOX_THEME_DIR"),
    }

    assert!(!theme.is_dark(), "latte should set isDark == false");
    assert_eq!(theme.string_prop("mode"), "light");
    assert_eq!(theme.string_prop("surfaceCard"), "#e3e4e8");
    assert_eq!(theme.string_prop("surfaceCanvas"), "#eff1f5");

    // Re-arming with the same directory should not emit changed, and switching
    // colors.toml should emit changed exactly once.
    theme.configure(tmp.path());
    let spy = ThemeSpyHandle::new(&theme);
    assert_eq!(spy.count(), 0);

    fs::write(&colors_path, &tokyo).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while spy.count() == 0 && Instant::now() < deadline {
        unsafe { aibox_test_process_events(50) };
        std::thread::sleep(Duration::from_millis(25));
    }
    // Drain any trailing debounce window to confirm only one signal was emitted.
    std::thread::sleep(Duration::from_millis(120));
    unsafe { aibox_test_process_events(50) };

    assert_eq!(
        spy.count(),
        1,
        "expected exactly 1 changed() signal on theme switch"
    );
    assert!(theme.is_dark(), "tokyo-night should set isDark == true");
    assert_eq!(theme.string_prop("mode"), "dark");
    assert_eq!(theme.string_prop("surfaceCard"), "#13141c");
}

fn main() {
    let _ = aibox_app::ensure_cxx_qt_linked();
    unsafe { aibox_test_init_gui_app() };

    let tests: &[(&str, fn())] = &[
        ("primary_action_counts", test_primary_action_counts),
        (
            "enable_file_access_requires_known_running_environment",
            test_enable_file_access_requires_known_running_environment,
        ),
        (
            "enter_environment_returns_without_blocking",
            test_enter_environment_returns_without_blocking,
        ),
        (
            "theme_qobject_loads_light_fixture_and_emits_changed_once",
            test_theme_qobject_loads_light_fixture_and_emits_changed_once,
        ),
    ];

    println!("running {} tests", tests.len());
    for (name, f) in tests {
        f();
        println!("test {name} ... ok");
    }
    println!("test result: ok. {} passed; 0 failed", tests.len());
    unsafe { aibox_test_shutdown_gui_app() };
}
