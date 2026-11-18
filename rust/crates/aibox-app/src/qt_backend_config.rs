use std::path::Path;
use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use aibox_core::config::model;
use aibox_core::config::settings::{
    complete_first_run, remove_os_entry, reset_create_options, reset_os_catalog,
    save_appearance_settings, save_auto_snapshot_settings, save_create_options, save_diff_excludes,
    save_idle_stop_settings, save_os_entry, save_preset, save_session_recording_settings,
    save_settings, save_stale_settings, save_watchdog_settings, FirstRunRequest,
};
use aibox_core::util::paths::{default_location_file_path, save_location_file};

use crate::qt_backend::{find_executable, qobject};
use crate::variant::json_array_to_qvariant_list;

const BUSY_CONFIG_MSG: &str =
    "Wait for the current operation to finish before changing the configuration.";

impl qobject::Backend {
    pub fn complete_first_run(mut self: Pin<&mut Self>, system_path: &QString) -> QString {
        let sys_arg = system_path.to_string();
        let res = {
            let req = FirstRunRequest {
                system_path: &sys_arg,
                current_system_path: &self.engine.paths.system_path,
                current_workspace_base: &self.engine.config.workspace_base,
                current_os_storage_path: &self.engine.config.os_storage_path,
                current_state_dir: &self.engine.paths.state_dir,
                current_config_path: &self.engine.paths.config_file_path,
                is_project_dir_mode: self.project_mode,
                remember_system_location: self.remember_system_location,
            };
            complete_first_run(&req)
        };
        let paths = match res {
            Ok(p) => p,
            Err(e) => return QString::from(e.to_string().as_str()),
        };

        let sys_changed = paths.system_path != self.engine.paths.system_path;
        let ws_changed = paths.workspace_base != self.engine.config.workspace_base;
        let os_changed = paths.os_storage_path != self.engine.config.os_storage_path;
        let cfg_changed = paths.config_path != self.engine.paths.config_file_path;

        {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.paths.system_path = paths.system_path.clone();
            rust.engine.paths.workspace_base = paths.workspace_base.clone();
            rust.engine.paths.os_storage_path = paths.os_storage_path.clone();
            rust.engine.paths.state_dir = paths.state_dir.clone();
            rust.engine.paths.config_file_path = paths.config_path.clone();
            rust.default_workspace_base = QString::from(paths.workspace_base.as_str());
            rust.default_os_storage_path = QString::from(paths.os_storage_path.as_str());
            if cfg_changed && Path::new(&paths.config_path).exists() {
                rust.engine.config = model::load(
                    &paths.config_path,
                    &paths.workspace_base,
                    &paths.os_storage_path,
                );
            } else {
                rust.engine.config.workspace_base = paths.workspace_base.clone();
                rust.engine.config.os_storage_path = paths.os_storage_path.clone();
            }
            rust.engine.config.first_run_completed = true;
        }

        if self.save_config().is_err() {
            return QString::from(
                format!(
                    "Could not save the configuration file: {}",
                    self.engine.paths.config_file_path
                )
                .as_str(),
            );
        }

        if self.remember_system_location
            && save_location_file(&self.engine.paths.system_path).is_err()
        {
            return QString::from(
                format!(
                    "Could not save the system location preference: {}",
                    default_location_file_path()
                )
                .as_str(),
            );
        }

        self.as_mut().sync_config_properties();
        self.as_mut().refresh_backups();
        self.as_mut().rebuild_os_catalog();

        {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.reload_history();
            let seen_id = rust.engine.history_seen_id().to_string();
            let q_hist = json_array_to_qvariant_list(rust.engine.cached_operation_history());
            rust.history_seen_id = seen_id;
            rust.operation_history = q_hist;
        }
        self.as_mut().operation_history_changed();

        if !self.demo_mode {
            self.as_mut().refresh_recorded_sessions();
            if find_executable("incus").is_some() {
                self.as_mut().start_background_workers();
                self.as_mut().refresh();
            }
        }

        self.as_mut().first_run_changed();
        if sys_changed {
            self.as_mut().system_path_changed();
        }
        if ws_changed {
            self.as_mut().workspace_base_changed();
        }
        if os_changed {
            self.as_mut().os_storage_path_changed();
        }
        if cfg_changed {
            self.as_mut().config_file_path_changed();
        }
        QString::default()
    }

