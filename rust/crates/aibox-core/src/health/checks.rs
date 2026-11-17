//! Host health checks and storage pool capacity health evaluation.

use crate::health::pool::{parse_incus_storage_info, sum_root_disk_quotas_bytes, StoragePoolInfo};
use crate::util::format::format_bytes;
use crate::util::paths::{clean_path, default_workspace_path};
use regex::Regex;
use std::ffi::CString;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub exit_code: i32,
    pub std_out: String,
    pub std_err: String,
}

impl Default for CommandOutput {
    fn default() -> Self {
        Self {
            exit_code: -1,
            std_out: String::new(),
            std_err: String::new(),
        }
    }
}

pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> CommandOutput;
}

/// Default host command runner using `std::process::Command` and `libc::access` for `__socket_rw__`.
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[&str]) -> CommandOutput {
        if program == "__socket_rw__" && !args.is_empty() {
            let sock = args[0];
            if !Path::new(sock).exists() {
                return CommandOutput {
                    exit_code: 1,
                    std_out: String::new(),
                    std_err: "Socket file not found.".to_string(),
                };
            }
            let rw = CString::new(sock)
                .map(|c| unsafe { libc::access(c.as_ptr(), libc::R_OK | libc::W_OK) == 0 })
                .unwrap_or(false);
            return CommandOutput {
                exit_code: if rw { 0 } else { 1 },
                std_out: String::new(),
                std_err: if rw {
                    String::new()
                } else {
                    "No read/write access on socket.".to_string()
                },
            };
        }

        let Ok(mut child) = Command::new(program)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        else {
            return CommandOutput::default();
        };

        let deadline = Instant::now() + Duration::from_millis(8000);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => {
                    let out = child.wait_with_output().ok();
                    return match out {
                        Some(o) => CommandOutput {
                            exit_code: o.status.code().unwrap_or(-1),
                            std_out: String::from_utf8_lossy(&o.stdout).into_owned(),
                            std_err: String::from_utf8_lossy(&o.stderr).into_owned(),
                        },
                        None => CommandOutput::default(),
                    };
                }
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        return CommandOutput {
                            exit_code: 124,
                            std_out: String::new(),
                            std_err: "Command timed out.".to_string(),
                        };
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => return CommandOutput::default(),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HealthCheckResult {
    pub id: String,
    pub title: String,
    pub ok: bool,
    pub warning: bool,
    pub repaired: bool,
    pub needs_root: bool,
    pub detail: String,
    pub fix_command: String,
}

/// Evaluates Incus storage pool capacity and overcommit status as a health check.
pub fn evaluate_pool_capacity_health(
    pool: &StoragePoolInfo,
    allocated_quotas_bytes: i64,
    unbounded_count: i32,
) -> HealthCheckResult {
    let mut r = HealthCheckResult {
        id: "pool_capacity".to_string(),
        title: format!("Storage pool '{}' capacity", pool.name),
        ..Default::default()
    };
    if !pool.valid || pool.total_bytes <= 0 {
        r.ok = true;
        r.warning = false;
        r.detail = format!("Could not query storage pool '{}' capacity.", pool.name);
        return r;
    }

    let mut nums = format!(
        "{} / {} ({}%), {} allocated across quotas",
        format_bytes(pool.used_bytes),
        format_bytes(pool.total_bytes),
        pool.used_percent,
        format_bytes(allocated_quotas_bytes)
    );
    if unbounded_count > 0 {
        nums.push_str(&format!(", {unbounded_count} without disk quota"));
    }
    if allocated_quotas_bytes > pool.total_bytes {
        nums.push_str(" [OVERCOMMITTED]");
    }

    if pool.used_percent > 95 {
        r.ok = false;
        r.warning = false;
        r.needs_root = false;
        r.detail = format!("Critical pool usage (>95%): {nums}.");
        r.fix_command = format!(
            "Wipe unused environments or delete old snapshots on pool '{}'.",
            pool.name
        );
    } else if pool.used_percent > 85 {
        r.ok = true;
        r.warning = true;
        r.needs_root = false;
        r.detail = format!("High pool usage warning (>85%): {nums}.");
    } else {
        r.ok = true;
        r.warning = false;
        r.detail = format!("Healthy pool usage: {nums}.");
    }
    r
}

fn resolve_target_user(custom_user: &str) -> String {
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

fn inspect_ai_gui_profile(runner: &dyn CommandRunner) -> (bool, String) {
    let out = runner.run("incus", &["profile", "show", "ai-gui"]);
    if out.exit_code != 0 {
        return (false, "Profile ai-gui does not exist.".to_string());
    }
    let txt = &out.std_out;
    let mut missing = Vec::new();
    if !txt.contains("security.nesting") {
        missing.push("security.nesting=true");
    }
    if !txt.contains("gpu:") {
        missing.push("gpu");
    }
    if !txt.contains("kvm:") && !txt.contains("/dev/kvm") {
        missing.push("kvm");
    }
    if !missing.is_empty() {
        return (
            false,
            format!("Profile ai-gui missing: {}", missing.join(", ")),
        );
    }
    (
        true,
        "Profile ai-gui configured with nesting, gpu, and kvm.".to_string(),
    )
}

/// Runs all 8 host health checks, optionally repairing bridge and `ai-gui` profile when `fix` is true.
pub fn run_health_checks(
    fix: bool,
    runner: &dyn CommandRunner,
    target_user: &str,
    ufw_rules_content: &str,
    workspace_base: &str,
) -> Vec<HealthCheckResult> {
    let user = resolve_target_user(target_user);
    let mut results = Vec::with_capacity(8);

    // 1. User group memberships
    {
        let mut r = HealthCheckResult {
            id: "groups".to_string(),
            title: "User group membership".to_string(),
            ..Default::default()
        };
        let out = runner.run("id", &["-nG", &user]);
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
        results.push(r);
    }

    // 2. Incus UNIX socket ACL
    {
        let mut r = HealthCheckResult {
            id: "socket_acl".to_string(),
            title: "Incus socket ACL".to_string(),
            ..Default::default()
        };
        let out = runner.run("__socket_rw__", &["/var/lib/incus/unix.socket"]);
        if out.exit_code == 0 {
            r.ok = true;
            r.detail = "Read/write access confirmed on /var/lib/incus/unix.socket.".to_string();
        } else {
            r.ok = false;
            r.needs_root = true;
            let trimmed_err = out.std_err.trim();
            r.detail = if trimmed_err.is_empty() {
                "Missing R/W permission on /var/lib/incus/unix.socket.".to_string()
            } else {
                trimmed_err.to_string()
            };
            r.fix_command = format!("sudo setfacl -m u:{user}:rw /var/lib/incus/unix.socket");
        }
        results.push(r);
    }

    // 3. Incus daemon reachability
    let mut daemon_reachable = false;
    let mut cached_list_json = String::new();
    {
        let mut r = HealthCheckResult {
            id: "incus_daemon".to_string(),
            title: "Incus daemon reachable".to_string(),
            ..Default::default()
        };
        let out = runner.run("incus", &["list", "--format=json"]);
        if out.exit_code == 0 {
            daemon_reachable = true;
            cached_list_json = out.std_out;
            r.ok = true;
            r.detail = "Incus daemon responded cleanly.".to_string();
        } else {
            r.ok = false;
            r.needs_root = true;
            let trimmed_err = out.std_err.trim();
            r.detail = if trimmed_err.is_empty() {
                "Incus daemon is not reachable.".to_string()
            } else {
                trimmed_err.to_string()
            };
            r.fix_command = "sudo systemctl enable --now incus.socket incus.service".to_string();
        }
        results.push(r);
    }

    // 4. Bridge network incusbr0
    {
        let mut r = HealthCheckResult {
            id: "bridge_incusbr0".to_string(),
            title: "Bridge network incusbr0".to_string(),
            ..Default::default()
        };
        let check_bridge = || {
            runner
                .run("incus", &["network", "show", "incusbr0"])
                .exit_code
                == 0
        };
        if check_bridge() {
            r.ok = true;
            r.detail = "Network bridge incusbr0 is configured.".to_string();
        } else {
            if fix && daemon_reachable {
                let rep = runner.run(
                    "incus",
                    &[
                        "network",
                        "create",
                        "incusbr0",
                        "ipv4.address=auto",
                        "ipv4.nat=true",
                        "ipv6.address=none",
                    ],
                );
                if rep.exit_code == 0 && check_bridge() {
                    r.ok = true;
                    r.repaired = true;
                    r.detail = "Created bridge incusbr0 automatically.".to_string();
                }
            }
            if !r.ok {
                r.needs_root = !daemon_reachable;
                r.detail = "Managed bridge incusbr0 is missing.".to_string();
                r.fix_command =
                    "incus network create incusbr0 ipv4.address=auto ipv4.nat=true ipv6.address=none"
                        .to_string();
            }
        }
        results.push(r);
    }

    // 5. ai-gui profile
    {
        let mut r = HealthCheckResult {
            id: "ai_gui_profile".to_string(),
            title: "ai-gui profile (nesting, gpu, kvm)".to_string(),
            ..Default::default()
        };
        let (ok_now, mut detail_now) = inspect_ai_gui_profile(runner);
        if ok_now {
            r.ok = true;
            r.detail = detail_now;
        } else {
            if fix && daemon_reachable {
                let show_before = runner.run("incus", &["profile", "show", "ai-gui"]);
                if show_before.exit_code != 0 {
                    runner.run("incus", &["profile", "create", "ai-gui"]);
                }
                runner.run(
                    "incus",
                    &["profile", "set", "ai-gui", "security.nesting=true"],
                );
                let show_mid = runner.run("incus", &["profile", "show", "ai-gui"]);
                if !show_mid.std_out.contains("gpu:") {
                    runner.run(
                        "incus",
                        &["profile", "device", "add", "ai-gui", "gpu", "gpu"],
                    );
                }
                if !show_mid.std_out.contains("kvm:") && !show_mid.std_out.contains("/dev/kvm") {
                    runner.run(
                        "incus",
                        &[
                            "profile",
                            "device",
                            "add",
                            "ai-gui",
                            "kvm",
                            "unix-char",
                            "path=/dev/kvm",
                        ],
                    );
                }
                let (ok_after, detail_after) = inspect_ai_gui_profile(runner);
                if ok_after {
                    r.ok = true;
                    r.repaired = true;
                    r.detail = "Repaired ai-gui profile automatically.".to_string();
                } else {
                    detail_now = detail_after;
                }
            }
            if !r.ok {
                r.needs_root = !daemon_reachable;
                r.detail = detail_now;
                r.fix_command = "incus profile create ai-gui; incus profile set ai-gui security.nesting=true; incus profile device add ai-gui gpu gpu; incus profile device add ai-gui kvm unix-char path=/dev/kvm".to_string();
            }
        }
        results.push(r);
    }

    // 6. UFW firewall rules for incusbr0
    {
        let mut r = HealthCheckResult {
            id: "ufw_rules".to_string(),
            title: "UFW firewall rules for incusbr0".to_string(),
            ..Default::default()
        };
        let ufw_state = runner.run("systemctl", &["is-active", "ufw"]);
        if ufw_state.std_out.trim() != "active" {
            r.ok = true;
            r.detail = "UFW firewall is inactive.".to_string();
        } else {
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
                    Regex::new(r"ufw-user-input\s+-i\s+incusbr0\s+-j\s+ACCEPT").unwrap()
                })
                .is_match(&rules);
            let has_fwd_in = RE_FWD_IN
                .get_or_init(|| {
                    Regex::new(r"ufw-user-forward\s+-i\s+incusbr0\s+-j\s+ACCEPT").unwrap()
                })
                .is_match(&rules);
            let has_fwd_out = RE_FWD_OUT
                .get_or_init(|| {
                    Regex::new(r"ufw-user-forward\s+-o\s+incusbr0\s+-j\s+ACCEPT").unwrap()
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
        }
        results.push(r);
    }

    // 7. Workspace Btrfs user_subvol_rm_allowed mount option
    {
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
        } else {
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
                r.fix_command =
                    format!("sudo mount -o remount,user_subvol_rm_allowed {mount_target}");
            }
        }
        results.push(r);
    }

    // 8. Incus storage pool capacity & overcommit check
    {
        let mut pool_info = StoragePoolInfo::default();
        if daemon_reachable {
            let mut st_out = runner.run("incus", &["storage", "info", "default", "--bytes"]);
            if st_out.exit_code != 0 || st_out.std_out.trim().is_empty() {
                st_out = runner.run("incus", &["storage", "info", "default"]);
            }
            if st_out.exit_code == 0 {
                pool_info = parse_incus_storage_info(&st_out.std_out);
            }
        }
        let (allocated_bytes, unbounded_count) =
            sum_root_disk_quotas_bytes(&cached_list_json, "", &pool_info.name);
        results.push(evaluate_pool_capacity_health(
            &pool_info,
            allocated_bytes,
            unbounded_count,
        ));
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct FnRunner<F>(F);
    impl<F: Fn(&str, &[&str]) -> CommandOutput> CommandRunner for FnRunner<F> {
        fn run(&self, program: &str, args: &[&str]) -> CommandOutput {
            (self.0)(program, args)
        }
    }

    #[test]
    fn health_checks_all_pass_with_healthy_host_stub() {
        let healthy_rules = "\
-A ufw-user-input -i incusbr0 -j ACCEPT
-A ufw-user-forward -i incusbr0 -j ACCEPT
-A ufw-user-forward -o incusbr0 -j ACCEPT
";
        let runner = FnRunner(|prog: &str, args: &[&str]| {
            match prog {
            "id" => CommandOutput {
                exit_code: 0,
                std_out: "testuser wheel incus-admin kvm render video\n".to_string(),
                std_err: String::new(),
            },
            "__socket_rw__" => CommandOutput {
                exit_code: 0,
                ..Default::default()
            },
            "systemctl" if args.first() == Some(&"is-active") => CommandOutput {
                exit_code: 0,
                std_out: "active\n".to_string(),
                std_err: String::new(),
            },
            "incus" => match (args.first().copied(), args.get(1).copied()) {
                (Some("list"), _) => CommandOutput {
                    exit_code: 0,
                    std_out: "[]".to_string(),
                    std_err: String::new(),
                },
                (Some("network"), Some("show")) => CommandOutput {
                    exit_code: 0,
                    std_out: "name: incusbr0\nmanaged: true\n".to_string(),
                    std_err: String::new(),
                },
                (Some("profile"), Some("show")) => CommandOutput {
                    exit_code: 0,
                    std_out: "config:\n  security.nesting: \"true\"\ndevices:\n  gpu:\n    type: gpu\n  kvm:\n    path: /dev/kvm\n".to_string(),
                    std_err: String::new(),
                },
                (Some("storage"), Some("info")) => CommandOutput {
                    exit_code: 0,
                    std_out: "info:\n  driver: btrfs\n  name: default\n  space used: \"1329242112\"\n  total space: \"32212254720\"\n".to_string(),
                    std_err: String::new(),
                },
                _ => CommandOutput {
                    exit_code: 1,
                    std_out: String::new(),
                    std_err: "unexpected incus".to_string(),
                },
            },
            "findmnt" => CommandOutput {
                exit_code: 0,
                std_out: "/home btrfs rw,relatime,user_subvol_rm_allowed\n".to_string(),
                std_err: String::new(),
            },
            _ => CommandOutput {
                exit_code: 1,
                std_out: String::new(),
                std_err: "unexpected command".to_string(),
            },
        }
        });

        let checks = run_health_checks(false, &runner, "testuser", healthy_rules, "/tmp");
        assert_eq!(checks.len(), 8);
        for c in checks {
            assert!(c.ok, "{}: {}", c.id, c.detail);
            assert!(!c.warning);
            assert!(!c.repaired);
        }
    }

    #[test]
    fn health_checks_fix_repairs_bridge_and_ai_gui_profile_without_root() {
        let state = RefCell::new((false, false, false, false));
        let runner = FnRunner(|prog: &str, args: &[&str]| {
            let mut st = state.borrow_mut();
            match prog {
                "id" => CommandOutput {
                    exit_code: 0,
                    std_out: "testuser incus-admin kvm render video\n".to_string(),
                    std_err: String::new(),
                },
                "__socket_rw__" => CommandOutput {
                    exit_code: 0,
                    ..Default::default()
                },
                "systemctl" => CommandOutput {
                    exit_code: 3,
                    std_out: "inactive\n".to_string(),
                    std_err: String::new(),
                },
                "incus" => match (args.first().copied(), args.get(1).copied()) {
                    (Some("list"), _) => CommandOutput {
                        exit_code: 0,
                        std_out: "[]".to_string(),
                        std_err: String::new(),
                    },
                    (Some("network"), Some("show")) => {
                        if st.0 {
                            CommandOutput {
                                exit_code: 0,
                                std_out: "name: incusbr0\n".to_string(),
                                std_err: String::new(),
                            }
                        } else {
                            CommandOutput {
                                exit_code: 1,
                                std_out: String::new(),
                                std_err: "not found".to_string(),
                            }
                        }
                    }
                    (Some("network"), Some("create")) if args.get(2) == Some(&"incusbr0") => {
                        st.0 = true;
                        CommandOutput {
                            exit_code: 0,
                            ..Default::default()
                        }
                    }
                    (Some("profile"), Some("show")) => {
                        let mut out = String::from("config:\n");
                        if st.1 {
                            out.push_str("  security.nesting: \"true\"\n");
                        }
                        out.push_str("devices:\n");
                        if st.2 {
                            out.push_str("  gpu:\n    type: gpu\n");
                        }
                        if st.3 {
                            out.push_str("  kvm:\n    path: /dev/kvm\n");
                        }
                        CommandOutput {
                            exit_code: 0,
                            std_out: out,
                            std_err: String::new(),
                        }
                    }
                    (Some("profile"), Some("set")) => {
                        st.1 = true;
                        CommandOutput {
                            exit_code: 0,
                            ..Default::default()
                        }
                    }
                    (Some("profile"), Some("device")) if args.get(2) == Some(&"add") => {
                        if args.get(4) == Some(&"gpu") {
                            st.2 = true;
                        }
                        if args.get(4) == Some(&"kvm") {
                            st.3 = true;
                        }
                        CommandOutput {
                            exit_code: 0,
                            ..Default::default()
                        }
                    }
                    _ => CommandOutput {
                        exit_code: 1,
                        std_out: String::new(),
                        std_err: "unhandled".to_string(),
                    },
                },
                _ => CommandOutput {
                    exit_code: 1,
                    std_out: String::new(),
                    std_err: "unhandled".to_string(),
                },
            }
        });

        let dry_run = run_health_checks(false, &runner, "testuser", "", "/tmp");
        assert!(!dry_run[3].ok);
        assert!(!dry_run[4].ok);

        let fixed = run_health_checks(true, &runner, "testuser", "", "/tmp");
        assert!(fixed[3].ok && fixed[3].repaired);
        assert!(fixed[4].ok && fixed[4].repaired);
        for c in fixed {
            assert!(c.ok);
        }
    }

    #[test]
    fn health_checks_report_root_fix_commands_when_privileged_items_fail() {
        let runner = FnRunner(|prog: &str, args: &[&str]| match prog {
            "id" => CommandOutput {
                exit_code: 0,
                std_out: "testuser wheel\n".to_string(),
                std_err: String::new(),
            },
            "__socket_rw__" => CommandOutput {
                exit_code: 1,
                std_out: String::new(),
                std_err: "permission denied".to_string(),
            },
            "systemctl" if args.first() == Some(&"is-active") => CommandOutput {
                exit_code: 0,
                std_out: "active\n".to_string(),
                std_err: String::new(),
            },
            "findmnt" => CommandOutput {
                exit_code: 0,
                std_out: "/home btrfs rw,relatime\n".to_string(),
                std_err: String::new(),
            },
            _ => CommandOutput {
                exit_code: 1,
                std_out: String::new(),
                std_err: "daemon down".to_string(),
            },
        });

        let checks = run_health_checks(true, &runner, "testuser", "# empty rules", "/tmp");
        assert_eq!(checks.len(), 8);
        assert!(
            !checks[0].ok
                && checks[0].needs_root
                && checks[0].fix_command.contains("sudo usermod -aG")
        );
        assert!(
            !checks[1].ok && checks[1].needs_root && checks[1].fix_command.contains("sudo setfacl")
        );
        assert!(
            !checks[2].ok
                && checks[2].needs_root
                && checks[2]
                    .fix_command
                    .contains("sudo systemctl enable --now")
        );
        assert!(
            !checks[5].ok
                && checks[5].needs_root
                && checks[5]
                    .fix_command
                    .contains("sudo ufw allow in on incusbr0")
        );
        assert_eq!(checks[6].id, "workspace_subvol_rm");
        assert!(
            !checks[6].ok
                && checks[6].needs_root
                && checks[6].fix_command.contains("user_subvol_rm_allowed")
        );
    }

    #[test]
    fn pool_capacity_health_check_thresholds() {
        let gib = 1024_i64 * 1024 * 1024;
        let mut pool = StoragePoolInfo {
            valid: true,
            name: "default".to_string(),
            total_bytes: 100 * gib,
            used_bytes: 85 * gib,
            used_percent: 85,
            ..Default::default()
        };
        let h85 = evaluate_pool_capacity_health(&pool, 60 * gib, 0);
        assert_eq!(h85.id, "pool_capacity");
        assert!(h85.ok && !h85.warning);
        assert!(h85.detail.contains("85%"));

        pool.used_bytes = 86 * gib;
        pool.used_percent = 86;
        let h86 = evaluate_pool_capacity_health(&pool, 60 * gib, 1);
        assert!(h86.ok && h86.warning);
        assert!(h86.detail.contains("86%"));
        assert!(h86.detail.contains("60.0 GiB"));

        pool.used_bytes = 96 * gib;
        pool.used_percent = 96;
        let h96 = evaluate_pool_capacity_health(&pool, 120 * gib, 0);
        assert!(!h96.ok && !h96.warning);
        assert!(h96.detail.contains("96%"));
        assert!(h96.detail.contains("OVERCOMMITTED"));

        let make_runner = |pct: &'static str| {
            FnRunner(move |prog: &str, args: &[&str]| {
                match prog {
                "id" => CommandOutput {
                    exit_code: 0,
                    std_out: "testuser incus-admin kvm render video\n".to_string(),
                    std_err: String::new(),
                },
                "__socket_rw__" => CommandOutput {
                    exit_code: 0,
                    ..Default::default()
                },
                "systemctl" => CommandOutput {
                    exit_code: 3,
                    std_out: "inactive\n".to_string(),
                    std_err: String::new(),
                },
                "incus" => match args.first().copied() {
                    Some("list") => CommandOutput {
                        exit_code: 0,
                        std_out: "[]".to_string(),
                        std_err: String::new(),
                    },
                    Some("network") => CommandOutput {
                        exit_code: 0,
                        std_out: "name: incusbr0\n".to_string(),
                        std_err: String::new(),
                    },
                    Some("profile") => CommandOutput {
                        exit_code: 0,
                        std_out: "config:\n  security.nesting: \"true\"\ndevices:\n  gpu:\n    type: gpu\n  kvm:\n    path: /dev/kvm\n".to_string(),
                        std_err: String::new(),
                    },
                    Some("storage") => CommandOutput {
                        exit_code: 0,
                        std_out: format!("info:\n  driver: btrfs\n  name: default\n  space used: \"{pct}\"\n  total space: \"100\"\n"),
                        std_err: String::new(),
                    },
                    _ => CommandOutput {
                        exit_code: 1,
                        ..Default::default()
                    },
                },
                "findmnt" => CommandOutput {
                    exit_code: 0,
                    std_out: "/home btrfs rw,user_subvol_rm_allowed\n".to_string(),
                    std_err: String::new(),
                },
                _ => CommandOutput {
                    exit_code: 1,
                    ..Default::default()
                },
            }
            })
        };

        let warn_checks = run_health_checks(false, &make_runner("90"), "testuser", "", "/tmp");
        let last_warn = warn_checks.last().unwrap();
        assert_eq!(last_warn.id, "pool_capacity");
        assert!(last_warn.ok && last_warn.warning);

        let fail_checks = run_health_checks(false, &make_runner("97"), "testuser", "", "/tmp");
        let last_fail = fail_checks.last().unwrap();
        assert_eq!(last_fail.id, "pool_capacity");
        assert!(!last_fail.ok && !last_fail.warning);
    }
}
