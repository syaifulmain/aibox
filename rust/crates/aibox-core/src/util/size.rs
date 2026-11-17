//! Size string parsing and resource option validation.

use std::collections::HashSet;
use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

fn size_parse_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^\s*(\d+(?:\.\d+)?)\s*(TiB|TB|GiB|GB|MiB|MB|KiB|KB|B)?\s*$")
            .expect("valid size parse regex")
    })
}

fn size_option_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^\d+(?:\.\d+)?(?:GiB|MiB|TiB|GB|MB|TB)$").expect("valid size option regex")
    })
}

/// Parses a human-readable byte size (e.g. `"1.5 GiB"`, `"100"` -> MiB).
/// Returns `0` for invalid or non-positive inputs.
pub fn parse_size_bytes(size_text: &str) -> i64 {
    let Some(caps) = size_parse_regex().captures(size_text) else {
        return 0;
    };
    let Ok(value) = caps[1].parse::<f64>() else {
        return 0;
    };
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }

    let unit = caps
        .get(2)
        .map(|m| m.as_str().to_ascii_uppercase())
        .unwrap_or_default();
    let scale = match unit.as_str() {
        "KIB" | "KB" => 1024.0,
        "MIB" | "MB" | "" => 1024.0 * 1024.0,
        "GIB" | "GB" => 1024.0 * 1024.0 * 1024.0,
        "TIB" | "TB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => 1.0,
    };

    (value * scale).round() as i64
}

/// Validates and deduplicates size option strings (case-insensitively).
pub fn validated_size_options(values: &[String]) -> Vec<String> {
    let re = size_option_regex();
    let mut result: Vec<String> = Vec::new();
    for raw in values {
        let value = raw.trim();
        if value.is_empty() || !re.is_match(value) {
            continue;
        }
        if !result
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(value))
        {
            result.push(value.to_string());
        }
    }
    result
}

/// Convenience wrapper for FFI callers passing a JSON-encoded array of CPU options.
pub fn validated_cpu_options_json(json_array: &str) -> Vec<i32> {
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(json_array) else {
        return Vec::new();
    };
    validated_cpu_options(&items)
}

/// Validates and deduplicates CPU core counts in `[1, 1024]` from JSON values.
pub fn validated_cpu_options(values: &[Value]) -> Vec<i32> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for raw in values {
        let parsed = match raw {
            Value::Number(n) => n
                .as_i64()
                .or_else(|| n.as_f64().map(|f| f.round() as i64))
                .and_then(|v| i32::try_from(v).ok()),
            Value::String(s) => s.trim().parse::<i32>().ok(),
            _ => None,
        };
        let Some(cores) = parsed else {
            continue;
        };
        if !(1..=1024).contains(&cores) || !seen.insert(cores) {
            continue;
        }
        result.push(cores);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_size_handles_units_and_defaults() {
        assert_eq!(parse_size_bytes(""), 0);
        assert_eq!(parse_size_bytes("0"), 0);
        assert_eq!(parse_size_bytes("-5GiB"), 0);
        assert_eq!(parse_size_bytes("100"), 100 * 1024 * 1024);
        assert_eq!(parse_size_bytes(" 1.5 GiB "), 1_610_612_736);
        assert_eq!(parse_size_bytes("0.5 B"), 1);
    }

    #[test]
    fn validated_options_filter_and_deduplicate() {
        let sizes = vec![
            "10GiB".into(),
            " 20gb ".into(),
            "10gib".into(),
            "512KiB".into(),
            "abc".into(),
        ];
        assert_eq!(validated_size_options(&sizes), vec!["10GiB", "20gb"]);

        let cpus = vec![
            json!(1),
            json!(0),
            json!(1025),
            json!(" 8 "),
            json!("abc"),
            json!(true),
            json!(null),
        ];
        assert_eq!(validated_cpu_options(&cpus), vec![1, 8]);
    }
}
