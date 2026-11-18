//! Host-level health checks (user groups, UFW rules, workspace Btrfs mount options).

use crate::health::checks::HealthCheckResult;
use crate::process::CommandRunner;
use crate::util::paths::{clean_path, default_workspace_path};
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

/// Resolves the target host username from explicit input or `USER`/`LOGNAME` env vars.
pub(crate) fn resolve_target_user(custom_user: &str) -> String {
    let u = custom_user.trim();
    if !u.is_empty() {
        return u.to_string();
    }
    if let Ok(env_user) = std::env::var("USER") {
        let trimmed = env_user.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Ok(env_logname) = std::env::var("LOGNAME") {
        let trimmed = env_logname.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    String::new()
}

/// Checks that the target user belongs to `incus-admin`, `kvm`, `render`, and `video`.
pub fn check_user_groups(runner: &dyn CommandRunner, user: &str) -> HealthCheckResult {
    let mut r = HealthCheckResult {
        id: "groups".to_string(),
        title: "User group membership".to_string(),
        ..Default::default()
    };
    let out = runner.run("id", &["-nG", user]);
    let groups: Vec<&str> = out.std_out.split_whitespace().collect();
    let required = ["incus-admin", "kvm", "render", "video"];
    let missing: Vec<&str> = required
        .iter()
        .copied()
        .filter(|req| !groups.contains(req))
        .collect();
    if out.exit_code == 0 && missing.is_empty() {
        r.ok = true;
        r.detail = format!("User '{user}' belongs to incus-admin, kvm, render, video.");
    } else {
        r.ok = false;
        r.needs_root = true;
        r.detail = if missing.is_empty() {
            format!("Could not inspect groups for '{user}'.")
        } else {
            format!("Missing group(s): {}", missing.join(", "))
        };
        r.fix_command = format!("sudo usermod -aG incus-admin,kvm,render,video {user}");
    }
    r
}

/// Checks UFW firewall rules for `incusbr0` when UFW is active.
pub fn check_ufw_rules(runner: &dyn CommandRunner, ufw_rules_content: &str) -> HealthCheckResult {
    let mut r = HealthCheckResult {
        id: "ufw_rules".to_string(),
        title: "UFW firewall rules for incusbr0".to_string(),
        ..Default::default()
    };
    let ufw_state = runner.run("systemctl", &["is-active", "ufw"]);
    if ufw_state.std_out.trim() != "active" {
        r.ok = true;
        r.detail = "UFW firewall is inactive.".to_string();
        return r;
    }

    let mut rules = ufw_rules_content.to_string();
    let mut read_ok = !rules.is_empty();
    if !read_ok {
        if let Ok(content) = std::fs::read_to_string("/etc/ufw/user.rules") {
            rules = content;
            read_ok = true;
        }
    }

    static RE_IN: OnceLock<Regex> = OnceLock::new();
    static RE_FWD_IN: OnceLock<Regex> = OnceLock::new();
    static RE_FWD_OUT: OnceLock<Regex> = OnceLock::new();
    let has_in = RE_IN
        .get_or_init(|| {
            Regex::new(r"ufw-user-input\s+-i\s+incusbr0\s+-j\s+ACCEPT").expect("valid static regex")
        })
        .is_match(&rules);
    let has_fwd_in = RE_FWD_IN
        .get_or_init(|| {
            Regex::new(r"ufw-user-forward\s+-i\s+incusbr0\s+-j\s+ACCEPT")
                .expect("valid static regex")
        })
        .is_match(&rules);
    let has_fwd_out = RE_FWD_OUT
        .get_or_init(|| {
            Regex::new(r"ufw-user-forward\s+-o\s+incusbr0\s+-j\s+ACCEPT")
                .expect("valid static regex")
        })
        .is_match(&rules);

    if read_ok && has_in && has_fwd_in && has_fwd_out {
        r.ok = true;
        r.detail = "UFW allows input & routed traffic on incusbr0.".to_string();
    } else {
        r.ok = false;
        r.needs_root = true;
        r.detail = if read_ok {
            "UFW is missing allow/route rules for incusbr0.".to_string()
        } else {
            "Could not inspect /etc/ufw/user.rules.".to_string()
        };
        r.fix_command = "sudo ufw allow in on incusbr0 && sudo ufw route allow in on incusbr0 && sudo ufw route allow out on incusbr0 && sudo ufw reload".to_string();
    }
    r
}

/// Checks whether the workspace Btrfs mount enables `user_subvol_rm_allowed`.
pub fn check_workspace_subvol_rm(
    runner: &dyn CommandRunner,
    workspace_base: &str,
) -> HealthCheckResult {
    let mut r = HealthCheckResult {
        id: "workspace_subvol_rm".to_string(),
        title: "Workspace Btrfs subvolume deletion".to_string(),
        ..Default::default()
    };
    let raw_ws = if workspace_base.trim().is_empty() {
        default_workspace_path()
    } else {
        workspace_base.trim().to_string()
    };
    let mut probe_path = clean_path(&raw_ws);
    for _ in 0..16 {
        if probe_path.is_empty() || Path::new(&probe_path).exists() {
            break;
        }
        let Some(parent) = Path::new(&probe_path).parent() else {
            break;
        };
        let parent_str = parent.to_string_lossy();
        if parent_str.is_empty() || parent_str == probe_path {
            break;
        }
        probe_path = parent_str.into_owned();
    }
    let out = runner.run(
        "findmnt",
        &["-rn", "-o", "TARGET,FSTYPE,OPTIONS", "-T", &probe_path],
    );
    if out.exit_code != 0 || out.std_out.trim().is_empty() {
        r.ok = true;
        r.detail = format!("Could not query mount options for {probe_path}.");
        return r;
    }

    let cols: Vec<&str> = out
        .std_out
        .trim()
        .split(' ')
        .filter(|s| !s.is_empty())
        .collect();
    let mount_target = cols.first().copied().unwrap_or("");
    let fs_type = cols.get(1).copied().unwrap_or("").to_lowercase();
    let opts: Vec<&str> = cols
        .get(2)
        .copied()
        .unwrap_or("")
        .split(',')
        .filter(|s| !s.is_empty())
        .collect();
    if fs_type != "btrfs" {
        r.ok = true;
        r.detail = format!(
            "Workspace mount {mount_target} uses {fs_type} (user_subvol_rm_allowed not applicable)."
        );
    } else if opts.contains(&"user_subvol_rm_allowed") {
        r.ok = true;
        r.detail = format!("Btrfs mount {mount_target} enables user_subvol_rm_allowed.");
    } else {
        r.ok = false;
        r.needs_root = true;
        r.detail = format!(
            "Btrfs mount {mount_target} lacks user_subvol_rm_allowed; btrfs subvolume delete needs sudo (add option to /etc/fstab)."
        );
        r.fix_command = format!("sudo mount -o remount,user_subvol_rm_allowed {mount_target}");
    }
    r
}
