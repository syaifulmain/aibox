//! Incus network ACL snippet and network control script builders.

use std::collections::HashSet;
use std::net::IpAddr;

use serde_json::Value;

use crate::network::allowlist::{parse_allowlist_entries, resolve_allowlist_cidrs, HostResolver};
use crate::util::shell::shell_quote;

/// Result of preparing a `setNetworkMode` or `setEgressRecording` shell task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkTaskPlan {
    pub description: String,
    pub script: String,
}

/// Difference between current and desired Incus ACL egress destination CIDRs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AclRuleDiff {
    pub to_add: Vec<String>,
    pub to_remove: Vec<String>,
}

/// Computes which egress CIDRs must be added or removed to transition from
/// `current_cidrs` to `desired_cidrs` without rewriting unchanged rules.
pub fn diff_acl_cidrs(current_cidrs: &[String], desired_cidrs: &[String]) -> AclRuleDiff {
    let current_set: HashSet<&str> = current_cidrs.iter().map(String::as_str).collect();
    let desired_set: HashSet<&str> = desired_cidrs.iter().map(String::as_str).collect();

    let mut seen_remove = HashSet::new();
    let mut to_remove = Vec::new();
    for cidr in current_cidrs {
        if !desired_set.contains(cidr.as_str()) && seen_remove.insert(cidr.as_str()) {
            to_remove.push(cidr.clone());
        }
    }

    let mut seen_add = HashSet::new();
    let mut to_add = Vec::new();
    for cidr in desired_cidrs {
        if !current_set.contains(cidr.as_str()) && seen_add.insert(cidr.as_str()) {
            to_add.push(cidr.clone());
        }
    }

    AclRuleDiff { to_add, to_remove }
}

/// Extracts `destination` CIDRs from `incus network acl show <acl> --format=json` output.
pub fn parse_acl_egress_destinations(acl_json: &str) -> Vec<String> {
    let Ok(Value::Object(root)) = serde_json::from_str::<Value>(acl_json) else {
        return Vec::new();
    };
    let Some(Value::Array(egress)) = root.get("egress") else {
        return Vec::new();
    };
    let mut cidrs = Vec::new();
    for rule in egress {
        let Some(obj) = rule.as_object() else {
            continue;
        };
        let action = obj.get("action").and_then(Value::as_str).unwrap_or("");
        let destination = obj.get("destination").and_then(Value::as_str).unwrap_or("");
        if action == "allow" && !destination.is_empty() && !cidrs.iter().any(|c| c == destination) {
            cidrs.push(destination.to_string());
        }
    }
    cidrs
}

fn format_acl_rule_add(q_acl: &str, cidr: &str, egress_logging: bool) -> String {
    let q_cidr = shell_quote(cidr);
    if egress_logging {
        format!("incus network acl rule add {q_acl} egress action=allow destination={q_cidr} state=logged")
    } else {
        format!("incus network acl rule add {q_acl} egress action=allow destination={q_cidr}")
    }
}

/// Builds an incremental Incus ACL diff snippet that only adds/removes changed rules
/// when `current_cidrs` is non-empty, falling back to full creation otherwise.
pub fn acl_diff_shell_snippet(
    name: &str,
    allowlist_raw: &str,
    current_cidrs: &[String],
    desired_cidrs: &[String],
    egress_logging: bool,
) -> String {
    if current_cidrs.is_empty() {
        return acl_apply_shell_snippet(name, allowlist_raw, desired_cidrs, egress_logging);
    }

    let diff = diff_acl_cidrs(current_cidrs, desired_cidrs);
    let q_name = shell_quote(name);
    let q_acl = shell_quote(&format!("aibox-{name}"));
    let q_allow = shell_quote(allowlist_raw);

    let mut lines = Vec::new();
    for cidr in &diff.to_remove {
        let q_cidr = shell_quote(cidr);
        lines.push(format!(
            "incus network acl rule remove {q_acl} egress action=allow destination={q_cidr} >/dev/null 2>&1 || true"
        ));
    }
    for cidr in &diff.to_add {
        lines.push(format_acl_rule_add(&q_acl, cidr, egress_logging));
    }
    lines.push(format!(
        "incus config set {q_name} user.aibox.netallow={q_allow}"
    ));
    lines.join("\n")
}

