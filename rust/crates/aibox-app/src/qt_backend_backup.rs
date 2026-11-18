use std::path::Path;
use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QMap, QMapPair_QString_QVariant, QString};

use aibox_core::backup::orphans::{
    delete_orphan_script, prepare_delete_orphan_as_admin, validate_orphan_request,
};
use aibox_core::backup::scripts::{
    export_and_wipe_environment_script_with_options, export_environment_script_with_options,
    import_environment_script_with_options,
};
use aibox_core::backup::{
    backups_dir_path, check_export_space, delete_backup_files, validate_export_request,
    validate_import_request, verify_backup_archive, CompressionCodec,
};
use aibox_core::incus::list::environment_by_name;
use aibox_core::secrets::store::environment_secrets_store_path;
use aibox_core::session::sessions_dir_path;
use aibox_core::util::names::sanitize_name;
use aibox_core::util::paths::clean_path;
use aibox_core::util::time::now_compact_stamp;
use aibox_core::workspace::file_access::stop_file_access_process;

use crate::qt_backend::{find_executable, launch_command_in_terminal, qobject, spawn_detached};
use crate::qt_convert::serialize_to_qvariant_map;

type QMapQStringQVariant = QMap<QMapPair_QString_QVariant>;

impl qobject::Backend {
    pub fn check_export_space(
        &self,
        name: &QString,
        include_workspace: bool,
    ) -> QMapQStringQVariant {
        let name_s = name.to_string();
        let env = environment_by_name(&self.engine.environments, &name_s);
        let disk_usage_bytes = env.as_ref().map_or(0, |e| e.disk_usage_bytes);
        let disk_usage_str = env
            .as_ref()
            .map(|e| e.disk_usage.clone())
            .unwrap_or_default();
        let mut ws_path = env
            .as_ref()
            .map(|e| e.workspace_path.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.workspace_path_for(&name_s));
        if ws_path.is_empty() {
            ws_path = self.workspace_path_for(&name_s);
        }
        let bdir = backups_dir_path(&self.engine.config.os_storage_path);
        let val = check_export_space(
            disk_usage_bytes,
            &disk_usage_str,
            &ws_path,
            include_workspace,
            self.demo_mode,
            &bdir,
        );
        serialize_to_qvariant_map(&val)
    }

    pub fn export_environment(
        mut self: Pin<&mut Self>,
        name: &QString,
        include_workspace: bool,
    ) -> QString {
        let name_s = name.to_string();
        let env_exists =
            environment_by_name(&self.engine.environments, &sanitize_name(&name_s)).is_some();
        let clean = match validate_export_request(*self.busy(), &name_s, env_exists) {
            Ok(c) => c,
            Err(e) => return QString::from(e.as_str()),
        };

        let env = environment_by_name(&self.engine.environments, &clean);
        let disk_usage_bytes = env.as_ref().map_or(0, |e| e.disk_usage_bytes);
        let disk_usage_str = env
            .as_ref()
            .map(|e| e.disk_usage.clone())
            .unwrap_or_default();
        let mut ws_path = env
            .as_ref()
            .map(|e| e.workspace_path.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.workspace_path_for(&clean));
        if ws_path.starts_with("~/") {
            ws_path = aibox_core::util::paths::expand_tilde(&ws_path);
        }
        let bdir = backups_dir_path(&self.engine.config.os_storage_path);
        let space = check_export_space(
            disk_usage_bytes,
            &disk_usage_str,
            &ws_path,
            include_workspace,
            self.demo_mode,
            &bdir,
        );
        if !space.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            let msg = space
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Insufficient disk space.");
            return QString::from(msg);
        }

        if self.demo_mode {
            let stamp = now_compact_stamp();
            let fn_name = format!("{clean}-{stamp}.tar.gz");
            self.as_mut()
                .set_status_message(&format!("[Demo] Exported backup '{fn_name}'."));
            return QString::default();
        }

        let _ = std::fs::create_dir_all(&bdir);
        let codec = CompressionCodec::detect();
        let ext = codec.extension();
        let stamp = now_compact_stamp();
        let backup_file = format!("{bdir}/{clean}-{stamp}.{ext}");
        let ws_archive_file =
            if include_workspace && !ws_path.is_empty() && Path::new(&ws_path).exists() {
                format!("{bdir}/{clean}-{stamp}-workspace.{ext}")
            } else {
                String::new()
            };

