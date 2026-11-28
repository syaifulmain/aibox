//! Host audio bridge: PulseAudio compatibility and PipeWire sockets.
//!
//! The bridge turns the host audio sockets into an Incus device plan plus the
//! environment variables that make programs inside the instance find them. It
//! is deliberately independent from the display bridge so a headless server
//! environment can still be silent while a GUI environment plays sound.
//!
//! Isolation limits: `Output` does not expose the native PipeWire socket, but
//! the PulseAudio compatibility socket still carries the host's source
//! (microphone) ports. A program inside the instance can therefore record from
//! them even in `Output` mode. True per-application microphone denial needs a
//! PipeWire policy layer, which AIBox does not implement yet. `Full` simply
//! also exposes the native PipeWire socket for lower-latency audio and
//! PipeWire-native tools.

use std::env;
use std::path::Path;

use crate::display::plan::{remove_device_lines, DeviceSpec};
use crate::util::shell::shell_quote;

/// Per-environment audio mode (`user.aibox.audio`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioMode {
    /// No host audio devices at all.
    Off,
    /// Playback through the host PulseAudio compatibility socket.
    Output,
    /// Playback plus the native PipeWire socket and microphone access.
    Full,
}

impl AudioMode {
    /// Parses a stored or user-supplied value.
    ///
    /// Empty and unknown values fall back to `Off`; the setter uses
    /// [`AudioMode::try_parse`] when it must reject invalid input instead.
    pub fn parse(raw: &str) -> Self {
        Self::try_parse(raw).unwrap_or(Self::Off)
    }

    /// Parses a known value, returning `None` for empty or unknown input.
    pub fn try_parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "off" | "false" | "0" => Some(Self::Off),
            "output" | "on" | "true" | "1" => Some(Self::Output),
            "full" | "mic" | "output+mic" | "input" => Some(Self::Full),
            _ => None,
        }
    }

    /// Stable value written to `user.aibox.audio`.
    pub fn config_value(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Output => "output",
            Self::Full => "full",
        }
    }

    pub fn from_bool(enabled: bool) -> Self {
        if enabled {
            Self::Output
        } else {
            Self::Off
        }
    }
}

/// Host-side audio sockets and the Pulse cookie, detected once per operation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostAudio {
    pub uid: u32,
    pub gid: u32,
    /// Host runtime directory, usually `/run/user/<uid>`.
    pub runtime_dir: String,
    pub pulse_socket: String,
    pub pulse_available: bool,
    pub pipewire_socket: String,
    pub pipewire_available: bool,
    /// Host `pulse/cookie`, empty when the file is missing.
    pub pulse_cookie: String,
}

/// Everything needed to expose host audio inside one environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioPlan {
    pub mode: AudioMode,
    pub devices: Vec<DeviceSpec>,
    pub env: Vec<(String, String)>,
    pub warnings: Vec<String>,
}

const PULSE_DEVICE: &str = "pulse";
const PIPEWIRE_DEVICE: &str = "pipewire";
const COOKIE_DEVICE: &str = "pulse-cookie";
const AUDIO_DEVICES: [&str; 3] = [PULSE_DEVICE, PIPEWIRE_DEVICE, COOKIE_DEVICE];
const CONTAINER_COOKIE_PATH: &str = "/run/aibox/pulse-cookie";

/// Reads the process environment and OS user IDs.
pub fn detect_host_audio() -> HostAudio {
    // SAFETY: getuid/getgid are always safe to call and do not touch memory.
    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };
    detect_host_audio_with(
        |key| env::var(key).ok().filter(|value| !value.trim().is_empty()),
        |path| Path::new(path).exists(),
        uid,
        gid,
    )
}

/// Pure host-audio detector; `exists` reports whether a host path is present.
pub fn detect_host_audio_with(
    get: impl Fn(&str) -> Option<String>,
    exists: impl Fn(&str) -> bool,
    uid: u32,
    gid: u32,
) -> HostAudio {
    detect_host_audio_inner(&get, &exists, uid, gid)
}

