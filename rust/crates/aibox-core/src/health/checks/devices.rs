//! Device passthrough and `ai-gui` profile (nesting, GPU, KVM) health checks.

use crate::health::checks::HealthCheckResult;
use crate::process::CommandRunner;

/// Inspects `incus profile show ai-gui` for `security.nesting`, `gpu`, and `kvm` devices.
pub(crate) fn inspect_ai_gui_profile(runner: &dyn CommandRunner) -> (bool, String) {
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

/// Checks the `ai-gui` profile (nesting, GPU, KVM), optionally repairing it when `fix` is true.
///
/// Hosts without `/dev/kvm` or GPU devices still receive a structured `HealthCheckResult`
/// status rather than a fatal error.
pub fn check_ai_gui_profile(
    fix: bool,
    daemon_reachable: bool,
    runner: &dyn CommandRunner,
) -> HealthCheckResult {
    let mut r = HealthCheckResult {
        id: "ai_gui_profile".to_string(),
        title: "ai-gui profile (nesting, gpu, kvm)".to_string(),
        ..Default::default()
    };
    let (ok_now, mut detail_now) = inspect_ai_gui_profile(runner);
    if ok_now {
        r.ok = true;
        r.detail = detail_now;
        return r;
    }

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
            return r;
        }
        detail_now = detail_after;
    }

    r.needs_root = !daemon_reachable;
    r.detail = detail_now;
    r.fix_command = "incus profile create ai-gui; incus profile set ai-gui security.nesting=true; incus profile device add ai-gui gpu gpu; incus profile device add ai-gui kvm unix-char path=/dev/kvm".to_string();
    r
}
