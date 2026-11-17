//! Stateful secrets management, environment resolution, and Incus binding scripts.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::secrets::store::{
    default_secrets_store_path, environment_secrets_store_path, instance_secrets_dir_path,
    is_valid_secret_name, load_secret_name_list, load_secrets_store, save_secrets_store,
};
use crate::util::paths::clean_path;
use crate::util::shell::shell_quote;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvSecretRow {
    pub name: String,
    pub value: String,
    pub masked_value: String,
    pub scope: String,
    pub assigned: bool,
}

impl EnvSecretRow {
    pub fn parse_list_json(json: &str) -> Vec<Self> {
        if json.trim().is_empty() {
            return Vec::new();
        }
        serde_json::from_str(json).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretsListRow {
    pub name: String,
    pub value: String,
    pub masked_value: String,
    pub visible_in_environments: bool,
}

fn masked_value(value: &str) -> String {
    if value.is_empty() {
        "(empty)".to_string()
    } else {
        "••••••••".to_string()
    }
}

/// In-memory cache and mutator state for global and per-environment secrets.
#[derive(Debug, Clone, Default)]
pub struct SecretsState {
    pub secrets_path: String,
    pub secrets: Vec<(String, String)>,
    pub secrets_error: String,
    pub hidden_global_secrets: Vec<String>,
    pub instance_secret_values: Vec<String>,
    pub environment_secrets_env: String,
    pub environment_secrets: Vec<EnvSecretRow>,
    pub environment_secrets_error: String,
}

impl SecretsState {
    pub fn new(secrets_path: String) -> Self {
        Self {
            secrets_path,
            ..Self::default()
        }
    }

    pub fn set_secrets_store_path(&mut self, path: &str, demo_mode: bool) {
        self.secrets_path = if path.is_empty() {
            default_secrets_store_path()
        } else {
            clean_path(path)
        };
        self.reload_secrets(demo_mode);
    }

    pub fn reload_secrets(&mut self, demo_mode: bool) {
        match load_secrets_store(&self.secrets_path) {
            Ok(loaded) => {
                self.secrets = loaded;
                self.secrets_error.clear();
            }
            Err(err) => {
                self.secrets.clear();
                self.secrets_error = err;
            }
        }
        self.hidden_global_secrets = load_secret_name_list(&self.secrets_path, "hiddenInInstances");
        self.refresh_instance_secret_values_cache(demo_mode);
    }

    pub fn refresh_instance_secret_values_cache(&mut self, demo_mode: bool) {
        self.instance_secret_values.clear();
        if demo_mode {
            for row in &self.environment_secrets {
                if !row.value.is_empty()
                    && !self.instance_secret_values.iter().any(|v| v == &row.value)
                {
                    self.instance_secret_values.push(row.value.clone());
                }
            }
            return;
        }

        let dir_path = instance_secrets_dir_path(&self.secrets_path);
        if dir_path.is_empty() || !Path::new(&dir_path).is_dir() {
            return;
        }
        let Ok(entries) = fs::read_dir(&dir_path) else {
            return;
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("json"))
            .collect();
        paths.sort();

        for path in paths {
            if let Ok(pairs) = load_secrets_store(&path.to_string_lossy()) {
                for (_, val) in pairs {
                    if !val.is_empty() && !self.instance_secret_values.iter().any(|v| v == &val) {
                        self.instance_secret_values.push(val);
                    }
                }
            }
        }
    }

    pub fn secrets_list(&self) -> Vec<SecretsListRow> {
        self.secrets
            .iter()
            .map(|(name, value)| SecretsListRow {
                name: name.clone(),
                value: value.clone(),
                masked_value: masked_value(value),
                visible_in_environments: !self.hidden_global_secrets.iter().any(|h| h == name),
            })
            .collect()
    }

    pub fn all_secret_values(&self) -> Vec<String> {
        let mut values = Vec::new();
        for (_, val) in &self.secrets {
            if !val.is_empty() {
                values.push(val.clone());
            }
        }
        for val in &self.instance_secret_values {
            if !val.is_empty() && !values.iter().any(|v| v == val) {
                values.push(val.clone());
            }
        }
        values
    }

    pub fn reload_environment_secrets(
        &mut self,
        env_name: &str,
        assigned_global_names: &[String],
        demo_mode: bool,
    ) {
        self.environment_secrets_env = env_name.trim().to_string();
        self.environment_secrets.clear();
        self.environment_secrets_error.clear();

        let store_path =
            environment_secrets_store_path(&self.secrets_path, &self.environment_secrets_env);
        if store_path.is_empty() {
            self.environment_secrets_error =
                "Select an environment to manage its secrets.".to_string();
            return;
        }

        let mut local = Vec::new();
        let mut disabled = Vec::new();
        if !demo_mode {
            match load_secrets_store(&store_path) {
                Ok(pairs) => {
                    local = pairs;
                    disabled = load_secret_name_list(&store_path, "disabled");
                }
                Err(err) => {
                    self.environment_secrets_error = err;
                }
            }
        }

        // Instance-only secrets come first because they belong to this environment.
        for (name, value) in local {
            let is_assigned = !disabled.iter().any(|d| d == &name);
            self.environment_secrets.push(EnvSecretRow {
                name,
                masked_value: masked_value(&value),
                value,
                scope: "environment".to_string(),
                assigned: is_assigned,
            });
        }
        for (name, value) in &self.secrets {
            if self.hidden_global_secrets.iter().any(|h| h == name) {
                continue;
            }
            let is_assigned = assigned_global_names.iter().any(|a| a == name);
            self.environment_secrets.push(EnvSecretRow {
                name: name.clone(),
                value: value.clone(),
                masked_value: masked_value(value),
                scope: "global".to_string(),
                assigned: is_assigned,
            });
        }
    }

    pub fn update_environment_secret_assignments(
        &mut self,
        env_name: &str,
        names: &[String],
    ) -> bool {
        if self.environment_secrets_env != env_name.trim() {
            return false;
        }
        let mut changed = false;
        for row in &mut self.environment_secrets {
            if row.scope != "global" {
                continue;
            }
            let assigned = names.iter().any(|n| n == &row.name);
            if row.assigned != assigned {
                row.assigned = assigned;
                changed = true;
            }
        }
        changed
    }

    pub fn save_secret(&mut self, name: &str, value: &str, demo_mode: bool) -> Result<(), String> {
        let clean_name = name.trim();
        if !is_valid_secret_name(clean_name) {
            return Err("Secret name must match ^[A-Z_][A-Z0-9_]*$.".to_string());
        }

        if demo_mode {
            upsert_pair(&mut self.secrets, clean_name, value);
            self.secrets_error.clear();
            return Ok(());
        }

        // Keep current in-memory list when recovering permissions on save.
        let mut next =
            load_secrets_store(&self.secrets_path).unwrap_or_else(|_| self.secrets.clone());
        upsert_pair(&mut next, clean_name, value);

        let mut hidden = load_secret_name_list(&self.secrets_path, "hiddenInInstances");
        if hidden.is_empty() && !self.hidden_global_secrets.is_empty() {
            hidden = self.hidden_global_secrets.clone();
        }

        save_secrets_store(&self.secrets_path, &next, &[], &hidden)?;
        self.secrets = next;
        self.hidden_global_secrets = hidden;
        self.secrets_error.clear();
        Ok(())
    }

    pub fn delete_secret(&mut self, name: &str, demo_mode: bool) -> Result<(), String> {
        let clean_name = name.trim();
        let next: Vec<(String, String)> = self
            .secrets
            .iter()
            .filter(|(n, _)| n != clean_name)
            .cloned()
            .collect();

        if demo_mode {
            self.secrets = next;
            self.hidden_global_secrets.retain(|n| n != clean_name);
            self.secrets_error.clear();
            return Ok(());
        }

        let mut hidden = load_secret_name_list(&self.secrets_path, "hiddenInInstances");
        hidden.retain(|n| n != clean_name);
        save_secrets_store(&self.secrets_path, &next, &[], &hidden)?;
        self.secrets = next;
        self.hidden_global_secrets = hidden;
        self.secrets_error.clear();
        Ok(())
    }

    pub fn set_global_secret_instance_visible(
        &mut self,
        name: &str,
        visible: bool,
        demo_mode: bool,
    ) -> Result<(), String> {
        let clean_name = name.trim();
        if !is_valid_secret_name(clean_name) {
            return Err(format!("Invalid secret name '{clean_name}'."));
        }

        if demo_mode {
            if !self.secrets.iter().any(|(n, _)| n == clean_name) {
                return Err(format!("Secret '{clean_name}' was not found."));
            }
            if visible {
                self.hidden_global_secrets.retain(|n| n != clean_name);
            } else if !self.hidden_global_secrets.iter().any(|n| n == clean_name) {
                self.hidden_global_secrets.push(clean_name.to_string());
            }
            return Ok(());
        }

        let pairs = load_secrets_store(&self.secrets_path)?;
        if !pairs.iter().any(|(n, _)| n == clean_name) {
            return Err(format!("Secret '{clean_name}' was not found."));
        }

        let mut hidden = load_secret_name_list(&self.secrets_path, "hiddenInInstances");
        hidden.retain(|n| n != clean_name);
        if !visible {
            hidden.push(clean_name.to_string());
        }

        save_secrets_store(&self.secrets_path, &pairs, &[], &hidden)?;
        self.secrets = pairs;
        self.hidden_global_secrets = hidden;
        self.secrets_error.clear();
        Ok(())
    }

    pub fn save_environment_secret(
        &mut self,
        env_name: &str,
        name: &str,
        value: &str,
        assigned_global_names: &[String],
        demo_mode: bool,
    ) -> Result<(), String> {
        let env = env_name.trim();
        let store_path = environment_secrets_store_path(&self.secrets_path, env);
        if store_path.is_empty() {
            return Err("Select an environment before adding a secret.".to_string());
        }

        let clean_name = name.trim();
        if !is_valid_secret_name(clean_name) {
            return Err("Secret name must match ^[A-Z_][A-Z0-9_]*$.".to_string());
        }

        // Global secrets are read-only in an environment. Reject duplicate names
        // so injection can never pick an ambiguous value.
        let globals = if !demo_mode {
            load_secrets_store(&self.secrets_path).unwrap_or_else(|_| self.secrets.clone())
        } else {
            self.secrets.clone()
        };
        if globals.iter().any(|(n, _)| n == clean_name) {
            return Err(format!(
                "'{clean_name}' is a global secret. Manage it in Tools → Secrets."
            ));
        }

        if demo_mode {
            if self.environment_secrets_env != env {
                self.reload_environment_secrets(env, assigned_global_names, demo_mode);
            }
            let mut updated = false;
            for row in &mut self.environment_secrets {
                if row.scope == "environment" && row.name == clean_name {
                    row.value = value.to_string();
                    row.masked_value = masked_value(value);
                    updated = true;
                    break;
                }
            }
            if !updated {
                self.environment_secrets.insert(
                    0,
                    EnvSecretRow {
                        name: clean_name.to_string(),
                        value: value.to_string(),
                        masked_value: masked_value(value),
                        scope: "environment".to_string(),
                        assigned: true,
                    },
                );
            }
            self.environment_secrets_error.clear();
            self.refresh_instance_secret_values_cache(demo_mode);
            return Ok(());
        }

        let mut next = load_secrets_store(&store_path)?;
        let disabled = load_secret_name_list(&store_path, "disabled");
        upsert_pair(&mut next, clean_name, value);
        save_secrets_store(&store_path, &next, &disabled, &[])?;

        if self.environment_secrets_env == env {
            self.reload_environment_secrets(env, assigned_global_names, demo_mode);
        }
        self.refresh_instance_secret_values_cache(demo_mode);
        Ok(())
    }

    pub fn delete_environment_secret(
        &mut self,
        env_name: &str,
        name: &str,
        assigned_global_names: &[String],
        demo_mode: bool,
    ) -> Result<(), String> {
        let env = env_name.trim();
        let store_path = environment_secrets_store_path(&self.secrets_path, env);
        if store_path.is_empty() {
            return Err("Select an environment before deleting a secret.".to_string());
        }

        let clean_name = name.trim();
        if !is_valid_secret_name(clean_name) {
            return Err(format!("Invalid secret name '{clean_name}'."));
        }

        let mut is_global = self.secrets.iter().any(|(n, _)| n == clean_name);
        if !is_global && !demo_mode {
            if let Ok(globals) = load_secrets_store(&self.secrets_path) {
                is_global = globals.iter().any(|(n, _)| n == clean_name);
            }
        }
        if is_global {
            return Err(format!(
                "'{clean_name}' is a global secret and cannot be deleted from an environment. Manage it in Tools → Secrets."
            ));
        }

        if demo_mode {
            if self.environment_secrets_env != env {
                self.reload_environment_secrets(env, assigned_global_names, demo_mode);
            }
            let before_len = self.environment_secrets.len();
            self.environment_secrets
                .retain(|r| !(r.scope == "environment" && r.name == clean_name));
            if self.environment_secrets.len() == before_len {
                return Err(format!(
                    "Secret '{clean_name}' does not belong to environment '{env}'."
                ));
            }
            self.environment_secrets_error.clear();
            self.refresh_instance_secret_values_cache(demo_mode);
            return Ok(());
        }

        let current = load_secrets_store(&store_path)?;
        let mut disabled = load_secret_name_list(&store_path, "disabled");
        let mut found = false;
        let mut next = Vec::with_capacity(current.len());
        for (n, v) in current {
            if n == clean_name {
                found = true;
            } else {
                next.push((n, v));
            }
        }
        if !found {
            return Err(format!(
                "Secret '{clean_name}' does not belong to environment '{env}'."
            ));
        }
        disabled.retain(|n| n != clean_name);

        save_secrets_store(&store_path, &next, &disabled, &[])?;
        if self.environment_secrets_env == env {
            self.reload_environment_secrets(env, assigned_global_names, demo_mode);
        }
        self.refresh_instance_secret_values_cache(demo_mode);
        Ok(())
    }

    pub fn set_environment_local_secret_enabled(
        &mut self,
        env_name: &str,
        name: &str,
        enabled: bool,
        assigned_global_names: &[String],
        demo_mode: bool,
    ) -> Result<(), String> {
        let env = env_name.trim();
        let store_path = environment_secrets_store_path(&self.secrets_path, env);
        if store_path.is_empty() {
            return Err("Select an environment before changing a secret.".to_string());
        }

        let clean_name = name.trim();
        if !is_valid_secret_name(clean_name) {
            return Err(format!("Invalid secret name '{clean_name}'."));
        }

        if demo_mode {
            for row in &mut self.environment_secrets {
                if row.scope == "environment" && row.name == clean_name {
                    row.assigned = enabled;
                    return Ok(());
                }
            }
            return Err(format!(
                "Secret '{clean_name}' does not belong to environment '{env}'."
            ));
        }

        let pairs = load_secrets_store(&store_path)?;
        if !pairs.iter().any(|(n, _)| n == clean_name) {
            return Err(format!(
                "Secret '{clean_name}' does not belong to environment '{env}'."
            ));
        }

        let mut disabled = load_secret_name_list(&store_path, "disabled");
        disabled.retain(|n| n != clean_name);
        if !enabled {
            disabled.push(clean_name.to_string());
        }

        save_secrets_store(&store_path, &pairs, &disabled, &[])?;
        if self.environment_secrets_env == env {
            self.reload_environment_secrets(env, assigned_global_names, demo_mode);
        }
        Ok(())
    }

    pub fn resolve_environment_secrets(
        &self,
        env_name: &str,
        assigned_global_names: &[String],
        demo_mode: bool,
    ) -> Result<Vec<(String, String)>, String> {
        let mut resolved: Vec<(String, String)> = Vec::new();
        if !assigned_global_names.is_empty() {
            let store = if demo_mode {
                self.secrets.clone()
            } else {
                load_secrets_store(&self.secrets_path)?
            };

            for req_name in assigned_global_names {
                if self.hidden_global_secrets.iter().any(|h| h == req_name) {
                    continue;
                }
                if let Some(pair) = store.iter().find(|(n, _)| n == req_name) {
                    resolved.push(pair.clone());
                }
            }
        }

        // Instance-only secrets are injected automatically for their own
        // environment, without an assignment entry in user.aibox.secrets.
        if !demo_mode {
            let local_path = environment_secrets_store_path(&self.secrets_path, env_name);
            if !local_path.is_empty() {
                let local = load_secrets_store(&local_path)?;
                let disabled = load_secret_name_list(&local_path, "disabled");
                for (name, val) in local {
                    if disabled.iter().any(|d| d == &name) {
                        continue;
                    }
                    if !resolved.iter().any(|(n, _)| n == &name) {
                        resolved.push((name, val));
                    }
                }
            }
        }

        Ok(resolved)
    }
}

fn upsert_pair(list: &mut Vec<(String, String)>, name: &str, value: &str) {
    if let Some(existing) = list.iter_mut().find(|(n, _)| n == name) {
        existing.1 = value.to_string();
    } else {
        list.push((name.to_string(), value.to_string()));
    }
}

/// Builds the Incus script to update `user.aibox.secrets` across environments after
/// hiding a global secret. `updates` holds `(env_name, remaining_secret_names)`.
pub fn build_clear_global_secret_script(updates: &[(String, Vec<String>)]) -> String {
    let mut script =
        String::from("set -euo pipefail\necho '@@STAGE:30:Removing hidden secret bindings...'\n");
    for (env_name, names) in updates {
        let q_env = shell_quote(env_name);
        if names.is_empty() {
            script.push_str(&format!(
                "incus config unset {q_env} user.aibox.secrets >/dev/null 2>&1 || true\n"
            ));
        } else {
            let csv = names.join(",");
            script.push_str(&format!(
                "incus config set {q_env} user.aibox.secrets {}\n",
                shell_quote(&csv)
            ));
        }
    }
    script.push_str("echo '@@STAGE:100:Secret bindings updated.'");
    script
}

/// Builds the Incus script for `Backend::setEnvironmentSecrets`.
pub fn build_set_environment_secrets_script(env_name: &str, clean_names: &[String]) -> String {
    let q_name = shell_quote(env_name);
    if clean_names.is_empty() {
        format!(
            "set -euo pipefail\n\
             echo '@@STAGE:50:Clearing environment secret bindings...'\n\
             incus config unset {q_name} user.aibox.secrets >/dev/null 2>&1 || true\n\
             echo '@@STAGE:100:Secrets updated.'"
        )
    } else {
        let csv = clean_names.join(",");
        format!(
            "set -euo pipefail\n\
             echo '@@STAGE:50:Updating environment secret bindings...'\n\
             incus config set {q_name} user.aibox.secrets {}\n\
             echo '@@STAGE:100:Secrets updated.'",
            shell_quote(&csv)
        )
    }
}
