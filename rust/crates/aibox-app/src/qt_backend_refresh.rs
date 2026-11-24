use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QMap, QMapPair_QString_QVariant};

use aibox_core::backup::{build_stale_report, scan_backup_archives};
use aibox_core::config::model::Config;
use aibox_core::engine::{Engine, RawSnapshot, RefreshContext, RefreshDelta, ResolvedPaths};
use aibox_core::health::pool::StoragePoolInfo;
use aibox_core::incus::catalog::OsCatalogEntry;
use aibox_core::incus::list::{CounterState, Environment};
use aibox_core::incus::monitor::parse_lifecycle_event;
use aibox_core::process::SystemRunner;
use aibox_core::session::scan_recorded_sessions;
use aibox_core::snapshot::{compute_workspace_snapshot_sizes, fetch_incus_snapshot_sizes};
use aibox_core::workspace::file_access::{
    apply_file_access_state, attach_file_access_fields, ApplyFileAccessResult, FileAccessEnvInput,
};

use crate::qt_backend::qobject;
use crate::qt_convert::{
    json_array_to_qvariant_list, json_object_to_qvariant_map, serialize_to_qvariant_list,
    value_to_qvariant_map,
};

type QMapQStringQVariant = QMap<QMapPair_QString_QVariant>;

pub(crate) const POOL_REFRESH_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScheduledJobKind {
    Refresh { include_catalog: bool },
    ClearStatus,
}

struct ScheduledEntry {
    run_at: Instant,
    job: ScheduledJobKind,
}

/// Single background timer/debounce worker replacing ad-hoc `thread::spawn` + `sleep` calls.
#[derive(Default)]
pub(crate) struct SchedulerState {
    queue: Mutex<Vec<ScheduledEntry>>,
    cv: Condvar,
    started: AtomicBool,
}

impl SchedulerState {
    pub(crate) fn notify_all(&self) {
        self.cv.notify_all();
    }

    pub(crate) fn schedule(
        self: &Arc<Self>,
        qt_thread: cxx_qt::CxxQtThread<qobject::Backend>,
        stop_flag: Arc<AtomicBool>,
        delay: Duration,
        job: ScheduledJobKind,
    ) {
        let run_at = Instant::now() + delay;
        if let Ok(mut guard) = self.queue.lock() {
            match job {
                ScheduledJobKind::Refresh { include_catalog } => {
                    let mut merged_catalog = include_catalog;
                    guard.retain(|entry| {
                        if let ScheduledJobKind::Refresh {
                            include_catalog: prev,
                        } = entry.job
                        {
                            merged_catalog |= prev;
                            false
                        } else {
                            true
                        }
                    });
                    guard.push(ScheduledEntry {
                        run_at,
                        job: ScheduledJobKind::Refresh {
                            include_catalog: merged_catalog,
                        },
                    });
                }
                ScheduledJobKind::ClearStatus => {
                    guard.retain(|entry| !matches!(entry.job, ScheduledJobKind::ClearStatus));
                    guard.push(ScheduledEntry { run_at, job });
                }
            }
            self.cv.notify_all();
        }

        if self
            .started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            let state = Arc::clone(self);
            std::thread::spawn(move || loop {
                if stop_flag.load(Ordering::Relaxed) {
                    return;
                }

                let mut due: Vec<ScheduledJobKind> = Vec::new();
                {
                    let Ok(mut guard) = state.queue.lock() else {
                        return;
                    };
                    loop {
                        if stop_flag.load(Ordering::Relaxed) {
                            return;
                        }
                        let now = Instant::now();
                        let next_at = guard.iter().map(|e| e.run_at).min();
                        match next_at {
                            None => {
                                guard = match state.cv.wait(guard) {
                                    Ok(g) => g,
                                    Err(_) => return,
                                };
                            }
                            Some(t) if t <= now => {
                                let mut remaining = Vec::with_capacity(guard.len());
                                for entry in guard.drain(..) {
                                    if entry.run_at <= now {
                                        due.push(entry.job);
                                    } else {
                                        remaining.push(entry);
                                    }
                                }
                                *guard = remaining;
                                break;
                            }
                            Some(t) => {
                                let wait_dur = t.saturating_duration_since(now);
                                let (g, _) = match state.cv.wait_timeout(guard, wait_dur) {
                                    Ok(res) => res,
                                    Err(_) => return,
                                };
                                guard = g;
                            }
                        }
                    }
                }

                for job in due {
                    match job {
                        ScheduledJobKind::Refresh { include_catalog } => {
                            let _ = qt_thread.queue(move |mut backend| {
                                backend.as_mut().refresh_with_options(include_catalog, None);
                            });
                        }
                        ScheduledJobKind::ClearStatus => {
                            let _ = qt_thread.queue(|mut backend| {
                                if !*backend.busy() {
                                    backend.as_mut().set_status_message("");
                                }
                            });
                        }
                    }
                }
            });
        }
    }
}

