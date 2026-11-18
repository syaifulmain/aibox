use crate::common;
use aibox_core::*;
use serde_json::{json, Value};

fn dispatch(fn_name: &str, _case: &str, args: &Value) -> Value {
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

#[test]
fn replay() {
    common::replay_jsonl("util", dispatch);
}
