use std::pin::Pin;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use cxx_qt::{CxxQtType, Threading};

use aibox_core::incus::monitor::parse_lifecycle_event;

use crate::qt_backend::qobject;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScheduledJobKind {
    Refresh { include_catalog: bool },
    ClearStatus,
}

struct ScheduledEntry {
    run_at: Instant,
    job: ScheduledJobKind,
}

#[derive(Debug, Clone)]
pub(crate) struct TaskWatchConfig {
    pub(crate) state_dir: String,
    pub(crate) active: bool,
    pub(crate) stopped: bool,
    pub(crate) reconfigure_gen: u64,
}

/// Single background timer/debounce worker replacing ad-hoc `thread::spawn` + `sleep` calls.
#[derive(Default)]
pub(crate) struct SchedulerState {
    queue: Mutex<Vec<ScheduledEntry>>,
    cv: Condvar,
    started: AtomicBool,
    stopped: AtomicBool,
}

impl SchedulerState {
    pub(crate) fn stop(&self, stop_flag: &AtomicBool) {
        stop_flag.store(true, Ordering::Release);
        let _guard = self.queue.lock();
        self.stopped.store(true, Ordering::Release);
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
                        if stop_flag.load(Ordering::Acquire)
                            || state.stopped.load(Ordering::Acquire)
                        {
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

impl qobject::Backend {
    pub(crate) fn set_task_watch_active(&self, active: bool) {
        if let Ok(mut w) = self.task_watch_state.0.lock() {
            w.active = active;
            self.task_watch_state.1.notify_all();
        }
    }

    pub(crate) fn reconfigure_task_watch(&self) {
        let next_dir = self.engine.paths.state_dir.clone();
        if let Ok(mut w) = self.task_watch_state.0.lock() {
            if w.state_dir != next_dir {
                w.state_dir = next_dir;
                w.reconfigure_gen = w.reconfigure_gen.wrapping_add(1);
                self.task_watch_state.1.notify_all();
            }
        }
    }

    pub fn set_window_active(mut self: Pin<&mut Self>, active: bool) {
        let was_active = self.window_active.swap(active, Ordering::Relaxed);
        if active && !was_active {
            {
                let _guard = self.poll_wake.0.lock();
                self.poll_wake.1.notify_all();
            }
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
                let stopped_before_store = {
                    let mut guard = monitor_child_slot.lock();
                    if stop_mon.load(Ordering::Acquire) {
                        true
                    } else if let Ok(ref mut g) = guard {
                        **g = Some(child);
                        false
                    } else {
                        true
                    }
                };
                if stopped_before_store {
                    // If `stop_flag` raced before storing `child`, ensure we never leak it.
                    if let Ok(mut guard) = monitor_child_slot.lock() {
                        if let Some(mut c) = guard.take() {
                            let _ = c.kill();
                            let _ = c.wait();
                        }
                    }
                    return;
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

        // inotify task watch on state_dir with wakeable condvar on reconfiguration/stop
        let qt_thread_watch = self.qt_thread();
        let watch_state = Arc::clone(&self.task_watch_state);
        std::thread::spawn(move || {
            use notify::{RecursiveMode, Watcher};
            use std::sync::mpsc;

            let (fs_tx, fs_rx) = mpsc::channel::<()>();
            let wake_pair = Arc::clone(&watch_state);
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
                            wake_pair.1.notify_all();
                        }
                    }
                })
                .ok();

            let mut bound_gen: u64 = u64::MAX;
            let mut bound_dir = String::new();

            loop {
                let (state_dir, active, stopped, cur_gen) = {
                    let Ok(mut guard) = watch_state.0.lock() else {
                        return;
                    };
                    if guard.stopped {
                        return;
                    }
                    if !guard.active && guard.reconfigure_gen == bound_gen {
                        guard = match watch_state.1.wait_timeout(guard, Duration::from_secs(5)) {
                            Ok((g, _)) => g,
                            Err(_) => return,
                        };
                        if guard.stopped {
                            return;
                        }
                    }
                    (
                        guard.state_dir.clone(),
                        guard.active,
                        guard.stopped,
                        guard.reconfigure_gen,
                    )
                };
                if stopped {
                    return;
                }

                if cur_gen != bound_gen || state_dir != bound_dir {
                    if let Some(ref mut w) = watcher {
                        if !bound_dir.is_empty() {
                            let _ = w.unwatch(std::path::Path::new(&bound_dir));
                        }
                        if !state_dir.is_empty() {
                            let _ = std::fs::create_dir_all(&state_dir);
                            let _ = w.watch(
                                std::path::Path::new(&state_dir),
                                RecursiveMode::NonRecursive,
                            );
                        }
                    }
                    bound_gen = cur_gen;
                    bound_dir = state_dir;
                }

                if active {
                    if fs_rx.recv_timeout(Duration::from_millis(500)).is_ok() {
                        while fs_rx.try_recv().is_ok() {}
                    }
                    let still_running = watch_state
                        .0
                        .lock()
                        .map(|g| !g.stopped && g.active)
                        .unwrap_or(false);
                    if still_running {
                        let _ = qt_thread_watch.queue(|mut backend| {
                            backend.as_mut().check_background_task();
                        });
                    }
                }
            }
        });
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
}
