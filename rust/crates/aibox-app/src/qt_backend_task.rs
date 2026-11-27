use std::collections::BTreeSet;
use std::path::Path;
use std::pin::Pin;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

use aibox_core::display::{
    apply_appearance_devices_script, apply_audio_devices_script, apply_dbus_devices_script,
    apply_devices_script, apply_extra_devices_script, build_appearance_plan, build_audio_plan,
    build_dbus_plan, build_desktop_plan, build_extra_devices_plan, combine_scripts,
    detect_host_appearance, detect_host_audio, detect_host_dbus, detect_host_devices,
    detect_host_session, download_dir_script, download_env, gpu_env, merge_env_pairs,
    stop_dbus_bridge_script_with, AppearancePlan, AudioMode, AudioPlan, DbusMode, DbusPlan,
    DesktopMode, DisplayPlan, ExtraDeviceToggles, ExtraDevicesPlan,
};
use aibox_core::health::idle::{
    effective_idle_stop_minutes, evaluate_idle_decision, parse_active_incus_exec_envs,
    parse_idle_stop_override, IdleCpuSample, IdleDecisionInput,
};
use aibox_core::health::watchdog::{evaluate_all_watchdogs, WatchdogEnvInput, WatchdogNotifyKind};
use aibox_core::history::{record_operation_history, RecordOperationParams};
use aibox_core::network::acl::acl_apply_shell_snippet;
use aibox_core::network::allowlist::{
    extract_allowlist_hostnames, parse_allowlist_entries, resolve_allowlist_cidrs, HostResolver,
    SystemResolver,
};
use aibox_core::provision::lifecycle::{
    build_enter_command_with_env, build_prepare_script_with_desktop,
    stop_environment_script_with_session,
};
use aibox_core::secrets::inject::{build_secret_env_file_content, create_host_temp_secret_file};
use aibox_core::session::{
    build_session_wrap_command, prepare_recorded_session_paths, sessions_dir_path,
};
use aibox_core::snapshot::auto::decide_auto_snapshot;
use aibox_core::task::files::read_task_meta;
use aibox_core::task::lock::is_task_lock_active;
use aibox_core::task::progress::{
    cancel_running_task, evaluate_attach_existing_task, evaluate_check_background_task,
    AttachOutcome, TaskProgressState,
};
use aibox_core::task::runner::{prepare_shell_task, PrepareShellTaskParams};
use aibox_core::util::shell::shell_quote;
use aibox_core::util::time::{now_compact_stamp, now_epoch_millis, now_epoch_secs, now_utc_iso};

use crate::qt_backend::{find_executable, launch_command_in_terminal, qobject, spawn_detached};
use crate::qt_convert::json_array_to_qvariant_list;

static REMOTE_ENV_SEQ: AtomicU64 = AtomicU64::new(1);

pub(crate) struct NoopResolver;

impl HostResolver for NoopResolver {
    fn resolve_ipv4(&self, _host: &str) -> Vec<String> {
        Vec::new()
    }
}

fn send_desktop_notification(critical: bool, title: &str, body: &str) {
    if !qobject::aibox_is_gui_session() || find_executable("notify-send").is_none() {
        return;
    }
    let mut args = Vec::new();
    if critical {
        args.push("-u");
        args.push("critical");
    }
    args.push("-a");
    args.push("AIBox");
    args.push(title);
    args.push(body);
    let _ = spawn_detached("notify-send", &args, None);
}

