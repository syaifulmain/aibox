use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QMap, QMapPair_QString_QVariant};

use aibox_core::backup::{build_stale_report, scan_backup_archives};
use aibox_core::config::model::Config;
use aibox_core::engine::{Engine, RawSnapshot, RefreshContext, RefreshDelta, ResolvedPaths};
use aibox_core::health::pool::{sum_env_root_disk_quotas, StoragePoolInfo};
use aibox_core::incus::catalog::OsCatalogEntry;
use aibox_core::incus::list::{templates as extract_templates, CounterState, Environment};
use aibox_core::process::SystemRunner;
use aibox_core::session::scan_recorded_sessions;
use aibox_core::snapshot::{compute_workspace_snapshot_sizes, fetch_incus_snapshot_sizes};
use aibox_core::workspace::file_access::{
    apply_file_access_state, attach_file_access_fields, ApplyFileAccessResult, FileAccessEnvInput,
};

use crate::qt_backend::{qobject, BackendRust};
use crate::qt_convert::{
    json_array_to_qvariant_list, json_object_to_qvariant_map, serialize_to_qvariant_list,
    value_to_qvariant_map,
};

type QMapQStringQVariant = QMap<QMapPair_QString_QVariant>;

pub(crate) const POOL_REFRESH_INTERVAL: Duration = Duration::from_secs(30);

pub(crate) fn should_include_pool(
    explicit: Option<bool>,
    last_pool_refresh_at: Option<Instant>,
    now: Instant,
) -> bool {
    match explicit {
        Some(v) => v,
        None => last_pool_refresh_at
            .map(|t| now.saturating_duration_since(t) >= POOL_REFRESH_INTERVAL)
            .unwrap_or(true),
    }
}

pub(crate) fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

fn hash_json_value(val: &serde_json::Value, hasher: &mut DefaultHasher) {
    match val {
        serde_json::Value::Null => 0u8.hash(hasher),
        serde_json::Value::Bool(b) => {
            1u8.hash(hasher);
            b.hash(hasher);
        }
        serde_json::Value::Number(n) => {
            2u8.hash(hasher);
            if let Some(i) = n.as_i64() {
                0u8.hash(hasher);
                i.hash(hasher);
            } else if let Some(u) = n.as_u64() {
                1u8.hash(hasher);
                u.hash(hasher);
            } else if let Some(f) = n.as_f64() {
                2u8.hash(hasher);
                f.to_bits().hash(hasher);
            }
        }
        serde_json::Value::String(s) => {
            3u8.hash(hasher);
            s.hash(hasher);
        }
        serde_json::Value::Array(arr) => {
            4u8.hash(hasher);
            arr.len().hash(hasher);
            for item in arr {
                hash_json_value(item, hasher);
            }
        }
        serde_json::Value::Object(map) => {
            5u8.hash(hasher);
            map.len().hash(hasher);
            for (k, v) in map {
                k.hash(hasher);
                hash_json_value(v, hasher);
            }
        }
    }
}

pub(crate) fn hash_json_values(values: &[serde_json::Value]) -> u64 {
    let mut hasher = DefaultHasher::new();
    values.len().hash(&mut hasher);
    for v in values {
        hash_json_value(v, &mut hasher);
    }
    hasher.finish()
}

pub(crate) fn build_environment_json_values(
    envs: &[Environment],
    fa: &ApplyFileAccessResult,
) -> Vec<serde_json::Value> {
    let mut env_values: Vec<serde_json::Value> = envs
        .iter()
        .map(|e| serde_json::to_value(e).unwrap_or(serde_json::Value::Null))
        .collect();
    for (idx, val) in env_values.iter_mut().enumerate() {
        attach_file_access_fields(val, fa.envs.get(idx));
    }
    env_values
}

pub(crate) struct WorkerRefreshInput {
    pub(crate) paths: ResolvedPaths,
    pub(crate) config: Config,
    pub(crate) counter_state: CounterState,
    pub(crate) prev_environments: Arc<[Environment]>,
    pub(crate) prev_templates: Vec<Environment>,
    pub(crate) prev_os_catalog: Vec<OsCatalogEntry>,
    pub(crate) prev_pool_info: StoragePoolInfo,
    pub(crate) prev_file_access: ApplyFileAccessResult,
    pub(crate) last_pool_quotas: (i64, i32),
    pub(crate) env_config_gen: u64,
    pub(crate) ctx: RefreshContext,
}

