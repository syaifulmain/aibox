//! Pure-Rust settings mutators and validation logic.

use std::fs;
use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use crate::config::defaults::{
    default_cpu_options, default_disk_options, default_os_entries, default_presets,
    default_ram_options, OsEntry,
};
use crate::config::model::Config;
use crate::util::csv::parse_csv;
use crate::util::paths::{
    default_os_storage_path as default_os_storage_path_fn, default_system_path,
    default_workspace_path, normalized_path,
};
use crate::util::size::{validated_cpu_options, validated_size_options};

pub fn parse_config_json(raw: &str) -> Result<Config, String> {
    serde_json::from_str::<Config>(raw).map_err(|e| format!("Invalid configuration JSON: {e}"))
}

#[derive(Debug, Clone)]
pub struct FirstRunRequest<'a> {
    pub system_path: &'a str,
    pub current_system_path: &'a str,
    pub current_workspace_base: &'a str,
    pub current_os_storage_path: &'a str,
    pub current_state_dir: &'a str,
    pub current_config_path: &'a str,
    pub is_project_dir_mode: bool,
    pub remember_system_location: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirstRunPaths {
    pub system_path: String,
    pub workspace_base: String,
    pub os_storage_path: String,
    pub state_dir: String,
    pub config_path: String,
}

fn ensure_writable_dir(path: &str, create_err: &str, write_err: &str) -> Result<(), String> {
    if fs::create_dir_all(path).is_err() {
        return Err(format!("{create_err}: {path}"));
    }
    match fs::metadata(path) {
        Ok(meta) if meta.is_dir() && !meta.permissions().readonly() => Ok(()),
        _ => Err(format!("{write_err}: {path}")),
    }
}

pub fn complete_first_run(req: &FirstRunRequest<'_>) -> Result<FirstRunPaths, String> {
    let fallback = if req.current_system_path.is_empty() {
        default_system_path()
    } else {
        req.current_system_path.to_string()
    };
    let target_root = normalized_path(req.system_path, &fallback);
    ensure_writable_dir(
        &target_root,
        "Could not create the system directory",
        "The system directory is not writable",
    )?;

    let use_structured = req.is_project_dir_mode
        || req.remember_system_location
        || !req.system_path.trim().is_empty();

    let next_workspace = if use_structured {
        format!("{target_root}/workspaces")
    } else {
        req.current_workspace_base.to_string()
    };
    let next_os_storage = if use_structured {
        format!("{target_root}/os-images")
    } else {
        req.current_os_storage_path.to_string()
    };
    let next_state_dir = if use_structured {
        format!("{target_root}/state")
    } else {
        req.current_state_dir.to_string()
    };
    let next_config_path = if use_structured {
        format!("{target_root}/config.json")
    } else {
        req.current_config_path.to_string()
    };

    ensure_writable_dir(
        &next_workspace,
        "Could not create the workspace directory",
        "The workspace directory is not writable",
    )?;
    ensure_writable_dir(
        &next_os_storage,
        "Could not create the OS storage directory",
        "The OS storage directory is not writable",
    )?;
    ensure_writable_dir(
        &next_state_dir,
        "Could not create the state directory",
        "The state directory is not writable",
    )?;

    Ok(FirstRunPaths {
        system_path: target_root,
        workspace_base: next_workspace,
        os_storage_path: next_os_storage,
        state_dir: next_state_dir,
        config_path: next_config_path,
    })
}

pub fn save_settings(
    cfg: &Config,
    workspace_path: &str,
    os_storage_path: &str,
    default_workspace_base: &str,
    default_os_storage_path: &str,
) -> Result<Config, String> {
    let ws_fallback = if default_workspace_base.is_empty() {
        default_workspace_path()
    } else {
        default_workspace_base.to_string()
    };
    let os_fallback = if default_os_storage_path.is_empty() {
        default_os_storage_path_fn()
    } else {
        default_os_storage_path.to_string()
    };

    let workspace = normalized_path(workspace_path, &ws_fallback);
    let os_storage = normalized_path(os_storage_path, &os_fallback);

    ensure_writable_dir(
        &workspace,
        "Could not create the workspace directory",
        "The workspace directory is not writable",
    )?;
    ensure_writable_dir(
        &os_storage,
        "Could not create the OS storage directory",
        "The OS storage directory is not writable",
    )?;

    let mut next = cfg.clone();
    next.workspace_base = workspace;
    next.os_storage_path = os_storage;
    next.first_run_completed = true;
    Ok(next)
}

