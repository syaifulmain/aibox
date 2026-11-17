use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use cxx_qt::{CxxQtType, Initialize, Threading};
use cxx_qt_lib::QString;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qproperty(QString, accent, READ, NOTIFY = changed)]
        #[qproperty(QString, accent_foreground, cxx_name = "accentForeground", READ, NOTIFY = changed)]
        #[qproperty(QString, background, READ, NOTIFY = changed)]
        #[qproperty(QString, dark_background, cxx_name = "darkBackground", READ, NOTIFY = changed)]
        #[qproperty(QString, lighter_background, cxx_name = "lighterBackground", READ, NOTIFY = changed)]
        #[qproperty(QString, foreground, READ, NOTIFY = changed)]
        #[qproperty(QString, muted, READ, NOTIFY = changed)]
        #[qproperty(QString, green, READ, NOTIFY = changed)]
        #[qproperty(QString, red, READ, NOTIFY = changed)]
        #[qproperty(QString, yellow, READ, NOTIFY = changed)]
        #[qproperty(i32, radius, READ, NOTIFY = changed)]
        type Theme = super::ThemeRust;

        #[qsignal]
        fn changed(self: Pin<&mut Theme>);

        #[qinvokable]
        fn configure(self: Pin<&mut Theme>, directory: &QString, hyprctl: &QString);
    }

    impl cxx_qt::Threading for Theme {}
    impl cxx_qt::Initialize for Theme {}
}

struct WatcherState {
    watcher: RecommendedWatcher,
    watched_paths: HashSet<PathBuf>,
}

pub struct ThemeRust {
    accent: QString,
    accent_foreground: QString,
    background: QString,
    dark_background: QString,
    lighter_background: QString,
    foreground: QString,
    muted: QString,
    green: QString,
    red: QString,
    yellow: QString,
    radius: i32,
    directory: String,
    hyprctl: String,
    watcher_state: Option<Arc<Mutex<WatcherState>>>,
    stop_flag: Arc<AtomicBool>,
    rounding_wake_tx: Option<mpsc::Sender<()>>,
}

impl Default for ThemeRust {
    fn default() -> Self {
        let defaults = aibox_core::theme::ThemeColors::default();
        let accent_fg = aibox_core::theme::accent_foreground(&defaults.accent);
        Self {
            accent: QString::from(&defaults.accent),
            accent_foreground: QString::from(accent_fg),
            background: QString::from(&defaults.background),
            dark_background: QString::from(&defaults.dark_background),
            lighter_background: QString::from(&defaults.lighter_background),
            foreground: QString::from(&defaults.foreground),
            muted: QString::from(&defaults.muted),
            green: QString::from(&defaults.green),
            red: QString::from(&defaults.red),
            yellow: QString::from(&defaults.yellow),
            radius: 0,
            directory: String::new(),
            hyprctl: String::new(),
            watcher_state: None,
            stop_flag: Arc::new(AtomicBool::new(false)),
            rounding_wake_tx: None,
        }
    }
}

impl Drop for ThemeRust {
    fn drop(&mut self) {
        self.stop_workers();
    }
}

impl ThemeRust {
    fn stop_workers(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        self.watcher_state = None;
        self.rounding_wake_tx = None;
    }
}

impl Initialize for qobject::Theme {
    fn initialize(self: Pin<&mut Self>) {
        let env_dir = std::env::var("AIBOX_THEME_DIR").unwrap_or_default();
        let dir = default_theme_dir(&env_dir);
        let hyprctl = if env_dir.is_empty()
            && std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
                .map(|v| !v.is_empty())
                .unwrap_or(false)
        {
            find_executable("hyprctl").unwrap_or_default()
        } else {
            String::new()
        };
        self.setup(dir, hyprctl);
    }
}

impl qobject::Theme {
    pub fn configure(self: Pin<&mut Self>, directory: &QString, hyprctl: &QString) {
        let dir_str = directory.to_string();
        let dir = default_theme_dir(&dir_str);
        let hypr_str = hyprctl.to_string();
        let hypr = if !hypr_str.is_empty() {
            hypr_str
        } else if dir_str.is_empty()
            && std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
                .map(|v| !v.is_empty())
                .unwrap_or(false)
        {
            find_executable("hyprctl").unwrap_or_default()
        } else {
            String::new()
        };
        self.setup(dir, hypr);
    }

