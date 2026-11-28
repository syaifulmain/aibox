use std::fs::{self, OpenOptions};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use chrono::{Local, Utc};
use serde_json::Value;

use crate::cli::commands::{require_environment, run_script_with_task_lock, to_json_qt_indented};
use crate::cli::{CliIo, ExitCode, ParsedArgs};
use crate::display::{
    build_appearance_plan, build_audio_plan, build_dbus_plan, build_desktop_plan,
    build_extra_devices_plan, combine_scripts, detect_host_appearance, detect_host_audio,
    detect_host_dbus, detect_host_devices, detect_host_session, download_env, gpu_env,
    merge_env_pairs, session_bridge_script, stop_dbus_bridge_script_with, AppearancePlan,
    AudioMode, AudioPlan, DbusMode, DbusPlan, DesktopMode, DisplayPlan, ExtraDeviceToggles,
    ExtraDevicesPlan,
};
use crate::engine::Engine;
use crate::history::{record_operation_history, RecordOperationParams};
use crate::incus::list::Environment;
use crate::network::acl::build_allowlist_refresh_if_needed;
use crate::network::allowlist::SystemResolver;
use crate::process::CommandRunner;
use crate::provision::lifecycle::{
    build_prepare_script_with_desktop, enter_incus_exec_args_for_user,
    exec_incus_command_args_for_user, start_environment_script_with_desktop,
    stop_environment_script_with_session,
};
use crate::provision::user::runtime_dir_owner_script;
use crate::secrets::inject::push_environment_secrets_to_container_as;
use crate::services::system_diff::{
    capture_system_state_script, diff_system_state, format_system_change_summary,
    parse_system_state,
};
use crate::session::{
    prepare_recorded_session_paths, prune_recorded_sessions, redact_session_log_file,
    sessions_dir_path, DEFAULT_PRUNE_MAX_PER_ENV, DEFAULT_PRUNE_MAX_TOTAL_BYTES,
};
use crate::snapshot::auto::decide_auto_snapshot;
use crate::snapshot::workspace::container_workspace_path_for;
use crate::workspace::file_access::attach_file_access_fields;

fn build_auto_snapshot_if_needed(engine: &Engine, env: &Environment) -> String {
    let now = Local::now();
    let now_epoch = now.timestamp();
    let now_local_stamp = now.format("%Y%m%d-%H%M%S").to_string();
    decide_auto_snapshot(
        &env.name,
        env.is_template,
        env.auto_snapshot_enabled,
        &env.snapshots,
        &env.workspace_path,
        env.workspace_snapshot_enabled,
        engine.config.auto_snapshot_keep,
        None,
        now_epoch,
        &now_local_stamp,
    )
    .unwrap_or_default()
}

fn build_allowlist_refresh_for_env(env: &Environment) -> String {
    let resolver = SystemResolver;
    build_allowlist_refresh_if_needed(
        &env.name,
        &env.network_mode,
        &env.network_allowlist,
        env.egress_recording,
        &resolver,
    )
}

fn desktop_plan_for(env: &Environment) -> DisplayPlan {
    let mode = if crate::provision::is_vm_env_type(&env.env_type) {
        DesktopMode::Off
    } else {
        DesktopMode::from_enabled(env.desktop_enabled)
    };
    build_desktop_plan(mode, &detect_host_session())
}

fn audio_plan_for(env: &Environment) -> AudioPlan {
    let mode = if crate::provision::is_vm_env_type(&env.env_type) {
        AudioMode::Off
    } else {
        AudioMode::parse(&env.audio_mode)
    };
    let host = detect_host_audio();
    build_audio_plan(mode, &host, host.uid)
}

fn dbus_plan_for(env: &Environment) -> DbusPlan {
    let mode = if crate::provision::is_vm_env_type(&env.env_type) || !env.dbus_enabled {
        DbusMode::Off
    } else {
        DbusMode::On
    };
    build_dbus_plan(mode, &detect_host_dbus(), &env.name)
}

fn extra_devices_plan_for(env: &Environment) -> ExtraDevicesPlan {
    let toggles = if crate::provision::is_vm_env_type(&env.env_type) {
        ExtraDeviceToggles::from_envs(false, false, false)
    } else {
        ExtraDeviceToggles::from_envs(env.camera_enabled, env.usb_enabled, env.printer_enabled)
    };
    build_extra_devices_plan(toggles, &detect_host_devices())
}

