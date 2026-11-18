use std::path::Path;

use chrono::Local;

use crate::task::files::{
    clear_task_files, read_process_start_time, read_task_lock, write_task_lock, write_task_meta,
    TaskMeta,
};

pub fn is_pid_alive(pid: i64) -> bool {
    if pid <= 0 {
        return false;
    }
    let Ok(pid_t) = libc::pid_t::try_from(pid) else {
        return false;
    };
    unsafe { libc::kill(pid_t, 0) == 0 }
}

pub fn send_signal_group(pid: i64, sig: i32) {
    if pid <= 0 {
        return;
    }
    if let Ok(pid_t) = libc::pid_t::try_from(pid) {
        unsafe {
            libc::kill(-pid_t, sig);
        }
    }
}

/// Returns `(active, pid)`. When `busy` is true, returns `(true, read_task_pid)`.
///
/// Compares `/proc/<pid>/stat` start time when recorded in `task.pid` so a
/// recycled PID after a crash or reboot is treated as stale.
pub fn is_task_lock_active(state_dir: &Path, busy: bool) -> (bool, i64) {
    let lock = read_task_lock(state_dir);
    if busy {
        return (true, lock.pid);
    }
    if !is_pid_alive(lock.pid) {
        return (false, 0);
    }
    if let Some(expected_start) = lock.start_time {
        if read_process_start_time(lock.pid) != Some(expected_start) {
            return (false, 0);
        }
    }
    (true, lock.pid)
}

pub fn acquire_cli_task_lock(
    state_dir: &Path,
    busy: bool,
    description: &str,
    env_name: &str,
) -> bool {
    if is_task_lock_active(state_dir, busy).0 {
        return false;
    }
    let meta = TaskMeta {
        description: description.to_string(),
        env_name: env_name.to_string(),
        target_env_name: env_name.to_string(),
        downloading_os_id: None,
        started_at: Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
        reason: None,
    };
    let _ = write_task_meta(state_dir, &meta);
    let pid = i64::from(unsafe { libc::getpid() });
    write_task_lock(state_dir, pid, read_process_start_time(pid)).is_ok()
}

pub fn release_cli_task_lock(state_dir: &Path) {
    clear_task_files(state_dir);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::files::write_task_pid;
    use std::fs;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("aibox_task_lock_{name}_{nanos}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    #[test]
    fn lock_handles_stale_live_and_release_matches_qt_test() {
        let dir = temp_dir("lock");

        // Stale PID must not lock
        write_task_pid(&dir, 99_999_999).expect("write stale pid");
        assert!(!is_task_lock_active(&dir, false).0);

        // Live PID locks, and releasing clears lock
        let mut sleeper = Command::new("/bin/sleep")
            .arg("10")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep");
        write_task_pid(&dir, i64::from(sleeper.id())).expect("write live pid");
        let (active, pid) = is_task_lock_active(&dir, false);
        assert!(active);
        assert_eq!(pid, i64::from(sleeper.id()));

        let _ = sleeper.kill();
        let _ = sleeper.wait();
        release_cli_task_lock(&dir);
        assert!(!is_task_lock_active(&dir, false).0);

        // CLI acquire and double-acquire prevention
        assert!(acquire_cli_task_lock(&dir, false, "Starting 'dev'", "dev"));
        assert!(is_task_lock_active(&dir, false).0);
        assert!(!acquire_cli_task_lock(&dir, false, "Other", "dev"));
        release_cli_task_lock(&dir);
        assert!(!is_task_lock_active(&dir, false).0);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn lock_with_live_pid_and_mismatched_start_time_is_stale() {
        let dir = temp_dir("pid_reuse");
        let mut sleeper = Command::new("/bin/sleep")
            .arg("10")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep");
        let pid = i64::from(sleeper.id());
        let actual_start = read_process_start_time(pid).expect("start time");

        // Live PID with different start time simulates PID reuse -> must be stale.
        write_task_lock(&dir, pid, Some(actual_start.wrapping_add(999)))
            .expect("write mismatched lock");
        assert_eq!(is_task_lock_active(&dir, false), (false, 0));

        // Live PID with matching start time -> active.
        write_task_lock(&dir, pid, Some(actual_start)).expect("write matching lock");
        assert_eq!(is_task_lock_active(&dir, false), (true, pid));

        let _ = sleeper.kill();
        let _ = sleeper.wait();
        let _ = fs::remove_dir_all(&dir);
    }
}
