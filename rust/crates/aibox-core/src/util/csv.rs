//! Comma/semicolon/newline-separated list parsing.

use std::collections::HashSet;

/// Splits `text` on `,`, `;`, or `\n`, trims items, drops empties, and
/// deduplicates while preserving first-seen order.
pub fn parse_csv(text: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut seen = HashSet::new();
    for part in text.split([',', ';', '\n']) {
        let value = part.trim();
        if !value.is_empty() && seen.insert(value) {
            values.push(value.to_string());
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mixed_separators_and_deduplicates() {
        assert_eq!(parse_csv(""), Vec::<String>::new());
        assert_eq!(parse_csv(" a , b ; a\n\n c ; , b , "), vec!["a", "b", "c"]);
    }
}