pub(crate) fn adaptive_poll_interval(
    window_active: bool,
    has_running_envs: bool,
    monitor_active: bool,
) -> Duration {
    if !window_active || !has_running_envs {
        Duration::from_secs(20)
    } else if monitor_active {
        Duration::from_secs(8)
    } else {
        Duration::from_secs(4)
    }
}

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

pub(crate) fn hash_json_values(values: &[serde_json::Value]) -> u64 {
    let mut hasher = DefaultHasher::new();
    values.len().hash(&mut hasher);
    for v in values {
        v.to_string().hash(&mut hasher);
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
    pub(crate) ctx: RefreshContext,
    pub(crate) last_list_hash: u64,
}

pub(crate) struct WorkerRefreshOutput {
    pub(crate) snapshot_list_hash: u64,
    pub(crate) list_json: Vec<u8>,
    pub(crate) images_json: Vec<u8>,
    pub(crate) include_catalog: bool,
    pub(crate) include_pool: bool,
    pub(crate) delta: RefreshDelta,
    pub(crate) environments: Arc<[Environment]>,
    pub(crate) templates: Vec<Environment>,
    pub(crate) os_catalog: Vec<OsCatalogEntry>,
    pub(crate) pool_info: StoragePoolInfo,
    pub(crate) counter_state: CounterState,
    pub(crate) file_access: ApplyFileAccessResult,
    pub(crate) env_values: Vec<serde_json::Value>,
    pub(crate) pool_json: Option<serde_json::Value>,
}

/// Parses `RawSnapshot` and pre-serializes JSON payloads on the worker thread so the Qt thread
/// only swaps state, applies snapshot size cache, and converts to `QVariant`.
pub(crate) fn parse_refresh_in_worker(
    snapshot: RawSnapshot,
    mut input: WorkerRefreshInput,
) -> Result<WorkerRefreshOutput, String> {
    let snapshot_list_hash = hash_bytes(&snapshot.list_json);
    let list_unchanged = input.last_list_hash != 0 && input.last_list_hash == snapshot_list_hash;
    input.ctx.compute_pool_quotas = !list_unchanged;

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

    let delta = temp_engine
        .apply_snapshot(&snapshot, &input.ctx)
        .map_err(|e| e.to_string())?;

    let env_values =
        build_environment_json_values(&temp_engine.environments, &temp_engine.file_access);

    let pool_json = if snapshot.include_pool {
        let pool_json_str = temp_engine
            .pool_info
            .to_json(delta.allocated_bytes, delta.unbounded_count);
        Some(serde_json::from_str(&pool_json_str).unwrap_or(serde_json::Value::Null))
    } else {
        None
    };

    let environments: Arc<[Environment]> = Arc::from(temp_engine.environments);

    Ok(WorkerRefreshOutput {
        snapshot_list_hash,
        list_json: if delta.list_parsed {
            snapshot.list_json
        } else {
            Vec::new()
        },
        images_json: snapshot.images_json,
        include_catalog: snapshot.include_catalog,
        include_pool: snapshot.include_pool,
        delta,
        environments,
        templates: temp_engine.templates,
        os_catalog: temp_engine.os_catalog,
        pool_info: temp_engine.pool_info,
        counter_state: temp_engine.counter_state,
        file_access: temp_engine.file_access,
        env_values,
        pool_json,
    })
}

impl qobject::Backend {
    /// The inspector reads root file access state from this map, so the live
    /// `file-access` keys must ride along with the selected environment.
    pub fn selected_environment(&self) -> QMapQStringQVariant {
        if self.selected_index >= 0 {
            if self.demo_mode {
                if let Some(serde_json::Value::Object(map)) =
                    self.demo_environments.get(self.selected_index as usize)
                {
                    let mut val = serde_json::Value::Object(map.clone());
                    attach_file_access_fields(&mut val, None);
                    return value_to_qvariant_map(&val);
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
                return value_to_qvariant_map(&val);
            }
        }
        QMapQStringQVariant::default()
    }

    pub fn set_window_active(mut self: Pin<&mut Self>, active: bool) {
        let was_active = self.window_active.swap(active, Ordering::Relaxed);
        if active && !was_active {
            self.poll_wake.1.notify_all();
            if self.poll_started && !self.demo_mode {
                let stale = self
                    .last_refresh_at
                    .map(|t| t.elapsed() >= Duration::from_secs(2))
                    .unwrap_or(true);
                if stale {
                    self.as_mut().refresh_with_options(false, None);
                }
            }
        }
    }

    pub(crate) fn schedule_refresh(self: Pin<&mut Self>, delay: Duration, include_catalog: bool) {
        self.scheduler.schedule(
            self.qt_thread(),
            Arc::clone(&self.stop_flag),
            delay,
            ScheduledJobKind::Refresh { include_catalog },
        );
    }

    pub(crate) fn schedule_status_clear(self: Pin<&mut Self>, delay: Duration) {
        self.scheduler.schedule(
            self.qt_thread(),
            Arc::clone(&self.stop_flag),
            delay,
            ScheduledJobKind::ClearStatus,
        );
    }

    pub(crate) fn sync_environments_from_engine(mut self: Pin<&mut Self>) {
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
        }
        self.as_mut().environments_changed();
        self.as_mut().count_changed();
        self.as_mut().selected_index_changed();
    }

    pub(crate) fn start_background_workers(mut self: Pin<&mut Self>) {
        if self.poll_started {
            return;
        }
        {
            let mut rust = self.as_mut().rust_mut();
            rust.poll_started = true;
        }

        // Adaptive metric refresh poll thread (4s active+running, 8s with monitor, 20s idle/hidden)
        let qt_thread_poll = self.qt_thread();
        let stop_poll = Arc::clone(&self.stop_flag);
        let monitor_active_poll = Arc::clone(&self.monitor_active);
        let window_active_poll = Arc::clone(&self.window_active);
        let has_running_poll = Arc::clone(&self.has_running_envs);
        let poll_wake = Arc::clone(&self.poll_wake);
        std::thread::spawn(move || {
            let (lock, cv) = &*poll_wake;
            while !stop_poll.load(Ordering::Relaxed) {
                let interval = adaptive_poll_interval(
                    window_active_poll.load(Ordering::Relaxed),
                    has_running_poll.load(Ordering::Relaxed),
                    monitor_active_poll.load(Ordering::Relaxed),
                );
                if let Ok(guard) = lock.lock() {
                    let _ = cv.wait_timeout(guard, interval);
                }
                if stop_poll.load(Ordering::Relaxed) {
                    return;
                }
                let _ = qt_thread_poll.queue(|mut backend| {
                    if !backend.busy {
                        backend.as_mut().refresh_with_options(false, None);
                    }
                });
            }
        });

        // Instant lifecycle event stream via `incus monitor --type=lifecycle --format=json`
        let qt_thread_mon = self.qt_thread();
        let stop_mon = Arc::clone(&self.stop_flag);
        let monitor_active_mon = Arc::clone(&self.monitor_active);
        let monitor_child_slot = Arc::clone(&self.monitor_child);
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            use std::process::Stdio;

            while !stop_mon.load(Ordering::Relaxed) {
                let child_res = Command::new("incus")
                    .args(["monitor", "--type=lifecycle", "--format=json"])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn();

                let Ok(mut child) = child_res else {
                    monitor_active_mon.store(false, Ordering::Relaxed);
                    for _ in 0..25 {
                        if stop_mon.load(Ordering::Relaxed) {
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(200));
                    }
                    continue;
                };

                let stdout = child.stdout.take();
                if let Ok(mut guard) = monitor_child_slot.lock() {
                    *guard = Some(child);
                }
                monitor_active_mon.store(true, Ordering::Relaxed);

                if let Some(out) = stdout {
                    let reader = BufReader::new(out);
                    for line in reader.lines() {
                        if stop_mon.load(Ordering::Relaxed) {
                            break;
                        }
                        let Ok(line_str) = line else {
                            break;
                        };
                        if let Some(ev) = parse_lifecycle_event(&line_str) {
                            if ev.should_trigger_refresh() {
                                let include_catalog = ev.invalidates_catalog();
                                let _ = qt_thread_mon.queue(move |mut backend| {
                                    if !backend.busy {
                                        backend
                                            .as_mut()
                                            .refresh_with_options(include_catalog, None);
                                    }
                                });
                            }
                        }
                    }
                }

                monitor_active_mon.store(false, Ordering::Relaxed);
                if let Ok(mut guard) = monitor_child_slot.lock() {
                    if let Some(mut c) = guard.take() {
                        let _ = c.kill();
                        let _ = c.wait();
                    }
                }

                if stop_mon.load(Ordering::Relaxed) {
                    return;
                }
                for _ in 0..25 {
                    if stop_mon.load(Ordering::Relaxed) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
        });

        // inotify task watch on state_dir with 500ms active / 5s idle fallback
        let qt_thread_watch = self.qt_thread();
        let stop_watch = Arc::clone(&self.stop_flag);
        let task_watch_active = Arc::clone(&self.task_watch_active);
        let watch_state_dir = self.engine.paths.state_dir.clone();
        std::thread::spawn(move || {
            use notify::{RecursiveMode, Watcher};
            use std::sync::mpsc;

            let _ = std::fs::create_dir_all(&watch_state_dir);
            let (fs_tx, fs_rx) = mpsc::channel::<()>();
            let mut watcher =
                notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                    if let Ok(ev) = res {
                        let touches_task = ev.paths.iter().any(|p| {
                            p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                                matches!(n, "task.log" | "task.pid" | "result.json")
                            })
                        });
                        if touches_task {
                            let _ = fs_tx.send(());
                        }
                    }
                })
                .ok();
            if let Some(ref mut w) = watcher {
                let _ = w.watch(
                    std::path::Path::new(&watch_state_dir),
                    RecursiveMode::NonRecursive,
                );
            }

            while !stop_watch.load(Ordering::Relaxed) {
                let timeout = if task_watch_active.load(Ordering::Relaxed) {
                    Duration::from_millis(500)
                } else {
                    Duration::from_secs(5)
                };
                if fs_rx.recv_timeout(timeout).is_ok() {
                    while fs_rx.try_recv().is_ok() {}
                }
                if stop_watch.load(Ordering::Relaxed) {
                    return;
                }
                if task_watch_active.load(Ordering::Relaxed) {
                    let _ = qt_thread_watch.queue(|mut backend| {
                        backend.as_mut().check_background_task();
                    });
                }
            }
            drop(watcher);
        });
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
                prev_os_catalog: rust.engine.os_catalog.clone(),
                prev_pool_info: rust.engine.pool_info.clone(),
                ctx: RefreshContext {
                    elapsed_wall_ns,
                    track_cpu_and_net: true,
                    force_reparse: thresholds_changed,
                    disk_warn_percent: warn,
                    disk_critical_percent: crit,
                    downloading_os_id: rust.downloading_os_id.to_string(),
                    compute_pool_quotas: true,
                },
                last_list_hash: rust.last_list_hash,
            }
        };

        std::thread::spawn(move || {
            let runner = SystemRunner;
            let result =
                Engine::fetch_snapshot_with_options(&runner, include_catalog, include_pool)
                    .map_err(|e| e.to_string())
                    .and_then(|snap| parse_refresh_in_worker(snap, worker_input));
            in_flight.store(false, Ordering::SeqCst);

            let Ok(output) = result else {
                return;
            };
            let _ = qt_thread.queue(move |mut backend| {
                backend.as_mut().apply_worker_refresh_output(output);
            });
        });
    }

    pub(crate) fn apply_worker_refresh_output(
        mut self: Pin<&mut Self>,
        mut output: WorkerRefreshOutput,
    ) {
        let (envs_changed, pool_changed, catalog_changed, pending_sizes) = {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.counter_state = output.counter_state;
            if output.include_pool {
                rust.last_pool_refresh_at = Some(Instant::now());
            }

            let list_unchanged =
                rust.last_list_hash != 0 && rust.last_list_hash == output.snapshot_list_hash;
            let (allocated_bytes, unbounded_count) = if list_unchanged {
                rust.last_pool_quotas
            } else {
                (output.delta.allocated_bytes, output.delta.unbounded_count)
            };
            let quotas_changed = (allocated_bytes, unbounded_count) != rust.last_pool_quotas;
            let pool_changed = output.delta.pool_changed
                || (output.include_pool && (quotas_changed || rust.pool_info.is_empty()));
            if pool_changed {
                rust.engine.pool_info = output.pool_info;
                rust.pool_info = match &output.pool_json {
                    Some(serde_json::Value::Object(map)) => json_object_to_qvariant_map(map),
                    _ => QMapQStringQVariant::default(),
                };
                rust.last_pool_quotas = (allocated_bytes, unbounded_count);
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
            if output.delta.list_parsed {
                rust.last_list_hash = output.snapshot_list_hash;
                rust.last_list_json = output.list_json;
            }
            let mut envs_vec = output.environments.to_vec();
            let mut cache = std::mem::take(&mut rust.snapshot_size_cache);
            let mut cache_modified_env = false;
            if output.delta.list_parsed {
                for env in &mut envs_vec {
                    if !env.snapshot_sizes.is_empty() {
                        cache.insert_incus(&env.name, env.snapshot_sizes.clone());
                    }
                    cache.prune_removed(&env.name, &env.snapshots, &env.workspace_snapshots);
                    let before_sizes = env.snapshot_sizes.clone();
                    let before_ws_sizes = env.workspace_snapshot_sizes.clone();
                    cache.apply_to_environment(env);
                    if env.snapshot_sizes != before_sizes
                        || env.workspace_snapshot_sizes != before_ws_sizes
                    {
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
            }
            rust.snapshot_size_cache = cache;

            if cache_modified_env {
                output.env_values = build_environment_json_values(&envs_vec, &output.file_access);
            }

            let model_hash = hash_json_values(&output.env_values);
            // Root file access changes do not touch `incus list`, so the model
            // hash must be compared on every refresh, not only when the list
            // JSON changed.
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
            }

            let max_idx = (rust.engine.environments.len() as i32 - 1).max(0);
            if rust.selected_index >= rust.engine.environments.len() as i32 {
                rust.selected_index = max_idx;
            }
            let envs_changed = changed;

            (envs_changed, pool_changed, catalog_changed, pending_sizes)
        };

        if pool_changed {
            self.as_mut().pool_info_changed();
        }
        if envs_changed {
            self.as_mut().environments_changed();
            self.as_mut().count_changed();
            self.as_mut().selected_index_changed();
            if !self.stale_report.is_empty() {
                self.as_mut().refresh_stale_report();
            }
        }
        self.as_mut().schedule_snapshot_size_worker(pending_sizes);
        self.as_mut().evaluate_all_watchdogs();
        self.as_mut().trigger_idle_auto_stops_check();
        if catalog_changed {
            self.as_mut().os_catalog_changed();
        }
    }

    fn schedule_snapshot_size_worker(
        self: Pin<&mut Self>,
        pending: Vec<(String, String, bool, Vec<String>)>,
    ) {
        if pending.is_empty() || self.demo_mode {
            return;
        }
        let in_flight = Arc::clone(&self.snapshot_sizes_in_flight);
        if in_flight.swap(true, Ordering::AcqRel) {
            return;
        }
        let qt_thread = self.qt_thread();
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
            let _ = qt_thread.queue(move |mut backend| {
                in_flight.store(false, Ordering::Release);
                if results.is_empty() {
                    return;
                }
                let changed = {
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
                        let env_values =
                            build_environment_json_values(&rust.engine.environments, &fa_result);
                        rust.last_environments_hash = hash_json_values(&env_values);
                        rust.environments_arc = Arc::from(rust.engine.environments.clone());
                        rust.environments = json_array_to_qvariant_list(&env_values);
                    }
                    any_new
                };
                if changed {
                    backend.as_mut().environments_changed();
                }
            });
        });
    }

    pub fn refresh_backups(mut self: Pin<&mut Self>) {
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
        let envs_arr = if self.demo_mode {
            self.demo_environments.clone()
        } else {
            let envs_val = serde_json::to_value(&self.engine.environments).unwrap_or_default();
            envs_val.as_array().cloned().unwrap_or_default()
        };
        let backups_dir = aibox_core::backup::backups_dir_path(&self.engine.config.os_storage_path);
        let report = build_stale_report(
            &envs_arr,
            &self.engine.config.workspace_base,
            &backups_dir,
            self.stale_days,
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adaptive_poll_interval_scales_by_window_and_running_state() {
        assert_eq!(
            adaptive_poll_interval(true, true, false),
            Duration::from_secs(4)
        );
        assert_eq!(
            adaptive_poll_interval(true, true, true),
            Duration::from_secs(8)
        );
        assert_eq!(
            adaptive_poll_interval(false, true, false),
            Duration::from_secs(20)
        );
        assert_eq!(
            adaptive_poll_interval(true, false, false),
            Duration::from_secs(20)
        );
    }

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
            ctx: RefreshContext {
                track_cpu_and_net: true,
                ..RefreshContext::default()
            },
            last_list_hash: 0,
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
            ctx: RefreshContext {
                track_cpu_and_net: true,
                ..RefreshContext::default()
            },
            last_list_hash: out1.snapshot_list_hash,
        };
        let out2 = parse_refresh_in_worker(snap2, input2).expect("second parse");
        assert!(!out2.delta.list_parsed);
        assert!(!out2.delta.environments_changed);
        assert_ne!(hash1, 0);

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
