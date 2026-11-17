use std::path::Path;
use std::pin::Pin;

use cxx_qt_lib::QString;

use aibox_core::incus::list::environment_by_name;
use aibox_core::secrets::store::environment_secrets_store_path;
use aibox_core::session::sessions_dir_path;
use aibox_core::snapshot::scripts::{
    create_snapshot_script, delete_snapshot_script, restore_snapshot_script,
    wipe_environment_script,
};
use aibox_core::snapshot::workspace::convert_workspace_to_subvolume_script;
use aibox_core::util::names::sanitize_name;
use aibox_core::util::paths::clean_path;
use aibox_core::util::shell::shell_quote;
use aibox_core::util::time::{now_compact_stamp, now_epoch_secs};
use aibox_core::workspace::file_access::stop_file_access_process;

use crate::qt_backend::qobject;

impl qobject::Backend {
    pub fn set_environment_auto_snapshot(mut self: Pin<&mut Self>, name: &QString, enabled: bool) {
        let name_s = name.to_string();
        if name_s.is_empty() || *self.busy() {
            return;
        }
        let val = if enabled { "true" } else { "false" };
        let script = format!(
            "set -euo pipefail\necho '@@STAGE:50:Updating auto-snapshot setting...'\n\
             incus config set {} user.aibox.autosnap {}\n\
             echo '@@STAGE:100:Auto-snapshot updated.'",
            shell_quote(&name_s),
            val
        );
        self.as_mut().run_shell_task(
            &format!("Updating auto-snapshot for '{name_s}'"),
            &script,
            "",
            "",
        );
    }

    pub fn set_environment_workspace_snapshot(
        mut self: Pin<&mut Self>,
        name: &QString,
        enabled: bool,
    ) {
        let name_s = name.to_string();
        if name_s.is_empty() || *self.busy() {
            return;
        }
        let val = if enabled { "true" } else { "false" };
        let script = format!(
            "set -euo pipefail\necho '@@STAGE:50:Updating workspace snapshot setting...'\n\
             incus config set {} user.aibox.wssnap {}\n\
             echo '@@STAGE:100:Workspace snapshot setting updated.'",
            shell_quote(&name_s),
            val
        );
        self.as_mut().run_shell_task(
            &format!("Updating workspace snapshots for '{name_s}'"),
            &script,
            "",
            &name_s,
        );
    }

    pub fn convert_workspace_to_subvolume(mut self: Pin<&mut Self>, name: &QString) {
        let name_s = name.to_string();
        if name_s.is_empty() || *self.busy() {
            return;
        }
        let Some(env) = environment_by_name(&self.engine.environments, &name_s) else {
            return;
        };
        if env.running {
            self.as_mut().set_error_message(&format!(
                "Stop '{name_s}' before converting its workspace to a Btrfs subvolume."
            ));
            return;
        }
        let ws_path = self.workspace_path_for(&name_s);
        let script = convert_workspace_to_subvolume_script(&name_s, &ws_path);
        self.as_mut().run_shell_task(
            &format!("Converting workspace of '{name_s}' to Btrfs subvolume"),
            &script,
            "",
            &name_s,
        );
    }

    pub fn delete_pre_subvol_backup(mut self: Pin<&mut Self>, name: &QString) {
        let name_s = name.to_string();
        if name_s.is_empty() || *self.busy() {
            return;
        }
        let pre_path = format!(
            "{}.pre-subvol",
            clean_path(&self.workspace_path_for(&name_s))
        );
        if !Path::new(&pre_path).exists() {
            return;
        }
        let script = format!(
            "set -euo pipefail\n\
             echo '@@STAGE:40:Removing pre-subvolume workspace backup...'\n\
             rm -rf {}\n\
             echo '@@STAGE:100:Backup removed.'",
            shell_quote(&pre_path)
        );
        self.as_mut().run_shell_task(
            &format!("Removing pre-subvolume backup for '{name_s}'"),
            &script,
            "",
            &name_s,
        );
    }

