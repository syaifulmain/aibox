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
        // Semantic mode
        #[qproperty(bool, is_dark, cxx_name = "isDark", READ, NOTIFY = changed)]
        #[qproperty(QString, mode, READ, NOTIFY = changed)]
        // Surfaces
        #[qproperty(QString, surface_canvas, cxx_name = "surfaceCanvas", READ, NOTIFY = changed)]
        #[qproperty(QString, surface_card, cxx_name = "surfaceCard", READ, NOTIFY = changed)]
        #[qproperty(QString, surface_sidebar, cxx_name = "surfaceSidebar", READ, NOTIFY = changed)]
        #[qproperty(QString, surface_input, cxx_name = "surfaceInput", READ, NOTIFY = changed)]
        #[qproperty(QString, surface_raised, cxx_name = "surfaceRaised", READ, NOTIFY = changed)]
        #[qproperty(QString, surface_selected, cxx_name = "surfaceSelected", READ, NOTIFY = changed)]
        #[qproperty(QString, surface_track, cxx_name = "surfaceTrack", READ, NOTIFY = changed)]
        #[qproperty(QString, surface_overlay, cxx_name = "surfaceOverlay", READ, NOTIFY = changed)]
        // Borders
        #[qproperty(QString, border_subtle, cxx_name = "borderSubtle", READ, NOTIFY = changed)]
        #[qproperty(QString, border_strong, cxx_name = "borderStrong", READ, NOTIFY = changed)]
        #[qproperty(QString, border_focus, cxx_name = "borderFocus", READ, NOTIFY = changed)]
        // Text
        #[qproperty(QString, text_primary, cxx_name = "textPrimary", READ, NOTIFY = changed)]
        #[qproperty(QString, text_secondary, cxx_name = "textSecondary", READ, NOTIFY = changed)]
        #[qproperty(QString, text_muted, cxx_name = "textMuted", READ, NOTIFY = changed)]
        #[qproperty(QString, text_disabled, cxx_name = "textDisabled", READ, NOTIFY = changed)]
        #[qproperty(QString, text_on_accent, cxx_name = "textOnAccent", READ, NOTIFY = changed)]
        // Accent
        #[qproperty(QString, accent, READ, NOTIFY = changed)]
        #[qproperty(QString, accent_text, cxx_name = "accentText", READ, NOTIFY = changed)]
        #[qproperty(QString, accent_surface, cxx_name = "accentSurface", READ, NOTIFY = changed)]
        #[qproperty(QString, accent_border, cxx_name = "accentBorder", READ, NOTIFY = changed)]
        #[qproperty(QString, on_accent, cxx_name = "onAccent", READ, NOTIFY = changed)]
        // Danger
        #[qproperty(QString, danger, READ, NOTIFY = changed)]
        #[qproperty(QString, danger_text, cxx_name = "dangerText", READ, NOTIFY = changed)]
        #[qproperty(QString, danger_surface, cxx_name = "dangerSurface", READ, NOTIFY = changed)]
        #[qproperty(QString, danger_border, cxx_name = "dangerBorder", READ, NOTIFY = changed)]
        #[qproperty(QString, on_danger, cxx_name = "onDanger", READ, NOTIFY = changed)]
        // Success
        #[qproperty(QString, success, READ, NOTIFY = changed)]
        #[qproperty(QString, success_text, cxx_name = "successText", READ, NOTIFY = changed)]
        #[qproperty(QString, success_surface, cxx_name = "successSurface", READ, NOTIFY = changed)]
        #[qproperty(QString, success_border, cxx_name = "successBorder", READ, NOTIFY = changed)]
        #[qproperty(QString, on_success, cxx_name = "onSuccess", READ, NOTIFY = changed)]
        // Warning
        #[qproperty(QString, warning, READ, NOTIFY = changed)]
        #[qproperty(QString, warning_text, cxx_name = "warningText", READ, NOTIFY = changed)]
        #[qproperty(QString, warning_surface, cxx_name = "warningSurface", READ, NOTIFY = changed)]
        #[qproperty(QString, warning_border, cxx_name = "warningBorder", READ, NOTIFY = changed)]
        #[qproperty(QString, on_warning, cxx_name = "onWarning", READ, NOTIFY = changed)]
        // Info
        #[qproperty(QString, info, READ, NOTIFY = changed)]
        #[qproperty(QString, info_text, cxx_name = "infoText", READ, NOTIFY = changed)]
        #[qproperty(QString, info_surface, cxx_name = "infoSurface", READ, NOTIFY = changed)]
        #[qproperty(QString, info_border, cxx_name = "infoBorder", READ, NOTIFY = changed)]
        #[qproperty(QString, on_info, cxx_name = "onInfo", READ, NOTIFY = changed)]
        // Overlay & selection
        #[qproperty(QString, scrim, READ, NOTIFY = changed)]
        #[qproperty(QString, scrim_strong, cxx_name = "scrimStrong", READ, NOTIFY = changed)]
        #[qproperty(QString, selection, READ, NOTIFY = changed)]
        #[qproperty(QString, selection_text, cxx_name = "selectionText", READ, NOTIFY = changed)]
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

