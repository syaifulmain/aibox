//! POSIX shell quoting helpers.

/// Wraps `value` in single quotes, escaping embedded single quotes as `'\''`.
pub fn shell_quote(value: &str) -> String {
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

/// Splits `words` on whitespace and quotes each token for shell interpolation.
pub fn shell_array_words(words: &str) -> String {
    words
        .split_whitespace()
        .map(shell_quote)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_and_arrays() {
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("it's 'fine'"), "'it'\\''s '\\''fine'\\'''");
        assert_eq!(
            shell_array_words("  --flag  it's  $VAR\n"),
            "'--flag' 'it'\\''s' '$VAR'"
        );
    }
}
