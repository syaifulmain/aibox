use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, OnceLock};
use std::time::{Duration, Instant};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// Debounce window for coalescing bursts of filesystem events (e.g. atomic
/// write-temp + rename or rapid editor saves) into a single reload.
const DEBOUNCE_WINDOW: Duration = Duration::from_millis(120);

enum WatcherMsg {
    FsEvent(notify::Result<notify::Event>),
    Register {
        id: u64,
        dirs: Vec<PathBuf>,
        callback: Box<dyn FnMut() + Send>,
        ack: mpsc::Sender<()>,
    },
    UpdateDirs {
        id: u64,
        dirs: Vec<PathBuf>,
        ack: mpsc::Sender<()>,
    },
    Unregister {
        id: u64,
    },
}

struct Subscriber {
    watched_dirs: HashSet<PathBuf>,
    match_prefixes: Vec<PathBuf>,
    callback: Box<dyn FnMut() + Send>,
    deadline: Option<Instant>,
}

/// Process-wide shared filesystem watcher running on a single background thread.
///
/// Both `Theme` and `TranslatorState` register directory watches here so the
/// application keeps one `notify` watcher and one event-loop thread.
pub struct SharedWatcher {
    next_id: AtomicU64,
    tx: mpsc::Sender<WatcherMsg>,
}

impl SharedWatcher {
    pub fn global() -> &'static Self {
        static INSTANCE: OnceLock<SharedWatcher> = OnceLock::new();
        INSTANCE.get_or_init(Self::spawn)
    }

    fn spawn() -> Self {
        let (tx, rx) = mpsc::channel::<WatcherMsg>();
        let fs_tx = tx.clone();

        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let _ = fs_tx.send(WatcherMsg::FsEvent(res));
        })
        .ok();

        std::thread::Builder::new()
            .name("aibox-fs-watcher".to_string())
            .spawn(move || run_watcher_loop(watcher, rx))
            .ok();

        Self {
            next_id: AtomicU64::new(1),
            tx,
        }
    }

    pub fn subscribe<F>(&self, dirs: Vec<PathBuf>, callback: F) -> WatchHandle
    where
        F: FnMut() + Send + 'static,
    {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (ack_tx, ack_rx) = mpsc::channel();
        if self
            .tx
            .send(WatcherMsg::Register {
                id,
                dirs,
                callback: Box::new(callback),
                ack: ack_tx,
            })
            .is_ok()
        {
            let _ = ack_rx.recv();
        }
        WatchHandle { id }
    }

    fn update_dirs(&self, id: u64, dirs: Vec<PathBuf>) {
        let (ack_tx, ack_rx) = mpsc::channel();
        if self
            .tx
            .send(WatcherMsg::UpdateDirs {
                id,
                dirs,
                ack: ack_tx,
            })
            .is_ok()
        {
            let _ = ack_rx.recv();
        }
    }

    fn unregister(&self, id: u64) {
        let _ = self.tx.send(WatcherMsg::Unregister { id });
    }
}

/// RAII handle for a `SharedWatcher` subscription. Unregisters on drop.
pub struct WatchHandle {
    id: u64,
}

impl WatchHandle {
    pub fn update_dirs(&self, dirs: Vec<PathBuf>) {
        SharedWatcher::global().update_dirs(self.id, dirs);
    }
}

impl Drop for WatchHandle {
    fn drop(&mut self) {
        SharedWatcher::global().unregister(self.id);
    }
}

fn run_watcher_loop(mut watcher: Option<RecommendedWatcher>, rx: mpsc::Receiver<WatcherMsg>) {
    let mut subscribers: HashMap<u64, Subscriber> = HashMap::new();
    let mut dir_refcounts: HashMap<PathBuf, usize> = HashMap::new();

    loop {
        let timeout = next_deadline_timeout(&subscribers);
        let msg = match timeout {
            Some(dur) => match rx.recv_timeout(dur) {
                Ok(m) => Some(m),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            },
            None => match rx.recv() {
                Ok(m) => Some(m),
                Err(_) => break,
            },
        };

        if let Some(m) = msg {
            handle_msg(m, watcher.as_mut(), &mut subscribers, &mut dir_refcounts);
            while let Ok(m) = rx.try_recv() {
                handle_msg(m, watcher.as_mut(), &mut subscribers, &mut dir_refcounts);
            }
        }

        let now = Instant::now();
        for sub in subscribers.values_mut() {
            if let Some(deadline) = sub.deadline {
                if now >= deadline {
                    sub.deadline = None;
                    (sub.callback)();
                }
            }
        }
    }
}

fn next_deadline_timeout(subscribers: &HashMap<u64, Subscriber>) -> Option<Duration> {
    let earliest = subscribers.values().filter_map(|s| s.deadline).min()?;
    Some(earliest.saturating_duration_since(Instant::now()))
}