/// Shared implementation so callers can inject both lookups.
fn detect_host_audio_inner(
    get: &impl Fn(&str) -> Option<String>,
    exists: &impl Fn(&str) -> bool,
    uid: u32,
    gid: u32,
) -> HostAudio {
    let runtime_dir = get("XDG_RUNTIME_DIR")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("/run/user/{uid}"));

    let pulse_socket = format!("{runtime_dir}/pulse/native");
    let pipewire_socket = format!("{runtime_dir}/pipewire-0");

    let config_home = get("XDG_CONFIG_HOME")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            get("HOME")
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .map(|home| format!("{home}/.config"))
        });
    let pulse_cookie = config_home
        .map(|config| format!("{config}/pulse/cookie"))
        .filter(|path| exists(path))
        .unwrap_or_default();

    HostAudio {
        uid,
        gid,
        runtime_dir,
        pulse_available: exists(&pulse_socket),
        pulse_socket,
        pipewire_available: exists(&pipewire_socket),
        pipewire_socket,
        pulse_cookie,
    }
}

/// Builds the audio plan for one environment.
///
/// `uid` is the container-side runtime directory owner, which currently matches
/// the host user because containers share the caller's UID mapping.
pub fn build_audio_plan(mode: AudioMode, host: &HostAudio, uid: u32) -> AudioPlan {
    let mut plan = AudioPlan {
        mode,
        devices: Vec::new(),
        env: Vec::new(),
        warnings: Vec::new(),
    };
    if mode == AudioMode::Off {
        return plan;
    }

    let container_runtime_dir = format!("/run/user/{uid}");
    if !host.pulse_available {
        plan.warnings.push(format!(
            "Audio is on, but the host PulseAudio compatibility socket {} is missing; nothing can play sound.",
            host.pulse_socket
        ));
        return plan;
    }

    plan.devices.push(DeviceSpec {
        name: PULSE_DEVICE.to_string(),
        kind: "proxy".to_string(),
        options: vec![
            ("connect".to_string(), format!("unix:{}", host.pulse_socket)),
            (
                "listen".to_string(),
                format!("unix:{container_runtime_dir}/pulse/native"),
            ),
            ("bind".to_string(), "container".to_string()),
            ("mode".to_string(), "0666".to_string()),
        ],
    });
    plan.env.extend([
        ("XDG_RUNTIME_DIR".to_string(), container_runtime_dir.clone()),
        (
            "PULSE_SERVER".to_string(),
            format!("unix:{container_runtime_dir}/pulse/native"),
        ),
    ]);

    if !host.pulse_cookie.is_empty() {
        plan.devices.push(DeviceSpec {
            name: COOKIE_DEVICE.to_string(),
            kind: "disk".to_string(),
            options: vec![
                ("source".to_string(), host.pulse_cookie.clone()),
                ("path".to_string(), CONTAINER_COOKIE_PATH.to_string()),
                ("readonly".to_string(), "true".to_string()),
            ],
        });
        plan.env.push((
            "PULSE_COOKIE".to_string(),
            CONTAINER_COOKIE_PATH.to_string(),
        ));
    }

    if mode == AudioMode::Full {
        if host.pipewire_available {
            plan.devices.push(DeviceSpec {
                name: PIPEWIRE_DEVICE.to_string(),
                kind: "proxy".to_string(),
                options: vec![
                    (
                        "connect".to_string(),
                        format!("unix:{}", host.pipewire_socket),
                    ),
                    (
                        "listen".to_string(),
                        format!("unix:{container_runtime_dir}/pipewire-0"),
                    ),
                    ("bind".to_string(), "container".to_string()),
                    ("mode".to_string(), "0666".to_string()),
                ],
            });
            plan.env
                .push(("PIPEWIRE_REMOTE".to_string(), "pipewire-0".to_string()));
        } else {
            plan.warnings.push(format!(
                "Audio input was requested, but the host PipeWire socket {} is missing; falling back to the PulseAudio compatibility socket only.",
                host.pipewire_socket
            ));
        }
    }

    plan
}

