pub mod clone;
pub mod create;
pub mod lifecycle;
pub mod resources;

use std::sync::OnceLock;

use regex::Regex;

pub(crate) fn is_valid_size_spec(value: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)^\d+(?:\.\d+)?(?:GiB|MiB|TiB|GB|MB|TB)$").expect("valid size regex")
    });
    re.is_match(value.trim())
}

pub(crate) fn clean_limit_value(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("unlimited") {
        String::new()
    } else {
        trimmed.to_string()
    }
}
