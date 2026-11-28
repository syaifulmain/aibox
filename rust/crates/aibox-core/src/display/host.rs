//! Detects the graphical session that the AIBox process runs in.
//!
//! Detection is split into a pure parser (`detect_host_session_with`) so tests
//! can feed values without mutating the process environment.

use std::env;

/// Host-side graphical session details used to build an environment plan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostSession {
    pub uid: u32,
    pub gid: u32,
    pub runtime_dir: String,
    pub x11_display: String,
    pub x11_socket: String,
    pub wayland_display: String,
    pub wayland_socket: String,
    /// Host desktop name (`Hyprland` on Omarchy), empty when unset.
    pub xdg_current_desktop: String,
}

/// Returns the current process's effective host user ID.
pub fn host_uid() -> u32 {
    // SAFETY: getuid is always safe to call and does not touch memory.
    unsafe { libc::getuid() }
}

/// Reads the current process environment and OS user IDs.
pub fn detect_host_session() -> HostSession {
    // SAFETY: getuid/getgid are always safe to call and do not touch memory.
    let uid = host_uid();
    let gid = unsafe { libc::getgid() };
    detect_host_session_with(
        |key| env::var(key).ok().filter(|value| !value.trim().is_empty()),
        uid,
        gid,
    )
}

/// Pure host-session detector; `get` returns a non-empty value for an env key.
pub fn detect_host_session_with(
    get: impl Fn(&str) -> Option<String>,
    uid: u32,
    gid: u32,
) -> HostSession {
    let runtime_dir = get("XDG_RUNTIME_DIR")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("/run/user/{uid}"));

    let x11_display = normalize_x11_display(&get("DISPLAY").unwrap_or_default());
    let x11_socket = if x11_display.is_empty() {
        String::new()
    } else {
        // XWayland and a real X server both expose a socket under this path.
        format!("/tmp/.X11-unix/X{}", &x11_display[1..])
    };

    let wayland_display = get("WAYLAND_DISPLAY")
        .map(|value| value.trim().to_string())
        .unwrap_or_default();
    let wayland_socket = if wayland_display.is_empty() {
        String::new()
    } else if wayland_display.starts_with('/') {
        wayland_display.clone()
    } else {
        format!("{runtime_dir}/{wayland_display}")
    };

    let xdg_current_desktop = get("XDG_CURRENT_DESKTOP")
        .map(|value| value.trim().to_string())
        .unwrap_or_default();

    HostSession {
        uid,
        gid,
        runtime_dir,
        x11_display,
        x11_socket,
        wayland_display,
        wayland_socket,
        xdg_current_desktop,
    }
}

/// Normalizes `DISPLAY` to the `:N` form and drops the host and screen parts.
///
/// `:0`, `:0.0`, `localhost:0`, and `host:0.0` all become `:0`.
fn normalize_x11_display(raw: &str) -> String {
    let raw = raw.trim();
    if raw.is_empty() {
        return String::new();
    }
    let Some((_host, display)) = raw.rsplit_once(':') else {
        return String::new();
    };
    let number = display.split('.').next().unwrap_or_default();
    if number.is_empty() || !number.chars().all(|ch| ch.is_ascii_digit()) {
        return String::new();
    }
    format!(":{number}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn getter<'a>(values: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| {
            values
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| value.to_string())
        }
    }

    #[test]
    fn parses_x11_display_variants_into_display_and_socket() {
        for raw in [":0", ":0.0", "host:0", "localhost:0.0"] {
            let session = detect_host_session_with(getter(&[("DISPLAY", raw)]), 1000, 1000);
            assert_eq!(session.x11_display, ":0", "raw {raw}");
            assert_eq!(session.x11_socket, "/tmp/.X11-unix/X0", "raw {raw}");
        }
    }

    #[test]
    fn parses_a_non_zero_display_and_screen() {
        let session = detect_host_session_with(getter(&[("DISPLAY", ":1.2")]), 1000, 1000);
        assert_eq!(session.x11_display, ":1");
        assert_eq!(session.x11_socket, "/tmp/.X11-unix/X1");
    }

    #[test]
    fn empty_and_invalid_display_produce_empty_socket() {
        for raw in ["", "bogus", ":abc"] {
            let session = detect_host_session_with(getter(&[("DISPLAY", raw)]), 1000, 1000);
            assert!(session.x11_display.is_empty(), "raw {raw}");
            assert!(session.x11_socket.is_empty(), "raw {raw}");
        }
    }

    #[test]
    fn defaults_runtime_dir_to_the_user_run_directory() {
        let session = detect_host_session_with(getter(&[]), 4242, 4243);
        assert_eq!(session.runtime_dir, "/run/user/4242");
        assert_eq!(session.uid, 4242);
        assert_eq!(session.gid, 4243);
    }

    #[test]
    fn builds_the_wayland_socket_from_the_runtime_dir() {
        let session = detect_host_session_with(
            getter(&[
                ("XDG_RUNTIME_DIR", "/run/user/1000"),
                ("WAYLAND_DISPLAY", "wayland-1"),
            ]),
            1000,
            1000,
        );
        assert_eq!(session.wayland_display, "wayland-1");
        assert_eq!(session.wayland_socket, "/run/user/1000/wayland-1");
    }

    #[test]
    fn keeps_an_absolute_wayland_socket() {
        let session = detect_host_session_with(
            getter(&[("WAYLAND_DISPLAY", "/run/user/1000/wayland-1")]),
            1000,
            1000,
        );
        assert_eq!(session.wayland_display, "/run/user/1000/wayland-1");
        assert_eq!(session.wayland_socket, "/run/user/1000/wayland-1");
    }

    #[test]
    fn missing_wayland_display_leaves_both_fields_empty() {
        let session =
            detect_host_session_with(getter(&[("XDG_RUNTIME_DIR", "/run/user/1000")]), 1000, 1000);
        assert!(session.wayland_display.is_empty());
        assert!(session.wayland_socket.is_empty());
    }

    #[test]
    fn missing_runtime_dir_still_builds_a_relative_wayland_socket() {
        let session =
            detect_host_session_with(getter(&[("WAYLAND_DISPLAY", "wayland-1")]), 1000, 1000);
        assert_eq!(session.wayland_socket, "/run/user/1000/wayland-1");
    }

    #[test]
    fn passes_through_the_host_desktop_name_only_when_set() {
        let set =
            detect_host_session_with(getter(&[("XDG_CURRENT_DESKTOP", "Hyprland")]), 1000, 1000);
        assert_eq!(set.xdg_current_desktop, "Hyprland");

        let unset = detect_host_session_with(getter(&[]), 1000, 1000);
        assert!(unset.xdg_current_desktop.is_empty());
    }
}
