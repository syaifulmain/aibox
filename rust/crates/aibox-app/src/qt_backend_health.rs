use std::path::Path;
use std::pin::Pin;

use cxx_qt_lib::QString;

use aibox_core::backup::available_disk_bytes;
use aibox_core::display::{
    build_audio_plan, build_desktop_plan, detect_host_audio, detect_host_session, gpu_env,
    gpu_test_script, merge_env_pairs, set_audio_mode_script, set_desktop_mode_script,
    test_audio_script, AudioMode, DesktopMode,
};
use aibox_core::health::idle::{
    set_environment_auto_stop_script, set_environment_idle_stop_script,
};
use aibox_core::health::pool::{
    pool_warning_for_create, pool_warning_for_quota_edit, resize_storage_pool_script,
    sum_env_root_disk_quotas, validate_storage_pool_resize,
};
use aibox_core::task::lock::is_task_lock_active;
use aibox_core::util::paths::home_dir;

use crate::qt_backend::qobject;

impl qobject::Backend {
    pub fn set_environment_auto_stop(mut self: Pin<&mut Self>, name: &QString, enabled: bool) {
        let name_s = name.to_string();
        if name_s.is_empty() || *self.busy() {
            return;
        }
        let script = set_environment_auto_stop_script(&name_s, enabled);
        self.as_mut().run_shell_task(
            &format!("Updating auto-stop for '{name_s}'"),
            &script,
            "",
            &name_s,
        );
    }

    pub fn set_environment_desktop(mut self: Pin<&mut Self>, name: &QString, enabled: bool) {
        let name_s = name.to_string();
        if name_s.is_empty() || *self.busy() {
            return;
        }
        let is_vm = self
            .engine
            .environments
            .iter()
            .find(|env| env.name == name_s)
            .is_some_and(|env| aibox_core::provision::is_vm_env_type(&env.env_type));
        if is_vm {
            return;
        }
        let script = set_desktop_mode_script(
            &name_s,
            DesktopMode::from_enabled(enabled),
            &detect_host_session(),
        );
        self.as_mut().run_shell_task(
            &format!("Updating desktop integration for '{name_s}'"),
            &script,
            "",
            &name_s,
        );
    }

    pub fn set_environment_audio(mut self: Pin<&mut Self>, name: &QString, mode: &QString) {
        let name_s = name.to_string();
        if name_s.is_empty() || *self.busy() {
            return;
        }
        let Some(mode) = AudioMode::try_parse(&mode.to_string()) else {
            return;
        };
        let (is_vm, is_template) = self
            .engine
            .environments
            .iter()
            .find(|env| env.name == name_s)
            .map(|env| {
                (
                    aibox_core::provision::is_vm_env_type(&env.env_type),
                    env.is_template,
                )
            })
            .unwrap_or((false, false));
        if is_vm || is_template {
            return;
        }
        let script = set_audio_mode_script(&name_s, mode, &detect_host_audio());
        self.as_mut().run_shell_task(
            &format!("Updating audio for '{name_s}'"),
            &script,
            "",
            &name_s,
        );
    }

    pub fn test_environment_audio(mut self: Pin<&mut Self>, name: &QString) {
        let name_s = name.to_string();
        if name_s.is_empty() || *self.busy() {
            return;
        }
        let Some(env) = self
            .engine
            .environments
            .iter()
            .find(|env| env.name == name_s)
        else {
            return;
        };
        if aibox_core::provision::is_vm_env_type(&env.env_type) || env.is_template {
            return;
        }
        let host = detect_host_audio();
        let plan = build_audio_plan(AudioMode::parse(&env.audio_mode), &host, host.uid);
        let script = test_audio_script(&name_s, &plan);
        self.as_mut().run_shell_task(
            &format!("Testing audio in '{name_s}'"),
            &script,
            "",
            &name_s,
        );
    }

