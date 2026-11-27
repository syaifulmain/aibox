//! Host desktop integration for isolated environments.
//!
//! The display and audio bridges turn the host session (graphical and audio
//! sockets) into per-environment Incus device plans plus the environment
//! variables needed to make GUI and sound programs work from every entry path
//! (GUI Enter, `aibox enter`, and `aibox exec`). Keeping the plans separate
//! from the shell snippets makes the behaviour unit-testable without a live
//! Incus daemon.

pub mod audio;
pub mod host;
pub mod plan;

pub use audio::{
    apply_audio_devices_script, build_audio_plan, detect_host_audio, detect_host_audio_with,
    remove_audio_devices_script, set_audio_mode_script, test_audio_script, AudioMode, AudioPlan,
    HostAudio,
};
pub use host::{detect_host_session, detect_host_session_with, HostSession};
pub use plan::{
    apply_devices_script, build_desktop_plan, combine_scripts, merge_env_pairs,
    remove_devices_script, set_desktop_mode_script, DesktopMode, DeviceSpec, DisplayPlan,
};
