use crate::display::{
    detect_host_audio, detect_host_dbus, detect_host_devices, detect_host_session,
    set_audio_mode_script, set_dbus_mode_script, set_desktop_mode_script, set_extra_devices_script,
    AudioMode, DbusMode, DesktopMode, ExtraDeviceToggles,
};
use crate::incus::list::Environment;
use crate::network::ports::add_port_forward_script_with_bind;
use crate::provision::autostart::{set_environment_autostart_script, validate_autostart_options};
use crate::util::shell::shell_quote;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvManifest {
    pub name: String,
    pub os: String,
    pub preset: String,
    pub cpu: i32,
    pub memory: String,
    pub disk: String,
    pub swap: String,
    pub autostart: bool,
    pub autostart_priority: i32,
    pub autostart_delay: i32,
    pub desktop: bool,
    pub audio: String,
    pub dbus: bool,
    pub camera: bool,
    pub usb: bool,
    pub printer: bool,
    pub network_mode: String,
    pub network_allowlist: Vec<String>,
    pub secrets: Vec<String>,
    pub ports: Vec<String>,
    pub packages: Vec<String>,
}

impl Default for EnvManifest {
    fn default() -> Self {
        Self {
            name: String::new(),
            os: "archlinux".to_string(),
            preset: "gui".to_string(),
            cpu: 4,
            memory: "4GiB".to_string(),
            disk: "20GiB".to_string(),
            swap: "Off".to_string(),
            autostart: false,
            autostart_priority: 0,
            autostart_delay: 0,
            desktop: true,
            audio: "output".to_string(),
            dbus: true,
            camera: false,
            usb: false,
            printer: false,
            network_mode: "online".to_string(),
            network_allowlist: Vec::new(),
            secrets: Vec::new(),
            ports: Vec::new(),
            packages: Vec::new(),
        }
    }
}

/// Builds an [`EnvManifest`] snapshot from a live [`Environment`].
pub fn manifest_from_environment(env: &Environment) -> EnvManifest {
    let preset = if env.env_type.eq_ignore_ascii_case("VIRTUAL-MACHINE")
        || env.env_type.eq_ignore_ascii_case("VM")
    {
        "vm"
    } else if env.desktop_enabled || env.preset.to_ascii_lowercase().contains("desktop") {
        "gui"
    } else {
        "headless"
    };

    let allowlist = env
        .network_allowlist
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .collect();

    let ports = env
        .port_forwards
        .iter()
        .map(|p| {
            if p.lan_exposed {
                format!("0.0.0.0:{}:{}", p.host_port, p.guest_port)
            } else {
                format!("{}:{}", p.host_port, p.guest_port)
            }
        })
        .collect();

    EnvManifest {
        name: env.name.clone(),
        os: if env.os_name.is_empty() {
            "archlinux".to_string()
        } else {
            env.os_name.clone()
        },
        preset: preset.to_string(),
        cpu: env.cpu_cores.max(1),
        memory: if env.memory_limit.is_empty() || env.memory_limit == "Dynamic" {
            "4GiB".to_string()
        } else {
            env.memory_limit.clone()
        },
        disk: if env.disk_limit.is_empty() || env.disk_limit == "Shared" {
            "20GiB".to_string()
        } else {
            env.disk_limit.clone()
        },
        swap: if env.swap_limit.is_empty() {
            "Off".to_string()
        } else {
            env.swap_limit.clone()
        },
        autostart: env.boot_autostart,
        autostart_priority: env.boot_autostart_priority,
        autostart_delay: env.boot_autostart_delay,
        desktop: env.desktop_enabled,
        audio: if env.audio_mode.is_empty() {
            "off".to_string()
        } else {
            env.audio_mode.clone()
        },
        dbus: env.dbus_enabled,
        camera: env.camera_enabled,
        usb: env.usb_enabled,
        printer: env.printer_enabled,
        network_mode: if env.network_mode.is_empty() {
            "online".to_string()
        } else {
            env.network_mode.clone()
        },
        network_allowlist: allowlist,
        secrets: env.secret_names.clone(),
        ports,
        packages: Vec::new(),
    }
}

fn toml_quote(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn toml_str_array(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|i| toml_quote(i)).collect();
    format!("[{}]", quoted.join(", "))
}

