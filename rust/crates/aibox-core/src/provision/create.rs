use crate::config::defaults::{OsEntry, Preset};
use crate::display::{
    build_audio_plan, build_desktop_plan, combine_scripts, detect_host_audio, detect_host_session,
    download_dir_script, set_audio_mode_script, set_desktop_mode_script, AudioMode, DesktopMode,
};
use crate::provision::templates::{render_template, rollback_trap_snippet, CREATE_SCRIPT_TEMPLATE};
use crate::provision::{
    is_valid_size_spec, swap_config_value, validate_limits, validate_new_env_name,
    validate_swap_limit, ProvisionError,
};
use crate::snapshot::workspace::workspace_create_snippet;
use crate::util::paths::{cache_base_path, resolve_workspace_path, CONTAINER_WORKSPACE_PATH};
use crate::util::shell::{shell_array_words, shell_quote};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateEnvironmentPlan {
    pub name: String,
    pub title: String,
    pub script: String,
}

#[allow(clippy::too_many_arguments)]
pub fn create_environment_script(
    name: &str,
    local_alias: &str,
    remote_image: &str,
    workspace: &str,
    cache_base: &str,
    profile_flags: &str,
    ram_gib: &str,
    disk_gib: &str,
    cpu_cores: i32,
    x11: bool,
    process_limit: &str,
    io_limit: &str,
    swap_limit: &str,
    is_vm: bool,
) -> String {
    create_environment_script_inner(
        name,
        local_alias,
        remote_image,
        workspace,
        cache_base,
        profile_flags,
        ram_gib,
        disk_gib,
        cpu_cores,
        x11,
        process_limit,
        io_limit,
        swap_limit,
        is_vm,
        None,
    )
}

/// Production create script that applies the combined display+audio bridge
/// snippet after the workspace device instead of mounting the X socket and
/// calling `xhost +local:`.
#[allow(clippy::too_many_arguments)]
pub fn create_environment_script_with_desktop(
    name: &str,
    local_alias: &str,
    remote_image: &str,
    workspace: &str,
    cache_base: &str,
    profile_flags: &str,
    ram_gib: &str,
    disk_gib: &str,
    cpu_cores: i32,
    x11: bool,
    process_limit: &str,
    io_limit: &str,
    swap_limit: &str,
    is_vm: bool,
    desktop_script: &str,
) -> String {
    create_environment_script_with_session(
        name,
        local_alias,
        remote_image,
        workspace,
        cache_base,
        profile_flags,
        ram_gib,
        disk_gib,
        cpu_cores,
        x11,
        process_limit,
        io_limit,
        swap_limit,
        is_vm,
        desktop_script,
    )
}

/// Like [`create_environment_script_with_desktop`], but the injected snippet is
/// the full session bridge (display and audio devices).
#[allow(clippy::too_many_arguments)]
pub fn create_environment_script_with_session(
    name: &str,
    local_alias: &str,
    remote_image: &str,
    workspace: &str,
    cache_base: &str,
    profile_flags: &str,
    ram_gib: &str,
    disk_gib: &str,
    cpu_cores: i32,
    x11: bool,
    process_limit: &str,
    io_limit: &str,
    swap_limit: &str,
    is_vm: bool,
    session_script: &str,
) -> String {
    let session_script = combine_scripts(&[session_script.to_string(), download_dir_script(name)]);
    create_environment_script_inner(
        name,
        local_alias,
        remote_image,
        workspace,
        cache_base,
        profile_flags,
        ram_gib,
        disk_gib,
        cpu_cores,
        x11,
        process_limit,
        io_limit,
        swap_limit,
        is_vm,
        Some(&session_script),
    )
}

const LEGACY_X11_BLOCK: &str = "if [[ \"$X11\" == \"1\" ]] && [[ -d /tmp/.X11-unix ]]; then\n    if command -v xhost >/dev/null 2>&1 && [[ -n \"${DISPLAY:-}\" ]]; then\n        xhost +local: >/dev/null 2>&1 || true\n    fi\n    incus config device add \"$NAME\" x11 disk source=/tmp/.X11-unix path=/tmp/.X11-unix >/dev/null 2>&1 || true\nfi";

