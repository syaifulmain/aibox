use std::fs;
use std::path::{Path, PathBuf};

const MAX_LINES: usize = 1500;

/// Legacy files tracked for splitting in their own module maintenance plans.
/// They are capped near their current size so they cannot grow further.
const LEGACY_ALLOWLIST: &[(&str, usize)] = &[
    // TODO(app-backend): split qt_backend.rs below 1500 lines.
    ("rust/crates/aibox-app/src/qt_backend.rs", 1650),
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("repo root")
        .to_path_buf()
}

fn is_non_test_source(rel_path: &str) -> bool {
    let is_rs = rel_path.ends_with(".rs");
    let is_qml = rel_path.ends_with(".qml");
    if !is_rs && !is_qml {
        return false;
    }
    !rel_path.contains("/tests/") && !rel_path.ends_with("_test.rs")
}

fn collect_sources(dir: &Path, root: &Path, out: &mut Vec<(String, usize)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_sources(&path, root, out);
        } else {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if is_non_test_source(&rel) {
                let text = fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
                let lines = text.lines().count();
                out.push((rel, lines));
            }
        }
    }
}

fn max_allowed_lines(rel_path: &str) -> usize {
    LEGACY_ALLOWLIST
        .iter()
        .find(|(p, _)| *p == rel_path)
        .map_or(MAX_LINES, |(_, cap)| *cap)
}

#[test]
fn non_test_sources_stay_within_line_limit() {
    let root = repo_root();
    let mut files = Vec::new();
    collect_sources(&root.join("rust/crates/aibox-core/src"), &root, &mut files);
    collect_sources(&root.join("rust/crates/aibox-app/src"), &root, &mut files);
    collect_sources(&root.join("src/qml"), &root, &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(!files.is_empty(), "expected non-test source files");

    let violations: Vec<String> = files
        .into_iter()
        .filter_map(|(rel, lines)| {
            let limit = max_allowed_lines(&rel);
            (lines > limit).then(|| format!("{rel}: {lines} lines (limit {limit})"))
        })
        .collect();

    assert!(
        violations.is_empty(),
        "Non-test source files exceed line limit:\n  {}",
        violations.join("\n  ")
    );
}

#[test]
fn file_size_lint_helpers_classify_paths() {
    assert!(is_non_test_source("rust/crates/aibox-core/src/engine.rs"));
    assert!(is_non_test_source("src/qml/Main.qml"));
    assert!(!is_non_test_source(
        "rust/crates/aibox-core/tests/golden/incus.rs"
    ));
    assert!(!is_non_test_source("rust/Cargo.toml"));
    assert_eq!(
        max_allowed_lines("rust/crates/aibox-core/src/engine.rs"),
        MAX_LINES
    );
}
