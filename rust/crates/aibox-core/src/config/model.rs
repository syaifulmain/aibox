//! Configuration model, loading, normalization, and atomic persistence.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config::defaults::{
    default_cpu_options, default_diff_excludes, default_disk_options, default_os_entries,
    default_presets, default_ram_options, default_swap_options, OsEntry, Preset,
};
use crate::config::error::ConfigError;
use crate::util::error::IoError;
use crate::util::json::{to_json_compact, to_json_indented};
use crate::util::paths::{atomic_write, normalized_path};
use crate::util::size::{validated_cpu_options, validated_size_options, validated_swap_options};

/// Current configuration schema version.
pub const CURRENT_SCHEMA_VERSION: u32 = 2;

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

fn is_current_schema_version(v: &u32) -> bool {
    *v == CURRENT_SCHEMA_VERSION
}

fn default_theme_mode_override() -> String {
    "auto".to_string()
}

fn is_default_theme_mode_override(v: &str) -> bool {
    v == "auto"
}

fn is_false(v: &bool) -> bool {
    !*v
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub version: i32,
    #[serde(
        default = "default_schema_version",
        skip_serializing_if = "is_current_schema_version"
    )]
    pub schema_version: u32,
    pub first_run_completed: bool,
    pub workspace_base: String,
    pub os_storage_path: String,
    pub os_catalog: Vec<OsEntry>,
    pub presets: Vec<Preset>,
    pub disk_options: Vec<String>,
    pub ram_options: Vec<String>,
    pub swap_options: Vec<String>,
    pub cpu_options: Vec<i32>,
    pub default_preset_id: String,
    pub default_os_id: String,
    pub default_disk: String,
    pub default_ram: String,
    pub default_cpu: i32,
    pub auto_snapshot_enabled: bool,
    pub auto_snapshot_keep: i32,
    pub watchdog_enabled: bool,
    pub watchdog_disk_warn: i32,
    pub watchdog_disk_critical: i32,
    pub watchdog_auto_stop: bool,
    pub idle_stop_minutes: i32,
    pub stale_days: i32,
    pub record_sessions: bool,
    pub diff_excludes: Vec<String>,
    #[serde(
        default = "default_theme_mode_override",
        skip_serializing_if = "is_default_theme_mode_override"
    )]
    pub theme_mode_override: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub high_contrast: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub reduce_motion: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Config {
    pub fn new_defaults(default_workspace_base: &str, default_os_storage_path: &str) -> Self {
        Self {
            version: 1,
            schema_version: CURRENT_SCHEMA_VERSION,
            first_run_completed: false,
            workspace_base: default_workspace_base.to_string(),
            os_storage_path: default_os_storage_path.to_string(),
            os_catalog: default_os_entries(),
            presets: default_presets(),
            disk_options: default_disk_options(),
            ram_options: default_ram_options(),
            swap_options: default_swap_options(),
            cpu_options: default_cpu_options(),
            default_preset_id: "full".to_string(),
            default_os_id: "ubuntu-24.04".to_string(),
            default_disk: "35GiB".to_string(),
            default_ram: "6GiB".to_string(),
            default_cpu: 4,
            auto_snapshot_enabled: true,
            auto_snapshot_keep: 5,
            watchdog_enabled: true,
            watchdog_disk_warn: 90,
            watchdog_disk_critical: 97,
            watchdog_auto_stop: false,
            idle_stop_minutes: 0,
            stale_days: 30,
            record_sessions: false,
            diff_excludes: default_diff_excludes(),
            theme_mode_override: default_theme_mode_override(),
            high_contrast: false,
            reduce_motion: false,
            extra: BTreeMap::new(),
        }
    }

    pub fn normalize_defaults(&mut self) {
        if !self.presets.iter().any(|p| p.id == self.default_preset_id) {
            self.default_preset_id = self
                .presets
                .first()
                .map(|p| p.id.clone())
                .unwrap_or_else(|| "standard".to_string());
        }

        if !self.disk_options.contains(&self.default_disk) {
            self.default_disk = self
                .disk_options
                .first()
                .cloned()
                .unwrap_or_else(|| "35GiB".to_string());
        }

        if !self.ram_options.contains(&self.default_ram) {
            self.default_ram = self
                .ram_options
                .first()
                .cloned()
                .unwrap_or_else(|| "6GiB".to_string());
        }

        if !self.cpu_options.contains(&self.default_cpu) {
            self.default_cpu = self.cpu_options.first().copied().unwrap_or(4);
        }

        if self.swap_options.is_empty() {
            self.swap_options = default_swap_options();
        }
        let missing_required: Vec<&str> = ["Off", "Host"]
            .into_iter()
            .filter(|required| {
                !self
                    .swap_options
                    .iter()
                    .any(|value| value.eq_ignore_ascii_case(required))
            })
            .collect();
        if !missing_required.is_empty() {
            let keep = 4usize.saturating_sub(missing_required.len());
            self.swap_options.truncate(keep);
            for required in missing_required {
                self.swap_options.push(required.to_string());
            }
        }

        if !self.os_catalog.iter().any(|e| e.id == self.default_os_id) {
            self.default_os_id = self
                .os_catalog
                .first()
                .map(|e| e.id.clone())
                .unwrap_or_else(|| "ubuntu-24.04".to_string());
        }
    }

    pub fn preset_by_id(&self, id: &str) -> Option<&Preset> {
        self.presets.iter().find(|p| p.id == id)
    }

    pub fn os_entry_by_id(&self, id: &str) -> Option<&OsEntry> {
        self.os_catalog.iter().find(|e| e.id == id)
    }
}

