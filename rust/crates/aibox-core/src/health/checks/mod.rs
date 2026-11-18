//! Host health checks, cadence caching, and storage pool capacity health evaluation.

pub mod devices;
pub mod host;
pub mod incus;

pub use incus::evaluate_pool_capacity_health;

use crate::process::CommandRunner;
use devices::check_ai_gui_profile;
use host::{check_ufw_rules, check_user_groups, check_workspace_subvol_rm, resolve_target_user};
use incus::{check_bridge_incusbr0, check_incus_daemon, check_pool_capacity, check_socket_acl};
use std::collections::BTreeMap;

/// Structured outcome of a single host or Incus health check.
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

/// Evaluation frequency class for a health check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckCadence {
    /// Host/setup checks (groups, socket ACL, bridge, KVM/GPU profile, UFW, Btrfs mount).
    Static,
    /// Runtime checks that follow periodic polling (daemon reachability, pool capacity).
    Dynamic,
}

/// Execution mode passed to [`HealthCheckCache::run_with_cadence`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckRunMode {
    /// Run all static and dynamic checks (app startup or explicit user refresh).
    StartupOrManual,
    /// Run only dynamic checks; reuse cached static check results when available.
    Poll,
}

/// Cached health check result paired with the epoch timestamp (ms) when it was last evaluated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedHealthCheck {
    pub result: HealthCheckResult,
    pub checked_at_ms: i64,
}

/// Returns the cadence classification for a health check ID.
pub fn check_cadence(id: &str) -> CheckCadence {
    match id {
        "incus_daemon" | "pool_capacity" => CheckCadence::Dynamic,
        _ => CheckCadence::Static,
    }
}

const CANONICAL_CHECK_ORDER: [&str; 8] = [
    "groups",
    "socket_acl",
    "incus_daemon",
    "bridge_incusbr0",
    "ai_gui_profile",
    "ufw_rules",
    "workspace_subvol_rm",
    "pool_capacity",
];

/// Stores per-check results and timestamps so static checks are skipped during background polls.
#[derive(Debug, Clone, Default)]
pub struct HealthCheckCache {
    entries: BTreeMap<String, CachedHealthCheck>,
    pub last_static_run_ms: Option<i64>,
    pub last_dynamic_run_ms: Option<i64>,
}

