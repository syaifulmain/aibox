use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use aibox_core::theme::{load_palette, Palette};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn collect_fixture_palettes() -> BTreeMap<String, Palette> {
    let fixtures_dir = manifest_dir().join("tests").join("fixtures").join("themes");
    let mut entries: Vec<PathBuf> = fs::read_dir(&fixtures_dir)
        .expect("fixtures/themes directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("toml"))
        .collect();
    entries.sort();
    assert_eq!(entries.len(), 23, "expected all 23 Omarchy theme fixtures");

    let mut out = BTreeMap::new();
    for path in entries {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("fixture stem")
            .to_string();
        out.insert(name, load_palette(&path));
    }
    out
}

#[test]
fn theme_palettes_json_matches_golden_snapshot() {
    let golden_path = manifest_dir()
        .join("tests")
        .join("golden")
        .join("theme_palettes.json");
    let actual_map = collect_fixture_palettes();
    let actual_json = serde_json::to_string_pretty(&actual_map).expect("serialize palettes") + "\n";

    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        fs::write(&golden_path, &actual_json).expect("write theme_palettes.json");
        return;
    }

    let expected_json = fs::read_to_string(&golden_path).unwrap_or_else(|e| {
        panic!(
            "failed to read {}: {e} (run with UPDATE_GOLDEN=1 to generate)",
            golden_path.display()
        )
    });
    assert_eq!(
        actual_json, expected_json,
        "derived palette JSON drifted from golden snapshot"
    );
}