fn json_val_to_int(v: &Value) -> Option<i32> {
    if let Some(i) = v.as_i64() {
        i32::try_from(i).ok()
    } else if let Some(u) = v.as_u64() {
        i32::try_from(u).ok()
    } else if let Some(f) = v.as_f64() {
        if f.is_finite() && f >= i32::MIN as f64 && f <= i32::MAX as f64 {
            Some(f as i32)
        } else {
            None
        }
    } else {
        None
    }
}

fn json_strings(arr: &[Value]) -> Vec<String> {
    let mut values = Vec::new();
    for v in arr {
        if let Some(s) = v.as_str() {
            values.push(s.to_string());
        } else if let Some(i) = json_val_to_int(v) {
            values.push(i.to_string());
        }
    }
    values
}

fn detect_schema_version(obj: &Map<String, Value>) -> u32 {
    obj.get("schemaVersion")
        .and_then(json_val_to_int)
        .filter(|&v| v > 0)
        .map(|v| v as u32)
        .unwrap_or(1)
}

/// Migrates a v1 configuration JSON object (without `schemaVersion` or with `schemaVersion: 1`)
/// to v2 in place.
pub fn migrate_v1_to_v2(obj: &mut Map<String, Value>) {
    obj.entry("version".to_string())
        .or_insert_with(|| Value::from(1));
    obj.entry("themeModeOverride".to_string())
        .or_insert_with(|| Value::from("auto"));
    obj.entry("highContrast".to_string())
        .or_insert_with(|| Value::from(false));
    obj.entry("reduceMotion".to_string())
        .or_insert_with(|| Value::from(false));
    obj.insert("schemaVersion".to_string(), Value::from(2));
}

fn migrate_object(obj: &mut Map<String, Value>) -> u32 {
    let mut version = detect_schema_version(obj);
    if version < 2 {
        migrate_v1_to_v2(obj);
        version = 2;
    }
    version
}

