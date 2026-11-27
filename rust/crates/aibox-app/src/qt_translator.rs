use std::path::{Path, PathBuf};
use std::pin::Pin;

use cxx_qt::{CxxQtType, Initialize, Threading};
use cxx_qt_lib::QString;

use crate::qt_watcher::{hash_files, nearest_existing_dir, SharedWatcher, WatchHandle};

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

pub struct TranslatorStateRust {
    language: QString,
    language_str: String,
    language_file: String,
    catalog_directory: String,
    catalog: aibox_core::i18n::Catalog,
    loaded: bool,
    last_content_hash: Option<u64>,
    watch_handle: Option<WatchHandle>,
}

impl Default for TranslatorStateRust {
    fn default() -> Self {
        Self {
            language: QString::default(),
            language_str: String::new(),
            language_file: String::new(),
            catalog_directory: ":/i18n".to_string(),
            catalog: aibox_core::i18n::Catalog::new(),
            loaded: false,
            last_content_hash: None,
            watch_handle: None,
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
        self.watch_handle = None;
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

        let qt_thread = self.qt_thread();
        let watch_handle = SharedWatcher::global().subscribe(
            language_watch_dirs(Path::new(&language_file), &catalog_directory),
            move || {
                let _ = qt_thread.queue(|state| {
                    state.reload();
                });
            },
        );

        {
            let mut rust = self.as_mut().rust_mut();
            rust.language_file = language_file;
            rust.catalog_directory = catalog_directory;
            rust.loaded = false;
            rust.last_content_hash = None;
            rust.watch_handle = Some(watch_handle);
        }

        self.reload();
    }

    fn reload(mut self: Pin<&mut Self>) {
        let (language_file, catalog_directory, loaded, last_hash) = {
            let rust = self.rust();
            (
                rust.language_file.clone(),
                rust.catalog_directory.clone(),
                rust.loaded,
                rust.last_content_hash,
            )
        };

        if let Some(handle) = self.rust().watch_handle.as_ref() {
            handle.update_dirs(language_watch_dirs(
                Path::new(&language_file),
                &catalog_directory,
            ));
        }

        let new_language = aibox_core::i18n::read_language(&language_file);
        let content_hash =
            translator_content_hash(&language_file, &catalog_directory, &new_language);
        if loaded && last_hash == Some(content_hash) {
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
            rust.language_str = new_language;
            rust.catalog = catalog;
            rust.loaded = true;
            rust.last_content_hash = Some(content_hash);
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
    match rust.catalog.lookup(source_text, &rust.language_str, n) {
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

fn language_watch_dirs(language_file: &Path, catalog_directory: &str) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(parent) = language_file.parent() {
        if let Some(ancestor) = nearest_existing_dir(parent) {
            dirs.push(ancestor);
        }
    }
    if !catalog_directory.starts_with(":/") {
        if let Some(ancestor) = nearest_existing_dir(Path::new(catalog_directory)) {
            dirs.push(ancestor);
        }
    }
    dirs
}

fn translator_content_hash(language_file: &str, catalog_directory: &str, language: &str) -> u64 {
    let mut paths = vec![PathBuf::from(language_file)];
    if !catalog_directory.starts_with(":/") {
        for code in aibox_core::i18n::language_catalog_codes(language) {
            paths.push(PathBuf::from(format!("{catalog_directory}/{code}.json")));
        }
    }
    hash_files(&paths, &(catalog_directory, language))
}
