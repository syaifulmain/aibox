//! Host desktop integration for isolated environments.
//!
//! The display and audio bridges turn the host session (graphical and audio
//! sockets) into per-environment Incus device plans plus the environment
//! variables needed to make GUI and sound programs work from every entry path
//! (GUI Enter, `aibox enter`, and `aibox exec`). Keeping the plans separate
//! from the shell snippets makes the behaviour unit-testable without a live
//! Incus daemon.

pub mod appearance;
pub mod audio;
pub mod dbus;
pub mod dbus_auth_proxy;
pub mod devices;
pub mod downloads;
pub mod gpu;
pub mod host;
pub mod plan;

pub use appearance::{
    apply_appearance_devices_script, build_appearance_plan, detect_host_appearance,
    detect_host_appearance_with, parse_gtk_settings, remove_appearance_devices_script,
    AppearancePlan, AppearanceSource, GtkSettings, HostAppearance, APPEARANCE_DEVICE_NAMES,
};
pub use audio::{
    apply_audio_devices_script, build_audio_plan, detect_host_audio, detect_host_audio_with,
    remove_audio_devices_script, set_audio_mode_script, test_audio_script, AudioMode, AudioPlan,
    HostAudio,
};
pub use dbus::{
    apply_dbus_devices_script, build_dbus_plan, dbus_env, dbus_test_script, detect_host_dbus,
    detect_host_dbus_with, remove_dbus_bridge_script, sanitize_env_name, set_dbus_mode_script,
    stop_dbus_bridge_script, stop_dbus_bridge_script_with, DbusMode, DbusPlan, HostDbus,
    DBUS_TALK_FILTERS,
};
pub use devices::{
    apply_extra_devices_script, build_extra_devices_plan, detect_host_devices,
    detect_host_devices_with, parse_toggle_flag, remove_extra_devices_script,
    set_extra_devices_script, ExtraDeviceToggles, ExtraDevicesPlan, HostDevices, CAMERA_PREFIX,
    CUPS_SOCKET, INPUT_PREFIX, PRINTER_DEVICE,
};
pub use downloads::{
    download_dir_script, download_env, upsert_download_dir, CONTAINER_DOWNLOAD_DIR,
};
pub use gpu::{
    detect_gpu, detect_gpu_at, detect_gpu_with, gpu_env, gpu_env_with, gpu_test_script, GpuInfo,
};
pub use host::{detect_host_session, detect_host_session_with, HostSession};
pub use plan::{
    apply_devices_script, build_desktop_plan, combine_scripts, merge_env_pairs,
    remove_devices_script, session_bridge_script, set_desktop_mode_script, DesktopMode, DeviceSpec,
    DisplayPlan,
};
