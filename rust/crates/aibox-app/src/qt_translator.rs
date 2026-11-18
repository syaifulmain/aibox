use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use cxx_qt::{CxxQtType, Initialize, Threading};
use cxx_qt_lib::QString;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

#[cxx_qt::bridge]
pub mod qobject {
    struct TranslatorLookup {
        found: bool,
        text: String,
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/qbytearray.h");
        type QByteArray = cxx_qt_lib::QByteArray;

        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("shim/qml_helpers.h");
        fn aibox_apply_locale(locale_name: &QString);
        fn aibox_read_qt_resource(path: &QString) -> QByteArray;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qproperty(QString, language, READ, NOTIFY = changed)]
        type TranslatorState = super::TranslatorStateRust;

        #[qsignal]
        fn changed(self: Pin<&mut TranslatorState>);

        #[qinvokable]
        fn configure(
            self: Pin<&mut TranslatorState>,
            language_file: &QString,
            catalog_directory: &QString,
        );
    }

    extern "Rust" {
        fn translator_state_is_empty(state: &TranslatorState) -> bool;
        fn translator_state_translate(
            state: &TranslatorState,
            source_text: &str,
            n: i32,
        ) -> TranslatorLookup;
    }

    impl cxx_qt::Threading for TranslatorState {}
    impl cxx_qt::Initialize for TranslatorState {}
}

struct WatcherState {
    watcher: RecommendedWatcher,
    watched_paths: HashSet<PathBuf>,
}

pub struct TranslatorStateRust {
    language: QString,
    language_file: String,
    catalog_directory: String,
    catalog: aibox_core::i18n::Catalog,
    loaded: bool,
    watcher_state: Option<Arc<Mutex<WatcherState>>>,
    stop_flag: Arc<AtomicBool>,
}

impl Default for TranslatorStateRust {
    fn default() -> Self {
        Self {
            language: QString::default(),
            language_file: String::new(),
            catalog_directory: ":/i18n".to_string(),
            catalog: aibox_core::i18n::Catalog::new(),
            loaded: false,
            watcher_state: None,
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Drop for TranslatorStateRust {
    fn drop(&mut self) {
        self.stop_workers();
    }
}

impl TranslatorStateRust {
    fn stop_workers(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        self.watcher_state = None;
    }
}

impl Initialize for qobject::TranslatorState {
    fn initialize(self: Pin<&mut Self>) {
        self.setup(default_language_file(""), ":/i18n".to_string());
    }
}

impl qobject::TranslatorState {
    pub fn configure(self: Pin<&mut Self>, language_file: &QString, catalog_directory: &QString) {
        let lang_file = default_language_file(&language_file.to_string());
        let cat_dir = catalog_directory.to_string();
        let cat_dir = if cat_dir.is_empty() {
            ":/i18n".to_string()
        } else {
            cat_dir
        };
        self.setup(lang_file, cat_dir);
    }

    fn setup(mut self: Pin<&mut Self>, language_file: String, catalog_directory: String) {
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

        let qt_thread = self.qt_thread();
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
                let _ = qt_thread.queue(|state| {
                    state.reload();
                });
            }
        });

        {
            let mut rust = self.as_mut().rust_mut();
            rust.language_file = language_file;
            rust.catalog_directory = catalog_directory;
            rust.loaded = false;
            rust.watcher_state = watcher_state;
            rust.stop_flag = stop_flag;
        }

        self.reload();
    }

    fn reload(mut self: Pin<&mut Self>) {
        let (language_file, catalog_directory, current_language, loaded, watcher_state) = {
            let rust = self.rust();
            (
                rust.language_file.clone(),
                rust.catalog_directory.clone(),
                rust.language.to_string(),
                rust.loaded,
                rust.watcher_state.clone(),
            )
        };

        if let Some(state_arc) = watcher_state {
            if let Ok(mut state) = state_arc.lock() {
                rearm_language_watches(&mut state, Path::new(&language_file));
            }
        }

        let new_language = aibox_core::i18n::read_language(&language_file);
        if loaded && new_language == current_language {
            return;
        }

        let mut catalog = aibox_core::i18n::Catalog::new();
        for code in aibox_core::i18n::language_catalog_codes(&new_language) {
            let path = format!("{catalog_directory}/{code}.json");
            if catalog_directory.starts_with(":/") {
                let bytes = qobject::aibox_read_qt_resource(&QString::from(&path));
                let slice = bytes.as_slice();
                if !slice.is_empty() {
                    catalog.merge_json(slice);
                }
            } else if let Ok(bytes) = std::fs::read(&path) {
                catalog.merge_json(&bytes);
            }
        }

        let omarchy_path = std::env::var("OMARCHY_PATH").ok();
        let locale_name =
            aibox_core::i18n::format_locale_name(&new_language, omarchy_path.as_deref());
        qobject::aibox_apply_locale(&QString::from(&locale_name));

        {
            let mut rust = self.as_mut().rust_mut();
            rust.language = QString::from(&new_language);
            rust.catalog = catalog;
            rust.loaded = true;
        }

        self.as_mut().changed();
    }
}

pub fn translator_state_is_empty(state: &qobject::TranslatorState) -> bool {
    state.rust().catalog.is_empty()
}

pub fn translator_state_translate(
    state: &qobject::TranslatorState,
    source_text: &str,
    n: i32,
) -> qobject::TranslatorLookup {
    let rust = state.rust();
    let language = rust.language.to_string();
    match rust.catalog.lookup(source_text, &language, n) {
        Some(text) => qobject::TranslatorLookup {
            found: true,
            text: text.to_string(),
        },
        None => qobject::TranslatorLookup {
            found: false,
            text: String::new(),
        },
    }
}

fn default_language_file(override_path: &str) -> String {
    if !override_path.is_empty() {
        return override_path.to_string();
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    format!("{home}/.config/omarchy/language")
}

fn rearm_language_watches(state: &mut WatcherState, language_file: &Path) {
    let old_paths: Vec<PathBuf> = state.watched_paths.drain().collect();
    for path in old_paths {
        let _ = state.watcher.unwatch(&path);
    }

    if let Some(parent) = language_file.parent() {
        if parent.exists()
            && state
                .watcher
                .watch(parent, RecursiveMode::NonRecursive)
                .is_ok()
        {
            state.watched_paths.insert(parent.to_path_buf());
        }
    }

    if language_file.exists()
        && !state.watched_paths.contains(language_file)
        && state
            .watcher
            .watch(language_file, RecursiveMode::NonRecursive)
            .is_ok()
    {
        state.watched_paths.insert(language_file.to_path_buf());
    }
}