impl HealthCheckCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the cached result and evaluation timestamp for `id`, if present.
    pub fn get(&self, id: &str) -> Option<&CachedHealthCheck> {
        self.entries.get(id)
    }

    /// Clears all cached check results so the next run evaluates every check.
    pub fn invalidate(&mut self) {
        self.entries.clear();
        self.last_static_run_ms = None;
        self.last_dynamic_run_ms = None;
    }

    /// Runs health checks according to `mode`, caching each result with `now_ms`.
    ///
    /// In [`CheckRunMode::Poll`] (when `fix` is false), static checks already in the cache
    /// are reused with their original `checked_at_ms`, while dynamic checks are always re-run.
    #[allow(clippy::too_many_arguments)]
    pub fn run_with_cadence(
        &mut self,
        mode: CheckRunMode,
        fix: bool,
        now_ms: i64,
        runner: &dyn CommandRunner,
        target_user: &str,
        ufw_rules_content: &str,
        workspace_base: &str,
    ) -> Vec<HealthCheckResult> {
        let user = resolve_target_user(target_user);
        let run_static = mode == CheckRunMode::StartupOrManual
            || fix
            || CANONICAL_CHECK_ORDER
                .iter()
                .filter(|id| check_cadence(id) == CheckCadence::Static)
                .any(|id| !self.entries.contains_key(*id));

        // Dynamic: Incus daemon reachability (also feeds bridge, profile, and pool checks).
        let (daemon_res, daemon_reachable, cached_list_json) = check_incus_daemon(runner);
        self.entries.insert(
            daemon_res.id.clone(),
            CachedHealthCheck {
                result: daemon_res,
                checked_at_ms: now_ms,
            },
        );

        if run_static {
            let static_results = [
                check_user_groups(runner, &user),
                check_socket_acl(runner, &user),
                check_bridge_incusbr0(fix, daemon_reachable, runner),
                check_ai_gui_profile(fix, daemon_reachable, runner),
                check_ufw_rules(runner, ufw_rules_content),
                check_workspace_subvol_rm(runner, workspace_base),
            ];
            for res in static_results {
                self.entries.insert(
                    res.id.clone(),
                    CachedHealthCheck {
                        result: res,
                        checked_at_ms: now_ms,
                    },
                );
            }
            self.last_static_run_ms = Some(now_ms);
        }

        // Dynamic: Storage pool capacity & overcommit check.
        let pool_res = check_pool_capacity(daemon_reachable, &cached_list_json, runner);
        self.entries.insert(
            pool_res.id.clone(),
            CachedHealthCheck {
                result: pool_res,
                checked_at_ms: now_ms,
            },
        );
        self.last_dynamic_run_ms = Some(now_ms);

        CANONICAL_CHECK_ORDER
            .iter()
            .filter_map(|id| self.entries.get(*id).map(|c| c.result.clone()))
            .collect()
    }
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

    results.push(check_user_groups(runner, &user));
    results.push(check_socket_acl(runner, &user));

    let (daemon_res, daemon_reachable, cached_list_json) = check_incus_daemon(runner);
    results.push(daemon_res);

    results.push(check_bridge_incusbr0(fix, daemon_reachable, runner));
    results.push(check_ai_gui_profile(fix, daemon_reachable, runner));
    results.push(check_ufw_rules(runner, ufw_rules_content));
    results.push(check_workspace_subvol_rm(runner, workspace_base));
    results.push(check_pool_capacity(
        daemon_reachable,
        &cached_list_json,
        runner,
    ));

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::pool::StoragePoolInfo;
    use crate::process::CommandOutput;
    use std::cell::RefCell;

    struct FnRunner<F, S> {
        run_fn: F,
        socket_fn: S,
    }

    impl<F, S> FnRunner<F, S>
    where
        F: Fn(&str, &[&str]) -> CommandOutput,
        S: Fn(&str) -> bool,
    {
        fn new(run_fn: F, socket_fn: S) -> Self {
            Self { run_fn, socket_fn }
        }
    }

    impl<F, S> CommandRunner for FnRunner<F, S>
    where
        F: Fn(&str, &[&str]) -> CommandOutput,
        S: Fn(&str) -> bool,
    {
        fn run(&self, program: &str, args: &[&str]) -> CommandOutput {
            (self.run_fn)(program, args)
        }

        fn socket_writable(&self, path: &str) -> bool {
            (self.socket_fn)(path)
        }
    }

    #[test]
    fn health_checks_all_pass_with_healthy_host_stub() {
        let healthy_rules = "\
-A ufw-user-input -i incusbr0 -j ACCEPT
-A ufw-user-forward -i incusbr0 -j ACCEPT
-A ufw-user-forward -o incusbr0 -j ACCEPT
";
        let runner = FnRunner::new(
            |prog: &str, args: &[&str]| {
                match prog {
                "id" => CommandOutput {
                    exit_code: 0,
                    std_out: "testuser wheel incus-admin kvm render video\n".to_string(),
                    std_err: String::new(),
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
            },
            |_| true,
        );

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
        let runner = FnRunner::new(
            |prog: &str, args: &[&str]| {
                let mut st = state.borrow_mut();
                match prog {
                    "id" => CommandOutput {
                        exit_code: 0,
                        std_out: "testuser incus-admin kvm render video\n".to_string(),
                        std_err: String::new(),
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
            },
            |_| true,
        );

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
        let runner = FnRunner::new(
            |prog: &str, args: &[&str]| match prog {
                "id" => CommandOutput {
                    exit_code: 0,
                    std_out: "testuser wheel\n".to_string(),
                    std_err: String::new(),
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
            },
            |_| false,
        );

        let checks = run_health_checks(true, &runner, "testuser", "# empty rules", "/tmp");
        assert_eq!(checks.len(), 8);
        assert!(
            !checks[0].ok
                && checks[0].needs_root
                && checks[0].fix_command.contains("sudo usermod -aG")
        );
        assert!(
            !checks[1].ok
                && checks[1].needs_root
                && checks[1].fix_command.contains("sudo setfacl")
                && checks[1].detail.contains("Missing R/W permission")
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
            FnRunner::new(
                move |prog: &str, args: &[&str]| {
                    match prog {
                    "id" => CommandOutput {
                        exit_code: 0,
                        std_out: "testuser incus-admin kvm render video\n".to_string(),
                        std_err: String::new(),
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
                },
                |_| true,
            )
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

    #[test]
    fn failure_combinations_daemon_down_socket_denied_and_no_kvm() {
        // 1. Daemon down, socket writable = true, groups ok -> daemon & dependent checks report status, no panic.
        let daemon_down = FnRunner::new(
            |prog: &str, args: &[&str]| {
                match prog {
                "id" => CommandOutput {
                    exit_code: 0,
                    std_out: "testuser incus-admin kvm render video\n".to_string(),
                    std_err: String::new(),
                },
                "systemctl" => CommandOutput {
                    exit_code: 3,
                    std_out: "inactive\n".to_string(),
                    std_err: String::new(),
                },
                "findmnt" => CommandOutput {
                    exit_code: 0,
                    std_out: "/ ext4 rw\n".to_string(),
                    std_err: String::new(),
                },
                "incus" if args.first() == Some(&"list") => CommandOutput {
                    exit_code: 1,
                    std_out: String::new(),
                    std_err: "Error: Get \"http://unix.socket/1.0\": dial unix /var/lib/incus/unix.socket: connect: connection refused".to_string(),
                },
                _ => CommandOutput {
                    exit_code: 1,
                    std_out: String::new(),
                    std_err: "daemon unreachable".to_string(),
                },
            }
            },
            |_| true,
        );
        let res_down = run_health_checks(false, &daemon_down, "testuser", "", "/tmp");
        assert_eq!(res_down.len(), 8);
        assert!(res_down[0].ok); // groups
        assert!(res_down[1].ok); // socket_acl
        assert!(!res_down[2].ok && res_down[2].needs_root); // incus_daemon
        assert!(!res_down[3].ok && res_down[3].needs_root); // bridge_incusbr0
        assert!(!res_down[4].ok && res_down[4].needs_root); // ai_gui_profile
        assert!(res_down[7].ok); // pool_capacity degrades to "Could not query"

        // 2. Socket permission denied while daemon check also fails -> socket_acl fails with setfacl fix.
        let no_socket = FnRunner::new(
            |prog: &str, _args: &[&str]| match prog {
                "id" => CommandOutput {
                    exit_code: 0,
                    std_out: "testuser incus-admin kvm render video\n".to_string(),
                    std_err: String::new(),
                },
                _ => CommandOutput {
                    exit_code: 1,
                    std_out: String::new(),
                    std_err: "permission denied".to_string(),
                },
            },
            |path| {
                assert_eq!(path, "/var/lib/incus/unix.socket");
                false
            },
        );
        let res_sock = run_health_checks(false, &no_socket, "alice", "", "/tmp");
        assert_eq!(res_sock[1].id, "socket_acl");
        assert!(!res_sock[1].ok);
        assert!(res_sock[1].needs_root);
        assert_eq!(
            res_sock[1].fix_command,
            "sudo setfacl -m u:alice:rw /var/lib/incus/unix.socket"
        );

        // 3. Machine without KVM/GPU: `incus profile device add` fails, profile lacks kvm/gpu,
        //    all checks still return structured status without fatal error.
        let no_kvm_gpu = FnRunner::new(
            |prog: &str, args: &[&str]| {
                match prog {
                "id" => CommandOutput {
                    exit_code: 0,
                    std_out: "testuser incus-admin render video\n".to_string(),
                    std_err: String::new(),
                },
                "systemctl" => CommandOutput {
                    exit_code: 3,
                    std_out: "inactive\n".to_string(),
                    std_err: String::new(),
                },
                "findmnt" => CommandOutput {
                    exit_code: 0,
                    std_out: "/home ext4 rw\n".to_string(),
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
                        std_out: "name: incusbr0\n".to_string(),
                        std_err: String::new(),
                    },
                    (Some("profile"), Some("show")) => CommandOutput {
                        exit_code: 0,
                        std_out: "config:\n  security.nesting: \"true\"\ndevices: {}\n"
                            .to_string(),
                        std_err: String::new(),
                    },
                    (Some("profile"), Some("device")) => CommandOutput {
                        exit_code: 1,
                        std_out: String::new(),
                        std_err: "Error: /dev/kvm not found".to_string(),
                    },
                    (Some("storage"), Some("info")) => CommandOutput {
                        exit_code: 0,
                        std_out: "info:\n  driver: btrfs\n  name: default\n  space used: \"10\"\n  total space: \"100\"\n".to_string(),
                        std_err: String::new(),
                    },
                    _ => CommandOutput {
                        exit_code: 0,
                        ..Default::default()
                    },
                },
                _ => CommandOutput::default(),
            }
            },
            |_| true,
        );
        let res_no_kvm = run_health_checks(true, &no_kvm_gpu, "testuser", "", "/tmp");
        assert_eq!(res_no_kvm.len(), 8);
        assert!(!res_no_kvm[0].ok && res_no_kvm[0].detail.contains("kvm"));
        assert!(!res_no_kvm[4].ok);
        assert!(res_no_kvm[4].detail.contains("gpu"));
        assert!(res_no_kvm[4].detail.contains("kvm"));
        assert!(!res_no_kvm[4].repaired);
    }

    #[test]
    fn health_check_cache_respects_static_and_dynamic_cadence() {
        let calls: RefCell<Vec<String>> = RefCell::new(Vec::new());
        let pool_used_pct = RefCell::new("20");
        let runner = FnRunner::new(
            |prog: &str, args: &[&str]| {
                calls
                    .borrow_mut()
                    .push(format!("{prog} {}", args.join(" ")));
                match prog {
                    "id" => CommandOutput {
                        exit_code: 0,
                        std_out: "testuser incus-admin kvm render video\n".to_string(),
                        std_err: String::new(),
                    },
                    "systemctl" => CommandOutput {
                        exit_code: 3,
                        std_out: "inactive\n".to_string(),
                        std_err: String::new(),
                    },
                    "findmnt" => CommandOutput {
                        exit_code: 0,
                        std_out: "/home btrfs rw,user_subvol_rm_allowed\n".to_string(),
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
                            std_out: "name: incusbr0\n".to_string(),
                            std_err: String::new(),
                        },
                        (Some("profile"), Some("show")) => CommandOutput {
                            exit_code: 0,
                            std_out: "config:\n  security.nesting: \"true\"\ndevices:\n  gpu:\n    type: gpu\n  kvm:\n    path: /dev/kvm\n".to_string(),
                            std_err: String::new(),
                        },
                        (Some("storage"), Some("info")) => CommandOutput {
                            exit_code: 0,
                            std_out: format!(
                                "info:\n  driver: btrfs\n  name: default\n  space used: \"{}\"\n  total space: \"100\"\n",
                                *pool_used_pct.borrow()
                            ),
                            std_err: String::new(),
                        },
                        _ => CommandOutput::default(),
                    },
                    _ => CommandOutput::default(),
                }
            },
            |_| {
                calls.borrow_mut().push("socket_writable".to_string());
                true
            },
        );

        let mut cache = HealthCheckCache::new();
        let initial = cache.run_with_cadence(
            CheckRunMode::StartupOrManual,
            false,
            1_000,
            &runner,
            "testuser",
            "",
            "/tmp",
        );
        assert_eq!(initial.len(), 8);
        assert_eq!(cache.last_static_run_ms, Some(1_000));
        assert_eq!(cache.last_dynamic_run_ms, Some(1_000));
        assert_eq!(cache.get("groups").unwrap().checked_at_ms, 1_000);
        assert_eq!(cache.get("pool_capacity").unwrap().checked_at_ms, 1_000);
        assert!(calls.borrow().iter().any(|c| c.starts_with("id ")));
        assert!(calls.borrow().iter().any(|c| c == "socket_writable"));

        // Poll run: should only invoke dynamic checks (`incus list`, `incus storage info`).
        calls.borrow_mut().clear();
        *pool_used_pct.borrow_mut() = "90";
        let polled = cache.run_with_cadence(
            CheckRunMode::Poll,
            false,
            5_000,
            &runner,
            "testuser",
            "",
            "/tmp",
        );
        assert_eq!(polled.len(), 8);
        assert!(polled[7].warning);
        assert_eq!(cache.last_static_run_ms, Some(1_000));
        assert_eq!(cache.last_dynamic_run_ms, Some(5_000));
        assert_eq!(cache.get("groups").unwrap().checked_at_ms, 1_000);
        assert_eq!(cache.get("pool_capacity").unwrap().checked_at_ms, 5_000);
        assert!(!calls.borrow().iter().any(|c| c.starts_with("id ")));
        assert!(!calls.borrow().iter().any(|c| c == "socket_writable"));
        assert!(!calls.borrow().iter().any(|c| c.starts_with("findmnt ")));
    }
}
