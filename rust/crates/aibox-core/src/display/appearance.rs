//! Host appearance bridge: fonts, icons, GTK themes, and cursor settings.
//!
//! Programs inside a GUI environment look native when they can see the host
//! font, icon, and theme directories and when they inherit the cursor and GTK
//! theme environment. The directories are mounted read-only, so an environment
//! can never rewrite the host's theme files, and every source is optional
//! because minimal hosts legitimately ship only `/usr/share/fonts`.
//!
//! VMs and headless environments are out of scope: the desktop integration
//! switch in [`crate::display::plan`] decides whether the plan is used at all.

use std::env;
use std::fs;
use std::path::Path;

use crate::display::plan::{device_options, remove_device_lines, DeviceSpec};
use crate::util::paths::home_dir;
use crate::util::shell::shell_quote;

/// The container-side home directory that receives the user appearance dirs.
///
/// Omarchy containers run programs as root, so read-only host user directories
/// are mapped into `/root` instead of mirroring the host user name. Non-root
/// container users keep their own home and see only the system directories.
const CONTAINER_HOME: &str = "/root";

/// Stable device names, also used to remove every appearance mount.
pub const APPEARANCE_DEVICE_NAMES: [&str; 10] = [
    "fonts-system",
    "fonts-user",
    "fonts-legacy",
    "icons-system",
    "icons-user",
    "icons-legacy",
    "themes-system",
    "themes-user",
    "gtk3-config",
    "gtk4-config",
];

/// A host appearance directory that exists and can be mounted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppearanceSource {
    pub name: String,
    pub source: String,
    pub target: String,
}

/// GTK settings parsed from `~/.config/gtk-3.0/settings.ini`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GtkSettings {
    pub theme_name: Option<String>,
    pub icon_theme_name: Option<String>,
    pub cursor_theme_name: Option<String>,
    /// Raw size value; [`build_appearance_plan`] validates it before use.
    pub cursor_theme_size: Option<String>,
    pub prefer_dark: bool,
}

/// Everything the appearance bridge reads from the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostAppearance {
    pub home: String,
    /// Only directories that exist on the host.
    pub sources: Vec<AppearanceSource>,
    pub gtk: GtkSettings,
    pub host_cursor_theme: Option<String>,
    pub host_cursor_size: Option<String>,
    pub host_gtk_theme: Option<String>,
}

/// Everything needed to make graphical programs match the host appearance.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppearancePlan {
    pub devices: Vec<DeviceSpec>,
    pub env: Vec<(String, String)>,
    pub warnings: Vec<String>,
}

/// Reads the live host environment, home directory, and settings file.
pub fn detect_host_appearance() -> HostAppearance {
    detect_host_appearance_with(
        |key| env::var(key).ok().filter(|value| !value.trim().is_empty()),
        |path| Path::new(path).exists(),
        &home_dir().to_string_lossy(),
    )
}

/// Testable detector; `get` returns a non-empty environment value and `exists`
/// reports whether a host path is present.
pub fn detect_host_appearance_with(
    get: impl Fn(&str) -> Option<String>,
    exists: impl Fn(&str) -> bool,
    home: &str,
) -> HostAppearance {
    let home = home.trim_end_matches('/').to_string();
    let home = if home.is_empty() { "/" } else { &home };

    let sources = appearance_sources(home)
        .into_iter()
        .filter(|source| exists(&source.source))
        .collect();

    // A missing settings file is normal on freshly installed hosts.
    let settings_path = format!("{home}/.config/gtk-3.0/settings.ini");
    let gtk = fs::read_to_string(settings_path)
        .map(|content| parse_gtk_settings(&content))
        .unwrap_or_default();

    HostAppearance {
        home: home.to_string(),
        sources,
        gtk,
        host_cursor_theme: non_empty(get("XCURSOR_THEME")),
        host_cursor_size: non_empty(get("XCURSOR_SIZE")),
        host_gtk_theme: non_empty(get("GTK_THEME")),
    }
}

