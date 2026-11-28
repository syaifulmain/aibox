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
    if let Some(rest) = trimmed
        .strip_prefix("http://")
        .or_else(|| trimmed.strip_prefix("https://"))
    {
        let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
        if authority.is_empty() || authority.starts_with('-') {
            return Err(format!("Invalid HTTP(S) URL target: '{trimmed}'."));
        }
        return Ok(trimmed.to_string());
    }
    if let Some((host, port_s)) = trimmed.rsplit_once(':') {
        let port = port_s
            .parse::<u16>()
            .map_err(|_| format!("Invalid port in health check target: '{trimmed}'."))?;
        if port == 0 || !is_valid_probe_host(host) {
            return Err(format!("Invalid host:port target: '{trimmed}'."));
        }
        return Ok(trimmed.to_string());
    }
    Err(format!(
        "Health check target must be http(s)://..., host:port, or a port number (got '{trimmed}')."
    ))
}

fn is_valid_probe_host(host: &str) -> bool {
    if host.is_empty() || host.starts_with('-') {
        return false;
    }
    if let Some(inner) = host.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        return !inner.is_empty()
            && inner
                .bytes()
                .all(|b| b.is_ascii_hexdigit() || matches!(b, b':' | b'.'));
    }
    host.bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

/// Builds a shell script that probes an HTTP(S) URL with `curl` or a TCP `host:port`.
pub fn probe_target_script(target: &str) -> String {
    if target.starts_with("http://") || target.starts_with("https://") {
        let q_target = shell_quote(target);
        return format!("curl -fsS --max-time 5 -o /dev/null {q_target}");
    }
    if let Some((host, port_s)) = target.rsplit_once(':') {
        let clean_host = host
            .strip_prefix('[')
            .and_then(|h| h.strip_suffix(']'))
            .unwrap_or(host);
        let q_host = shell_quote(clean_host);
        let q_port = shell_quote(port_s);
        return format!(
            "if command -v nc >/dev/null 2>&1; then \
               nc -z -w 3 {q_host} {q_port}; \
             else \
               timeout 3 bash -c 'cat < /dev/null > \"/dev/tcp/$1/$2\"' _ {q_host} {q_port}; \
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
            "incus config set {q_name} user.aibox.healthcheck={q_target} user.aibox.healthcheck_restart={restart_val}"
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
        assert!(validate_probe_target("http://").is_err());
        assert!(validate_probe_target("https:///path").is_err());
        assert!(validate_probe_target("notaport").is_err());
        assert!(validate_probe_target("-e:80").is_err());
        assert!(validate_probe_target("a\";id;:80").is_err());
        assert!(validate_probe_target("$(id):80").is_err());
        assert_eq!(validate_probe_target("[::1]:8080").unwrap(), "[::1]:8080");
    }

    #[test]
    fn builds_probe_config_and_notify_scripts() {
        let http = probe_target_script("http://127.0.0.1:3000/health");
        assert!(http.contains("curl -fsS"));

        let tcp = probe_target_script("127.0.0.1:5432");
        assert!(tcp.contains("nc -z"));
        assert!(tcp.contains("\"/dev/tcp/$1/$2\""));

        let ipv6 = probe_target_script("[::1]:8080");
        assert!(ipv6.contains("'::1' '8080'"));

        let set = set_healthcheck_script("web", "127.0.0.1:8080", true);
        assert!(set.contains("user.aibox.healthcheck='127.0.0.1:8080'"));
        assert!(set.contains("user.aibox.healthcheck_restart=true"));

        let unset = set_healthcheck_script("web", "", false);
        assert!(unset.contains("incus config unset 'web' user.aibox.healthcheck"));

        let notify = notify_healthcheck_down_script("web", "127.0.0.1:8080", true);
        assert!(notify.contains("notify-send"));
        assert!(notify.contains("Restarted web"));
    }
}
