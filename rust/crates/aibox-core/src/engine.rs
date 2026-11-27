use std::path::Path;

use chrono::Local;

use serde_json::Value;

use crate::config::error::ConfigError;
use crate::config::model::{self, Config};
use crate::health::pool::{parse_incus_storage_info, sum_env_root_disk_quotas, StoragePoolInfo};
use crate::history::{
    acknowledge_operation_history, clear_operation_history, load_history_seen_marker,
    load_operation_history, recent_failed_history_count, record_operation_history,
    RecordOperationParams,
};
use crate::incus::catalog::{build_os_catalog, catalog_for_entries, OsCatalogEntry};
use crate::incus::list::{
    parse_checked as parse_incus_list_checked,
    parse_if_changed_checked as parse_incus_list_if_changed_checked,
    templates as extract_templates, CounterState, Environment, ParseDefaults,
};
use crate::process::CommandRunner;
use crate::secrets::error::SecretsError;
use crate::secrets::state::SecretsState;
use crate::task::lock::{acquire_cli_task_lock, is_task_lock_active, release_cli_task_lock};
use crate::util::error::IoError;
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

/// Typed error for `Engine` operations.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Incus daemon is not reachable: {0}")]
    IncusDaemonUnreachable(String),
    #[error("invalid JSON from {context}: {source}")]
    InvalidJson {
        context: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Secrets(#[from] SecretsError),
    #[error(transparent)]
    Io(#[from] IoError),
}

/// Raw Incus CLI outputs captured during `Engine::fetch_snapshot` on a worker thread.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawSnapshot {
    pub list_json: Vec<u8>,
    pub images_json: Vec<u8>,
    pub storage_info_text: String,
    pub pool_size_text: String,
    pub include_catalog: bool,
    pub include_pool: bool,
}

/// Pure parameters controlling how `Engine::apply_snapshot` parses and evaluates a snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RefreshContext {
    pub elapsed_wall_ns: i64,
    pub track_cpu_and_net: bool,
    pub force_reparse: bool,
    pub disk_warn_percent: i32,
    pub disk_critical_percent: i32,
    pub downloading_os_id: String,
    pub compute_pool_quotas: bool,
}

/// Change flags and derived pool quota totals returned by `Engine::apply_snapshot`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RefreshDelta {
    pub environments_changed: bool,
    pub catalog_changed: bool,
    pub pool_changed: bool,
    pub list_parsed: bool,
    pub allocated_bytes: i64,
    pub unbounded_count: i32,
}

/// In-memory cache of operation history entries and the seen marker.
#[derive(Debug, Clone, Default)]
pub struct HistoryCache {
    pub entries: Vec<Value>,
    pub seen_id: String,
    pub loaded: bool,
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
    pub history_cache: HistoryCache,
}