fn os_id_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-z0-9][a-z0-9._-]{0,63}$").expect("valid os id regex"))
}

fn os_alias_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._/-]{0,127}$").expect("valid alias regex")
    })
}

pub fn save_os_entry(
    cfg: &Config,
    original_id: &str,
    id: &str,
    alias: &str,
    name: &str,
    badge: &str,
    description: &str,
) -> Result<Config, String> {
    let new_id = id.trim().to_lowercase();
    if !os_id_regex().is_match(&new_id) {
        return Err(
            "OS ID must use lowercase letters, numbers, dots, dashes, or underscores.".to_string(),
        );
    }

    let trimmed_alias = alias.trim();
    let new_alias = if trimmed_alias.is_empty() {
        new_id.clone()
    } else {
        trimmed_alias.to_string()
    };
    if !os_alias_regex().is_match(&new_alias) {
        return Err(
            "Image alias must use letters, numbers, dots, slashes, dashes, or underscores."
                .to_string(),
        );
    }

    let trimmed_name = name.trim();
    let new_name = if trimmed_name.is_empty() {
        new_alias.clone()
    } else {
        trimmed_name.to_string()
    };
    let trimmed_badge = badge.trim();
    let new_badge = if trimmed_badge.is_empty() {
        "Custom".to_string()
    } else {
        trimmed_badge.to_string()
    };
    let new_description = description.trim().to_string();

    let mut found_index: Option<usize> = None;
    for (i, entry) in cfg.os_catalog.iter().enumerate() {
        if !original_id.is_empty() && entry.id == original_id {
            found_index = Some(i);
        }
        if entry.id == new_id && entry.id != original_id {
            return Err(format!(
                "An OS entry with the ID '{new_id}' already exists."
            ));
        }
    }
    if !original_id.is_empty() && found_index.is_none() {
        return Err(format!("The OS entry '{original_id}' no longer exists."));
    }

    let entry = OsEntry {
        id: new_id.clone(),
        alias: new_alias,
        name: new_name,
        badge: new_badge,
        description: new_description,
    };

    let mut next = cfg.clone();
    if let Some(idx) = found_index {
        next.os_catalog[idx] = entry;
    } else {
        next.os_catalog.push(entry);
    }

    if next.default_os_id == original_id {
        next.default_os_id = new_id;
    }

    Ok(next)
}

pub fn remove_os_entry(cfg: &Config, id: &str) -> Result<Config, String> {
    if cfg.os_catalog.len() <= 1 {
        return Err("Keep at least one OS entry in the catalog.".to_string());
    }

    let target = id.trim();
    let Some(idx) = cfg.os_catalog.iter().position(|e| e.id == target) else {
        return Err(format!("The OS entry '{target}' was not found."));
    };

    let mut next = cfg.clone();
    next.os_catalog.remove(idx);
    next.normalize_defaults();
    Ok(next)
}

pub fn reset_os_catalog(cfg: &Config) -> Config {
    let mut next = cfg.clone();
    next.os_catalog = default_os_entries();
    next.default_os_id = "ubuntu-24.04".to_string();
    next.normalize_defaults();
    next
}

