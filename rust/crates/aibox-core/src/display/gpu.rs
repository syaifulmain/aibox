//! Host GPU passthrough details for containers.
//!
//! The `ai-gui` profile already exposes `/dev/dri` to containers, but nothing
//! checks whether the host really has render nodes or tells the user which
//! packages make VA-API/WebGL work inside the environment. This module keeps
//! that knowledge in one place: detection, optional driver environment
//! passthrough, and the in-container diagnostic script used by the GUI and CLI.
//!
//! VMs are intentionally out of scope: they use guest drivers and do not see
//! the host card, so the GUI hides the GPU card for them.

use std::env;
use std::fs;
use std::path::Path;

use crate::util::shell::shell_quote;

/// Host `/dev/dri` nodes and the GPU vendor, detected once per operation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GpuInfo {
    /// Directory that was inspected, usually `/dev/dri`.
    pub dri_dir: String,
    /// `renderD*` nodes used by VA-API, WebGL, and compute.
    pub render_nodes: Vec<String>,
    /// `card*` nodes used by KMS and some legacy GL paths.
    pub card_nodes: Vec<String>,
    /// True when any `/sys/class/drm/card*/device/vendor` reports Intel (`0x8086`).
    pub intel: bool,
}

impl GpuInfo {
    /// True when at least one render node lets containers reach the GPU.
    pub fn has_render_node(&self) -> bool {
        !self.render_nodes.is_empty()
    }
}

/// Inspects the live host.
pub fn detect_gpu() -> GpuInfo {
    detect_gpu_with(Path::new("/dev/dri"))
}

/// Inspects `dri_dir` and the standard sysfs DRM tree.
///
/// Missing directories or unreadable vendor files are treated as "absent"
/// instead of failing, because GPU-less hosts still need a structured health
/// result.
pub fn detect_gpu_with(dri_dir: &Path) -> GpuInfo {
    detect_gpu_at(dri_dir, Path::new("/sys/class/drm"))
}

/// Testable detector that takes both the device tree and the sysfs DRM tree.
pub fn detect_gpu_at(dri_dir: &Path, sys_drm_dir: &Path) -> GpuInfo {
    let (render_nodes, card_nodes) = read_device_nodes(dri_dir);
    GpuInfo {
        dri_dir: dri_dir.to_string_lossy().into_owned(),
        render_nodes,
        card_nodes,
        intel: detect_intel(sys_drm_dir),
    }
}

/// Collects `renderD*` and `card*` entries, sorted for stable output.
fn read_device_nodes(dri_dir: &Path) -> (Vec<String>, Vec<String>) {
    let Ok(entries) = fs::read_dir(dri_dir) else {
        return (Vec::new(), Vec::new());
    };
    let mut render_nodes = Vec::new();
    let mut card_nodes = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_numbered_node(&name, "renderD") {
            render_nodes.push(name);
        } else if is_numbered_node(&name, "card") {
            card_nodes.push(name);
        }
    }
    render_nodes.sort();
    card_nodes.sort();
    (render_nodes, card_nodes)
}

/// Matches `prefix` followed by digits only, so connector entries such as
/// `card0-DP-1` are not mistaken for GPU nodes.
fn is_numbered_node(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix)
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_digit()))
}

fn detect_intel(sys_drm_dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(sys_drm_dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("card") {
            continue;
        }
        let vendor_path = entry.path().join("device").join("vendor");
        let Ok(vendor) = fs::read_to_string(vendor_path) else {
            continue;
        };
        if vendor.trim().to_ascii_lowercase().contains("0x8086") {
            return true;
        }
    }
    false
}

/// Environment values worth forwarding from the host to the container.
///
/// The driver name is only forwarded when the host sets it explicitly; AIBox
/// never picks an Intel/AMD/NVIDIA driver itself, so `vainfo` can still detect
/// the right backend on its own.
pub fn gpu_env() -> Vec<(String, String)> {
    gpu_env_with(|key| env::var(key).ok().filter(|value| !value.trim().is_empty()))
}

/// Pure variant of [`gpu_env`] for tests.
pub fn gpu_env_with(get: impl Fn(&str) -> Option<String>) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    if let Some(driver) = get("LIBVA_DRIVER_NAME").map(|value| value.trim().to_string()) {
        if !driver.is_empty() {
            pairs.push(("LIBVA_DRIVER_NAME".to_string(), driver));
        }
    }
    pairs
}

/// Builds the in-container GPU diagnostic used by the GUI Test GPU button.
///
/// The environment mirrors what Enter/exec pass to graphical programs
/// (`DISPLAY`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`, ...), so `vainfo` and
/// `glxinfo` probe the same session the app would use. Every probe is optional:
/// a missing tool prints an install hint instead of aborting the whole test.
pub fn gpu_test_script(name: &str, env: &[(String, String)]) -> String {
    let mut cmd = format!("incus exec {}", shell_quote(name));
    for (key, value) in env {
        cmd.push_str(&format!(
            " --env {}",
            shell_quote(&format!("{key}={value}"))
        ));
    }
    cmd.push_str(&format!(" -- sh -c {}", shell_quote(&probe_script())));
    cmd
}

