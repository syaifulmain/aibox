//! Environment and resource name sanitization.

/// Normalizes `raw_name` to lowercase `[a-z0-9-]`, collapsing non-matching
/// runs to a single `-` and stripping leading/trailing dashes.
pub fn sanitize_name(raw_name: &str) -> String {
    let lowered = raw_name.trim().to_lowercase();
    let mut out = String::with_capacity(lowered.len());
    let mut last_dash = false;
    for c in lowered.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
            last_dash = false;
        } else if !out.is_empty() && !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_environment_name_matches_qt_tests() {
        assert_eq!(
            sanitize_name("  Project POS AI 2026! "),
            "project-pos-ai-2026"
        );
        assert_eq!(sanitize_name("---Mobile__POS---"), "mobile-pos");
        assert_eq!(sanitize_name(""), "");
        assert_eq!(sanitize_name("@@@"), "");
        assert_eq!(sanitize_name("123.Café_Env.v1"), "123-caf-env-v1");
    }
}
