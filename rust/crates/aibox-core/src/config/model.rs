//! Configuration model, loading, normalization, and atomic persistence.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config::defaults::{
    default_cpu_options, default_diff_excludes, default_disk_options, default_os_entries,
    default_presets, default_ram_options, OsEntry, Preset,
};
use crate::util::json::{to_json_compact, to_json_indented};
use crate::util::paths::{atomic_write, normalized_path};
use crate::util::size::{validated_cpu_options, validated_size_options};

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
    pub first_run_completed: bool,
    pub workspace_base: String,
    pub os_storage_path: String,
    pub os_catalog: Vec<OsEntry>,
    pub presets: Vec<Preset>,
    pub disk_options: Vec<String>,
    pub ram_options: Vec<String>,
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
            first_run_completed: false,
            workspace_base: default_workspace_base.to_string(),
            os_storage_path: default_os_storage_path.to_string(),
            os_catalog: default_os_entries(),
            presets: default_presets(),
            disk_options: default_disk_options(),
            ram_options: default_ram_options(),
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

fn apply_object(
    obj: &Map<String, Value>,
    default_workspace_base: &str,
    default_os_storage_path: &str,
) -> Config {
    let mut cfg = Config::new_defaults(default_workspace_base, default_os_storage_path);

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
    for (k, v) in obj {
        if !KNOWN_KEYS.contains(&k.as_str()) {
            cfg.extra.insert(k.clone(), v.clone());
        }
    }

    cfg.normalize_defaults();
    cfg
}

/// Loads and normalizes a config from a JSON string.
pub fn load_from_str(
    json_str: &str,
    default_workspace_base: &str,
    default_os_storage_path: &str,
) -> Config {
    match serde_json::from_str::<Value>(json_str) {
        Ok(Value::Object(obj)) => {
            apply_object(&obj, default_workspace_base, default_os_storage_path)
        }
        _ => Config::new_defaults(default_workspace_base, default_os_storage_path),
    }
}

/// Loads and normalizes `config.json` from `path`.
pub fn load(path: &str, default_workspace_base: &str, default_os_storage_path: &str) -> Config {
    match fs::read_to_string(path) {
        Ok(contents) => load_from_str(&contents, default_workspace_base, default_os_storage_path),
        Err(_) => Config::new_defaults(default_workspace_base, default_os_storage_path),
    }
}

/// Saves `config` atomically to `path` in Qt-compatible indented JSON format.
pub fn save(path: &str, config: &Config) -> Result<(), String> {
    let val = serde_json::to_value(config)
        .map_err(|e| format!("failed to convert config to JSON: {e}"))?;
    let json_text =
        to_json_indented(&val).map_err(|e| format!("failed to format config JSON: {e}"))?;
    atomic_write(Path::new(path), json_text.as_bytes()).map_err(String::from)
}

/// Saves a JSON config string atomically to `path` in Qt-compatible indented JSON format.
pub fn save_json_str(path: &str, json_str: &str) -> Result<(), String> {
    let val: Value =
        serde_json::from_str(json_str).map_err(|e| format!("invalid config JSON: {e}"))?;
    let json_text =
        to_json_indented(&val).map_err(|e| format!("failed to format config JSON: {e}"))?;
    atomic_write(Path::new(path), json_text.as_bytes()).map_err(String::from)
}

/// Normalizes a config JSON string and returns compact JSON (for FFI).
pub fn normalize_json_str(json_str: &str) -> Result<String, String> {
    let mut cfg: Config =
        serde_json::from_str(json_str).map_err(|e| format!("invalid config JSON: {e}"))?;
    cfg.normalize_defaults();
    to_json_compact(&cfg).map_err(|e| format!("failed to serialize config: {e}"))
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
}
