use std::path::Path;

use chrono::Local;

use crate::config::model::{self, Config};
use crate::health::checks::CommandRunner;
use crate::health::pool::{parse_incus_storage_info, StoragePoolInfo};
use crate::history::{record_operation_history, RecordOperationParams};
use crate::incus::catalog::{build_os_catalog, OsCatalogEntry};
use crate::incus::list::{
    parse as parse_incus_list, templates as extract_templates, CounterState, Environment,
    ParseDefaults,
};
use crate::secrets::state::SecretsState;
use crate::task::lock::{acquire_cli_task_lock, is_task_lock_active, release_cli_task_lock};
use crate::util::paths::{
    clean_path, configured_system_path, default_os_storage_path, default_workspace_path,
    resolve_project_aibox_dir,
};
use crate::workspace::file_access::{
    apply_file_access_state, ApplyFileAccessResult, FileAccessEnvInput,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPaths {
    pub project_dir: String,
    pub system_path: String,
    pub workspace_base: String,
    pub state_dir: String,
    pub config_file_path: String,
    pub os_storage_path: String,
}

impl ResolvedPaths {
    pub fn resolve(path_override: Option<&str>, project_mode: bool, cwd: &str) -> Self {
        let system_path = match path_override {
            Some(p) if !p.is_empty() && project_mode => {
                let aibox_dir = resolve_project_aibox_dir(p, cwd);
                clean_path(&aibox_dir)
            }
            Some(p) if !p.is_empty() => clean_path(p),
            _ if project_mode => clean_path(&resolve_project_aibox_dir("", cwd)),
            _ => configured_system_path(),
        };

        let project_dir = if project_mode {
            match path_override {
                Some(p) if !p.is_empty() => clean_path(p),
                _ => clean_path(cwd),
            }
        } else {
            String::new()
        };

        let default_ws = if project_mode {
            clean_path(&format!("{system_path}/workspaces"))
        } else {
            default_workspace_path()
        };
        let default_os = default_os_storage_path();
        let config_file_path = clean_path(&format!("{system_path}/config.json"));
        let state_dir = if project_mode {
            clean_path(&format!("{system_path}/state"))
        } else {
            clean_path(&format!("{system_path}/.state"))
        };

        Self {
            project_dir,
            system_path,
            workspace_base: default_ws,
            state_dir,
            config_file_path,
            os_storage_path: default_os,
        }
    }
}

pub struct Engine {
    pub paths: ResolvedPaths,
    pub config: Config,
    pub environments: Vec<Environment>,
    pub templates: Vec<Environment>,
    pub os_catalog: Vec<OsCatalogEntry>,
    pub pool_info: StoragePoolInfo,
    pub counter_state: CounterState,
    pub secrets: SecretsState,
    pub file_access: ApplyFileAccessResult,
}

impl Engine {
    pub fn new(path_override: Option<&str>, project_mode: bool, cwd: &str) -> Self {
        let mut paths = ResolvedPaths::resolve(path_override, project_mode, cwd);
        let config = model::load(
            &paths.config_file_path,
            &paths.workspace_base,
            &paths.os_storage_path,
        );
        paths.workspace_base = config.workspace_base.clone();
        paths.os_storage_path = config.os_storage_path.clone();

        let mut secrets = SecretsState::new(String::new());
        secrets.reload_secrets(false);
        secrets.refresh_instance_secret_values_cache(false);

        Self {
            paths,
            config,
            environments: Vec::new(),
            templates: Vec::new(),
            os_catalog: Vec::new(),
            pool_info: StoragePoolInfo::default(),
            counter_state: CounterState::new(),
            secrets,
            file_access: ApplyFileAccessResult::default(),
        }
    }

    pub fn from_parts(paths: ResolvedPaths, config: Config, secrets: SecretsState) -> Self {
        Self {
            paths,
            config,
            environments: Vec::new(),
            templates: Vec::new(),
            os_catalog: Vec::new(),
            pool_info: StoragePoolInfo::default(),
            counter_state: CounterState::new(),
            secrets,
            file_access: ApplyFileAccessResult::default(),
        }
    }

    fn parse_defaults(&self) -> ParseDefaults {
        ParseDefaults {
            auto_snapshot: self.config.auto_snapshot_enabled,
            auto_stop: self.config.watchdog_auto_stop,
            record_sessions: self.config.record_sessions,
        }
    }

    fn refresh_file_access(&mut self) {
        let inputs: Vec<FileAccessEnvInput> = self
            .environments
            .iter()
            .map(|e| FileAccessEnvInput {
                name: e.name.clone(),
                running: e.running,
            })
            .collect();
        self.file_access = apply_file_access_state(&self.paths.state_dir, &inputs);
    }

    /// Synchronous refresh of `incus list --format=json` (used by `Backend::refreshSync` and CLI).
    pub fn refresh_sync(&mut self, runner: &dyn CommandRunner) -> Result<(), String> {
        let out = runner.run("incus", &["list", "--format=json"]);
        if out.exit_code != 0 {
            let err = out.std_err.trim();
            return Err(if err.is_empty() {
                "Incus daemon is not reachable.".to_string()
            } else {
                err.to_string()
            });
        }

        let defaults = self.parse_defaults();
        let envs = parse_incus_list(
            out.std_out.as_bytes(),
            &self.paths.workspace_base,
            &mut self.counter_state,
            0,
            defaults,
            false,
        );
        self.templates = extract_templates(&envs).into_iter().cloned().collect();
        self.environments = envs;
        self.refresh_file_access();
        Ok(())
    }

    /// Full synchronous refresh: `incus list`, `incus image list`, and `incus storage info`.
    pub fn refresh(&mut self, runner: &dyn CommandRunner) -> Result<(), String> {
        self.refresh_sync(runner)?;

        let img_out = runner.run("incus", &["image", "list", "--format=json"]);
        if img_out.exit_code == 0 && !img_out.std_out.trim().is_empty() {
            self.os_catalog = build_os_catalog(img_out.std_out.as_bytes(), "");
        }

        let mut pool_out = runner.run("incus", &["storage", "info", "default", "--bytes"]);
        if pool_out.exit_code != 0 {
            pool_out = runner.run("incus", &["storage", "info", "default"]);
        }
        if pool_out.exit_code == 0 {
            self.pool_info = parse_incus_storage_info(&pool_out.std_out);
        }

        Ok(())
    }

    pub fn is_task_lock_active(&self) -> (bool, i64) {
        is_task_lock_active(Path::new(&self.paths.state_dir), false)
    }

    pub fn acquire_cli_task_lock(&self, description: &str, env_name: &str) -> bool {
        acquire_cli_task_lock(
            Path::new(&self.paths.state_dir),
            false,
            description,
            env_name,
        )
    }

    pub fn release_cli_task_lock(&self) {
        release_cli_task_lock(Path::new(&self.paths.state_dir));
    }

    /// Runs a shell script synchronously under `/bin/bash -c` and records operation history.
    pub fn run_script_blocking(
        &self,
        runner: &dyn CommandRunner,
        description: &str,
        env_name: &str,
        script: &str,
    ) -> i32 {
        let started_at = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let out = runner.run("/bin/bash", &["-c", script]);
        let mut combined = Vec::new();
        combined.extend_from_slice(out.std_out.as_bytes());
        if !out.std_err.is_empty() {
            if !combined.is_empty() && !combined.ends_with(b"\n") {
                combined.push(b'\n');
            }
            combined.extend_from_slice(out.std_err.as_bytes());
        }

        let status = if out.exit_code == 0 {
            "success"
        } else {
            "failed"
        };
        let secret_vals = self.secrets.all_secret_values();
        let _ = record_operation_history(&RecordOperationParams {
            state_dir: &self.paths.state_dir,
            description,
            env_name,
            status,
            exit_code: out.exit_code,
            started_at_iso: &started_at,
            log_bytes: &combined,
            secret_values: &secret_vals,
            reason: "",
            max_entries: 50,
        });

        out.exit_code
    }

    pub fn selected_environment(&self, selected_index: i32) -> Option<&Environment> {
        if selected_index < 0 {
            return None;
        }
        self.environments.get(selected_index as usize)
    }

    pub fn environments_json(&self) -> String {
        serde_json::to_string(&self.environments).unwrap_or_else(|_| "[]".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::checks::CommandOutput;
    use std::cell::RefCell;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct MockRunner {
        calls: RefCell<Vec<(String, Vec<String>)>>,
        list_json: String,
    }

    impl CommandRunner for MockRunner {
        fn run(&self, program: &str, args: &[&str]) -> CommandOutput {
            self.calls.borrow_mut().push((
                program.to_string(),
                args.iter().map(|s| s.to_string()).collect(),
            ));
            if program == "incus" && args.first() == Some(&"list") {
                CommandOutput {
                    exit_code: 0,
                    std_out: self.list_json.clone(),
                    std_err: String::new(),
                }
            } else if program == "incus" && args.first() == Some(&"image") {
                CommandOutput {
                    exit_code: 0,
                    std_out: "[]".to_string(),
                    std_err: String::new(),
                }
            } else if program == "incus" && args.first() == Some(&"storage") {
                CommandOutput {
                    exit_code: 0,
                    std_out: "info:\n  description: \"\"\n  driver: btrfs\n  name: default\n  space used: 1000\n  total space: 10000\n".to_string(),
                    std_err: String::new(),
                }
            } else {
                CommandOutput {
                    exit_code: 0,
                    std_out: "ok\n".to_string(),
                    std_err: String::new(),
                }
            }
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("aibox_engine_{name}_{nanos}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    #[test]
    fn engine_refresh_and_run_script_blocking() {
        let dir = temp_dir("refresh");
        let sys_dir = dir.join("sys");
        fs::create_dir_all(sys_dir.join(".state")).expect("mkdir");
        let paths = ResolvedPaths::resolve(Some(&sys_dir.to_string_lossy()), false, "/tmp");
        let cfg = Config::new_defaults(&paths.workspace_base, &paths.os_storage_path);
        let secrets = SecretsState::new(dir.join("secrets.json").to_string_lossy().to_string());
        let mut engine = Engine::from_parts(paths, cfg, secrets);

        let runner = MockRunner {
            calls: RefCell::new(Vec::new()),
            list_json: r#"[{"name":"dev","type":"container","status":"Running","config":{},"expanded_devices":{},"snapshots":[]}]"#.to_string(),
        };

        engine.refresh(&runner).expect("refresh");
        assert_eq!(engine.environments.len(), 1);
        assert_eq!(engine.environments[0].name, "dev");
        assert!(engine.selected_environment(0).is_some());
        assert!(engine.selected_environment(1).is_none());

        let code = engine.run_script_blocking(&runner, "Test op", "dev", "echo hi");
        assert_eq!(code, 0);
        let history = crate::history::load_operation_history(&engine.paths.state_dir);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0]["description"], "Test op");

        let _ = fs::remove_dir_all(&dir);
    }
}
