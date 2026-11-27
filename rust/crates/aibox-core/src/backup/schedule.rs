use crate::util::names::sanitize_name;
use crate::util::shell::shell_quote;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupCadence {
    Daily,
    Weekly,
    Off,
}

impl BackupCadence {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "daily" => Some(Self::Daily),
            "weekly" => Some(Self::Weekly),
            "off" | "disable" | "none" => Some(Self::Off),
            _ => None,
        }
    }

    pub fn on_calendar(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Off => "",
        }
    }
}

/// Returns the systemd user unit base name (`aibox-backup-<sanitized-env>`).
pub fn unit_base_name(env_name: &str) -> String {
    let clean = sanitize_name(env_name);
    let suffix = if clean.is_empty() { "env" } else { &clean };
    format!("aibox-backup-{suffix}")
}

/// Renders the systemd user `.service` unit that creates a snapshot and prunes old ones.
pub fn render_backup_service_unit(
    env_name: &str,
    aibox_bin: &str,
    project_dir: &str,
    keep: usize,
) -> String {
    let q_bin = shell_quote(aibox_bin);
    let q_env = shell_quote(env_name);
    let proj_flag = if project_dir.trim().is_empty() {
        String::new()
    } else {
        format!("--project {} ", shell_quote(project_dir.trim()))
    };
    let keep_count = keep.clamp(1, 365);
    let q_exec = shell_quote(&format!(
        "{q_bin} {proj_flag}snapshot {q_env} \"sched-$(date +\\%Y\\%m\\%d-\\%H\\%M\\%S)\" && \
         old=$({q_bin} {proj_flag}snapshots {q_env} --json | grep -o '\"sched-[^\"]*\"' | tr -d '\"' | sort -r | tail -n +{}); \
         for s in $old; do incus snapshot delete {q_env} \"$s\" >/dev/null 2>&1 || true; done",
        keep_count + 1
    ));
    format!(
        "[Unit]\n\
         Description=AIBox scheduled backup for {env_name}\n\
         \n\
         [Service]\n\
         Type=oneshot\n\
         ExecStart=/bin/sh -c {q_exec}\n"
    )
}

/// Renders the systemd user `.timer` unit for `cadence`.
pub fn render_backup_timer_unit(env_name: &str, cadence: BackupCadence) -> String {
    let cal = cadence.on_calendar();
    format!(
        "[Unit]\n\
         Description=AIBox {cal} backup timer for {env_name}\n\
         \n\
         [Timer]\n\
         OnCalendar={cal}\n\
         Persistent=true\n\
         \n\
         [Install]\n\
         WantedBy=timers.target\n"
    )
}

/// Builds a shell script that installs/enables or disables/removes the systemd user
/// `.service` and `.timer` units for `env_name`.
pub fn install_schedule_script(
    env_name: &str,
    cadence: BackupCadence,
    aibox_bin: &str,
    project_dir: &str,
    keep: usize,
) -> String {
    let base = unit_base_name(env_name);
    let q_timer = shell_quote(&format!("{base}.timer"));
    let q_service = shell_quote(&format!("{base}.service"));

    if cadence == BackupCadence::Off {
        return format!(
            "set -euo pipefail\n\
             UNIT_DIR=\"${{XDG_CONFIG_HOME:-$HOME/.config}}/systemd/user\"\n\
             systemctl --user disable --now {q_timer} >/dev/null 2>&1 || true\n\
             rm -f \"$UNIT_DIR/\"{q_timer} \"$UNIT_DIR/\"{q_service}\n\
             systemctl --user daemon-reload >/dev/null 2>&1 || true"
        );
    }

    let svc = render_backup_service_unit(env_name, aibox_bin, project_dir, keep);
    let tmr = render_backup_timer_unit(env_name, cadence);
    format!(
        "set -euo pipefail\n\
         UNIT_DIR=\"${{XDG_CONFIG_HOME:-$HOME/.config}}/systemd/user\"\n\
         mkdir -p \"$UNIT_DIR\"\n\
         cat > \"$UNIT_DIR/\"{q_service} <<'AIBOX_EOF'\n\
         {svc}AIBOX_EOF\n\
         cat > \"$UNIT_DIR/\"{q_timer} <<'AIBOX_EOF'\n\
         {tmr}AIBOX_EOF\n\
         systemctl --user daemon-reload\n\
         systemctl --user enable --now {q_timer}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cadence_and_builds_units() {
        assert_eq!(BackupCadence::parse("daily"), Some(BackupCadence::Daily));
        assert_eq!(BackupCadence::parse("WEEKLY"), Some(BackupCadence::Weekly));
        assert_eq!(BackupCadence::parse("off"), Some(BackupCadence::Off));
        assert_eq!(BackupCadence::parse("hourly"), None);
        assert_eq!(unit_base_name("Web App!"), "aibox-backup-web-app");

        let svc = render_backup_service_unit("web", "aibox", "/tmp/proj", 7);
        assert!(svc.contains("Type=oneshot"));
        assert!(svc.contains("snapshot"));
        assert!(svc.contains("tail -n +8"));

        let tmr = render_backup_timer_unit("web", BackupCadence::Daily);
        assert!(tmr.contains("OnCalendar=daily"));
        assert!(tmr.contains("Persistent=true"));

        let install = install_schedule_script("web", BackupCadence::Weekly, "aibox", "", 14);
        assert!(install.contains("aibox-backup-web.timer"));
        assert!(install.contains("OnCalendar=weekly"));
        assert!(install.contains("systemctl --user enable --now"));

        let disable = install_schedule_script("web", BackupCadence::Off, "aibox", "", 7);
        assert!(disable.contains("systemctl --user disable --now"));
        assert!(disable.contains("rm -f"));
    }
}
