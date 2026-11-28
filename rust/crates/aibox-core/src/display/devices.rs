//! Opt-in camera, USB/input, and printer access for environments.
//!
//! These devices grant real host hardware access, so every category is off by
//! default and only the selected environment receives the nodes. The plan is
//! kept separate from the shell snippets so detection and device naming stay
//! unit-testable without a live Incus daemon. Containers run as root, so the
//! char devices use `mode=0666` like the GPU and audio bridges.

use std::path::Path;

use crate::display::plan::DeviceSpec;
use crate::util::shell::shell_quote;

/// Per-environment extra device switches (`user.aibox.camera`, `...usb`,
/// `...printer`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExtraDeviceToggles {
    pub camera: bool,
    pub usb: bool,
    pub printer: bool,
}

impl ExtraDeviceToggles {
    pub fn from_envs(camera: bool, usb: bool, printer: bool) -> Self {
        Self {
            camera,
            usb,
            printer,
        }
    }

    pub fn any(self) -> bool {
        self.camera || self.usb || self.printer
    }

    /// Stable value written to the matching `user.aibox.*` key.
    pub fn config_value(enabled: bool) -> &'static str {
        if enabled {
            "on"
        } else {
            "off"
        }
    }
}

/// Parses a stored toggle flag. Empty or unknown values stay off, because
/// hardware access must never be inferred.
pub fn parse_toggle_flag(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "on" | "true" | "1"
    )
}

/// Host device nodes detected once per operation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostDevices {
    /// `/dev/video*` nodes, sorted.
    pub video_nodes: Vec<String>,
    /// `/dev/input/event*`, `/dev/input/js*`, and `/dev/input/mouse*` nodes.
    pub input_nodes: Vec<String>,
    /// `/run/cups/cups.sock`, empty when the host CUPS socket is missing.
    pub cups_socket: String,
}

/// Everything needed to expose the opt-in host devices inside one environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraDevicesPlan {
    pub toggles: ExtraDeviceToggles,
    pub devices: Vec<DeviceSpec>,
    pub env: Vec<(String, String)>,
    pub warnings: Vec<String>,
}

/// Host CUPS socket for the explicit printer proxy.
pub const CUPS_SOCKET: &str = "/run/cups/cups.sock";
pub const CAMERA_PREFIX: &str = "camera-";
pub const INPUT_PREFIX: &str = "input-";
pub const PRINTER_DEVICE: &str = "printer";

/// Reads the host device directories and the CUPS socket path.
pub fn detect_host_devices() -> HostDevices {
    detect_host_devices_with(
        |dir| {
            std::fs::read_dir(dir)
                .map(|entries| {
                    entries
                        .flatten()
                        .filter_map(|entry| entry.file_name().into_string().ok())
                        .collect()
                })
                .unwrap_or_default()
        },
        |path| Path::new(path).exists(),
    )
}

/// Pure device detector; `read_dir` returns the entry names of a directory and
/// `exists` reports whether a host path is present.
pub fn detect_host_devices_with(
    read_dir: impl Fn(&str) -> Vec<String>,
    exists: impl Fn(&str) -> bool,
) -> HostDevices {
    let mut video_nodes: Vec<String> = read_dir("/dev")
        .into_iter()
        .filter(|name| has_numeric_suffix(name, "video"))
        .map(|name| format!("/dev/{name}"))
        .collect();
    stable_sort(&mut video_nodes);

    let mut input_nodes: Vec<String> = read_dir("/dev/input")
        .into_iter()
        .filter(|name| {
            has_numeric_suffix(name, "event")
                || has_numeric_suffix(name, "js")
                || has_numeric_suffix(name, "mouse")
        })
        .map(|name| format!("/dev/input/{name}"))
        .collect();
    stable_sort(&mut input_nodes);

    let cups_socket = if exists(CUPS_SOCKET) {
        CUPS_SOCKET.to_string()
    } else {
        String::new()
    };

    HostDevices {
        video_nodes,
        input_nodes,
        cups_socket,
    }
}

fn has_numeric_suffix(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix)
        .is_some_and(|suffix| !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()))
}

fn stable_sort(nodes: &mut Vec<String>) {
    nodes.sort();
    nodes.dedup();
}