fn apply_object(
    obj: &mut Map<String, Value>,
    default_workspace_base: &str,
    default_os_storage_path: &str,
) -> Config {
    let schema_version = migrate_object(obj);
    let mut cfg = Config::new_defaults(default_workspace_base, default_os_storage_path);
    cfg.schema_version = schema_version;

    cfg.first_run_completed = obj
        .get("firstRunCompleted")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if let Some(ws) = obj.get("workspaceBase").and_then(|v| v.as_str()) {
        let trimmed = ws.trim();
        if !trimmed.is_empty() {
            cfg.workspace_base = normalized_path(trimmed, default_workspace_base);
        }
    }

    if let Some(os_st) = obj.get("osStoragePath").and_then(|v| v.as_str()) {
        let trimmed = os_st.trim();
        if !trimmed.is_empty() {
            cfg.os_storage_path = normalized_path(trimmed, default_os_storage_path);
        }
    }

    if let Some(arr) = obj.get("osCatalog").and_then(|v| v.as_array()) {
        let mut entries = Vec::new();
        let mut seen = HashSet::new();
        for item in arr {
            let Some(source) = item.as_object() else {
                continue;
            };
            let id = source
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_lowercase();
            if id.is_empty() || seen.contains(&id) {
                continue;
            }
            seen.insert(id.clone());

            let alias = source
                .get("alias")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let name = source
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let badge = source
                .get("badge")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let description = source
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();

            entries.push(OsEntry {
                id: id.clone(),
                alias: if alias.is_empty() {
                    id.clone()
                } else {
                    alias.to_string()
                },
                name: if name.is_empty() {
                    id
                } else {
                    name.to_string()
                },
                badge: if badge.is_empty() {
                    "Custom".to_string()
                } else {
                    badge.to_string()
                },
                description: description.to_string(),
            });
        }
        if !entries.is_empty() {
            cfg.os_catalog = entries;
        }
    }

    if let Some(arr) = obj.get("presets").and_then(|v| v.as_array()) {
        let defaults = default_presets();
        let mut presets = Vec::new();
        for item in arr {
            let Some(source) = item.as_object() else {
                continue;
            };
            let id = source
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let title = source
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if id.is_empty() || title.is_empty() {
                continue;
            }
            let fallback = defaults.iter().find(|p| p.id == id);
            let sub = source
                .get("sub")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let flags_raw = source
                .get("flags")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let flags = if flags_raw.is_empty() {
                fallback
                    .map(|f| f.flags.clone())
                    .unwrap_or_else(|| "-p default".to_string())
            } else {
                flags_raw.to_string()
            };
            let x11 = if let Some(v) = source.get("x11") {
                v.as_bool().unwrap_or(false)
            } else {
                fallback.map(|f| f.x11).unwrap_or(false)
            };

            presets.push(Preset {
                id: id.to_string(),
                title: title.to_string(),
                sub: sub.to_string(),
                flags,
                x11,
            });
        }
        if !presets.is_empty() {
            cfg.presets = presets;
        }
    }

    if let Some(arr) = obj.get("diskOptions").and_then(|v| v.as_array()) {
        let disk = validated_size_options(&json_strings(arr));
        if !disk.is_empty() {
            cfg.disk_options = disk;
        }
    }

    if let Some(arr) = obj.get("ramOptions").and_then(|v| v.as_array()) {
        let ram = validated_size_options(&json_strings(arr));
        if !ram.is_empty() {
            cfg.ram_options = ram;
        }
    }

    if let Some(arr) = obj.get("swapOptions").and_then(|v| v.as_array()) {
        let swap = validated_swap_options(&json_strings(arr));
        if !swap.is_empty() {
            cfg.swap_options = swap;
        }
    }

    if let Some(arr) = obj.get("cpuOptions").and_then(|v| v.as_array()) {
        let mut cpu_vals = Vec::new();
        for item in arr {
            if let Some(i) = json_val_to_int(item) {
                cpu_vals.push(Value::from(i));
            } else if let Some(s) = item.as_str() {
                let parsed = s.trim().parse::<i32>().unwrap_or(0);
                cpu_vals.push(Value::from(parsed));
            }
        }
        let cpus = validated_cpu_options(&cpu_vals);
        if !cpus.is_empty() {
            cfg.cpu_options = cpus;
        }
    }

    if let Some(s) = obj.get("defaultPresetId").and_then(|v| v.as_str()) {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            cfg.default_preset_id = trimmed.to_string();
        }
    }
    if let Some(s) = obj.get("defaultOsId").and_then(|v| v.as_str()) {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            cfg.default_os_id = trimmed.to_string();
        }
    }
    if let Some(s) = obj.get("defaultDisk").and_then(|v| v.as_str()) {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            cfg.default_disk = trimmed.to_string();
        }
    }
    if let Some(s) = obj.get("defaultRam").and_then(|v| v.as_str()) {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            cfg.default_ram = trimmed.to_string();
        }
    }
    if let Some(v) = obj.get("defaultCpu") {
        if v.is_number() {
            if let Some(cores) = json_val_to_int(v) {
                if cores > 0 && cores <= 1024 {
                    cfg.default_cpu = cores;
                }
            }
        }
    }

    if let Some(v) = obj.get("autoSnapshotEnabled") {
        cfg.auto_snapshot_enabled = v.as_bool().unwrap_or(true);
    }
    if let Some(v) = obj.get("autoSnapshotKeep") {
        if v.is_number() {
            let keep = json_val_to_int(v).unwrap_or(5);
            cfg.auto_snapshot_keep = keep.clamp(1, 50);
        }
    }

    if let Some(v) = obj.get("watchdogEnabled") {
        cfg.watchdog_enabled = v.as_bool().unwrap_or(true);
    }
    if let Some(v) = obj.get("watchdogDiskWarn") {
        if v.is_number() {
            let warn = json_val_to_int(v).unwrap_or(90);
            cfg.watchdog_disk_warn = warn.clamp(50, 98);
        }
    }
    if let Some(v) = obj.get("watchdogDiskCritical") {
        if v.is_number() {
            let crit = json_val_to_int(v).unwrap_or(97);
            cfg.watchdog_disk_critical = crit.clamp(cfg.watchdog_disk_warn + 1, 99);
        }
    }
    if let Some(v) = obj.get("watchdogAutoStop") {
        cfg.watchdog_auto_stop = v.as_bool().unwrap_or(false);
    }
    if let Some(v) = obj.get("idleStopMinutes") {
        if v.is_number() {
            let mins = json_val_to_int(v).unwrap_or(0);
            cfg.idle_stop_minutes = mins.clamp(0, 1440);
        }
    }
    if let Some(v) = obj.get("staleDays") {
        if v.is_number() {
            let days = json_val_to_int(v).unwrap_or(30);
            cfg.stale_days = days.clamp(1, 3650);
        }
    }
    if let Some(v) = obj.get("recordSessions") {
        cfg.record_sessions = v.as_bool().unwrap_or(false);
    }
    if let Some(s) = obj.get("themeModeOverride").and_then(|v| v.as_str()) {
        let normalized = s.trim().to_ascii_lowercase();
        if matches!(normalized.as_str(), "auto" | "light" | "dark") {
            cfg.theme_mode_override = normalized;
        }
    }
    if let Some(v) = obj.get("highContrast") {
        cfg.high_contrast = v.as_bool().unwrap_or(false);
    }
    if let Some(v) = obj.get("reduceMotion") {
        cfg.reduce_motion = v.as_bool().unwrap_or(false);
    }

    if let Some(arr) = obj.get("diffExcludes").and_then(|v| v.as_array()) {
        let mut loaded = Vec::new();
        for item in arr {
            let pattern = item.as_str().unwrap_or("").trim();
            if !pattern.is_empty() && !loaded.iter().any(|s: &String| s == pattern) {
                loaded.push(pattern.to_string());
            }
        }
        cfg.diff_excludes = loaded;
    }

    const KNOWN_KEYS: &[&str] = &[
        "version",
        "schemaVersion",
        "firstRunCompleted",
        "workspaceBase",
        "osStoragePath",
        "osCatalog",
        "presets",
        "diskOptions",
        "ramOptions",
        "cpuOptions",
        "defaultPresetId",
        "defaultOsId",
        "defaultDisk",
        "defaultRam",
        "defaultCpu",
        "autoSnapshotEnabled",
        "autoSnapshotKeep",
        "watchdogEnabled",
        "watchdogDiskWarn",
        "watchdogDiskCritical",
        "watchdogAutoStop",
        "idleStopMinutes",
        "staleDays",
        "recordSessions",
        "diffExcludes",
        "themeModeOverride",
        "highContrast",
        "reduceMotion",
    ];
    for (k, v) in obj.iter() {
        if !KNOWN_KEYS.contains(&k.as_str()) {
            cfg.extra.insert(k.clone(), v.clone());
        }
    }

    cfg.normalize_defaults();
    cfg
}