fn plan_runtime_dir(plan: &AudioPlan) -> Option<&str> {
    plan.env
        .iter()
        .find(|(key, _)| key == "XDG_RUNTIME_DIR")
        .map(|(_, value)| value.as_str())
}

/// Returns an idempotent snippet that installs the plan's devices.
///
/// The container must already be running. Incus proxy devices fail when the
/// listen path's parent is missing, so `/run/user/<uid>/pulse` and
/// `/run/aibox` are created first. Existing devices are removed before adding
/// so a host socket change (after a logout/login) is picked up.
pub fn apply_audio_devices_script(name: &str, plan: &AudioPlan) -> String {
    if plan.devices.is_empty() {
        return String::new();
    }
    let q_name = shell_quote(name);
    let mut lines = Vec::new();
    let q_runtime = plan_runtime_dir(plan).map(|dir| shell_quote(&format!("{dir}/pulse")));
    let q_cookie = shell_quote("/run/aibox");
    let prep = match q_runtime {
        Some(runtime) => {
            format!("incus exec {q_name} -- mkdir -p {runtime} {q_cookie} >/dev/null 2>&1 || true")
        }
        None => format!("incus exec {q_name} -- mkdir -p {q_cookie} >/dev/null 2>&1 || true"),
    };
    lines.push(prep);
    // Remove every audio device, not only the planned ones: switching from
    // Full to Output must drop the native PipeWire socket, and a cookie that
    // disappeared from the host must not leave a stale read-only mount behind.
    lines.push(remove_audio_devices_script(name));

    for device in &plan.devices {
        lines.push(crate::display::plan::add_device_line(&q_name, device));
    }
    lines.join("\n")
}

/// Removes every audio bridge device.
pub fn remove_audio_devices_script(name: &str) -> String {
    remove_device_lines(name, &AUDIO_DEVICES)
}

/// Writes the mode and applies or removes the matching audio devices.
pub fn set_audio_mode_script(name: &str, mode: AudioMode, host: &HostAudio) -> String {
    let q_name = shell_quote(name);
    let mut lines = vec![
        "set -euo pipefail".to_string(),
        format!(
            "incus config set {q_name} user.aibox.audio={}",
            shell_quote(mode.config_value())
        ),
    ];
    if mode == AudioMode::Off {
        // Off must also clean up devices on a stopped instance, because a
        // lingering proxy would reappear on the next start.
        lines.push(remove_audio_devices_script(name));
        return lines.join("\n");
    }

    let plan = build_audio_plan(mode, host, host.uid);
    // Drop devices that no longer belong to the mode immediately, even while
    // stopped, so a Full -> Output downgrade cannot keep the PipeWire socket
    // alive until the next Start/Enter. Devices still in the plan stay
    // configured so a GUI Start keeps working before the next Enter.
    let planned: Vec<&str> = plan
        .devices
        .iter()
        .map(|device| device.name.as_str())
        .collect();
    let stale: Vec<&str> = AUDIO_DEVICES
        .iter()
        .copied()
        .filter(|name| !planned.contains(name))
        .collect();
    if !stale.is_empty() {
        lines.push(remove_device_lines(name, &stale));
    }
    let apply = apply_audio_devices_script(name, &plan);
    if !apply.is_empty() {
        // A stopped instance cannot accept a proxy that listens on a container
        // path; the next Start/Enter applies the devices again.
        lines.push(format!(
            "STATUS=$(incus info {q_name} | awk '/^Status:/ {{print $2}}')"
        ));
        lines.push("if [[ \"$STATUS\" == \"RUNNING\" ]]; then".to_string());
        lines.push(apply);
        lines.push("fi".to_string());
    }
    lines.join("\n")
}

