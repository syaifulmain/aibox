pub mod clone;
pub mod create;
pub mod error;
pub mod lifecycle;
pub mod resources;
pub mod templates;

pub use error::ProvisionError;

use std::sync::OnceLock;

use regex::Regex;

use crate::health::limits::{is_valid_io_limit, is_valid_process_limit};
use crate::util::names::sanitize_name;

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

pub(crate) fn validate_new_env_name(
    raw_name: &str,
    existing_names: &[String],
) -> Result<String, ProvisionError> {
    let name = sanitize_name(raw_name);
    if name.is_empty() {
        return Err(ProvisionError::InvalidName);
    }
    if existing_names.iter().any(|n| n == &name) {
        return Err(ProvisionError::AlreadyExists(name));
    }
    Ok(name)
}

pub(crate) fn validate_limits(
    process_limit: &str,
    io_limit: &str,
) -> Result<(String, String), ProvisionError> {
    let clean_proc = clean_limit_value(process_limit);
    let clean_io = clean_limit_value(io_limit);
    if !is_valid_process_limit(&clean_proc) {
        return Err(ProvisionError::InvalidProcessLimit);
    }
    if !is_valid_io_limit(&clean_io) {
        return Err(ProvisionError::InvalidIoLimit);
    }
    Ok((clean_proc, clean_io))
}
