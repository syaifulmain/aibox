use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QString, QStringList};

use aibox_core::incus::list::environment_by_name;
use aibox_core::secrets::state::{
    build_clear_global_secret_script, build_set_environment_secrets_script,
};
use aibox_core::secrets::store::parse_env_secret_names;

use crate::qt_backend::qobject;
use crate::variant::serialize_to_qvariant_list;

impl qobject::Backend {
    fn sync_secrets_properties(mut self: Pin<&mut Self>) {
        let err = QString::from(self.engine.secrets.secrets_error.as_str());
        {
            let mut rust = self.as_mut().rust_mut();
            rust.secrets_error = err;
        }
        self.as_mut().secrets_changed();
    }

    fn sync_environment_secrets_properties(mut self: Pin<&mut Self>) {
        let q_list = serialize_to_qvariant_list(&self.engine.secrets.environment_secrets);
        let q_err = QString::from(self.engine.secrets.environment_secrets_error.as_str());
        let q_env = QString::from(self.engine.secrets.environment_secrets_env.as_str());
        {
            let mut rust = self.as_mut().rust_mut();
            rust.environment_secrets = q_list;
            rust.environment_secrets_error = q_err;
            rust.environment_secrets_env_name = q_env;
        }
        self.as_mut().environment_secrets_changed();
    }

    fn assigned_secrets_for(&self, env_name: &str) -> Vec<String> {
        let trimmed = env_name.trim();
        if let Some(env) = environment_by_name(&self.engine.environments, trimmed) {
            return env.secret_names.clone();
        }
        Vec::new()
    }

    pub fn reload_secrets(mut self: Pin<&mut Self>) {
        let demo = self.demo_mode;
        {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.secrets.reload_secrets(demo);
        }
        self.as_mut().sync_secrets_properties();
    }

    pub fn reload_environment_secrets(mut self: Pin<&mut Self>, env_name: &QString) {
        let env_s = env_name.to_string();
        let assigned = self.assigned_secrets_for(&env_s);
        let demo = self.demo_mode;
        {
            let mut rust = self.as_mut().rust_mut();
            rust.engine
                .secrets
                .reload_environment_secrets(&env_s, &assigned, demo);
        }
        self.as_mut().sync_environment_secrets_properties();
    }

    pub fn save_secret(mut self: Pin<&mut Self>, name: &QString, value: &QString) -> QString {
        let name_s = name.to_string();
        let val_s = value.to_string();
        let demo = self.demo_mode;
        let res = {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.secrets.save_secret(&name_s, &val_s, demo)
        };
        if let Err(e) = res {
            return QString::from(e.to_string().as_str());
        }
        self.as_mut().sync_secrets_properties();
        QString::default()
    }

    pub fn delete_secret(mut self: Pin<&mut Self>, name: &QString) -> QString {
        let name_s = name.to_string();
        let demo = self.demo_mode;
        let res = {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.secrets.delete_secret(&name_s, demo)
        };
        if let Err(e) = res {
            return QString::from(e.to_string().as_str());
        }
        self.as_mut().sync_secrets_properties();
        QString::default()
    }

    pub fn copy_secret(self: Pin<&mut Self>, name: &QString) -> bool {
        let clean = name.to_string().trim().to_string();
        let found = self
            .engine
            .secrets
            .secrets
            .iter()
            .find(|(k, _)| k == &clean)
            .map(|(_, v)| v.clone());
        match found {
            Some(val) => {
                let status = format!("Copied secret '{clean}' to clipboard.");
                self.copy_text_with_status(&val, &status)
            }
            None => false,
        }
    }

    pub fn set_global_secret_instance_visible(
        mut self: Pin<&mut Self>,
        name: &QString,
        visible: bool,
    ) -> QString {
        let name_s = name.to_string();
        let demo = self.demo_mode;
        let res = {
            let mut rust = self.as_mut().rust_mut();
            rust.engine
                .secrets
                .set_global_secret_instance_visible(&name_s, visible, demo)
        };
        if let Err(e) = res {
            return QString::from(e.to_string().as_str());
        }

        if !visible {
            let clean = name_s.trim().to_string();
            let mut updates: Vec<(String, Vec<String>)> = Vec::new();
            let mut changed_envs = false;
            {
                let mut rust = self.as_mut().rust_mut();
                for env in &mut rust.engine.environments {
                    if env.secret_names.iter().any(|s| s == &clean) {
                        env.secret_names.retain(|s| s != &clean);
                        changed_envs = true;
                        if !demo {
                            updates.push((env.name.clone(), env.secret_names.clone()));
                        }
                    }
                }
            }
            if changed_envs {
                self.as_mut().sync_environments_from_engine();
            }
            if !demo && !updates.is_empty() {
                let script = build_clear_global_secret_script(&updates);
                self.as_mut().run_shell_task(
                    &format!("Removing secret '{clean}' from environments"),
                    &script,
                    "",
                    "",
                );
            }
        }

        self.as_mut().sync_secrets_properties();
        let current_env = self.engine.secrets.environment_secrets_env.clone();
        if !current_env.is_empty() {
            self.as_mut()
                .reload_environment_secrets(&QString::from(current_env.as_str()));
        }
        QString::default()
    }