fn appearance_plan_for_mode(desktop_mode: DesktopMode) -> AppearancePlan {
    if desktop_mode == DesktopMode::Off {
        return AppearancePlan::default();
    }
    build_appearance_plan(&detect_host_appearance())
}

fn appearance_plan_for(env: &Environment) -> AppearancePlan {
    let desktop_mode = if crate::provision::is_vm_env_type(&env.env_type) {
        DesktopMode::Off
    } else {
        DesktopMode::from_enabled(env.desktop_enabled)
    };
    appearance_plan_for_mode(desktop_mode)
}

fn session_environment(
    desktop: &DisplayPlan,
    audio: &AudioPlan,
    appearance: &AppearancePlan,
    dbus: &DbusPlan,
    extra: &ExtraDevicesPlan,
) -> Vec<(String, String)> {
    let downloads = download_env();
    let gpu = gpu_env();
    merge_env_pairs(&[
        &desktop.env,
        &audio.env,
        &dbus.env,
        &extra.env,
        &downloads,
        &gpu,
        &appearance.env,
    ])
}

struct SessionPlans {
    desktop: DisplayPlan,
    audio: AudioPlan,
    appearance: AppearancePlan,
    dbus: DbusPlan,
    extra: ExtraDevicesPlan,
}

impl SessionPlans {
    fn for_env(env: &Environment) -> Self {
        Self {
            desktop: desktop_plan_for(env),
            audio: audio_plan_for(env),
            appearance: appearance_plan_for(env),
            dbus: dbus_plan_for(env),
            extra: extra_devices_plan_for(env),
        }
    }

    fn bridge_script(&self, name: &str) -> String {
        session_bridge_script(
            name,
            &self.desktop,
            &self.audio,
            &self.appearance,
            &self.dbus,
            &self.extra,
        )
    }

    fn session_env(&self) -> Vec<(String, String)> {
        session_environment(
            &self.desktop,
            &self.audio,
            &self.appearance,
            &self.dbus,
            &self.extra,
        )
    }
}

fn prepare_environment_script_with_plans(
    engine: &Engine,
    env: &Environment,
    plans: &SessionPlans,
) -> String {
    let auto_snap = build_auto_snapshot_if_needed(engine, env);
    let allow_refresh = build_allowlist_refresh_for_env(env);
    let mut session = plans.bridge_script(&env.name);
    if let Some(user) = env.login_user() {
        let rt_script = runtime_dir_owner_script(&env.name, &user, &plans.session_env());
        if !rt_script.is_empty() {
            session = combine_scripts(&[session, rt_script]);
        }
    }
    build_prepare_script_with_desktop(&env.name, &auto_snap, &allow_refresh, &session)
}

fn start_environment_script_for(engine: &Engine, env: &Environment) -> String {
    let auto_snap = build_auto_snapshot_if_needed(engine, env);
    let allow_refresh = build_allowlist_refresh_for_env(env);
    let desktop = desktop_plan_for(env);
    let audio = audio_plan_for(env);
    let appearance = appearance_plan_for(env);
    let dbus = dbus_plan_for(env);
    let extra = extra_devices_plan_for(env);
    let session = session_bridge_script(&env.name, &desktop, &audio, &appearance, &dbus, &extra);
    start_environment_script_with_desktop(&env.name, &auto_snap, &allow_refresh, &session)
}

fn stop_environment_script_for(
    engine: &Engine,
    env: &Environment,
    include_auto_snapshot: bool,
) -> String {
    let auto_snap = if include_auto_snapshot {
        build_auto_snapshot_if_needed(engine, env)
    } else {
        String::new()
    };
    let dbus_stop = if env.dbus_enabled && !crate::provision::is_vm_env_type(&env.env_type) {
        stop_dbus_bridge_script_with(&env.name, &detect_host_dbus())
    } else {
        String::new()
    };
    stop_environment_script_with_session(&env.name, include_auto_snapshot, &auto_snap, &dbus_stop)
}

fn find_executable(name: &str) -> bool {
    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            if let Ok(meta) = fs::metadata(&candidate) {
                if meta.permissions().mode() & 0o111 != 0 {
                    return true;
                }
            }
        }
    }
    false
}