impl Engine {
    pub fn new(path_override: Option<&str>, project_mode: bool, cwd: &str) -> Self {
        let mut paths = ResolvedPaths::resolve(path_override, project_mode, cwd);
        let config = if Path::new(&paths.config_file_path).exists() {
            match model::load_checked(
                &paths.config_file_path,
                &paths.workspace_base,
                &paths.os_storage_path,
            ) {
                Ok(cfg) => cfg,
                Err(err) => {
                    eprintln!("warning: {err}; using default configuration");
                    Config::new_defaults(&paths.workspace_base, &paths.os_storage_path)
                }
            }
        } else {
            Config::new_defaults(&paths.workspace_base, &paths.os_storage_path)
        };
        paths.workspace_base = config.workspace_base.clone();
        paths.os_storage_path = config.os_storage_path.clone();

        let mut secrets = SecretsState::new(String::new());
        secrets.set_secrets_store_path("", false);

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
            history_cache: HistoryCache::default(),
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
            history_cache: HistoryCache::default(),
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

    /// Fetches raw Incus CLI outputs without mutating `Engine` (safe to run on a worker thread).
    pub fn fetch_snapshot(runner: &dyn CommandRunner) -> Result<RawSnapshot, EngineError> {
        Self::fetch_snapshot_with_options(runner, true, true)
    }

    /// Fetches raw Incus CLI outputs with optional catalog and storage pool queries.
    pub fn fetch_snapshot_with_options(
        runner: &dyn CommandRunner,
        include_catalog: bool,
        include_pool: bool,
    ) -> Result<RawSnapshot, EngineError> {
        let out = runner.run("incus", &["list", "--format=json"]);
        if out.exit_code != 0 {
            let err = out.std_err.trim();
            return Err(EngineError::IncusDaemonUnreachable(if err.is_empty() {
                "Incus daemon is not reachable.".to_string()
            } else {
                err.to_string()
            }));
        }

        let mut images_json = Vec::new();
        if include_catalog {
            let img_out = runner.run("incus", &["image", "list", "--format=json"]);
            if img_out.exit_code == 0 {
                images_json = img_out.std_out.into_bytes();
            }
        }

        let mut storage_info_text = String::new();
        let mut pool_size_text = String::new();
        if include_pool {
            let mut pool_out = runner.run("incus", &["storage", "info", "default", "--bytes"]);
            if pool_out.exit_code != 0 {
                pool_out = runner.run("incus", &["storage", "info", "default"]);
            }
            if pool_out.exit_code == 0 {
                storage_info_text = pool_out.std_out;
            }

            let size_out = runner.run("incus", &["storage", "get", "default", "size"]);
            if size_out.exit_code == 0 {
                pool_size_text = size_out.std_out.trim().to_string();
            }
        }

        Ok(RawSnapshot {
            list_json: out.std_out.into_bytes(),
            images_json,
            storage_info_text,
            pool_size_text,
            include_catalog,
            include_pool,
        })
    }

    /// Applies a previously fetched `RawSnapshot` to `Engine` state and returns what changed.
    pub fn apply_snapshot(
        &mut self,
        raw: &RawSnapshot,
        ctx: &RefreshContext,
    ) -> Result<RefreshDelta, EngineError> {
        let defaults = self.parse_defaults();
        let parsed_envs = if ctx.force_reparse || !ctx.track_cpu_and_net {
            let envs = parse_incus_list_checked(
                &raw.list_json,
                &self.paths.workspace_base,
                &mut self.counter_state,
                ctx.elapsed_wall_ns,
                defaults,
                ctx.track_cpu_and_net,
            )
            .map_err(|source| EngineError::InvalidJson {
                context: "incus list",
                source,
            })?;
            if ctx.track_cpu_and_net {
                self.counter_state.last_list_hash = Some(crate::incus::list::hash_list_inputs(
                    &raw.list_json,
                    &self.paths.workspace_base,
                    defaults,
                ));
                self.counter_state.last_parse_had_active_rates = envs.iter().any(|e| {
                    e.running && (e.cpu_percent > 0 || e.net_rx_rate > 0 || e.net_tx_rate > 0)
                });
            }
            Some(envs)
        } else {
            parse_incus_list_if_changed_checked(
                &raw.list_json,
                &self.paths.workspace_base,
                &mut self.counter_state,
                ctx.elapsed_wall_ns,
                defaults,
                ctx.track_cpu_and_net,
            )
            .map_err(|source| EngineError::InvalidJson {
                context: "incus list",
                source,
            })?
        };

        let mut delta = RefreshDelta::default();
        if let Some(mut envs) = parsed_envs {
            delta.list_parsed = true;
            if ctx.disk_warn_percent > 0 || ctx.disk_critical_percent > 0 {
                for env in &mut envs {
                    env.disk_alert_level = if env.disk_percent >= ctx.disk_critical_percent {
                        2
                    } else if env.disk_percent >= ctx.disk_warn_percent {
                        1
                    } else {
                        0
                    };
                }
            }
            delta.environments_changed = envs != self.environments;
            self.templates = extract_templates(&envs).into_iter().cloned().collect();
            self.environments = envs;
        }

        // Root file access lives outside `incus list`, so its state must be
        // re-evaluated on every refresh, not only when the list JSON changes.
        if delta.list_parsed || !self.environments.is_empty() || !self.file_access.envs.is_empty() {
            self.refresh_file_access();
        }

        if raw.include_catalog {
            let next_catalog = if !self.config.os_catalog.is_empty() {
                catalog_for_entries(
                    &self.config.os_catalog,
                    &raw.images_json,
                    &ctx.downloading_os_id,
                    &self.config.os_storage_path,
                )
            } else {
                build_os_catalog(&raw.images_json, &ctx.downloading_os_id)
            };
            delta.catalog_changed = next_catalog != self.os_catalog;
            self.os_catalog = next_catalog;
        }

        if raw.include_pool {
            let mut next_pool = parse_incus_storage_info(&raw.storage_info_text);
            if next_pool.valid && !raw.pool_size_text.is_empty() {
                next_pool.size_text = raw.pool_size_text.clone();
                next_pool.resizable = true;
            }
            if ctx.compute_pool_quotas {
                let (allocated, unbounded) =
                    sum_env_root_disk_quotas(&self.environments, "", &next_pool.name);
                delta.allocated_bytes = allocated;
                delta.unbounded_count = unbounded;
            }
            delta.pool_changed = next_pool != self.pool_info;
            self.pool_info = next_pool;
        }

        Ok(delta)
    }

    /// Synchronous refresh of `incus list --format=json` (used by CLI).
    pub fn refresh_sync(
        &mut self,
        runner: &dyn CommandRunner,
    ) -> Result<RefreshDelta, EngineError> {
        let raw = Self::fetch_snapshot_with_options(runner, false, false)?;
        self.apply_snapshot(&raw, &RefreshContext::default())
    }

    /// Full synchronous refresh: `incus list`, `incus image list`, and `incus storage info`.
    pub fn refresh(&mut self, runner: &dyn CommandRunner) -> Result<RefreshDelta, EngineError> {
        let raw = Self::fetch_snapshot(runner)?;
        self.apply_snapshot(&raw, &RefreshContext::default())
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
        &mut self,
        runner: &dyn CommandRunner,
        description: &str,
        env_name: &str,
        script: &str,
    ) -> i32 {
        let started_at = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let out = runner.run_with_timeout("/bin/bash", &["-c", script], None);
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
        if let Some(entries) = record_operation_history(&RecordOperationParams {
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
        }) {
            self.set_history_cache(entries);
        }

        out.exit_code
    }

    /// Reloads operation history and the seen marker from disk into the in-memory cache.
    pub fn reload_history(&mut self) {
        self.history_cache.entries = load_operation_history(&self.paths.state_dir);
        self.history_cache.seen_id = load_history_seen_marker(&self.paths.state_dir);
        self.history_cache.loaded = true;
    }

    /// Returns cached history entries, loading from disk only if not yet loaded.
    pub fn operation_history(&mut self) -> &[Value] {
        if !self.history_cache.loaded {
            self.reload_history();
        }
        &self.history_cache.entries
    }

    /// Returns cached history entries without touching disk.
    pub fn cached_operation_history(&self) -> &[Value] {
        &self.history_cache.entries
    }

    /// Returns the cached seen marker ID without touching disk.
    pub fn history_seen_id(&self) -> &str {
        &self.history_cache.seen_id
    }

    /// Counts unread failed history entries from the in-memory cache without touching disk.
    pub fn recent_failed_history_count(&self) -> i32 {
        recent_failed_history_count(&self.history_cache.entries, &self.history_cache.seen_id)
    }

    /// Updates the in-memory history cache after a new entry is recorded.
    pub fn set_history_cache(&mut self, entries: Vec<Value>) {
        self.history_cache.entries = entries;
        self.history_cache.loaded = true;
    }

    /// Marks the in-memory history cache as stale so the next access reloads from disk.
    pub fn invalidate_history(&mut self) {
        self.history_cache.loaded = false;
    }

    /// Acknowledges the newest cached history entry and updates the in-memory seen marker.
    pub fn acknowledge_history(&mut self) -> Option<String> {
        if !self.history_cache.loaded {
            self.reload_history();
        }
        let newest_id = self
            .history_cache
            .entries
            .first()
            .and_then(|v| v.get("id"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let next_seen = acknowledge_operation_history(
            &self.paths.state_dir,
            newest_id,
            &self.history_cache.seen_id,
        )?;
        self.history_cache.seen_id = next_seen.clone();
        Some(next_seen)
    }

    /// Clears all persisted history and resets the in-memory cache.
    pub fn clear_history(&mut self) {
        clear_operation_history(&self.paths.state_dir);
        self.history_cache.entries.clear();
        self.history_cache.seen_id.clear();
        self.history_cache.loaded = true;
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
    use crate::process::CommandOutput;
    use std::cell::RefCell;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct MockRunner {
        calls: RefCell<Vec<(String, Vec<String>)>>,
        list_exit_code: i32,
        list_json: String,
        list_stderr: String,
        image_exit_code: i32,
        image_json: String,
        pool_exit_code: i32,
        pool_info_out: String,
        pool_size_out: String,
    }

    impl MockRunner {
        fn new(list_json: &str) -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
                list_exit_code: 0,
                list_json: list_json.to_string(),
                list_stderr: String::new(),
                image_exit_code: 0,
                image_json: "[]".to_string(),
                pool_exit_code: 0,
                pool_info_out: "info:\n  description: \"\"\n  driver: btrfs\n  name: default\n  space used: 1000\n  total space: 10000\n".to_string(),
                pool_size_out: "20GiB\n".to_string(),
            }
        }
    }

    impl CommandRunner for MockRunner {
        fn run(&self, program: &str, args: &[&str]) -> CommandOutput {
            self.calls.borrow_mut().push((
                program.to_string(),
                args.iter().map(|s| s.to_string()).collect(),
            ));
            if program == "incus" && args.first() == Some(&"list") {
                CommandOutput {
                    exit_code: self.list_exit_code,
                    std_out: self.list_json.clone(),
                    std_err: self.list_stderr.clone(),
                }
            } else if program == "incus" && args.first() == Some(&"image") {
                CommandOutput {
                    exit_code: self.image_exit_code,
                    std_out: self.image_json.clone(),
                    std_err: String::new(),
                }
            } else if program == "incus" && args.first() == Some(&"storage") {
                if args.get(1) == Some(&"get") {
                    CommandOutput {
                        exit_code: self.pool_exit_code,
                        std_out: self.pool_size_out.clone(),
                        std_err: String::new(),
                    }
                } else {
                    CommandOutput {
                        exit_code: self.pool_exit_code,
                        std_out: self.pool_info_out.clone(),
                        std_err: String::new(),
                    }
                }
            } else {
                CommandOutput {
                    exit_code: 0,
                    std_out: "ok\n".to_string(),
                    std_err: String::new(),
                }
            }
        }

        fn socket_writable(&self, _path: &str) -> bool {
            true
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

        let runner = MockRunner::new(
            r#"[{"name":"dev","type":"container","status":"Running","config":{},"expanded_devices":{},"snapshots":[]}]"#,
        );

        engine.refresh(&runner).expect("refresh");
        assert_eq!(engine.environments.len(), 1);
        assert_eq!(engine.environments[0].name, "dev");
        assert!(engine.selected_environment(0).is_some());
        assert!(engine.selected_environment(1).is_none());

        let code = engine.run_script_blocking(&runner, "Test op", "dev", "echo hi");
        assert_eq!(code, 0);
        assert_eq!(engine.cached_operation_history().len(), 1);
        assert_eq!(
            engine.cached_operation_history()[0]["description"],
            "Test op"
        );
        assert_eq!(engine.recent_failed_history_count(), 0);

        let history = crate::history::load_operation_history(&engine.paths.state_dir);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0]["description"], "Test op");

        // Deleting files on disk does not affect cached count until invalidated/reloaded.
        let _ = fs::remove_dir_all(crate::history::history_dir_path(&engine.paths.state_dir));
        assert_eq!(engine.cached_operation_history().len(), 1);

        engine.clear_history();
        assert!(engine.cached_operation_history().is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn engine_history_cache_tracks_unread_failures_and_acknowledges() {
        let dir = temp_dir("hist-cache");
        let sys_dir = dir.join("sys");
        fs::create_dir_all(sys_dir.join(".state")).expect("mkdir");
        let paths = ResolvedPaths::resolve(Some(&sys_dir.to_string_lossy()), false, "/tmp");
        let cfg = Config::new_defaults(&paths.workspace_base, &paths.os_storage_path);
        let secrets = SecretsState::new(dir.join("secrets.json").to_string_lossy().to_string());
        let mut engine = Engine::from_parts(paths, cfg, secrets);

        let entries = record_operation_history(&RecordOperationParams {
            state_dir: &engine.paths.state_dir,
            max_entries: 50,
            description: "Failing op",
            env_name: "dev",
            status: "failed",
            exit_code: 1,
            started_at_iso: "",
            log_bytes: b"boom\n",
            reason: "",
            secret_values: &[],
        })
        .unwrap();
        engine.set_history_cache(entries);
        assert_eq!(engine.recent_failed_history_count(), 1);

        let seen = engine.acknowledge_history().unwrap();
        assert!(!seen.is_empty());
        assert_eq!(engine.recent_failed_history_count(), 0);

        let _ = fs::remove_dir_all(&dir);
    }

    fn make_test_engine(name: &str) -> (PathBuf, Engine) {
        let dir = temp_dir(name);
        let sys_dir = dir.join("sys");
        fs::create_dir_all(sys_dir.join(".state")).expect("mkdir");
        let paths = ResolvedPaths::resolve(Some(&sys_dir.to_string_lossy()), false, "/tmp");
        let cfg = Config::new_defaults(&paths.workspace_base, &paths.os_storage_path);
        let secrets = SecretsState::new(dir.join("secrets.json").to_string_lossy().to_string());
        (dir, Engine::from_parts(paths, cfg, secrets))
    }

    #[test]
    fn refresh_fails_when_incus_daemon_is_unreachable() {
        let (dir, mut engine) = make_test_engine("daemon-down");
        let mut runner = MockRunner::new("[]");
        runner.list_exit_code = 1;
        runner.list_stderr = "Error: Get \"http://unix.socket/1.0\": dial unix".to_string();

        let err = engine.refresh(&runner).expect_err("expected daemon error");
        assert!(matches!(err, EngineError::IncusDaemonUnreachable(_)));
        assert!(err.to_string().contains("dial unix"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn refresh_fails_on_corrupted_list_json() {
        let (dir, mut engine) = make_test_engine("bad-json");
        let runner = MockRunner::new("{not valid json");

        let err = engine.refresh(&runner).expect_err("expected JSON error");
        assert!(matches!(
            err,
            EngineError::InvalidJson {
                context: "incus list",
                ..
            }
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn refresh_succeeds_when_pool_is_missing() {
        let (dir, mut engine) = make_test_engine("no-pool");
        let mut runner = MockRunner::new(
            r#"[{"name":"box1","type":"container","status":"Stopped","config":{},"expanded_devices":{},"snapshots":[]}]"#,
        );
        runner.pool_exit_code = 1;
        runner.pool_info_out.clear();
        runner.pool_size_out.clear();

        let delta = engine.refresh(&runner).expect("refresh without pool");
        assert!(delta.environments_changed);
        assert_eq!(engine.environments.len(), 1);
        assert_eq!(engine.environments[0].name, "box1");
        assert!(!engine.pool_info.valid);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_snapshot_reports_refresh_delta_flags() {
        let (dir, mut engine) = make_test_engine("delta-flags");
        let mut runner = MockRunner::new(
            r#"[{"name":"box1","type":"container","status":"Running","config":{},"expanded_devices":{},"snapshots":[]}]"#,
        );
        let raw1 = Engine::fetch_snapshot(&runner).expect("fetch 1");
        let ctx = RefreshContext {
            track_cpu_and_net: true,
            compute_pool_quotas: true,
            ..RefreshContext::default()
        };
        let d1 = engine.apply_snapshot(&raw1, &ctx).expect("apply 1");
        assert!(d1.environments_changed);
        assert!(d1.catalog_changed);
        assert!(d1.pool_changed);
        assert!(d1.list_parsed);
        assert_eq!(d1.unbounded_count, 1);

        // Identical snapshot on next tick should report no changes.
        let d2 = engine.apply_snapshot(&raw1, &ctx).expect("apply 2");
        assert!(!d2.environments_changed);
        assert!(!d2.catalog_changed);
        assert!(!d2.pool_changed);

        // Changing pool size triggers pool_changed only.
        runner.pool_size_out = "50GiB\n".to_string();
        let raw2 = Engine::fetch_snapshot(&runner).expect("fetch 2");
        let d3 = engine.apply_snapshot(&raw2, &ctx).expect("apply 3");
        assert!(!d3.environments_changed);
        assert!(!d3.catalog_changed);
        assert!(d3.pool_changed);
        assert_eq!(engine.pool_info.size_text, "50GiB");

        let _ = fs::remove_dir_all(&dir);
    }
}