#[allow(clippy::too_many_arguments)]
fn create_environment_script_inner(
    name: &str,
    local_alias: &str,
    remote_image: &str,
    workspace: &str,
    cache_base: &str,
    profile_flags: &str,
    ram_gib: &str,
    disk_gib: &str,
    cpu_cores: i32,
    x11: bool,
    process_limit: &str,
    io_limit: &str,
    swap_limit: &str,
    is_vm: bool,
    session_script: Option<&str>,
) -> String {
    let mut root_opts = format!("size={}", disk_gib.trim());
    let clean_io = io_limit.trim();
    if !clean_io.is_empty() && !clean_io.eq_ignore_ascii_case("unlimited") {
        root_opts.push_str(&format!(",limits.read={clean_io},limits.write={clean_io}"));
    }

    let mut quota_args = format!(
        "-c limits.cpu={} -c limits.memory={} -d root,{root_opts}",
        cpu_cores.max(1),
        ram_gib.trim()
    );
    let clean_proc = process_limit.trim();
    if !is_vm && !clean_proc.is_empty() && !clean_proc.eq_ignore_ascii_case("unlimited") {
        quota_args.push_str(&format!(" -c limits.processes={clean_proc}"));
    }
    if !is_vm {
        if let Some(swap_value) = swap_config_value(swap_limit) {
            quota_args.push_str(&format!(" -c limits.memory.swap={swap_value}"));
        }
    }

    let ws_snippet = workspace_create_snippet(workspace);
    let cache_root = format!("{cache_base}.root");
    let rollback_trap = rollback_trap_snippet("NAME");
    let (x11_block, session_setup) = match session_script {
        Some(script) => ("", script),
        None => (LEGACY_X11_BLOCK, ""),
    };

    render_template(
        CREATE_SCRIPT_TEMPLATE,
        &[
            ("NAME", &shell_quote(name)),
            ("LOCAL_ALIAS", &shell_quote(local_alias)),
            ("REMOTE_IMAGE", &shell_quote(remote_image)),
            ("WORKSPACE", &shell_quote(workspace)),
            ("WS_PATH", &shell_quote(CONTAINER_WORKSPACE_PATH)),
            ("CACHE", &shell_quote(cache_base)),
            ("CACHE_ROOT", &shell_quote(&cache_root)),
            ("PROFILE_ARGS", &shell_array_words(profile_flags)),
            ("QUOTA_ARGS", &shell_array_words(&quota_args)),
            ("X11", if x11 { "1" } else { "0" }),
            ("ROLLBACK_TRAP", &rollback_trap),
            ("WORKSPACE_CREATE", &ws_snippet),
            ("X11_BLOCK", x11_block),
            ("DESKTOP_SETUP", session_setup),
        ],
    )
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_create_environment(
    busy: bool,
    raw_name: &str,
    existing_names: &[String],
    os_id: &str,
    os_entries: &[OsEntry],
    preset_id: &str,
    default_preset_id: &str,
    presets: &[Preset],
    disk_gib: &str,
    ram_gib: &str,
    cpu_cores: i32,
    process_limit: &str,
    io_limit: &str,
    swap_limit: &str,
    workspace_parent: &str,
    workspace_base: &str,
    os_storage_path: &str,
) -> Result<CreateEnvironmentPlan, ProvisionError> {
    if busy {
        return Err(ProvisionError::Busy);
    }

    let name = validate_new_env_name(raw_name, existing_names)?;

    if !(1..=1024).contains(&cpu_cores) {
        return Err(ProvisionError::InvalidCpuCores);
    }
    if !is_valid_size_spec(disk_gib) {
        return Err(ProvisionError::InvalidDiskQuota);
    }
    if !is_valid_size_spec(ram_gib) {
        return Err(ProvisionError::InvalidRamLimit);
    }

    let (clean_proc, clean_io) = validate_limits(process_limit, io_limit)?;
    validate_swap_limit(swap_limit)?;

    let os_entry = os_entries
        .iter()
        .find(|e| e.id == os_id)
        .or_else(|| os_entries.first())
        .ok_or(ProvisionError::EmptyOsCatalog)?;

    let preset = presets
        .iter()
        .find(|p| p.id == preset_id)
        .or_else(|| presets.iter().find(|p| p.id == default_preset_id))
        .or_else(|| presets.first())
        .ok_or(ProvisionError::EmptyPresetCatalog)?;

    let host_workspace = resolve_workspace_path(workspace_parent, &name, workspace_base)
        .map_err(ProvisionError::InvalidWorkspacePath)?;
    let local_alias = format!("aibox-{}", os_entry.id);
    let cache_base = cache_base_path(os_storage_path, &os_entry.id);
    let is_vm = preset.flags.contains("--vm");

    // VMs stay headless for now; SPICE/VNC is out of scope for this bridge.
    let has_desktop = preset.x11 && !is_vm;
    let desktop_mode = if has_desktop {
        DesktopMode::On
    } else {
        DesktopMode::Off
    };
    let host = detect_host_session();
    let desktop_plan = build_desktop_plan(desktop_mode, &host);
    let mut desktop_script = set_desktop_mode_script(&name, desktop_mode, &host);
    for warning in &desktop_plan.warnings {
        desktop_script.push_str(&format!("\necho {} >&2", shell_quote(warning)));
    }

    // GUI containers start with playback only; microphone access is opt-in
    // later through the Audio card. Headless containers and VMs stay silent.
    let audio_mode = if has_desktop {
        AudioMode::Output
    } else {
        AudioMode::Off
    };
    let host_audio = detect_host_audio();
    let audio_plan = build_audio_plan(audio_mode, &host_audio, host_audio.uid);
    let mut audio_script = set_audio_mode_script(&name, audio_mode, &host_audio);
    for warning in &audio_plan.warnings {
        audio_script.push_str(&format!("\necho {} >&2", shell_quote(warning)));
    }
    let session_script = combine_scripts(&[desktop_script, audio_script]);

    let script = create_environment_script_with_session(
        &name,
        &local_alias,
        &format!("images:{}", os_entry.alias),
        &host_workspace,
        &cache_base,
        &preset.flags,
        ram_gib,
        disk_gib,
        cpu_cores,
        preset.x11,
        &clean_proc,
        &clean_io,
        swap_limit,
        is_vm,
        &session_script,
    );

    Ok(CreateEnvironmentPlan {
        title: format!("Creating environment '{name}'"),
        name,
        script,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    fn write_executable(path: &std::path::Path, content: &str) {
        fs::write(path, content).unwrap();
        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).unwrap();
    }

    #[test]
    fn create_environment_script_passes_preset_flags_as_arguments_match_qt_test() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("aibox-prov-create-{nanos}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let stub_dir = dir.join("stubs");
        fs::create_dir_all(&stub_dir).unwrap();
        let log_path = dir.join("incus.log");

        write_executable(
            &stub_dir.join("incus"),
            "#!/bin/sh\necho \"$@\" >> \"$AIBOX_STUB_LOG\"\nif [ \"$1\" = image ] && [ \"$2\" = info ]; then exit 1; fi\nif [ \"$1\" = list ]; then echo \"10.0.0.5\"; fi\nexit 0\n",
        );
        write_executable(&stub_dir.join("script"), "#!/bin/sh\nexec bash -c \"$4\"\n");
        write_executable(&stub_dir.join("sleep"), "#!/bin/sh\nexit 0\n");

        let script_path = dir.join("task.sh");
        let script = create_environment_script(
            "mobile-porting",
            "aibox-archlinux",
            "images:archlinux",
            &dir.join("workspace").to_string_lossy(),
            &dir.join("os-images/aibox-archlinux").to_string_lossy(),
            "-p default -p ai-gui",
            "6GiB",
            "60GiB",
            4,
            true,
            "",
            "",
            "",
            false,
        );
        fs::write(&script_path, script).unwrap();

        let path_env = format!(
            "{}:{}",
            stub_dir.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let output = Command::new("/bin/bash")
            .arg(&script_path)
            .env("PATH", path_env)
            .env("AIBOX_STUB_LOG", &log_path)
            .env_remove("DISPLAY")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");

        let log = fs::read_to_string(&log_path).unwrap();
        assert!(
            log.contains(
                "launch aibox-archlinux mobile-porting -p default -p ai-gui -c limits.cpu=4 -c limits.memory=6GiB -d root,size=60GiB"
            ),
            "unexpected log: {log}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn runaway_limits_container_vs_vm_create_script() {
        let ct_script = create_environment_script(
            "ct-box",
            "aibox-ubuntu",
            "images:ubuntu/24.04",
            "/tmp/ws",
            "/tmp/cache",
            "",
            "6GiB",
            "35GiB",
            4,
            false,
            "512",
            "200MB",
            "Off",
            false,
        );
        assert!(ct_script.contains("limits.processes=512"));
        assert!(ct_script.contains("limits.read=200MB,limits.write=200MB"));
        assert!(ct_script.contains("limits.memory.swap=false"));

        let vm_script = create_environment_script(
            "vm-box",
            "aibox-ubuntu",
            "images:ubuntu/24.04",
            "/tmp/ws",
            "/tmp/cache",
            "--vm",
            "6GiB",
            "35GiB",
            4,
            false,
            "512",
            "200MB",
            "4GiB",
            true,
        );
        assert!(!vm_script.contains("limits.processes="));
        assert!(!vm_script.contains("limits.memory.swap="));
        assert!(vm_script.contains("limits.read=200MB,limits.write=200MB"));
    }

    #[test]
    fn prepare_create_environment_validates_inputs_with_typed_errors() {
        let os = vec![OsEntry {
            id: "arch".into(),
            alias: "archlinux".into(),
            name: "Arch".into(),
            badge: "Rolling".into(),
            description: "Arch Linux".into(),
        }];
        let presets = vec![Preset {
            id: "gui".into(),
            title: "GUI".into(),
            sub: "GUI".into(),
            flags: "-p default".into(),
            x11: true,
        }];
        let existing = vec!["taken".to_string()];

        assert_eq!(
            prepare_create_environment(
                true, "box1", &existing, "arch", &os, "gui", "gui", &presets, "35GiB", "6GiB", 4,
                "512", "200MB", "", "", "/tmp/ws", "/tmp/os"
            )
            .unwrap_err(),
            ProvisionError::Busy
        );
        assert_eq!(
            prepare_create_environment(
                false, "!!!", &existing, "arch", &os, "gui", "gui", &presets, "35GiB", "6GiB", 4,
                "512", "200MB", "", "", "/tmp/ws", "/tmp/os"
            )
            .unwrap_err(),
            ProvisionError::InvalidName
        );
        assert_eq!(
            prepare_create_environment(
                false, "taken", &existing, "arch", &os, "gui", "gui", &presets, "35GiB", "6GiB", 4,
                "512", "200MB", "", "", "/tmp/ws", "/tmp/os"
            )
            .unwrap_err(),
            ProvisionError::AlreadyExists("taken".into())
        );
        assert_eq!(
            prepare_create_environment(
                false, "box1", &existing, "arch", &os, "gui", "gui", &presets, "35GiB", "6GiB", 0,
                "512", "200MB", "", "", "/tmp/ws", "/tmp/os"
            )
            .unwrap_err(),
            ProvisionError::InvalidCpuCores
        );
        assert_eq!(
            prepare_create_environment(
                false, "box1", &existing, "arch", &os, "gui", "gui", &presets, "bad", "6GiB", 4,
                "512", "200MB", "", "", "/tmp/ws", "/tmp/os"
            )
            .unwrap_err(),
            ProvisionError::InvalidDiskQuota
        );
        assert_eq!(
            prepare_create_environment(
                false, "box1", &existing, "arch", &os, "gui", "gui", &presets, "35GiB", "bad", 4,
                "512", "200MB", "", "", "/tmp/ws", "/tmp/os"
            )
            .unwrap_err(),
            ProvisionError::InvalidRamLimit
        );
        assert_eq!(
            prepare_create_environment(
                false, "box1", &existing, "arch", &os, "gui", "gui", &presets, "35GiB", "6GiB", 4,
                "1", "200MB", "", "", "/tmp/ws", "/tmp/os"
            )
            .unwrap_err(),
            ProvisionError::InvalidProcessLimit
        );
        assert_eq!(
            prepare_create_environment(
                false, "box1", &existing, "arch", &os, "gui", "gui", &presets, "35GiB", "6GiB", 4,
                "512", "bad", "", "", "/tmp/ws", "/tmp/os"
            )
            .unwrap_err(),
            ProvisionError::InvalidIoLimit
        );
        assert!(matches!(
            prepare_create_environment(
                false, "box1", &existing, "arch", &os, "gui", "gui", &presets, "35GiB", "6GiB", 4,
                "512", "200MB", "", "/etc/bad", "/tmp/ws", "/tmp/os"
            )
            .unwrap_err(),
            ProvisionError::InvalidWorkspacePath(_)
        ));
        assert_eq!(
            prepare_create_environment(
                false,
                "box1",
                &existing,
                "arch",
                &[],
                "gui",
                "gui",
                &presets,
                "35GiB",
                "6GiB",
                4,
                "512",
                "200MB",
                "",
                "",
                "/tmp/ws",
                "/tmp/os"
            )
            .unwrap_err(),
            ProvisionError::EmptyOsCatalog
        );
        assert_eq!(
            prepare_create_environment(
                false,
                "box1",
                &existing,
                "arch",
                &os,
                "gui",
                "gui",
                &[],
                "35GiB",
                "6GiB",
                4,
                "512",
                "200MB",
                "",
                "",
                "/tmp/ws",
                "/tmp/os"
            )
            .unwrap_err(),
            ProvisionError::EmptyPresetCatalog
        );
    }

    #[test]
    fn desktop_create_script_uses_the_proxy_bridge_not_legacy_xhost() {
        let desktop = "incus config set 'box' user.aibox.desktop 'on'\nincus config device add 'box' 'x11' 'proxy' connect='unix:/tmp/.X11-unix/X0' listen='unix:/tmp/.X11-unix/X0' bind='container'";
        let script = create_environment_script_with_desktop(
            "box",
            "aibox-archlinux",
            "images:archlinux",
            "/tmp/ws",
            "/tmp/cache",
            "-p default",
            "6GiB",
            "35GiB",
            4,
            true,
            "",
            "",
            "",
            false,
            desktop,
        );
        assert!(script.contains("user.aibox.desktop 'on'"));
        assert!(script.contains("'x11' 'proxy'"));
        assert!(script.contains("connect='unix:/tmp/.X11-unix/X0'"));
        assert!(!script.contains("xhost +local:"));
        let workspace_at = script
            .find("config device add \"$NAME\" workspace")
            .unwrap();
        let desktop_at = script.find("user.aibox.desktop").unwrap();
        assert!(workspace_at < desktop_at);
    }

    #[test]
    fn session_create_script_carries_display_and_audio_setup() {
        let session = "incus config set 'box' user.aibox.desktop 'on'\nincus config set 'box' user.aibox.audio 'output'\nincus config device add 'box' 'pulse' 'proxy'";
        let script = create_environment_script_with_session(
            "box",
            "aibox-archlinux",
            "images:archlinux",
            "/tmp/ws",
            "/tmp/cache",
            "-p default",
            "6GiB",
            "35GiB",
            4,
            true,
            "",
            "",
            "",
            false,
            session,
        );
        assert!(script.contains("user.aibox.desktop 'on'"));
        assert!(script.contains("user.aibox.audio 'output'"));
        assert!(script.contains("'pulse' 'proxy'"));
        assert!(script.contains("mkdir -p '/home/workspace/download'"));
        assert!(script.contains("user-dirs.dirs"));
        let workspace_at = script
            .find("config device add \"$NAME\" workspace")
            .unwrap();
        let desktop_at = script.find("user.aibox.desktop").unwrap();
        assert!(workspace_at < desktop_at);
    }

    #[test]
    fn create_environment_script_rolls_back_at_every_stage() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("aibox-prov-create-rb-{nanos}"));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        let stub_dir = base.join("stubs");
        fs::create_dir_all(&stub_dir).unwrap();

        write_executable(
            &stub_dir.join("incus"),
            r#"#!/bin/sh
echo "$@" >> "$AIBOX_STUB_LOG"
if [ "$1" = image ] && [ "$2" = info ]; then exit 1; fi
if [ "$1" = image ] && [ "$2" = copy ] && [ "$FAIL_AT" = "image_copy" ]; then exit 2; fi
if [ "$1" = launch ] && [ "$FAIL_AT" = "launch" ]; then exit 3; fi
if [ "$1" = config ] && [ "$2" = device ] && [ "$3" = add ] && [ "$FAIL_AT" = "device_add" ]; then exit 4; fi
if [ "$1" = list ]; then echo "10.0.0.5"; fi
exit 0
"#,
        );
        write_executable(&stub_dir.join("script"), "#!/bin/sh\nexec bash -c \"$4\"\n");
        write_executable(&stub_dir.join("sleep"), "#!/bin/sh\nexit 0\n");

        let path_env = format!(
            "{}:{}",
            stub_dir.display(),
            std::env::var("PATH").unwrap_or_default()
        );

        for fail_at in ["image_copy", "launch", "device_add"] {
            let ws = base.join(format!("ws-{fail_at}"));
            let log_path = base.join(format!("incus-{fail_at}.log"));
            let script_path = base.join(format!("create-{fail_at}.sh"));
            let script = create_environment_script(
                "rb-box",
                "aibox-archlinux",
                "images:archlinux",
                &ws.to_string_lossy(),
                &base.join("cache/aibox-archlinux").to_string_lossy(),
                "-p default",
                "6GiB",
                "35GiB",
                4,
                false,
                "",
                "",
                "",
                false,
            );
            fs::write(&script_path, script).unwrap();

            let output = Command::new("/bin/bash")
                .arg(&script_path)
                .env("PATH", &path_env)
                .env("AIBOX_STUB_LOG", &log_path)
                .env("FAIL_AT", fail_at)
                .env_remove("DISPLAY")
                .output()
                .unwrap();
            assert!(!output.status.success(), "expected failure at {fail_at}");
            assert!(
                !ws.exists(),
                "workspace should have been rolled back on failure at {fail_at}"
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("[rollback]"),
                "expected rollback log at {fail_at}, got: {stderr}"
            );
            let log = fs::read_to_string(&log_path).unwrap();
            if fail_at == "device_add" {
                assert!(log.contains("delete rb-box --force"), "log: {log}");
                assert!(
                    log.contains("network acl delete aibox-rb-box"),
                    "log: {log}"
                );
            } else {
                assert!(!log.contains("delete rb-box --force"), "log: {log}");
            }
        }

        // Pre-existing workspace must NOT be deleted on rollback.
        let preexisting_ws = base.join("ws-preexisting");
        fs::create_dir_all(&preexisting_ws).unwrap();
        let marker = preexisting_ws.join("keep.txt");
        fs::write(&marker, "keep").unwrap();
        let log_path = base.join("incus-preexisting.log");
        let script_path = base.join("create-preexisting.sh");
        let script = create_environment_script(
            "rb-box",
            "aibox-archlinux",
            "images:archlinux",
            &preexisting_ws.to_string_lossy(),
            &base.join("cache/aibox-archlinux").to_string_lossy(),
            "-p default",
            "6GiB",
            "35GiB",
            4,
            false,
            "",
            "",
            "",
            false,
        );
        fs::write(&script_path, script).unwrap();
        let output = Command::new("/bin/bash")
            .arg(&script_path)
            .env("PATH", &path_env)
            .env("AIBOX_STUB_LOG", &log_path)
            .env("FAIL_AT", "launch")
            .env_remove("DISPLAY")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(marker.exists(), "pre-existing workspace must be preserved");

        let _ = fs::remove_dir_all(&base);
    }
}