    fn setup(mut self: Pin<&mut Self>, directory: String, hyprctl: String) {
        self.as_mut().rust_mut().stop_workers();

        let stop_flag = Arc::new(AtomicBool::new(false));
        let (fs_tx, fs_rx) = mpsc::channel::<()>();

        let watcher_state =
            notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if res.is_ok() {
                    let _ = fs_tx.send(());
                }
            })
            .ok()
            .map(|watcher| {
                Arc::new(Mutex::new(WatcherState {
                    watcher,
                    watched_paths: HashSet::new(),
                }))
            });

        let qt_thread_fs = self.qt_thread();
        let stop_fs = stop_flag.clone();
        std::thread::spawn(move || {
            while fs_rx.recv().is_ok() {
                if stop_fs.load(Ordering::Relaxed) {
                    break;
                }
                while fs_rx.recv_timeout(Duration::from_millis(80)).is_ok() {
                    if stop_fs.load(Ordering::Relaxed) {
                        return;
                    }
                }
                if stop_fs.load(Ordering::Relaxed) {
                    break;
                }
                let _ = qt_thread_fs.queue(|theme| {
                    theme.reload();
                });
            }
        });

        let rounding_wake_tx = if !hyprctl.is_empty() {
            let (wake_tx, wake_rx) = mpsc::channel::<()>();
            let qt_thread_round = self.qt_thread();
            let stop_round = stop_flag.clone();
            let hypr_cmd = hyprctl.clone();
            std::thread::spawn(move || {
                loop {
                    if stop_round.load(Ordering::Relaxed) {
                        break;
                    }
                    if let Some(radius) = query_hyprctl_rounding(&hypr_cmd) {
                        let _ = qt_thread_round.queue(move |theme| {
                            theme.apply_rounding(radius);
                        });
                    }
                    match wake_rx.recv_timeout(Duration::from_millis(1000)) {
                        Ok(()) => {
                            // Drain any extra wakeups before polling once.
                            while wake_rx.try_recv().is_ok() {}
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            });
            Some(wake_tx)
        } else {
            None
        };

        {
            let mut rust = self.as_mut().rust_mut();
            rust.directory = directory;
            rust.hyprctl = hyprctl;
            rust.watcher_state = watcher_state;
            rust.stop_flag = stop_flag;
            rust.rounding_wake_tx = rounding_wake_tx;
        }

        self.reload();
    }

    fn reload(mut self: Pin<&mut Self>) {
        let (directory, watcher_state, rounding_wake_tx) = {
            let rust = self.rust();
            (
                rust.directory.clone(),
                rust.watcher_state.clone(),
                rust.rounding_wake_tx.clone(),
            )
        };

        if let Some(tx) = rounding_wake_tx {
            let _ = tx.send(());
        }

        if let Some(state_arc) = watcher_state {
            if let Ok(mut state) = state_arc.lock() {
                rearm_theme_watches(&mut state, Path::new(&directory));
            }
        }

        let toml_path = format!("{directory}/theme/colors.toml");
        let colors = aibox_core::theme::parse_colors_file(&toml_path);
        let accent = QString::from(&colors.accent);
        let bg = QString::from(&colors.background);
        let dark_bg = QString::from(&colors.dark_background);
        let lighter_bg = QString::from(&colors.lighter_background);
        let fg = QString::from(&colors.foreground);
        let muted = QString::from(&colors.muted);
        let green = QString::from(&colors.green);
        let red = QString::from(&colors.red);
        let yellow = QString::from(&colors.yellow);

        let changed = {
            let rust = self.rust();
            accent != rust.accent
                || bg != rust.background
                || dark_bg != rust.dark_background
                || lighter_bg != rust.lighter_background
                || fg != rust.foreground
                || muted != rust.muted
                || green != rust.green
                || red != rust.red
                || yellow != rust.yellow
        };

        if changed {
            let accent_fg = QString::from(aibox_core::theme::accent_foreground(&colors.accent));
            {
                let mut rust = self.as_mut().rust_mut();
                rust.accent = accent;
                rust.accent_foreground = accent_fg;
                rust.background = bg;
                rust.dark_background = dark_bg;
                rust.lighter_background = lighter_bg;
                rust.foreground = fg;
                rust.muted = muted;
                rust.green = green;
                rust.red = red;
                rust.yellow = yellow;
            }
            self.as_mut().changed();
        }
    }

    fn apply_rounding(mut self: Pin<&mut Self>, radius: i32) {
        if self.rust().radius != radius {
            self.as_mut().rust_mut().radius = radius;
            self.as_mut().changed();
        }
    }
}

fn default_theme_dir(override_dir: &str) -> String {
    if !override_dir.is_empty() {
        return override_dir.to_string();
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    format!("{home}/.local/state/omarchy/current")
}

fn find_executable(name: &str) -> Option<String> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

fn rearm_theme_watches(state: &mut WatcherState, directory: &Path) {
    let old_paths: Vec<PathBuf> = state.watched_paths.drain().collect();
    for path in old_paths {
        let _ = state.watcher.unwatch(&path);
    }

    let mut ancestor = directory.to_path_buf();
    while !ancestor.exists() {
        if let Some(parent) = ancestor.parent() {
            if parent == ancestor {
                break;
            }
            ancestor = parent.to_path_buf();
        } else {
            break;
        }
    }

    let mut candidates = Vec::new();
    candidates.push(ancestor.clone());
    if let Some(parent) = ancestor.parent() {
        candidates.push(parent.to_path_buf());
    }
    candidates.push(directory.to_path_buf());
    candidates.push(directory.join("theme"));
    candidates.push(directory.join("theme").join("colors.toml"));

    for path in candidates {
        if path.exists()
            && !state.watched_paths.contains(&path)
            && state
                .watcher
                .watch(&path, RecursiveMode::NonRecursive)
                .is_ok()
        {
            state.watched_paths.insert(path);
        }
    }
}

fn query_hyprctl_rounding(hyprctl: &str) -> Option<i32> {
    let mut child = Command::new(hyprctl)
        .args(["-j", "getoption", "decoration:rounding"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let deadline = Instant::now() + Duration::from_millis(500);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let output = child.wait_with_output().ok()?;
                return aibox_core::theme::parse_rounding(&output.stdout);
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(15));
            }
            Err(_) => return None,
        }
    }
}