pub(crate) struct WorkerRefreshOutput {
    pub(crate) snapshot_list_hash: u64,
    pub(crate) images_json: Vec<u8>,
    pub(crate) include_catalog: bool,
    pub(crate) include_pool: bool,
    pub(crate) delta: RefreshDelta,
    pub(crate) environments: Arc<[Environment]>,
    pub(crate) templates: Vec<Environment>,
    pub(crate) os_catalog: Vec<OsCatalogEntry>,
    pub(crate) pool_info: StoragePoolInfo,
    pub(crate) pool_quotas: (i64, i32),
    pub(crate) counter_state: CounterState,
    pub(crate) file_access: ApplyFileAccessResult,
    pub(crate) env_values: Vec<serde_json::Value>,
    pub(crate) env_values_hash: u64,
    pub(crate) env_values_unchanged: bool,
    pub(crate) env_config_gen: u64,
    pub(crate) pool_json: Option<serde_json::Value>,
}

/// Parses `RawSnapshot` and pre-serializes JSON payloads on the worker thread so the Qt thread
/// only swaps state, applies snapshot size cache, and converts to `QVariant`.
pub(crate) fn parse_refresh_in_worker(
    snapshot: RawSnapshot,
    mut input: WorkerRefreshInput,
) -> Result<WorkerRefreshOutput, String> {
    let snapshot_list_hash = hash_bytes(&snapshot.list_json);
    input.ctx.compute_pool_quotas = true;

    let mut temp_engine = Engine::from_parts(
        input.paths,
        input.config,
        aibox_core::secrets::SecretsState::new(String::new()),
    );
    temp_engine.counter_state = input.counter_state;
    temp_engine.environments = input.prev_environments.to_vec();
    temp_engine.templates = input.prev_templates;
    temp_engine.os_catalog = input.prev_os_catalog;
    temp_engine.pool_info = input.prev_pool_info;
    temp_engine.file_access = input.prev_file_access.clone();

    let delta = temp_engine
        .apply_snapshot(&snapshot, &input.ctx)
        .map_err(|e| e.to_string())?;

    let env_values_unchanged =
        !delta.list_parsed && temp_engine.file_access == input.prev_file_access;
    let (env_values, env_values_hash) = if env_values_unchanged {
        (Vec::new(), 0)
    } else {
        let vals =
            build_environment_json_values(&temp_engine.environments, &temp_engine.file_access);
        let h = hash_json_values(&vals);
        (vals, h)
    };

    let (pool_quotas, pool_json) = if snapshot.include_pool {
        let quotas = (delta.allocated_bytes, delta.unbounded_count);
        (
            quotas,
            Some(temp_engine.pool_info.to_value(quotas.0, quotas.1)),
        )
    } else if delta.list_parsed && temp_engine.pool_info.valid {
        let quotas =
            sum_env_root_disk_quotas(&temp_engine.environments, "", &temp_engine.pool_info.name);
        if quotas != input.last_pool_quotas {
            (
                quotas,
                Some(temp_engine.pool_info.to_value(quotas.0, quotas.1)),
            )
        } else {
            (input.last_pool_quotas, None)
        }
    } else {
        (input.last_pool_quotas, None)
    };

    let environments: Arc<[Environment]> = if delta.list_parsed {
        Arc::from(temp_engine.environments)
    } else {
        input.prev_environments
    };

    Ok(WorkerRefreshOutput {
        snapshot_list_hash,
        images_json: snapshot.images_json,
        include_catalog: snapshot.include_catalog,
        include_pool: snapshot.include_pool,
        delta,
        environments,
        templates: temp_engine.templates,
        os_catalog: temp_engine.os_catalog,
        pool_info: temp_engine.pool_info,
        pool_quotas,
        counter_state: temp_engine.counter_state,
        file_access: temp_engine.file_access,
        env_values,
        env_values_hash,
        env_values_unchanged,
        env_config_gen: input.env_config_gen,
        pool_json,
    })
}

impl BackendRust {
    pub(crate) fn rebuild_selected_environment_cache(&mut self) {
        if self.selected_index >= 0 {
            if self.demo_mode {
                if let Some(serde_json::Value::Object(map)) =
                    self.demo_environments.get(self.selected_index as usize)
                {
                    let mut val = serde_json::Value::Object(map.clone());
                    attach_file_access_fields(&mut val, None);
                    self.selected_environment_cache = value_to_qvariant_map(&val);
                    return;
                }
            } else if let Some(env) = self.engine.environments.get(self.selected_index as usize) {
                let mut val = serde_json::to_value(env).unwrap_or(serde_json::Value::Null);
                attach_file_access_fields(
                    &mut val,
                    self.engine
                        .file_access
                        .envs
                        .get(self.selected_index as usize),
                );
                self.selected_environment_cache = value_to_qvariant_map(&val);
                return;
            }
        }
        self.selected_environment_cache = QMapQStringQVariant::default();
    }

    pub(crate) fn rebuild_templates_cache(&mut self) {
        if self.demo_mode {
            let tmpls: Vec<serde_json::Value> = self
                .demo_environments
                .iter()
                .filter(|v| v.get("isTemplate").and_then(|b| b.as_bool()) == Some(true))
                .cloned()
                .collect();
            self.templates_cache = json_array_to_qvariant_list(&tmpls);
            return;
        }
        let tmpls: Vec<_> = extract_templates(&self.engine.environments)
            .into_iter()
            .cloned()
            .collect();
        self.templates_cache = serialize_to_qvariant_list(&tmpls);
    }

