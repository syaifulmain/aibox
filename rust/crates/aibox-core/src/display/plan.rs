//! Desktop integration plan and the shell snippets that apply it.

use crate::display::host::HostSession;
use crate::util::shell::shell_quote;

/// Per-environment desktop integration switch (`user.aibox.desktop`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopMode {
    Off,
    On,
}

impl DesktopMode {
    /// Parses the stored config value; empty or unknown values default to On.
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "off" | "false" | "0" => Self::Off,
            _ => Self::On,
        }
    }

    /// Stable value written to `user.aibox.desktop`.
    pub fn config_value(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::On => "on",
        }
    }

    pub fn from_enabled(enabled: bool) -> Self {
        if enabled {
            Self::On
        } else {
            Self::Off
        }
    }
}

/// One Incus device that the display bridge wants on the instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceSpec {
    pub name: String,
    pub kind: String,
    pub options: Vec<(String, String)>,
}

/// Everything needed to expose the host desktop inside one environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayPlan {
    pub mode: DesktopMode,
    pub devices: Vec<DeviceSpec>,
    pub env: Vec<(String, String)>,
    pub warnings: Vec<String>,
}

impl DisplayPlan {
    pub fn has_x11(&self) -> bool {
        self.devices.iter().any(|device| device.name == "x11")
    }
}

/// Builds the display plan for one environment.
///
/// Module 1.1 only implements the safe X11 proxy. Wayland is added by a later
/// module, so the Wayland fields on `HostSession` are carried but unused.
pub fn build_desktop_plan(mode: DesktopMode, host: &HostSession) -> DisplayPlan {
    let mut plan = DisplayPlan {
        mode,
        devices: Vec::new(),
        env: Vec::new(),
        warnings: Vec::new(),
    };

    if mode == DesktopMode::Off {
        return plan;
    }

    if host.x11_socket.is_empty() {
        plan.warnings.push(
            "Desktop integration is on, but no X11 socket is available on the host.".to_string(),
        );
        return plan;
    }

    // A proxy device is required: the host-side Incus proxy connects as root,
    // so `xhost +si:localuser:root` authorizes the container without opening
    // the X server to every local process the way `xhost +local:` does.
    plan.devices.push(DeviceSpec {
        name: "x11".to_string(),
        kind: "proxy".to_string(),
        options: vec![
            ("connect".to_string(), format!("unix:{}", host.x11_socket)),
            ("listen".to_string(), format!("unix:{}", host.x11_socket)),
            ("bind".to_string(), "container".to_string()),
        ],
    });
    plan.env
        .push(("DISPLAY".to_string(), host.x11_display.clone()));
    plan
}

