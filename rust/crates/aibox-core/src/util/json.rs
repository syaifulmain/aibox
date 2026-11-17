//! Qt-compatible JSON serialization (`QJsonDocument::Indented` and `Compact`).

use serde::Serialize;
use serde_json::ser::{PrettyFormatter, Serializer};

/// Serializes `value` using 4-space indentation and a trailing newline,
/// matching `QJsonDocument::toJson(QJsonDocument::Indented)`.
pub fn to_json_indented<T: Serialize + ?Sized>(value: &T) -> Result<String, serde_json::Error> {
    let mut buf = Vec::new();
    let formatter = PrettyFormatter::with_indent(b"    ");
    let mut ser = Serializer::with_formatter(&mut buf, formatter);
    value.serialize(&mut ser)?;
    buf.push(b'\n');
    // serde_json only emits valid UTF-8.
    Ok(String::from_utf8(buf).expect("serde_json emits valid UTF-8"))
}

/// Serializes `value` with no extra whitespace, matching `QJsonDocument::Compact`.
pub fn to_json_compact<T: Serialize + ?Sized>(value: &T) -> Result<String, serde_json::Error> {
    serde_json::to_string(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn round_trips_qjson_indented_format() {
        let qjson_fixture = concat!(
            "{\n",
            "    \"alpha\": 1,\n",
            "    \"beta\": [\n",
            "        \"one\",\n",
            "        \"two\"\n",
            "    ],\n",
            "    \"nested\": {\n",
            "        \"flag\": true\n",
            "    }\n",
            "}\n"
        );
        let parsed: Value = serde_json::from_str(qjson_fixture).unwrap();
        assert_eq!(to_json_indented(&parsed).unwrap(), qjson_fixture);
        assert_eq!(
            to_json_compact(&json!({"b": 2, "a": 1})).unwrap(),
            r#"{"a":1,"b":2}"#
        );
    }
}
