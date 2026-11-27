use std::path::Path;
use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

use aibox_core::incus::list::environment_by_name;
use aibox_core::network::acl::build_allowlist_refresh_if_needed;
use aibox_core::network::allowlist::{
    extract_allowlist_hostnames, parse_allowlist_entries, SystemResolver,
};
use aibox_core::provision::clone::prepare_clone_environment;
use aibox_core::provision::create::prepare_create_environment;
use aibox_core::provision::lifecycle::{
    ai_exec_command_text, prepare_toggle_start_stop_with_session_and_stop,
    set_environment_template_script,
};
use aibox_core::provision::resources::{
    download_os_image_script, prepare_prefetch_os_image, remove_os_image_script,
    validate_and_prepare_update_resources,
};
use aibox_core::snapshot::auto::decide_auto_snapshot;
use aibox_core::task::lock::is_task_lock_active;
use aibox_core::util::paths::{cache_base_path, CONTAINER_WORKSPACE_PATH};
use aibox_core::util::time::{now_compact_stamp, now_epoch_secs};

use crate::qt_backend::{find_executable, qobject};
use crate::qt_backend_task::NoopResolver;

impl qobject::Backend {
    #[allow(clippy::too_many_arguments)]
    pub fn create_environment(
        mut self: Pin<&mut Self>,
        name: &QString,
        os_id: &QString,
        preset: &QString,
        disk_gib: &QString,
        ram_gib: &QString,
        cpu_cores: i32,
        process_limit: &QString,
        io_limit: &QString,
        swap_limit: &QString,
        workspace_parent: &QString,
    ) {
        let existing_names: Vec<String> = self
            .engine
            .environments
            .iter()
            .map(|e| e.name.clone())
            .collect();
        let plan = match prepare_create_environment(
            *self.busy(),
            &name.to_string(),
            &existing_names,
            &os_id.to_string(),
            &self.engine.config.os_catalog,
            &preset.to_string(),
            &self.engine.config.default_preset_id,
            &self.engine.config.presets,
            &disk_gib.to_string(),
            &ram_gib.to_string(),
            cpu_cores,
            &process_limit.to_string(),
            &io_limit.to_string(),
            &swap_limit.to_string(),
            &workspace_parent.to_string(),
            &self.engine.config.workspace_base,
            &self.engine.config.os_storage_path,
        ) {
            Ok(p) => p,
            Err(err) => {
                self.as_mut().set_error_message(&err.to_string());
                return;
            }
        };

        self.as_mut()
            .run_shell_task(&plan.title, &plan.script, &plan.name, &plan.name);
    }

    pub fn clone_environment_simple(
        self: Pin<&mut Self>,
        source_name: &QString,
        new_name: &QString,
    ) {
        let empty = QString::default();
        self.clone_environment(
            source_name,
            new_name,
            &empty,
            &empty,
            0,
            false,
            &empty,
            &empty,
            &empty,
            &empty,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn clone_environment(
        mut self: Pin<&mut Self>,
        source_name: &QString,
        new_name: &QString,
        disk_gib: &QString,
        ram_gib: &QString,
        cpu_cores: i32,
        start_after: bool,
        process_limit: &QString,
        io_limit: &QString,
        swap_limit: &QString,
        workspace_parent: &QString,
    ) {
        let busy_now = *self.busy();
        let src_s = source_name.to_string();
        let src_env = environment_by_name(&self.engine.environments, &src_s);
        let src_exists = src_env.is_some();
        let src_net_mode = src_env
            .as_ref()
            .map(|e| e.network_mode.clone())
            .unwrap_or_default();
        let src_net_allowlist = src_env
            .as_ref()
            .map(|e| e.network_allowlist.clone())
            .unwrap_or_default();
        let src_env_type = src_env
            .as_ref()
            .map(|e| e.env_type.clone())
            .unwrap_or_default();
        let src_ws_path = src_env
            .as_ref()
            .map(|e| e.workspace_path.clone())
            .unwrap_or_default();
        let existing_names: Vec<String> = self
            .engine
            .environments
            .iter()
            .map(|e| e.name.clone())
            .collect();
        let new_name_s = new_name.to_string();
        let disk_s = disk_gib.to_string();
        let ram_s = ram_gib.to_string();
        let process_s = process_limit.to_string();
        let io_s = io_limit.to_string();
        let swap_s = swap_limit.to_string();
        let ws_parent_s = workspace_parent.to_string();
        let ws_base = self.engine.config.workspace_base.clone();

        let needs_dns = !busy_now
            && !self.demo_mode
            && src_exists
            && src_net_mode.trim().eq_ignore_ascii_case("allowlist")
            && !extract_allowlist_hostnames(&parse_allowlist_entries(&src_net_allowlist))
                .is_empty();

        if !needs_dns {
            let plan = match prepare_clone_environment(
                busy_now,
                &src_s,
                src_exists,
                &src_net_mode,
                &src_net_allowlist,
                &src_env_type,
                &src_ws_path,
                &new_name_s,
                &existing_names,
                &disk_s,
                &ram_s,
                cpu_cores,
                start_after,
                &process_s,
                &io_s,
                &swap_s,
                &ws_parent_s,
                &ws_base,
                &NoopResolver,
            ) {
                Ok(p) => p,
                Err(err) => {
                    self.as_mut().set_error_message(&err.to_string());
                    return;
                }
            };

            self.as_mut()
                .run_shell_task(&plan.title, &plan.script, &plan.new_name, &plan.new_name);
            return;
        }

        self.as_mut()
            .set_busy(true, "Resolving network allowlist...");
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let resolver = SystemResolver;
            let res = prepare_clone_environment(
                false,
                &src_s,
                src_exists,
                &src_net_mode,
                &src_net_allowlist,
                &src_env_type,
                &src_ws_path,
                &new_name_s,
                &existing_names,
                &disk_s,
                &ram_s,
                cpu_cores,
                start_after,
                &process_s,
                &io_s,
                &swap_s,
                &ws_parent_s,
                &ws_base,
                &resolver,
            );
            let _ = qt_thread.queue(move |mut backend| {
                backend.as_mut().set_busy(false, "");
                match res {
                    Ok(plan) => {
                        backend.as_mut().run_shell_task(
                            &plan.title,
                            &plan.script,
                            &plan.new_name,
                            &plan.new_name,
                        );
                    }
                    Err(err) => {
                        backend.as_mut().set_error_message(&err.to_string());
                    }
                }
            });
        });
    }