    pub fn save_settings(
        mut self: Pin<&mut Self>,
        workspace_path: &QString,
        os_storage_path: &QString,
    ) -> QString {
        if *self.busy() {
            return QString::from(BUSY_CONFIG_MSG);
        }
        let ws_arg = workspace_path.to_string();
        let os_arg = os_storage_path.to_string();
        let default_ws = self.default_workspace_base().to_string();
        let default_os = self.default_os_storage_path().to_string();
        let next_cfg = match save_settings(
            &self.engine.config,
            &ws_arg,
            &os_arg,
            &default_ws,
            &default_os,
        ) {
            Ok(c) => c,
            Err(e) => return QString::from(e.to_string().as_str()),
        };

        let prev_ws = self.engine.config.workspace_base.clone();
        let prev_os = self.engine.config.os_storage_path.clone();
        let was_first_run = *self.first_run();

        {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.config = next_cfg;
        }
        if self.save_config().is_err() {
            return QString::from(
                format!(
                    "Could not save the configuration file: {}",
                    self.engine.paths.config_file_path
                )
                .as_str(),
            );
        }

        let ws_changed = self.engine.config.workspace_base != prev_ws;
        let os_changed = self.engine.config.os_storage_path != prev_os;
        self.as_mut().sync_config_properties();
        if ws_changed {
            self.as_mut().workspace_base_changed();
        }
        if os_changed {
            self.as_mut().os_storage_path_changed();
            self.as_mut().rebuild_os_catalog();
        }
        if was_first_run {
            self.as_mut().first_run_changed();
        }
        self.as_mut().set_status_message("Configuration saved.");
        QString::default()
    }

    pub fn save_os_entry(
        mut self: Pin<&mut Self>,
        original_id: &QString,
        id: &QString,
        alias: &QString,
        name: &QString,
        badge: &QString,
        description: &QString,
    ) -> QString {
        if *self.busy() {
            return QString::from(BUSY_CONFIG_MSG);
        }
        let orig_s = original_id.to_string();
        let id_s = id.to_string();
        let alias_s = alias.to_string();
        let name_s = name.to_string();
        let badge_s = badge.to_string();
        let desc_s = description.to_string();

        let next_cfg = match save_os_entry(
            &self.engine.config,
            &orig_s,
            &id_s,
            &alias_s,
            &name_s,
            &badge_s,
            &desc_s,
        ) {
            Ok(c) => c,
            Err(e) => return QString::from(e.to_string().as_str()),
        };

        {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.config = next_cfg;
        }
        if self.save_config().is_err() {
            return QString::from(
                format!(
                    "Could not save the configuration file: {}",
                    self.engine.paths.config_file_path
                )
                .as_str(),
            );
        }

        self.as_mut().rebuild_os_catalog();
        let norm_id = id_s.trim().to_lowercase();
        let saved_name = self
            .engine
            .config
            .os_entry_by_id(&norm_id)
            .map(|e| e.name.clone())
            .unwrap_or_default();
        self.as_mut()
            .set_status_message(&format!("OS entry '{saved_name}' saved."));
        QString::default()
    }

    pub fn remove_os_entry(mut self: Pin<&mut Self>, id: &QString) -> QString {
        if *self.busy() {
            return QString::from(BUSY_CONFIG_MSG);
        }
        let id_s = id.to_string();
        let next_cfg = match remove_os_entry(&self.engine.config, &id_s) {
            Ok(c) => c,
            Err(e) => return QString::from(e.to_string().as_str()),
        };
        {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.config = next_cfg;
        }
        if self.save_config().is_err() {
            return QString::from(
                format!(
                    "Could not save the configuration file: {}",
                    self.engine.paths.config_file_path
                )
                .as_str(),
            );
        }
        self.as_mut().rebuild_os_catalog();
        self.as_mut()
            .set_status_message("OS entry removed from the catalog.");
        QString::default()
    }

    pub fn reset_os_catalog(mut self: Pin<&mut Self>) -> QString {
        if *self.busy() {
            return QString::from(BUSY_CONFIG_MSG);
        }
        let next_cfg = reset_os_catalog(&self.engine.config);
        {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.config = next_cfg;
        }
        if self.save_config().is_err() {
            return QString::from(
                format!(
                    "Could not save the configuration file: {}",
                    self.engine.paths.config_file_path
                )
                .as_str(),
            );
        }
        self.as_mut().rebuild_os_catalog();
        QString::default()
    }