    pub fn save_environment_secret(
        mut self: Pin<&mut Self>,
        env_name: &QString,
        name: &QString,
        value: &QString,
    ) -> QString {
        let env_s = env_name.to_string();
        let env_trimmed = env_s.trim().to_string();
        let assigned = self.assigned_secrets_for(&env_trimmed);
        let demo = self.demo_mode;
        let res = {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.secrets.save_environment_secret(
                &env_s,
                &name.to_string(),
                &value.to_string(),
                &assigned,
                demo,
            )
        };
        if let Err(e) = res {
            return QString::from(e.to_string().as_str());
        }
        if demo || self.engine.secrets.environment_secrets_env == env_trimmed {
            self.as_mut().sync_environment_secrets_properties();
        }
        if !demo {
            self.as_mut().sync_secrets_properties();
        }
        QString::default()
    }

    pub fn delete_environment_secret(
        mut self: Pin<&mut Self>,
        env_name: &QString,
        name: &QString,
    ) -> QString {
        let env_s = env_name.to_string();
        let env_trimmed = env_s.trim().to_string();
        let assigned = self.assigned_secrets_for(&env_trimmed);
        let demo = self.demo_mode;
        let res = {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.secrets.delete_environment_secret(
                &env_s,
                &name.to_string(),
                &assigned,
                demo,
            )
        };
        if let Err(e) = res {
            return QString::from(e.to_string().as_str());
        }
        if demo || self.engine.secrets.environment_secrets_env == env_trimmed {
            self.as_mut().sync_environment_secrets_properties();
        }
        if !demo {
            self.as_mut().sync_secrets_properties();
        }
        QString::default()
    }

    pub fn copy_environment_secret(
        mut self: Pin<&mut Self>,
        env_name: &QString,
        name: &QString,
    ) -> bool {
        let env_trimmed = env_name.to_string().trim().to_string();
        let clean_name = name.to_string().trim().to_string();
        if self.engine.secrets.environment_secrets_env != env_trimmed {
            self.as_mut()
                .reload_environment_secrets(&QString::from(env_trimmed.as_str()));
        }
        let found = self
            .engine
            .secrets
            .environment_secrets
            .iter()
            .find(|row| row.name == clean_name)
            .map(|row| row.value.clone());
        match found {
            Some(val) => {
                let status = format!("Copied secret '{clean_name}' to clipboard.");
                self.copy_text_with_status(&val, &status)
            }
            None => false,
        }
    }

    pub fn set_environment_local_secret_enabled(
        mut self: Pin<&mut Self>,
        env_name: &QString,
        name: &QString,
        enabled: bool,
    ) -> QString {
        let env_s = env_name.to_string();
        let env_trimmed = env_s.trim().to_string();
        let assigned = self.assigned_secrets_for(&env_trimmed);
        let demo = self.demo_mode;
        let res = {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.secrets.set_environment_local_secret_enabled(
                &env_s,
                &name.to_string(),
                enabled,
                &assigned,
                demo,
            )
        };
        if let Err(e) = res {
            return QString::from(e.to_string().as_str());
        }
        if demo || self.engine.secrets.environment_secrets_env == env_trimmed {
            self.as_mut().sync_environment_secrets_properties();
        }
        QString::default()
    }

    pub fn set_environment_secrets(
        mut self: Pin<&mut Self>,
        name: &QString,
        secret_names: &QStringList,
    ) {
        let name_s = name.to_string();
        if name_s.is_empty() {
            return;
        }
        let joined = secret_names
            .iter()
            .map(|q| q.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let clean_names = parse_env_secret_names(&joined);

        if self.demo_mode {
            let mut updated = false;
            {
                let mut rust = self.as_mut().rust_mut();
                for env in &mut rust.engine.environments {
                    if env.name == name_s {
                        env.secret_names = clean_names.clone();
                        updated = true;
                        break;
                    }
                }
            }
            if updated {
                self.as_mut().sync_environments_from_engine();
                if self.engine.secrets.environment_secrets_env == name_s {
                    self.as_mut().reload_environment_secrets(name);
                }
            }
            return;
        }

        if *self.busy() {
            return;
        }

        let updated_rows = {
            let mut rust = self.as_mut().rust_mut();
            rust.engine
                .secrets
                .update_environment_secret_assignments(&name_s, &clean_names)
        };
        if updated_rows {
            self.as_mut().sync_environment_secrets_properties();
        }
        let script = build_set_environment_secrets_script(&name_s, &clean_names);
        self.as_mut().run_shell_task(
            &format!("Updating secrets for '{name_s}'"),
            &script,
            "",
            &name_s,
        );
    }

    pub fn copy_text(self: Pin<&mut Self>, text: &QString, label: &QString) -> bool {
        let text_s = text.to_string();
        let label_s = label.to_string();
        let status = if label_s.is_empty() {
            "Copied to clipboard.".to_string()
        } else {
            format!("Copied {label_s} to clipboard.")
        };
        self.copy_text_with_status(&text_s, &status)
    }
}