        let script = export_environment_script_with_options(
            &clean,
            &backup_file,
            &ws_path,
            &ws_archive_file,
            codec,
            true,
        );
        self.as_mut().run_shell_task(
            &format!("Exporting backup of '{clean}'"),
            &script,
            "",
            &clean,
        );
        QString::default()
    }

    pub fn export_and_wipe_environment(
        mut self: Pin<&mut Self>,
        name: &QString,
        include_workspace: bool,
        delete_workspace_files: bool,
    ) -> QString {
        let name_s = name.to_string();
        let env_exists =
            environment_by_name(&self.engine.environments, &sanitize_name(&name_s)).is_some();
        let clean = match validate_export_request(*self.busy(), &name_s, env_exists) {
            Ok(c) => c,
            Err(e) => return QString::from(e.as_str()),
        };

        let env = environment_by_name(&self.engine.environments, &clean);
        let disk_usage_bytes = env.as_ref().map_or(0, |e| e.disk_usage_bytes);
        let disk_usage_str = env
            .as_ref()
            .map(|e| e.disk_usage.clone())
            .unwrap_or_default();
        let mut ws_path = env
            .as_ref()
            .map(|e| e.workspace_path.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.workspace_path_for(&clean));
        if ws_path.starts_with("~/") {
            ws_path = aibox_core::util::paths::expand_tilde(&ws_path);
        }
        let bdir = backups_dir_path(&self.engine.config.os_storage_path);
        let space = check_export_space(
            disk_usage_bytes,
            &disk_usage_str,
            &ws_path,
            include_workspace,
            self.demo_mode,
            &bdir,
        );
        if !space.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            let msg = space
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Insufficient disk space.");
            return QString::from(msg);
        }

        if self.demo_mode {
            let stamp = now_compact_stamp();
            let fn_name = format!("{clean}-{stamp}.tar.gz");
            {
                let mut rust = self.as_mut().rust_mut();
                rust.engine.environments.retain(|e| e.name != clean);
                if rust.selected_index >= rust.engine.environments.len() as i32 {
                    rust.selected_index = (rust.engine.environments.len() as i32 - 1).max(0);
                }
            }
            self.as_mut().sync_environments_from_engine();
            self.as_mut().refresh_stale_report();
            self.as_mut().set_status_message(&format!(
                "[Demo] Exported backup '{fn_name}' and wiped '{clean}'."
            ));
            return QString::default();
        }

        let _ = std::fs::create_dir_all(&bdir);
        let codec = CompressionCodec::detect();
        let ext = codec.extension();
        let stamp = now_compact_stamp();
        let backup_file = format!("{bdir}/{clean}-{stamp}.{ext}");
        let ws_archive_file =
            if include_workspace && !ws_path.is_empty() && Path::new(&ws_path).exists() {
                format!("{bdir}/{clean}-{stamp}-workspace.{ext}")
            } else {
                String::new()
            };

        let state_dir = self.engine.paths.state_dir.clone();
        let env_sessions_dir = format!("{}/{}", sessions_dir_path(&state_dir), clean);
        let env_secrets_path =
            environment_secrets_store_path(&self.engine.secrets.secrets_path, &clean);
        stop_file_access_process(&state_dir, &clean);

        let script = export_and_wipe_environment_script_with_options(
            &clean,
            &backup_file,
            &ws_path,
            &ws_archive_file,
            delete_workspace_files,
            &env_sessions_dir,
            &env_secrets_path,
            codec,
            true,
        );
        self.as_mut().run_shell_task(
            &format!("Exporting & wiping '{clean}'"),
            &script,
            "",
            &clean,
        );
        QString::default()
    }

    pub fn import_environment(
        mut self: Pin<&mut Self>,
        backup_file: &QString,
        new_name: &QString,
        workspace_archive_file: &QString,
    ) -> QString {
        let new_name_s = new_name.to_string();
        let env_exists =
            environment_by_name(&self.engine.environments, &sanitize_name(&new_name_s)).is_some();
        let (target_name, clean_backup, clean_ws_archive) = match validate_import_request(
            *self.busy(),
            &new_name_s,
            env_exists,
            &backup_file.to_string(),
            &workspace_archive_file.to_string(),
            self.demo_mode,
        ) {
            Ok(v) => v,
            Err(e) => return QString::from(e.as_str()),
        };

        let ws_path = self.workspace_path_for(&target_name);
        if self.demo_mode {
            self.as_mut()
                .set_status_message(&format!("[Demo] Imported '{target_name}' from backup."));
            return QString::default();
        }

        if let Err(err) = verify_backup_archive(&clean_backup, &clean_ws_archive) {
            return QString::from(err.to_string().as_str());
        }

        let script = import_environment_script_with_options(
            &clean_backup,
            &target_name,
            &ws_path,
            &clean_ws_archive,
            true,
        );
        self.as_mut().run_shell_task(
            &format!("Importing backup as '{target_name}'"),
            &script,
            &target_name,
            &target_name,
        );
        QString::default()
    }

    pub fn delete_backup(
        mut self: Pin<&mut Self>,
        backup_file: &QString,
        workspace_archive_file: &QString,
    ) {
        if self.demo_mode {
            self.as_mut().set_status_message("[Demo] Backup deleted.");
            return;
        }
        delete_backup_files(
            &backup_file.to_string(),
            &workspace_archive_file.to_string(),
        );
        self.as_mut().refresh_backups();
        self.as_mut().set_status_message("Backup archive deleted.");
    }

    pub fn open_orphan_folder(self: Pin<&mut Self>, path: &QString) {
        let clean = clean_path(path.to_string().trim());
        if clean.is_empty() {
            return;
        }
        let p = Path::new(&clean);
        let target = if p.is_dir() {
            clean
        } else {
            p.parent()
                .map(|par| par.to_string_lossy().to_string())
                .unwrap_or(clean)
        };
        let _ = spawn_detached("xdg-open", &[&target], None);
    }

    pub fn delete_orphan_as_admin(
        mut self: Pin<&mut Self>,
        kind: &QString,
        path: &QString,
    ) -> QString {
        let kind_s = kind.to_string();
        let path_s = path.to_string();
        let ws_base = self.engine.config.workspace_base.clone();
        let bdir = backups_dir_path(&self.engine.config.os_storage_path);
        if self.demo_mode {
            let clean = match validate_orphan_request(&kind_s, &path_s, &ws_base, &bdir) {
                Ok(c) => c,
                Err(e) => return QString::from(e.as_str()),
            };
            return self.delete_orphan(kind, &QString::from(clean.as_str()));
        }

        let has_sudo = find_executable("sudo").is_some();
        let (command, file_name) = match prepare_delete_orphan_as_admin(
            &kind_s,
            &path_s,
            &ws_base,
            &bdir,
            &self.engine.paths.state_dir,
            has_sudo,
        ) {
            Ok(p) => p,
            Err(e) => return QString::from(e.as_str()),
        };

        launch_command_in_terminal(&command);
        self.as_mut().set_status_message(&format!(
            "Admin delete opened in terminal for '{file_name}'."
        ));
        QString::default()
    }

    pub fn delete_orphan(mut self: Pin<&mut Self>, kind: &QString, path: &QString) -> QString {
        let kind_s = kind.to_string();
        let path_s = path.to_string();
        let ws_base = self.engine.config.workspace_base.clone();
        let bdir = backups_dir_path(&self.engine.config.os_storage_path);

        let clean = match validate_orphan_request(&kind_s, &path_s, &ws_base, &bdir) {
            Ok(c) => c,
            Err(e) => return QString::from(e.as_str()),
        };
        let file_name = Path::new(&clean)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&clean)
            .to_string();

        if self.demo_mode {
            self.as_mut()
                .set_status_message(&format!("[Demo] Removed orphan '{file_name}'."));
            return QString::default();
        }

        if !Path::new(&clean).exists() {
            return QString::from(format!("Orphan path '{clean}' no longer exists.").as_str());
        }

        if kind_s == "backup-workspace" {
            if std::fs::remove_file(&clean).is_err() {
                return QString::from(
                    format!("Could not delete orphan archive '{clean}'.").as_str(),
                );
            }
            self.as_mut().refresh_backups();
            self.as_mut().refresh_stale_report();
            self.as_mut()
                .set_status_message(&format!("Removed orphan archive '{file_name}'."));
            return QString::default();
        }

        if *self.busy() {
            return QString::from("Another operation is currently running.");
        }

        let script = delete_orphan_script(&kind_s, &clean);
        self.as_mut()
            .run_shell_task(&format!("Removing orphan '{file_name}'"), &script, "", "");
        QString::default()
    }
}
