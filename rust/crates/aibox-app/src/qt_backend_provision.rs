use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use aibox_core::incus::list::environment_by_name;
use aibox_core::network::acl::build_allowlist_refresh_if_needed;
use aibox_core::network::allowlist::SystemResolver;
use aibox_core::provision::clone::prepare_clone_environment;
use aibox_core::provision::create::prepare_create_environment;
use aibox_core::provision::lifecycle::{
    ai_exec_command_text, prepare_toggle_start_stop, set_environment_template_script,
};
use aibox_core::provision::resources::{
    download_os_image_script, remove_os_image_script, validate_and_prepare_update_resources,
};
use aibox_core::snapshot::auto::decide_auto_snapshot;
use aibox_core::util::paths::{cache_base_path, CONTAINER_WORKSPACE_PATH};
use aibox_core::util::time::{now_compact_stamp, now_epoch_secs};

use crate::qt_backend::qobject;

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
            &workspace_parent.to_string(),
            &self.engine.config.workspace_base,
            &self.engine.config.os_storage_path,
        ) {
            Ok(p) => p,
            Err(err) => {
                self.as_mut().set_error_message(&err);
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
        workspace_parent: &QString,
    ) {
        let src_s = source_name.to_string();
        let src_env = environment_by_name(&self.engine.environments, &src_s);
        let existing_names: Vec<String> = self
            .engine
            .environments
            .iter()
            .map(|e| e.name.clone())
            .collect();
        let resolver = SystemResolver;
        let plan = match prepare_clone_environment(
            *self.busy(),
            &src_s,
            src_env.is_some(),
            src_env
                .as_ref()
                .map(|e| e.network_mode.as_str())
                .unwrap_or(""),
            src_env
                .as_ref()
                .map(|e| e.network_allowlist.as_str())
                .unwrap_or(""),
            src_env.as_ref().map(|e| e.env_type.as_str()).unwrap_or(""),
            &new_name.to_string(),
            &existing_names,
            &disk_gib.to_string(),
            &ram_gib.to_string(),
            cpu_cores,
            start_after,
            &process_limit.to_string(),
            &io_limit.to_string(),
            &workspace_parent.to_string(),
            &self.engine.config.workspace_base,
            &resolver,
        ) {
            Ok(p) => p,
            Err(err) => {
                self.as_mut().set_error_message(&err);
                return;
            }
        };

        self.as_mut()
            .run_shell_task(&plan.title, &plan.script, &plan.new_name, &plan.new_name);
    }

    pub fn toggle_start_stop(mut self: Pin<&mut Self>, name: &QString) {
        let name_s = name.to_string();
        if name_s.is_empty() || *self.busy() {
            return;
        }
        let Some(env) = environment_by_name(&self.engine.environments, &name_s) else {
            return;
        };

        let auto_snap = if env.is_template {
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

        let allow_refresh = if !env.is_template && !env.running {
            let resolver = SystemResolver;
            build_allowlist_refresh_if_needed(
                &name_s,
                &env.network_mode,
                &env.network_allowlist,
                env.egress_recording,
                &resolver,
            )
        } else {
            String::new()
        };

        let plan = match prepare_toggle_start_stop(
            &name_s,
            env.is_template,
            env.running,
            &auto_snap,
            &allow_refresh,
        ) {
            Ok(p) => p,
            Err(err) => {
                self.as_mut().set_error_message(&err);
                return;
            }
        };
        self.as_mut()
            .run_shell_task(&plan.title, &plan.script, "", &name_s);
    }

    pub fn set_environment_template(mut self: Pin<&mut Self>, name: &QString, is_template: bool) {
        let name_s = name.to_string();
        if name_s.is_empty() || *self.busy() {
            return;
        }
        let is_running =
            environment_by_name(&self.engine.environments, &name_s).is_some_and(|e| e.running);
        let plan = set_environment_template_script(&name_s, is_running, is_template);
        self.as_mut()
            .run_shell_task(&plan.title, &plan.script, "", &name_s);
    }

    pub fn update_environment_resources(
        mut self: Pin<&mut Self>,
        name: &QString,
        cpu_cores: i32,
        ram_gib: &QString,
        disk_gib: &QString,
        process_limit: &QString,
        io_limit: &QString,
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
            env.is_some(),
            env.as_ref().map_or(0, |e| e.disk_usage_bytes),
            env.as_ref().map(|e| e.disk_usage.as_str()).unwrap_or(""),
            env.as_ref().is_some_and(|e| e.env_type == "VM"),
        ) {
            Ok(p) => p,
            Err(err) => return QString::from(err.as_str()),
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
