use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

fn lang_code_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-z]{2,3}(-[A-Za-z]{2})?$").expect("valid lang code regex"))
}

/// Reads the Omarchy language code from the first line of `path`, falling back
/// to `"en"` when missing or invalid.
pub fn read_language(path: &str) -> String {
    let Ok(file) = File::open(path) else {
        return "en".to_string();
    };
    let mut reader = BufReader::new(file);
    let mut first_line = String::new();
    if reader.read_line(&mut first_line).is_err() {
        return "en".to_string();
    }
    let code = first_line.trim();
    if lang_code_re().is_match(code) {
        code.to_string()
    } else {
        "en".to_string()
    }
}

/// CLDR cardinal plural categories for whole counts, matching Omarchy's
/// `shell/Commons/Plural.js` and `Translator::pluralCategory`.
pub fn plural_category(language: &str, count: i64) -> &'static str {
    let n = count.unsigned_abs();
    let n10 = n % 10;
    let n100 = n % 100;
    let millions = n != 0 && n.is_multiple_of(1_000_000);
    let teens = (12..=14).contains(&n100);

    match language {
        "ja" | "ko" | "zh-CN" | "zh-TW" | "th" | "vi" | "id" | "ms" => "other",
        "hi" | "bn" | "fa" | "si" => {
            if n <= 1 {
                "one"
            } else {
                "other"
            }
        }
        "fr" | "pt-BR" => {
            if n <= 1 {
                "one"
            } else if millions {
                "many"
            } else {
                "other"
            }
        }
        "ca" | "es-MX" | "it" | "pt" => {
            if n == 1 {
                "one"
            } else if millions {
                "many"
            } else {
                "other"
            }
        }
        "fil" => {
            if n10 == 4 || n10 == 6 || n10 == 9 {
                "other"
            } else {
                "one"
            }
        }
        "is" => {
            if n10 == 1 && n100 != 11 {
                "one"
            } else {
                "other"
            }
        }
        "cs" | "sk" => {
            if n == 1 {
                "one"
            } else if (2..=4).contains(&n) {
                "few"
            } else {
                "other"
            }
        }
        "pl" => {
            if n == 1 {
                "one"
            } else if (2..=4).contains(&n10) && !teens {
                "few"
            } else {
                "many"
            }
        }
        "ru" | "uk" => {
            if n10 == 1 && n100 != 11 {
                "one"
            } else if (2..=4).contains(&n10) && !teens {
                "few"
            } else {
                "many"
            }
        }
        "hr" | "sr" => {
            if n10 == 1 && n100 != 11 {
                "one"
            } else if (2..=4).contains(&n10) && !teens {
                "few"
            } else {
                "other"
            }
        }
        "sl" => {
            if n100 == 1 {
                "one"
            } else if n100 == 2 {
                "two"
            } else if (3..=4).contains(&n100) {
                "few"
            } else {
                "other"
            }
        }
        "lt" => {
            if (11..=19).contains(&n100) {
                "other"
            } else if n10 == 1 {
                "one"
            } else if n10 >= 2 {
                "few"
            } else {
                "other"
            }
        }
        "lv" => {
            if n10 == 0 || (11..=19).contains(&n100) {
                "zero"
            } else if n10 == 1 {
                "one"
            } else {
                "other"
            }
        }
        "ro" => {
            if n == 1 {
                "one"
            } else if n == 0 || (1..=19).contains(&n100) {
                "few"
            } else {
                "other"
            }
        }
        "ar" => {
            if n <= 2 {
                match n {
                    0 => "zero",
                    1 => "one",
                    _ => "two",
                }
            } else if (3..=10).contains(&n100) {
                "few"
            } else if n100 >= 11 {
                "many"
            } else {
                "other"
            }
        }
        "he" => match n {
            1 => "one",
            2 => "two",
            _ => "other",
        },
        "ga" => {
            if n == 1 {
                "one"
            } else if n == 2 {
                "two"
            } else if (3..=6).contains(&n) {
                "few"
            } else if (7..=10).contains(&n) {
                "many"
            } else {
                "other"
            }
        }
        _ => {
            if n == 1 {
                "one"
            } else {
                "other"
            }
        }
    }
}

/// Resolves the Omarchy `formatLocale` for `language` from
/// `<omarchy_path>/default/i18n/locales.json`, falling back to `language`.
pub fn format_locale_name(language: &str, omarchy_path: Option<&str>) -> String {
    let base = omarchy_path.unwrap_or("");
    let registry_path = format!("{base}/default/i18n/locales.json");
    if let Ok(bytes) = fs::read(&registry_path) {
        if let Ok(Value::Object(root)) = serde_json::from_slice::<Value>(&bytes) {
            if let Some(name) = root
                .get(language)
                .and_then(|v| v.get("formatLocale"))
                .and_then(|v| v.as_str())
            {
                if !name.is_empty() {
                    return name.to_string();
                }
            }
        }
    }
    language.to_string()
}

/// Ordered, deduplicated catalog codes to load for `language`:
/// `["en", <base>, <language>]`.
pub fn language_catalog_codes(language: &str) -> Vec<String> {
    let base = language.split('-').next().unwrap_or(language);
    let mut codes = Vec::with_capacity(3);
    for candidate in ["en", base, language] {
        if !candidate.is_empty() && !codes.iter().any(|c| c == candidate) {
            codes.push(candidate.to_string());
        }
    }
    codes
}

#[derive(Debug, Clone)]
enum Entry {
    Plain(String),
    Plural(HashMap<String, String>),
}

