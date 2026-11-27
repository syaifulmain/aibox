//! Device passthrough and `ai-gui` profile (nesting, GPU, KVM) health checks.

use crate::health::checks::HealthCheckResult;
use crate::process::CommandRunner;

fn inspect_ai_gui_profile_with_stdout(
    runner: &dyn CommandRunner,
) -> (bool, String, Option<String>) {
    let out = runner.run("incus", &["profile", "show", "ai-gui"]);
    if out.exit_code != 0 {
        return (false, "Profile ai-gui does not exist.".to_string(), None);
    }
    let txt = out.std_out;
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
            Some(txt),
        );
    }
    (
        true,
        "Profile ai-gui configured with nesting, gpu, and kvm.".to_string(),
        Some(txt),
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
    let fix_cmd = "incus profile create ai-gui; incus profile set ai-gui security.nesting=true; incus profile device add ai-gui gpu gpu; incus profile device add ai-gui kvm unix-char path=/dev/kvm";
    if !daemon_reachable {
        r.needs_root = true;
        r.detail = "Profile ai-gui does not exist.".to_string();
        r.fix_command = fix_cmd.to_string();
        return r;
    }

    let (ok_now, mut detail_now, initial_stdout) = inspect_ai_gui_profile_with_stdout(runner);
    if ok_now {
        r.ok = true;
        r.detail = detail_now;
        return r;
    }

    if fix {
        if initial_stdout.is_none() {
            runner.run("incus", &["profile", "create", "ai-gui"]);
        }
        let needs_nesting = initial_stdout
            .as_deref()
            .is_none_or(|txt| !txt.contains("security.nesting"));
        if needs_nesting {
            runner.run(
                "incus",
                &["profile", "set", "ai-gui", "security.nesting=true"],
            );
        }
        let needs_gpu = initial_stdout
            .as_deref()
            .is_none_or(|txt| !txt.contains("gpu:"));
        if needs_gpu {
            runner.run(
                "incus",
                &["profile", "device", "add", "ai-gui", "gpu", "gpu"],
            );
        }
        let needs_kvm = initial_stdout
            .as_deref()
            .is_none_or(|txt| !txt.contains("kvm:") && !txt.contains("/dev/kvm"));
        if needs_kvm {
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
        let (ok_after, detail_after, _) = inspect_ai_gui_profile_with_stdout(runner);
        if ok_after {
            r.ok = true;
            r.repaired = true;
            r.detail = "Repaired ai-gui profile automatically.".to_string();
            return r;
        }
        detail_now = detail_after;
    }

    r.needs_root = false;
    r.detail = detail_now;
    r.fix_command = fix_cmd.to_string();
    r
}
