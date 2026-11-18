//! Safety, watchdog, lifecycle, recording, diff-exclude, and appearance settings.

use crate::config::error::ConfigError;
use crate::config::model::Config;

pub fn save_auto_snapshot_settings(
    cfg: &Config,
    enabled: bool,
    keep_count: i32,
) -> Result<Config, ConfigError> {
    if !(1..=50).contains(&keep_count) {
        return Err(ConfigError::invalid(
            "autoSnapshotKeep",
            "Auto-snapshot retention must be between 1 and 50.",
        ));
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
) -> Result<Config, ConfigError> {
    if !(50..=98).contains(&disk_warn) {
        return Err(ConfigError::invalid(
            "watchdogDiskWarn",
            "Disk warning threshold must be between 50% and 98%.",
        ));
    }
    if disk_critical <= disk_warn || disk_critical > 99 {
        return Err(ConfigError::invalid(
            "watchdogDiskCritical",
            "Disk critical threshold must be higher than the warning threshold (up to 99%).",
        ));
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

pub fn save_idle_stop_settings(cfg: &Config, minutes: i32) -> Result<Config, ConfigError> {
    if !(0..=1440).contains(&minutes) {
        return Err(ConfigError::invalid(
            "idleStopMinutes",
            "Idle auto-stop timeout must be between 0 (off) and 1440 minutes.",
        ));
    }
    let mut next = cfg.clone();
    next.idle_stop_minutes = minutes;
    Ok(next)
}

pub fn save_stale_settings(cfg: &Config, days: i32) -> Result<Config, ConfigError> {
    if !(1..=3650).contains(&days) {
        return Err(ConfigError::invalid(
            "staleDays",
            "Stale threshold must be between 1 and 3650 days.",
        ));
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
) -> Result<Config, ConfigError> {
    let normalized = mode_override.trim().to_ascii_lowercase();
    if !matches!(normalized.as_str(), "auto" | "light" | "dark") {
        return Err(ConfigError::invalid(
            "themeModeOverride",
            "Theme mode override must be auto, light, or dark.",
        ));
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
