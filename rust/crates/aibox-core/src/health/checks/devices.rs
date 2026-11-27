//! Device passthrough and `ai-gui` profile (nesting, GPU, KVM) health checks.

use crate::display::gpu::{detect_gpu, GpuInfo};
use crate::health::checks::HealthCheckResult;
use crate::process::CommandRunner;

const GPU_PACKAGE_HINT: &str = "Install mesa, intel-media-driver, libva-utils and mesa-utils inside the environment for VA-API, WebGL and GL.";

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

/// Checks that the host exposes GPU nodes that containers can use.
///
/// A render node (`renderD*`) is required for VA-API/WebGL; card-only hosts
/// still work for basic KMS but are reported as a warning. Reading `/dev/dri`
/// and sysfs needs no root privileges, and the check never changes host state.
pub fn check_gpu_devices() -> HealthCheckResult {
    check_gpu_devices_with(&detect_gpu())
}

/// Pure variant of [`check_gpu_devices`] so tests do not depend on the host.
pub fn check_gpu_devices_with(gpu: &GpuInfo) -> HealthCheckResult {
    let mut r = HealthCheckResult {
        id: "gpu_devices".to_string(),
        title: "GPU render nodes (/dev/dri)".to_string(),
        fix_command: GPU_PACKAGE_HINT.to_string(),
        ..Default::default()
    };

    let render = gpu.render_nodes.join(", ");
    let cards = gpu.card_nodes.join(", ");

    if gpu.render_nodes.is_empty() && gpu.card_nodes.is_empty() {
        r.detail = format!(
            "{} is missing; no GPU nodes are available to containers. Check the host GPU driver and enable the ai-gui profile (gpu device). {GPU_PACKAGE_HINT}",
            gpu.dri_dir
        );
        r.fix_command = format!(
            "Check the host GPU driver and add the ai-gui gpu device (incus profile device add ai-gui gpu gpu). {GPU_PACKAGE_HINT}"
        );
        return r;
    }

    if gpu.render_nodes.is_empty() {
        r.ok = true;
        r.warning = true;
        r.detail = format!(
            "Only card nodes are present ({cards}); no renderD* node was found. The ai-gui profile exposes /dev/dri with mode=0666, so container root can access them, but VA-API/WebGL need a render node. {GPU_PACKAGE_HINT}"
        );
        return r;
    }

    let vendor = if gpu.intel {
        " Intel GPU detected."
    } else {
        ""
    };
    let cards_part = if cards.is_empty() {
        String::new()
    } else {
        format!(" Card nodes: {cards}.")
    };
    r.ok = true;
    r.detail = format!(
        "Render nodes: {render}.{cards_part}{vendor} The ai-gui profile exposes /dev/dri with mode=0666, so container root can access them. {GPU_PACKAGE_HINT}"
    );
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu(render: &[&str], cards: &[&str], intel: bool) -> GpuInfo {
        GpuInfo {
            dri_dir: "/dev/dri".to_string(),
            render_nodes: render.iter().map(|node| node.to_string()).collect(),
            card_nodes: cards.iter().map(|node| node.to_string()).collect(),
            intel,
        }
    }

    #[test]
    fn render_nodes_pass_with_the_container_mode_and_install_hint() {
        let r = check_gpu_devices_with(&gpu(&["renderD128"], &["card1"], true));
        assert_eq!(r.id, "gpu_devices");
        assert_eq!(r.title, "GPU render nodes (/dev/dri)");
        assert!(r.ok && !r.warning && !r.needs_root && !r.repaired);
        assert!(r.detail.contains("renderD128"));
        assert!(r.detail.contains("card1"));
        assert!(r.detail.contains("Intel GPU detected"));
        assert!(r.detail.contains("mode=0666"));
        for package in ["mesa", "intel-media-driver", "libva-utils", "mesa-utils"] {
            assert!(r.detail.contains(package), "hint missing {package}");
        }
    }

    #[test]
    fn card_only_hosts_warn_but_do_not_fail() {
        let r = check_gpu_devices_with(&gpu(&[], &["card0", "card1"], false));
        assert!(r.ok);
        assert!(r.warning);
        assert!(r.detail.contains("card0, card1"));
        assert!(r.detail.contains("no renderD* node"));
        assert!(r.detail.contains("mode=0666"));
    }

    #[test]
    fn a_missing_dri_directory_fails_with_a_non_destructive_hint() {
        let r = check_gpu_devices_with(&GpuInfo {
            dri_dir: "/dev/dri".to_string(),
            ..Default::default()
        });
        assert!(!r.ok && !r.warning && !r.needs_root);
        assert!(r.detail.contains("/dev/dri is missing"));
        assert!(r.fix_command.contains("ai-gui"));
        assert!(r.fix_command.contains("mesa-utils"));
    }
}
