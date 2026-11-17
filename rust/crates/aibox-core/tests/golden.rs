use std::path::PathBuf;

#[allow(clippy::match_single_binding)]
fn dispatch(fn_name: &str, _args: &serde_json::Value) -> serde_json::Value {
    match fn_name {
        other => panic!("unregistered golden function: {other}"),
    }
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("tests")
        .join("golden")
}

// Modules are enabled here as each migration phase implements them in Rust.
const ACTIVE_MODULES: &[&str] = &[];

#[test]
fn replay_active_goldens() {
    let dir = golden_dir();
    for module in ACTIVE_MODULES {
        let path = dir.join(format!("{module}.jsonl"));
        let contents = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        for (idx, line) in contents.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let row: serde_json::Value = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("{}:{}: invalid JSON: {e}", path.display(), idx + 1));
            let fn_name = row["fn"].as_str().expect("missing fn");
            let case = row["case"].as_str().unwrap_or("");
            let actual = dispatch(fn_name, &row["args"]);
            assert_eq!(
                actual,
                row["out"],
                "{}:{}: mismatch for {fn_name} ({case})",
                path.display(),
                idx + 1
            );
        }
    }
}