    pub fn toggle_start_stop(mut self: Pin<&mut Self>, name: &QString) {
        let name_s = name.to_string();
        if name_s.is_empty() || *self.busy() {
            return;
        }
        let Some(env) = environment_by_name(&self.engine.environments, &name_s) else {
            return;
        };
        if env.is_template {
            self.as_mut().set_error_message(
                "Cannot start or stop a template. Clone it to a new environment first.",
            );
            return;
        }

        let auto_snap = if env.running {
            String::new()
        } else {
            let ws_path = if env.workspace_path.is_empty() {
                self.workspace_path_for(&name_s)
            } else {
                env.workspace_path.clone()
            };
            decide_auto_snapshot(
                &name_s,
                env.is_template,
                env.auto_snapshot_enabled,
                &env.snapshots,
                &ws_path,
                env.workspace_snapshot_enabled,
                self.engine.config.auto_snapshot_keep,
                None,
                now_epoch_secs(),
                &now_compact_stamp(),
            )
            .unwrap_or_default()
        };
        let session_script = if env.running {
            String::new()
        } else {
            self.session_bridge_script_for_env(&name_s)
        };
        let dbus_stop = if env.running {
            self.dbus_stop_script_for_env(&name_s)
        } else {
            String::new()
        };

        let needs_dns = !self.demo_mode
            && !env.is_template
            && !env.running
            && env.network_mode.trim().eq_ignore_ascii_case("allowlist")
            && !extract_allowlist_hostnames(&parse_allowlist_entries(&env.network_allowlist))
                .is_empty();

        if !needs_dns {
            let allow_refresh = if !env.is_template && !env.running {
                build_allowlist_refresh_if_needed(
                    &name_s,
                    &env.network_mode,
                    &env.network_allowlist,
                    env.egress_recording,
                    &NoopResolver,
                )
            } else {
                String::new()
            };

            let plan = match prepare_toggle_start_stop_with_session_and_stop(
                &name_s,
                env.is_template,
                env.running,
                &auto_snap,
                &allow_refresh,
                &session_script,
                &dbus_stop,
            ) {
                Ok(p) => p,
                Err(err) => {
                    self.as_mut().set_error_message(&err.to_string());
                    return;
                }
            };
            self.as_mut()
                .run_shell_task(&plan.title, &plan.script, "", &name_s);
            return;
        }

        let net_mode = env.network_mode.clone();
        let net_allowlist = env.network_allowlist.clone();
        let egress_recording = env.egress_recording;
        let is_template = env.is_template;
        let running = env.running;
        self.as_mut()
            .set_busy(true, "Resolving network allowlist...");
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let resolver = SystemResolver;
            let allow_refresh = build_allowlist_refresh_if_needed(
                &name_s,
                &net_mode,
                &net_allowlist,
                egress_recording,
                &resolver,
            );
            let res = prepare_toggle_start_stop_with_session_and_stop(
                &name_s,
                is_template,
                running,
                &auto_snap,
                &allow_refresh,
                &session_script,
                &dbus_stop,
            );
            let _ = qt_thread.queue(move |mut backend| {
                backend.as_mut().set_busy(false, "");
                match res {
                    Ok(plan) => {
                        backend
                            .as_mut()
                            .run_shell_task(&plan.title, &plan.script, "", &name_s);
                    }
                    Err(err) => {
                        backend.as_mut().set_error_message(&err.to_string());
                    }
                }
            });
        });
    }

    pub fn set_environment_template(mut self: Pin<&mut Self>, name: &QString, is_template: bool) {
        let name_s = name.to_string();
        if name_s.is_empty() || *self.busy() {
            return;
        }
        let is_running =
            environment_by_name(&self.engine.environments, &name_s).is_some_and(|e| e.running);
        let plan = set_environment_template_script(&name_s, is_running, is_template);
        let script = if is_template && is_running {
            aibox_core::display::combine_scripts(&[
                self.dbus_stop_script_for_env(&name_s),
                plan.script,
            ])
        } else {
            plan.script
        };
        self.as_mut()
            .run_shell_task(&plan.title, &script, "", &name_s);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_environment_resources(
        mut self: Pin<&mut Self>,
        name: &QString,
        cpu_cores: i32,
        ram_gib: &QString,
        disk_gib: &QString,
        process_limit: &QString,
        io_limit: &QString,
        swap_limit: &QString,
    ) -> QString {
        let name_s = name.to_string();
        let env = environment_by_name(&self.engine.environments, &name_s);
        let plan = match validate_and_prepare_update_resources(
            &name_s,
            *self.busy(),
            cpu_cores,
            &ram_gib.to_string(),
            &disk_gib.to_string(),
            &process_limit.to_string(),
            &io_limit.to_string(),
            &swap_limit.to_string(),
            env.is_some(),
            env.as_ref().map_or(0, |e| e.disk_usage_bytes),
            env.as_ref().map(|e| e.disk_usage.as_str()).unwrap_or(""),
            env.as_ref()
                .is_some_and(|e| aibox_core::provision::is_vm_env_type(&e.env_type)),
        ) {
            Ok(p) => p,
            Err(err) => return QString::from(err.to_string().as_str()),
        };

        self.as_mut()
            .run_shell_task(&plan.title, &plan.script, "", &name_s);
        QString::default()
    }

    pub fn download_os_image(mut self: Pin<&mut Self>, os_id: &QString) {
        let os_s = os_id.to_string();
        if os_s.is_empty() || *self.busy() {
            return;
        }
        if is_task_lock_active(Path::new(&self.engine.paths.state_dir), false).0 {
            self.as_mut()
                .set_error_message("Another AIBox task is currently running.");
            return;
        }
        let Some(entry) = self.engine.config.os_entry_by_id(&os_s).cloned() else {
            self.as_mut()
                .set_error_message(&format!("Unknown OS catalog entry: {os_s}"));
            return;
        };
        let cache_base = cache_base_path(&self.engine.config.os_storage_path, &entry.id);
        let plan = download_os_image_script(&entry.id, &entry.alias, &cache_base);
        {
            let mut rust = self.as_mut().rust_mut();
            rust.downloading_os_id = QString::from(entry.id.as_str());
        }
        self.as_mut().rebuild_os_catalog();
        self.as_mut()
            .run_shell_task(&plan.title, &plan.script, "", "");
    }

    pub fn remove_os_image(mut self: Pin<&mut Self>, os_id: &QString) {
        let os_s = os_id.to_string();
        if os_s.is_empty() || *self.busy() {
            return;
        }
        let cache_base = cache_base_path(&self.engine.config.os_storage_path, &os_s);
        let plan = remove_os_image_script(&os_s, &cache_base);
        self.as_mut()
            .run_shell_task(&plan.title, &plan.script, "", "");
    }

    /// Starts a non-blocking background pre-fetch for an OS image when selected in the catalog.
    pub fn prefetch_os_image(self: Pin<&mut Self>, os_id: &QString) {
        if self.demo_mode || find_executable("incus").is_none() {
            return;
        }
        let os_s = os_id.to_string();
        let cache_base = cache_base_path(&self.engine.config.os_storage_path, &os_s);
        let already_cached = std::path::Path::new(&cache_base).exists()
            && std::path::Path::new(&format!("{cache_base}.root")).exists();
        let Ok(Some(plan)) = prepare_prefetch_os_image(
            &os_s,
            &self.engine.config.os_catalog,
            &self.engine.config.os_storage_path,
            already_cached,
        ) else {
            return;
        };
        let in_flight = std::sync::Arc::clone(&self.prefetch_in_flight);
        if let Ok(mut guard) = in_flight.lock() {
            if !guard.insert(os_s.clone()) {
                return;
            }
        }
        std::thread::spawn(move || {
            let _ = std::process::Command::new("/bin/bash")
                .args(["-c", &plan.script])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            if let Ok(mut guard) = in_flight.lock() {
                guard.remove(&os_s);
            }
        });
    }

    pub fn copy_ai_command(self: Pin<&mut Self>, name: &QString) {
        let name_s = name.to_string();
        if name_s.is_empty() {
            return;
        }
        let env = environment_by_name(&self.engine.environments, &name_s);
        let has_secrets = env.as_ref().is_some_and(|e| !e.secret_names.is_empty());
        let ws_mount = env
            .as_ref()
            .map(|e| e.workspace_mount_path.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(CONTAINER_WORKSPACE_PATH);
        let cmd = ai_exec_command_text(&name_s, has_secrets, ws_mount);
        self.copy_text_with_status(&cmd, &format!("Copied command: {cmd}"));
    }
}