/// In-memory translation catalog layered from English -> base -> regional JSON files.
#[derive(Debug, Default, Clone)]
pub struct Catalog {
    entries: HashMap<String, Entry>,
}

impl Catalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Merges a single JSON catalog blob on top of existing entries.
    pub fn merge_json(&mut self, json_bytes: &[u8]) {
        let Ok(Value::Object(obj)) = serde_json::from_slice::<Value>(json_bytes) else {
            return;
        };
        for (key, val) in obj {
            match val {
                Value::String(s) if !s.is_empty() => {
                    self.entries.insert(key, Entry::Plain(s));
                }
                Value::Object(forms_obj) => {
                    let mut forms = HashMap::new();
                    for (form_key, form_val) in forms_obj {
                        if let Some(s) = form_val.as_str() {
                            forms.insert(form_key, s.to_string());
                        }
                    }
                    self.entries.insert(key, Entry::Plural(forms));
                }
                _ => {}
            }
        }
    }

    /// Builds a catalog from ordered JSON byte layers (lowest to highest priority).
    pub fn from_json_layers(layers: &[&[u8]]) -> Self {
        let mut cat = Self::new();
        for layer in layers {
            cat.merge_json(layer);
        }
        cat
    }

    /// Loads layered catalogs (`en`, base, `language`) from a filesystem directory.
    pub fn load_from_dir(catalog_dir: &str, language: &str) -> Self {
        let mut cat = Self::new();
        for code in language_catalog_codes(language) {
            let path = Path::new(catalog_dir).join(format!("{code}.json"));
            if let Ok(bytes) = fs::read(path) {
                cat.merge_json(&bytes);
            }
        }
        cat
    }

    /// Looks up `source_text` for `language` and optional count `n` (`n < 0` means uncounted).
    pub fn lookup(&self, source_text: &str, language: &str, n: i32) -> Option<String> {
        match self.entries.get(source_text)? {
            Entry::Plain(s) => Some(s.clone()),
            Entry::Plural(forms) => {
                let category = if n < 0 {
                    "other"
                } else {
                    plural_category(language, n as i64)
                };
                forms
                    .get(category)
                    .filter(|s| !s.is_empty())
                    .or_else(|| forms.get("other").filter(|s| !s.is_empty()))
                    .cloned()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("aibox-i18n-{label}-{nanos}"));
        fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    #[test]
    fn translator_falls_back_to_english() {
        let dir = temp_dir("fallback");
        let lang_path = dir.join("language");
        let lang = read_language(&lang_path.to_string_lossy());
        assert_eq!(lang, "en");
        let cat = Catalog::load_from_dir(&dir.to_string_lossy(), &lang);
        assert_eq!(cat.lookup("Save", &lang, -1), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn translator_follows_the_language_file() {
        let dir = temp_dir("ptbr");
        fs::write(dir.join("language"), "pt-BR\n").unwrap();
        fs::write(
            dir.join("pt.json"),
            r#"{"Save": "Guardar", "Open": "Abrir"}"#,
        )
        .unwrap();
        fs::write(dir.join("pt-BR.json"), r#"{"Save": "Salvar"}"#).unwrap();

        let lang = read_language(&dir.join("language").to_string_lossy());
        assert_eq!(lang, "pt-BR");
        let cat = Catalog::load_from_dir(&dir.to_string_lossy(), &lang);
        assert_eq!(cat.lookup("Save", &lang, -1).as_deref(), Some("Salvar"));
        assert_eq!(cat.lookup("Open", &lang, -1).as_deref(), Some("Abrir"));
        assert_eq!(cat.lookup("Quit", &lang, -1), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn translator_picks_plural_forms() {
        let dir = temp_dir("plurals");
        fs::write(dir.join("language"), "ru\n").unwrap();
        fs::write(
            dir.join("en.json"),
            r#"{"%n clips": {"one": "%n clip", "other": "%n clips"}}"#,
        )
        .unwrap();
        fs::write(
            dir.join("ru.json"),
            r#"{"%n clips": {"one": "%n клип", "few": "%n клипа", "many": "%n клипов", "other": "%n клипа"}}"#,
        )
        .unwrap();

        let lang = read_language(&dir.join("language").to_string_lossy());
        let cat = Catalog::load_from_dir(&dir.to_string_lossy(), &lang);
        assert_eq!(
            cat.lookup("%n clips", &lang, 21).as_deref(),
            Some("%n клип")
        );
        assert_eq!(
            cat.lookup("%n clips", &lang, 22).as_deref(),
            Some("%n клипа")
        );
        assert_eq!(
            cat.lookup("%n clips", &lang, 25).as_deref(),
            Some("%n клипов")
        );
        assert_eq!(plural_category("ar", 11), "many");
        assert_eq!(plural_category("ja", 1), "other");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn format_locale_reads_registry_and_falls_back() {
        let dir = temp_dir("locales");
        let i18n_dir = dir.join("default").join("i18n");
        fs::create_dir_all(&i18n_dir).unwrap();
        fs::write(
            i18n_dir.join("locales.json"),
            r#"{"ar": {"formatLocale": "ar-AE"}, "da": {"formatLocale": "da-DK"}}"#,
        )
        .unwrap();
        let base = dir.to_string_lossy();
        assert_eq!(format_locale_name("ar", Some(&base)), "ar-AE");
        assert_eq!(format_locale_name("da", Some(&base)), "da-DK");
        assert_eq!(format_locale_name("fr", Some(&base)), "fr");
        assert_eq!(format_locale_name("ar", None), "ar");
        let _ = fs::remove_dir_all(&dir);
    }
}