/// Builds the opt-in device plan. Categories without host devices only emit a
/// warning; the rest of the plan still applies.
pub fn build_extra_devices_plan(
    toggles: ExtraDeviceToggles,
    host: &HostDevices,
) -> ExtraDevicesPlan {
    let mut plan = ExtraDevicesPlan {
        toggles,
        devices: Vec::new(),
        env: Vec::new(),
        warnings: Vec::new(),
    };

    if toggles.camera {
        if host.video_nodes.is_empty() {
            plan.warnings.push(
                "Camera is on, but no /dev/video* devices were found on the host.".to_string(),
            );
        }
        for node in &host.video_nodes {
            plan.devices.push(unix_char_device(
                &format!("{CAMERA_PREFIX}{}", device_stem(node)),
                node,
            ));
        }
    }

    if toggles.usb {
        if host.input_nodes.is_empty() {
            plan.warnings.push(
                "USB/input is on, but no /dev/input/event*, js*, or mouse* devices were found on the host."
                    .to_string(),
            );
        }
        for node in &host.input_nodes {
            plan.devices.push(unix_char_device(
                &format!("{INPUT_PREFIX}{}", device_stem(node)),
                node,
            ));
        }
    }

    if toggles.printer {
        if host.cups_socket.is_empty() {
            plan.warnings.push(format!(
                "Printer is on, but the host CUPS socket {CUPS_SOCKET} is missing."
            ));
        } else {
            plan.devices.push(DeviceSpec {
                name: PRINTER_DEVICE.to_string(),
                kind: "proxy".to_string(),
                options: vec![
                    ("connect".to_string(), format!("unix:{}", host.cups_socket)),
                    ("listen".to_string(), format!("unix:{}", host.cups_socket)),
                    ("bind".to_string(), "container".to_string()),
                    ("mode".to_string(), "0666".to_string()),
                ],
            });
            plan.env
                .push(("CUPS_SERVER".to_string(), host.cups_socket.clone()));
        }
    }

    plan
}