pub fn save_create_options(
    cfg: &Config,
    disk_csv: &str,
    ram_csv: &str,
    cpu_csv: &str,
) -> Result<Config, String> {
    let raw_disk = parse_csv(disk_csv);
    let disk = validated_size_options(&raw_disk);
    if disk.is_empty() {
        return Err("Add at least one disk quota value, for example 35GiB.".to_string());
    }
    if disk.len() != raw_disk.len() {
        return Err("Disk quota values must look like 35GiB, 60GiB, or 512MiB.".to_string());
    }

    let raw_ram = parse_csv(ram_csv);
    let ram = validated_size_options(&raw_ram);
    if ram.is_empty() {
        return Err("Add at least one RAM limit value, for example 6GiB.".to_string());
    }
    if ram.len() != raw_ram.len() {
        return Err("RAM limit values must look like 4GiB, 6GiB, or 512MiB.".to_string());
    }

    let mut cpu_values = Vec::new();
    for raw in parse_csv(cpu_csv) {
        let Ok(cores) = raw.trim().parse::<i32>() else {
            return Err("CPU core values must be whole numbers between 1 and 1024.".to_string());
        };
        if !(1..=1024).contains(&cores) {
            return Err("CPU core values must be whole numbers between 1 and 1024.".to_string());
        }
        cpu_values.push(Value::from(cores));
    }
    let cpus = validated_cpu_options(&cpu_values);
    if cpus.is_empty() {
        return Err("Add at least one CPU core value, for example 4.".to_string());
    }

    let mut next = cfg.clone();
    next.disk_options = disk;
    next.ram_options = ram;
    next.cpu_options = cpus;
    next.normalize_defaults();
    Ok(next)
}

pub fn save_preset(cfg: &Config, id: &str, title: &str, subtitle: &str) -> Result<Config, String> {
    let new_title = title.trim();
    if new_title.is_empty() {
        return Err("The preset title cannot be empty.".to_string());
    }

    let mut next = cfg.clone();
    for preset in &mut next.presets {
        if preset.id == id {
            preset.title = new_title.to_string();
            preset.sub = subtitle.trim().to_string();
            return Ok(next);
        }
    }

    Err(format!("The preset '{id}' no longer exists."))
}

pub fn reset_create_options(cfg: &Config) -> Config {
    let mut next = cfg.clone();
    next.presets = default_presets();
    next.disk_options = default_disk_options();
    next.ram_options = default_ram_options();
    next.cpu_options = default_cpu_options();
    next.default_preset_id = "full".to_string();
    next.default_disk = "35GiB".to_string();
    next.default_ram = "6GiB".to_string();
    next.default_cpu = 4;
    next.normalize_defaults();
    next
}

pub fn save_auto_snapshot_settings(
    cfg: &Config,
    enabled: bool,
    keep_count: i32,
) -> Result<Config, String> {
    if !(1..=50).contains(&keep_count) {
        return Err("Auto-snapshot retention must be between 1 and 50.".to_string());
    }
    let mut next = cfg.clone();
    next.auto_snapshot_enabled = enabled;
    next.auto_snapshot_keep = keep_count;
    Ok(next)
}

pub fn save_watchdog_settings(
    cfg: &Config,
    enabled: bool,
    disk_warn: i32,
    disk_critical: i32,
    auto_stop: bool,
) -> Result<Config, String> {
    if !(50..=98).contains(&disk_warn) {
        return Err("Disk warning threshold must be between 50% and 98%.".to_string());
    }
    if disk_critical <= disk_warn || disk_critical > 99 {
        return Err(
            "Disk critical threshold must be higher than the warning threshold (up to 99%)."
                .to_string(),
        );
    }
    let mut next = cfg.clone();
    next.watchdog_enabled = enabled;
    next.watchdog_disk_warn = disk_warn;
    next.watchdog_disk_critical = disk_critical;
    next.watchdog_auto_stop = auto_stop;
    Ok(next)
}

pub fn stale_day_options() -> Vec<i32> {
    vec![7, 14, 30, 60, 90, 180]
}

pub fn save_idle_stop_settings(cfg: &Config, minutes: i32) -> Result<Config, String> {
    if !(0..=1440).contains(&minutes) {
        return Err("Idle auto-stop timeout must be between 0 (off) and 1440 minutes.".to_string());
    }
    let mut next = cfg.clone();
    next.idle_stop_minutes = minutes;
    Ok(next)
}