macro_rules! define_theme_rust {
    ($($palette_field:ident),+ $(,)?) => {
        pub struct ThemeRust {
            is_dark: bool,
            mode: QString,
            $($palette_field: QString,)+
            radius: i32,
            colors: aibox_core::theme::ThemeColors,
            palette: aibox_core::theme::Palette,
            directory: String,
            hyprctl: String,
            watcher_state: Option<Arc<Mutex<WatcherState>>>,
            stop_flag: Arc<AtomicBool>,
            rounding_wake_tx: Option<mpsc::Sender<()>>,
        }

        impl Default for ThemeRust {
            fn default() -> Self {
                let colors = aibox_core::theme::ThemeColors::default();
                let palette = aibox_core::theme::derive_palette(&colors);
                Self {
                    is_dark: palette.is_dark,
                    mode: QString::from(&colors.mode),
                    $($palette_field: QString::from(&palette.$palette_field),)+
                    radius: 0,
                    colors,
                    palette,
                    directory: String::new(),
                    hyprctl: String::new(),
                    watcher_state: None,
                    stop_flag: Arc::new(AtomicBool::new(false)),
                    rounding_wake_tx: None,
                }
            }
        }

        impl ThemeRust {
            fn apply_theme(
                &mut self,
                colors: aibox_core::theme::ThemeColors,
                palette: aibox_core::theme::Palette,
            ) {
                self.is_dark = palette.is_dark;
                self.mode = QString::from(&colors.mode);
                $(self.$palette_field = QString::from(&palette.$palette_field);)+
                self.colors = colors;
                self.palette = palette;
            }
        }
    };
}

define_theme_rust!(
    surface_canvas,
    surface_card,
    surface_sidebar,
    surface_input,
    surface_raised,
    surface_selected,
    surface_track,
    surface_overlay,
    border_subtle,
    border_strong,
    border_focus,
    text_primary,
    text_secondary,
    text_muted,
    text_disabled,
    text_on_accent,
    accent,
    accent_text,
    accent_surface,
    accent_border,
    on_accent,
    danger,
    danger_text,
    danger_surface,
    danger_border,
    on_danger,
    success,
    success_text,
    success_surface,
    success_border,
    on_success,
    warning,
    warning_text,
    warning_surface,
    warning_border,
    on_warning,
    info,
    info_text,
    info_surface,
    info_border,
    on_info,
    scrim,
    scrim_strong,
    selection,
    selection_text,
);

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

        let (colors, palette) = load_colors_and_palette(Path::new(&directory));

        let changed = {
            let rust = self.rust();
            colors != rust.colors || palette != rust.palette
        };

        if changed {
            self.as_mut().rust_mut().apply_theme(colors, palette);
            // Emit one changed signal per theme switch; M7 will hook window icon retinting here.
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

fn load_colors_and_palette(
    theme_dir: &Path,
) -> (aibox_core::theme::ThemeColors, aibox_core::theme::Palette) {
    let candidate = if theme_dir.is_file() {
        Some(theme_dir.to_path_buf())
    } else if theme_dir.join("theme").join("colors.toml").is_file() {
        Some(theme_dir.join("theme").join("colors.toml"))
    } else if theme_dir.join("colors.toml").is_file() {
        Some(theme_dir.join("colors.toml"))
    } else {
        None
    };

    let colors = match candidate {
        Some(path) => match path.to_str() {
            Some(s) => aibox_core::theme::parse_colors_file(s),
            None => aibox_core::theme::ThemeColors::default(),
        },
        None => aibox_core::theme::ThemeColors::default(),
    };
    let palette = aibox_core::theme::derive_palette(&colors);
    (colors, palette)
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
    candidates.push(directory.join("colors.toml"));
    candidates.push(directory.join("theme.name"));
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