/// Builds the Incus ACL shell snippet for allowlist mode.
pub fn acl_apply_shell_snippet(
    name: &str,
    allowlist_raw: &str,
    resolved_cidrs: &[String],
    egress_logging: bool,
) -> String {
    let q_name = shell_quote(name);
    let acl_name = format!("aibox-{name}");
    let q_acl = shell_quote(&acl_name);
    let q_log_acl = shell_quote(&format!("aibox-log-{name}"));
    let q_allow = shell_quote(allowlist_raw);

    let mut lines = Vec::new();
    lines.push(format!(
        "incus config device remove {q_name} eth0 >/dev/null 2>&1 || true"
    ));
    lines.push(format!(
        "incus network acl delete {q_acl} >/dev/null 2>&1 || true"
    ));
    lines.push(format!(
        "incus network acl delete {q_log_acl} >/dev/null 2>&1 || true"
    ));
    lines.push(format!("incus network acl create {q_acl}"));
    for cidr in resolved_cidrs {
        lines.push(format_acl_rule_add(&q_acl, cidr, egress_logging));
    }
    if egress_logging {
        lines.push(format!(
            "incus config device override {q_name} eth0 security.acls={q_acl} security.acls.default.egress.action=drop security.acls.default.egress.logged=true"
        ));
        lines.push(format!(
            "incus config set {q_name} user.aibox.egresslog=true"
        ));
    } else {
        lines.push(format!(
            "incus config device override {q_name} eth0 security.acls={q_acl} security.acls.default.egress.action=drop"
        ));
        lines.push(format!(
            "incus config unset {q_name} user.aibox.egresslog >/dev/null 2>&1 || true"
        ));
    }
    lines.push(format!(
        "incus config set {q_name} user.aibox.netmode=allowlist"
    ));
    lines.push(format!(
        "incus config set {q_name} user.aibox.netallow={q_allow}"
    ));
    lines.join("\n")
}

/// Builds the full shell script for `online`, `offline`, or `allowlist` network mode.
pub fn network_control_script(
    name: &str,
    mode: &str,
    allowlist_raw: &str,
    resolved_cidrs: &[String],
    egress_logging: bool,
) -> String {
    let q_name = shell_quote(name);
    let q_acl = shell_quote(&format!("aibox-{name}"));
    let q_log_acl = shell_quote(&format!("aibox-log-{name}"));
    let mut lines = vec!["set -euo pipefail".to_string()];

    if mode == "offline" {
        lines.push("echo '@@STAGE:40:Disabling network interface...'".to_string());
        lines.push(format!(
            "incus config device remove {q_name} eth0 >/dev/null 2>&1 || true"
        ));
        lines.push(format!("incus config device add {q_name} eth0 none"));
        lines.push(format!(
            "incus network acl delete {q_acl} >/dev/null 2>&1 || true"
        ));
        lines.push(format!(
            "incus network acl delete {q_log_acl} >/dev/null 2>&1 || true"
        ));
        lines.push(format!(
            "incus config set {q_name} user.aibox.netmode=offline"
        ));
        lines.push(format!(
            "incus config unset {q_name} user.aibox.egresslog >/dev/null 2>&1 || true"
        ));
        lines.push("echo '@@STAGE:100:Network disabled.'".to_string());
    } else if mode == "allowlist" {
        lines.push("echo '@@STAGE:40:Applying network allowlist ACL...'".to_string());
        lines.push(acl_apply_shell_snippet(
            name,
            allowlist_raw,
            resolved_cidrs,
            egress_logging,
        ));
        lines.push("echo '@@STAGE:100:Network allowlist active.'".to_string());
    } else {
        let stage40 = if egress_logging {
            "Enabling online egress logging ACL..."
        } else {
            "Restoring full network access..."
        };
        lines.push(format!("echo '@@STAGE:40:{stage40}'"));
        lines.push(format!(
            "incus config device remove {q_name} eth0 >/dev/null 2>&1 || true"
        ));
        lines.push(format!(
            "incus network acl delete {q_acl} >/dev/null 2>&1 || true"
        ));
        lines.push(format!(
            "incus network acl delete {q_log_acl} >/dev/null 2>&1 || true"
        ));
        lines.push(format!(
            "incus config unset {q_name} user.aibox.netmode >/dev/null 2>&1 || true"
        ));
        lines.push(format!(
            "incus config unset {q_name} user.aibox.netallow >/dev/null 2>&1 || true"
        ));
        if egress_logging {
            lines.push(format!("incus network acl create {q_log_acl}"));
            lines.push(format!(
                "incus config device override {q_name} eth0 security.acls={q_log_acl} security.acls.default.egress.action=allow security.acls.default.egress.logged=true"
            ));
            lines.push(format!(
                "incus config set {q_name} user.aibox.egresslog=true"
            ));
        } else {
            lines.push(format!(
                "incus config unset {q_name} user.aibox.egresslog >/dev/null 2>&1 || true"
            ));
        }
        let stage100 = if egress_logging {
            "Egress logging active."
        } else {
            "Network online."
        };
        lines.push(format!("echo '@@STAGE:100:{stage100}'"));
    }

    lines.join("\n")
}

