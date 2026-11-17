use std::path::PathBuf;

use aibox_core::util;
use serde_json::{json, Value};

fn dispatch(fn_name: &str, args: &Value) -> Value {
    match fn_name {
        "formatBytes" => {
            let bytes = args["bytes"].as_f64().expect("bytes") as i64;
            json!(util::format::format_bytes(bytes))
        }
        "formatByteRate" => {
            let bps = args["bytesPerSecond"].as_f64().expect("bytesPerSecond") as i64;
            json!(util::format::format_byte_rate(bps))
        }
        "formatByteTotal" => {
            let bytes = args["bytes"].as_f64().expect("bytes") as i64;
            json!(util::format::format_byte_total(bytes))
        }
        "parseSizeBytes" => {
            let text = args["sizeText"].as_str().expect("sizeText");
            json!(util::size::parse_size_bytes(text))
        }
        "validatedSizeOptions" => {
            let values: Vec<String> = args["values"]
                .as_array()
                .expect("values")
                .iter()
                .map(|v| v.as_str().expect("string").to_string())
                .collect();
            json!(util::size::validated_size_options(&values))
        }
        "validatedCpuOptions" => {
            let values = args["values"].as_array().expect("values");
            json!(util::size::validated_cpu_options(values))
        }
        "Backend::sanitizeName" => {
            let raw = args["rawName"].as_str().expect("rawName");
            json!(util::names::sanitize_name(raw))
        }
        "Backend::shellQuote" => {
            let val = args["value"].as_str().expect("value");
            json!(util::shell::shell_quote(val))
        }
        "Backend::shellArrayWords" => {
            let words = args["words"].as_str().expect("words");
            json!(util::shell::shell_array_words(words))
        }
        "Backend::parseCsv" => {
            let text = args["text"].as_str().expect("text");
            json!(util::csv::parse_csv(text))
        }
        "Backend::truncateLogTail" => {
            let data = args["data"].as_str().expect("data").as_bytes();
            let max_bytes = args["maxBytes"].as_f64().expect("maxBytes") as i64;
            let out = util::bytes::truncate_log_tail(data, max_bytes);
            json!(String::from_utf8(out).expect("utf8"))
        }
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
const ACTIVE_MODULES: &[&str] = &["util"];

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
            let row: Value = serde_json::from_str(line)
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