fn handle_msg(
    msg: WatcherMsg,
    watcher: Option<&mut RecommendedWatcher>,
    subscribers: &mut HashMap<u64, Subscriber>,
    dir_refcounts: &mut HashMap<PathBuf, usize>,
) {
    match msg {
        WatcherMsg::FsEvent(Ok(event)) => {
            if matches!(event.kind, EventKind::Access(_)) {
                return;
            }
            let deadline = Instant::now() + DEBOUNCE_WINDOW;
            for sub in subscribers.values_mut() {
                if event_matches_subscriber(&event.paths, sub) {
                    sub.deadline = Some(deadline);
                }
            }
        }
        WatcherMsg::FsEvent(Err(_)) => {}
        WatcherMsg::Register {
            id,
            dirs,
            callback,
            ack,
        } => {
            let mut sub = Subscriber {
                watched_dirs: HashSet::new(),
                match_prefixes: Vec::new(),
                callback,
                deadline: None,
            };
            apply_dirs(&mut sub, dirs, watcher, dir_refcounts);
            subscribers.insert(id, sub);
            let _ = ack.send(());
        }
        WatcherMsg::UpdateDirs { id, dirs, ack } => {
            if let Some(sub) = subscribers.get_mut(&id) {
                apply_dirs(sub, dirs, watcher, dir_refcounts);
            }
            let _ = ack.send(());
        }
        WatcherMsg::Unregister { id } => {
            if let Some(mut sub) = subscribers.remove(&id) {
                apply_dirs(&mut sub, Vec::new(), watcher, dir_refcounts);
            }
        }
    }
}

fn event_matches_subscriber(paths: &[PathBuf], sub: &Subscriber) -> bool {
    if paths.is_empty() {
        return !sub.match_prefixes.is_empty();
    }
    paths.iter().any(|p| {
        sub.match_prefixes
            .iter()
            .any(|prefix| p.starts_with(prefix) || prefix.starts_with(p))
    })
}

fn apply_dirs(
    sub: &mut Subscriber,
    new_dirs: Vec<PathBuf>,
    mut watcher: Option<&mut RecommendedWatcher>,
    dir_refcounts: &mut HashMap<PathBuf, usize>,
) {
    let mut target_set = HashSet::new();
    let mut prefixes = Vec::new();

    for dir in new_dirs {
        if dir.is_dir() {
            if target_set.insert(dir.clone()) {
                prefixes.push(dir.clone());
            }
            if let Ok(canon) = std::fs::canonicalize(&dir) {
                if canon.is_dir() && target_set.insert(canon.clone()) {
                    prefixes.push(canon);
                }
            }
        }
    }

    let to_remove: Vec<PathBuf> = sub.watched_dirs.difference(&target_set).cloned().collect();
    for dir in to_remove {
        sub.watched_dirs.remove(&dir);
        if let Some(count) = dir_refcounts.get_mut(&dir) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                dir_refcounts.remove(&dir);
                if let Some(w) = watcher.as_deref_mut() {
                    let _ = w.unwatch(&dir);
                }
            }
        }
    }

    let to_add: Vec<PathBuf> = target_set.difference(&sub.watched_dirs).cloned().collect();
    for dir in to_add {
        let count = dir_refcounts.entry(dir.clone()).or_insert(0);
        let should_watch = *count == 0;
        let ok = if should_watch {
            watcher
                .as_deref_mut()
                .map(|w| w.watch(&dir, RecursiveMode::NonRecursive).is_ok())
                .unwrap_or(false)
        } else {
            true
        };
        if ok {
            *count += 1;
            sub.watched_dirs.insert(dir);
        } else if should_watch {
            dir_refcounts.remove(&dir);
        }
    }

    sub.match_prefixes = prefixes;
}

/// Returns the nearest existing ancestor directory for `path`.
pub fn nearest_existing_dir(path: &Path) -> Option<PathBuf> {
    let mut cur = path.to_path_buf();
    while !cur.exists() {
        let parent = cur.parent()?.to_path_buf();
        if parent == cur {
            return None;
        }
        cur = parent;
    }
    if cur.is_dir() {
        Some(cur)
    } else {
        cur.parent().map(Path::to_path_buf)
    }
}

/// Computes a fast content hash over a list of file paths and extra salt bytes
/// so callers can skip parsing/reloading when file contents are unchanged.
pub fn hash_files<H: Hash>(paths: &[PathBuf], extra: &H) -> u64 {
    let mut hasher = DefaultHasher::new();
    extra.hash(&mut hasher);
    for path in paths {
        path.hash(&mut hasher);
        match std::fs::read(path) {
            Ok(bytes) => {
                true.hash(&mut hasher);
                bytes.hash(&mut hasher);
            }
            Err(_) => {
                false.hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::sync::Arc;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let mut p = std::env::temp_dir();
            let pid = std::process::id();
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            p.push(format!("aibox-watcher-test-{prefix}-{pid}-{nanos}"));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn coalesces_ten_file_events_within_50ms_into_single_reload() {
        let tmp = TempDir::new("burst");
        let file = tmp.0.join("colors.toml");
        std::fs::write(&file, "initial = true\n").unwrap();

        let count = Arc::new(AtomicUsize::new(0));
        let count_cb = count.clone();
        let _handle = SharedWatcher::global().subscribe(vec![tmp.0.clone()], move || {
            count_cb.fetch_add(1, AtomicOrdering::SeqCst);
        });

        // Allow the watcher thread to register the directory before bursting.
        std::thread::sleep(Duration::from_millis(40));

        for i in 0..10 {
            std::fs::write(&file, format!("step = {i}\n")).unwrap();
            std::thread::sleep(Duration::from_millis(4));
        }

        // Wait past the 120 ms debounce window.
        let deadline = Instant::now() + Duration::from_secs(2);
        while count.load(AtomicOrdering::SeqCst) == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(15));
        }
        std::thread::sleep(Duration::from_millis(180));

        assert_eq!(
            count.load(AtomicOrdering::SeqCst),
            1,
            "10 file events in <50ms must coalesce into exactly 1 reload"
        );
    }
}