/// `camera-video0` and `input-event0` need the bare node name as the suffix.
fn device_stem(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn unix_char_device(name: &str, node: &str) -> DeviceSpec {
    DeviceSpec {
        name: name.to_string(),
        kind: "unix-char".to_string(),
        options: vec![
            ("source".to_string(), node.to_string()),
            ("path".to_string(), node.to_string()),
            ("mode".to_string(), "0666".to_string()),
            ("required".to_string(), "false".to_string()),
        ],
    }
}

/// Returns an idempotent snippet that installs the plan's devices.
///
/// `/run/cups` must exist before Incus can listen on the proxied socket. Every
/// extra device is removed first so a node that vanished on the host does not
/// leave a stale device behind.
pub fn apply_extra_devices_script(name: &str, plan: &ExtraDevicesPlan) -> String {
    if plan.devices.is_empty() {
        return String::new();
    }
    let q_name = shell_quote(name);
    let mut lines = Vec::new();
    if plan.devices.iter().any(|d| d.name == PRINTER_DEVICE) {
        lines.push(format!(
            "incus exec {q_name} -- mkdir -p {} >/dev/null 2>&1 || true",
            shell_quote("/run/cups")
        ));
    }
    lines.push(remove_extra_devices_script(name));

    for device in &plan.devices {
        lines.push(crate::display::plan::add_device_line(&q_name, device));
    }
    lines.join("\n")
}

/// Removes every device owned by this bridge, including names that are no
/// longer in the current plan. Device names depend on host nodes, so the list
/// is discovered from the instance config instead of a fixed set.
pub fn remove_extra_devices_script(name: &str) -> String {
    let q_name = shell_quote(name);
    format!(
        "incus config device show {q_name} 2>/dev/null | awk '/^[^[:space:]][^:]*:$/ {{ sub(/:$/, \"\"); print }}' | while IFS= read -r dev; do\n    case \"$dev\" in\n        {CAMERA_PREFIX}*|{INPUT_PREFIX}*|{PRINTER_DEVICE})\n            incus config device remove {q_name} \"$dev\" >/dev/null 2>&1 || true\n            ;;\n    esac\ndone || true"
    )
}

/// Writes all three toggles and applies the devices while the instance runs.
///
/// Turning any category off removes stale devices immediately, even while the
/// instance is stopped, because configured devices are only attached while it
/// runs. Enabled categories are re-added by the next Start/Enter.
pub fn set_extra_devices_script(
    name: &str,
    toggles: &ExtraDeviceToggles,
    host: &HostDevices,
) -> String {
    let q_name = shell_quote(name);
    let mut lines = vec![
        "set -euo pipefail".to_string(),
        format!(
            "incus config set {q_name} user.aibox.camera={}",
            shell_quote(ExtraDeviceToggles::config_value(toggles.camera))
        ),
        format!(
            "incus config set {q_name} user.aibox.usb={}",
            shell_quote(ExtraDeviceToggles::config_value(toggles.usb))
        ),
        format!(
            "incus config set {q_name} user.aibox.printer={}",
            shell_quote(ExtraDeviceToggles::config_value(toggles.printer))
        ),
    ];

    let plan = build_extra_devices_plan(*toggles, host);
    if plan.devices.is_empty() {
        lines.push(remove_extra_devices_script(name));
        return lines.join("\n");
    }

    lines.push(format!(
        "STATUS=$(incus info {q_name} | awk '/^Status:/ {{print $2}}')"
    ));
    lines.push("if [[ \"$STATUS\" == \"RUNNING\" ]]; then".to_string());
    lines.push(apply_extra_devices_script(name, &plan));
    lines.push("else".to_string());
    lines.push(remove_extra_devices_script(name));
    lines.push("fi".to_string());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn host_ready() -> HostDevices {
        HostDevices {
            video_nodes: vec!["/dev/video0".to_string(), "/dev/video1".to_string()],
            input_nodes: vec![
                "/dev/input/event0".to_string(),
                "/dev/input/event2".to_string(),
                "/dev/input/js0".to_string(),
                "/dev/input/mouse0".to_string(),
            ],
            cups_socket: CUPS_SOCKET.to_string(),
        }
    }

    fn device<'a>(plan: &'a ExtraDevicesPlan, name: &str) -> Option<&'a DeviceSpec> {
        plan.devices.iter().find(|device| device.name == name)
    }

    fn env_value<'a>(plan: &'a ExtraDevicesPlan, key: &str) -> Option<&'a str> {
        plan.env
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }

    #[test]
    fn toggles_default_off_and_only_any_reports_enabled_categories() {
        let off = ExtraDeviceToggles::from_envs(false, false, false);
        assert!(!off.any());
        assert!(ExtraDeviceToggles::from_envs(true, false, false).any());
        assert!(ExtraDeviceToggles::from_envs(false, true, false).any());
        assert!(ExtraDeviceToggles::from_envs(false, false, true).any());
        assert_eq!(ExtraDeviceToggles::config_value(true), "on");
        assert_eq!(ExtraDeviceToggles::config_value(false), "off");
    }

    #[test]
    fn toggle_flag_accepts_only_explicit_on_values() {
        for raw in ["on", "true", "1", " ON ", "True"] {
            assert!(parse_toggle_flag(raw), "raw {raw}");
        }
        for raw in ["", "off", "false", "0", "yes", "bogus"] {
            assert!(!parse_toggle_flag(raw), "raw {raw}");
        }
    }

    #[test]
    fn detection_keeps_numeric_nodes_sorts_and_reads_the_cups_socket() {
        let host = detect_host_devices_with(
            |dir| match dir {
                "/dev" => vec![
                    "video10".to_string(),
                    "video2".to_string(),
                    "video0".to_string(),
                    "video".to_string(),
                    "video0x".to_string(),
                    "tty0".to_string(),
                ],
                "/dev/input" => vec![
                    "event1".to_string(),
                    "event".to_string(),
                    "js0".to_string(),
                    "jsx".to_string(),
                    "mouse0".to_string(),
                    "mouse".to_string(),
                    "by-id".to_string(),
                ],
                _ => Vec::new(),
            },
            |path| path == CUPS_SOCKET,
        );
        assert_eq!(
            host.video_nodes,
            vec![
                "/dev/video0".to_string(),
                "/dev/video10".to_string(),
                "/dev/video2".to_string()
            ]
        );
        assert_eq!(
            host.input_nodes,
            vec![
                "/dev/input/event1".to_string(),
                "/dev/input/js0".to_string(),
                "/dev/input/mouse0".to_string()
            ]
        );
        assert_eq!(host.cups_socket, CUPS_SOCKET);
    }

    #[test]
    fn detection_reports_missing_directories_and_socket_as_empty() {
        let host = detect_host_devices_with(|_| Vec::new(), |_| false);
        assert!(host.video_nodes.is_empty());
        assert!(host.input_nodes.is_empty());
        assert!(host.cups_socket.is_empty());
    }

    #[test]
    fn off_plan_has_no_devices_or_environment() {
        let plan = build_extra_devices_plan(
            ExtraDeviceToggles::from_envs(false, false, false),
            &host_ready(),
        );
        assert!(plan.devices.is_empty());
        assert!(plan.env.is_empty());
        assert!(plan.warnings.is_empty());
        assert_eq!(apply_extra_devices_script("box", &plan), "");
    }

    #[test]
    fn camera_adds_one_required_false_unix_char_per_video_node() {
        let plan = build_extra_devices_plan(
            ExtraDeviceToggles::from_envs(true, false, false),
            &host_ready(),
        );
        assert!(plan.warnings.is_empty());
        assert_eq!(plan.devices.len(), 2);
        let camera = device(&plan, "camera-video0").expect("camera device");
        assert_eq!(camera.kind, "unix-char");
        assert_eq!(
            camera.options,
            vec![
                ("source".to_string(), "/dev/video0".to_string()),
                ("path".to_string(), "/dev/video0".to_string()),
                ("mode".to_string(), "0666".to_string()),
                ("required".to_string(), "false".to_string()),
            ]
        );
        assert!(device(&plan, "camera-video1").is_some());
        assert!(plan.env.is_empty());
    }

    #[test]
    fn usb_adds_input_devices_for_every_event_js_and_mouse_node() {
        let plan = build_extra_devices_plan(
            ExtraDeviceToggles::from_envs(false, true, false),
            &host_ready(),
        );
        assert!(plan.warnings.is_empty());
        assert_eq!(plan.devices.len(), 4);
        for (name, path) in [
            ("input-event0", "/dev/input/event0"),
            ("input-event2", "/dev/input/event2"),
            ("input-js0", "/dev/input/js0"),
            ("input-mouse0", "/dev/input/mouse0"),
        ] {
            let node = device(&plan, name).unwrap_or_else(|| panic!("missing {name}"));
            assert_eq!(node.kind, "unix-char");
            assert_eq!(
                node.options,
                vec![
                    ("source".to_string(), path.to_string()),
                    ("path".to_string(), path.to_string()),
                    ("mode".to_string(), "0666".to_string()),
                    ("required".to_string(), "false".to_string()),
                ]
            );
        }
    }

    #[test]
    fn printer_adds_the_cups_proxy_and_environment() {
        let plan = build_extra_devices_plan(
            ExtraDeviceToggles::from_envs(false, false, true),
            &host_ready(),
        );
        assert!(plan.warnings.is_empty());
        let printer = device(&plan, "printer").expect("printer device");
        assert_eq!(printer.kind, "proxy");
        assert_eq!(
            printer.options,
            vec![
                (
                    "connect".to_string(),
                    "unix:/run/cups/cups.sock".to_string()
                ),
                ("listen".to_string(), "unix:/run/cups/cups.sock".to_string()),
                ("bind".to_string(), "container".to_string()),
                ("mode".to_string(), "0666".to_string()),
            ]
        );
        assert_eq!(env_value(&plan, "CUPS_SERVER"), Some(CUPS_SOCKET));
    }

    #[test]
    fn warnings_explain_each_requested_category_without_host_devices() {
        let empty = HostDevices::default();
        let plan =
            build_extra_devices_plan(ExtraDeviceToggles::from_envs(true, true, true), &empty);
        assert!(plan.devices.is_empty());
        assert!(plan.env.is_empty());
        assert_eq!(plan.warnings.len(), 3);
        assert!(plan.warnings[0].contains("no /dev/video*"));
        assert!(plan.warnings[1].contains("no /dev/input/event*"));
        assert!(plan.warnings[2].contains(CUPS_SOCKET));
    }

    #[test]
    fn apply_script_prepares_cups_and_removes_then_adds_devices() {
        let plan = build_extra_devices_plan(
            ExtraDeviceToggles::from_envs(true, true, true),
            &host_ready(),
        );
        let script = apply_extra_devices_script("box", &plan);
        let prep_at = script
            .find("incus exec 'box' -- mkdir -p '/run/cups' >/dev/null 2>&1 || true")
            .expect("cups prep");
        let remove_at = script.find("config device show").expect("remove loop");
        let add_at = script.find("config device add").expect("add");
        assert!(prep_at < remove_at);
        assert!(remove_at < add_at);
        assert!(script.contains("camera-*|input-*|printer"));
        assert!(script.contains("incus config device add 'box' 'camera-video0' 'unix-char'"));
        assert!(script.contains("source='/dev/video0'"));
        assert!(script.contains("mode='0666'"));
        assert!(script.contains("required='false'"));
        assert!(script.contains("incus config device add 'box' 'printer' 'proxy'"));
        assert!(script.contains("connect='unix:/run/cups/cups.sock'"));
    }

    #[test]
    fn remove_script_discovers_known_prefixes_quoted_and_idempotent() {
        let script = remove_extra_devices_script("it's a box; rm -rf /");
        assert!(script.contains("incus config device show 'it'\\''s a box; rm -rf /'"));
        assert!(script.contains("camera-*|input-*|printer"));
        assert!(script.contains("incus config device remove 'it'\\''s a box; rm -rf /' \"$dev\""));
        assert!(script.contains(">/dev/null 2>&1 || true"));
        assert!(script.ends_with("done || true"));
        assert!(!script.contains("; rm -rf /\n"));
    }

    #[test]
    fn set_script_writes_every_toggle_and_removes_when_all_off() {
        let off = set_extra_devices_script(
            "box",
            &ExtraDeviceToggles::from_envs(false, false, false),
            &host_ready(),
        );
        assert!(off.contains("incus config set 'box' user.aibox.camera='off'"));
        assert!(off.contains("incus config set 'box' user.aibox.usb='off'"));
        assert!(off.contains("incus config set 'box' user.aibox.printer='off'"));
        assert!(off.contains("config device show"));
        assert!(!off.contains("STATUS="));
        assert!(!off.contains("config device add"));
    }

    #[test]
    fn set_script_removes_stale_devices_before_applying_while_running() {
        let on = set_extra_devices_script(
            "box",
            &ExtraDeviceToggles::from_envs(true, false, true),
            &host_ready(),
        );
        assert!(on.contains("user.aibox.camera='on'"));
        assert!(on.contains("user.aibox.usb='off'"));
        assert!(on.contains("user.aibox.printer='on'"));
        let guard_at = on
            .find("if [[ \"$STATUS\" == \"RUNNING\" ]]; then")
            .expect("running guard");
        let remove_at = on[guard_at..]
            .find("config device show")
            .map(|rel| guard_at + rel)
            .expect("stale removal inside running branch");
        let camera_add_at = on
            .find("config device add 'box' 'camera-video0'")
            .expect("camera add");
        assert!(guard_at < remove_at);
        assert!(remove_at < camera_add_at);
        assert!(on.contains("else\nincus config device show"));
        assert!(on.contains("config device add 'box' 'printer' 'proxy'"));
        assert!(!on.contains("'input-event0'"));
    }

    #[test]
    fn generated_scripts_pass_bash_syntax_check() {
        let host = host_ready();
        let all = build_extra_devices_plan(ExtraDeviceToggles::from_envs(true, true, true), &host);
        let scripts = [
            apply_extra_devices_script("box", &all),
            remove_extra_devices_script("box"),
            set_extra_devices_script(
                "box",
                &ExtraDeviceToggles::from_envs(false, false, false),
                &host,
            ),
            set_extra_devices_script(
                "box",
                &ExtraDeviceToggles::from_envs(true, false, false),
                &host,
            ),
            set_extra_devices_script(
                "box",
                &ExtraDeviceToggles::from_envs(true, true, true),
                &host,
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

    #[test]
    fn detection_reads_the_real_host_without_panicking() {
        // The real host may have no camera/printer; the call must still be safe.
        let host = detect_host_devices();
        for node in host.video_nodes.iter().chain(host.input_nodes.iter()) {
            assert!(node.starts_with('/'));
        }
        assert!(
            host.cups_socket.is_empty() || host.cups_socket == CUPS_SOCKET,
            "unexpected cups socket {}",
            host.cups_socket
        );
    }
}