impl qobject::Backend {
    pub(crate) fn set_busy(mut self: Pin<&mut Self>, busy: bool, status_msg: &str) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.busy = busy;
            rust.status_message = QString::from(status_msg);
        }
        self.as_mut().busy_changed();
        self.as_mut().status_message_changed();
    }

    pub(crate) fn run_shell_task(
        self: Pin<&mut Self>,
        description: &str,
        script: &str,
        created_env_name: &str,
        target_env_name: &str,
    ) {
        self.run_shell_task_with_reason(description, script, created_env_name, target_env_name, "");
    }

    pub(crate) fn run_shell_task_with_reason(
        mut self: Pin<&mut Self>,
        description: &str,
        script: &str,
        created_env_name: &str,
        target_env_name: &str,
        reason: &str,
    ) {
        if *self.busy() {
            if !self.downloading_os_id.is_empty() {
                self.as_mut().rust_mut().downloading_os_id = QString::default();
                self.as_mut().rebuild_os_catalog();
            }
            return;
        }
        let state_dir = self.engine.paths.state_dir.clone();
        if is_task_lock_active(Path::new(&state_dir), *self.busy()).0 {
            if !self.downloading_os_id.is_empty() {
                self.as_mut().rust_mut().downloading_os_id = QString::default();
                self.as_mut().rebuild_os_catalog();
            }
            self.as_mut()
                .set_error_message("Another AIBox task is currently running.");
            return;
        }

        self.as_mut().clear_error();
        {
            let mut rust = self.as_mut().rust_mut();
            rust.pending_env_name = QString::from(created_env_name);
            rust.log_offset = 0;
            rust.line_buffer.clear();
            rust.has_exit_code = false;
            rust.last_exit_code = -1;
            rust.task_progress = 5;
            rust.task_stage = QString::from(description);
            rust.task_log = QString::from("Initializing...");
        }
        self.as_mut().set_busy(true, description);
        self.as_mut().task_progress_changed();

        let downloading_os_id = self.downloading_os_id().to_string();
        let launch_cmd = match prepare_shell_task(&PrepareShellTaskParams {
            state_dir: Path::new(&state_dir),
            description,
            script,
            created_env_name,
            target_env_name,
            downloading_os_id: &downloading_os_id,
            reason,
        }) {
            Ok(cmd) => cmd,
            Err(err) => {
                let had_dl = !self.downloading_os_id.is_empty();
                {
                    let mut rust = self.as_mut().rust_mut();
                    rust.pending_env_name = QString::default();
                    rust.downloading_os_id = QString::default();
                }
                if had_dl {
                    self.as_mut().rebuild_os_catalog();
                }
                self.as_mut().set_busy(false, "");
                self.as_mut().set_error_message(&err);
                return;
            }
        };

        if !launch_cmd.is_empty() {
            let _ = Command::new("/bin/sh").arg("-c").arg(&launch_cmd).status();
        }
        self.set_task_watch_active(true);
    }

    pub(crate) fn attach_existing_task(mut self: Pin<&mut Self>) {
        let state_dir = self.engine.paths.state_dir.clone();
        let secret_values = self.engine.secrets.all_secret_values();
        let outcome = evaluate_attach_existing_task(Path::new(&state_dir), &secret_values, 50);

        match outcome {
            AttachOutcome::NoTask => {}
            AttachOutcome::StillRunning {
                description,
                env_name,
                downloading_os_id,
                log_offset,
                progress,
            } => {
                let task_log = if progress.task_log.is_empty() {
                    "Resuming background task monitor...".to_string()
                } else {
                    progress.task_log
                };
                {
                    let mut rust = self.as_mut().rust_mut();
                    rust.pending_env_name = QString::from(env_name.as_str());
                    rust.downloading_os_id = QString::from(downloading_os_id.as_str());
                    rust.log_offset = log_offset;
                    rust.line_buffer = progress.line_buffer;
                    rust.has_exit_code = progress.has_exit_code;
                    rust.last_exit_code = progress.last_exit_code;
                    rust.task_progress = progress.task_progress;
                    rust.task_stage = QString::from(progress.task_stage.as_str());
                    rust.task_log = QString::from(task_log.as_str());
                }
                self.as_mut().set_busy(true, &description);
                self.as_mut().task_progress_changed();
                self.as_mut().rebuild_os_catalog();
                self.set_task_watch_active(true);
            }
            AttachOutcome::Finished {
                env_name,
                exit_code,
                has_exit_code,
                task_log,
                history_json,
            } => {
                if !history_json.is_empty() {
                    if let Ok(serde_json::Value::Array(arr)) = serde_json::from_str(&history_json) {
                        let q_hist = json_array_to_qvariant_list(&arr);
                        {
                            let mut rust = self.as_mut().rust_mut();
                            rust.engine.set_history_cache(arr);
                            rust.operation_history = q_hist;
                        }
                        self.as_mut().operation_history_changed();
                    }
                }
                {
                    let mut rust = self.as_mut().rust_mut();
                    rust.downloading_os_id = QString::default();
                    rust.pending_env_name = QString::default();
                    rust.log_offset = 0;
                    rust.line_buffer.clear();
                    rust.has_exit_code = has_exit_code;
                    rust.last_exit_code = exit_code;
                    rust.task_log = QString::from(task_log.as_str());
                }
                self.as_mut().rebuild_os_catalog();
                self.as_mut().set_busy(false, "");
                if has_exit_code && exit_code == 0 {
                    let msg = if env_name.is_empty() {
                        "Background operation completed successfully.".to_string()
                    } else {
                        format!("Environment '{env_name}' finished creating in the background.")
                    };
                    self.as_mut().set_status_message(&msg);
                } else if has_exit_code && exit_code != 0 {
                    self.as_mut().set_error_message(&format!(
                        "Previous background operation failed: {task_log}"
                    ));
                }
            }
        }
    }

    pub(crate) fn check_background_task(mut self: Pin<&mut Self>) {
        let watch_active = self
            .task_watch_state
            .0
            .lock()
            .map(|w| w.active)
            .unwrap_or(false);
        if !watch_active {
            return;
        }
        let state = TaskProgressState {
            task_progress: *self.task_progress(),
            task_stage: self.task_stage().to_string(),
            task_log: self.task_log().to_string(),
            has_exit_code: self.has_exit_code,
            last_exit_code: self.last_exit_code,
            pending_env_name: self.pending_env_name().to_string(),
            line_buffer: self.line_buffer.clone(),
        };
        let state_dir = self.engine.paths.state_dir.clone();
        let log_offset = self.log_offset;
        let secret_values = self.engine.secrets.all_secret_values();
        let res = evaluate_check_background_task(
            Path::new(&state_dir),
            log_offset,
            state,
            &secret_values,
            50,
        );

        {
            let mut rust = self.as_mut().rust_mut();
            rust.log_offset = res.log_offset;
            rust.line_buffer = res.progress.line_buffer;
            rust.task_progress = res.progress.task_progress;
            rust.task_stage = QString::from(res.progress.task_stage.as_str());
            rust.task_log = QString::from(res.progress.task_log.as_str());
            rust.has_exit_code = res.progress.has_exit_code;
            rust.last_exit_code = res.progress.last_exit_code;
        }
        if res.progress_changed {
            self.as_mut().task_progress_changed();
        }
        if !res.finished {
            return;
        }

        self.set_task_watch_active(false);
        if !res.history_json.is_empty() {
            if let Ok(serde_json::Value::Array(arr)) = serde_json::from_str(&res.history_json) {
                let q_hist = json_array_to_qvariant_list(&arr);
                {
                    let mut rust = self.as_mut().rust_mut();
                    rust.engine.set_history_cache(arr);
                    rust.operation_history = q_hist;
                }
                self.as_mut().operation_history_changed();
            }
        }

        {
            let mut rust = self.as_mut().rust_mut();
            rust.downloading_os_id = QString::default();
            rust.pending_env_name = QString::default();
            rust.log_offset = 0;
            rust.line_buffer.clear();
        }
        self.as_mut().rebuild_os_catalog();
        self.as_mut().set_busy(false, "");
        self.as_mut().refresh();
        self.as_mut().refresh_backups();
        if !self.stale_report.is_empty() {
            self.as_mut().refresh_stale_report();
        }

        if res.exit_code == 0 {
            if !res.finished_env.is_empty() {
                self.as_mut()
                    .set_status_message(&format!("Environment '{}' is ready.", res.finished_env));
                self.as_mut()
                    .environment_created(&QString::from(res.finished_env.as_str()));
            }
        } else {
            let msg = if res.last_log.is_empty() {
                format!("Operation failed (exit code {}).", res.exit_code)
            } else {
                res.last_log
            };
            self.as_mut().set_error_message(&msg);
        }
    }

    pub fn cancel_task(mut self: Pin<&mut Self>) {
        if !*self.busy() {
            return;
        }
        self.set_task_watch_active(false);
        let state_dir = self.engine.paths.state_dir.clone();
        let task_stage = self.task_stage().to_string();
        let cleanup_name = self.pending_env_name().to_string();
        let secret_values = self.engine.secrets.all_secret_values();
        let hist_json = cancel_running_task(
            Path::new(&state_dir),
            &task_stage,
            &cleanup_name,
            &secret_values,
            50,
        );
        if !hist_json.is_empty() {
            if let Ok(serde_json::Value::Array(arr)) = serde_json::from_str(&hist_json) {
                let q_hist = json_array_to_qvariant_list(&arr);
                {
                    let mut rust = self.as_mut().rust_mut();
                    rust.engine.set_history_cache(arr);
                    rust.operation_history = q_hist;
                }
                self.as_mut().operation_history_changed();
            }
        }

        {
            let mut rust = self.as_mut().rust_mut();
            rust.downloading_os_id = QString::default();
            rust.pending_env_name = QString::default();
            rust.log_offset = 0;
            rust.line_buffer.clear();
        }
        self.as_mut().rebuild_os_catalog();
        self.as_mut().set_busy(false, "Operation cancelled.");

        if !cleanup_name.is_empty() {
            let ws_path = self.workspace_path_for(&cleanup_name);
            let cmd = format!(
                "incus delete {} --force >/dev/null 2>&1 || true; rmdir {} 2>/dev/null || true",
                shell_quote(&cleanup_name),
                shell_quote(&ws_path)
            );
            let _ = spawn_detached("/bin/bash", &["-c", &cmd], None);
        }

        self.as_mut()
            .schedule_refresh(Duration::from_millis(800), true);
        self.as_mut()
            .schedule_status_clear(Duration::from_millis(3000));
    }

    pub fn enter_environment(mut self: Pin<&mut Self>, name: &QString) {
        let name_s = name.to_string();
        if name_s.is_empty() {
            return;
        }
        if self.demo_mode {
            self.as_mut()
                .set_status_message(&format!("[Demo] Opened terminal in '{name_s}'."));
            self.as_mut()
                .schedule_status_clear(Duration::from_millis(3000));
            return;
        }
        if !self.enter_env_name.is_empty() {
            return;
        }
        let env_opt = self
            .engine
            .environments
            .iter()
            .find(|e| e.name == name_s)
            .cloned();
        if let Some(ref env) = env_opt {
            if env.is_template {
                self.as_mut().set_error_message(
                    "Templates cannot be entered directly. Create a new environment from this template first.",
                );
                return;
            }
        }

        {
            let mut rust = self.as_mut().rust_mut();
            rust.enter_env_name = name_s.clone();
            rust.enter_allowlist_snippet.clear();
        }
        let prep_msg = format!("Preparing environment '{name_s}'...");
        if self.status_message().to_string() != prep_msg {
            self.as_mut().set_status_message(&prep_msg);
        }

        let (network_mode, raw_allow, egress_recording) = env_opt
            .as_ref()
            .map(|e| {
                (
                    e.network_mode.clone(),
                    e.network_allowlist.clone(),
                    e.egress_recording,
                )
            })
            .unwrap_or_default();
        let entries = parse_allowlist_entries(&raw_allow);
        if network_mode != "allowlist" || entries.is_empty() {
            self.as_mut().start_enter_prepare_worker();
            return;
        }

        let hostnames = extract_allowlist_hostnames(&entries);
        if hostnames.is_empty() {
            let (cidrs, _) = resolve_allowlist_cidrs(&entries, &NoopResolver);
            if !cidrs.is_empty() {
                let snippet =
                    acl_apply_shell_snippet(&name_s, &raw_allow, &cidrs, egress_recording);
                let mut rust = self.as_mut().rust_mut();
                rust.enter_allowlist_snippet = snippet;
            }
            self.as_mut().start_enter_prepare_worker();
            return;
        }

        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let resolver = SystemResolver;
            let (cidrs, _) = resolve_allowlist_cidrs(&entries, &resolver);
            let _ = qt_thread.queue(move |mut backend| {
                if backend.enter_env_name != name_s {
                    return;
                }
                if !cidrs.is_empty() {
                    let snippet =
                        acl_apply_shell_snippet(&name_s, &raw_allow, &cidrs, egress_recording);
                    let mut rust = backend.as_mut().rust_mut();
                    rust.enter_allowlist_snippet = snippet;
                }
                backend.as_mut().start_enter_prepare_worker();
            });
        });
    }

    fn auto_snapshot_snippet_for(&self, env_name: &str) -> String {
        self.engine
            .environments
            .iter()
            .find(|e| e.name == env_name)
            .and_then(|env| {
                decide_auto_snapshot(
                    &env.name,
                    env.is_template,
                    env.auto_snapshot_enabled,
                    &env.snapshots,
                    &env.workspace_path,
                    env.workspace_snapshot_enabled,
                    self.engine.config.auto_snapshot_keep,
                    None,
                    now_epoch_secs(),
                    &now_compact_stamp(),
                )
            })
            .unwrap_or_default()
    }

    fn desktop_plan_for_env(&self, env_name: &str) -> DisplayPlan {
        let env = self
            .engine
            .environments
            .iter()
            .find(|env| env.name == env_name);
        let mode = match env {
            Some(env) if !aibox_core::provision::is_vm_env_type(&env.env_type) => {
                DesktopMode::from_enabled(env.desktop_enabled)
            }
            _ => DesktopMode::Off,
        };
        build_desktop_plan(mode, &detect_host_session())
    }

    fn audio_plan_for_env(&self, env_name: &str) -> AudioPlan {
        let env = self
            .engine
            .environments
            .iter()
            .find(|env| env.name == env_name);
        let mode = match env {
            Some(env) if !aibox_core::provision::is_vm_env_type(&env.env_type) => {
                AudioMode::parse(&env.audio_mode)
            }
            _ => AudioMode::Off,
        };
        let host = detect_host_audio();
        build_audio_plan(mode, &host, host.uid)
    }

    fn appearance_plan_for_env(&self, env_name: &str) -> AppearancePlan {
        let env = self
            .engine
            .environments
            .iter()
            .find(|env| env.name == env_name);
        let desktop_mode = match env {
            Some(env) if !aibox_core::provision::is_vm_env_type(&env.env_type) => {
                DesktopMode::from_enabled(env.desktop_enabled)
            }
            _ => DesktopMode::Off,
        };
        if desktop_mode == DesktopMode::Off {
            return AppearancePlan::default();
        }
        build_appearance_plan(&detect_host_appearance())
    }

    fn dbus_plan_for_env(&self, env_name: &str) -> DbusPlan {
        let env = self
            .engine
            .environments
            .iter()
            .find(|env| env.name == env_name);
        let mode = match env {
            Some(env)
                if !aibox_core::provision::is_vm_env_type(&env.env_type) && env.dbus_enabled =>
            {
                DbusMode::On
            }
            _ => DbusMode::Off,
        };
        build_dbus_plan(mode, &detect_host_dbus(), env_name)
    }

    fn extra_devices_plan_for_env(&self, env_name: &str) -> ExtraDevicesPlan {
        let env = self
            .engine
            .environments
            .iter()
            .find(|env| env.name == env_name);
        let toggles = match env {
            Some(env) if !aibox_core::provision::is_vm_env_type(&env.env_type) => {
                ExtraDeviceToggles::from_envs(
                    env.camera_enabled,
                    env.usb_enabled,
                    env.printer_enabled,
                )
            }
            _ => ExtraDeviceToggles::from_envs(false, false, false),
        };
        build_extra_devices_plan(toggles, &detect_host_devices())
    }

    pub(crate) fn dbus_stop_script_for_env(&self, env_name: &str) -> String {
        let enabled = self
            .engine
            .environments
            .iter()
            .find(|env| env.name == env_name)
            .is_some_and(|env| {
                env.dbus_enabled && !aibox_core::provision::is_vm_env_type(&env.env_type)
            });
        if enabled {
            stop_dbus_bridge_script_with(env_name, &detect_host_dbus())
        } else {
            String::new()
        }
    }

    fn session_env_for_env(&self, env_name: &str) -> Vec<(String, String)> {
        let desktop = self.desktop_plan_for_env(env_name);
        let audio = self.audio_plan_for_env(env_name);
        let appearance = self.appearance_plan_for_env(env_name);
        let dbus = self.dbus_plan_for_env(env_name);
        let extra = self.extra_devices_plan_for_env(env_name);
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

    /// Applies display, audio, appearance, download, filtered-bus, and opt-in
    /// extra devices for GUI Start, not only Enter.
    pub(crate) fn session_bridge_script_for_env(&self, env_name: &str) -> String {
        let desktop = self.desktop_plan_for_env(env_name);
        let audio = self.audio_plan_for_env(env_name);
        let appearance = self.appearance_plan_for_env(env_name);
        let dbus = self.dbus_plan_for_env(env_name);
        let extra = self.extra_devices_plan_for_env(env_name);
        combine_scripts(&[
            apply_devices_script(env_name, &desktop),
            apply_audio_devices_script(env_name, &audio),
            apply_appearance_devices_script(env_name, &appearance),
            apply_dbus_devices_script(env_name, &dbus),
            apply_extra_devices_script(env_name, &extra),
            download_dir_script(env_name),
        ])
    }

    fn start_enter_prepare_worker(self: Pin<&mut Self>) {
        let env_name = self.enter_env_name.clone();
        if env_name.is_empty() {
            return;
        }
        let auto_snap = self.auto_snapshot_snippet_for(&env_name);
        let session_script = self.session_bridge_script_for_env(&env_name);
        let script = build_prepare_script_with_desktop(
            &env_name,
            &auto_snap,
            &self.enter_allowlist_snippet,
            &session_script,
        );
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let output = Command::new("/bin/bash").arg("-c").arg(&script).output();
            let _ = qt_thread.queue(move |mut backend| {
                if backend.enter_env_name != env_name {
                    return;
                }
                let Ok(out) = output else {
                    backend
                        .as_mut()
                        .finish_enter("Could not start environment preparation.");
                    return;
                };
                let mut combined = out.stdout;
                combined.extend_from_slice(&out.stderr);
                if combined.windows(8).any(|w| w == b"Warning:") {
                    let state_dir = backend.engine.paths.state_dir.clone();
                    let secret_values = backend.engine.secrets.all_secret_values();
                    let exit_code = out.status.code().unwrap_or(1);
                    let now_iso = now_utc_iso();
                    if let Some(entries) = record_operation_history(&RecordOperationParams {
                        state_dir: &state_dir,
                        description: &format!("Auto-snapshot warning for '{env_name}'"),
                        env_name: &env_name,
                        status: "warning",
                        exit_code,
                        started_at_iso: &now_iso,
                        log_bytes: &combined,
                        secret_values: &secret_values,
                        reason: "",
                        max_entries: 50,
                    }) {
                        let q_hist = json_array_to_qvariant_list(&entries);
                        {
                            let mut rust = backend.as_mut().rust_mut();
                            rust.engine.set_history_cache(entries);
                            rust.operation_history = q_hist;
                        }
                        backend.as_mut().operation_history_changed();
                    }
                }
                backend.as_mut().push_enter_secrets();
            });
        });
    }

    fn push_enter_secrets(mut self: Pin<&mut Self>) {
        let env_name = self.enter_env_name.clone();
        if env_name.is_empty() {
            return;
        }
        let assigned_globals = self
            .engine
            .environments
            .iter()
            .find(|e| e.name == env_name)
            .map(|e| e.secret_names.clone())
            .unwrap_or_default();
        let pairs = match self.engine.secrets.resolve_environment_secrets(
            &env_name,
            &assigned_globals,
            self.demo_mode,
        ) {
            Ok(p) => p,
            Err(err) => {
                self.as_mut().finish_enter(&err.to_string());
                return;
            }
        };
        if pairs.is_empty() {
            self.as_mut().launch_enter_terminal("");
            return;
        }

        let mut content = build_secret_env_file_content(&pairs);
        let file_res = create_host_temp_secret_file(&content);
        aibox_core::secrets::zeroize_string(&mut content);
        let host_secret_file = match file_res {
            Ok(path) => path,
            Err(err) => {
                self.as_mut().finish_enter(&err.to_string());
                return;
            }
        };
        let seq = REMOTE_ENV_SEQ.fetch_add(1, Ordering::Relaxed);
        let remote_env_path = format!(
            "/dev/shm/aibox-{}-{}-{seq}.env",
            std::process::id(),
            now_epoch_secs()
        );
        let dest = format!("{env_name}{remote_env_path}");

        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let output = Command::new("incus")
                .args(["file", "push", "--mode", "0400", &host_secret_file, &dest])
                .output();
            let _ = std::fs::remove_file(&host_secret_file);

            let _ = qt_thread.queue(move |mut backend| {
                if backend.enter_env_name != env_name {
                    return;
                }
                match output {
                    Ok(out) if out.status.success() => {
                        backend.as_mut().launch_enter_terminal(&remote_env_path);
                    }
                    Ok(out) => {
                        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                        let msg = if err.is_empty() {
                            "Failed to inject secrets into container.".to_string()
                        } else {
                            err
                        };
                        backend.as_mut().finish_enter(&msg);
                    }
                    Err(_) => {
                        backend
                            .as_mut()
                            .finish_enter("Failed to inject secrets into container.");
                    }
                }
            });
        });
    }

    fn launch_enter_terminal(mut self: Pin<&mut Self>, remote_env_path: &str) {
        let name = self.enter_env_name.clone();
        if name.is_empty() {
            return;
        }
        let mut enter_cmd =
            build_enter_command_with_env(&name, remote_env_path, &self.session_env_for_env(&name));
        let record_enabled = self
            .engine
            .environments
            .iter()
            .find(|e| e.name == name)
            .map(|e| e.record_enabled)
            .unwrap_or(false);
        if record_enabled {
            let state_dir = self.engine.paths.state_dir.clone();
            let sessions_dir = sessions_dir_path(&state_dir);
            if let Some((log_file, timing_file)) = prepare_recorded_session_paths(&state_dir, &name)
            {
                let has_script = find_executable("script").is_some();
                let app_path = std::env::current_exe()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                enter_cmd = build_session_wrap_command(
                    &enter_cmd,
                    &log_file,
                    &timing_file,
                    &app_path,
                    &sessions_dir,
                    has_script,
                );
            }
        }

        {
            let mut rust = self.as_mut().rust_mut();
            rust.enter_env_name.clear();
            rust.enter_allowlist_snippet.clear();
        }
        self.as_mut().set_status_message("");
        launch_command_in_terminal(&enter_cmd);
        self.as_mut().refresh();
    }

    fn finish_enter(mut self: Pin<&mut Self>, error_message: &str) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.enter_env_name.clear();
            rust.enter_allowlist_snippet.clear();
        }
        self.as_mut().set_status_message("");
        if !error_message.is_empty() {
            self.as_mut().set_error_message(error_message);
        }
        self.as_mut().refresh();
    }

    pub(crate) fn evaluate_all_watchdogs(mut self: Pin<&mut Self>) {
        if !*self.watchdog_enabled() || self.demo_mode {
            return;
        }
        let inputs: Vec<WatchdogEnvInput> = self
            .engine
            .environments
            .iter()
            .map(|env| WatchdogEnvInput {
                name: env.name.clone(),
                running: env.running,
                disk_percent: env.disk_percent,
                disk_usage: env.disk_usage.clone(),
                disk_limit: env.disk_limit.clone(),
                disk_alert_level: env.disk_alert_level,
                auto_stop_on_critical: env.auto_stop_on_critical,
                cpu_percent: env.cpu_percent,
                memory_percent: env.memory_percent,
                prev_disk_level: *self.watchdog_disk_level.get(&env.name).unwrap_or(&0),
                prev_cpu_level: *self.watchdog_cpu_level.get(&env.name).unwrap_or(&0),
                prev_cpu_streak: *self.watchdog_cpu_high_streak.get(&env.name).unwrap_or(&0),
                prev_ram_level: *self.watchdog_ram_level.get(&env.name).unwrap_or(&0),
                prev_ram_streak: *self.watchdog_ram_high_streak.get(&env.name).unwrap_or(&0),
            })
            .collect();

        let disk_warn = *self.watchdog_disk_warn();
        let disk_critical = *self.watchdog_disk_critical();
        let busy = *self.busy();
        let batch = evaluate_all_watchdogs(&inputs, disk_warn, disk_critical, 95, 92, busy);

        let live_names: std::collections::HashSet<&str> =
            inputs.iter().map(|i| i.name.as_str()).collect();
        let mut alert_changed = false;
        {
            let mut rust = self.as_mut().rust_mut();
            let prune = |map: &mut std::collections::HashMap<String, i32>| {
                map.retain(|k, _| live_names.contains(k.as_str()));
            };
            prune(&mut rust.watchdog_disk_level);
            prune(&mut rust.watchdog_cpu_level);
            prune(&mut rust.watchdog_cpu_high_streak);
            prune(&mut rust.watchdog_ram_level);
            prune(&mut rust.watchdog_ram_high_streak);

            for (idx, er) in batch.envs.iter().enumerate() {
                rust.watchdog_disk_level
                    .insert(er.name.clone(), er.new_disk_level);
                rust.watchdog_cpu_level
                    .insert(er.name.clone(), er.new_cpu_level);
                rust.watchdog_cpu_high_streak
                    .insert(er.name.clone(), er.new_cpu_streak);
                rust.watchdog_ram_level
                    .insert(er.name.clone(), er.new_ram_level);
                rust.watchdog_ram_high_streak
                    .insert(er.name.clone(), er.new_ram_streak);
                if er.disk_alert_level_changed {
                    if let Some(env) = rust.engine.environments.get_mut(idx) {
                        env.disk_alert_level = er.new_disk_level;
                        alert_changed = true;
                    }
                }
            }
        }
        if alert_changed {
            self.as_mut().sync_environments_from_engine();
        }

        for n in &batch.notifications {
            match n.kind {
                WatchdogNotifyKind::DiskWarning | WatchdogNotifyKind::DiskCritical => {
                    let mut msg = if n.kind == WatchdogNotifyKind::DiskCritical {
                        format!(
                            "Disk critical ({}%) in '{}' ({} of {}).",
                            n.percent, n.env_name, n.disk_usage, n.disk_limit
                        )
                    } else {
                        format!(
                            "Disk warning ({}%) in '{}' ({} of {}).",
                            n.percent, n.env_name, n.disk_usage, n.disk_limit
                        )
                    };
                    if n.auto_stopping {
                        msg.push_str(" Auto-stopping container to protect host.");
                    }
                    send_desktop_notification(n.critical, "AIBox Watchdog", &msg);
                }
                WatchdogNotifyKind::CpuHigh => {
                    send_desktop_notification(
                        false,
                        "AIBox Watchdog",
                        &format!("Sustained high CPU ({}%) in '{}'.", n.percent, n.env_name),
                    );
                }
                WatchdogNotifyKind::RamHigh => {
                    send_desktop_notification(
                        false,
                        "AIBox Watchdog",
                        &format!("High memory pressure ({}%) in '{}'.", n.percent, n.env_name),
                    );
                }
            }
        }

        if !batch.auto_stop_target.is_empty() {
            let desc = format!(
                "Watchdog auto-stopping '{}' (disk >= {}%)",
                batch.auto_stop_target, disk_critical
            );
            self.as_mut().run_shell_task(
                &desc,
                &batch.auto_stop_script,
                "",
                &batch.auto_stop_target,
            );
        }
    }

    pub(crate) fn trigger_idle_auto_stops_check(mut self: Pin<&mut Self>) {
        if self.demo_mode {
            return;
        }
        let global_minutes = *self.idle_stop_minutes();
        let any_tracking = self.engine.environments.iter().any(|env| {
            let eff = effective_idle_stop_minutes(
                global_minutes,
                &parse_idle_stop_override(&env.idle_stop_override),
            );
            env.running && !env.is_template && eff > 0
        });
        if !any_tracking {
            let mut rust = self.as_mut().rust_mut();
            rust.idle_samples.clear();
            rust.idle_warned.clear();
            return;
        }

        if self
            .idle_check_in_flight
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        let in_flight = Arc::clone(&self.idle_check_in_flight);
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let user = std::env::var("USER").unwrap_or_default();
            let user = user.trim().to_string();
            let mut cmd = Command::new("pgrep");
            if !user.is_empty() {
                cmd.args(["-u", &user]);
            }
            cmd.args(["-af", "incus exec"]);
            let active_exec_envs = match cmd.output() {
                Ok(out) if out.status.success() => {
                    parse_active_incus_exec_envs(&String::from_utf8_lossy(&out.stdout))
                }
                _ => BTreeSet::new(),
            };
            let in_flight_cb = Arc::clone(&in_flight);
            if qt_thread
                .queue(move |mut backend| {
                    in_flight_cb.store(false, Ordering::SeqCst);
                    backend.as_mut().apply_idle_auto_stops(active_exec_envs);
                })
                .is_err()
            {
                in_flight.store(false, Ordering::SeqCst);
            }
        });
    }

    fn apply_idle_auto_stops(mut self: Pin<&mut Self>, active_exec_envs: BTreeSet<String>) {
        if self.demo_mode {
            return;
        }
        let now_ms = now_epoch_millis();
        let cutoff_ms = now_ms - 25 * 60 * 60 * 1000;
        let busy = *self.busy();
        let active_task_env = if busy {
            read_task_meta(Path::new(&self.engine.paths.state_dir))
                .map(|m| m.target_env_name)
                .unwrap_or_default()
        } else {
            String::new()
        };

        let global_minutes = *self.idle_stop_minutes();
        let pending_env_name = self.pending_env_name().to_string();
        let envs: Vec<_> = self
            .engine
            .environments
            .iter()
            .map(|e| {
                (
                    e.name.clone(),
                    e.running,
                    e.is_template,
                    e.port_forward_count as i32,
                    e.idle_stop_ignore_ports,
                    e.idle_stop_override.clone(),
                    e.cpu_percent,
                )
            })
            .collect();

        let mut idle_stop_target = String::new();
        {
            let mut rust = self.as_mut().rust_mut();
            for (name, running, is_template, port_count, ignore_ports, override_raw, cpu_percent) in
                envs
            {
                let threshold_min = effective_idle_stop_minutes(
                    global_minutes,
                    &parse_idle_stop_override(&override_raw),
                );
                let busy_on_this_env =
                    busy && (active_task_env == name || pending_env_name == name);
                let has_exec = active_exec_envs.contains(&name);

                if !running
                    || is_template
                    || threshold_min <= 0
                    || busy_on_this_env
                    || has_exec
                    || (port_count > 0 && !ignore_ports)
                {
                    rust.idle_samples.remove(&name);
                    rust.idle_warned.remove(&name);
                    continue;
                }

                let samples = rust.idle_samples.entry(name.clone()).or_default();
                samples.push(IdleCpuSample {
                    timestamp_ms: now_ms,
                    cpu_percent,
                });
                samples.retain(|s| s.timestamp_ms >= cutoff_ms);
                let samples_snapshot = samples.clone();
                let warning_already_sent = rust.idle_warned.contains(&name);

                let res = evaluate_idle_decision(&IdleDecisionInput {
                    running,
                    is_template,
                    busy_on_this_env,
                    has_active_exec_session: has_exec,
                    port_forward_count: port_count,
                    ignore_port_forwards: ignore_ports,
                    threshold_minutes: threshold_min,
                    now_ms,
                    warning_already_sent,
                    samples: samples_snapshot,
                });
                if res.should_warn_one_min {
                    rust.idle_warned.insert(name.clone());
                    send_desktop_notification(
                        false,
                        "AIBox Idle Auto-Stop",
                        &format!("'{name}' has been idle and will stop in 1 minute."),
                    );
                }
                if res.should_stop_now && idle_stop_target.is_empty() && !busy {
                    idle_stop_target = name;
                }
            }

            if !idle_stop_target.is_empty() {
                rust.idle_samples.remove(&idle_stop_target);
                rust.idle_warned.remove(&idle_stop_target);
            }
        }

        if !idle_stop_target.is_empty() {
            let desc = format!("Idle auto-stopping '{idle_stop_target}' (reason: idle)");
            let auto_snap = self.auto_snapshot_snippet_for(&idle_stop_target);
            let dbus_stop = self.dbus_stop_script_for_env(&idle_stop_target);
            let script = stop_environment_script_with_session(
                &idle_stop_target,
                true,
                &auto_snap,
                &dbus_stop,
            );
            self.as_mut()
                .run_shell_task_with_reason(&desc, &script, "", &idle_stop_target, "idle");
        }
    }
}