/// Renders an [`EnvManifest`] into a declarative `aibox.toml` document.
pub fn render_manifest_toml(m: &EnvManifest) -> String {
    format!(
        "[environment]\n\
         name = {}\n\
         os = {}\n\
         preset = {}\n\
         packages = {}\n\
         \n\
         [resources]\n\
         cpu = {}\n\
         memory = {}\n\
         disk = {}\n\
         swap = {}\n\
         \n\
         [boot]\n\
         autostart = {}\n\
         priority = {}\n\
         delay = {}\n\
         \n\
         [integration]\n\
         desktop = {}\n\
         audio = {}\n\
         dbus = {}\n\
         camera = {}\n\
         usb = {}\n\
         printer = {}\n\
         \n\
         [network]\n\
         mode = {}\n\
         allowlist = {}\n\
         ports = {}\n\
         secrets = {}\n",
        toml_quote(&m.name),
        toml_quote(&m.os),
        toml_quote(&m.preset),
        toml_str_array(&m.packages),
        m.cpu,
        toml_quote(&m.memory),
        toml_quote(&m.disk),
        toml_quote(&m.swap),
        m.autostart,
        m.autostart_priority,
        m.autostart_delay,
        m.desktop,
        toml_quote(&m.audio),
        m.dbus,
        m.camera,
        m.usb,
        m.printer,
        toml_quote(&m.network_mode),
        toml_str_array(&m.network_allowlist),
        toml_str_array(&m.ports),
        toml_str_array(&m.secrets),
    )
}

fn strip_toml_comment(line: &str) -> &str {
    let mut in_quotes = false;
    let mut escaped = false;
    for (idx, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && in_quotes {
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_quotes = !in_quotes;
            continue;
        }
        if ch == '#' && !in_quotes {
            return &line[..idx];
        }
    }
    line
}

fn unquote_toml_str(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if let Some(inner) = trimmed.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        let mut out = String::with_capacity(inner.len());
        let mut chars = inner.chars();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                match chars.next() {
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('n') => out.push('\n'),
                    Some(other) => {
                        out.push('\\');
                        out.push(other);
                    }
                    None => return Err("Unterminated escape in string.".to_string()),
                }
            } else {
                out.push(ch);
            }
        }
        Ok(out)
    } else if let Some(inner) = trimmed
        .strip_prefix('\'')
        .and_then(|s| s.strip_suffix('\''))
    {
        Ok(inner.to_string())
    } else {
        Err(format!("Expected quoted string, got: {trimmed}"))
    }
}

fn parse_toml_bool(raw: &str) -> Result<bool, String> {
    match raw.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!("Expected boolean, got: {other}")),
    }
}

fn parse_toml_str_array(raw: &str) -> Result<Vec<String>, String> {
    let trimmed = raw.trim();
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(|| format!("Expected array [...], got: {trimmed}"))?
        .trim();
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;
    let mut escaped = false;
    for (idx, ch) in inner.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && in_quotes {
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_quotes = !in_quotes;
            continue;
        }
        if ch == ',' && !in_quotes {
            let part = inner[start..idx].trim();
            if !part.is_empty() {
                items.push(unquote_toml_str(part)?);
            }
            start = idx + 1;
        }
    }
    let tail = inner[start..].trim();
    if !tail.is_empty() {
        items.push(unquote_toml_str(tail)?);
    }
    Ok(items)
}

