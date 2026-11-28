//! Filtered host D-Bus session for containers.
//!
//! A container cannot use the host session bus directly: the container's root
//! UID differs from the host user, and the host bus rejects the container's
//! `AUTH EXTERNAL` line because Incus connects on its behalf as root. The
//! bridge therefore runs two host processes per environment:
//!
//! 1. `xdg-dbus-proxy` connects to the real session bus and exposes a private
//!    socket that only allows notifications, portals, and the tray watcher.
//! 2. `aibox --dbus-auth-proxy` accepts the container connection and rewrites
//!    the D-Bus auth lines to the host user before forwarding to the filtered
//!    socket (see [`crate::display::dbus_auth_proxy`]).
//!
//! The plan is deliberately self-contained so it can be unit-tested without a
//! live Incus daemon or a running session bus.

use std::env;
use std::path::Path;

use crate::display::dbus_auth_proxy::parse_bus_path;
use crate::display::plan::{device_options, DeviceSpec};
use crate::util::shell::shell_quote;

/// Per-environment host service switch (`user.aibox.dbus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbusMode {
    Off,
    On,
}

impl DbusMode {
    /// Parses the stored config value; empty or unknown values default to Off.
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "on" | "true" | "1" => Self::On,
            _ => Self::Off,
        }
    }

    /// Stable value written to `user.aibox.dbus`.
    pub fn config_value(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
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

/// Host-side details needed to start and validate the two proxy processes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostDbus {
    pub uid: u32,
    pub gid: u32,
    pub runtime_dir: String,
    pub bus_address: String,
    /// Resolved `aibox` binary used for `--dbus-auth-proxy`.
    pub aibox_bin: String,
    /// Resolved `xdg-dbus-proxy`; empty when it is not installed.
    pub xdg_dbus_proxy_bin: String,
}

/// Everything needed to expose the filtered host bus inside one environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbusPlan {
    pub enabled: bool,
    pub uid: u32,
    /// Private per-environment state directory under `$XDG_RUNTIME_DIR`.
    pub dir: String,
    pub bus_address: String,
    pub auth_socket: String,
    pub filter_socket: String,
    pub auth_pid: String,
    pub filter_pid: String,
    pub aibox_bin: String,
    pub xdg_dbus_proxy_bin: String,
    pub devices: Vec<DeviceSpec>,
    pub env: Vec<(String, String)>,
    pub warnings: Vec<String>,
    pub stop_script: String,
}

/// Well-known names the filtered proxy may talk to.
///
/// Secrets are intentionally absent; `org.freedesktop.secrets` must stay
/// unreachable from the container.
pub const DBUS_TALK_FILTERS: [&str; 4] = [
    "org.freedesktop.Notifications",
    "org.freedesktop.portal.Desktop",
    "org.freedesktop.portal.Documents",
    "org.kde.StatusNotifierWatcher",
];

const DBUS_DEVICE: &str = "dbus";

/// Reads the process environment and OS user IDs.
pub fn detect_host_dbus() -> HostDbus {
    // SAFETY: getuid/getgid are always safe to call and do not touch memory.
    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };
    let current_exe = env::current_exe()
        .ok()
        .map(|path| path.to_string_lossy().to_string())
        .filter(|path| !path.is_empty());
    detect_host_dbus_with(
        |key| env::var(key).ok().filter(|value| !value.trim().is_empty()),
        |path| Path::new(path).exists(),
        find_binary,
        uid,
        gid,
        current_exe,
    )
}

/// Pure host detector; callers inject environment, filesystem, PATH, and IDs.
pub fn detect_host_dbus_with(
    get: impl Fn(&str) -> Option<String>,
    exists: impl Fn(&str) -> bool,
    find: impl Fn(&str) -> Option<String>,
    uid: u32,
    gid: u32,
    current_exe: Option<String>,
) -> HostDbus {
    let runtime_dir = get("XDG_RUNTIME_DIR")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("/run/user/{uid}"));

    let bus_address = get("DBUS_SESSION_BUS_ADDRESS")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("unix:path={runtime_dir}/bus"));

    let xdg_dbus_proxy_bin = find("xdg-dbus-proxy")
        .or_else(|| {
            exists("/usr/bin/xdg-dbus-proxy").then(|| "/usr/bin/xdg-dbus-proxy".to_string())
        })
        .unwrap_or_default();

    let aibox_bin = current_exe
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .or_else(|| find("aibox"))
        .unwrap_or_else(|| "aibox".to_string());

    HostDbus {
        uid,
        gid,
        runtime_dir,
        bus_address,
        aibox_bin,
        xdg_dbus_proxy_bin,
    }
}