    pub(crate) fn rebuild_secrets_list_cache(&mut self) {
        let list = self.engine.secrets.secrets_list();
        self.secrets_list_cache = serialize_to_qvariant_list(&list);
    }

    pub(crate) fn env_exists_active(&self, name: &str) -> bool {
        if self.demo_mode {
            self.demo_environments
                .iter()
                .any(|e| e.get("name").and_then(|v| v.as_str()) == Some(name))
        } else {
            aibox_core::incus::list::environment_by_name(&self.engine.environments, name).is_some()
        }
    }

    pub(crate) fn demo_env_str(&self, name: &str, key: &str) -> String {
        self.demo_environments
            .iter()
            .find(|e| e.get("name").and_then(|v| v.as_str()) == Some(name))
            .and_then(|e| e.get(key))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    pub(crate) fn demo_env_i64(&self, name: &str, key: &str) -> i64 {
        self.demo_environments
            .iter()
            .find(|e| e.get("name").and_then(|v| v.as_str()) == Some(name))
            .and_then(|e| e.get(key))
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
    }

    pub(crate) fn demo_env_secret_names(&self, name: &str) -> Vec<String> {
        self.demo_environments
            .iter()
            .find(|e| e.get("name").and_then(|v| v.as_str()) == Some(name))
            .and_then(|e| e.get("secretNames"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(ToString::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl qobject::Backend {
    /// The inspector reads root file access state from this cached map, so it stays
    /// updated whenever `selected_index`, `environments`, or `file_access` changes.
    pub fn selected_environment(&self) -> QMapQStringQVariant {
        self.selected_environment_cache.clone()
    }

    pub(crate) fn sync_demo_environments(mut self: Pin<&mut Self>) {
        let q_envs = json_array_to_qvariant_list(&self.demo_environments);
        let count = self.demo_environments.len() as i32;
        {
            let mut rust = self.as_mut().rust_mut();
            let max_idx = (count - 1).max(0);
            if rust.selected_index >= count {
                rust.selected_index = max_idx;
            }
            rust.environments = q_envs;
            rust.count = count;
            rust.rebuild_templates_cache();
            rust.rebuild_selected_environment_cache();
        }
        self.as_mut().environments_changed();
        self.as_mut().count_changed();
        self.as_mut().selected_index_changed();
    }

    pub(crate) fn sync_environments_from_engine(mut self: Pin<&mut Self>) {
        if self.demo_mode {
            self.sync_demo_environments();
            return;
        }
        let state_dir = self.engine.paths.state_dir.clone();
        let fa_inputs: Vec<FileAccessEnvInput> = self
            .engine
            .environments
            .iter()
            .map(|e| FileAccessEnvInput {
                name: e.name.clone(),
                running: e.running,
            })
            .collect();
        let fa_result = apply_file_access_state(&state_dir, &fa_inputs);
        let env_values = build_environment_json_values(&self.engine.environments, &fa_result);
        let model_hash = hash_json_values(&env_values);
        let q_envs = json_array_to_qvariant_list(&env_values);
        let count = self.engine.environments.len() as i32;
        let envs_arc: Arc<[Environment]> = Arc::from(self.engine.environments.clone());
        let has_running = envs_arc.iter().any(|e| e.running && !e.is_template);
        self.has_running_envs.store(has_running, Ordering::Relaxed);
        {
            let mut rust = self.as_mut().rust_mut();
            // Keep the engine cache in step so `selectedEnvironment` reports the
            // same root file access state as the environment list.
            rust.engine.file_access = fa_result;
            rust.environments = q_envs;
            rust.environments_arc = envs_arc;
            rust.last_environments_hash = model_hash;
            rust.count = count;
            rust.rebuild_templates_cache();
            rust.rebuild_selected_environment_cache();
        }
        self.as_mut().environments_changed();
        self.as_mut().count_changed();
        self.as_mut().selected_index_changed();
    }

    pub fn refresh(self: Pin<&mut Self>) {
        self.refresh_with_options(true, Some(true));
    }

    pub(crate) fn refresh_with_options(
        mut self: Pin<&mut Self>,
        include_catalog: bool,
        include_pool_opt: Option<bool>,
    ) {
        if self.demo_mode {
            return;
        }
        if self
            .refresh_in_flight
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            let mut rust = self.as_mut().rust_mut();
            rust.pending_refresh = Some(match rust.pending_refresh {
                Some((prev_cat, prev_pool)) => {
                    let merged_pool = match (prev_pool, include_pool_opt) {
                        (Some(true), _) | (_, Some(true)) => Some(true),
                        (Some(false), Some(false)) => Some(false),
                        (Some(v), None) | (None, Some(v)) => Some(v),
                        (None, None) => None,
                    };
                    (prev_cat || include_catalog, merged_pool)
                }
                None => (include_catalog, include_pool_opt),
            });
            return;
        }

        let now = Instant::now();
        let include_pool = should_include_pool(include_pool_opt, self.last_pool_refresh_at, now);
        let in_flight = Arc::clone(&self.refresh_in_flight);
        let qt_thread = self.qt_thread();
        let elapsed_wall_ns = self
            .last_refresh_at
            .map(|t| t.elapsed().as_nanos() as i64)
            .unwrap_or(0);

        let worker_input = {
            let mut rust = self.as_mut().rust_mut();
            rust.last_refresh_at = Some(now);
            let warn = rust.watchdog_disk_warn;
            let crit = rust.watchdog_disk_critical;
            let thresholds_changed = rust.last_alert_thresholds != (warn, crit);
            rust.last_alert_thresholds = (warn, crit);

            rust.engine.config.auto_snapshot_enabled = rust.auto_snapshot_enabled;
            rust.engine.config.watchdog_auto_stop = rust.watchdog_auto_stop;
            rust.engine.config.record_sessions = rust.record_sessions;

            WorkerRefreshInput {
                paths: rust.engine.paths.clone(),
                config: rust.engine.config.clone(),
                counter_state: rust.engine.counter_state.clone(),
                prev_environments: Arc::clone(&rust.environments_arc),
                prev_templates: rust.engine.templates.clone(),
                prev_os_catalog: if include_catalog {
                    Vec::new()
                } else {
                    rust.engine.os_catalog.clone()
                },
                prev_pool_info: if include_pool {
                    StoragePoolInfo::default()
                } else {
                    rust.engine.pool_info.clone()
                },
                prev_file_access: rust.engine.file_access.clone(),
                last_pool_quotas: rust.last_pool_quotas,
                env_config_gen: rust.env_config_gen,
                ctx: RefreshContext {
                    elapsed_wall_ns,
                    track_cpu_and_net: true,
                    force_reparse: thresholds_changed,
                    disk_warn_percent: warn,
                    disk_critical_percent: crit,
                    downloading_os_id: rust.downloading_os_id.to_string(),
                    compute_pool_quotas: true,
                },
            }
        };

        std::thread::spawn(move || {
            let runner = SystemRunner;
            let result =
                Engine::fetch_snapshot_with_options(&runner, include_catalog, include_pool)
                    .map_err(|e| e.to_string())
                    .and_then(|snap| parse_refresh_in_worker(snap, worker_input));

            let in_flight_cb = Arc::clone(&in_flight);
            if qt_thread
                .queue(move |mut backend| {
                    in_flight_cb.store(false, Ordering::SeqCst);
                    match result {
                        Ok(output) => backend.as_mut().apply_worker_refresh_output(output),
                        Err(_) => {
                            let pending = backend.as_mut().rust_mut().pending_refresh.take();
                            if let Some((cat, pool)) = pending {
                                backend.as_mut().refresh_with_options(cat, pool);
                            }
                        }
                    }
                })
                .is_err()
            {
                in_flight.store(false, Ordering::SeqCst);
            }
        });
    }

    pub(crate) fn apply_worker_refresh_output(
        mut self: Pin<&mut Self>,
        mut output: WorkerRefreshOutput,
    ) {
        let (envs_changed, selected_clamped, pool_changed, catalog_changed, pending_sizes) = {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.counter_state = output.counter_state;
            if output.include_pool {
                rust.last_pool_refresh_at = Some(Instant::now());
            }

            let quotas_changed = output.pool_quotas != rust.last_pool_quotas;
            let pool_changed = output.delta.pool_changed
                || (output.include_pool && (quotas_changed || rust.pool_info.is_empty()))
                || (quotas_changed && output.pool_json.is_some());
            if pool_changed {
                rust.engine.pool_info = output.pool_info;
                rust.pool_info = match &output.pool_json {
                    Some(serde_json::Value::Object(map)) => json_object_to_qvariant_map(map),
                    _ => QMapQStringQVariant::default(),
                };
                rust.last_pool_quotas = output.pool_quotas;
            } else if output.include_pool {
                rust.engine.pool_info = output.pool_info;
            }

            if output.include_catalog {
                rust.last_images_json = output.images_json;
            }
            let catalog_changed = output.delta.catalog_changed
                || (output.include_catalog && rust.os_catalog.is_empty());
            if catalog_changed {
                rust.engine.os_catalog = output.os_catalog;
                rust.os_catalog = serialize_to_qvariant_list(&rust.engine.os_catalog);
            }

            let mut pending_sizes: Vec<(String, String, bool, Vec<String>)> = Vec::new();
            let mut envs_changed = false;
            let mut selected_clamped = false;

            if output.delta.list_parsed {
                rust.last_list_hash = output.snapshot_list_hash;
                let mut envs_vec = output.environments.to_vec();
                let mut cache_modified_env = false;

                if output.env_config_gen != rust.env_config_gen {
                    let auto_snap_default = rust.engine.config.auto_snapshot_enabled;
                    let auto_stop_default = rust.engine.config.watchdog_auto_stop;
                    let record_default = rust.engine.config.record_sessions;
                    for env in &mut envs_vec {
                        if env.auto_snapshot_override.is_empty() {
                            env.auto_snapshot_enabled = auto_snap_default;
                        }
                        if env.auto_stop_override.is_empty() {
                            env.auto_stop_on_critical = auto_stop_default;
                        }
                        if env.record_override.is_empty() || env.record_override == "inherit" {
                            env.record_enabled = record_default;
                        }
                    }
                    cache_modified_env = true;
                }

                let mut cache = std::mem::take(&mut rust.snapshot_size_cache);
                let live_names: Vec<&str> = envs_vec.iter().map(|e| e.name.as_str()).collect();
                cache.prune_deleted_envs(&live_names);

                for env in &mut envs_vec {
                    if !env.snapshot_sizes.is_empty() {
                        cache.insert_incus(&env.name, env.snapshot_sizes.clone());
                    }
                    cache.prune_removed(&env.name, &env.snapshots, &env.workspace_snapshots);
                    if cache.apply_to_environment(env) {
                        cache_modified_env = true;
                    }

                    let missing_incus = !cache
                        .missing_incus_snapshots(&env.name, &env.snapshots)
                        .is_empty();
                    let missing_ws =
                        cache.missing_workspace_snapshots(&env.name, &env.workspace_snapshots);
                    if missing_incus || !missing_ws.is_empty() {
                        pending_sizes.push((
                            env.name.clone(),
                            env.workspace_snapshot_dir.clone(),
                            missing_incus,
                            missing_ws,
                        ));
                    }
                }
                rust.snapshot_size_cache = cache;

                if cache_modified_env {
                    output.env_values =
                        build_environment_json_values(&envs_vec, &output.file_access);
                }
                let model_hash = if cache_modified_env {
                    hash_json_values(&output.env_values)
                } else {
                    output.env_values_hash
                };
                let changed = model_hash != rust.last_environments_hash
                    || (rust.environments.is_empty() && !envs_vec.is_empty());

                if changed {
                    let final_arc: Arc<[Environment]> = if cache_modified_env {
                        Arc::from(envs_vec.clone())
                    } else {
                        Arc::clone(&output.environments)
                    };
                    let has_running = final_arc.iter().any(|e| e.running && !e.is_template);
                    rust.has_running_envs.store(has_running, Ordering::Relaxed);
                    rust.engine.environments = envs_vec;
                    rust.engine.templates = output.templates;
                    rust.engine.file_access = output.file_access;
                    rust.environments_arc = final_arc;
                    rust.environments = json_array_to_qvariant_list(&output.env_values);
                    rust.count = rust.engine.environments.len() as i32;
                    rust.last_environments_hash = model_hash;

                    let max_idx = (rust.engine.environments.len() as i32 - 1).max(0);
                    if rust.selected_index >= rust.engine.environments.len() as i32 {
                        rust.selected_index = max_idx;
                    }
                    rust.rebuild_templates_cache();
                    rust.rebuild_selected_environment_cache();
                    envs_changed = true;
                }
            } else if !output.env_values_unchanged && output.file_access != rust.engine.file_access
            {
                // Preserve any concurrent main-thread updates (snapshot sizes, config
                // propagations, secret removals) on `rust.engine.environments` and only
                // re-attach the newly observed file access state.
                rust.engine.file_access = output.file_access;
                let env_values = build_environment_json_values(
                    &rust.engine.environments,
                    &rust.engine.file_access,
                );
                let model_hash = hash_json_values(&env_values);
                if model_hash != rust.last_environments_hash {
                    rust.environments = json_array_to_qvariant_list(&env_values);
                    rust.last_environments_hash = model_hash;
                    rust.rebuild_selected_environment_cache();
                    envs_changed = true;
                }
            }

            if !envs_changed {
                let max_idx = (rust.engine.environments.len() as i32 - 1).max(0);
                if rust.selected_index >= rust.engine.environments.len() as i32 {
                    rust.selected_index = max_idx;
                    rust.rebuild_selected_environment_cache();
                    selected_clamped = true;
                }
            }

            (
                envs_changed,
                selected_clamped,
                pool_changed,
                catalog_changed,
                pending_sizes,
            )
        };

        if pool_changed {
            self.as_mut().pool_info_changed();
        }
        if envs_changed {
            self.as_mut().environments_changed();
            self.as_mut().count_changed();
            self.as_mut().selected_index_changed();
        } else if selected_clamped {
            self.as_mut().selected_index_changed();
        }
        self.as_mut().schedule_snapshot_size_worker(pending_sizes);
        self.as_mut().evaluate_all_watchdogs();
        self.as_mut().trigger_idle_auto_stops_check();
        if catalog_changed {
            self.as_mut().os_catalog_changed();
        }
        let pending = self.as_mut().rust_mut().pending_refresh.take();
        if let Some((cat, pool)) = pending {
            self.as_mut().refresh_with_options(cat, pool);
        }
    }

    fn schedule_snapshot_size_worker(
        mut self: Pin<&mut Self>,
        pending: Vec<(String, String, bool, Vec<String>)>,
    ) {
        if pending.is_empty() || self.demo_mode {
            return;
        }
        let in_flight = Arc::clone(&self.snapshot_sizes_in_flight);
        if in_flight.swap(true, Ordering::AcqRel) {
            self.as_mut().rust_mut().pending_snapshot_sizes = true;
            return;
        }
        let qt_thread = self.qt_thread();
        let in_flight_thread = Arc::clone(&in_flight);
        std::thread::spawn(move || {
            let mut results = Vec::with_capacity(pending.len());
            for (env_name, snap_dir, need_incus, missing_ws) in pending {
                let incus_sizes = if need_incus {
                    fetch_incus_snapshot_sizes(&env_name)
                } else {
                    Default::default()
                };
                let ws_sizes = if !missing_ws.is_empty() && !snap_dir.is_empty() {
                    compute_workspace_snapshot_sizes(&snap_dir, &missing_ws)
                } else {
                    Default::default()
                };
                if !incus_sizes.is_empty() || !ws_sizes.is_empty() {
                    results.push((env_name, incus_sizes, ws_sizes));
                }
            }
            let in_flight_cb = Arc::clone(&in_flight_thread);
            if qt_thread
                .queue(move |mut backend| {
                    in_flight_cb.store(false, Ordering::Release);
                    let (changed, next_pending) = {
                        let mut rust = backend.as_mut().rust_mut();
                        let mut any_new = false;
                        for (env_name, incus_sizes, ws_sizes) in results {
                            if rust
                                .snapshot_size_cache
                                .insert_incus(&env_name, incus_sizes)
                            {
                                any_new = true;
                            }
                            if rust
                                .snapshot_size_cache
                                .insert_workspace(&env_name, ws_sizes)
                            {
                                any_new = true;
                            }
                        }
                        if any_new {
                            let cache = rust.snapshot_size_cache.clone();
                            for env in &mut rust.engine.environments {
                                cache.apply_to_environment(env);
                            }
                            let state_dir = rust.engine.paths.state_dir.clone();
                            let fa_inputs: Vec<FileAccessEnvInput> = rust
                                .engine
                                .environments
                                .iter()
                                .map(|e| FileAccessEnvInput {
                                    name: e.name.clone(),
                                    running: e.running,
                                })
                                .collect();
                            let fa_result = apply_file_access_state(&state_dir, &fa_inputs);
                            let env_values = build_environment_json_values(
                                &rust.engine.environments,
                                &fa_result,
                            );
                            rust.engine.file_access = fa_result;
                            rust.last_environments_hash = hash_json_values(&env_values);
                            rust.environments_arc = Arc::from(rust.engine.environments.clone());
                            rust.environments = json_array_to_qvariant_list(&env_values);
                            rust.rebuild_templates_cache();
                            rust.rebuild_selected_environment_cache();
                        }
                        let mut follow_up = Vec::new();
                        if rust.pending_snapshot_sizes {
                            rust.pending_snapshot_sizes = false;
                            for env in &rust.engine.environments {
                                let missing_incus = !rust
                                    .snapshot_size_cache
                                    .missing_incus_snapshots(&env.name, &env.snapshots)
                                    .is_empty();
                                let missing_ws =
                                    rust.snapshot_size_cache.missing_workspace_snapshots(
                                        &env.name,
                                        &env.workspace_snapshots,
                                    );
                                if missing_incus || !missing_ws.is_empty() {
                                    follow_up.push((
                                        env.name.clone(),
                                        env.workspace_snapshot_dir.clone(),
                                        missing_incus,
                                        missing_ws,
                                    ));
                                }
                            }
                        }
                        (any_new, follow_up)
                    };
                    if changed {
                        backend.as_mut().environments_changed();
                        backend.as_mut().selected_index_changed();
                    }
                    if !next_pending.is_empty() {
                        backend.as_mut().schedule_snapshot_size_worker(next_pending);
                    }
                })
                .is_err()
            {
                in_flight_thread.store(false, Ordering::Release);
            }
        });
    }

    pub fn refresh_backups(mut self: Pin<&mut Self>) {
        if self.demo_mode {
            return;
        }
        let dir = aibox_core::backup::backups_dir_path(&self.engine.config.os_storage_path);
        let archives = scan_backup_archives(&dir);
        let q_archives = json_array_to_qvariant_list(&archives);
        {
            let mut rust = self.as_mut().rust_mut();
            rust.backup_archives = q_archives;
        }
        self.as_mut().backup_archives_changed();
    }

    pub fn refresh_recorded_sessions(mut self: Pin<&mut Self>) {
        if self.demo_mode {
            return;
        }
        let dir = aibox_core::session::sessions_dir_path(&self.engine.paths.state_dir);
        let sessions = scan_recorded_sessions(&dir);
        let q_sessions = json_array_to_qvariant_list(&sessions);
        {
            let mut rust = self.as_mut().rust_mut();
            rust.recorded_sessions = q_sessions;
        }
        self.as_mut().recorded_sessions_changed();
    }

    pub fn refresh_stale_report(mut self: Pin<&mut Self>) {
        let workspace_base = self.engine.config.workspace_base.clone();
        let backups_dir = aibox_core::backup::backups_dir_path(&self.engine.config.os_storage_path);
        let stale_days = self.stale_days;

        if self.demo_mode {
            let report = build_stale_report(
                &self.demo_environments,
                &workspace_base,
                &backups_dir,
                stale_days,
                0,
            );
            let q_report = match report {
                serde_json::Value::Object(map) => json_object_to_qvariant_map(&map),
                _ => QMapQStringQVariant::default(),
            };
            {
                let mut rust = self.as_mut().rust_mut();
                rust.stale_report = q_report;
            }
            self.as_mut().stale_report_changed();
            return;
        }

        let gen = self.stale_report_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let gen_ref = Arc::clone(&self.stale_report_generation);
        let envs_arc = Arc::clone(&self.environments_arc);
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let envs_val = serde_json::to_value(&*envs_arc).unwrap_or_default();
            let envs_arr = envs_val.as_array().cloned().unwrap_or_default();
            let report =
                build_stale_report(&envs_arr, &workspace_base, &backups_dir, stale_days, 0);
            let _ = qt_thread.queue(move |mut backend| {
                if gen_ref.load(Ordering::SeqCst) != gen {
                    return;
                }
                let q_report = match report {
                    serde_json::Value::Object(map) => json_object_to_qvariant_map(&map),
                    _ => QMapQStringQVariant::default(),
                };
                {
                    let mut rust = backend.as_mut().rust_mut();
                    rust.stale_report = q_report;
                }
                backend.as_mut().stale_report_changed();
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_cadence_respects_30s_interval_and_explicit_override() {
        let now = Instant::now();
        assert!(should_include_pool(None, None, now));
        assert!(!should_include_pool(
            None,
            Some(now - Duration::from_secs(10)),
            now
        ));
        assert!(should_include_pool(
            None,
            Some(now - Duration::from_secs(31)),
            now
        ));
        assert!(should_include_pool(
            Some(true),
            Some(now - Duration::from_secs(1)),
            now
        ));
        assert!(!should_include_pool(Some(false), None, now));
    }

    #[test]
    fn worker_parse_produces_identical_delta_and_gates_unchanged_hash() {
        let sample = br#"[
            {
                "name": "dev-box",
                "status": "Running",
                "type": "container",
                "profiles": ["default"],
                "config": {"limits.cpu": "2", "limits.memory": "4GiB"},
                "state": {"status": "Running", "cpu": {"usage": 1000}, "memory": {"usage": 1024}}
            }
        ]"#;
        let tmp = std::env::temp_dir().join(format!(
            "aibox-refresh-unit-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&tmp);
        let paths = ResolvedPaths::resolve(Some(tmp.to_string_lossy().as_ref()), true, ".");
        let config = Config::new_defaults(&paths.workspace_base, &paths.os_storage_path);

        let snap1 = RawSnapshot {
            list_json: sample.to_vec(),
            images_json: Vec::new(),
            storage_info_text: String::new(),
            pool_size_text: String::new(),
            include_catalog: false,
            include_pool: false,
        };
        let input1 = WorkerRefreshInput {
            paths: paths.clone(),
            config: config.clone(),
            counter_state: CounterState::new(),
            prev_environments: Arc::from(Vec::new()),
            prev_templates: Vec::new(),
            prev_os_catalog: Vec::new(),
            prev_pool_info: StoragePoolInfo::default(),
            prev_file_access: ApplyFileAccessResult::default(),
            last_pool_quotas: (0, 0),
            env_config_gen: 0,
            ctx: RefreshContext {
                track_cpu_and_net: true,
                ..RefreshContext::default()
            },
        };
        let out1 = parse_refresh_in_worker(snap1, input1).expect("first parse");
        assert!(out1.delta.list_parsed);
        assert!(out1.delta.environments_changed);
        assert_eq!(out1.environments.len(), 1);
        let hash1 = hash_json_values(&out1.env_values);

        let snap2 = RawSnapshot {
            list_json: sample.to_vec(),
            images_json: Vec::new(),
            storage_info_text: String::new(),
            pool_size_text: String::new(),
            include_catalog: false,
            include_pool: false,
        };
        let input2 = WorkerRefreshInput {
            paths,
            config,
            counter_state: out1.counter_state,
            prev_environments: Arc::clone(&out1.environments),
            prev_templates: out1.templates,
            prev_os_catalog: out1.os_catalog,
            prev_pool_info: out1.pool_info,
            prev_file_access: out1.file_access.clone(),
            last_pool_quotas: out1.pool_quotas,
            env_config_gen: 0,
            ctx: RefreshContext {
                track_cpu_and_net: true,
                ..RefreshContext::default()
            },
        };
        let out2 = parse_refresh_in_worker(snap2, input2).expect("second parse");
        assert!(!out2.delta.list_parsed);
        assert!(!out2.delta.environments_changed);
        assert_ne!(hash1, 0);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn unchanged_list_with_pool_refresh_preserves_allocated_quotas() {
        let sample = br#"[
            {
                "name": "quota-box",
                "status": "Stopped",
                "type": "container",
                "expanded_devices": {
                    "root": { "pool": "default", "size": "20GiB" }
                }
            }
        ]"#;
        let tmp = std::env::temp_dir().join(format!(
            "aibox-refresh-pool-unit-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&tmp);
        let paths = ResolvedPaths::resolve(Some(tmp.to_string_lossy().as_ref()), true, ".");
        let config = Config::new_defaults(&paths.workspace_base, &paths.os_storage_path);

        let pool_yaml = "info:\n  driver: btrfs\n  name: default\n  space used: \"1000\"\n  total space: \"100000000000\"\n".to_string();

        let snap1 = RawSnapshot {
            list_json: sample.to_vec(),
            images_json: Vec::new(),
            storage_info_text: pool_yaml.clone(),
            pool_size_text: "100GiB".to_string(),
            include_catalog: false,
            include_pool: true,
        };
        let input1 = WorkerRefreshInput {
            paths: paths.clone(),
            config: config.clone(),
            counter_state: CounterState::new(),
            prev_environments: Arc::from(Vec::new()),
            prev_templates: Vec::new(),
            prev_os_catalog: Vec::new(),
            prev_pool_info: StoragePoolInfo::default(),
            prev_file_access: ApplyFileAccessResult::default(),
            last_pool_quotas: (0, 0),
            env_config_gen: 0,
            ctx: RefreshContext {
                track_cpu_and_net: true,
                ..RefreshContext::default()
            },
        };
        let out1 = parse_refresh_in_worker(snap1, input1).expect("first parse");
        let expected_bytes = 20 * 1024 * 1024 * 1024_i64;
        assert_eq!(out1.pool_quotas, (expected_bytes, 0));
        assert_eq!(
            out1.pool_json.as_ref().unwrap()["allocatedBytes"],
            expected_bytes
        );

        // Second refresh: identical list_json + include_pool == true must NOT zero out quotas.
        let snap2 = RawSnapshot {
            list_json: sample.to_vec(),
            images_json: Vec::new(),
            storage_info_text: pool_yaml,
            pool_size_text: "100GiB".to_string(),
            include_catalog: false,
            include_pool: true,
        };
        let input2 = WorkerRefreshInput {
            paths,
            config,
            counter_state: out1.counter_state,
            prev_environments: Arc::clone(&out1.environments),
            prev_templates: out1.templates,
            prev_os_catalog: out1.os_catalog,
            prev_pool_info: out1.pool_info,
            prev_file_access: out1.file_access.clone(),
            last_pool_quotas: out1.pool_quotas,
            env_config_gen: 0,
            ctx: RefreshContext {
                track_cpu_and_net: true,
                ..RefreshContext::default()
            },
        };
        let out2 = parse_refresh_in_worker(snap2, input2).expect("second parse");
        assert!(!out2.delta.list_parsed);
        assert_eq!(out2.pool_quotas, (expected_bytes, 0));
        assert_eq!(
            out2.pool_json.as_ref().unwrap()["allocatedBytes"],
            expected_bytes
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn hash_json_values_distinguishes_types_and_matches_identical() {
        let a = vec![serde_json::json!({"x": 1, "y": "true"})];
        let b = vec![serde_json::json!({"x": 1, "y": true})];
        let c = vec![serde_json::json!({"x": 1, "y": "true"})];
        assert_eq!(hash_json_values(&a), hash_json_values(&c));
        assert_ne!(hash_json_values(&a), hash_json_values(&b));
    }
}
