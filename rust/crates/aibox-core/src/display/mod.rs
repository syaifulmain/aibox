//! Host desktop integration for isolated environments.
//!
//! The display bridge turns the host session (X11/Wayland sockets) into a
//! per-environment Incus device plan plus the environment variables needed to
//! make GUI programs work from every entry path (GUI Enter, `aibox enter`, and
//! `aibox exec`). Keeping the plan separate from the shell snippets makes the
//! behaviour unit-testable without a live Incus daemon.

pub mod host;
pub mod plan;

pub use host::{detect_host_session, detect_host_session_with, HostSession};
pub use plan::{
    apply_devices_script, build_desktop_plan, remove_devices_script, set_desktop_mode_script,
    DesktopMode, DeviceSpec, DisplayPlan,
};