    pub fn create_snapshot(mut self: Pin<&mut Self>, name: &QString, snapshot_name: &QString) {
        let name_s = name.to_string();
        if name_s.is_empty() || *self.busy() {
            return;
        }
        let mut snap = sanitize_name(&snapshot_name.to_string());
        if snap.is_empty() {
            snap = format!("snap-{}", now_compact_stamp());
        }
        let env = environment_by_name(&self.engine.environments, &name_s);
        let ws_path = env
            .map(|e| e.workspace_path.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.workspace_path_for(&name_s));
        let include_ws = env.is_some_and(|e| e.workspace_snapshot_enabled);
        let script = create_snapshot_script(&name_s, &snap, &ws_path, include_ws);
        self.as_mut().run_shell_task(
            &format!("Saving snapshot '{snap}' for '{name_s}'"),
            &script,
            "",
            &name_s,
        );
    }

    pub fn restore_snapshot(
        mut self: Pin<&mut Self>,
        name: &QString,
        snapshot_name: &QString,
        mode: &QString,
    ) {
        let name_s = name.to_string();
        let snap_s = snapshot_name.to_string();
        let mode_s = mode.to_string();
        if name_s.is_empty() || snap_s.is_empty() || *self.busy() {
            return;
        }
        let env = environment_by_name(&self.engine.environments, &name_s);
        let ws_path = env
            .map(|e| e.workspace_path.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.workspace_path_for(&name_s));
        let has_ws_snap = env.is_some_and(|e| e.workspace_snapshots.iter().any(|s| s == &snap_s));
        if mode_s.trim().eq_ignore_ascii_case("workspace") && !has_ws_snap {
            self.as_mut()
                .set_error_message(&format!("Workspace snapshot '{snap_s}' was not found."));
            return;
        }
        let script = restore_snapshot_script(
            &name_s,
            &snap_s,
            &ws_path,
            &mode_s,
            has_ws_snap,
            now_epoch_secs(),
        );
        self.as_mut().run_shell_task(
            &format!("Restoring '{name_s}' to snapshot '{snap_s}'"),
            &script,
            "",
            &name_s,
        );
    }

    pub fn delete_snapshot(mut self: Pin<&mut Self>, name: &QString, snapshot_name: &QString) {
        let name_s = name.to_string();
        let snap_s = snapshot_name.to_string();
        if name_s.is_empty() || snap_s.is_empty() || *self.busy() {
            return;
        }
        let env = environment_by_name(&self.engine.environments, &name_s);
        let ws_path = env
            .map(|e| e.workspace_path.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.workspace_path_for(&name_s));
        let has_incus = env.is_some_and(|e| e.snapshots.iter().any(|s| s == &snap_s));
        let has_ws = env.is_some_and(|e| e.workspace_snapshots.iter().any(|s| s == &snap_s));
        let script =
            delete_snapshot_script(&name_s, &snap_s, &ws_path, has_incus || !has_ws, has_ws);
        self.as_mut().run_shell_task(
            &format!("Deleting snapshot '{snap_s}'"),
            &script,
            "",
            &name_s,
        );
    }

    pub fn wipe_environment(
        mut self: Pin<&mut Self>,
        name: &QString,
        delete_workspace_files: bool,
    ) {
        let name_s = name.to_string();
        if name_s.is_empty() || *self.busy() {
            return;
        }
        let host_ws = self.workspace_path_for(&name_s);
        let state_dir = self.engine.paths.state_dir.clone();
        let env_sessions_dir = format!(
            "{}/{}",
            sessions_dir_path(&state_dir),
            sanitize_name(&name_s)
        );
        let env_secrets_path =
            environment_secrets_store_path(&self.engine.secrets.secrets_path, &name_s);
        stop_file_access_process(&state_dir, &name_s);
        let script = wipe_environment_script(
            &name_s,
            &host_ws,
            delete_workspace_files,
            &env_sessions_dir,
            &env_secrets_path,
        );
        self.as_mut().run_shell_task(
            &format!("Wiping environment '{name_s}'"),
            &script,
            "",
            &name_s,
        );
    }
}