pub fn save_stale_settings(cfg: &Config, days: i32) -> Result<Config, String> {
    if !(1..=3650).contains(&days) {
        return Err("Stale threshold must be between 1 and 3650 days.".to_string());
    }
    let mut next = cfg.clone();
    next.stale_days = days;
    Ok(next)
}

pub fn save_session_recording_settings(cfg: &Config, enabled: bool) -> Config {
    let mut next = cfg.clone();
    next.record_sessions = enabled;
    next
}

pub fn save_appearance_settings(
    cfg: &Config,
    mode_override: &str,
    high_contrast: bool,
    reduce_motion: bool,
) -> Result<Config, String> {
    let normalized = mode_override.trim().to_ascii_lowercase();
    if !matches!(normalized.as_str(), "auto" | "light" | "dark") {
        return Err("Theme mode override must be auto, light, or dark.".to_string());
    }
    let mut next = cfg.clone();
    next.theme_mode_override = normalized;
    next.high_contrast = high_contrast;
    next.reduce_motion = reduce_motion;
    Ok(next)
}

pub fn parse_diff_excludes(raw_text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in raw_text.split([',', ';', '\r', '\n']) {
        let clean = part.trim();
        if !clean.is_empty() && !out.iter().any(|existing| existing == clean) {
            out.push(clean.to_string());
        }
    }
    out
}

