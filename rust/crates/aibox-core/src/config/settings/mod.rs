//! Pure-Rust settings mutators and validation logic, split by UI section.

pub mod catalog;
pub mod locations;
pub mod safety;

pub use catalog::{
    remove_os_entry, reset_create_options, reset_os_catalog, save_create_options, save_os_entry,
    save_preset,
};
pub use locations::{complete_first_run, save_settings, FirstRunPaths, FirstRunRequest};
pub use safety::{
    parse_diff_excludes, save_appearance_settings, save_auto_snapshot_settings, save_diff_excludes,
    save_idle_stop_settings, save_session_recording_settings, save_stale_settings,
    save_watchdog_settings, stale_day_options,
};

use crate::config::error::ConfigError;
use crate::config::model::Config;

/// Parses a raw JSON string directly into a `Config`.
pub fn parse_config_json(raw: &str) -> Result<Config, ConfigError> {
    serde_json::from_str::<Config>(raw).map_err(|e| ConfigError::parse("<memory>", e))
}