fn run_pre_steps_for_exec_with_plans(
    engine: &Engine,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
    env: &Environment,
    plans: &SessionPlans,
    label: &str,
) -> i32 {
    if !engine.acquire_cli_task_lock(label, &env.name) {
        io.write_stderr(b"Error: Another AIBox background task is currently running.\n");
        return ExitCode::Busy.as_i32();
    }

    let script = prepare_environment_script_with_plans(engine, env, plans);
    let out = runner.run("/bin/bash", &["-c", &script]);
    let mut combined = String::new();
    combined.push_str(&out.std_out);
    if !out.std_err.is_empty() {
        combined.push_str(&out.std_err);
    }

    if combined.contains("Warning:") {
        let started_at = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let secret_vals = engine.secrets.all_secret_values();
        let _ = record_operation_history(&RecordOperationParams {
            state_dir: &engine.paths.state_dir,
            description: &format!("Auto-snapshot warning for '{}'", env.name),
            env_name: &env.name,
            status: "warning",
            exit_code: out.exit_code,
            started_at_iso: &started_at,
            log_bytes: combined.as_bytes(),
            secret_values: &secret_vals,
            reason: "",
            max_entries: 50,
        });
        io.write_stderr(combined.as_bytes());
    }
    engine.release_cli_task_lock();

    if out.exit_code != 0 {
        if !combined.is_empty() && !combined.contains("Warning:") {
            io.write_stderr(combined.as_bytes());
        }
        io.write_stderr(
            format!("Error: Failed to prepare environment '{}'.\n", env.name).as_bytes(),
        );
        return ExitCode::Failed.as_i32();
    }
    ExitCode::Ok.as_i32()
}

fn push_secrets_for_env(engine: &Engine, env: &Environment) -> Result<String, String> {
    let resolved =
        engine
            .secrets
            .resolve_environment_secrets(&env.name, &env.secret_names, false)?;
    let owner = env.login_user().map(|u| (u.uid, u.gid));
    Ok(push_environment_secrets_to_container_as(&env.name, &resolved, owner)?.unwrap_or_default())
}

pub fn run_list(engine: &mut Engine, io: &mut dyn CliIo, runner: &dyn CommandRunner) -> i32 {
    if let Err(err) = engine.refresh_sync(runner) {
        io.write_stderr(format!("Error: {err}\n").as_bytes());
        return ExitCode::Failed.as_i32();
    }

    let mut rows: Vec<Value> = Vec::with_capacity(engine.environments.len());
    for (idx, env) in engine.environments.iter().enumerate() {
        let mut val = serde_json::to_value(env).unwrap_or(Value::Null);
        attach_file_access_fields(&mut val, engine.file_access.envs.get(idx));
        rows.push(val);
    }

    let json_text = to_json_qt_indented(&rows).unwrap_or_else(|_| "[\n]\n".to_string());
    io.write_stdout(json_text.as_bytes());
    ExitCode::Ok.as_i32()
}

