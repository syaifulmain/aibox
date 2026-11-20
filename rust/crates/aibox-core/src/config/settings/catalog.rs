//! OS catalog, create-dialog resource options, and preset settings.

use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use crate::config::defaults::{
    default_cpu_options, default_disk_options, default_os_entries, default_presets,
    default_ram_options, default_swap_options, OsEntry,
};
use crate::config::error::ConfigError;
use crate::config::model::Config;
use crate::util::csv::parse_csv;
use crate::util::size::{validated_cpu_options, validated_size_options, validated_swap_options};

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
) -> Result<Config, ConfigError> {
    let new_id = id.trim().to_lowercase();
    if !os_id_regex().is_match(&new_id) {
        return Err(ConfigError::invalid(
            "osCatalog.id",
            "OS ID must use lowercase letters, numbers, dots, dashes, or underscores.",
        ));
    }

    let trimmed_alias = alias.trim();
    let new_alias = if trimmed_alias.is_empty() {
        new_id.clone()
    } else {
        trimmed_alias.to_string()
    };
    if !os_alias_regex().is_match(&new_alias) {
        return Err(ConfigError::invalid(
            "osCatalog.alias",
            "Image alias must use letters, numbers, dots, slashes, dashes, or underscores.",
        ));
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
            return Err(ConfigError::invalid(
                "osCatalog.id",
                format!("An OS entry with the ID '{new_id}' already exists."),
            ));
        }
    }
    if !original_id.is_empty() && found_index.is_none() {
        return Err(ConfigError::invalid(
            "osCatalog.id",
            format!("The OS entry '{original_id}' no longer exists."),
        ));
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

pub fn remove_os_entry(cfg: &Config, id: &str) -> Result<Config, ConfigError> {
    if cfg.os_catalog.len() <= 1 {
        return Err(ConfigError::invalid(
            "osCatalog",
            "Keep at least one OS entry in the catalog.",
        ));
    }

    let target = id.trim();
    let Some(idx) = cfg.os_catalog.iter().position(|e| e.id == target) else {
        return Err(ConfigError::invalid(
            "osCatalog.id",
            format!("The OS entry '{target}' was not found."),
        ));
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
    swap_csv: &str,
    cpu_csv: &str,
) -> Result<Config, ConfigError> {
    let raw_disk = parse_csv(disk_csv);
    let disk = validated_size_options(&raw_disk);
    if disk.is_empty() {
        return Err(ConfigError::invalid(
            "diskOptions",
            "Add at least one disk quota value, for example 35GiB.",
        ));
    }
    if disk.len() != raw_disk.len() {
        return Err(ConfigError::invalid(
            "diskOptions",
            "Disk quota values must look like 35GiB, 60GiB, or 512MiB.",
        ));
    }

    let raw_ram = parse_csv(ram_csv);
    let ram = validated_size_options(&raw_ram);
    if ram.is_empty() {
        return Err(ConfigError::invalid(
            "ramOptions",
            "Add at least one RAM limit value, for example 6GiB.",
        ));
    }
    if ram.len() != raw_ram.len() {
        return Err(ConfigError::invalid(
            "ramOptions",
            "RAM limit values must look like 4GiB, 6GiB, or 512MiB.",
        ));
    }

    let raw_swap = parse_csv(swap_csv);
    let swap = validated_swap_options(&raw_swap);
    if swap.is_empty() {
        return Err(ConfigError::invalid(
            "swapOptions",
            "Add at least one swap choice, for example Off, Host, or 4GiB.",
        ));
    }
    if swap.len() != raw_swap.len() {
        return Err(ConfigError::invalid(
            "swapOptions",
            "Swap choices must be Off, Host, or a size such as 4GiB, without duplicates.",
        ));
    }
    if swap.len() > 4 {
        return Err(ConfigError::invalid(
            "swapOptions",
            "Keep at most 4 swap choices.",
        ));
    }
    if !swap.iter().any(|value| value == "Off") || !swap.iter().any(|value| value == "Host") {
        return Err(ConfigError::invalid(
            "swapOptions",
            "Swap choices must include Off and Host.",
        ));
    }

    let mut cpu_values = Vec::new();
    for raw in parse_csv(cpu_csv) {
        let Ok(cores) = raw.trim().parse::<i32>() else {
            return Err(ConfigError::invalid(
                "cpuOptions",
                "CPU core values must be whole numbers between 1 and 1024.",
            ));
        };
        if !(1..=1024).contains(&cores) {
            return Err(ConfigError::invalid(
                "cpuOptions",
                "CPU core values must be whole numbers between 1 and 1024.",
            ));
        }
        cpu_values.push(Value::from(cores));
    }
    let cpus = validated_cpu_options(&cpu_values);
    if cpus.is_empty() {
        return Err(ConfigError::invalid(
            "cpuOptions",
            "Add at least one CPU core value, for example 4.",
        ));
    }

    let mut next = cfg.clone();
    next.disk_options = disk;
    next.ram_options = ram;
    next.swap_options = swap;
    next.cpu_options = cpus;
    next.normalize_defaults();
    Ok(next)
}

pub fn save_preset(
    cfg: &Config,
    id: &str,
    title: &str,
    subtitle: &str,
) -> Result<Config, ConfigError> {
    let new_title = title.trim();
    if new_title.is_empty() {
        return Err(ConfigError::invalid(
            "presets.title",
            "The preset title cannot be empty.",
        ));
    }

    let mut next = cfg.clone();
    for preset in &mut next.presets {
        if preset.id == id {
            preset.title = new_title.to_string();
            preset.sub = subtitle.trim().to_string();
            return Ok(next);
        }
    }

    Err(ConfigError::invalid(
        "presets.id",
        format!("The preset '{id}' no longer exists."),
    ))
}

pub fn reset_create_options(cfg: &Config) -> Config {
    let mut next = cfg.clone();
    next.presets = default_presets();
    next.disk_options = default_disk_options();
    next.ram_options = default_ram_options();
    next.swap_options = default_swap_options();
    next.cpu_options = default_cpu_options();
    next.default_preset_id = "full".to_string();
    next.default_disk = "35GiB".to_string();
    next.default_ram = "6GiB".to_string();
    next.default_cpu = 4;
    next.normalize_defaults();
    next
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(save_create_options(&cfg, "", "4GiB", "Off", "2").is_err());
        assert!(save_create_options(&cfg, "35GiB, bad", "4GiB", "Off", "2").is_err());
        assert!(save_create_options(&cfg, "35GiB", "", "Off", "2").is_err());
        assert!(save_create_options(&cfg, "35GiB", "bad", "Off", "2").is_err());
        assert!(save_create_options(&cfg, "35GiB", "4GiB", "", "2").is_err());
        assert!(save_create_options(&cfg, "35GiB", "4GiB", "Off, bogus", "2").is_err());
        assert!(save_create_options(&cfg, "35GiB", "4GiB", "Host, 4GiB", "2").is_err());
        assert!(save_create_options(&cfg, "35GiB", "4GiB", "Off, 4GiB", "2").is_err());
        assert!(
            save_create_options(&cfg, "35GiB", "4GiB", "Off, Host, 2GiB, 4GiB, 8GiB", "2").is_err()
        );
        assert!(save_create_options(&cfg, "35GiB", "4GiB", "Off, Host", "0").is_err());
        assert!(save_create_options(&cfg, "35GiB", "4GiB", "Off, Host", "1025").is_err());
        assert!(save_create_options(&cfg, "35GiB", "4GiB", "Off, Host", "").is_err());

        let updated = save_create_options(
            &cfg,
            "20GiB, 50GiB",
            "4GiB, 16GiB",
            "Off, Host, 4GiB",
            "2, 8",
        )
        .expect("ok");
        assert_eq!(updated.disk_options, vec!["20GiB", "50GiB"]);
        assert_eq!(updated.ram_options, vec!["4GiB", "16GiB"]);
        assert_eq!(updated.swap_options, vec!["Off", "Host", "4GiB"]);
        let aliases =
            save_create_options(&cfg, "20GiB", "4GiB", "false, true", "2").expect("aliases");
        assert_eq!(aliases.swap_options, vec!["Off", "Host"]);
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
        assert_eq!(reset.swap_options, default_swap_options());
        assert_eq!(reset.default_disk, "35GiB");
        assert_eq!(reset.default_cpu, 4);
    }
}
