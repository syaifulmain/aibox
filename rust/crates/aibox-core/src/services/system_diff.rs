use std::collections::BTreeSet;

use crate::util::shell::shell_quote;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SystemStateSnapshot {
    pub packages: BTreeSet<String>,
    pub services: BTreeSet<String>,
    pub listening_ports: BTreeSet<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SystemChangeSummary {
    pub added_packages: Vec<String>,
    pub removed_packages: Vec<String>,
    pub added_services: Vec<String>,
    pub removed_services: Vec<String>,
    pub added_ports: Vec<String>,
    pub removed_ports: Vec<String>,
    pub rollback_snapshot: String,
}

impl SystemChangeSummary {
    pub fn is_empty(&self) -> bool {
        self.added_packages.is_empty()
            && self.removed_packages.is_empty()
            && self.added_services.is_empty()
            && self.removed_services.is_empty()
            && self.added_ports.is_empty()
            && self.removed_ports.is_empty()
    }
}

/// Captures installed packages, enabled services, and listening ports inside `env_name`.
pub fn capture_system_state_script(env_name: &str) -> String {
    let q_name = shell_quote(env_name);
    let inner = "\
        echo '__PKGS__'; \
        if command -v pacman >/dev/null 2>&1; then pacman -Q 2>/dev/null || true; \
        elif command -v dpkg-query >/dev/null 2>&1; then dpkg-query -W -f='${Package} ${Version}\\n' 2>/dev/null || true; \
        elif command -v apk >/dev/null 2>&1; then apk info -v 2>/dev/null || true; fi; \
        echo '__SVCS__'; \
        if command -v systemctl >/dev/null 2>&1; then systemctl list-unit-files --state=enabled --no-legend 2>/dev/null | awk '{print $1}' || true; \
        elif command -v rc-update >/dev/null 2>&1; then rc-update show 2>/dev/null | awk '{print $1}' || true; fi; \
        echo '__PORTS__'; \
        if command -v ss >/dev/null 2>&1; then ss -tulpnH 2>/dev/null | awk '{print $1 \" \" $5}' || true; fi";
    format!("incus exec {q_name} -- sh -c {}", shell_quote(inner))
}

/// Parses output from [`capture_system_state_script`] into a [`SystemStateSnapshot`].
pub fn parse_system_state(output: &str) -> SystemStateSnapshot {
    #[derive(Clone, Copy)]
    enum Section {
        None,
        Pkgs,
        Svcs,
        Ports,
    }

    let mut snap = SystemStateSnapshot::default();
    let mut section = Section::None;

    for raw in output.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        match line {
            "__PKGS__" => section = Section::Pkgs,
            "__SVCS__" => section = Section::Svcs,
            "__PORTS__" => section = Section::Ports,
            _ => match section {
                Section::None => {}
                Section::Pkgs => {
                    snap.packages.insert(line.to_string());
                }
                Section::Svcs => {
                    snap.services.insert(line.to_string());
                }
                Section::Ports => {
                    snap.listening_ports.insert(line.to_string());
                }
            },
        }
    }
    snap
}

/// Computes added and removed packages, services, and listening ports between two snapshots.
pub fn diff_system_state(
    before: &SystemStateSnapshot,
    after: &SystemStateSnapshot,
    rollback_snapshot: &str,
) -> SystemChangeSummary {
    SystemChangeSummary {
        added_packages: after
            .packages
            .difference(&before.packages)
            .cloned()
            .collect(),
        removed_packages: before
            .packages
            .difference(&after.packages)
            .cloned()
            .collect(),
        added_services: after
            .services
            .difference(&before.services)
            .cloned()
            .collect(),
        removed_services: before
            .services
            .difference(&after.services)
            .cloned()
            .collect(),
        added_ports: after
            .listening_ports
            .difference(&before.listening_ports)
            .cloned()
            .collect(),
        removed_ports: before
            .listening_ports
            .difference(&after.listening_ports)
            .cloned()
            .collect(),
        rollback_snapshot: rollback_snapshot.to_string(),
    }
}

/// Formats a human-readable summary of system changes and a rollback command when available.
pub fn format_system_change_summary(env_name: &str, summary: &SystemChangeSummary) -> String {
    let mut lines = Vec::new();
    lines.push(format!("=== System Changes ({env_name}) ==="));
    if summary.is_empty() {
        lines.push("No package, service, or listening port changes detected.".to_string());
    } else {
        let mut append_group = |label: &str, items: &[String]| {
            if !items.is_empty() {
                lines.push(format!("{label}: {}", items.join(", ")));
            }
        };
        append_group("+ Packages", &summary.added_packages);
        append_group("- Packages", &summary.removed_packages);
        append_group("+ Services", &summary.added_services);
        append_group("- Services", &summary.removed_services);
        append_group("+ Ports", &summary.added_ports);
        append_group("- Ports", &summary.removed_ports);
    }
    if !summary.rollback_snapshot.is_empty() {
        lines.push(format!(
            "Rollback: aibox restore {} {}",
            env_name, summary.rollback_snapshot
        ));
    }
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_diffs_and_formats_system_changes() {
        let before_raw = "\
__PKGS__
bash 5.2
curl 8.0
__SVCS__
sshd.service
__PORTS__
tcp 127.0.0.1:22
";
        let after_raw = "\
__PKGS__
bash 5.2
nginx 1.26
__SVCS__
sshd.service
nginx.service
__PORTS__
tcp 127.0.0.1:22
tcp 0.0.0.0:80
";
        let before = parse_system_state(before_raw);
        let after = parse_system_state(after_raw);
        let summary = diff_system_state(&before, &after, "auto-20261127-230000");

        assert_eq!(summary.added_packages, vec!["nginx 1.26"]);
        assert_eq!(summary.removed_packages, vec!["curl 8.0"]);
        assert_eq!(summary.added_services, vec!["nginx.service"]);
        assert!(summary.removed_services.is_empty());
        assert_eq!(summary.added_ports, vec!["tcp 0.0.0.0:80"]);

        let text = format_system_change_summary("dev", &summary);
        assert!(text.contains("+ Packages: nginx 1.26"));
        assert!(text.contains("- Packages: curl 8.0"));
        assert!(text.contains("+ Services: nginx.service"));
        assert!(text.contains("+ Ports: tcp 0.0.0.0:80"));
        assert!(text.contains("Rollback: aibox restore dev auto-20261127-230000"));

        let empty = diff_system_state(&before, &before, "");
        assert!(empty.is_empty());
        assert!(format_system_change_summary("dev", &empty)
            .contains("No package, service, or listening port changes detected."));
    }
}