    pub fn save_create_options(
        mut self: Pin<&mut Self>,
        disk_csv: &QString,
        ram_csv: &QString,
        cpu_csv: &QString,
    ) -> QString {
        if *self.busy() {
            return QString::from(BUSY_CONFIG_MSG);
        }
        let next_cfg = match save_create_options(
            &self.engine.config,
            &disk_csv.to_string(),
            &ram_csv.to_string(),
            &cpu_csv.to_string(),
        ) {
            Ok(c) => c,
            Err(e) => return QString::from(e.to_string().as_str()),
        };
        {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.config = next_cfg;
        }
        if self.save_config().is_err() {
            return QString::from(
                format!(
                    "Could not save the configuration file: {}",
                    self.engine.paths.config_file_path
                )
                .as_str(),
            );
        }
        self.as_mut().sync_config_properties();
        self.as_mut().create_options_changed();
        QString::default()
    }

    pub fn save_preset(
        mut self: Pin<&mut Self>,
        id: &QString,
        title: &QString,
        subtitle: &QString,
    ) -> QString {
        if *self.busy() {
            return QString::from(BUSY_CONFIG_MSG);
        }
        let next_cfg = match save_preset(
            &self.engine.config,
            &id.to_string(),
            &title.to_string(),
            &subtitle.to_string(),
        ) {
            Ok(c) => c,
            Err(e) => return QString::from(e.to_string().as_str()),
        };
        {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.config = next_cfg;
        }
        if self.save_config().is_err() {
            return QString::from(
                format!(
                    "Could not save the configuration file: {}",
                    self.engine.paths.config_file_path
                )
                .as_str(),
            );
        }
        self.as_mut().sync_config_properties();
        self.as_mut().create_options_changed();
        QString::default()
    }

    pub fn reset_create_options(mut self: Pin<&mut Self>) -> QString {
        if *self.busy() {
            return QString::from(BUSY_CONFIG_MSG);
        }
        let next_cfg = reset_create_options(&self.engine.config);
        {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.config = next_cfg;
        }
        if self.save_config().is_err() {
            return QString::from(
                format!(
                    "Could not save the configuration file: {}",
                    self.engine.paths.config_file_path
                )
                .as_str(),
            );
        }
        self.as_mut().sync_config_properties();
        self.as_mut().create_options_changed();
        QString::default()
    }

    pub fn save_auto_snapshot_settings(
        mut self: Pin<&mut Self>,
        enabled: bool,
        keep_count: i32,
    ) -> QString {
        if *self.busy() {
            return QString::from(BUSY_CONFIG_MSG);
        }
        let next_cfg = match save_auto_snapshot_settings(&self.engine.config, enabled, keep_count) {
            Ok(c) => c,
            Err(e) => return QString::from(e.to_string().as_str()),
        };
        let mut env_updated = false;
        {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.config = next_cfg;
            for env in &mut rust.engine.environments {
                if env.auto_snapshot_override.is_empty() {
                    env.auto_snapshot_enabled = enabled;
                    env_updated = true;
                }
            }
        }
        if self.save_config().is_err() {
            return QString::from(
                format!(
                    "Could not save the configuration file: {}",
                    self.engine.paths.config_file_path
                )
                .as_str(),
            );
        }
        self.as_mut().sync_config_properties();
        self.as_mut().auto_snapshot_settings_changed();
        if env_updated {
            self.as_mut().sync_environments_from_engine();
        }
        QString::default()
    }

    pub fn save_watchdog_settings(
        mut self: Pin<&mut Self>,
        enabled: bool,
        disk_warn: i32,
        disk_critical: i32,
        auto_stop: bool,
    ) -> QString {
        if *self.busy() {
            return QString::from(BUSY_CONFIG_MSG);
        }
        let next_cfg = match save_watchdog_settings(
            &self.engine.config,
            enabled,
            disk_warn,
            disk_critical,
            auto_stop,
        ) {
            Ok(c) => c,
            Err(e) => return QString::from(e.to_string().as_str()),
        };
        let mut env_updated = false;
        {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.config = next_cfg;
            for env in &mut rust.engine.environments {
                if env.auto_stop_override.is_empty() {
                    env.auto_stop_on_critical = auto_stop;
                    env_updated = true;
                }
            }
        }
        if self.save_config().is_err() {
            return QString::from(
                format!(
                    "Could not save the configuration file: {}",
                    self.engine.paths.config_file_path
                )
                .as_str(),
            );
        }
        self.as_mut().sync_config_properties();
        self.as_mut().watchdog_settings_changed();
        if env_updated {
            self.as_mut().sync_environments_from_engine();
        }
        QString::default()
    }

