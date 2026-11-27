pub mod system_diff;

use serde::Serialize;

use crate::util::shell::shell_quote;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceEntry {
    pub unit: String,
    pub load: String,
    pub active: String,
    pub sub: String,
    pub description: String,
    pub manager: String,
}

/// Validates a systemd or OpenRC unit/service name.
pub fn is_valid_unit_name(unit: &str) -> bool {
    let trimmed = unit.trim();
    !trimmed.is_empty()
        && trimmed.len() <= 128
        && trimmed
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'@' | b':'))
}

/// Builds a shell script that lists services in a running container using
/// `systemctl` (with OpenRC `rc-status -a` fallback).
pub fn list_services_script(env_name: &str) -> String {
    let q_name = shell_quote(env_name);
    format!(
        "incus exec {q_name} -- sh -c '\
         if command -v systemctl >/dev/null 2>&1; then \
           echo \"__MGR:systemd\"; \
           systemctl list-units --type=service --all --no-pager --no-legend --plain; \
         elif command -v rc-status >/dev/null 2>&1; then \
           echo \"__MGR:openrc\"; \
           rc-status -a; \
         else \
           echo \"Error: Neither systemctl nor rc-status found in container.\" >&2; \
           exit 1; \
         fi'"
    )
}

/// Parses `systemctl list-units --type=service --all --no-pager --no-legend --plain` output.
pub fn parse_systemd_units(output: &str) -> Vec<ServiceEntry> {
    let mut entries = Vec::new();
    for raw_line in output.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("__MGR:") {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(unit) = parts.next() else {
            continue;
        };
        let Some(load) = parts.next() else {
            continue;
        };
        let Some(active) = parts.next() else {
            continue;
        };
        let Some(sub) = parts.next() else {
            continue;
        };
        let description = parts.collect::<Vec<_>>().join(" ");
        entries.push(ServiceEntry {
            unit: unit.to_string(),
            load: load.to_string(),
            active: active.to_string(),
            sub: sub.to_string(),
            description,
            manager: "systemd".to_string(),
        });
    }
    entries
}

/// Parses `rc-status -a` output from OpenRC containers.
pub fn parse_openrc_units(output: &str) -> Vec<ServiceEntry> {
    let mut entries = Vec::new();
    let mut runlevel = String::new();
    for raw_line in output.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("__MGR:") {
            continue;
        }
        if let Some(level) = line.strip_prefix("Runlevel:") {
            runlevel = level.trim().to_string();
            continue;
        }
        if let (Some(lb), Some(rb)) = (line.rfind('['), line.rfind(']')) {
            if lb < rb {
                let unit = line[..lb].trim();
                let state = line[lb + 1..rb].trim();
                if !unit.is_empty() {
                    let active = if state == "started" {
                        "active"
                    } else {
                        "inactive"
                    };
                    entries.push(ServiceEntry {
                        unit: unit.to_string(),
                        load: "loaded".to_string(),
                        active: active.to_string(),
                        sub: state.to_string(),
                        description: if runlevel.is_empty() {
                            String::new()
                        } else {
                            format!("runlevel {runlevel}")
                        },
                        manager: "openrc".to_string(),
                    });
                }
            }
        }
    }
    entries
}

/// Dispatches service list parsing based on the `__MGR:` marker emitted by `list_services_script`.
pub fn parse_service_list(output: &str) -> Vec<ServiceEntry> {
    if output.lines().any(|l| l.trim() == "__MGR:openrc") {
        parse_openrc_units(output)
    } else {
        parse_systemd_units(output)
    }
}

