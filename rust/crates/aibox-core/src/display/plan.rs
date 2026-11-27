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
        self.devices.iter().any(|device| device.name == X11_DEVICE)
    }

    pub fn has_wayland(&self) -> bool {
        self.devices
            .iter()
            .any(|device| device.name == WAYLAND_DEVICE)
    }
}

/// Display name exposed inside the container, regardless of the host name.
const CONTAINER_WAYLAND_DISPLAY: &str = "wayland-0";
const WAYLAND_DEVICE: &str = "wayland";
const X11_DEVICE: &str = "x11";

/// Builds the display plan for one environment.
///
/// Wayland is preferred because it provides clipboard, drag-and-drop, and HiDPI
/// scaling from the host session. X11 stays as a proxy fallback for legacy
/// toolkits, Wine, and Java applications.
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

    let has_wayland = !host.wayland_socket.is_empty();
    let has_x11 = !host.x11_socket.is_empty();

    if !has_wayland && !has_x11 {
        plan.warnings.push(
            "Desktop integration is on, but no Wayland or X11 socket is available on the host."
                .to_string(),
        );
        return plan;
    }

    if has_wayland {
        let container_runtime_dir = format!("/run/user/{}", host.uid);
        // The host proxy runs as root, so the container-side socket needs to be
        // world-accessible; `mode=0666` matches how the compositor socket is
        // exposed on the host while keeping it inside the instance runtime dir.
        plan.devices.push(DeviceSpec {
            name: WAYLAND_DEVICE.to_string(),
            kind: "proxy".to_string(),
            options: vec![
                (
                    "connect".to_string(),
                    format!("unix:{}", host.wayland_socket),
                ),
                (
                    "listen".to_string(),
                    format!("unix:{container_runtime_dir}/{CONTAINER_WAYLAND_DISPLAY}"),
                ),
                ("bind".to_string(), "container".to_string()),
                ("mode".to_string(), "0666".to_string()),
            ],
        });
        plan.env.extend([
            (
                "WAYLAND_DISPLAY".to_string(),
                CONTAINER_WAYLAND_DISPLAY.to_string(),
            ),
            ("XDG_RUNTIME_DIR".to_string(), container_runtime_dir),
            ("XDG_SESSION_TYPE".to_string(), "wayland".to_string()),
            ("MOZ_ENABLE_WAYLAND".to_string(), "1".to_string()),
            ("QT_QPA_PLATFORM".to_string(), "wayland;xcb".to_string()),
            ("GDK_BACKEND".to_string(), "wayland,x11".to_string()),
            ("SDL_VIDEODRIVER".to_string(), "wayland".to_string()),
            (
                "ELECTRON_OZONE_PLATFORM_HINT".to_string(),
                "auto".to_string(),
            ),
            ("CLUTTER_BACKEND".to_string(), "wayland".to_string()),
        ]);
        if !host.xdg_current_desktop.is_empty() {
            plan.env.push((
                "XDG_CURRENT_DESKTOP".to_string(),
                host.xdg_current_desktop.clone(),
            ));
        }
    }

    if has_x11 {
        // A proxy device is required: the host-side Incus proxy connects as
        // root, so `xhost +si:localuser:root` authorizes the container without
        // opening the X server to every local process.
        plan.devices.push(DeviceSpec {
            name: X11_DEVICE.to_string(),
            kind: "proxy".to_string(),
            options: vec![
                ("connect".to_string(), format!("unix:{}", host.x11_socket)),
                ("listen".to_string(), format!("unix:{}", host.x11_socket)),
                ("bind".to_string(), "container".to_string()),
            ],
        });
        plan.env
            .push(("DISPLAY".to_string(), host.x11_display.clone()));
    }

    if has_wayland && !has_x11 {
        plan.warnings.push(
            "Wayland is available, but no X11 socket is available on the host; the X11 fallback is unavailable."
                .to_string(),
        );
    }

    plan
}

