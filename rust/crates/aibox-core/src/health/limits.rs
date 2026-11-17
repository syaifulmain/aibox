//! Process and I/O resource limit validators.

use regex::Regex;
use std::sync::OnceLock;

/// Validates an Incus `limits.processes` value (`""`, `"unlimited"`, or integer `10..=1_000_000`).
pub fn is_valid_process_limit(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("unlimited") {
        return true;
    }
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^[1-9][0-9]*$").expect("valid process limit regex"));
    if !re.is_match(trimmed) {
        return false;
    }
    match trimmed.parse::<i32>() {
        Ok(count) => (10..=1_000_000).contains(&count),
        Err(_) => false,
    }
}

/// Validates an Incus `limits.read` / `limits.write` value (`""`, `"unlimited"`, or `<n><unit>`).
pub fn is_valid_io_limit(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("unlimited") {
        return true;
    }
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"^[1-9][0-9]*(?:B|kB|KB|MB|GB|KiB|MiB|GiB|iops)$")
            .expect("valid io limit regex")
    });
    re.is_match(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_and_io_limits_match_qt_tests() {
        assert!(is_valid_process_limit(""));
        assert!(is_valid_process_limit("Unlimited"));
        assert!(is_valid_process_limit("512"));
        assert!(is_valid_process_limit("10"));
        assert!(is_valid_process_limit("1000000"));
        assert!(!is_valid_process_limit("5"));
        assert!(!is_valid_process_limit("9"));
        assert!(!is_valid_process_limit("1000001"));
        assert!(!is_valid_process_limit("-1"));
        assert!(!is_valid_process_limit("abc"));

        assert!(is_valid_io_limit(""));
        assert!(is_valid_io_limit("Unlimited"));
        assert!(is_valid_io_limit("200MB"));
        assert!(is_valid_io_limit("2500iops"));
        assert!(!is_valid_io_limit("fast"));
    }
}