pub fn run_enter(
    engine: &mut Engine,
    parsed: &ParsedArgs,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
) -> i32 {
    let env = match require_environment(engine, io, runner, &parsed.env_name, true) {
        Ok(e) => e,
        Err(rc) => return rc,
    };

    let plans = SessionPlans::for_env(&env);
    let pre_rc = run_pre_steps_for_exec_with_plans(
        engine,
        io,
        runner,
        &env,
        &plans,
        &format!("Entering '{}' (CLI)", parsed.env_name),
    );
    if pre_rc != 0 {
        return pre_rc;
    }

    let remote_env_path = match push_secrets_for_env(engine, &env) {
        Ok(p) => p,
        Err(err) => {
            io.write_stderr(format!("Error: {err}\n").as_bytes());
            return ExitCode::Failed.as_i32();
        }
    };

    let term = std::env::var("TERM").unwrap_or_default();
    let c_ws_path = container_workspace_path_for(&parsed.env_name, false);
    let session_env = plans.session_env();
    let login_user = env.login_user();
    // The plan owns DISPLAY when desktop integration is on; passing an empty
    // display keeps Off purely headless instead of falling back to `:0`.
    let exec_args = enter_incus_exec_args_for_user(
        &parsed.env_name,
        &remote_env_path,
        &c_ws_path,
        "",
        &term,
        &session_env,
        login_user.as_ref(),
    );

    if env.record_enabled && find_executable("script") {
        if let Some((log_file, timing_file)) =
            prepare_recorded_session_paths(&engine.paths.state_dir, &parsed.env_name)
        {
            for path in [&log_file, &timing_file] {
                if OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(false)
                    .mode(0o600)
                    .open(path)
                    .is_ok()
                {
                    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
                }
            }
            let mut quoted_cmd = String::from("incus");
            for arg in &exec_args {
                let q = arg.replace('\'', "'\\''");
                quoted_cmd.push_str(&format!(" '{q}'"));
            }
            let script_args = vec![
                "-q".to_string(),
                "-f".to_string(),
                "--log-out".to_string(),
                log_file.clone(),
                "--log-timing".to_string(),
                timing_file,
                "-c".to_string(),
                quoted_cmd,
            ];
            let exit_code = io.run_forwarded("script", &script_args);
            redact_session_log_file(&log_file, &engine.secrets.all_secret_values());
            let _ = fs::set_permissions(&log_file, fs::Permissions::from_mode(0o600));
            prune_recorded_sessions(
                &sessions_dir_path(&engine.paths.state_dir),
                DEFAULT_PRUNE_MAX_PER_ENV,
                DEFAULT_PRUNE_MAX_TOTAL_BYTES,
            );
            return exit_code;
        }
    }

    io.exec_replace("incus", &exec_args)
}

pub fn run_exec(
    engine: &mut Engine,
    parsed: &ParsedArgs,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
) -> i32 {
    let env = match require_environment(engine, io, runner, &parsed.env_name, true) {
        Ok(e) => e,
        Err(rc) => return rc,
    };

    let plans = SessionPlans::for_env(&env);
    let pre_rc = run_pre_steps_for_exec_with_plans(
        engine,
        io,
        runner,
        &env,
        &plans,
        &format!("Executing command in '{}' (CLI)", parsed.env_name),
    );
    if pre_rc != 0 {
        return pre_rc;
    }

    let remote_env_path = match push_secrets_for_env(engine, &env) {
        Ok(p) => p,
        Err(err) => {
            io.write_stderr(format!("Error: {err}\n").as_bytes());
            return ExitCode::Failed.as_i32();
        }
    };

    let c_ws_path = container_workspace_path_for(&parsed.env_name, false);
    let session_env = plans.session_env();
    let login_user = env.login_user();
    let args = exec_incus_command_args_for_user(
        &parsed.env_name,
        &parsed.exec_command,
        &remote_env_path,
        &c_ws_path,
        &session_env,
        login_user.as_ref(),
    );
    let before_state = if parsed.summary {
        let probe = capture_system_state_script(&parsed.env_name);
        let out = runner.run("/bin/bash", &["-c", &probe]);
        if out.exit_code == 0 {
            Some(parse_system_state(&out.std_out))
        } else {
            io.write_stderr(
                b"Warning: Could not capture pre-execution system state for --summary.\n",
            );
            None
        }
    } else {
        None
    };

    let rc = if let Some(secs) = parsed.timeout_secs {
        let mut timeout_args = vec![
            "--signal=TERM".to_string(),
            "--kill-after=10s".to_string(),
            format!("{secs}s"),
            "incus".to_string(),
        ];
        timeout_args.extend(args);
        let rc = io.run_forwarded("timeout", &timeout_args);
        if rc == 124 || rc == 137 {
            io.write_stderr(format!("Warning: Command timed out after {secs}s.\n").as_bytes());
        }
        rc
    } else {
        io.run_forwarded("incus", &args)
    };

    if let Some(before) = before_state {
        let probe = capture_system_state_script(&parsed.env_name);
        let out = runner.run("/bin/bash", &["-c", &probe]);
        if out.exit_code != 0 {
            io.write_stderr(
                b"Warning: Could not capture post-execution system state for --summary.\n",
            );
        } else {
            let after = parse_system_state(&out.std_out);
            let _ = engine.refresh_sync(runner);
            let latest_snapshots = engine
                .environments
                .iter()
                .find(|e| e.name == parsed.env_name)
                .map(|e| &e.snapshots)
                .unwrap_or(&env.snapshots);
            let rollback_snap = latest_snapshots
                .iter()
                .rev()
                .find(|s| s.starts_with("auto-"))
                .or_else(|| latest_snapshots.last())
                .map(String::as_str)
                .unwrap_or("");
            let summary = diff_system_state(&before, &after, rollback_snap);
            let text = format_system_change_summary(&parsed.env_name, &summary);
            io.write_stderr(text.as_bytes());
        }
    }
    rc
}