fn probe_script() -> String {
    [
        "echo '== /dev/dri =='",
        "ls -l /dev/dri 2>&1 || echo '/dev/dri is missing; enable the ai-gui GPU device'",
        "echo '== VA-API =='",
        "if command -v vainfo >/dev/null 2>&1; then vainfo 2>&1; else echo 'vainfo is not installed (install libva-utils)'; fi",
        "echo '== OpenGL =='",
        "if command -v glxinfo >/dev/null 2>&1; then glxinfo -B 2>&1; else echo 'glxinfo is not installed (install mesa-utils)'; fi",
        "echo \"XDG_SESSION_TYPE=${XDG_SESSION_TYPE:-unset}\"",
        "echo \"LIBVA_DRIVER_NAME=${LIBVA_DRIVER_NAME:-unset}\"",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempTree(PathBuf);

    impl TempTree {
        fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default();
            let path =
                env::temp_dir().join(format!("aibox-gpu-{prefix}-{}-{nanos}", std::process::id()));
            fs::create_dir_all(&path).expect("create temp dir");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, content).expect("write fixture");
    }

    #[test]
    fn detects_render_and_card_nodes_and_ignores_connectors() {
        let dri = TempTree::new("dri");
        for name in ["card0", "card1", "renderD128", "by-path", "card0-DP-1"] {
            write_file(&dri.path().join(name), "");
        }
        let sys = TempTree::new("sys");
        write_file(&sys.path().join("card1/device/vendor"), "0x8086\n");

        let gpu = detect_gpu_at(dri.path(), sys.path());
        assert_eq!(gpu.render_nodes, vec!["renderD128"]);
        assert_eq!(gpu.card_nodes, vec!["card0", "card1"]);
        assert!(gpu.intel);
        assert!(gpu.has_render_node());
        assert_eq!(gpu.dri_dir, dri.path().to_string_lossy());
    }

    #[test]
    fn missing_directories_are_tolerated() {
        let missing = env::temp_dir().join("aibox-gpu-does-not-exist");
        let gpu = detect_gpu_at(&missing, &missing);
        assert!(gpu.render_nodes.is_empty());
        assert!(gpu.card_nodes.is_empty());
        assert!(!gpu.intel);
        assert!(!gpu.has_render_node());
        assert!(gpu.dri_dir.ends_with("aibox-gpu-does-not-exist"));
    }

    #[test]
    fn detect_gpu_with_uses_the_live_sysfs_tree_without_failing() {
        let missing = env::temp_dir().join("aibox-gpu-with-missing");
        let gpu = detect_gpu_with(&missing);
        assert!(gpu.render_nodes.is_empty());
        assert!(gpu.card_nodes.is_empty());
        assert_eq!(gpu.dri_dir, missing.to_string_lossy().to_string());
    }

    #[test]
    fn intel_detection_skips_unreadable_vendor_files_and_other_vendors() {
        let dri = TempTree::new("dri-empty");
        let sys = TempTree::new("sys-mixed");
        fs::create_dir_all(sys.path().join("card0/device")).expect("card0");
        write_file(&sys.path().join("card1/device/vendor"), "0x1002\n");
        write_file(&sys.path().join("card2/device/vendor"), "0x10de\n");
        assert!(!detect_gpu_at(dri.path(), sys.path()).intel);

        write_file(&sys.path().join("card0/device/vendor"), "0x8086\n");
        assert!(detect_gpu_at(dri.path(), sys.path()).intel);
    }

    #[test]
    fn gpu_env_forwards_only_an_explicit_libva_driver() {
        let forwarded = gpu_env_with(|key| (key == "LIBVA_DRIVER_NAME").then(|| "iHD".to_string()));
        assert_eq!(
            forwarded,
            vec![("LIBVA_DRIVER_NAME".to_string(), "iHD".to_string())]
        );

        assert!(gpu_env_with(|_| None).is_empty());
        assert!(gpu_env_with(|_| Some("   ".to_string())).is_empty());
    }

    #[test]
    fn test_script_quotes_the_name_and_environment_and_never_aborts() {
        let env = vec![
            ("DISPLAY".to_string(), ":0".to_string()),
            ("XDG_RUNTIME_DIR".to_string(), "/run/user/1000".to_string()),
        ];
        let script = gpu_test_script("it's a box; rm -rf /", &env);
        assert!(script.contains("incus exec 'it'\\''s a box; rm -rf /'"));
        assert!(script.contains("--env 'DISPLAY=:0'"));
        assert!(script.contains("--env 'XDG_RUNTIME_DIR=/run/user/1000'"));
        assert!(script.contains("-- sh -c"));
        assert!(script.contains("ls -l /dev/dri"));
        assert!(script.contains("vainfo is not installed (install libva-utils)"));
        assert!(script.contains("glxinfo is not installed (install mesa-utils)"));
        assert!(!script.contains("set -e"));
        assert!(!script.contains("; rm -rf /\n"));
    }

    #[test]
    fn generated_scripts_pass_bash_syntax_check() {
        let scripts = [
            gpu_test_script("box", &[]),
            gpu_test_script(
                "box",
                &[
                    ("DISPLAY".to_string(), ":0".to_string()),
                    ("LIBVA_DRIVER_NAME".to_string(), "iHD".to_string()),
                    ("WAYLAND_DISPLAY".to_string(), "wayland-0".to_string()),
                ],
            ),
        ];
        for script in scripts {
            let status = Command::new("bash")
                .arg("-n")
                .arg("-c")
                .arg(&script)
                .status()
                .expect("run bash -n");
            assert!(status.success(), "bash -n failed:\n{script}");
        }
    }
}