    /// Runs `vainfo`/`glxinfo -B` inside a container with the same desktop
    /// session environment that Enter/exec use.
    ///
    /// VMs use guest drivers and do not see the host card, so the test is
    /// rejected for them (and for templates, which are not meant to run).
    pub fn test_environment_gpu(mut self: Pin<&mut Self>, name: &QString) {
        let name_s = name.to_string();
        if name_s.is_empty() || *self.busy() {
            return;
        }
        let Some(env) = self
            .engine
            .environments
            .iter()
            .find(|env| env.name == name_s)
        else {
            return;
        };
        if aibox_core::provision::is_vm_env_type(&env.env_type) || env.is_template {
            return;
        }
        let plan = build_desktop_plan(
            DesktopMode::from_enabled(env.desktop_enabled),
            &detect_host_session(),
        );
        let env_pairs = merge_env_pairs(&[&plan.env, &gpu_env()]);
        let script = gpu_test_script(&name_s, &env_pairs);
        self.as_mut()
            .run_shell_task(&format!("Testing GPU in '{name_s}'"), &script, "", &name_s);
    }

    pub fn set_environment_idle_stop(
        mut self: Pin<&mut Self>,
        name: &QString,
        value: &QString,
        allow_with_port_forwards: bool,
    ) {
        let name_s = name.to_string();
        if name_s.is_empty() || *self.busy() {
            return;
        }
        let script =
            set_environment_idle_stop_script(&name_s, &value.to_string(), allow_with_port_forwards);
        self.as_mut().run_shell_task(
            &format!("Updating idle auto-stop for '{name_s}'"),
            &script,
            "",
            &name_s,
        );
    }

    pub fn pool_warning_for_create(&self, disk_spec: &QString) -> QString {
        let pool = &self.engine.pool_info;
        if !pool.valid {
            return QString::default();
        }
        let (allocated_bytes, unbounded_count) =
            sum_env_root_disk_quotas(&self.engine.environments, "", &pool.name);
        let warn = pool_warning_for_create(
            pool,
            allocated_bytes,
            unbounded_count,
            &disk_spec.to_string(),
        );
        QString::from(warn.as_str())
    }

    pub fn pool_warning_for_quota_edit(&self, env_name: &QString, disk_spec: &QString) -> QString {
        let pool = &self.engine.pool_info;
        if !pool.valid {
            return QString::default();
        }
        let fallback: Vec<(String, i64)> = self
            .engine
            .environments
            .iter()
            .filter(|e| pool.name.is_empty() || e.root_pool.is_empty() || e.root_pool == pool.name)
            .map(|e| (e.name.clone(), e.disk_quota_bytes))
            .collect();
        let warn = pool_warning_for_quota_edit(
            pool,
            "",
            &fallback,
            &env_name.to_string(),
            &disk_spec.to_string(),
        );
        QString::from(warn.as_str())
    }

    pub fn validate_storage_pool_resize(&self, size_spec: &QString) -> QString {
        let pool = &self.engine.pool_info;
        let (lock_active, _) =
            is_task_lock_active(Path::new(&self.engine.paths.state_dir), *self.busy());
        let probe_path = if !self.engine.config.os_storage_path.is_empty() {
            self.engine.config.os_storage_path.clone()
        } else {
            home_dir().to_string_lossy().to_string()
        };
        let avail = available_disk_bytes(&probe_path);
        let err = validate_storage_pool_resize(
            *self.busy(),
            lock_active,
            pool,
            &size_spec.to_string(),
            avail,
        );
        QString::from(err.as_str())
    }

    pub fn resize_storage_pool(mut self: Pin<&mut Self>, size_spec: &QString) -> QString {
        let err = self.validate_storage_pool_resize(size_spec);
        if !err.is_empty() {
            return err;
        }
        let pool_name = self.engine.pool_info.name.clone();
        let clean_size = size_spec.to_string().trim().to_string();
        let script = resize_storage_pool_script(&pool_name, &clean_size);
        self.as_mut().run_shell_task(
            &format!("Growing storage pool '{pool_name}' to {clean_size}"),
            &script,
            "",
            "",
        );
        QString::default()
    }
}
