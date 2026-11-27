pub mod defaults;
pub mod error;
pub mod manifest;
pub mod model;
pub mod settings;

pub use defaults::{
    default_cpu_options, default_diff_excludes, default_disk_options, default_os_entries,
    default_presets, default_ram_options, OsEntry, Preset,
};
pub use error::ConfigError;
pub use model::{
    load, load_checked, load_from_str, load_from_str_checked, migrate_v1_to_v2, normalize_json_str,
    save, save_json_str, Config, CURRENT_SCHEMA_VERSION,
};
