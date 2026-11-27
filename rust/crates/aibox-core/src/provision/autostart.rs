use crate::util::shell::shell_quote;

pub const MAX_AUTOSTART_PRIORITY: i32 = 1000;
pub const MAX_AUTOSTART_DELAY_SECS: i32 = 3600;

/// Validates boot autostart priority and delay bounds.
pub fn validate_autostart_options(priority: i32, delay_secs: i32) -> Result<(), String> {
    if !(0..=MAX_AUTOSTART_PRIORITY).contains(&priority) {
        return Err(format!(
            "Autostart priority must be between 0 and {MAX_AUTOSTART_PRIORITY}."
        ));
    }
    if !(0..=MAX_AUTOSTART_DELAY_SECS).contains(&delay_secs) {
        return Err(format!(
            "Autostart delay must be between 0 and {MAX_AUTOSTART_DELAY_SECS} seconds."
        ));
    }
    Ok(())
}

/// Builds the shell script to update `boot.autostart`, `boot.autostart.priority`,
/// and `boot.autostart.delay` for an Incus instance.
pub fn set_environment_autostart_script(
    name: &str,
    enabled: bool,
    priority: i32,
    delay_secs: i32,
) -> String {
    let q_name = shell_quote(name);
    let mut lines = Vec::new();
    lines.push("set -euo pipefail".to_string());
    lines.push("echo '@@STAGE:50:Updating boot autostart settings...'".to_string());
    if enabled {
        let mut set_cmd = format!("incus config set {q_name} boot.autostart=true");
        if priority > 0 {
            set_cmd.push_str(&format!(" boot.autostart.priority={priority}"));
        }
        if delay_secs > 0 {
            set_cmd.push_str(&format!(" boot.autostart.delay={delay_secs}"));
        }
        lines.push(set_cmd);
        if priority <= 0 {
            lines.push(format!(
                "incus config unset {q_name} boot.autostart.priority >/dev/null 2>&1 || true"
            ));
        }
        if delay_secs <= 0 {
            lines.push(format!(
                "incus config unset {q_name} boot.autostart.delay >/dev/null 2>&1 || true"
            ));
        }
    } else {
        // Keep existing priority/delay intact so re-enabling restores them.
        lines.push(format!("incus config set {q_name} boot.autostart=false"));
    }
    lines.push("echo '@@STAGE:100:Updated.'".to_string());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_autostart_bounds() {
        assert!(validate_autostart_options(0, 0).is_ok());
        assert!(
            validate_autostart_options(MAX_AUTOSTART_PRIORITY, MAX_AUTOSTART_DELAY_SECS).is_ok()
        );
        assert!(validate_autostart_options(-1, 0).is_err());
        assert!(validate_autostart_options(MAX_AUTOSTART_PRIORITY + 1, 0).is_err());
        assert!(validate_autostart_options(0, -1).is_err());
        assert!(validate_autostart_options(0, MAX_AUTOSTART_DELAY_SECS + 1).is_err());
    }

    #[test]
    fn generates_autostart_enable_and_disable_scripts() {
        let on_with_values = set_environment_autostart_script("srv-1", true, 50, 15);
        assert!(on_with_values.contains(
            "incus config set 'srv-1' boot.autostart=true boot.autostart.priority=50 boot.autostart.delay=15"
        ));

        let on_zero = set_environment_autostart_script("srv-1", true, 0, 0);
        assert!(on_zero.contains("incus config set 'srv-1' boot.autostart=true"));
        assert!(on_zero.contains("incus config unset 'srv-1' boot.autostart.priority"));
        assert!(on_zero.contains("incus config unset 'srv-1' boot.autostart.delay"));

        let off = set_environment_autostart_script("srv-1", false, 50, 15);
        assert!(off.contains("incus config set 'srv-1' boot.autostart=false"));
        assert!(!off.contains("boot.autostart.priority"));
        assert!(!off.contains("boot.autostart.delay"));
    }
}