pub fn save_diff_excludes(cfg: &Config, raw_excludes: &str) -> Config {
    let mut next = cfg.clone();
    next.diff_excludes = parse_diff_excludes(raw_excludes);
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("aibox-settings-{label}-{nanos}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn first_run_completes_and_persists() {
        let dir = temp_dir("firstrun");
        let custom_sys = dir.join("custom-system");
        let cur_ws = dir.join("ws");
        let cur_os = dir.join("os");
        let cur_state = dir.join("state");
        let cur_cfg = dir.join("config.json");
        let req = FirstRunRequest {
            system_path: custom_sys.to_str().unwrap(),
            current_system_path: dir.to_str().unwrap(),
            current_workspace_base: cur_ws.to_str().unwrap(),
            current_os_storage_path: cur_os.to_str().unwrap(),
            current_state_dir: cur_state.to_str().unwrap(),
            current_config_path: cur_cfg.to_str().unwrap(),
            is_project_dir_mode: false,
            remember_system_location: false,
        };
        let paths = complete_first_run(&req).expect("first run");
        assert_eq!(paths.system_path, custom_sys.to_str().unwrap());
        assert_eq!(
            paths.workspace_base,
            custom_sys.join("workspaces").to_str().unwrap()
        );
        assert_eq!(
            paths.os_storage_path,
            custom_sys.join("os-images").to_str().unwrap()
        );
        assert_eq!(paths.state_dir, custom_sys.join("state").to_str().unwrap());
        assert_eq!(
            paths.config_path,
            custom_sys.join("config.json").to_str().unwrap()
        );
        assert!(custom_sys.join("workspaces").is_dir());
        assert!(custom_sys.join("os-images").is_dir());
        assert!(custom_sys.join("state").is_dir());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn first_run_empty_keeps_current_workspace() {
        let dir = temp_dir("firstrun-empty");
        let custom_ws = dir.join("custom-ws");
        let os_dir = dir.join("os-images");
        let state_dir = dir.join("state");
        let cfg_path = dir.join("config.json");
        let req = FirstRunRequest {
            system_path: "",
            current_system_path: dir.to_str().unwrap(),
            current_workspace_base: custom_ws.to_str().unwrap(),
            current_os_storage_path: os_dir.to_str().unwrap(),
            current_state_dir: state_dir.to_str().unwrap(),
            current_config_path: cfg_path.to_str().unwrap(),
            is_project_dir_mode: false,
            remember_system_location: false,
        };
        let paths = complete_first_run(&req).expect("empty first run");
        assert_eq!(paths.workspace_base, custom_ws.to_str().unwrap());
        assert_eq!(paths.os_storage_path, os_dir.to_str().unwrap());
        assert_eq!(paths.config_path, cfg_path.to_str().unwrap());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn storage_settings_validate_and_persist() {
        let dir = temp_dir("storage");
        let cfg = Config::new_defaults(
            dir.join("ws-default").to_str().unwrap(),
            dir.join("os-default").to_str().unwrap(),
        );
        let new_ws = dir.join("ws-new");
        let new_os = dir.join("os-new");
        let updated = save_settings(
            &cfg,
            new_ws.to_str().unwrap(),
            new_os.to_str().unwrap(),
            &cfg.workspace_base,
            &cfg.os_storage_path,
        )
        .expect("save settings");
        assert!(updated.first_run_completed);
        assert_eq!(updated.workspace_base, new_ws.to_str().unwrap());
        assert_eq!(updated.os_storage_path, new_os.to_str().unwrap());
        assert!(new_ws.is_dir());
        assert!(new_os.is_dir());

        let blocking = dir.join("blocking-file");
        fs::write(&blocking, "x").expect("write blocking file");
        let err = save_settings(
            &cfg,
            blocking.to_str().unwrap(),
            new_os.to_str().unwrap(),
            &cfg.workspace_base,
            &cfg.os_storage_path,
        )
        .unwrap_err();
        assert!(err.contains("Could not create the workspace directory"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn os_catalog_can_be_added_edited_and_removed() {
        let cfg = Config::new_defaults("/tmp/ws", "/tmp/os");
        let initial_len = cfg.os_catalog.len();

        assert!(save_os_entry(&cfg, "", "Bad ID!", "", "", "", "").is_err());
        assert!(save_os_entry(&cfg, "", "valid-id", "bad alias!", "", "", "").is_err());
        assert!(save_os_entry(&cfg, "", "ubuntu-24.04", "", "", "", "").is_err());
        assert!(save_os_entry(&cfg, "missing", "new-id", "", "", "", "").is_err());

        let added = save_os_entry(&cfg, "", "debian-13", "", "", "", "Testing").expect("add");
        assert_eq!(added.os_catalog.len(), initial_len + 1);
        let last = added.os_catalog.last().unwrap();
        assert_eq!(last.id, "debian-13");
        assert_eq!(last.alias, "debian-13");
        assert_eq!(last.name, "debian-13");
        assert_eq!(last.badge, "Custom");
        assert_eq!(last.description, "Testing");

        let edited = save_os_entry(
            &added,
            "debian-13",
            "debian-trixie",
            "debian/trixie",
            "Debian Trixie",
            "Dev",
            "Updated",
        )
        .expect("edit");
        assert!(edited.os_entry_by_id("debian-13").is_none());
        let entry = edited.os_entry_by_id("debian-trixie").unwrap();
        assert_eq!(entry.alias, "debian/trixie");
        assert_eq!(entry.badge, "Dev");

        let removed = remove_os_entry(&edited, "debian-trixie").expect("remove");
        assert_eq!(removed.os_catalog.len(), initial_len);
        assert!(remove_os_entry(&removed, "missing").is_err());

        let mut one = removed;
        while one.os_catalog.len() > 1 {
            let id = one.os_catalog[0].id.clone();
            one = remove_os_entry(&one, &id).unwrap();
        }
        let last_id = one.os_catalog[0].id.clone();
        assert!(remove_os_entry(&one, &last_id).is_err());

        let reset = reset_os_catalog(&one);
        assert_eq!(reset.os_catalog.len(), default_os_entries().len());
        assert_eq!(reset.default_os_id, "ubuntu-24.04");
    }

    #[test]
    fn create_options_are_editable_and_validated() {
        let cfg = Config::new_defaults("/tmp/ws", "/tmp/os");
        assert!(save_create_options(&cfg, "", "4GiB", "2").is_err());
        assert!(save_create_options(&cfg, "35GiB, bad", "4GiB", "2").is_err());
        assert!(save_create_options(&cfg, "35GiB", "", "2").is_err());
        assert!(save_create_options(&cfg, "35GiB", "bad", "2").is_err());
        assert!(save_create_options(&cfg, "35GiB", "4GiB", "0").is_err());
        assert!(save_create_options(&cfg, "35GiB", "4GiB", "1025").is_err());
        assert!(save_create_options(&cfg, "35GiB", "4GiB", "").is_err());

        let updated = save_create_options(&cfg, "20GiB, 50GiB", "4GiB, 16GiB", "2, 8").expect("ok");
        assert_eq!(updated.disk_options, vec!["20GiB", "50GiB"]);
        assert_eq!(updated.ram_options, vec!["4GiB", "16GiB"]);
        assert_eq!(updated.cpu_options, vec![2, 8]);
        assert_eq!(updated.default_disk, "20GiB");
        assert_eq!(updated.default_ram, "4GiB");
        assert_eq!(updated.default_cpu, 2);

        assert!(save_preset(&updated, "full", "   ", "sub").is_err());
        assert!(save_preset(&updated, "missing", "Title", "sub").is_err());
        let with_preset =
            save_preset(&updated, "full", "  Custom Title  ", "  Custom Sub  ").expect("preset");
        let p = with_preset.preset_by_id("full").unwrap();
        assert_eq!(p.title, "Custom Title");
        assert_eq!(p.sub, "Custom Sub");

        let reset = reset_create_options(&with_preset);
        assert_eq!(reset.disk_options, default_disk_options());
        assert_eq!(reset.default_disk, "35GiB");
        assert_eq!(reset.default_cpu, 4);
    }

    #[test]
    fn auto_snapshot_watchdog_idle_stale_and_recording_settings_validate() {
        let cfg = Config::new_defaults("/tmp/ws", "/tmp/os");

        assert!(save_auto_snapshot_settings(&cfg, true, 0).is_err());
        assert!(save_auto_snapshot_settings(&cfg, true, 51).is_err());
        let snap = save_auto_snapshot_settings(&cfg, false, 10).expect("snap");
        assert!(!snap.auto_snapshot_enabled);
        assert_eq!(snap.auto_snapshot_keep, 10);

        assert!(save_watchdog_settings(&cfg, true, 49, 95, false).is_err());
        assert!(save_watchdog_settings(&cfg, true, 99, 100, false).is_err());
        assert!(save_watchdog_settings(&cfg, true, 80, 80, false).is_err());
        assert!(save_watchdog_settings(&cfg, true, 80, 100, false).is_err());
        let wd = save_watchdog_settings(&cfg, false, 75, 95, true).expect("wd");
        assert!(!wd.watchdog_enabled);
        assert_eq!(wd.watchdog_disk_warn, 75);
        assert_eq!(wd.watchdog_disk_critical, 95);
        assert!(wd.watchdog_auto_stop);

        assert!(save_idle_stop_settings(&cfg, -1).is_err());
        assert!(save_idle_stop_settings(&cfg, 1441).is_err());
        let idle = save_idle_stop_settings(&cfg, 45).expect("idle");
        assert_eq!(idle.idle_stop_minutes, 45);

        assert_eq!(stale_day_options(), vec![7, 14, 30, 60, 90, 180]);
        assert!(save_stale_settings(&cfg, 0).is_err());
        assert!(save_stale_settings(&cfg, 3651).is_err());
        let stale = save_stale_settings(&cfg, 90).expect("stale");
        assert_eq!(stale.stale_days, 90);

        let rec = save_session_recording_settings(&cfg, true);
        assert!(rec.record_sessions);

        let exc = save_diff_excludes(&cfg, "*.log, .git;\nnode_modules\n*.log");
        assert_eq!(exc.diff_excludes, vec!["*.log", ".git", "node_modules"]);
    }
}