/// Parses an `aibox.toml` manifest string and validates required fields.
pub fn parse_manifest_toml(raw: &str) -> Result<EnvManifest, String> {
    let mut manifest = EnvManifest::default();
    let mut section = String::new();

    for (line_no, raw_line) in raw.lines().enumerate() {
        let line = strip_toml_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(sec) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            section = sec.trim().to_ascii_lowercase();
            continue;
        }
        let Some((key_raw, val_raw)) = line.split_once('=') else {
            return Err(format!("Line {}: expected key = value.", line_no + 1));
        };
        let key = key_raw.trim().to_ascii_lowercase();
        let val = val_raw.trim();

        match (section.as_str(), key.as_str()) {
            ("environment" | "", "name") => manifest.name = unquote_toml_str(val)?,
            ("environment" | "", "os") => manifest.os = unquote_toml_str(val)?,
            ("environment" | "", "preset") => manifest.preset = unquote_toml_str(val)?,
            ("environment" | "", "packages") => manifest.packages = parse_toml_str_array(val)?,
            ("resources", "cpu") => {
                manifest.cpu = val
                    .parse::<i32>()
                    .map_err(|_| format!("Invalid cpu integer: {val}"))?;
            }
            ("resources", "memory") => manifest.memory = unquote_toml_str(val)?,
            ("resources", "disk") => manifest.disk = unquote_toml_str(val)?,
            ("resources", "swap") => manifest.swap = unquote_toml_str(val)?,
            ("boot", "autostart") => manifest.autostart = parse_toml_bool(val)?,
            ("boot", "priority") => {
                manifest.autostart_priority = val
                    .parse::<i32>()
                    .map_err(|_| format!("Invalid priority integer: {val}"))?;
            }
            ("boot", "delay") => {
                manifest.autostart_delay = val
                    .parse::<i32>()
                    .map_err(|_| format!("Invalid delay integer: {val}"))?;
            }
            ("integration", "desktop") => manifest.desktop = parse_toml_bool(val)?,
            ("integration", "audio") => manifest.audio = unquote_toml_str(val)?,
            ("integration", "dbus") => manifest.dbus = parse_toml_bool(val)?,
            ("integration", "camera") => manifest.camera = parse_toml_bool(val)?,
            ("integration", "usb") => manifest.usb = parse_toml_bool(val)?,
            ("integration", "printer") => manifest.printer = parse_toml_bool(val)?,
            ("network", "mode") => manifest.network_mode = unquote_toml_str(val)?,
            ("network", "allowlist") => manifest.network_allowlist = parse_toml_str_array(val)?,
            ("network", "ports") => manifest.ports = parse_toml_str_array(val)?,
            ("network", "secrets") => manifest.secrets = parse_toml_str_array(val)?,
            _ => {
                return Err(format!(
                    "Line {}: unknown key '{key}' in section '[{section}]'.",
                    line_no + 1
                ));
            }
        }
    }

    if manifest.name.trim().is_empty() {
        return Err("Manifest [environment].name cannot be empty.".to_string());
    }
    if manifest.cpu < 1 {
        return Err("Manifest [resources].cpu must be at least 1.".to_string());
    }
    if !matches!(
        manifest.audio.trim().to_ascii_lowercase().as_str(),
        "off" | "output" | "full" | "mic"
    ) {
        return Err(format!(
            "Manifest [integration].audio must be off, output, or full (got '{}').",
            manifest.audio
        ));
    }
    if !matches!(
        manifest.network_mode.trim().to_ascii_lowercase().as_str(),
        "online" | "offline" | "allowlist"
    ) {
        return Err(format!(
            "Manifest [network].mode must be online, offline, or allowlist (got '{}').",
            manifest.network_mode
        ));
    }
    validate_autostart_options(manifest.autostart_priority, manifest.autostart_delay)?;

    Ok(manifest)
}

/// Parses a port spec like `"3000"`, `"3000:80"`, or `"0.0.0.0:3000:80"` into `(host_port, guest_port, bind_lan)`.
pub fn parse_manifest_port_spec(spec: &str) -> Result<(i32, i32, bool), String> {
    let parts: Vec<&str> = spec.trim().split(':').collect();
    match parts.as_slice() {
        [single] => {
            let p = single
                .parse::<i32>()
                .map_err(|_| format!("Invalid port spec: '{spec}'."))?;
            Ok((p, p, false))
        }
        [host, guest] => {
            let hp = host
                .parse::<i32>()
                .map_err(|_| format!("Invalid host port in '{spec}'."))?;
            let gp = guest
                .parse::<i32>()
                .map_err(|_| format!("Invalid guest port in '{spec}'."))?;
            Ok((hp, gp, false))
        }
        [bind, host, guest] => {
            let bind_lan = *bind == "0.0.0.0";
            if !bind_lan && *bind != "127.0.0.1" {
                return Err(format!("Unsupported bind address '{bind}' in '{spec}'."));
            }
            let hp = host
                .parse::<i32>()
                .map_err(|_| format!("Invalid host port in '{spec}'."))?;
            let gp = guest
                .parse::<i32>()
                .map_err(|_| format!("Invalid guest port in '{spec}'."))?;
            Ok((hp, gp, bind_lan))
        }
        _ => Err(format!("Invalid port spec: '{spec}'.")),
    }
}