/// Builds the host source list in a stable order: system dirs, then user dirs.
fn appearance_sources(home: &str) -> Vec<AppearanceSource> {
    let pairs = [
        (
            "fonts-system",
            "/usr/share/fonts".to_string(),
            "/usr/share/fonts".to_string(),
        ),
        (
            "fonts-user",
            format!("{home}/.local/share/fonts"),
            format!("{CONTAINER_HOME}/.local/share/fonts"),
        ),
        (
            "fonts-legacy",
            format!("{home}/.fonts"),
            format!("{CONTAINER_HOME}/.fonts"),
        ),
        (
            "icons-system",
            "/usr/share/icons".to_string(),
            "/usr/share/icons".to_string(),
        ),
        (
            "icons-user",
            format!("{home}/.local/share/icons"),
            format!("{CONTAINER_HOME}/.local/share/icons"),
        ),
        (
            "icons-legacy",
            format!("{home}/.icons"),
            format!("{CONTAINER_HOME}/.icons"),
        ),
        (
            "themes-system",
            "/usr/share/themes".to_string(),
            "/usr/share/themes".to_string(),
        ),
        (
            "themes-user",
            format!("{home}/.themes"),
            format!("{CONTAINER_HOME}/.themes"),
        ),
        (
            "gtk3-config",
            format!("{home}/.config/gtk-3.0"),
            format!("{CONTAINER_HOME}/.config/gtk-3.0"),
        ),
        (
            "gtk4-config",
            format!("{home}/.config/gtk-4.0"),
            format!("{CONTAINER_HOME}/.config/gtk-4.0"),
        ),
    ];
    pairs
        .into_iter()
        .map(|(name, source, target)| AppearanceSource {
            name: name.to_string(),
            source,
            target,
        })
        .collect()
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Parses a GTK key file without failing on missing keys or noisy input.
pub fn parse_gtk_settings(content: &str) -> GtkSettings {
    let mut settings = GtkSettings::default();
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with(';')
            || line.starts_with('[')
        {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = strip_inline_comment(value.trim());
        let value = unquote(value.trim());
        if value.is_empty() {
            continue;
        }
        match key.trim() {
            "gtk-theme-name" => settings.theme_name = Some(value.to_string()),
            "gtk-icon-theme-name" => settings.icon_theme_name = Some(value.to_string()),
            "gtk-cursor-theme-name" => settings.cursor_theme_name = Some(value.to_string()),
            "gtk-cursor-theme-size" => settings.cursor_theme_size = Some(value.to_string()),
            "gtk-application-prefer-dark-theme" => {
                settings.prefer_dark = matches!(
                    value.to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            _ => {}
        }
    }
    settings
}

/// Drops a `#` or `;` comment that starts after whitespace.
fn strip_inline_comment(value: &str) -> &str {
    let mut previous_is_space = false;
    for (idx, ch) in value.char_indices() {
        if (ch == '#' || ch == ';') && (idx == 0 || previous_is_space) {
            return &value[..idx];
        }
        previous_is_space = ch.is_whitespace();
    }
    value
}

fn unquote(value: &str) -> &str {
    for quote in ['"', '\''] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner;
        }
    }
    value
}

/// Parses a cursor size, rejecting values outside GTK's usable range.
fn valid_cursor_size(raw: &str) -> Option<u32> {
    raw.trim()
        .parse::<u32>()
        .ok()
        .filter(|size| (8..=256).contains(size))
}

/// Resolves the environment and read-only mounts for one environment.
pub fn build_appearance_plan(host: &HostAppearance) -> AppearancePlan {
    let mut plan = AppearancePlan::default();

    for source in &host.sources {
        plan.devices.push(DeviceSpec {
            name: source.name.clone(),
            kind: "disk".to_string(),
            options: vec![
                ("source".to_string(), source.source.clone()),
                ("path".to_string(), source.target.clone()),
                ("readonly".to_string(), "true".to_string()),
            ],
        });
    }

    let cursor_theme = host
        .host_cursor_theme
        .clone()
        .or_else(|| host.gtk.cursor_theme_name.clone())
        .unwrap_or_else(|| "default".to_string());
    let cursor_size = host
        .host_cursor_size
        .as_deref()
        .and_then(valid_cursor_size)
        .or_else(|| {
            host.gtk
                .cursor_theme_size
                .as_deref()
                .and_then(valid_cursor_size)
        })
        .unwrap_or(24);
    plan.env.extend([
        ("XCURSOR_THEME".to_string(), cursor_theme),
        ("XCURSOR_SIZE".to_string(), cursor_size.to_string()),
    ]);

    // The dark suffix is GTK's own variant syntax; the portal color-scheme
    // arrives with the D-Bus module, so no colour values are invented here.
    let gtk_theme = host.host_gtk_theme.clone().or_else(|| {
        host.gtk.theme_name.as_ref().map(|name| {
            if host.gtk.prefer_dark {
                format!("{name}:dark")
            } else {
                name.clone()
            }
        })
    });
    if let Some(theme) = gtk_theme {
        plan.env.push(("GTK_THEME".to_string(), theme));
    }

    if plan.devices.is_empty() {
        plan.warnings.push(
            "No host font, icon, or theme directory was found; the environment keeps its own appearance."
                .to_string(),
        );
    }

    plan
}

fn add_device_line(q_name: &str, device: &DeviceSpec) -> String {
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

/// Returns an idempotent snippet that installs the plan's read-only mounts.
///
/// The container must already be running. Incus needs the target directory to
/// exist before a disk device is added, so every planned target is created
/// first. Existing appearance devices are always removed, which also clears a
/// stale mount whose host source disappeared and picks up a new theme directory
/// on the next Start/Enter.
pub fn apply_appearance_devices_script(name: &str, plan: &AppearancePlan) -> String {
    // A default plan means desktop integration is off (or the target is a VM):
    // leave headless instances untouched instead of issuing pointless removals.
    if plan.devices.is_empty() && plan.env.is_empty() {
        return String::new();
    }
    let q_name = shell_quote(name);
    let mut lines = Vec::new();
    if !plan.devices.is_empty() {
        let q_targets = plan
            .devices
            .iter()
            .filter_map(|device| {
                device
                    .options
                    .iter()
                    .find(|(key, _)| key == "path")
                    .map(|(_, value)| shell_quote(value))
            })
            .collect::<Vec<_>>()
            .join(" ");
        lines.push(format!(
            "incus exec {q_name} -- mkdir -p {q_targets} >/dev/null 2>&1 || true"
        ));
    }
    lines.push(remove_appearance_devices_script(name));
    for device in &plan.devices {
        lines.push(add_device_line(&q_name, device));
    }
    lines.join("\n")
}

/// Removes every appearance mount, including sources that disappeared.
pub fn remove_appearance_devices_script(name: &str) -> String {
    remove_device_lines(name, &APPEARANCE_DEVICE_NAMES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn getter<'a>(values: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| {
            values
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| value.to_string())
        }
    }

    fn exists_set<'a>(present: &'a [&'a str]) -> impl Fn(&str) -> bool + 'a {
        move |path| present.contains(&path)
    }

    fn plan_env<'a>(plan: &'a AppearancePlan, key: &str) -> Option<&'a str> {
        plan.env
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }

    fn device<'a>(plan: &'a AppearancePlan, name: &str) -> &'a DeviceSpec {
        plan.devices
            .iter()
            .find(|device| device.name == name)
            .expect("device present")
    }

    fn option<'a>(device: &'a DeviceSpec, key: &str) -> &'a str {
        device
            .options
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
            .expect("option present")
    }

    struct TempHome(PathBuf);

    impl TempHome {
        fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default();
            let path = env::temp_dir().join(format!(
                "aibox-appearance-{prefix}-{}-{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create temp home");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempHome {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn only_existing_sources_become_devices_and_targets_stay_stable() {
        let home = "/home/user";
        let present = [
            "/usr/share/fonts",
            "/usr/share/icons",
            "/home/user/.themes",
            "/home/user/.config/gtk-4.0",
        ];
        let host = detect_host_appearance_with(getter(&[]), exists_set(&present), home);
        assert_eq!(
            host.sources
                .iter()
                .map(|source| source.name.as_str())
                .collect::<Vec<_>>(),
            vec!["fonts-system", "icons-system", "themes-user", "gtk4-config"]
        );

        let plan = build_appearance_plan(&host);
        assert_eq!(plan.devices.len(), 4);
        assert_eq!(
            option(device(&plan, "fonts-system"), "source"),
            "/usr/share/fonts"
        );
        assert_eq!(
            option(device(&plan, "fonts-system"), "path"),
            "/usr/share/fonts"
        );
        assert_eq!(
            option(device(&plan, "themes-user"), "path"),
            "/root/.themes"
        );
        assert_eq!(
            option(device(&plan, "gtk4-config"), "path"),
            "/root/.config/gtk-4.0"
        );
        for device in &plan.devices {
            assert_eq!(device.kind, "disk");
            assert_eq!(option(device, "readonly"), "true");
        }
    }

    #[test]
    fn user_directories_map_into_the_container_root_home() {
        let home = "/home/user";
        let present = [
            "/home/user/.local/share/fonts",
            "/home/user/.local/share/icons",
            "/home/user/.fonts",
            "/home/user/.icons",
            "/home/user/.themes",
            "/home/user/.config/gtk-3.0",
            "/home/user/.config/gtk-4.0",
        ];
        let host = detect_host_appearance_with(getter(&[]), exists_set(&present), home);
        let plan = build_appearance_plan(&host);
        for (name, target) in [
            ("fonts-user", "/root/.local/share/fonts"),
            ("icons-user", "/root/.local/share/icons"),
            ("fonts-legacy", "/root/.fonts"),
            ("icons-legacy", "/root/.icons"),
            ("themes-user", "/root/.themes"),
            ("gtk3-config", "/root/.config/gtk-3.0"),
            ("gtk4-config", "/root/.config/gtk-4.0"),
        ] {
            assert_eq!(option(device(&plan, name), "path"), target, "device {name}");
        }
    }

    #[test]
    fn gtk_parser_reads_keys_and_tolerates_noise() {
        let content = "\
# written by hand
; another comment
[Settings]
  gtk-theme-name = Adwaita-dark
gtk-icon-theme-name=\"Papirus\"
gtk-cursor-theme-name = Bibata-Modern-Ice
gtk-cursor-theme-size = 32
gtk-application-prefer-dark-theme=1
broken line without equals
unknown-key = ignored
";
        let settings = parse_gtk_settings(content);
        assert_eq!(settings.theme_name.as_deref(), Some("Adwaita-dark"));
        assert_eq!(settings.icon_theme_name.as_deref(), Some("Papirus"));
        assert_eq!(
            settings.cursor_theme_name.as_deref(),
            Some("Bibata-Modern-Ice")
        );
        assert_eq!(settings.cursor_theme_size.as_deref(), Some("32"));
        assert!(settings.prefer_dark);
    }

    #[test]
    fn gtk_parser_reports_missing_keys_and_reads_inline_comments() {
        let empty = parse_gtk_settings("");
        assert_eq!(empty, GtkSettings::default());
        assert!(!empty.prefer_dark);

        let partial = parse_gtk_settings(
            "gtk-theme-name = Adwaita # host default\ngtk-application-prefer-dark-theme = false\n",
        );
        assert_eq!(partial.theme_name.as_deref(), Some("Adwaita"));
        assert!(!partial.prefer_dark);
        assert!(partial.cursor_theme_name.is_none());
        assert!(partial.cursor_theme_size.is_none());
    }

    fn host_with(
        cursor_theme: Option<&str>,
        cursor_size: Option<&str>,
        gtk_theme: Option<&str>,
        gtk: GtkSettings,
    ) -> HostAppearance {
        HostAppearance {
            home: "/home/user".to_string(),
            sources: Vec::new(),
            gtk,
            host_cursor_theme: cursor_theme.map(str::to_string),
            host_cursor_size: cursor_size.map(str::to_string),
            host_gtk_theme: gtk_theme.map(str::to_string),
        }
    }

    #[test]
    fn environment_prefers_host_values_then_gtk_settings_then_defaults() {
        let gtk = GtkSettings {
            theme_name: Some("Adwaita".to_string()),
            cursor_theme_name: Some("Adwaita".to_string()),
            cursor_theme_size: Some("32".to_string()),
            prefer_dark: true,
            ..GtkSettings::default()
        };

        let defaults = build_appearance_plan(&host_with(None, None, None, GtkSettings::default()));
        assert_eq!(plan_env(&defaults, "XCURSOR_THEME"), Some("default"));
        assert_eq!(plan_env(&defaults, "XCURSOR_SIZE"), Some("24"));
        assert_eq!(plan_env(&defaults, "GTK_THEME"), None);

        let from_gtk = build_appearance_plan(&host_with(None, None, None, gtk));
        assert_eq!(plan_env(&from_gtk, "XCURSOR_THEME"), Some("Adwaita"));
        assert_eq!(plan_env(&from_gtk, "XCURSOR_SIZE"), Some("32"));
        assert_eq!(plan_env(&from_gtk, "GTK_THEME"), Some("Adwaita:dark"));

        let from_host = build_appearance_plan(&host_with(
            Some("Bibata"),
            Some("48"),
            Some("Custom"),
            GtkSettings {
                theme_name: Some("Adwaita".to_string()),
                cursor_theme_name: Some("Adwaita".to_string()),
                cursor_theme_size: Some("32".to_string()),
                prefer_dark: true,
                ..GtkSettings::default()
            },
        ));
        assert_eq!(plan_env(&from_host, "XCURSOR_THEME"), Some("Bibata"));
        assert_eq!(plan_env(&from_host, "XCURSOR_SIZE"), Some("48"));
        assert_eq!(
            plan_env(&from_host, "GTK_THEME"),
            Some("Custom"),
            "an explicit host GTK_THEME is forwarded unchanged"
        );
    }

    #[test]
    fn invalid_cursor_sizes_fall_back_within_range() {
        for invalid in ["0", "7", "257", "-4", "abc", ""] {
            let plan = build_appearance_plan(&host_with(
                None,
                Some(invalid),
                None,
                GtkSettings::default(),
            ));
            assert_eq!(
                plan_env(&plan, "XCURSOR_SIZE"),
                Some("24"),
                "value {invalid}"
            );
        }
        for valid in ["8", "24", "256"] {
            let plan =
                build_appearance_plan(&host_with(None, Some(valid), None, GtkSettings::default()));
            assert_eq!(
                plan_env(&plan, "XCURSOR_SIZE"),
                Some(valid),
                "value {valid}"
            );
        }

        let gtk = GtkSettings {
            cursor_theme_size: Some("40".to_string()),
            ..GtkSettings::default()
        };
        let plan = build_appearance_plan(&host_with(None, Some("999"), None, gtk));
        assert_eq!(
            plan_env(&plan, "XCURSOR_SIZE"),
            Some("40"),
            "an invalid host size should not discard a valid GTK size"
        );
    }

    #[test]
    fn detection_reads_the_gtk_settings_file_and_host_environment() {
        let home = TempHome::new("detect");
        let settings_path = home.path().join(".config/gtk-3.0/settings.ini");
        fs::create_dir_all(settings_path.parent().expect("settings parent"))
            .expect("create settings dir");
        fs::write(
            &settings_path,
            "gtk-theme-name=Catppuccin-Mocha\ngtk-cursor-theme-name=catppuccin\ngtk-cursor-theme-size=30\ngtk-application-prefer-dark-theme=1\n",
        )
        .expect("write settings");

        let home_str = home.path().to_string_lossy().into_owned();
        let host = detect_host_appearance_with(
            getter(&[("XCURSOR_THEME", "host-cursor"), ("GTK_THEME", "HostTheme")]),
            |_| false,
            &home_str,
        );
        assert_eq!(host.home, home_str);
        assert!(host.sources.is_empty());
        assert_eq!(host.gtk.theme_name.as_deref(), Some("Catppuccin-Mocha"));
        assert!(host.gtk.prefer_dark);
        assert_eq!(host.host_cursor_theme.as_deref(), Some("host-cursor"));
        assert_eq!(host.host_gtk_theme.as_deref(), Some("HostTheme"));

        let plan = build_appearance_plan(&host);
        assert_eq!(plan_env(&plan, "XCURSOR_THEME"), Some("host-cursor"));
        assert_eq!(plan_env(&plan, "XCURSOR_SIZE"), Some("30"));
        assert_eq!(plan_env(&plan, "GTK_THEME"), Some("HostTheme"));
        assert_eq!(plan.warnings.len(), 1);
    }

    #[test]
    fn missing_settings_file_and_empty_environment_are_tolerated() {
        let home = TempHome::new("missing");
        let home_str = home.path().to_string_lossy().into_owned();
        let host = detect_host_appearance_with(getter(&[]), |_| false, &home_str);
        assert_eq!(host.gtk, GtkSettings::default());
        assert!(host.host_cursor_theme.is_none());
        assert!(host.host_cursor_size.is_none());
        assert!(host.host_gtk_theme.is_none());

        let whitespace = detect_host_appearance_with(
            getter(&[("XCURSOR_THEME", "   "), ("GTK_THEME", "  ")]),
            |_| false,
            &home_str,
        );
        assert!(whitespace.host_cursor_theme.is_none());
        assert!(whitespace.host_gtk_theme.is_none());
    }

    fn sample_plan() -> AppearancePlan {
        let host = detect_host_appearance_with(
            getter(&[]),
            exists_set(&[
                "/usr/share/fonts",
                "/usr/share/icons",
                "/home/user/.themes",
                "/home/user/.config/gtk-3.0",
            ]),
            "/home/user",
        );
        build_appearance_plan(&host)
    }

    #[test]
    fn apply_script_prepares_targets_removes_then_adds_read_only_devices() {
        let plan = sample_plan();
        let script = apply_appearance_devices_script("box", &plan);
        let prep_at = script
            .find("incus exec 'box' -- mkdir -p")
            .expect("target prep");
        let remove_at = script
            .find("incus config device remove 'box' 'fonts-system'")
            .expect("remove");
        let add_at = script
            .find("incus config device add 'box' 'fonts-system' 'disk'")
            .expect("add");
        assert!(prep_at < remove_at);
        assert!(remove_at < add_at);
        assert!(script.contains("readonly='true'"));
        assert!(script.contains("source='/usr/share/fonts'"));
        assert!(script.contains("path='/usr/share/fonts'"));
        assert!(script.contains(">/dev/null 2>&1 || true"));
    }

    #[test]
    fn apply_script_without_devices_still_clears_stale_mounts() {
        assert_eq!(
            apply_appearance_devices_script("box", &AppearancePlan::default()),
            "",
            "an off/VM plan must not touch the instance"
        );

        let plan = build_appearance_plan(&host_with(None, None, None, GtkSettings::default()));
        assert!(plan.devices.is_empty());
        let script = apply_appearance_devices_script("box", &plan);
        assert!(!script.contains("incus exec"));
        for name in APPEARANCE_DEVICE_NAMES {
            assert!(
                script.contains(&format!("incus config device remove 'box' '{name}'")),
                "missing removal for {name}"
            );
        }
    }

    #[test]
    fn remove_script_covers_every_appearance_device() {
        let script = remove_appearance_devices_script("box");
        for name in APPEARANCE_DEVICE_NAMES {
            assert!(
                script.contains(&format!("incus config device remove 'box' '{name}'")),
                "missing removal for {name}"
            );
        }
        assert_eq!(script.lines().count(), APPEARANCE_DEVICE_NAMES.len());
    }

    #[test]
    fn scripts_quote_names_and_paths() {
        let plan = sample_plan();
        let script = apply_appearance_devices_script("it's a box; rm -rf /", &plan);
        assert!(script.contains("'it'\\''s a box; rm -rf /'"));
        assert!(!script.contains("; rm -rf /\n"));
        assert!(script.contains("path='/root/.themes'"));
    }

    #[test]
    fn generated_scripts_pass_bash_syntax_check() {
        let plans = [
            sample_plan(),
            build_appearance_plan(&host_with(None, None, None, GtkSettings::default())),
        ];
        let mut scripts = vec![remove_appearance_devices_script("it's a box; rm -rf /")];
        for plan in plans {
            scripts.push(apply_appearance_devices_script("box", &plan));
            scripts.push(apply_appearance_devices_script(
                "it's a box; rm -rf /",
                &plan,
            ));
        }
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