    pub fn save_idle_stop_settings(mut self: Pin<&mut Self>, minutes: i32) -> QString {
        if *self.busy() {
            return QString::from(BUSY_CONFIG_MSG);
        }
        let next_cfg = match save_idle_stop_settings(&self.engine.config, minutes) {
            Ok(c) => c,
            Err(e) => return QString::from(e.to_string().as_str()),
        };
        {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.config = next_cfg;
        }
        if self.save_config().is_err() {
            return QString::from(
                format!(
                    "Could not save the configuration file: {}",
                    self.engine.paths.config_file_path
                )
                .as_str(),
            );
        }
        self.as_mut().sync_config_properties();
        self.as_mut().idle_stop_settings_changed();
        QString::default()
    }

    pub fn save_stale_settings(mut self: Pin<&mut Self>, days: i32) -> QString {
        if *self.busy() {
            return QString::from(BUSY_CONFIG_MSG);
        }
        let next_cfg = match save_stale_settings(&self.engine.config, days) {
            Ok(c) => c,
            Err(e) => return QString::from(e.to_string().as_str()),
        };
        {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.config = next_cfg;
        }
        if self.save_config().is_err() {
            return QString::from(
                format!(
                    "Could not save the configuration file: {}",
                    self.engine.paths.config_file_path
                )
                .as_str(),
            );
        }
        self.as_mut().sync_config_properties();
        self.as_mut().stale_report_settings_changed();
        if !self.stale_report().is_empty() {
            self.as_mut().refresh_stale_report();
        }
        QString::default()
    }

    pub fn save_session_recording_settings(mut self: Pin<&mut Self>, enabled: bool) -> QString {
        if *self.busy() {
            return QString::from(BUSY_CONFIG_MSG);
        }
        let next_cfg = save_session_recording_settings(&self.engine.config, enabled);
        let mut env_updated = false;
        {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.config = next_cfg;
            for env in &mut rust.engine.environments {
                if env.record_override.is_empty() || env.record_override == "inherit" {
                    env.record_enabled = enabled;
                    env_updated = true;
                }
            }
        }
        if self.save_config().is_err() {
            return QString::from(
                format!(
                    "Could not save the configuration file: {}",
                    self.engine.paths.config_file_path
                )
                .as_str(),
            );
        }
        self.as_mut().sync_config_properties();
        self.as_mut().record_sessions_settings_changed();
        if env_updated {
            self.as_mut().sync_environments_from_engine();
        }
        QString::default()
    }

    pub fn save_diff_excludes(mut self: Pin<&mut Self>, raw_excludes: &QString) -> QString {
        if *self.busy() {
            return QString::from(BUSY_CONFIG_MSG);
        }
        let next_cfg = save_diff_excludes(&self.engine.config, &raw_excludes.to_string());
        {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.config = next_cfg;
        }
        if self.save_config().is_err() {
            return QString::from(
                format!(
                    "Could not save the configuration file: {}",
                    self.engine.paths.config_file_path
                )
                .as_str(),
            );
        }
        self.as_mut().sync_config_properties();
        self.as_mut().diff_excludes_changed();
        QString::default()
    }

    pub fn save_appearance_settings(
        mut self: Pin<&mut Self>,
        mode_override: &QString,
        high_contrast: bool,
        reduce_motion: bool,
    ) -> QString {
        if *self.busy() {
            return QString::from(BUSY_CONFIG_MSG);
        }
        let next_cfg = match save_appearance_settings(
            &self.engine.config,
            &mode_override.to_string(),
            high_contrast,
            reduce_motion,
        ) {
            Ok(c) => c,
            Err(e) => return QString::from(e.to_string().as_str()),
        };
        {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.config = next_cfg;
        }
        if self.save_config().is_err() {
            return QString::from(
                format!(
                    "Could not save the configuration file: {}",
                    self.engine.paths.config_file_path
                )
                .as_str(),
            );
        }
        self.as_mut().sync_config_properties();
        self.as_mut().appearance_settings_changed();
        QString::default()
    }
}
