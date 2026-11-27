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

pub fn is_vm_env_type(env_type: &str) -> bool {
    let trimmed = env_type.trim();
    trimmed.eq_ignore_ascii_case("VM") || trimmed.eq_ignore_ascii_case("VIRTUAL-MACHINE")
}

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

/// True when the value asks Incus to use its own swap default.
///
/// The empty value is intentionally not treated as "host": an empty value
/// means "leave the instance setting untouched" for update and clone plans.
pub(crate) fn is_host_swap_value(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.eq_ignore_ascii_case("host")
        || trimmed.eq_ignore_ascii_case("default")
        || trimmed.eq_ignore_ascii_case("true")
}

/// Maps a validated swap choice to the value written to
/// `limits.memory.swap`. `None` means no explicit value should be written.
pub(crate) fn swap_config_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || is_host_swap_value(trimmed) {
        return None;
    }
    if trimmed.eq_ignore_ascii_case("off")
        || trimmed.eq_ignore_ascii_case("false")
        || trimmed == "0"
    {
        return Some("false".to_string());
    }
    Some(trimmed.to_string())
}

pub(crate) fn validate_swap_limit(value: &str) -> Result<(), ProvisionError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || is_host_swap_value(trimmed)
        || trimmed.eq_ignore_ascii_case("off")
        || trimmed.eq_ignore_ascii_case("false")
        || trimmed.parse::<u64>().is_ok()
        || is_valid_size_spec(trimmed)
    {
        Ok(())
    } else {
        Err(ProvisionError::InvalidSwapLimit)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swap_choices_normalize_for_incus() {
        assert_eq!(swap_config_value("Off"), Some("false".to_string()));
        assert_eq!(swap_config_value("false"), Some("false".to_string()));
        assert_eq!(swap_config_value("0"), Some("false".to_string()));
        assert_eq!(swap_config_value("4GiB"), Some("4GiB".to_string()));
        assert_eq!(
            swap_config_value("4294967296"),
            Some("4294967296".to_string())
        );
        assert_eq!(swap_config_value("Host"), None);
        assert_eq!(swap_config_value("true"), None);
        assert_eq!(swap_config_value(""), None);

        assert!(validate_swap_limit("").is_ok());
        assert!(validate_swap_limit("Host").is_ok());
        assert!(validate_swap_limit("Off").is_ok());
        assert!(validate_swap_limit("0").is_ok());
        assert!(validate_swap_limit("4GiB").is_ok());
        assert!(validate_swap_limit("4294967296").is_ok());
        assert_eq!(
            validate_swap_limit("bogus").unwrap_err(),
            ProvisionError::InvalidSwapLimit
        );
    }
}
