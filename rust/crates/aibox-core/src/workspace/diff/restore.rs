use std::ffi::{CString, OsStr};
use std::fs;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::snapshot::workspace::workspace_snapshot_dir;
use crate::util::paths::clean_path;

use super::file::{is_safe_relative_path, is_safe_snapshot_name};

fn to_cstring(s: &str) -> Result<CString, String> {
    CString::new(s).map_err(|_| format!("Invalid path component containing NUL byte: {s}"))
}

fn open_dir_root(path: &Path) -> Result<OwnedFd, String> {
    let c_path = to_cstring(&path.to_string_lossy())?;
    // SAFETY: `c_path` is a valid NUL-terminated path string; `open` returns -1 on error.
    let fd = unsafe {
        libc::open(
            c_path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(format!(
            "Could not open workspace root '{}': {}",
            path.display(),
            io::Error::last_os_error()
        ));
    }
    // SAFETY: `fd` is a newly opened valid file descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

fn open_or_create_subdir_nofollow(parent_fd: &OwnedFd, seg: &str) -> Result<OwnedFd, String> {
    let c_seg = to_cstring(seg)?;
    let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    // SAFETY: `parent_fd` is a valid directory fd and `c_seg` is NUL-terminated.
    let mut fd = unsafe { libc::openat(parent_fd.as_raw_fd(), c_seg.as_ptr(), flags) };
    if fd < 0 {
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::ENOENT) {
            // SAFETY: `parent_fd` is a valid directory fd and `c_seg` is NUL-terminated.
            let mk_rc = unsafe { libc::mkdirat(parent_fd.as_raw_fd(), c_seg.as_ptr(), 0o755) };
            if mk_rc < 0 {
                let mk_err = io::Error::last_os_error();
                if mk_err.raw_os_error() != Some(libc::EEXIST) {
                    return Err(format!(
                        "Could not create parent directory '{seg}': {mk_err}"
                    ));
                }
            }
            // SAFETY: `parent_fd` is a valid directory fd and `c_seg` is NUL-terminated.
            fd = unsafe { libc::openat(parent_fd.as_raw_fd(), c_seg.as_ptr(), flags) };
        }
    }
    if fd < 0 {
        let err = io::Error::last_os_error();
        if matches!(err.raw_os_error(), Some(libc::ELOOP) | Some(libc::ENOTDIR)) {
            return Err(format!(
                "Refusing to traverse symlink in workspace path: {seg}"
            ));
        }
        return Err(format!("Could not open parent directory '{seg}': {err}"));
    }
    // SAFETY: `fd` is a newly opened valid file descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

fn readlinkat_path(dir_fd: &OwnedFd, c_name: &CString) -> Result<PathBuf, String> {
    let mut buf = vec![0_u8; 4096];
    // SAFETY: `dir_fd` is valid, `c_name` is NUL-terminated, and `buf` is writable up to `buf.len()`.
    let n = unsafe {
        libc::readlinkat(
            dir_fd.as_raw_fd(),
            c_name.as_ptr(),
            buf.as_mut_ptr().cast::<libc::c_char>(),
            buf.len(),
        )
    };
    if n < 0 {
        return Err(format!(
            "Could not inspect symlink target: {}",
            io::Error::last_os_error()
        ));
    }
    let len = n as usize;
    Ok(PathBuf::from(OsStr::from_bytes(&buf[..len])))
}

fn normalize_lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Prefix(p) => out.push(p.as_os_str()),
            Component::RootDir => out.push(comp.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(s) => out.push(s),
        }
    }
    out
}

fn symlink_escapes_root(ws_root_canon: &Path, parent_canon: &Path, target: &Path) -> bool {
    let candidate = if target.is_absolute() {
        target.to_path_buf()
    } else {
        parent_canon.join(target)
    };
    let normalized = normalize_lexical(&candidate);
    if !normalized.starts_with(ws_root_canon) {
        return true;
    }
    if let Ok(resolved) = fs::canonicalize(&candidate) {
        if !resolved.starts_with(ws_root_canon) {
            return true;
        }
    }
    false
}