/// Builds the in-container audio self-test used by the GUI Test Audio button.
///
/// The test must receive the same bridge environment as Enter, otherwise root
/// inside the container cannot discover the proxied Pulse socket.
pub fn test_audio_script(name: &str, plan: &AudioPlan) -> String {
    if plan.mode == AudioMode::Off {
        return format!(
            "echo {} >&2\nexit 1",
            shell_quote("Audio is off for this environment. Choose Output or Output + Mic first.")
        );
    }
    if plan.env.is_empty() {
        let msg = if plan.warnings.is_empty() {
            "Host audio bridge is unavailable on this host.".to_string()
        } else {
            plan.warnings.join(" ")
        };
        return format!("echo {} >&2\nexit 1", shell_quote(&msg));
    }
    let mut cmd = format!("incus exec {}", shell_quote(name));
    for (key, value) in &plan.env {
        cmd.push_str(&format!(
            " --env {}",
            shell_quote(&format!("{key}={value}"))
        ));
    }
    cmd.push_str(&format!(
        " -- sh -c {}",
        shell_quote(
            "if command -v pactl >/dev/null 2>&1; then pactl info; else echo \"pactl is not installed in this environment.\"; exit 1; fi"
        )
    ));
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::plan::merge_env_pairs;
    use std::process::Command;

    fn host_ready() -> HostAudio {
        HostAudio {
            uid: 1000,
            gid: 1000,
            runtime_dir: "/run/user/1000".to_string(),
            pulse_socket: "/run/user/1000/pulse/native".to_string(),
            pulse_available: true,
            pipewire_socket: "/run/user/1000/pipewire-0".to_string(),
            pipewire_available: true,
            pulse_cookie: "/home/user/.config/pulse/cookie".to_string(),
        }
    }

    fn env_value<'a>(plan: &'a AudioPlan, key: &str) -> Option<&'a str> {
        plan.env
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }

    fn device<'a>(plan: &'a AudioPlan, name: &str) -> Option<&'a DeviceSpec> {
        plan.devices.iter().find(|device| device.name == name)
    }

    #[test]
    fn audio_mode_parses_stable_values() {
        assert_eq!(AudioMode::parse("off"), AudioMode::Off);
        assert_eq!(AudioMode::parse("false"), AudioMode::Off);
        assert_eq!(AudioMode::parse("0"), AudioMode::Off);
        assert_eq!(AudioMode::parse("output"), AudioMode::Output);
        assert_eq!(AudioMode::parse("on"), AudioMode::Output);
        assert_eq!(AudioMode::parse("true"), AudioMode::Output);
        assert_eq!(AudioMode::parse("1"), AudioMode::Output);
        assert_eq!(AudioMode::parse("full"), AudioMode::Full);
        assert_eq!(AudioMode::parse("mic"), AudioMode::Full);
        assert_eq!(AudioMode::parse("output+mic"), AudioMode::Full);
        assert_eq!(AudioMode::parse("input"), AudioMode::Full);
        assert_eq!(AudioMode::parse(""), AudioMode::Off);
        assert_eq!(AudioMode::parse("bogus"), AudioMode::Off);
        assert_eq!(AudioMode::try_parse(""), None);
        assert_eq!(AudioMode::try_parse("bogus"), None);
        assert_eq!(AudioMode::from_bool(true), AudioMode::Output);
        assert_eq!(AudioMode::from_bool(false), AudioMode::Off);
        assert_eq!(AudioMode::Off.config_value(), "off");
        assert_eq!(AudioMode::Output.config_value(), "output");
        assert_eq!(AudioMode::Full.config_value(), "full");
    }

    #[test]
    fn off_has_no_devices_or_environment() {
        let plan = build_audio_plan(AudioMode::Off, &host_ready(), 1000);
        assert!(plan.devices.is_empty());
        assert!(plan.env.is_empty());
        assert!(plan.warnings.is_empty());
        assert_eq!(apply_audio_devices_script("box", &plan), "");
        assert!(!remove_audio_devices_script("box").is_empty());
    }

    #[test]
    fn output_exposes_the_pulse_proxy_and_cookie_read_only() {
        let plan = build_audio_plan(AudioMode::Output, &host_ready(), 1000);
        assert!(plan.warnings.is_empty());
        assert_eq!(plan.devices.len(), 2);

        let pulse = device(&plan, "pulse").expect("pulse device");
        assert_eq!(pulse.kind, "proxy");
        assert_eq!(
            pulse.options,
            vec![
                (
                    "connect".to_string(),
                    "unix:/run/user/1000/pulse/native".to_string()
                ),
                (
                    "listen".to_string(),
                    "unix:/run/user/1000/pulse/native".to_string()
                ),
                ("bind".to_string(), "container".to_string()),
                ("mode".to_string(), "0666".to_string()),
            ]
        );

        let cookie = device(&plan, "pulse-cookie").expect("cookie device");
        assert_eq!(cookie.kind, "disk");
        assert_eq!(
            cookie.options,
            vec![
                (
                    "source".to_string(),
                    "/home/user/.config/pulse/cookie".to_string()
                ),
                ("path".to_string(), "/run/aibox/pulse-cookie".to_string()),
                ("readonly".to_string(), "true".to_string()),
            ]
        );

        assert_eq!(env_value(&plan, "XDG_RUNTIME_DIR"), Some("/run/user/1000"));
        assert_eq!(
            env_value(&plan, "PULSE_SERVER"),
            Some("unix:/run/user/1000/pulse/native")
        );
        assert_eq!(
            env_value(&plan, "PULSE_COOKIE"),
            Some("/run/aibox/pulse-cookie")
        );
        assert_eq!(env_value(&plan, "PIPEWIRE_REMOTE"), None);
        assert!(device(&plan, "pipewire").is_none());
    }

    #[test]
    fn full_adds_the_pipewire_proxy_but_stays_off_the_native_socket_for_output() {
        let full = build_audio_plan(AudioMode::Full, &host_ready(), 1000);
        let pipewire = device(&full, "pipewire").expect("pipewire device");
        assert_eq!(pipewire.kind, "proxy");
        assert_eq!(
            pipewire.options,
            vec![
                (
                    "connect".to_string(),
                    "unix:/run/user/1000/pipewire-0".to_string()
                ),
                (
                    "listen".to_string(),
                    "unix:/run/user/1000/pipewire-0".to_string()
                ),
                ("bind".to_string(), "container".to_string()),
                ("mode".to_string(), "0666".to_string()),
            ]
        );
        assert_eq!(env_value(&full, "PIPEWIRE_REMOTE"), Some("pipewire-0"));
        assert!(full.warnings.is_empty());

        let output = build_audio_plan(AudioMode::Output, &host_ready(), 1000);
        assert!(device(&output, "pipewire").is_none());
    }

    #[test]
    fn missing_cookie_is_not_mounted() {
        let mut host = host_ready();
        host.pulse_cookie = String::new();
        let plan = build_audio_plan(AudioMode::Output, &host, 1000);
        assert!(device(&plan, "pulse-cookie").is_none());
        assert_eq!(env_value(&plan, "PULSE_COOKIE"), None);

        // A cookie that vanished on the host must not leave a stale mount.
        let script = set_audio_mode_script("box", AudioMode::Output, &host);
        assert!(script.contains("incus config device remove 'box' 'pulse-cookie'"));
    }

    #[test]
    fn missing_pulse_socket_warns_and_adds_nothing() {
        let mut host = host_ready();
        host.pulse_available = false;
        let plan = build_audio_plan(AudioMode::Output, &host, 1000);
        assert!(plan.devices.is_empty());
        assert!(plan.env.is_empty());
        assert_eq!(plan.warnings.len(), 1);
        assert!(plan.warnings[0].contains("PulseAudio compatibility socket"));
        assert!(plan.warnings[0].contains("/run/user/1000/pulse/native"));
    }

    #[test]
    fn full_without_pipewire_falls_back_to_pulse_and_warns() {
        let mut host = host_ready();
        host.pipewire_available = false;
        let plan = build_audio_plan(AudioMode::Full, &host, 1000);
        assert!(device(&plan, "pulse").is_some());
        assert!(device(&plan, "pipewire").is_none());
        assert_eq!(env_value(&plan, "PIPEWIRE_REMOTE"), None);
        assert_eq!(plan.warnings.len(), 1);
        assert!(plan.warnings[0].contains("PipeWire socket"));
    }

    #[test]
    fn apply_script_prepares_directories_and_uses_container_mode() {
        let plan = build_audio_plan(AudioMode::Full, &host_ready(), 1000);
        let script = apply_audio_devices_script("box", &plan);
        let prep_at = script
            .find("incus exec 'box' -- mkdir -p '/run/user/1000/pulse' '/run/aibox' >/dev/null 2>&1 || true")
            .expect("audio prep");
        let remove_at = script.find("incus config device remove").expect("remove");
        let add_at = script.find("incus config device add").expect("add");
        assert!(prep_at < remove_at);
        assert!(remove_at < add_at);
        assert!(script.contains("mode='0666'"));
        assert!(script.contains("listen='unix:/run/user/1000/pulse/native'"));
        assert!(script.contains("listen='unix:/run/user/1000/pipewire-0'"));
        assert!(script.contains("path='/run/aibox/pulse-cookie'"));
        assert!(script.contains("readonly='true'"));
    }

    #[test]
    fn apply_script_drops_devices_missing_from_the_plan() {
        // Switching Full -> Output must remove the native PipeWire socket and
        // must not leave a stale cookie mount when the host cookie disappeared.
        let script = apply_audio_devices_script(
            "box",
            &build_audio_plan(AudioMode::Output, &host_ready(), 1000),
        );
        assert!(script.contains("incus config device remove 'box' 'pipewire'"));
        assert!(script.contains("incus config device remove 'box' 'pulse-cookie'"));
        assert!(!script.contains("incus config device add 'box' 'pipewire'"));
    }

    #[test]
    fn scripts_quote_names_and_paths() {
        let plan = build_audio_plan(AudioMode::Output, &host_ready(), 1000);
        let script = apply_audio_devices_script("it's a box; rm -rf /", &plan);
        assert!(script.contains("'it'\\''s a box; rm -rf /'"));
        assert!(!script.contains("; rm -rf /\n"));
    }

    #[test]
    fn remove_script_removes_every_audio_device() {
        let script = remove_audio_devices_script("box");
        assert!(script.contains("incus config device remove 'box' 'pulse'"));
        assert!(script.contains("incus config device remove 'box' 'pipewire'"));
        assert!(script.contains("incus config device remove 'box' 'pulse-cookie'"));
    }

    #[test]
    fn set_mode_script_applies_only_while_running_and_off_always_removes() {
        let host = host_ready();
        let output = set_audio_mode_script("box", AudioMode::Output, &host);
        assert!(output.contains("incus config set 'box' user.aibox.audio='output'"));
        assert!(output.contains("incus config device remove 'box' 'pipewire'"));
        assert!(output.contains("STATUS=$(incus info 'box' | awk '/^Status:/ {print $2}')"));
        let guard_at = output
            .find("if [[ \"$STATUS\" == \"RUNNING\" ]]; then")
            .expect("running guard");
        let stale_remove_at = output
            .find("incus config device remove 'box' 'pipewire'")
            .expect("stale pipewire removal");
        let add_at = output
            .find("incus config device add 'box' 'pulse' 'proxy'")
            .expect("pulse add");
        assert!(stale_remove_at < guard_at);
        assert!(guard_at < add_at);

        let off = set_audio_mode_script("box", AudioMode::Off, &host);
        assert!(off.contains("incus config set 'box' user.aibox.audio='off'"));
        assert!(off.contains("incus config device remove 'box' 'pulse'"));
        assert!(!off.contains("STATUS="));
    }

    #[test]
    fn test_audio_script_quotes_the_name_and_forwards_bridge_env() {
        let plan = build_audio_plan(AudioMode::Output, &host_ready(), 1000);
        let script = test_audio_script("it's a box", &plan);
        assert!(script.contains("incus exec 'it'\\''s a box' --env"));
        assert!(script.contains("PULSE_SERVER=unix:/run/user/1000/pulse/native"));
        assert!(script.contains("-- sh -c"));
        assert!(script.contains("pactl info"));
        assert!(script.contains("pactl is not installed in this environment."));

        let off = test_audio_script(
            "box",
            &build_audio_plan(AudioMode::Off, &host_ready(), 1000),
        );
        assert!(off.contains("Audio is off for this environment."));
        assert!(!off.contains("pactl"));
    }

    #[test]
    fn detection_defaults_runtime_dir_and_uses_existing_paths() {
        let existing = [
            "/run/user/4242/pulse/native",
            "/home/user/.config/pulse/cookie",
        ];
        let host = detect_host_audio_with(
            |key| match key {
                "HOME" => Some("/home/user".to_string()),
                _ => None,
            },
            |path| existing.contains(&path),
            4242,
            4243,
        );
        assert_eq!(host.runtime_dir, "/run/user/4242");
        assert_eq!(host.pulse_socket, "/run/user/4242/pulse/native");
        assert!(host.pulse_available);
        assert!(!host.pipewire_available);
        assert_eq!(host.pulse_cookie, "/home/user/.config/pulse/cookie");
    }

    #[test]
    fn detection_prefers_xdg_config_home_and_ignores_missing_cookie() {
        let host = detect_host_audio_with(
            |key| match key {
                "XDG_RUNTIME_DIR" => Some("/tmp/rt".to_string()),
                "XDG_CONFIG_HOME" => Some("/tmp/cfg".to_string()),
                "HOME" => Some("/home/user".to_string()),
                _ => None,
            },
            |_| false,
            1000,
            1000,
        );
        assert_eq!(host.runtime_dir, "/tmp/rt");
        assert!(!host.pulse_available);
        assert!(host.pulse_cookie.is_empty());
    }

    #[test]
    fn merged_environment_is_deduplicated() {
        let display = vec![("XDG_RUNTIME_DIR".to_string(), "/run/user/1000".to_string())];
        let audio = build_audio_plan(AudioMode::Output, &host_ready(), 1000).env;
        let merged = merge_env_pairs(&[&display, &audio]);
        assert_eq!(
            merged
                .iter()
                .filter(|(key, _)| key == "XDG_RUNTIME_DIR")
                .count(),
            1
        );
        assert!(merged.iter().any(|(key, _)| key == "PULSE_SERVER"));
    }

    #[test]
    fn generated_scripts_pass_bash_syntax_check() {
        let host = host_ready();
        let output = build_audio_plan(AudioMode::Output, &host, 1000);
        let full = build_audio_plan(AudioMode::Full, &host, 1000);
        let scripts = [
            apply_audio_devices_script("box", &output),
            apply_audio_devices_script("box", &full),
            remove_audio_devices_script("box"),
            set_audio_mode_script("box", AudioMode::Output, &host),
            set_audio_mode_script("box", AudioMode::Full, &host),
            set_audio_mode_script("box", AudioMode::Off, &host),
            test_audio_script("box", &output),
            test_audio_script("box", &build_audio_plan(AudioMode::Off, &host, 1000)),
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
