use serde::Serialize;

use crate::util::shell::shell_quote;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthProbeResult {
    pub env_name: String,
    pub target: String,
    pub healthy: bool,
    pub detail: String,
    pub restarted: bool,
}

/// Validates and normalizes a health-check probe target (`http(s)://...`, `host:port`,
/// or a plain TCP port `1..=65535` normalized to `127.0.0.1:<port>`).
pub fn validate_probe_target(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Health check target cannot be empty.".to_string());
    }
    if trimmed
        .bytes()
        .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
    {
        return Err(
            "Health check target cannot contain whitespace or control characters.".to_string(),
        );
    }
    if let Ok(port) = trimmed.parse::<u16>() {
        if port == 0 {
            return Err("Port must be between 1 and 65535.".to_string());
        }
        return Ok(format!("127.0.0.1:{port}"));
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Ok(trimmed.to_string());
    }
    if let Some((host, port_s)) = trimmed.rsplit_once(':') {
        let port = port_s
            .parse::<u16>()
            .map_err(|_| format!("Invalid port in health check target: '{trimmed}'."))?;
        if host.is_empty() || port == 0 {
            return Err(format!("Invalid host:port target: '{trimmed}'."));
        }
        return Ok(trimmed.to_string());
    }
    Err(format!(
        "Health check target must be http(s)://..., host:port, or a port number (got '{trimmed}')."
    ))
}

/// Builds a shell script that probes an HTTP(S) URL with `curl` or a TCP `host:port`.
pub fn probe_target_script(target: &str) -> String {
    if target.starts_with("http://") || target.starts_with("https://") {
        let q_target = shell_quote(target);
        return format!("curl -fsS --max-time 5 -o /dev/null {q_target}");
    }
    if let Some((host, port_s)) = target.rsplit_once(':') {
        let q_host = shell_quote(host);
        let q_port = shell_quote(port_s);
        return format!(
            "if command -v nc >/dev/null 2>&1; then \
               nc -z -w 3 {q_host} {q_port}; \
             else \
               timeout 3 bash -c \"cat < /dev/null > /dev/tcp/{host}/{port_s}\"; \
             fi"
        );
    }
    "false".to_string()
}

/// Persists or clears `user.aibox.healthcheck` and `user.aibox.healthcheck_restart`.
pub fn set_healthcheck_script(env_name: &str, target: &str, auto_restart: bool) -> String {
    let q_name = shell_quote(env_name);
    if target.trim().is_empty() {
        format!(
            "incus config unset {q_name} user.aibox.healthcheck >/dev/null 2>&1 || true\n\
             incus config unset {q_name} user.aibox.healthcheck_restart >/dev/null 2>&1 || true"
        )
    } else {
        let q_target = shell_quote(target.trim());
        let restart_val = if auto_restart { "true" } else { "false" };
        format!(
            "incus config set {q_name} user.aibox.healthcheck {q_target} user.aibox.healthcheck_restart {restart_val}"
        )
    }
}

/// Best-effort desktop notification when a health check probe fails.
pub fn notify_healthcheck_down_script(env_name: &str, target: &str, restarted: bool) -> String {
    let summary = shell_quote(&format!("AIBox health check failed: {env_name}"));
    let body = if restarted {
        shell_quote(&format!("Target {target} is down. Restarted {env_name}."))
    } else {
        shell_quote(&format!("Target {target} is not responding."))
    };
    format!("if command -v notify-send >/dev/null 2>&1; then notify-send -u critical {summary} {body} || true; fi")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_and_normalizes_probe_targets() {
        assert_eq!(validate_probe_target("8080").unwrap(), "127.0.0.1:8080");
        assert_eq!(
            validate_probe_target("http://127.0.0.1:3000/health").unwrap(),
            "http://127.0.0.1:3000/health"
        );
        assert_eq!(
            validate_probe_target("10.0.0.5:5432").unwrap(),
            "10.0.0.5:5432"
        );
        assert!(validate_probe_target("").is_err());
        assert!(validate_probe_target("0").is_err());
        assert!(validate_probe_target("http://bad url").is_err());
        assert!(validate_probe_target("notaport").is_err());
    }

    #[test]
    fn builds_probe_config_and_notify_scripts() {
        let http = probe_target_script("http://127.0.0.1:3000/health");
        assert!(http.contains("curl -fsS"));

        let tcp = probe_target_script("127.0.0.1:5432");
        assert!(tcp.contains("nc -z"));

        let set = set_healthcheck_script("web", "127.0.0.1:8080", true);
        assert!(set.contains("user.aibox.healthcheck '127.0.0.1:8080'"));
        assert!(set.contains("user.aibox.healthcheck_restart true"));

        let unset = set_healthcheck_script("web", "", false);
        assert!(unset.contains("incus config unset 'web' user.aibox.healthcheck"));

        let notify = notify_healthcheck_down_script("web", "127.0.0.1:8080", true);
        assert!(notify.contains("notify-send"));
        assert!(notify.contains("Restarted web"));
    }
}
