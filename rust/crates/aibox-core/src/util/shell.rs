//! POSIX shell quoting helpers.

/// Wraps `value` in single quotes, escaping embedded single quotes as `'\''`.
///
/// ```
/// use aibox_core::util::shell::shell_quote;
///
/// assert_eq!(shell_quote("plain"), "'plain'");
/// assert_eq!(shell_quote("$HOME"), "'$HOME'");
/// assert_eq!(shell_quote("it's"), r"'it'\''s'");
/// ```
pub fn shell_quote(value: &str) -> String {
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

/// Splits `words` on whitespace and quotes each token for shell interpolation.
///
/// ```
/// use aibox_core::util::shell::shell_array_words;
///
/// assert_eq!(shell_array_words(" -a  $B "), "'-a' '$B'");
/// ```
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

    /// Deterministic xorshift generator so the property test needs no extra crate.
    fn next_rand(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    #[test]
    fn quoted_random_input_round_trips_through_sh() {
        // Shell metacharacters weighted heavily; NUL cannot appear in argv.
        const ALPHABET: &[char] = &[
            '\'',
            '"',
            '\\',
            '$',
            '`',
            '!',
            '*',
            '?',
            '[',
            ']',
            '{',
            '}',
            '(',
            ')',
            ';',
            '&',
            '|',
            '<',
            '>',
            '~',
            '#',
            '%',
            '=',
            ' ',
            '\t',
            '\n',
            'a',
            'Z',
            '0',
            '-',
            '.',
            '/',
            '\u{e6}',
            '\u{1F600}',
        ];
        let mut state = 0x9E37_79B9_7F4A_7C15_u64;
        let mut script = String::new();
        let mut expected = Vec::new();
        for _ in 0..200 {
            let len = (next_rand(&mut state) % 24) as usize;
            let input: String = (0..len)
                .map(|_| ALPHABET[(next_rand(&mut state) % ALPHABET.len() as u64) as usize])
                .collect();
            // Each case is framed by a sentinel so a quoting leak shows up as a
            // mismatch rather than bleeding silently into the next case.
            script.push_str(&format!("printf '%s\\036' {};", shell_quote(&input)));
            expected.push(input);
        }
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(&script)
            .output()
            .expect("run sh");
        assert!(output.status.success(), "sh failed: {output:?}");
        let stdout = String::from_utf8(output.stdout).expect("utf-8 output");
        let got: Vec<&str> = stdout.split('\u{1e}').collect();
        assert_eq!(got.len(), expected.len() + 1);
        for (got, want) in got.iter().zip(&expected) {
            assert_eq!(got, want);
        }
    }
}