fn load_from_str_checked_with_path(
    json_str: &str,
    path: &Path,
    default_workspace_base: &str,
    default_os_storage_path: &str,
) -> Result<Config, ConfigError> {
    let val = serde_json::from_str::<Value>(json_str).map_err(|e| ConfigError::parse(path, e))?;
    match val {
        Value::Object(mut obj) => Ok(apply_object(
            &mut obj,
            default_workspace_base,
            default_os_storage_path,
        )),
        _ => Err(ConfigError::invalid(
            "root",
            "config root must be a JSON object",
        )),
    }
}

/// Loads and normalizes a config from a JSON string, returning a structured error on failure.
pub fn load_from_str_checked(
    json_str: &str,
    default_workspace_base: &str,
    default_os_storage_path: &str,
) -> Result<Config, ConfigError> {
    load_from_str_checked_with_path(
        json_str,
        Path::new("<memory>"),
        default_workspace_base,
        default_os_storage_path,
    )
}

/// Loads and normalizes a config from a JSON string, falling back to defaults on error.
pub fn load_from_str(
    json_str: &str,
    default_workspace_base: &str,
    default_os_storage_path: &str,
) -> Config {
    load_from_str_checked(json_str, default_workspace_base, default_os_storage_path)
        .unwrap_or_else(|_| Config::new_defaults(default_workspace_base, default_os_storage_path))
}