/// Builds a shell script to run `start`, `stop`, `restart`, `enable`, or `disable`
/// on `unit` inside `env_name`.
pub fn service_action_script(env_name: &str, action: &str, unit: &str) -> Result<String, String> {
    let clean_action = action.trim().to_ascii_lowercase();
    if !matches!(
        clean_action.as_str(),
        "start" | "stop" | "restart" | "enable" | "disable"
    ) {
        return Err(format!("Unsupported service action: '{action}'."));
    }
    if !is_valid_unit_name(unit) {
        return Err(format!("Invalid unit name: '{unit}'."));
    }
    let clean_unit = unit.trim();
    let q_name = shell_quote(env_name);
    let q_unit = shell_quote(clean_unit);
    let inner = match clean_action.as_str() {
        "enable" => format!(
            "if command -v systemctl >/dev/null 2>&1; then \
               systemctl enable {q_unit}; \
             elif command -v rc-update >/dev/null 2>&1; then \
               rc-update add {q_unit} default; \
             else \
               echo \"Error: Neither systemctl nor rc-update found.\" >&2; exit 1; \
             fi"
        ),
        "disable" => format!(
            "if command -v systemctl >/dev/null 2>&1; then \
               systemctl disable {q_unit}; \
             elif command -v rc-update >/dev/null 2>&1; then \
               rc-update del {q_unit} default; \
             else \
               echo \"Error: Neither systemctl nor rc-update found.\" >&2; exit 1; \
             fi"
        ),
        _ => format!(
            "if command -v systemctl >/dev/null 2>&1; then \
               systemctl {clean_action} {q_unit}; \
             elif command -v rc-service >/dev/null 2>&1; then \
               rc-service {q_unit} {clean_action}; \
             else \
               echo \"Error: Neither systemctl nor rc-service found.\" >&2; exit 1; \
             fi"
        ),
    };
    Ok(format!(
        "incus exec {q_name} -- sh -c {}",
        shell_quote(&inner)
    ))
}

/// Builds a shell script to tail logs for `unit` inside `env_name`.
pub fn service_logs_script(env_name: &str, unit: &str, lines: usize) -> Result<String, String> {
    if !is_valid_unit_name(unit) {
        return Err(format!("Invalid unit name: '{unit}'."));
    }
    let clean_unit = unit.trim();
    let count = lines.clamp(1, 2000);
    let q_name = shell_quote(env_name);
    let q_unit = shell_quote(clean_unit);
    let q_log = shell_quote(&format!("/var/log/{clean_unit}.log"));
    let inner = format!(
        "if command -v journalctl >/dev/null 2>&1; then \
           journalctl -u {q_unit} -n {count} --no-pager; \
         elif [ -f {q_log} ]; then \
           tail -n {count} {q_log}; \
         else \
           echo \"Error: journalctl not available and {q_log} not found.\" >&2; exit 1; \
         fi"
    );
    Ok(format!(
        "incus exec {q_name} -- sh -c {}",
        shell_quote(&inner)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_name_validation_rejects_shell_metacharacters() {
        assert!(is_valid_unit_name("nginx.service"));
        assert!(is_valid_unit_name("postgresql@16-main.service"));
        assert!(is_valid_unit_name("sshd"));
        assert!(!is_valid_unit_name(""));
        assert!(!is_valid_unit_name("nginx; rm -rf /"));
        assert!(!is_valid_unit_name("foo/bar"));
    }

    #[test]
    fn parses_systemd_and_openrc_service_lists() {
        let systemd_raw = "\
__MGR:systemd
nginx.service loaded active running A high performance web server
ssh.service   loaded active running OpenBSD Secure Shell server
";
        let systemd = parse_service_list(systemd_raw);
        assert_eq!(systemd.len(), 2);
        assert_eq!(systemd[0].unit, "nginx.service");
        assert_eq!(systemd[0].active, "active");
        assert_eq!(systemd[0].sub, "running");
        assert_eq!(systemd[0].manager, "systemd");

        let openrc_raw = "\
__MGR:openrc
Runlevel: default
 sshd                                                             [  started  ]
 cron                                                             [  stopped  ]
";
        let openrc = parse_service_list(openrc_raw);
        assert_eq!(openrc.len(), 2);
        assert_eq!(openrc[0].unit, "sshd");
        assert_eq!(openrc[0].active, "active");
        assert_eq!(openrc[0].sub, "started");
        assert_eq!(openrc[1].active, "inactive");
        assert_eq!(openrc[1].manager, "openrc");
    }

    #[test]
    fn builds_action_and_log_scripts() {
        let act = service_action_script("srv-1", "restart", "nginx.service").expect("action");
        assert!(act.contains("incus exec 'srv-1'"));
        assert!(act.contains("systemctl restart"));
        assert!(act.contains("rc-service"));

        let logs = service_logs_script("srv-1", "nginx.service", 120).expect("logs");
        assert!(logs.contains("journalctl -u"));
        assert!(logs.contains("-n 120"));

        assert!(service_action_script("srv-1", "bogus", "nginx").is_err());
        assert!(service_action_script("srv-1", "start", "bad;unit").is_err());
        assert!(service_logs_script("srv-1", "bad unit", 50).is_err());
    }
}
