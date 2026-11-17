//! Secret value redaction for logs and byte streams.

/// Replaces every non-empty value in `values` with `[REDACTED]`, longest values first
/// so shared prefixes never leave trailing fragments behind.
pub fn redact_secrets(text: &str, values: &[String]) -> String {
    if text.is_empty() || values.is_empty() {
        return text.to_string();
    }

    let mut unique: Vec<&str> = Vec::with_capacity(values.len());
    for val in values {
        if !val.is_empty() && !unique.contains(&val.as_str()) {
            unique.push(val.as_str());
        }
    }
    unique.sort_by_key(|a| std::cmp::Reverse(a.encode_utf16().count()));

    let mut result = text.to_string();
    for secret_val in unique {
        result = result.replace(secret_val, "[REDACTED]");
    }
    result
}

/// UTF-8 byte-stream wrapper around `redact_secrets`.
pub fn redact_secrets_bytes(data: &[u8], values: &[String]) -> Vec<u8> {
    if data.is_empty() || values.is_empty() {
        return data.to_vec();
    }
    let text = String::from_utf8_lossy(data);
    redact_secrets(&text, values).into_bytes()
}