/// Loads and normalizes `config.json` from `path`, returning a structured error on failure.
pub fn load_checked(
    path: &str,
    default_workspace_base: &str,
    default_os_storage_path: &str,
) -> Result<Config, ConfigError> {
    let path_ref = Path::new(path);
    let contents = fs::read_to_string(path_ref).map_err(|e| IoError::new("read", path_ref, e))?;
    load_from_str_checked_with_path(
        &contents,
        path_ref,
        default_workspace_base,
        default_os_storage_path,
    )
}

/// Loads and normalizes `config.json` from `path`, falling back to defaults on error.
pub fn load(path: &str, default_workspace_base: &str, default_os_storage_path: &str) -> Config {
    load_checked(path, default_workspace_base, default_os_storage_path)
        .unwrap_or_else(|_| Config::new_defaults(default_workspace_base, default_os_storage_path))
}

fn write_json_if_changed(path: &Path, val: &Value) -> Result<(), ConfigError> {
    let json_text = to_json_indented(val).map_err(|e| {
        ConfigError::invalid("config", format!("failed to format config JSON: {e}"))
    })?;
    if fs::read(path).ok().as_deref() == Some(json_text.as_bytes()) {
        return Ok(());
    }
    atomic_write(path, json_text.as_bytes())?;
    Ok(())
}

/// Saves `config` atomically to `path` in Qt-compatible indented JSON format, skipping the write
/// if the on-disk content is already identical.
pub fn save(path: &str, config: &Config) -> Result<(), ConfigError> {
    let val = serde_json::to_value(config).map_err(|e| {
        ConfigError::invalid("config", format!("failed to convert config to JSON: {e}"))
    })?;
    write_json_if_changed(Path::new(path), &val)
}