/// Returns the ACL refresh snippet if `network_mode == "allowlist"` and entries resolve.
pub fn build_allowlist_refresh_if_needed(
    name: &str,
    network_mode: &str,
    network_allowlist: &str,
    egress_recording: bool,
    resolver: &dyn HostResolver,
) -> String {
    if network_mode != "allowlist" {
        return String::new();
    }
    let entries = parse_allowlist_entries(network_allowlist);
    let (cidrs, _) = resolve_allowlist_cidrs(&entries, resolver);
    if entries.is_empty() || cidrs.is_empty() {
        return String::new();
    }
    acl_apply_shell_snippet(name, network_allowlist, &cidrs, egress_recording)
}

/// Validates and builds the task plan for `Backend::setNetworkMode`.
pub fn prepare_set_network_mode(
    name: &str,
    mode: &str,
    allowlist: &str,
    egress_recording: bool,
    resolver: &dyn HostResolver,
) -> Result<NetworkTaskPlan, String> {
    let normalized_mode = mode.trim().to_ascii_lowercase();
    if normalized_mode != "online" && normalized_mode != "offline" && normalized_mode != "allowlist"
    {
        return Err(format!("Unsupported network mode: {mode}"));
    }

    let mut cidrs = Vec::new();
    let mut normalized_allowlist = String::new();
    if normalized_mode == "allowlist" {
        let entries = parse_allowlist_entries(allowlist);
        if entries.is_empty() {
            return Err("Enter at least one domain, IP, or CIDR for allowlist mode.".to_string());
        }
        let (resolved, warnings) = resolve_allowlist_cidrs(&entries, resolver);
        if resolved.is_empty() {
            return Err(if warnings.is_empty() {
                "Could not resolve any public IPv4 destinations from the allowlist.".to_string()
            } else {
                warnings.join(" ")
            });
        }
        cidrs = resolved;
        normalized_allowlist = entries.join(", ");
    }

    let keep_logging = normalized_mode != "offline" && egress_recording;
    let script = network_control_script(
        name,
        &normalized_mode,
        &normalized_allowlist,
        &cidrs,
        keep_logging,
    );
    Ok(NetworkTaskPlan {
        description: format!("Updating network mode for '{name}'"),
        script,
    })
}

/// Validates and builds the task plan for `Backend::setEgressRecording`.
pub fn prepare_set_egress_recording(
    name: &str,
    mode: &str,
    allowlist: &str,
    enabled: bool,
    resolver: &dyn HostResolver,
) -> Result<NetworkTaskPlan, String> {
    if enabled && mode == "offline" {
        return Err("Disable Offline mode first to record egress traffic.".to_string());
    }

    let mut allow_raw = String::new();
    let mut cidrs = Vec::new();
    let target_mode = if mode == "allowlist" {
        allow_raw = allowlist.to_string();
        let entries = parse_allowlist_entries(&allow_raw);
        let (resolved, _) = resolve_allowlist_cidrs(&entries, resolver);
        if entries.is_empty() || resolved.is_empty() {
            return Err(
                "Allowlist entries could not be resolved. Update the allowlist first.".to_string(),
            );
        }
        cidrs = resolved;
        "allowlist"
    } else {
        "online"
    };

    let script = network_control_script(name, target_mode, &allow_raw, &cidrs, enabled);
    let description = if enabled {
        format!("Enabling egress recording for '{name}'")
    } else {
        format!("Disabling egress recording for '{name}'")
    };
    Ok(NetworkTaskPlan {
        description,
        script,
    })
}