pub(crate) fn device_options(options: &[(String, String)]) -> String {
    options
        .iter()
        .map(|(key, value)| format!("{key}={}", shell_quote(value)))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn remove_device_lines(name: &str, device_names: &[&str]) -> String {
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

/// Merges environment groups, keeping the first value for a duplicated key.
///
/// The display plan is passed first, so its `XDG_RUNTIME_DIR` (and any future
/// shared key) wins over the audio plan while both plans stay self-contained.
pub fn merge_env_pairs(groups: &[&[(String, String)]]) -> Vec<(String, String)> {
    let mut merged: Vec<(String, String)> = Vec::new();
    for group in groups {
        for (key, value) in group.iter() {
            if !merged.iter().any(|(existing, _)| existing == key) {
                merged.push((key.clone(), value.clone()));
            }
        }
    }
    merged
}

/// Joins non-empty shell snippets with a newline.
pub fn combine_scripts(parts: &[String]) -> String {
    parts
        .iter()
        .filter(|part| !part.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn add_device_line(q_name: &str, device: &DeviceSpec) -> String {
    let options = device_options(&device.options);
    if options.is_empty() {
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
    }
}

/// Returns an idempotent snippet that installs the plan's devices.
///
/// The container must already be running. Parent directories are created first
/// because Incus proxy devices fail to connect when the listen path's parent is
/// missing. Existing devices are removed first so a changed host socket (for
/// example after a logout/login) is refreshed.
pub fn apply_devices_script(name: &str, plan: &DisplayPlan) -> String {
    if plan.devices.is_empty() {
        return String::new();
    }
    let q_name = shell_quote(name);
    let mut lines = Vec::new();
    if plan.has_wayland() {
        if let Some(runtime_dir) = plan
            .env
            .iter()
            .find(|(key, _)| key == "XDG_RUNTIME_DIR")
            .map(|(_, value)| value.as_str())
        {
            lines.push(format!(
                "incus exec {q_name} -- sh -c {} aibox {} >/dev/null 2>&1 || true",
                shell_quote("mkdir -p \"$1\" && chmod 700 \"$1\""),
                shell_quote(runtime_dir)
            ));
        }
    }
    if plan.has_x11() {
        lines.push(format!(
            "incus exec {q_name} -- mkdir -p /tmp/.X11-unix >/dev/null 2>&1 || true"
        ));
        lines.push(XHOST_ROOT_SNIPPET.to_string());
    }
    lines.push(remove_device_lines(name, &[WAYLAND_DEVICE, X11_DEVICE]));
    for device in &plan.devices {
        lines.push(add_device_line(&q_name, device));
    }
    lines.join("\n")
}

/// Builds a batched session-bridge script for Start, Enter, and Exec.
///
/// Combines all container-side directory creation + XDG download setup into a
/// single `incus exec` call and queries `incus config device show` once so
/// absent devices do not incur redundant `incus config device remove` calls.
pub fn session_bridge_script(
    name: &str,
    desktop: &DisplayPlan,
    audio: &crate::display::AudioPlan,
    appearance: &crate::display::AppearancePlan,
    dbus: &crate::display::DbusPlan,
    extra: &crate::display::ExtraDevicesPlan,
) -> String {
    let q_name = shell_quote(name);
    let mut mkdir_dirs: Vec<String> = Vec::new();
    let mut runtime_chmod_dir: Option<String> = None;

    if desktop.has_wayland() {
        if let Some(runtime_dir) = desktop
            .env
            .iter()
            .find(|(k, _)| k == "XDG_RUNTIME_DIR")
            .map(|(_, v)| v.as_str())
        {
            runtime_chmod_dir = Some(runtime_dir.to_string());
        }
    }
    if desktop.has_x11() {
        mkdir_dirs.push("/tmp/.X11-unix".to_string());
    }
    if !audio.devices.is_empty() {
        if let Some(pulse_dir) = audio
            .env
            .iter()
            .find(|(k, _)| k == "PULSE_SERVER")
            .and_then(|(_, v)| v.strip_prefix("unix:"))
            .and_then(|p| p.rsplit_once('/'))
            .map(|(dir, _)| dir.to_string())
        {
            mkdir_dirs.push(pulse_dir);
        }
        mkdir_dirs.push("/run/aibox".to_string());
    }
    for device in &appearance.devices {
        if let Some((_, path)) = device.options.iter().find(|(k, _)| k == "path") {
            mkdir_dirs.push(path.clone());
        }
    }
    if dbus.enabled && !dbus.devices.is_empty() {
        let rt = format!("/run/user/{}", dbus.uid);
        if runtime_chmod_dir.as_deref() != Some(rt.as_str()) {
            mkdir_dirs.push(rt);
        }
    }
    if extra
        .devices
        .iter()
        .any(|d| d.name == crate::display::PRINTER_DEVICE)
    {
        mkdir_dirs.push("/run/cups".to_string());
    }

    let mut lines = Vec::new();
    let mut exec_cmds = Vec::new();
    if let Some(ref rt) = runtime_chmod_dir {
        let q_rt = shell_quote(rt);
        exec_cmds.push(format!("mkdir -p {q_rt} && chmod 700 {q_rt}"));
    }
    if !mkdir_dirs.is_empty() {
        let q_dirs: Vec<String> = mkdir_dirs.iter().map(|d| shell_quote(d)).collect();
        exec_cmds.push(format!("mkdir -p {}", q_dirs.join(" ")));
    }
    if !exec_cmds.is_empty() {
        let combined = format!("{} >/dev/null 2>&1 || true", exec_cmds.join(" && "));
        lines.push(format!(
            "incus exec {q_name} -- sh -c {} >/dev/null 2>&1 || true",
            shell_quote(&combined)
        ));
    }

    if desktop.has_x11() {
        lines.push(XHOST_ROOT_SNIPPET.to_string());
    }

    // Collect all planned device names + known managed prefixes to remove only what exists.
    let mut managed_exact: Vec<&str> = Vec::new();
    if !desktop.devices.is_empty() {
        managed_exact.extend([WAYLAND_DEVICE, X11_DEVICE]);
    }
    if !audio.devices.is_empty() {
        managed_exact.extend(["pulse", "pipewire", "pulse-cookie"]);
    }
    if !appearance.devices.is_empty() {
        managed_exact.extend(crate::display::APPEARANCE_DEVICE_NAMES.iter().copied());
    }
    if dbus.enabled && !dbus.devices.is_empty() {
        managed_exact.push("dbus");
    }

    if !managed_exact.is_empty() || !extra.devices.is_empty() {
        let exact_pat = managed_exact.join("|");
        let case_pat = if exact_pat.is_empty() {
            format!(
                "{}*|{}*|{}",
                crate::display::CAMERA_PREFIX,
                crate::display::INPUT_PREFIX,
                crate::display::PRINTER_DEVICE
            )
        } else {
            format!(
                "{exact_pat}|{}*|{}*|{}",
                crate::display::CAMERA_PREFIX,
                crate::display::INPUT_PREFIX,
                crate::display::PRINTER_DEVICE
            )
        };
        lines.push(format!(
            "incus config device show {q_name} 2>/dev/null | awk '/^[^[:space:]][^:]*:$/ {{ sub(/:$/, \"\"); print }}' | while IFS= read -r dev; do\n    case \"$dev\" in\n        {case_pat})\n            incus config device remove {q_name} \"$dev\" >/dev/null 2>&1 || true\n            ;;\n    esac\ndone || true"
        ));
    }

    for device in &desktop.devices {
        lines.push(add_device_line(&q_name, device));
    }
    for device in &audio.devices {
        lines.push(add_device_line(&q_name, device));
    }
    for device in &appearance.devices {
        lines.push(add_device_line(&q_name, device));
    }
    for device in &extra.devices {
        lines.push(add_device_line(&q_name, device));
    }

    let dbus_script = crate::display::apply_dbus_devices_script(name, dbus);
    if !dbus_script.is_empty() {
        lines.push(dbus_script);
    }
    let dl_script = crate::display::download_dir_script(name);
    if !dl_script.is_empty() {
        lines.push(dl_script);
    }

    lines.join("\n")
}

/// Removes both bridge devices. Shared `xhost` grants are intentionally kept
/// because other environments may still rely on them.
pub fn remove_devices_script(name: &str, _plan: &DisplayPlan) -> String {
    remove_device_lines(name, &[WAYLAND_DEVICE, X11_DEVICE])
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
            let apply_display = apply_devices_script(name, &build_desktop_plan(mode, host));
            let appearance_plan =
                crate::display::build_appearance_plan(&crate::display::detect_host_appearance());
            let apply_appearance =
                crate::display::apply_appearance_devices_script(name, &appearance_plan);
            let combined_apply = combine_scripts(&[apply_display, apply_appearance]);
            if !combined_apply.is_empty() {
                // A stopped instance cannot accept a proxy device that listens
                // on a container path, so only configure it once it is running.
                // The next Start/Enter applies the devices again.
                lines.push(format!(
                    "STATUS=$(incus info {q_name} | awk '/^Status:/ {{print $2}}')"
                ));
                lines.push("if [[ \"$STATUS\" == \"RUNNING\" ]]; then".to_string());
                lines.push(combined_apply);
                lines.push("fi".to_string());
            }
        }
        // Off means headless: remove every bridge device even though an Off
        // plan has no devices of its own. Appearance mounts follow the desktop
        // switch too, so a headless environment keeps no host theme files.
        DesktopMode::Off => {
            lines.push(remove_device_lines(name, &[WAYLAND_DEVICE, X11_DEVICE]));
            lines.push(crate::display::appearance::remove_appearance_devices_script(name));
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn host_with_both() -> HostSession {
        HostSession {
            uid: 1000,
            gid: 1000,
            runtime_dir: "/run/user/1000".to_string(),
            x11_display: ":0".to_string(),
            x11_socket: "/tmp/.X11-unix/X0".to_string(),
            wayland_display: "wayland-1".to_string(),
            wayland_socket: "/run/user/1000/wayland-1".to_string(),
            xdg_current_desktop: "Hyprland".to_string(),
        }
    }

    fn host_with_x11_only() -> HostSession {
        HostSession {
            wayland_display: String::new(),
            wayland_socket: String::new(),
            ..host_with_both()
        }
    }

    fn host_with_wayland_only() -> HostSession {
        HostSession {
            x11_display: String::new(),
            x11_socket: String::new(),
            ..host_with_both()
        }
    }

    fn host_without_display() -> HostSession {
        HostSession {
            x11_display: String::new(),
            x11_socket: String::new(),
            wayland_display: String::new(),
            wayland_socket: String::new(),
            xdg_current_desktop: String::new(),
            ..host_with_both()
        }
    }

    fn env_value<'a>(plan: &'a DisplayPlan, key: &str) -> Option<&'a str> {
        plan.env
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
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
    fn on_with_wayland_and_x11_exposes_both_devices() {
        let plan = build_desktop_plan(DesktopMode::On, &host_with_both());
        assert!(plan.has_wayland());
        assert!(plan.has_x11());
        assert_eq!(plan.devices.len(), 2);
        assert_eq!(plan.devices[0].name, "wayland");
        assert_eq!(plan.devices[1].name, "x11");

        let wayland = &plan.devices[0];
        assert_eq!(wayland.kind, "proxy");
        assert_eq!(
            wayland.options,
            vec![
                (
                    "connect".to_string(),
                    "unix:/run/user/1000/wayland-1".to_string()
                ),
                (
                    "listen".to_string(),
                    "unix:/run/user/1000/wayland-0".to_string()
                ),
                ("bind".to_string(), "container".to_string()),
                ("mode".to_string(), "0666".to_string()),
            ]
        );

        let x11 = &plan.devices[1];
        assert_eq!(
            x11.options,
            vec![
                ("connect".to_string(), "unix:/tmp/.X11-unix/X0".to_string()),
                ("listen".to_string(), "unix:/tmp/.X11-unix/X0".to_string()),
                ("bind".to_string(), "container".to_string()),
            ]
        );
        assert!(plan.warnings.is_empty());
    }

    #[test]
    fn on_with_wayland_adds_wayland_environment() {
        let plan = build_desktop_plan(DesktopMode::On, &host_with_both());
        for (key, value) in [
            ("WAYLAND_DISPLAY", "wayland-0"),
            ("XDG_RUNTIME_DIR", "/run/user/1000"),
            ("XDG_SESSION_TYPE", "wayland"),
            ("MOZ_ENABLE_WAYLAND", "1"),
            ("QT_QPA_PLATFORM", "wayland;xcb"),
            ("GDK_BACKEND", "wayland,x11"),
            ("SDL_VIDEODRIVER", "wayland"),
            ("ELECTRON_OZONE_PLATFORM_HINT", "auto"),
            ("CLUTTER_BACKEND", "wayland"),
            ("XDG_CURRENT_DESKTOP", "Hyprland"),
            ("DISPLAY", ":0"),
        ] {
            assert_eq!(env_value(&plan, key), Some(value), "env {key}");
        }
    }

    #[test]
    fn xdg_current_desktop_is_omitted_when_the_host_has_none() {
        let host = HostSession {
            xdg_current_desktop: String::new(),
            ..host_with_both()
        };
        let plan = build_desktop_plan(DesktopMode::On, &host);
        assert_eq!(env_value(&plan, "XDG_CURRENT_DESKTOP"), None);
    }

    #[test]
    fn on_with_x11_only_keeps_the_fallback_working() {
        let plan = build_desktop_plan(DesktopMode::On, &host_with_x11_only());
        assert!(!plan.has_wayland());
        assert!(plan.has_x11());
        assert_eq!(plan.devices.len(), 1);
        assert_eq!(env_value(&plan, "DISPLAY"), Some(":0"));
        assert!(plan.warnings.is_empty());
    }

    #[test]
    fn on_with_wayland_only_warns_that_x11_fallback_is_unavailable() {
        let plan = build_desktop_plan(DesktopMode::On, &host_with_wayland_only());
        assert!(plan.has_wayland());
        assert!(!plan.has_x11());
        assert_eq!(plan.devices.len(), 1);
        assert_eq!(plan.warnings.len(), 1);
        assert!(plan.warnings[0].contains("X11 fallback is unavailable"));
    }

    #[test]
    fn off_is_pure_headless() {
        let plan = build_desktop_plan(DesktopMode::Off, &host_with_both());
        assert!(plan.devices.is_empty());
        assert!(plan.env.is_empty());
        assert!(plan.warnings.is_empty());
        assert_eq!(apply_devices_script("box", &plan), "");
        assert!(!remove_devices_script("box", &plan).is_empty());
    }

    #[test]
    fn on_without_any_socket_warns_and_adds_nothing() {
        let plan = build_desktop_plan(DesktopMode::On, &host_without_display());
        assert!(plan.devices.is_empty());
        assert!(plan.env.is_empty());
        assert_eq!(plan.warnings.len(), 1);
        assert!(plan.warnings[0].contains("Wayland or X11"));
    }

    #[test]
    fn apply_script_prepares_parent_dirs_before_adding_devices() {
        let plan = build_desktop_plan(DesktopMode::On, &host_with_both());
        let script = apply_devices_script("box", &plan);
        let wayland_prep_at = script
            .find("incus exec 'box' -- sh -c 'mkdir -p \"$1\" && chmod 700 \"$1\"' aibox '/run/user/1000'")
            .expect("wayland prep");
        let x11_prep_at = script
            .find("incus exec 'box' -- mkdir -p /tmp/.X11-unix >/dev/null 2>&1 || true")
            .expect("x11 prep");
        let remove_at = script.find("incus config device remove").expect("remove");
        let add_at = script.find("incus config device add").expect("add");
        assert!(wayland_prep_at < remove_at);
        assert!(x11_prep_at < remove_at);
        assert!(remove_at < add_at);
    }

    #[test]
    fn apply_script_uses_the_container_mode_and_authorizes_only_root() {
        let plan = build_desktop_plan(DesktopMode::On, &host_with_both());
        let script = apply_devices_script("box", &plan);
        assert!(script.contains("mode='0666'"));
        assert!(script.contains("xhost +si:localuser:root"));
        assert!(!script.contains("xhost +local:"));
        assert!(script.contains("connect='unix:/run/user/1000/wayland-1'"));
        assert!(script.contains("listen='unix:/run/user/1000/wayland-0'"));
        assert!(script.contains("connect='unix:/tmp/.X11-unix/X0'"));
    }

    #[test]
    fn scripts_quote_names_and_paths() {
        let plan = build_desktop_plan(DesktopMode::On, &host_with_both());
        let script = apply_devices_script("it's a box; rm -rf /", &plan);
        assert!(script.contains("'it'\\''s a box; rm -rf /'"));
        assert!(!script.contains("; rm -rf /\n"));
    }

    #[test]
    fn remove_script_removes_both_devices_and_keeps_xhost() {
        let plan = build_desktop_plan(DesktopMode::On, &host_with_x11_only());
        let script = remove_devices_script("box", &plan);
        assert!(script.contains("incus config device remove 'box' 'wayland'"));
        assert!(script.contains("incus config device remove 'box' 'x11'"));
        assert!(!script.contains("xhost"));
    }

    #[test]
    fn set_mode_script_applies_devices_only_while_running() {
        let on = set_desktop_mode_script("box", DesktopMode::On, &host_with_both());
        assert!(on.contains("incus config set 'box' user.aibox.desktop 'on'"));
        assert!(on.contains("STATUS=$(incus info 'box' | awk '/^Status:/ {print $2}')"));
        let guard_at = on
            .find("if [[ \"$STATUS\" == \"RUNNING\" ]]; then")
            .expect("running guard");
        let add_at = on
            .find("incus config device add 'box' 'wayland' 'proxy'")
            .expect("wayland add");
        assert!(guard_at < add_at);
        assert!(on.contains("incus config device add 'box' 'x11' 'proxy'"));

        let off = set_desktop_mode_script("box", DesktopMode::Off, &host_with_both());
        assert!(off.contains("incus config set 'box' user.aibox.desktop 'off'"));
        assert!(off.contains("incus config device remove 'box' 'wayland'"));
        assert!(off.contains("incus config device remove 'box' 'x11'"));
        assert!(off.contains("incus config device remove 'box' 'fonts-system'"));
        assert!(!off.contains("xhost"));
    }

    #[test]
    fn generated_scripts_pass_bash_syntax_check() {
        let both = build_desktop_plan(DesktopMode::On, &host_with_both());
        let scripts = [
            apply_devices_script("box", &both),
            remove_devices_script("box", &both),
            set_desktop_mode_script("box", DesktopMode::On, &host_with_both()),
            set_desktop_mode_script("box", DesktopMode::Off, &host_with_both()),
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

    #[test]
    fn merge_env_pairs_keeps_the_first_value_and_skips_empty_scripts() {
        let display = vec![
            ("XDG_RUNTIME_DIR".to_string(), "/run/user/1000".to_string()),
            ("WAYLAND_DISPLAY".to_string(), "wayland-0".to_string()),
        ];
        let audio = vec![
            ("XDG_RUNTIME_DIR".to_string(), "/run/user/9999".to_string()),
            (
                "PULSE_SERVER".to_string(),
                "unix:/run/user/1000/pulse/native".to_string(),
            ),
        ];
        let merged = merge_env_pairs(&[&display, &audio]);
        assert_eq!(merged.len(), 3);
        assert_eq!(
            merged
                .iter()
                .find(|(key, _)| key == "XDG_RUNTIME_DIR")
                .map(|(_, value)| value.as_str()),
            Some("/run/user/1000")
        );
        assert_eq!(merged[2].0, "PULSE_SERVER");

        assert_eq!(combine_scripts(&["".to_string(), "  \n".to_string()]), "");
        assert_eq!(
            combine_scripts(&["A".to_string(), String::new(), "B".to_string()]),
            "A\nB"
        );
    }
}