pub fn run_start(
    engine: &mut Engine,
    parsed: &ParsedArgs,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
) -> i32 {
    let env = match require_environment(engine, io, runner, &parsed.env_name, true) {
        Ok(e) => e,
        Err(rc) => return rc,
    };
    if engine.is_task_lock_active().0 {
        io.write_stderr(b"Error: Another AIBox background task is currently running.\n");
        return ExitCode::Busy.as_i32();
    }
    if env.running {
        io.write_stdout(
            format!("Environment '{}' is already running.\n", parsed.env_name).as_bytes(),
        );
        return ExitCode::Ok.as_i32();
    }
    let script = start_environment_script_for(engine, &env);
    run_script_with_task_lock(
        engine,
        io,
        runner,
        &format!("Starting '{}'", parsed.env_name),
        &parsed.env_name,
        &script,
        true,
    )
}

pub fn run_stop(
    engine: &mut Engine,
    parsed: &ParsedArgs,
    io: &mut dyn CliIo,
    runner: &dyn CommandRunner,
) -> i32 {
    let env = match require_environment(engine, io, runner, &parsed.env_name, true) {
        Ok(e) => e,
        Err(rc) => return rc,
    };
    if engine.is_task_lock_active().0 {
        io.write_stderr(b"Error: Another AIBox background task is currently running.\n");
        return ExitCode::Busy.as_i32();
    }
    if !env.running {
        io.write_stdout(
            format!("Environment '{}' is already stopped.\n", parsed.env_name).as_bytes(),
        );
        return ExitCode::Ok.as_i32();
    }
    let script = stop_environment_script_for(engine, &env, false);
    run_script_with_task_lock(
        engine,
        io,
        runner,
        &format!("Stopping '{}'", parsed.env_name),
        &parsed.env_name,
        &script,
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::DeviceSpec;
    use crate::provision::lifecycle::{
        enter_incus_exec_args_with_env, exec_incus_command_args_with_env,
    };

    fn appearance_plan() -> AppearancePlan {
        AppearancePlan {
            devices: vec![DeviceSpec {
                name: "fonts-system".to_string(),
                kind: "disk".to_string(),
                options: vec![
                    ("source".to_string(), "/usr/share/fonts".to_string()),
                    ("path".to_string(), "/usr/share/fonts".to_string()),
                    ("readonly".to_string(), "true".to_string()),
                ],
            }],
            env: vec![
                ("XCURSOR_THEME".to_string(), "Bibata".to_string()),
                ("XCURSOR_SIZE".to_string(), "32".to_string()),
                ("GTK_THEME".to_string(), "Adwaita:dark".to_string()),
            ],
            warnings: Vec::new(),
        }
    }

    fn desktop_plan() -> DisplayPlan {
        DisplayPlan {
            mode: DesktopMode::On,
            devices: Vec::new(),
            env: vec![
                ("DISPLAY".to_string(), ":0".to_string()),
                ("WAYLAND_DISPLAY".to_string(), "wayland-0".to_string()),
            ],
            warnings: Vec::new(),
        }
    }

    fn empty_audio_plan() -> AudioPlan {
        AudioPlan {
            mode: AudioMode::Off,
            devices: Vec::new(),
            env: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn empty_dbus_plan() -> DbusPlan {
        let host = crate::display::HostDbus {
            uid: 1000,
            gid: 1000,
            runtime_dir: "/run/user/1000".to_string(),
            bus_address: "unix:path=/run/user/1000/bus".to_string(),
            aibox_bin: "aibox".to_string(),
            xdg_dbus_proxy_bin: "/usr/bin/xdg-dbus-proxy".to_string(),
        };
        build_dbus_plan(DbusMode::Off, &host, "box")
    }

    fn empty_extra_plan() -> ExtraDevicesPlan {
        build_extra_devices_plan(
            ExtraDeviceToggles::from_envs(false, false, false),
            &crate::display::HostDevices::default(),
        )
    }

    #[test]
    fn session_environment_adds_appearance_env_after_the_other_bridges() {
        let desktop = desktop_plan();
        let audio = AudioPlan {
            mode: AudioMode::Off,
            devices: Vec::new(),
            env: vec![("DISPLAY".to_string(), ":99".to_string())],
            warnings: Vec::new(),
        };
        let appearance = appearance_plan();
        let dbus = empty_dbus_plan();
        let env = session_environment(&desktop, &audio, &appearance, &dbus, &empty_extra_plan());

        assert_eq!(
            env.iter()
                .find(|(key, _)| key == "DISPLAY")
                .map(|(_, value)| value.as_str()),
            Some(":0"),
            "display still wins over a later duplicate"
        );
        for (key, value) in [
            ("XCURSOR_THEME", "Bibata"),
            ("XCURSOR_SIZE", "32"),
            ("GTK_THEME", "Adwaita:dark"),
        ] {
            assert_eq!(
                env.iter()
                    .find(|(name, _)| name == key)
                    .map(|(_, value)| value.as_str()),
                Some(value),
                "env {key}"
            );
        }
    }

    #[test]
    fn enter_and_exec_args_carry_appearance_environment_when_desktop_is_on() {
        let env = session_environment(
            &desktop_plan(),
            &empty_audio_plan(),
            &appearance_plan(),
            &empty_dbus_plan(),
            &empty_extra_plan(),
        );
        for args in [
            enter_incus_exec_args_with_env("box", "", "/home/workspace", "", "", &env),
            exec_incus_command_args_with_env(
                "box",
                &["firefox".to_string()],
                "",
                "/home/workspace",
                &env,
            ),
        ] {
            for pair in [
                "XCURSOR_THEME=Bibata",
                "XCURSOR_SIZE=32",
                "GTK_THEME=Adwaita:dark",
            ] {
                assert!(
                    args.iter().any(|arg| arg == pair),
                    "missing {pair} in {args:?}"
                );
            }
        }
    }

    #[test]
    fn session_bridge_script_includes_appearance_devices() {
        let script = session_bridge_script(
            "box",
            &desktop_plan(),
            &empty_audio_plan(),
            &appearance_plan(),
            &empty_dbus_plan(),
            &empty_extra_plan(),
        );
        assert!(script.contains("incus config device add 'box' 'fonts-system' 'disk'"));
        assert!(script.contains("readonly='true'"));
        assert!(script.contains("incus config device show 'box'"));
        assert!(script.contains("fonts-system"));
    }

    #[test]
    fn session_bridge_script_and_env_carry_extra_devices() {
        let extra = build_extra_devices_plan(
            ExtraDeviceToggles::from_envs(true, false, true),
            &crate::display::HostDevices {
                video_nodes: vec!["/dev/video0".to_string()],
                input_nodes: Vec::new(),
                cups_socket: "/run/cups/cups.sock".to_string(),
            },
        );
        let script = session_bridge_script(
            "box",
            &desktop_plan(),
            &empty_audio_plan(),
            &appearance_plan(),
            &empty_dbus_plan(),
            &extra,
        );
        assert!(script.contains("incus config device add 'box' 'camera-video0' 'unix-char'"));
        assert!(script.contains("incus config device add 'box' 'printer' 'proxy'"));
        assert!(script.contains("required='false'"));

        let env = session_environment(
            &desktop_plan(),
            &empty_audio_plan(),
            &appearance_plan(),
            &empty_dbus_plan(),
            &extra,
        );
        assert!(env
            .iter()
            .any(|(key, value)| { key == "CUPS_SERVER" && value == "/run/cups/cups.sock" }));
    }

    #[test]
    fn appearance_plan_is_empty_when_desktop_is_off() {
        assert_eq!(
            appearance_plan_for_mode(DesktopMode::Off),
            AppearancePlan::default()
        );
        // The GUI/CLI wrappers force DesktopMode::Off for VMs before this call.
        let on = appearance_plan_for_mode(DesktopMode::On);
        assert!(
            !on.env.is_empty(),
            "an on plan always carries cursor defaults"
        );
    }
}