/// Appends `destination_ip` to `existing_allowlist` if not already present (as IP or `/32` / `/128`).
pub fn add_egress_destination_allowlist(
    existing_allowlist: &str,
    destination_ip: &str,
) -> Option<String> {
    let ip = destination_ip.trim().to_ascii_lowercase();
    if ip.is_empty() {
        return None;
    }
    let mut entries = parse_allowlist_entries(existing_allowlist);
    let ip_cidr = match ip.parse::<IpAddr>() {
        Ok(IpAddr::V6(_)) => format!("{ip}/128"),
        _ => format!("{ip}/32"),
    };
    if !entries.iter().any(|e| e == &ip || e == &ip_cidr) {
        entries.push(ip);
    }
    Some(entries.join(", "))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::network::allowlist::StaticResolver;

    #[test]
    fn network_control_scripts_match_qt_tests() {
        let allow_script = network_control_script(
            "agent-1",
            "allowlist",
            "github.com, 1.1.1.1",
            &[
                "140.82.121.4/32".to_string(),
                "1.1.1.1/32".to_string(),
                "10.0.0.0/8".to_string(),
            ],
            false,
        );
        assert!(allow_script.contains("incus network acl create 'aibox-agent-1'"));
        assert!(allow_script.contains(
            "incus network acl rule add 'aibox-agent-1' egress action=allow destination='140.82.121.4/32'"
        ));
        assert!(allow_script.contains("security.acls.default.egress.action=drop"));

        let offline_script = network_control_script("agent-1", "offline", "", &[], false);
        assert!(offline_script.contains("incus config device add 'agent-1' eth0 none"));
        assert!(offline_script.contains("user.aibox.netmode=offline"));

        let online_script = network_control_script("agent-1", "online", "", &[], false);
        assert!(online_script.contains("incus config device remove 'agent-1' eth0"));
        assert!(online_script.contains("incus network acl delete 'aibox-agent-1'"));

        let online_log_script = network_control_script("agent-1", "online", "", &[], true);
        assert!(online_log_script.contains("incus network acl create 'aibox-log-agent-1'"));
        assert!(online_log_script.contains("security.acls.default.egress.action=allow"));
        assert!(online_log_script.contains("security.acls.default.egress.logged=true"));

        let allow_log_script = network_control_script(
            "agent-1",
            "allowlist",
            "140.82.121.4",
            &["140.82.121.4/32".to_string()],
            true,
        );
        assert!(
            allow_log_script.contains("action=allow destination='140.82.121.4/32' state=logged")
        );
        assert!(allow_log_script.contains(
            "security.acls.default.egress.action=drop security.acls.default.egress.logged=true"
        ));
    }

    #[test]
    fn validation_errors_for_mode_and_egress_recording() {
        let empty_resolver = StaticResolver::new(BTreeMap::new());
        assert!(prepare_set_network_mode("box1", "bogus", "", false, &empty_resolver).is_err());
        assert!(
            prepare_set_network_mode("box1", "allowlist", "   ", false, &empty_resolver).is_err()
        );
        assert!(
            prepare_set_egress_recording("box1", "offline", "", true, &empty_resolver).is_err()
        );
        assert_eq!(
            add_egress_destination_allowlist("1.1.1.1", "1.1.1.1"),
            Some("1.1.1.1".to_string())
        );
        assert_eq!(
            add_egress_destination_allowlist("1.1.1.1", "8.8.8.8"),
            Some("1.1.1.1, 8.8.8.8".to_string())
        );
        assert_eq!(
            add_egress_destination_allowlist("2606:4700:4700::1111/128", "2606:4700:4700::1111"),
            Some("2606:4700:4700::1111/128".to_string())
        );
    }

    #[test]
    fn acl_diff_computes_minimal_add_remove_and_parses_incus_json() {
        let json = r#"{
            "name": "aibox-agent-1",
            "egress": [
                {"action": "allow", "destination": "1.1.1.1/32"},
                {"action": "allow", "destination": "8.8.8.8/32"},
                {"action": "drop", "destination": "0.0.0.0/0"}
            ]
        }"#;
        let current = parse_acl_egress_destinations(json);
        assert_eq!(current, vec!["1.1.1.1/32", "8.8.8.8/32"]);

        let desired = vec!["8.8.8.8/32".to_string(), "140.82.121.4/32".to_string()];
        let diff = diff_acl_cidrs(&current, &desired);
        assert_eq!(diff.to_remove, vec!["1.1.1.1/32"]);
        assert_eq!(diff.to_add, vec!["140.82.121.4/32"]);

        let snippet = acl_diff_shell_snippet(
            "agent-1",
            "dns.google, github.com",
            &current,
            &desired,
            true,
        );
        assert!(!snippet.contains("incus network acl delete"));
        assert!(!snippet.contains("incus network acl create"));
        assert!(snippet.contains(
            "incus network acl rule remove 'aibox-agent-1' egress action=allow destination='1.1.1.1/32'"
        ));
        assert!(snippet.contains(
            "incus network acl rule add 'aibox-agent-1' egress action=allow destination='140.82.121.4/32' state=logged"
        ));
        assert!(!snippet.contains("8.8.8.8/32"));
    }
}
