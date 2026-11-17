//! Resource watchdog hysteresis and evaluation logic.

use crate::util::shell::shell_quote;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WatchdogDecision {
    pub new_level: i32,
    pub should_notify: bool,
    pub should_auto_stop: bool,
}

/// Evaluates disk usage against warn/critical thresholds with a 5-point hysteresis band.
pub fn evaluate_watchdog(
    percent: i32,
    previous_level: i32,
    warn_threshold: i32,
    critical_threshold: i32,
    auto_stop_enabled: bool,
) -> WatchdogDecision {
    let warn = warn_threshold.clamp(50, 98);
    let crit = critical_threshold.clamp(warn + 1, 100);
    let prev = previous_level.clamp(0, 2);

    let new_level = if percent >= crit {
        2
    } else if percent >= warn {
        // Keep level 2 until dropping 5 points below critical (hysteresis).
        if prev == 2 && percent >= crit - 5 {
            2
        } else {
            1
        }
    } else if prev >= 1 && percent >= warn - 5 {
        // Keep level 1/2 if within 5-point hysteresis band below warn threshold.
        1
    } else {
        0
    };

    WatchdogDecision {
        new_level,
        should_notify: new_level > prev,
        should_auto_stop: new_level == 2 && auto_stop_enabled,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchdogEnvInput {
    pub name: String,
    pub running: bool,
    pub disk_percent: i32,
    pub disk_usage: String,
    pub disk_limit: String,
    pub disk_alert_level: i32,
    pub auto_stop_on_critical: bool,
    pub cpu_percent: i32,
    pub memory_percent: i32,
    pub prev_disk_level: i32,
    pub prev_cpu_level: i32,
    pub prev_cpu_streak: i32,
    pub prev_ram_level: i32,
    pub prev_ram_streak: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchdogNotification {
    pub critical: bool,
    pub kind: WatchdogNotifyKind,
    pub env_name: String,
    pub percent: i32,
    pub disk_usage: String,
    pub disk_limit: String,
    pub auto_stopping: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogNotifyKind {
    DiskWarning,
    DiskCritical,
    CpuHigh,
    RamHigh,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchdogEnvResult {
    pub name: String,
    pub new_disk_level: i32,
    pub disk_alert_level_changed: bool,
    pub new_cpu_level: i32,
    pub new_cpu_streak: i32,
    pub new_ram_level: i32,
    pub new_ram_streak: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WatchdogBatchResult {
    pub envs: Vec<WatchdogEnvResult>,
    pub notifications: Vec<WatchdogNotification>,
    pub auto_stop_target: String,
    pub auto_stop_script: String,
}

/// Evaluates all environments for disk, CPU, and RAM watchdog alerts.
pub fn evaluate_all_watchdogs(
    envs: &[WatchdogEnvInput],
    disk_warn: i32,
    disk_critical: i32,
    cpu_warn: i32,
    ram_warn: i32,
    busy: bool,
) -> WatchdogBatchResult {
    let mut batch = WatchdogBatchResult::default();

    for env in envs {
        let d = evaluate_watchdog(
            env.disk_percent,
            env.prev_disk_level,
            disk_warn,
            disk_critical,
            env.running && env.auto_stop_on_critical,
        );

        if d.should_notify {
            let is_crit = d.new_level == 2;
            batch.notifications.push(WatchdogNotification {
                critical: is_crit,
                kind: if is_crit {
                    WatchdogNotifyKind::DiskCritical
                } else {
                    WatchdogNotifyKind::DiskWarning
                },
                env_name: env.name.clone(),
                percent: env.disk_percent,
                disk_usage: env.disk_usage.clone(),
                disk_limit: env.disk_limit.clone(),
                auto_stopping: d.should_auto_stop,
            });
        }

        if d.should_auto_stop && batch.auto_stop_target.is_empty() && !busy {
            batch.auto_stop_target = env.name.clone();
            batch.auto_stop_script = watchdog_auto_stop_script(&env.name);
        }

        let (new_cpu_level, new_cpu_streak, notify_cpu) = eval_streak(
            env.running,
            env.cpu_percent,
            cpu_warn,
            env.prev_cpu_level,
            env.prev_cpu_streak,
        );
        if notify_cpu {
            batch.notifications.push(WatchdogNotification {
                critical: false,
                kind: WatchdogNotifyKind::CpuHigh,
                env_name: env.name.clone(),
                percent: env.cpu_percent,
                disk_usage: String::new(),
                disk_limit: String::new(),
                auto_stopping: false,
            });
        }

        let (new_ram_level, new_ram_streak, notify_ram) = eval_streak(
            env.running,
            env.memory_percent,
            ram_warn,
            env.prev_ram_level,
            env.prev_ram_streak,
        );
        if notify_ram {
            batch.notifications.push(WatchdogNotification {
                critical: false,
                kind: WatchdogNotifyKind::RamHigh,
                env_name: env.name.clone(),
                percent: env.memory_percent,
                disk_usage: String::new(),
                disk_limit: String::new(),
                auto_stopping: false,
            });
        }

        batch.envs.push(WatchdogEnvResult {
            name: env.name.clone(),
            new_disk_level: d.new_level,
            disk_alert_level_changed: env.disk_alert_level != d.new_level,
            new_cpu_level,
            new_cpu_streak,
            new_ram_level,
            new_ram_streak,
        });
    }

    batch
}

fn eval_streak(
    running: bool,
    percent: i32,
    warn: i32,
    prev_level: i32,
    prev_streak: i32,
) -> (i32, i32, bool) {
    if !running {
        return (0, 0, false);
    }
    let streak = if percent >= warn { prev_streak + 1 } else { 0 };
    if streak >= 3 && prev_level == 0 {
        (1, streak, true)
    } else if percent < warn - 10 {
        (0, streak, false)
    } else {
        (prev_level, streak, false)
    }
}

pub fn watchdog_auto_stop_script(name: &str) -> String {
    let q = shell_quote(name);
    format!(
        "echo '@@STAGE:50:Stopping container on critical disk quota...'; \
         incus stop {q} --timeout=15 || incus stop {q} --force; \
         echo '@@STAGE:100:Stopped by watchdog.'"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watchdog_hysteresis_and_auto_stop_match_qt_test() {
        let d1 = evaluate_watchdog(89, 0, 90, 97, true);
        assert_eq!(d1.new_level, 0);
        assert!(!d1.should_notify);
        assert!(!d1.should_auto_stop);

        let d2 = evaluate_watchdog(91, 0, 90, 97, true);
        assert_eq!(d2.new_level, 1);
        assert!(d2.should_notify);
        assert!(!d2.should_auto_stop);

        // Stay silent inside 5% hysteresis band when already at warn
        let d3 = evaluate_watchdog(86, 1, 90, 97, true);
        assert_eq!(d3.new_level, 1);
        assert!(!d3.should_notify);

        // Drop below warn - 5 re-arms
        let d4 = evaluate_watchdog(84, 1, 90, 97, true);
        assert_eq!(d4.new_level, 0);
        assert!(!d4.should_notify);

        // Hit critical with auto-stop disabled vs enabled
        let d5 = evaluate_watchdog(98, 1, 90, 97, false);
        assert_eq!(d5.new_level, 2);
        assert!(d5.should_notify);
        assert!(!d5.should_auto_stop);

        let d6 = evaluate_watchdog(98, 1, 90, 97, true);
        assert_eq!(d6.new_level, 2);
        assert!(d6.should_notify);
        assert!(d6.should_auto_stop);

        // Stay at critical while within 5% hysteresis band below critical
        let d7 = evaluate_watchdog(94, 2, 90, 97, true);
        assert_eq!(d7.new_level, 2);
        assert!(!d7.should_notify);
    }
}
