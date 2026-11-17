use aibox_core::i18n::{self, Catalog};

pub struct CatalogHandle(Catalog);

#[cxx::bridge(namespace = "aibox::ffi")]
mod ffi {
    pub struct LookupResultFfi {
        pub found: bool,
        pub text: String,
    }

    extern "Rust" {
        type CatalogHandle;

        fn i18n_read_language(path: &str) -> String;
        fn i18n_plural_category(language: &str, count: i64) -> String;
        fn i18n_format_locale_name(language: &str, omarchy_path: &str) -> String;
        fn i18n_catalog_codes(language: &str) -> Vec<String>;
        fn i18n_catalog_new() -> Box<CatalogHandle>;
        fn i18n_catalog_merge_json(catalog: &mut CatalogHandle, bytes: &[u8]);
        fn i18n_catalog_is_empty(catalog: &CatalogHandle) -> bool;
        fn i18n_catalog_lookup(
            catalog: &CatalogHandle,
            source_text: &str,
            language: &str,
            n: i32,
        ) -> LookupResultFfi;
    }
}

fn i18n_read_language(path: &str) -> String {
    i18n::read_language(path)
}

fn i18n_plural_category(language: &str, count: i64) -> String {
    i18n::plural_category(language, count).to_string()
}

fn i18n_format_locale_name(language: &str, omarchy_path: &str) -> String {
    let opt = if omarchy_path.is_empty() {
        None
    } else {
        Some(omarchy_path)
    };
    i18n::format_locale_name(language, opt)
}

fn i18n_catalog_codes(language: &str) -> Vec<String> {
    i18n::language_catalog_codes(language)
}

fn i18n_catalog_new() -> Box<CatalogHandle> {
    Box::new(CatalogHandle(Catalog::new()))
}

fn i18n_catalog_merge_json(catalog: &mut CatalogHandle, bytes: &[u8]) {
    catalog.0.merge_json(bytes);
}

fn i18n_catalog_is_empty(catalog: &CatalogHandle) -> bool {
    catalog.0.is_empty()
}

fn i18n_catalog_lookup(
    catalog: &CatalogHandle,
    source_text: &str,
    language: &str,
    n: i32,
) -> ffi::LookupResultFfi {
    match catalog.0.lookup(source_text, language, n) {
        Some(text) => ffi::LookupResultFfi { found: true, text },
        None => ffi::LookupResultFfi {
            found: false,
            text: String::new(),
        },
    }
}