/// Finds an executable on `PATH`.
pub fn find_binary(name: &str) -> Option<String> {
    use std::os::unix::fs::PermissionsExt;

    let path_var = env::var_os("PATH")?;
    for dir in env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            if let Ok(metadata) = std::fs::metadata(&candidate) {
                if metadata.permissions().mode() & 0o111 != 0 {
                    return Some(candidate.to_string_lossy().to_string());
                }
            }
        }
    }
    None
}

/// Builds the filtered-bus plan for one environment.
pub fn build_dbus_plan(mode: DbusMode, host: &HostDbus, env_name: &str) -> DbusPlan {
    let (dir, auth_socket, filter_socket, auth_pid, filter_pid) = bridge_paths(host, env_name);
    let mut plan = DbusPlan {
        enabled: mode == DbusMode::On,
        uid: host.uid,
        dir,
        bus_address: host.bus_address.clone(),
        auth_socket,
        filter_socket,
        auth_pid,
        filter_pid,
        aibox_bin: host.aibox_bin.clone(),
        xdg_dbus_proxy_bin: host.xdg_dbus_proxy_bin.clone(),
        devices: Vec::new(),
        env: Vec::new(),
        warnings: Vec::new(),
        stop_script: String::new(),
    };

    if mode == DbusMode::Off {
        return plan;
    }

    plan.stop_script = stop_dbus_bridge_script_with(env_name, host);

    if host.xdg_dbus_proxy_bin.trim().is_empty() {
        plan.warnings.push(
            "Host services are on, but xdg-dbus-proxy is not installed; install xdg-dbus-proxy on the host to enable notifications and portals."
                .to_string(),
        );
        return plan;
    }
    if let Err(error) = parse_bus_path(&host.bus_address) {
        plan.warnings.push(format!(
            "Host services are on, but the session bus address cannot be proxied: {error}"
        ));
        return plan;
    }

    let container_runtime_dir = format!("/run/user/{}", host.uid);
    plan.devices.push(DeviceSpec {
        name: DBUS_DEVICE.to_string(),
        kind: "proxy".to_string(),
        options: vec![
            ("connect".to_string(), format!("unix:{}", plan.auth_socket)),
            (
                "listen".to_string(),
                format!("unix:{container_runtime_dir}/bus"),
            ),
            ("bind".to_string(), "container".to_string()),
            ("mode".to_string(), "0666".to_string()),
        ],
    });
    // The container talks to the host bus through a stable user runtime path;
    // the display and audio plans already use the same convention.
    plan.env.extend([
        (
            "DBUS_SESSION_BUS_ADDRESS".to_string(),
            format!("unix:path={container_runtime_dir}/bus"),
        ),
        ("XDG_RUNTIME_DIR".to_string(), container_runtime_dir),
    ]);

    plan
}

/// Returns the bridge environment pairs, empty when the bridge is off.
pub fn dbus_env(plan: &DbusPlan) -> Vec<(String, String)> {
    if plan.enabled {
        plan.env.clone()
    } else {
        Vec::new()
    }
}

/// Returns the socket and pid paths for one environment.
fn bridge_paths(host: &HostDbus, env_name: &str) -> (String, String, String, String, String) {
    let dir = format!(
        "{}/aibox-dbus/{}",
        host.runtime_dir.trim_end_matches('/'),
        sanitize_env_name(env_name)
    );
    let auth_socket = format!("{dir}/auth.sock");
    let filter_socket = format!("{dir}/filter.sock");
    let auth_pid = format!("{dir}/auth.pid");
    let filter_pid = format!("{dir}/filter.pid");
    (dir, auth_socket, filter_socket, auth_pid, filter_pid)
}

/// Replaces unsafe characters so an environment name cannot escape its state
/// directory or introduce a path separator.
pub fn sanitize_env_name(name: &str) -> String {
    let mut sanitized: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        sanitized.push_str("env");
    }
    if sanitized == "." || sanitized == ".." {
        sanitized = "_".repeat(sanitized.len());
    }
    sanitized
}