fn check_target_in_dir(
    parent_fd: &OwnedFd,
    c_file: &CString,
    relative_path: &str,
    ws_root_canon: &Path,
    parent_canon: &Path,
) -> Result<(), String> {
    // SAFETY: `libc::stat` is plain old data; zeroed initialization is valid before `fstatat`.
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: `parent_fd` is valid, `c_file` is NUL-terminated, and `st` is writable.
    let rc = unsafe {
        libc::fstatat(
            parent_fd.as_raw_fd(),
            c_file.as_ptr(),
            &mut st,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if rc < 0 {
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::ENOENT) {
            return Ok(());
        }
        return Err(format!(
            "Could not inspect workspace file '{relative_path}': {err}"
        ));
    }

    let mode_type = st.st_mode & libc::S_IFMT;
    if mode_type == libc::S_IFDIR {
        return Err(format!(
            "Could not overwrite directory at workspace path: {relative_path}"
        ));
    }
    if mode_type == libc::S_IFLNK {
        let target = readlinkat_path(parent_fd, c_file)?;
        if symlink_escapes_root(ws_root_canon, parent_canon, &target) {
            return Err(format!(
                "Refusing to overwrite symlink pointing outside workspace: {relative_path}"
            ));
        }
    }
    Ok(())
}

fn unique_temp_name() -> CString {
    let nanos = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let name = format!(".aibox-restore-tmp-{}-{nanos}", std::process::id());
    CString::new(name).expect("temp filename has no NUL")
}

pub fn restore_single_file(
    name: &str,
    snapshot_name: &str,
    ws_path: &str,
    relative_path: &str,
) -> Result<String, String> {
    if name.is_empty()
        || !is_safe_snapshot_name(snapshot_name)
        || !is_safe_relative_path(relative_path)
    {
        return Err("Invalid file restore request.".to_string());
    }

    let snap_dir = workspace_snapshot_dir(ws_path, name);
    let snap_file = format!("{snap_dir}/{snapshot_name}/{relative_path}");
    let clean_ws = clean_path(ws_path);
    let ws_file = format!("{clean_ws}/{relative_path}");

    let src_path = Path::new(&snap_file);
    let Ok(src_meta) = fs::symlink_metadata(src_path) else {
        return Err(format!(
            "File '{relative_path}' does not exist in snapshot '{snapshot_name}'."
        ));
    };

    if src_meta.is_dir() {
        return Err("Cannot single-restore a directory; use Snapshot Restore instead.".to_string());
    }
    if !src_meta.file_type().is_symlink() && !src_meta.is_file() {
        return Err(
            "Cannot restore special file; only regular files and symlinks are supported."
                .to_string(),
        );
    }

    let ws_root_canon = fs::canonicalize(&clean_ws)
        .map_err(|_| format!("Workspace directory does not exist: {clean_ws}"))?;
    let mut cur_fd = open_dir_root(&ws_root_canon)?;
    let mut parent_canon = ws_root_canon.clone();

    let segments: Vec<&str> = relative_path.split('/').filter(|s| !s.is_empty()).collect();
    let (parent_segs, file_name) = (
        &segments[..segments.len() - 1],
        segments[segments.len() - 1],
    );

    for seg in parent_segs {
        cur_fd = open_or_create_subdir_nofollow(&cur_fd, seg)?;
        parent_canon.push(seg);
    }

    let c_file = to_cstring(file_name)?;
    check_target_in_dir(
        &cur_fd,
        &c_file,
        relative_path,
        &ws_root_canon,
        &parent_canon,
    )?;

    let c_tmp = unique_temp_name();

    if src_meta.file_type().is_symlink() {
        let Ok(target) = fs::read_link(src_path) else {
            return Err(format!("Could not restore symlink: {relative_path}"));
        };
        let c_target = to_cstring(&target.to_string_lossy())?;
        // SAFETY: `c_target`, `cur_fd`, and `c_tmp` are valid.
        let sym_rc =
            unsafe { libc::symlinkat(c_target.as_ptr(), cur_fd.as_raw_fd(), c_tmp.as_ptr()) };
        if sym_rc < 0 {
            return Err(format!("Could not restore symlink: {relative_path}"));
        }
        // SAFETY: `cur_fd`, `c_tmp`, and `c_file` are valid. `renameat` atomically replaces `c_file` without following symlinks.
        let ren_rc = unsafe {
            libc::renameat(
                cur_fd.as_raw_fd(),
                c_tmp.as_ptr(),
                cur_fd.as_raw_fd(),
                c_file.as_ptr(),
            )
        };
        if ren_rc < 0 {
            // SAFETY: Best-effort cleanup of temp symlink in `cur_fd`.
            unsafe { libc::unlinkat(cur_fd.as_raw_fd(), c_tmp.as_ptr(), 0) };
            return Err(format!("Could not restore symlink: {relative_path}"));
        }
    } else {
        let mut src_file = fs::File::open(src_path)
            .map_err(|_| format!("Could not copy file from snapshot: {relative_path}"))?;
        let mode = (src_meta.permissions().mode() & 0o7777) as libc::mode_t;
        // SAFETY: `cur_fd` and `c_tmp` are valid; `O_EXCL | O_NOFOLLOW` prevents symlink traversal.
        let tmp_raw = unsafe {
            libc::openat(
                cur_fd.as_raw_fd(),
                c_tmp.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                mode,
            )
        };
        if tmp_raw < 0 {
            return Err(format!("Could not overwrite workspace file: {ws_file}"));
        }
        // SAFETY: `tmp_raw` is a newly opened valid file descriptor.
        let mut tmp_file = unsafe { fs::File::from_raw_fd(tmp_raw) };
        if io::copy(&mut src_file, &mut tmp_file).is_err() {
            drop(tmp_file);
            // SAFETY: Best-effort cleanup of temp file in `cur_fd`.
            unsafe { libc::unlinkat(cur_fd.as_raw_fd(), c_tmp.as_ptr(), 0) };
            return Err(format!(
                "Could not copy file from snapshot: {relative_path}"
            ));
        }
        // SAFETY: `tmp_file` is a valid open file descriptor.
        let _ = unsafe { libc::fchmod(tmp_file.as_raw_fd(), mode) };
        drop(tmp_file);

        // SAFETY: `cur_fd`, `c_tmp`, and `c_file` are valid. `renameat` atomically replaces `c_file` without following symlinks.
        let ren_rc = unsafe {
            libc::renameat(
                cur_fd.as_raw_fd(),
                c_tmp.as_ptr(),
                cur_fd.as_raw_fd(),
                c_file.as_ptr(),
            )
        };
        if ren_rc < 0 {
            // SAFETY: Best-effort cleanup of temp file in `cur_fd`.
            unsafe { libc::unlinkat(cur_fd.as_raw_fd(), c_tmp.as_ptr(), 0) };
            return Err(format!("Could not overwrite workspace file: {ws_file}"));
        }
    }

    Ok(format!("Restored {snap_file} -> {ws_file}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::diff::file::{prepare_file_diff, shape_file_diff_output, FileDiffPrep};
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    struct TempDirGuard(PathBuf);
    impl TempDirGuard {
        fn new(prefix: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "aibox-rust-restore-{prefix}-{}",
                std::time::SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn prepare_and_restore_single_file_handle_files_symlinks_and_errors() {
        let dir = TempDirGuard::new("restore");
        let ws = dir.path().join("workspaces/box1");
        let snap_dir = workspace_snapshot_dir(ws.to_str().unwrap(), "box1");
        let snap = Path::new(&snap_dir).join("s1");
        fs::create_dir_all(ws.join("sub")).unwrap();
        fs::create_dir_all(snap.join("sub")).unwrap();

        fs::write(snap.join("sub/a.txt"), "original\n").unwrap();
        fs::set_permissions(snap.join("sub/a.txt"), fs::Permissions::from_mode(0o640)).unwrap();
        fs::write(ws.join("sub/a.txt"), "modified\n").unwrap();
        symlink("a.txt", snap.join("sub/link")).unwrap();
        fs::write(ws.join("bin.dat"), b"ABC\x00DEF").unwrap();

        assert_eq!(
            prepare_file_diff("box1", "s1", ws.to_str().unwrap(), "../bad", "modified"),
            FileDiffPrep::Immediate {
                text: String::new(),
                info: "Invalid file path.".to_string(),
            }
        );

        assert_eq!(
            prepare_file_diff("box1", "s1", ws.to_str().unwrap(), "bin.dat", "added"),
            FileDiffPrep::Immediate {
                text: String::new(),
                info: "Binary file — text diff unavailable.".to_string(),
            }
        );

        match prepare_file_diff("box1", "s1", ws.to_str().unwrap(), "sub/a.txt", "modified") {
            FileDiffPrep::RunDiff { args } => {
                assert_eq!(args[0], "-u");
                assert_eq!(args[2], "snapshot");
                assert_eq!(args[4], "current");
            }
            other => panic!("expected RunDiff, got {other:?}"),
        }

        let ok = restore_single_file("box1", "s1", ws.to_str().unwrap(), "sub/a.txt").unwrap();
        assert!(ok.starts_with("Restored "));
        assert_eq!(
            fs::read_to_string(ws.join("sub/a.txt")).unwrap(),
            "original\n"
        );
        assert_eq!(
            fs::metadata(ws.join("sub/a.txt"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o640
        );

        restore_single_file("box1", "s1", ws.to_str().unwrap(), "sub/link").unwrap();
        assert!(fs::symlink_metadata(ws.join("sub/link"))
            .unwrap()
            .file_type()
            .is_symlink());

        assert!(restore_single_file("box1", "s1", ws.to_str().unwrap(), "sub").is_err());
        assert!(restore_single_file("box1", "s1", ws.to_str().unwrap(), "missing.txt").is_err());
        assert!(restore_single_file("box1", "s1", ws.to_str().unwrap(), "../escape").is_err());

        let shaped = shape_file_diff_output(2, "", "sub/a.txt");
        assert_eq!(shaped.info, "Could not generate diff for sub/a.txt.");
    }

    #[test]
    fn restore_single_file_rejects_symlinks_escaping_root() {
        let dir = TempDirGuard::new("sym-escape");
        let outside = dir.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.txt"), "do-not-touch\n").unwrap();

        let ws = dir.path().join("workspaces/box1");
        let snap_dir = workspace_snapshot_dir(ws.to_str().unwrap(), "box1");
        let snap = Path::new(&snap_dir).join("s1");
        fs::create_dir_all(ws.join("sub")).unwrap();
        fs::create_dir_all(snap.join("sub")).unwrap();
        fs::create_dir_all(snap.join("escape_dir")).unwrap();

        fs::write(snap.join("sub/file.txt"), "restored\n").unwrap();
        fs::write(snap.join("escape_dir/pwn.txt"), "overwritten\n").unwrap();

        // 1. Target file itself is a symlink pointing outside workspace root
        symlink(outside.join("secret.txt"), ws.join("sub/file.txt")).unwrap();
        let err = restore_single_file("box1", "s1", ws.to_str().unwrap(), "sub/file.txt")
            .expect_err("must reject target symlink escaping root");
        assert!(err.contains("outside workspace"), "unexpected err: {err}");
        assert_eq!(
            fs::read_to_string(outside.join("secret.txt")).unwrap(),
            "do-not-touch\n"
        );

        // 2. Intermediate parent directory is a symlink pointing outside workspace root
        symlink(&outside, ws.join("escape_dir")).unwrap();
        let err = restore_single_file("box1", "s1", ws.to_str().unwrap(), "escape_dir/pwn.txt")
            .expect_err("must reject intermediate symlink directory");
        assert!(err.contains("symlink"), "unexpected err: {err}");
        assert!(!outside.join("pwn.txt").exists());
    }

    #[test]
    fn restore_single_file_symlink_race_never_writes_outside_root() {
        let dir = TempDirGuard::new("sym-race");
        let outside = dir.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        let outside_file = outside.join("victim.txt");
        fs::write(&outside_file, "intact\n").unwrap();

        let ws = dir.path().join("workspaces/box1");
        let snap_dir = workspace_snapshot_dir(ws.to_str().unwrap(), "box1");
        let snap = Path::new(&snap_dir).join("s1");
        fs::create_dir_all(&ws).unwrap();
        fs::create_dir_all(snap.join("dir")).unwrap();
        fs::write(snap.join("dir/victim.txt"), "pwned\n").unwrap();

        let stop = Arc::new(AtomicBool::new(false));
        let stop_racer = stop.clone();
        let race_dir = ws.join("dir");
        let outside_clone = outside.clone();

        let racer = std::thread::spawn(move || {
            while !stop_racer.load(Ordering::Relaxed) {
                let _ = fs::remove_dir_all(&race_dir);
                let _ = fs::remove_file(&race_dir);
                let _ = symlink(&outside_clone, &race_dir);
                std::thread::yield_now();
                let _ = fs::remove_file(&race_dir);
                let _ = fs::create_dir_all(&race_dir);
                std::thread::yield_now();
            }
        });

        for _ in 0..50 {
            let _ = restore_single_file("box1", "s1", ws.to_str().unwrap(), "dir/victim.txt");
        }
        stop.store(true, Ordering::Relaxed);
        racer.join().unwrap();

        assert_eq!(fs::read_to_string(&outside_file).unwrap(), "intact\n");
    }
}
