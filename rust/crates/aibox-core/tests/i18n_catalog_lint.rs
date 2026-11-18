use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use aibox_core::i18n::find_placeholder_mismatches;
use regex::Regex;
use serde_json::Value;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("repo root")
        .to_path_buf()
}

fn collect_qml_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_qml_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("qml") {
            out.push(path);
        }
    }
}

fn unescape_qml_string(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn load_json_map(path: &Path) -> HashMap<String, Value> {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let Value::Object(obj) =
        serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
    else {
        panic!("{} must be a JSON object", path.display());
    };
    obj.into_iter().collect()
}

#[test]
fn qml_strings_have_danish_translations_and_valid_placeholders() {
    let root = repo_root();
    let qml_dir = root.join("src/qml");
    let mut qml_files = Vec::new();
    collect_qml_files(&qml_dir, &mut qml_files);
    qml_files.sort();
    assert!(!qml_files.is_empty(), "no QML files found under src/qml");

    let qs_tr_re = Regex::new(r#"qsTr\(\s*"((?:\\.|[^"\\])*)""#).expect("valid qsTr regex");
    let mut qml_keys = BTreeSet::new();
    for path in &qml_files {
        let src =
            fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for cap in qs_tr_re.captures_iter(&src) {
            qml_keys.insert(unescape_qml_string(&cap[1]));
        }
    }

    let da_path = root.join("src/i18n/da.json");
    let en_path = root.join("src/i18n/en.json");
    let da_map = load_json_map(&da_path);
    let en_map = load_json_map(&en_path);

    let missing_in_da: Vec<&str> = qml_keys
        .iter()
        .filter(|k| !da_map.contains_key(k.as_str()))
        .map(String::as_str)
        .collect();
    assert!(
        missing_in_da.is_empty(),
        "QML qsTr() strings missing from src/i18n/da.json ({}):\n  {}",
        missing_in_da.len(),
        missing_in_da.join("\n  ")
    );

    let mut orphan_da: Vec<&str> = da_map
        .keys()
        .filter(|k| !qml_keys.contains(k.as_str()))
        .map(String::as_str)
        .collect();
    orphan_da.sort_unstable();
    assert!(
        orphan_da.is_empty(),
        "Orphan keys in src/i18n/da.json not used in src/qml/ ({}):\n  {}",
        orphan_da.len(),
        orphan_da.join("\n  ")
    );

    let mut orphan_en: Vec<&str> = en_map
        .keys()
        .filter(|k| !qml_keys.contains(k.as_str()))
        .map(String::as_str)
        .collect();
    orphan_en.sort_unstable();
    assert!(
        orphan_en.is_empty(),
        "Orphan keys in src/i18n/en.json not used in src/qml/ ({}):\n  {}",
        orphan_en.len(),
        orphan_en.join("\n  ")
    );

    for path in [&da_path, &en_path] {
        let bytes = fs::read(path).expect("read catalog");
        let mismatches = find_placeholder_mismatches(&bytes)
            .unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
        assert!(
            mismatches.is_empty(),
            "Placeholder mismatches in {}:\n{:#?}",
            path.display(),
            mismatches
        );
    }
}