/// Saves a JSON config string atomically to `path` in Qt-compatible indented JSON format,
/// skipping the write if the on-disk content is already identical.
pub fn save_json_str(path: &str, json_str: &str) -> Result<(), ConfigError> {
    let path_ref = Path::new(path);
    let val: Value = serde_json::from_str(json_str).map_err(|e| ConfigError::parse(path_ref, e))?;
    write_json_if_changed(path_ref, &val)
}

/// Normalizes a config JSON string and returns compact JSON (for FFI).
pub fn normalize_json_str(json_str: &str) -> Result<String, ConfigError> {
    let mut cfg: Config =
        serde_json::from_str(json_str).map_err(|e| ConfigError::parse("<memory>", e))?;
    cfg.normalize_defaults();
    to_json_compact(&cfg)
        .map_err(|e| ConfigError::invalid("config", format!("failed to serialize config: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::paths::{
        default_os_storage_path, default_system_path, default_workspace_path, home_dir,
        resolve_workspace_path,
    };
    use serde_json::json;
    use std::env;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn defaults_are_home_aibox() {
        let _guard = env_lock().lock().unwrap();
        let home = home_dir().to_string_lossy().into_owned();
        let home_aibox = format!("{home}/aibox");
        assert_eq!(default_system_path(), home_aibox);
        assert_eq!(default_workspace_path(), format!("{home_aibox}/workspaces"));
        assert_eq!(default_os_storage_path(), format!("{home_aibox}/os-images"));
    }

    #[test]
    fn resolve_workspace_path_handles_custom_base_and_validation() {
        let _guard = env_lock().lock().unwrap();
        let base = "/tmp/aibox-test-ws";
        let custom = "/tmp/aibox-test-projects";

        assert_eq!(
            resolve_workspace_path("", "demo", base).unwrap(),
            "/tmp/aibox-test-ws/demo"
        );
        assert_eq!(
            resolve_workspace_path(custom, "demo", base).unwrap(),
            "/tmp/aibox-test-projects/demo"
        );
        assert_eq!(
            resolve_workspace_path(&format!("{custom}/nested"), "demo", base).unwrap(),
            "/tmp/aibox-test-projects/nested/demo"
        );

        assert!(resolve_workspace_path("relative/path", "demo", base).is_err());
        assert!(resolve_workspace_path("/", "demo", base).is_err());
        let home = home_dir().to_string_lossy().into_owned();
        assert!(resolve_workspace_path(&home, "demo", base).is_err());
    }

    #[test]
    fn config_load_save_round_trips_and_preserves_extra_keys() {
        let tmp = env::temp_dir().join(format!("aibox-cfg-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let cfg_path = tmp.join("config.json");
        let cfg_path_str = cfg_path.to_string_lossy().into_owned();

        // Missing file -> first_run_completed is false.
        let missing = load(&cfg_path_str, "/tmp/ws", "/tmp/os");
        assert!(!missing.first_run_completed);

        // Save with extra unknown keys and reload.
        let mut cfg = Config::new_defaults("/tmp/ws", "/tmp/os");
        cfg.first_run_completed = true;
        cfg.extra
            .insert("customPluginFlag".to_string(), json!(true));
        cfg.extra.insert("extraNested".to_string(), json!({"a": 1}));
        save(&cfg_path_str, &cfg).unwrap();

        let reloaded = load(&cfg_path_str, "/tmp/ws", "/tmp/os");
        assert!(reloaded.first_run_completed);
        assert_eq!(reloaded.extra.get("customPluginFlag"), Some(&json!(true)));
        assert_eq!(reloaded.extra.get("extraNested"), Some(&json!({"a": 1})));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn broken_and_truncated_config_returns_structured_error_and_falls_back_to_defaults() {
        let tmp = env::temp_dir().join(format!("aibox-cfg-broken-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let cfg_path = tmp.join("config.json");
        let cfg_path_str = cfg_path.to_string_lossy().into_owned();

        // Truncated JSON on disk.
        let truncated = r#"{ "version": 1, "firstRunCompleted": true, "workspaceBase": "/tmp/"#;
        fs::write(&cfg_path, truncated).unwrap();

        let err = load_checked(&cfg_path_str, "/tmp/ws", "/tmp/os").unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
        assert!(err.to_string().contains(&cfg_path_str));

        // Non-object root returns ConfigError::Invalid.
        let arr_err = load_from_str_checked("[1, 2, 3]", "/tmp/ws", "/tmp/os").unwrap_err();
        assert!(matches!(
            arr_err,
            ConfigError::Invalid { field: "root", .. }
        ));

        // Fallback load still returns valid defaults so the app starts cleanly.
        let fallback = load(&cfg_path_str, "/tmp/ws", "/tmp/os");
        assert_eq!(fallback, Config::new_defaults("/tmp/ws", "/tmp/os"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn schema_migrates_from_v1_and_preserves_v2() {
        // v1 fixture: no schemaVersion key, legacy fields only.
        let v1_implicit = r#"{
            "version": 1,
            "firstRunCompleted": true,
            "workspaceBase": "/tmp/v1-ws",
            "osStoragePath": "/tmp/v1-os",
            "staleDays": 60
        }"#;
        let cfg_v1 = load_from_str_checked(v1_implicit, "/tmp/ws", "/tmp/os").expect("v1 loads");
        assert_eq!(cfg_v1.schema_version, CURRENT_SCHEMA_VERSION);
        assert!(cfg_v1.first_run_completed);
        assert_eq!(cfg_v1.workspace_base, "/tmp/v1-ws");
        assert_eq!(cfg_v1.stale_days, 60);
        assert_eq!(cfg_v1.theme_mode_override, "auto");
        assert!(!cfg_v1.extra.contains_key("schemaVersion"));

        // Explicit schemaVersion: 1 fixture.
        let v1_explicit = r#"{
            "version": 1,
            "schemaVersion": 1,
            "firstRunCompleted": false,
            "idleStopMinutes": 45
        }"#;
        let cfg_v1_exp =
            load_from_str_checked(v1_explicit, "/tmp/ws", "/tmp/os").expect("v1 explicit");
        assert_eq!(cfg_v1_exp.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(cfg_v1_exp.idle_stop_minutes, 45);

        // v2 fixture: already at current schema version.
        let v2 = r#"{
            "version": 1,
            "schemaVersion": 2,
            "firstRunCompleted": true,
            "themeModeOverride": "dark",
            "highContrast": true
        }"#;
        let cfg_v2 = load_from_str_checked(v2, "/tmp/ws", "/tmp/os").expect("v2 loads");
        assert_eq!(cfg_v2.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(cfg_v2.theme_mode_override, "dark");
        assert!(cfg_v2.high_contrast);
    }

    #[test]
    fn save_skips_disk_write_when_content_is_unchanged() {
        let tmp = env::temp_dir().join(format!("aibox-cfg-noop-save-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let cfg_path = tmp.join("config.json");
        let cfg_path_str = cfg_path.to_string_lossy().into_owned();

        let mut cfg = Config::new_defaults("/tmp/ws", "/tmp/os");
        save(&cfg_path_str, &cfg).unwrap();
        let mtime_first = fs::metadata(&cfg_path).unwrap().modified().unwrap();

        // Sleep briefly so any rewrite would produce a strictly newer timestamp.
        std::thread::sleep(std::time::Duration::from_millis(20));
        save(&cfg_path_str, &cfg).unwrap();
        let mtime_second = fs::metadata(&cfg_path).unwrap().modified().unwrap();
        assert_eq!(mtime_first, mtime_second);

        // Mutating a field must trigger an actual rewrite.
        std::thread::sleep(std::time::Duration::from_millis(20));
        cfg.stale_days = 90;
        save(&cfg_path_str, &cfg).unwrap();
        let mtime_third = fs::metadata(&cfg_path).unwrap().modified().unwrap();
        assert!(mtime_third > mtime_second);

        let _ = fs::remove_dir_all(&tmp);
    }
}