/// Builds an idempotent reconciliation script applying boot, integration, port,
/// and package settings from `m`.
pub fn reconcile_manifest_script(m: &EnvManifest, existing_ports: &[i32]) -> String {
    let mut parts = Vec::new();
    parts.push("set -euo pipefail".to_string());
    parts.push(set_environment_autostart_script(
        &m.name,
        m.autostart,
        m.autostart_priority,
        m.autostart_delay,
    ));

    if !m.preset.eq_ignore_ascii_case("vm") {
        parts.push(set_desktop_mode_script(
            &m.name,
            DesktopMode::from_enabled(m.desktop),
            &detect_host_session(),
        ));
        parts.push(set_audio_mode_script(
            &m.name,
            AudioMode::parse(&m.audio),
            &detect_host_audio(),
        ));
        parts.push(set_dbus_mode_script(
            &m.name,
            DbusMode::from_enabled(m.dbus),
            &detect_host_dbus(),
        ));
        parts.push(set_extra_devices_script(
            &m.name,
            &ExtraDeviceToggles::from_envs(m.camera, m.usb, m.printer),
            &detect_host_devices(),
        ));
    }

    for spec in &m.ports {
        if let Ok((hp, gp, bind_lan)) = parse_manifest_port_spec(spec) {
            if !existing_ports.contains(&hp) {
                parts.push(add_port_forward_script_with_bind(&m.name, hp, gp, bind_lan));
            }
        }
    }

    if !m.packages.is_empty() {
        let q_name = shell_quote(&m.name);
        let q_pkgs: Vec<String> = m.packages.iter().map(|p| shell_quote(p)).collect();
        let pkg_list = q_pkgs.join(" ");
        let inner = format!(
            "if command -v pacman >/dev/null 2>&1; then \
               pacman -S --noconfirm --needed {pkg_list}; \
             elif command -v apt-get >/dev/null 2>&1; then \
               apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y {pkg_list}; \
             elif command -v apk >/dev/null 2>&1; then \
               apk add --no-cache {pkg_list}; \
             fi"
        );
        parts.push(format!(
            "incus exec {q_name} -- sh -c {}",
            shell_quote(&inner)
        ));
    }

    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_toml_round_trips_and_validates() {
        let mut m = EnvManifest {
            name: "srv-api".to_string(),
            os: "archlinux".to_string(),
            preset: "gui".to_string(),
            cpu: 4,
            memory: "8GiB".to_string(),
            disk: "40GiB".to_string(),
            swap: "Off".to_string(),
            autostart: true,
            autostart_priority: 20,
            autostart_delay: 5,
            desktop: false,
            audio: "off".to_string(),
            dbus: false,
            camera: false,
            usb: false,
            printer: false,
            network_mode: "allowlist".to_string(),
            network_allowlist: vec!["github.com".to_string(), "1.1.1.1".to_string()],
            secrets: vec!["GITHUB_TOKEN".to_string()],
            ports: vec!["3000:8000".to_string(), "0.0.0.0:8080:80".to_string()],
            packages: vec!["nginx".to_string(), "curl".to_string()],
        };
        let toml = render_manifest_toml(&m);
        let parsed = parse_manifest_toml(&toml).expect("parse round-trip");
        assert_eq!(parsed, m);

        m.name.clear();
        let bad_toml = render_manifest_toml(&m);
        assert!(parse_manifest_toml(&bad_toml).is_err());

        assert!(
            parse_manifest_toml("[environment]\nname = \"x\"\n[network]\nmode = \"bogus\"")
                .is_err()
        );
        assert!(parse_manifest_toml(
            "[environment]\nname = \"x\"\n[integration]\naudio = \"bogus\""
        )
        .is_err());
    }

    #[test]
    fn reconcile_script_skips_existing_ports_and_installs_packages() {
        let m = EnvManifest {
            name: "srv-api".to_string(),
            autostart: true,
            ports: vec!["3000:8000".to_string(), "8080:80".to_string()],
            packages: vec!["nginx".to_string()],
            ..EnvManifest::default()
        };
        let script = reconcile_manifest_script(&m, &[3000]);
        assert!(script.contains("boot.autostart=true"));
        assert!(!script.contains("fwd-3000"));
        assert!(script.contains("fwd-8080"));
        assert!(script.contains("pacman -S --noconfirm --needed"));
        assert!(script.contains("nginx"));
    }
}