fn filter_arguments() -> String {
    DBUS_TALK_FILTERS
        .iter()
        .map(|name| format!("--talk={name}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Starts/reuses both proxies and installs the container's `dbus` device.
///
/// The filtered proxy starts first because the auth proxy connects to it per
/// accepted client. Both processes are idempotent: a healthy socket plus a live
/// PID file is reused, while stale files are removed before restarting.
pub fn apply_dbus_devices_script(name: &str, plan: &DbusPlan) -> String {
    if !plan.enabled || plan.devices.is_empty() {
        return String::new();
    }

    let q_name = shell_quote(name);
    let q_dir = shell_quote(&plan.dir);
    let q_auth = shell_quote(&plan.auth_socket);
    let q_filter = shell_quote(&plan.filter_socket);
    let q_auth_pid = shell_quote(&plan.auth_pid);
    let q_filter_pid = shell_quote(&plan.filter_pid);
    let q_bus = shell_quote(&plan.bus_address);
    let q_aibox = shell_quote(&plan.aibox_bin);
    let q_xdg = shell_quote(&plan.xdg_dbus_proxy_bin);
    let q_container_runtime = shell_quote(&format!("/run/user/{}", plan.uid));

    let filter_command = format!(
        "setsid {q_xdg} {q_bus} {q_filter} --filter {filters}",
        filters = filter_arguments()
    );
    let auth_command = format!("setsid {q_aibox} --dbus-auth-proxy {q_auth} {q_filter}");

    let mut lines = vec![
        format!("mkdir -p {q_dir} >/dev/null 2>&1 || true"),
        format!("chmod 700 {q_dir} 2>/dev/null || true"),
        format!("incus exec {q_name} -- mkdir -p {q_container_runtime} >/dev/null 2>&1 || true"),
        start_or_reuse(&q_filter, &q_filter_pid, &filter_command),
        start_or_reuse(&q_auth, &q_auth_pid, &auth_command),
        // The directory is 0700; chmod both sockets as defence in depth and to
        // reset the filtered proxy's default permissions after a restart.
        format!("chmod 600 {q_auth} {q_filter} 2>/dev/null || true"),
        format!(
            "incus config device remove {q_name} {device} >/dev/null 2>&1 || true",
            device = shell_quote(DBUS_DEVICE)
        ),
    ];

    for device in &plan.devices {
        let options = device_options(&device.options);
        // D-Bus is an optional convenience: a proxy startup failure must not
        // abort Start/Enter for an otherwise healthy environment.
        lines.push(format!(
            "incus config device add {q_name} {} {} {options} >/dev/null 2>&1 || echo {} >&2",
            shell_quote(&device.name),
            shell_quote(&device.kind),
            shell_quote(
                "Warning: host D-Bus bridge could not attach; notifications and portals stay unavailable."
            )
        ));
    }

    lines.join("\n")
}

fn start_or_reuse(socket: &str, pid_file: &str, command: &str) -> String {
    format!(
        "if [ -S {socket} ] && [ -f {pid_file} ] && kill -0 \"$(cat {pid_file} 2>/dev/null)\" 2>/dev/null; then\n    :\nelse\n    if [ -f {pid_file} ]; then\n        OLD_PID=$(cat {pid_file} 2>/dev/null || true)\n        if [ -n \"$OLD_PID\" ] && grep -qaE 'xdg-dbus-proxy|aibox' \"/proc/$OLD_PID/cmdline\" 2>/dev/null; then\n            kill \"$OLD_PID\" 2>/dev/null || true\n        fi\n    fi\n    rm -f {socket} {pid_file}\n    {command} >/dev/null 2>&1 &\n    PID=$!\n    echo \"$PID\" > {pid_file}\n    DBUS_WAIT=0\n    while [ ! -S {socket} ] && [ \"$DBUS_WAIT\" -lt 50 ] && kill -0 \"$PID\" 2>/dev/null; do\n        DBUS_WAIT=$((DBUS_WAIT + 1))\n        sleep 0.1\n    done\nfi"
    )
}

/// Removes both proxy processes, their sockets, and the container device.
pub fn stop_dbus_bridge_script(name: &str) -> String {
    stop_dbus_bridge_script_with(name, &detect_host_dbus())
}

pub fn stop_dbus_bridge_script_with(name: &str, host: &HostDbus) -> String {
    let (dir, auth_socket, filter_socket, auth_pid, filter_pid) = bridge_paths(host, name);
    let q_name = shell_quote(name);
    let q_dir = shell_quote(&dir);
    let q_auth = shell_quote(&auth_socket);
    let q_filter = shell_quote(&filter_socket);
    let q_auth_pid = shell_quote(&auth_pid);
    let q_filter_pid = shell_quote(&filter_pid);

    format!(
        "AUTH_PID=\"\"\nFILTER_PID=\"\"\nif [ -f {q_auth_pid} ]; then\n    CAND=$(cat {q_auth_pid} 2>/dev/null || true)\n    if [ -n \"$CAND\" ] && grep -qa 'aibox' \"/proc/$CAND/cmdline\" 2>/dev/null; then AUTH_PID=\"$CAND\"; fi\nfi\nif [ -f {q_filter_pid} ]; then\n    CAND=$(cat {q_filter_pid} 2>/dev/null || true)\n    if [ -n \"$CAND\" ] && grep -qa 'xdg-dbus-proxy' \"/proc/$CAND/cmdline\" 2>/dev/null; then FILTER_PID=\"$CAND\"; fi\nfi\nif [ -n \"$AUTH_PID\" ]; then kill \"$AUTH_PID\" 2>/dev/null || true; fi\nif [ -n \"$FILTER_PID\" ]; then kill \"$FILTER_PID\" 2>/dev/null || true; fi\nif [ -n \"$AUTH_PID$FILTER_PID\" ]; then\n    sleep 0.2\n    if [ -n \"$AUTH_PID\" ]; then kill -9 \"$AUTH_PID\" 2>/dev/null || true; fi\n    if [ -n \"$FILTER_PID\" ]; then kill -9 \"$FILTER_PID\" 2>/dev/null || true; fi\nfi\nrm -f {q_auth} {q_filter} {q_auth_pid} {q_filter_pid}\nrmdir {q_dir} 2>/dev/null || true\nincus config device remove {q_name} {device} >/dev/null 2>&1 || true",
        device = shell_quote(DBUS_DEVICE)
    )
}

/// Removes the bridge on host-service disable, even while the instance is
/// stopped. Kept separate from `stop` so callers can express intent.
pub fn remove_dbus_bridge_script(name: &str) -> String {
    stop_dbus_bridge_script(name)
}

/// Writes the mode and applies or tears down the bridge.
pub fn set_dbus_mode_script(name: &str, mode: DbusMode, host: &HostDbus) -> String {
    let q_name = shell_quote(name);
    let mut lines = vec![
        "set -euo pipefail".to_string(),
        format!(
            "incus config set {q_name} user.aibox.dbus={}",
            shell_quote(mode.config_value())
        ),
    ];

    if mode == DbusMode::Off {
        lines.push(stop_dbus_bridge_script_with(name, host));
        return lines.join("\n");
    }

    let plan = build_dbus_plan(mode, host, name);
    for warning in &plan.warnings {
        lines.push(format!("echo {} >&2", shell_quote(warning)));
    }
    let apply = apply_dbus_devices_script(name, &plan);
    if !apply.is_empty() {
        // A stopped instance cannot accept a container listen socket; the next
        // Start/Enter applies the devices again.
        lines.push(format!(
            "STATUS=$(incus info {q_name} | awk '/^Status:/ {{print $2}}')"
        ));
        lines.push("if [[ \"$STATUS\" == \"RUNNING\" ]]; then".to_string());
        lines.push(apply);
        lines.push("fi".to_string());
    }
    lines.join("\n")
}

/// Builds the in-container D-Bus self-test used by the GUI button.
///
/// The probes use the same bridge environment as Enter/exec and check that the
/// portal and notification services answer while `org.freedesktop.secrets`
/// stays blocked.
pub fn dbus_test_script(name: &str, plan: &DbusPlan) -> String {
    if !plan.enabled {
        return format!(
            "echo {} >&2\nexit 1",
            shell_quote("Host services are off for this environment. Enable Filtered bus first.")
        );
    }
    if plan.env.is_empty() {
        let msg = if plan.warnings.is_empty() {
            "Host D-Bus bridge is unavailable on this host.".to_string()
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
    let probe = "fail=0\n\
if ! command -v gdbus >/dev/null 2>&1; then\n    echo \"gdbus is not installed in this environment.\"\n    exit 1\nfi\n\
probe() {\n    if gdbus call --session --dest \"$1\" --object-path \"$2\" --method org.freedesktop.DBus.Peer.Ping >/dev/null 2>&1; then\n        echo \"$1: reachable\"\n    else\n        echo \"$1: unreachable\"\n        fail=1\n    fi\n}\n\
probe org.freedesktop.portal.Desktop /org/freedesktop/portal/desktop\n\
probe org.freedesktop.Notifications /org/freedesktop/Notifications\n\
if gdbus call --session --dest org.freedesktop.secrets --object-path /org/freedesktop/secrets --method org.freedesktop.DBus.Peer.Ping >/dev/null 2>&1; then\n    echo \"org.freedesktop.secrets: VISIBLE (unexpected)\"\n    fail=1\nelse\n    echo \"org.freedesktop.secrets: blocked\"\nfi\n\
exit $fail";
    cmd.push_str(&format!(" -- sh -c {}", shell_quote(probe)));
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn host_ready() -> HostDbus {
        HostDbus {
            uid: 1000,
            gid: 1000,
            runtime_dir: "/run/user/1000".to_string(),
            bus_address: "unix:path=/run/user/1000/bus".to_string(),
            aibox_bin: "/home/user/.local/bin/aibox".to_string(),
            xdg_dbus_proxy_bin: "/usr/bin/xdg-dbus-proxy".to_string(),
        }
    }

    fn env_value<'a>(plan: &'a DbusPlan, key: &str) -> Option<&'a str> {
        plan.env
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }

    #[test]
    fn dbus_mode_parses_stable_values() {
        assert_eq!(DbusMode::parse("on"), DbusMode::On);
        assert_eq!(DbusMode::parse("true"), DbusMode::On);
        assert_eq!(DbusMode::parse("1"), DbusMode::On);
        assert_eq!(DbusMode::parse("off"), DbusMode::Off);
        assert_eq!(DbusMode::parse("false"), DbusMode::Off);
        assert_eq!(DbusMode::parse("0"), DbusMode::Off);
        assert_eq!(DbusMode::parse(""), DbusMode::Off);
        assert_eq!(DbusMode::parse("bogus"), DbusMode::Off);
        assert_eq!(DbusMode::from_enabled(true), DbusMode::On);
        assert_eq!(DbusMode::from_enabled(false), DbusMode::Off);
        assert_eq!(DbusMode::On.config_value(), "on");
        assert_eq!(DbusMode::Off.config_value(), "off");
    }

    #[test]
    fn off_has_no_devices_environment_or_stop_script() {
        let plan = build_dbus_plan(DbusMode::Off, &host_ready(), "box");
        assert!(!plan.enabled);
        assert!(plan.devices.is_empty());
        assert!(plan.env.is_empty());
        assert!(plan.warnings.is_empty());
        assert!(plan.stop_script.is_empty());
        assert_eq!(apply_dbus_devices_script("box", &plan), "");
        assert!(dbus_env(&plan).is_empty());
    }

    #[test]
    fn on_exposes_a_filtered_proxy_device_and_environment() {
        let plan = build_dbus_plan(DbusMode::On, &host_ready(), "box");
        assert!(plan.enabled);
        assert!(plan.warnings.is_empty());
        assert_eq!(plan.devices.len(), 1);

        let device = &plan.devices[0];
        assert_eq!(device.name, "dbus");
        assert_eq!(device.kind, "proxy");
        assert_eq!(
            device.options,
            vec![
                (
                    "connect".to_string(),
                    "unix:/run/user/1000/aibox-dbus/box/auth.sock".to_string()
                ),
                ("listen".to_string(), "unix:/run/user/1000/bus".to_string()),
                ("bind".to_string(), "container".to_string()),
                ("mode".to_string(), "0666".to_string()),
            ]
        );
        assert_eq!(
            env_value(&plan, "DBUS_SESSION_BUS_ADDRESS"),
            Some("unix:path=/run/user/1000/bus")
        );
        assert_eq!(env_value(&plan, "XDG_RUNTIME_DIR"), Some("/run/user/1000"));
        assert!(!plan.stop_script.is_empty());
    }

    #[test]
    fn paths_use_a_sanitized_per_environment_directory() {
        let plan = build_dbus_plan(DbusMode::On, &host_ready(), "it's a box/../x");
        assert_eq!(plan.dir, "/run/user/1000/aibox-dbus/it_s_a_box_.._x");
        assert!(plan.auth_socket.ends_with("/auth.sock"));
        assert!(plan.filter_socket.ends_with("/filter.sock"));

        for name in ["", ".", "..", "../../etc"] {
            let sanitized = sanitize_env_name(name);
            assert!(!sanitized.is_empty(), "name {name}");
            assert!(!sanitized.contains('/'), "name {name}");
            assert!(sanitized != "." && sanitized != "..", "name {name}");
        }
    }

    #[test]
    fn missing_xdg_proxy_warns_and_adds_nothing() {
        let mut host = host_ready();
        host.xdg_dbus_proxy_bin = String::new();
        let plan = build_dbus_plan(DbusMode::On, &host, "box");
        assert!(plan.enabled);
        assert!(plan.devices.is_empty());
        assert!(plan.env.is_empty());
        assert_eq!(plan.warnings.len(), 1);
        assert!(plan.warnings[0].contains("xdg-dbus-proxy is not installed"));
        assert_eq!(apply_dbus_devices_script("box", &plan), "");
    }

    #[test]
    fn unsupported_bus_address_warns_and_adds_nothing() {
        let mut host = host_ready();
        host.bus_address = "tcp:host=127.0.0.1,port=4444".to_string();
        let plan = build_dbus_plan(DbusMode::On, &host, "box");
        assert!(plan.devices.is_empty());
        assert_eq!(plan.warnings.len(), 1);
        assert!(plan.warnings[0].contains("cannot be proxied"));
    }

    #[test]
    fn apply_script_starts_the_filtered_proxy_before_the_auth_proxy() {
        let plan = build_dbus_plan(DbusMode::On, &host_ready(), "box");
        let script = apply_dbus_devices_script("box", &plan);
        let filter_start = script
            .find("--talk=org.freedesktop.Notifications")
            .expect("filter start");
        let auth_start = script.find("--dbus-auth-proxy").expect("auth proxy start");
        let remove_at = script.find("incus config device remove").expect("remove");
        let add_at = script.find("incus config device add").expect("add");
        assert!(filter_start < auth_start);
        assert!(auth_start < remove_at);
        assert!(remove_at < add_at);
        assert!(script.contains("--filter --talk=org.freedesktop.Notifications"));
        assert!(script.contains("chmod 600"));
        assert!(script.contains("mkdir -p '/run/user/1000/aibox-dbus/box'"));
        assert!(script.contains("chmod 700 '/run/user/1000/aibox-dbus/box'"));
        assert!(script.contains("incus exec 'box' -- mkdir -p '/run/user/1000'"));
    }

    #[test]
    fn apply_script_is_idempotent_and_uses_the_allowlist() {
        let plan = build_dbus_plan(DbusMode::On, &host_ready(), "box");
        let script = apply_dbus_devices_script("box", &plan);
        assert!(script.contains("[ -S '/run/user/1000/aibox-dbus/box/filter.sock' ]"));
        assert!(script.contains("kill -0 \"$(cat"));
        assert!(script.contains("rm -f '/run/user/1000/aibox-dbus/box/filter.sock'"));
        for name in DBUS_TALK_FILTERS {
            assert!(script.contains(&format!("--talk={name}")), "filter {name}");
        }
        assert!(!script.contains("org.freedesktop.secrets"));
    }

    #[test]
    fn stop_script_kills_both_processes_and_removes_the_device() {
        let script = stop_dbus_bridge_script_with("box", &host_ready());
        assert!(script.contains("if [ -f '/run/user/1000/aibox-dbus/box/auth.pid' ]"));
        assert!(script.contains("if [ -f '/run/user/1000/aibox-dbus/box/filter.pid' ]"));
        assert!(script.contains("/proc/$CAND/cmdline"));
        assert!(script.contains("kill -9 \"$AUTH_PID\""));
        assert!(script.contains("kill -9 \"$FILTER_PID\""));
        assert!(script.contains("rm -f '/run/user/1000/aibox-dbus/box/auth.sock'"));
        assert!(script.contains("incus config device remove 'box' 'dbus'"));
        assert!(!script.contains("incus exec"));
    }

    #[test]
    fn set_mode_script_guards_apply_and_off_always_tears_down() {
        let host = host_ready();
        let on = set_dbus_mode_script("box", DbusMode::On, &host);
        assert!(on.contains("incus config set 'box' user.aibox.dbus='on'"));
        assert!(on.contains("STATUS=$(incus info 'box' | awk '/^Status:/ {print $2}')"));
        let guard_at = on
            .find("if [[ \"$STATUS\" == \"RUNNING\" ]]; then")
            .expect("running guard");
        let add_at = on
            .find("incus config device add 'box' 'dbus' 'proxy'")
            .expect("dbus add");
        assert!(guard_at < add_at);

        let off = set_dbus_mode_script("box", DbusMode::Off, &host);
        assert!(off.contains("incus config set 'box' user.aibox.dbus='off'"));
        assert!(off.contains("kill \"$AUTH_PID\""));
        assert!(off.contains("incus config device remove 'box' 'dbus'"));
        assert!(!off.contains("STATUS="));
    }

    #[test]
    fn test_script_probes_portals_notifications_and_secrets() {
        let plan = build_dbus_plan(DbusMode::On, &host_ready(), "box");
        let script = dbus_test_script("it's a box", &plan);
        assert!(script.contains("incus exec 'it'\\''s a box' --env"));
        assert!(script.contains("DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus"));
        assert!(script.contains("org.freedesktop.portal.Desktop"));
        assert!(script.contains("org.freedesktop.Notifications"));
        assert!(script.contains("org.freedesktop.secrets: blocked"));
        assert!(script.contains("gdbus is not installed"));

        let off = dbus_test_script("box", &build_dbus_plan(DbusMode::Off, &host_ready(), "box"));
        assert!(off.contains("Host services are off for this environment."));
        assert!(!off.contains("gdbus"));
    }

    #[test]
    fn detection_defaults_runtime_dir_and_bus_address() {
        let host = detect_host_dbus_with(
            |_| None,
            |path| path == "/usr/bin/xdg-dbus-proxy",
            |_| None,
            4242,
            4243,
            Some("/home/user/.local/bin/aibox".to_string()),
        );
        assert_eq!(host.runtime_dir, "/run/user/4242");
        assert_eq!(host.bus_address, "unix:path=/run/user/4242/bus");
        assert_eq!(host.xdg_dbus_proxy_bin, "/usr/bin/xdg-dbus-proxy");
        assert_eq!(host.aibox_bin, "/home/user/.local/bin/aibox");
        assert_eq!(host.gid, 4243);
    }

    #[test]
    fn detection_prefers_environment_and_path_lookups() {
        let host = detect_host_dbus_with(
            |key| match key {
                "XDG_RUNTIME_DIR" => Some("/tmp/rt".to_string()),
                "DBUS_SESSION_BUS_ADDRESS" => Some("unix:path=/tmp/bus".to_string()),
                _ => None,
            },
            |_| false,
            |name| (name == "xdg-dbus-proxy").then(|| "/opt/xdg-dbus-proxy".to_string()),
            1000,
            1000,
            None,
        );
        assert_eq!(host.runtime_dir, "/tmp/rt");
        assert_eq!(host.bus_address, "unix:path=/tmp/bus");
        assert_eq!(host.xdg_dbus_proxy_bin, "/opt/xdg-dbus-proxy");
        assert_eq!(host.aibox_bin, "aibox");
    }

    #[test]
    fn generated_scripts_pass_bash_syntax_check() {
        let host = host_ready();
        let on = build_dbus_plan(DbusMode::On, &host, "box");
        let off = build_dbus_plan(DbusMode::Off, &host, "box");
        let scripts = [
            apply_dbus_devices_script("box", &on),
            stop_dbus_bridge_script_with("box", &host),
            set_dbus_mode_script("box", DbusMode::On, &host),
            set_dbus_mode_script("box", DbusMode::Off, &host),
            dbus_test_script("box", &on),
            dbus_test_script("box", &off),
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
