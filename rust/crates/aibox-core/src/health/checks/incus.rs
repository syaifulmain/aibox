//! Incus daemon, socket ACL, bridge network, and storage pool health checks.

use crate::health::checks::HealthCheckResult;
use crate::health::pool::{parse_incus_storage_info, sum_root_disk_quotas_bytes, StoragePoolInfo};
use crate::process::CommandRunner;
use crate::util::format::format_bytes;

const INCUS_SOCKET_PATH: &str = "/var/lib/incus/unix.socket";

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

/// Checks read/write access on `/var/lib/incus/unix.socket`.
pub fn check_socket_acl(runner: &dyn CommandRunner, user: &str) -> HealthCheckResult {
    let mut r = HealthCheckResult {
        id: "socket_acl".to_string(),
        title: "Incus socket ACL".to_string(),
        ..Default::default()
    };
    if runner.socket_writable(INCUS_SOCKET_PATH) {
        r.ok = true;
        r.detail = format!("Read/write access confirmed on {INCUS_SOCKET_PATH}.");
    } else {
        r.ok = false;
        r.needs_root = true;
        r.detail = format!("Missing R/W permission on {INCUS_SOCKET_PATH}.");
        r.fix_command = format!("sudo setfacl -m u:{user}:rw {INCUS_SOCKET_PATH}");
    }
    r
}

/// Checks Incus daemon reachability and returns `(result, reachable, cached_list_json)`.
pub fn check_incus_daemon(runner: &dyn CommandRunner) -> (HealthCheckResult, bool, String) {
    let mut r = HealthCheckResult {
        id: "incus_daemon".to_string(),
        title: "Incus daemon reachable".to_string(),
        ..Default::default()
    };
    let out = runner.run("incus", &["list", "--format=json"]);
    if out.exit_code == 0 {
        r.ok = true;
        r.detail = "Incus daemon responded cleanly.".to_string();
        (r, true, out.std_out)
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
        (r, false, String::new())
    }
}

/// Checks the managed `incusbr0` bridge network, optionally creating it when `fix` is true.
pub fn check_bridge_incusbr0(
    fix: bool,
    daemon_reachable: bool,
    runner: &dyn CommandRunner,
) -> HealthCheckResult {
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
        return r;
    }

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
            return r;
        }
    }

    r.needs_root = !daemon_reachable;
    r.detail = "Managed bridge incusbr0 is missing.".to_string();
    r.fix_command =
        "incus network create incusbr0 ipv4.address=auto ipv4.nat=true ipv6.address=none"
            .to_string();
    r
}

/// Queries `incus storage info default` and evaluates pool capacity health.
pub fn check_pool_capacity(
    daemon_reachable: bool,
    cached_list_json: &str,
    runner: &dyn CommandRunner,
) -> HealthCheckResult {
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
        sum_root_disk_quotas_bytes(cached_list_json, "", &pool_info.name);
    evaluate_pool_capacity_health(&pool_info, allocated_bytes, unbounded_count)
}