fn device_options(options: &[(String, String)]) -> String {
    options
        .iter()
        .map(|(key, value)| format!("{key}={}", shell_quote(value)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn remove_device_lines(name: &str, device_names: &[&str]) -> String {
    let q_name = shell_quote(name);
    device_names
        .iter()
        .map(|device| {
            format!(
                "incus config device remove {q_name} {} >/dev/null 2>&1 || true",
                shell_quote(device)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

const XHOST_ROOT_SNIPPET: &str = "if command -v xhost >/dev/null 2>&1; then\n    xhost +si:localuser:root >/dev/null 2>&1 || true\nfi";

/// Returns an idempotent snippet that installs the plan's devices.
///
/// The container must already be running. Existing devices are removed first so
/// a changed host socket (for example after a logout/login) is refreshed.
pub fn apply_devices_script(name: &str, plan: &DisplayPlan) -> String {
    if plan.devices.is_empty() {
        return String::new();
    }
    let q_name = shell_quote(name);
    let mut lines = Vec::new();
    if plan.has_x11() {
        lines.push(XHOST_ROOT_SNIPPET.to_string());
    }
    for device in &plan.devices {
        lines.push(format!(
            "incus config device remove {q_name} {} >/dev/null 2>&1 || true",
            shell_quote(&device.name)
        ));
        let options = device_options(&device.options);
        let add = if options.is_empty() {
            format!(
                "incus config device add {q_name} {} {}",
                shell_quote(&device.name),
                shell_quote(&device.kind)
            )
        } else {
            format!(
                "incus config device add {q_name} {} {} {options}",
                shell_quote(&device.name),
                shell_quote(&device.kind)
            )
        };
        lines.push(add);
    }
    lines.join("\n")
}

/// Removes the plan's devices; shared `xhost` grants are intentionally kept
/// because other environments may still rely on them.
pub fn remove_devices_script(name: &str, plan: &DisplayPlan) -> String {
    if plan.devices.is_empty() {
        return String::new();
    }
    let names: Vec<&str> = plan
        .devices
        .iter()
        .map(|device| device.name.as_str())
        .collect();
    remove_device_lines(name, &names)
}

/// Writes the mode and applies or removes the matching desktop devices.
pub fn set_desktop_mode_script(name: &str, mode: DesktopMode, host: &HostSession) -> String {
    let q_name = shell_quote(name);
    let mut lines = vec![
        "set -euo pipefail".to_string(),
        format!(
            "incus config set {q_name} user.aibox.desktop {}",
            shell_quote(mode.config_value())
        ),
    ];
    match mode {
        DesktopMode::On => {
            let apply = apply_devices_script(name, &build_desktop_plan(mode, host));
            if !apply.is_empty() {
                lines.push(apply);
            }
        }
        // Off means headless: remove every bridge device even though an Off
        // plan has no devices of its own.
        DesktopMode::Off => lines.push(remove_device_lines(name, &["x11", "wayland"])),
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host_with_x11() -> HostSession {
        HostSession {
            uid: 1000,
            gid: 1000,
            runtime_dir: "/run/user/1000".to_string(),
            x11_display: ":0".to_string(),
            x11_socket: "/tmp/.X11-unix/X0".to_string(),
            wayland_display: "wayland-0".to_string(),
            wayland_socket: "/run/user/1000/wayland-0".to_string(),
        }
    }

    fn host_without_x11() -> HostSession {
        HostSession {
            x11_display: String::new(),
            x11_socket: String::new(),
            ..host_with_x11()
        }
    }

    #[test]
    fn desktop_mode_parses_stable_values() {
        assert_eq!(DesktopMode::parse("off"), DesktopMode::Off);
        assert_eq!(DesktopMode::parse("false"), DesktopMode::Off);
        assert_eq!(DesktopMode::parse("0"), DesktopMode::Off);
        assert_eq!(DesktopMode::parse("on"), DesktopMode::On);
        assert_eq!(DesktopMode::parse("true"), DesktopMode::On);
        assert_eq!(DesktopMode::parse("1"), DesktopMode::On);
        assert_eq!(DesktopMode::parse(""), DesktopMode::On);
        assert_eq!(DesktopMode::Off.config_value(), "off");
        assert_eq!(DesktopMode::On.config_value(), "on");
    }

    #[test]
    fn on_with_x11_produces_a_proxy_device_and_display_env() {
        let plan = build_desktop_plan(DesktopMode::On, &host_with_x11());
        assert_eq!(plan.devices.len(), 1);
        let device = &plan.devices[0];
        assert_eq!(device.name, "x11");
        assert_eq!(device.kind, "proxy");
        assert_eq!(
            device.options,
            vec![
                ("connect".to_string(), "unix:/tmp/.X11-unix/X0".to_string()),
                ("listen".to_string(), "unix:/tmp/.X11-unix/X0".to_string()),
                ("bind".to_string(), "container".to_string()),
            ]
        );
        assert_eq!(plan.env, vec![("DISPLAY".to_string(), ":0".to_string())]);
        assert!(plan.warnings.is_empty());
    }

    #[test]
    fn off_is_pure_headless() {
        let plan = build_desktop_plan(DesktopMode::Off, &host_with_x11());
        assert!(plan.devices.is_empty());
        assert!(plan.env.is_empty());
        assert!(plan.warnings.is_empty());
        assert_eq!(apply_devices_script("box", &plan), "");
        assert_eq!(remove_devices_script("box", &plan), "");
    }

    #[test]
    fn on_without_x11_warns_and_adds_nothing() {
        let plan = build_desktop_plan(DesktopMode::On, &host_without_x11());
        assert!(plan.devices.is_empty());
        assert!(plan.env.is_empty());
        assert_eq!(plan.warnings.len(), 1);
        assert!(plan.warnings[0].contains("X11"));
    }

    #[test]
    fn apply_script_removes_then_adds_and_authorizes_only_root() {
        let plan = build_desktop_plan(DesktopMode::On, &host_with_x11());
        let script = apply_devices_script("box", &plan);
        let remove_at = script.find("incus config device remove").expect("remove");
        let add_at = script.find("incus config device add").expect("add");
        assert!(remove_at < add_at);
        assert!(script.contains("xhost +si:localuser:root"));
        assert!(!script.contains("xhost +local:"));
        assert!(script.contains("connect='unix:/tmp/.X11-unix/X0'"));
        assert!(script.contains("listen='unix:/tmp/.X11-unix/X0'"));
        assert!(script.contains("bind='container'"));
    }

    #[test]
    fn scripts_quote_names_and_paths() {
        let plan = build_desktop_plan(DesktopMode::On, &host_with_x11());
        let script = apply_devices_script("it's a box; rm -rf /", &plan);
        assert!(script.contains("'it'\\''s a box; rm -rf /'"));
        assert!(!script.contains("; rm -rf /\n"));
    }

    #[test]
    fn remove_script_does_not_revoke_xhost() {
        let plan = build_desktop_plan(DesktopMode::On, &host_with_x11());
        let script = remove_devices_script("box", &plan);
        assert!(script.contains("incus config device remove 'box' 'x11'"));
        assert!(!script.contains("xhost"));
    }

    #[test]
    fn set_mode_script_writes_config_then_applies_or_removes() {
        let on = set_desktop_mode_script("box", DesktopMode::On, &host_with_x11());
        assert!(on.contains("incus config set 'box' user.aibox.desktop 'on'"));
        assert!(on.contains("incus config device add 'box' 'x11' 'proxy'"));

        let off = set_desktop_mode_script("box", DesktopMode::Off, &host_with_x11());
        assert!(off.contains("incus config set 'box' user.aibox.desktop 'off'"));
        assert!(off.contains("incus config device remove 'box' 'x11'"));
        assert!(off.contains("incus config device remove 'box' 'wayland'"));
        assert!(!off.contains("xhost +local:"));
    }
}
