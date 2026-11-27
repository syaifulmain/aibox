//! Workspace Btrfs snapshot paths, filesystem checks, and shell snippets.

use std::ffi::CString;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use crate::util::paths::{clean_path, CONTAINER_WORKSPACE_PATH};
use crate::util::shell::shell_quote;

pub(crate) fn parent_dir(clean: &str) -> &str {
    if clean == "/" {
        return "/";
    }
    match clean.rfind('/') {
        Some(0) => "/",
        Some(idx) => &clean[..idx],
        None => ".",
    }
}

fn path_is_btrfs(path: &Path) -> bool {
    let Ok(cstr) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    let mut buf: libc::statfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statfs(cstr.as_ptr(), &mut buf) };
    if rc != 0 {
        return false;
    }
    buf.f_type == libc::BTRFS_SUPER_MAGIC
}

/// Returns `<parent>/.snapshots/<envName>` for a workspace path, or empty if either is empty.
pub fn workspace_snapshot_dir(workspace_path: &str, env_name: &str) -> String {
    let clean_ws = clean_path(workspace_path.trim());
    let clean_env = env_name.trim();
    if clean_ws.is_empty() || clean_env.is_empty() {
        return String::new();
    }
    let parent = parent_dir(&clean_ws);
    clean_path(&format!("{parent}/.snapshots/{clean_env}"))
}

/// Walks up to 16 parent directories to find an existing path and checks whether it resides on Btrfs.
pub fn is_btrfs_filesystem(path: &str) -> bool {
    let mut cur = clean_path(path.trim());
    if cur.is_empty() {
        return false;
    }
    for _ in 0..16 {
        if cur.is_empty() {
            break;
        }
        let p = Path::new(&cur);
        if p.exists() {
            return path_is_btrfs(p);
        }
        let parent = parent_dir(&cur).to_string();
        if parent == cur {
            break;
        }
        cur = parent;
    }
    false
}

/// Returns `true` if `path` exists on a Btrfs filesystem and is a directory with inode 256.
pub fn is_btrfs_subvolume(path: &str) -> bool {
    let clean = clean_path(path.trim());
    if clean.is_empty() {
        return false;
    }
    let p = Path::new(&clean);
    let Ok(meta) = fs::symlink_metadata(p) else {
        return false;
    };
    meta.file_type().is_dir() && meta.ino() == 256 && path_is_btrfs(p)
}

/// Lists sorted directory names under the workspace snapshot directory for `env_name`.
pub fn list_workspace_snapshots(workspace_path: &str, env_name: &str) -> Vec<String> {
    let snap_dir = workspace_snapshot_dir(workspace_path, env_name);
    if snap_dir.is_empty() {
        return Vec::new();
    }
    let dir_path = Path::new(&snap_dir);
    if !dir_path.is_dir() {
        return Vec::new();
    }
    let Ok(entries) = fs::read_dir(dir_path) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for entry in entries.flatten() {
        if entry.path().is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    names
}

/// Returns workspace snapshot names that do not have a matching Incus snapshot.
pub fn find_orphan_workspace_snapshots(
    incus_snapshots: &[String],
    ws_snapshots: &[String],
) -> Vec<String> {
    ws_snapshots
        .iter()
        .filter(|ws| !incus_snapshots.iter().any(|incus| incus == *ws))
        .cloned()
        .collect()
}

/// Shell snippet that creates `workspace_path` as a Btrfs subvolume when on Btrfs, or `mkdir -p` otherwise.
pub fn workspace_create_snippet(workspace_path: &str) -> String {
    let q_ws = shell_quote(&clean_path(workspace_path.trim()));
    format!(
        "mkdir -p \"$(dirname {q_ws})\"\n\
        if [ ! -e {q_ws} ]; then\n    \
            if command -v btrfs >/dev/null 2>&1 && [[ \"$(stat -f -c %%T \"$(dirname {q_ws})\" 2>/dev/null || true)\" == \"btrfs\" ]]; then\n        \
                btrfs subvolume create {q_ws} >/dev/null\n    \
            else\n        \
                mkdir -p {q_ws}\n    \
            fi\n\
        fi"
    )
}

/// Shell snippet that migrates a legacy workspace device mount to `/home/workspace`.
pub fn workspace_mount_migration_script(env_name: &str) -> String {
    let q_name = shell_quote(env_name);
    let q_path = shell_quote(CONTAINER_WORKSPACE_PATH);
    format!(
        "WS_MOUNT=$(incus config device get {q_name} workspace path 2>/dev/null || true)\n\
        if [ -n \"$WS_MOUNT\" ] && [ \"$WS_MOUNT\" != {q_path} ]; then\n    \
            if incus config device set {q_name} workspace path={q_path} >/dev/null 2>&1; then\n        \
                echo \"Workspace mount moved: $WS_MOUNT -> {q_path}\"\n    \
            fi\n\
        fi"
    )
}

/// Parses `incus config device get <env> workspace path` output, falling back to `CONTAINER_WORKSPACE_PATH`.
pub fn parse_container_workspace_path_output(exit_ok: bool, stdout: &str) -> String {
    let trimmed = stdout.trim();
    if exit_ok && trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        CONTAINER_WORKSPACE_PATH.to_string()
    }
}

/// Queries Incus for the live workspace mount path of `name` (with a 5s timeout), falling back to `/home/workspace`.
pub fn container_workspace_path_for(name: &str, demo_mode: bool) -> String {
    if demo_mode || name.trim().is_empty() {
        return CONTAINER_WORKSPACE_PATH.to_string();
    }
    let Ok(child) = Command::new("incus")
        .args(["config", "device", "get", name, "workspace", "path"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return CONTAINER_WORKSPACE_PATH.to_string();
    };

    let pid = child.id();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });

    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(out)) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            parse_container_workspace_path_output(out.status.success(), &stdout)
        }
        Ok(Err(_)) => CONTAINER_WORKSPACE_PATH.to_string(),
        Err(_) => {
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGKILL);
            }
            CONTAINER_WORKSPACE_PATH.to_string()
        }
    }
}

/// Shell snippet that takes a read-only Btrfs snapshot of `workspace_path` at `snap_dir/snap_name`.
pub fn workspace_snapshot_snippet(workspace_path: &str, snap_dir: &str, snap_name: &str) -> String {
    let clean_ws = clean_path(workspace_path.trim());
    let clean_dir = clean_path(snap_dir.trim());
    let clean_snap = snap_name.trim();
    if clean_ws.is_empty() || clean_dir.is_empty() || clean_snap.is_empty() {
        return String::new();
    }
    let q_ws = shell_quote(&clean_ws);
    let q_dir = shell_quote(&clean_dir);
    let q_dst = shell_quote(&format!("{clean_dir}/{clean_snap}"));
    format!(
        "if command -v btrfs >/dev/null 2>&1 && [ -d {q_ws} ] && [[ \"$(stat -f -c %%T {q_ws} 2>/dev/null || true)\" == \"btrfs\" ]] && [[ \"$(stat -c %%i {q_ws} 2>/dev/null || true)\" == \"256\" ]]; then\n    \
            mkdir -p {q_dir}\n    \
            if [ ! -e {q_dst} ]; then\n        \
                btrfs subvolume snapshot -r {q_ws} {q_dst} >/dev/null\n    \
            fi\n\
        fi"
    )
}

/// Shell snippet that deletes a workspace Btrfs snapshot or directory, optionally only warning on failure.
pub fn workspace_delete_snapshot_snippet(snap_path: &str, warn_only: bool) -> String {
    let clean_snap = clean_path(snap_path.trim());
    if clean_snap.is_empty() {
        return String::new();
    }
    let q_snap = shell_quote(&clean_snap);
    let exit_line = if warn_only { "" } else { "        exit 1\n" };
    let q_warn = shell_quote(&format!(
        "Warning: could not delete subvolume or root-owned files; run: sudo rm -rf {q_snap}"
    ));
    format!(
        "if [ -e {q_snap} ]; then\n    \
            if ! btrfs subvolume delete {q_snap} >/dev/null 2>&1; then\n        \
                if command -v btrfs >/dev/null 2>&1 && [[ \"$(stat -f -c %%T {q_snap} 2>/dev/null || true)\" == \"btrfs\" ]] && [[ \"$(stat -c %%i {q_snap} 2>/dev/null || true)\" == \"256\" ]]; then\n            \
                    btrfs property set -ts {q_snap} ro false >/dev/null 2>&1 || true\n        \
                fi\n        \
                rm -rf {q_snap} >/dev/null 2>&1 || true\n    \
            fi\n    \
            if [ -e {q_snap} ] && command -v sudo >/dev/null 2>&1 && sudo -n true >/dev/null 2>&1; then\n        \
                sudo -n rm -rf {q_snap} >/dev/null 2>&1 || true\n    \
            fi\n    \
            if [ -e {q_snap} ]; then\n        \
                echo {q_warn} >&2\n\
                {exit_line}    fi\n\
        fi"
    )
}

/// Shell snippet that restores `workspace_path` from `snap_dir/snap_name` after taking a pre-restore safety snapshot.
pub fn workspace_restore_snippet(
    workspace_path: &str,
    snap_dir: &str,
    snap_name: &str,
    safety_epoch: i64,
) -> String {
    let clean_ws = clean_path(workspace_path.trim());
    let clean_dir = clean_path(snap_dir.trim());
    let clean_snap = snap_name.trim();
    if clean_ws.is_empty() || clean_dir.is_empty() || clean_snap.is_empty() {
        return String::new();
    }
    let q_ws = shell_quote(&clean_ws);
    let q_dir = shell_quote(&clean_dir);
    let q_snap = shell_quote(&format!("{clean_dir}/{clean_snap}"));
    let q_tmp = shell_quote(&format!("{clean_ws}.restore-tmp"));
    let q_old = shell_quote(&format!("{clean_ws}.restore-old"));
    let epoch_expr = if safety_epoch > 0 {
        safety_epoch.to_string()
    } else {
        "$(date +%s)".to_string()
    };
    let safety_path_expr = format!("{}\"/pre-restore-{epoch_expr}\"", shell_quote(&clean_dir));
    let del_tmp = workspace_delete_snapshot_snippet(&format!("{clean_ws}.restore-tmp"), true);
    let del_old = workspace_delete_snapshot_snippet(&format!("{clean_ws}.restore-old"), true);
    let q_missing_msg = shell_quote(&format!("Workspace snapshot {q_snap} does not exist."));

    format!(
        "if [ ! -d {q_snap} ]; then\n    \
            echo {q_missing_msg} >&2\n    \
            exit 1\n\
        fi\n\
        if ! command -v btrfs >/dev/null 2>&1; then\n    \
            echo \"btrfs tool is required for workspace snapshot restore.\" >&2\n    \
            exit 1\n\
        fi\n\
        mkdir -p {q_dir}\n\
        if [ -d {q_ws} ] && [[ \"$(stat -f -c %%T {q_ws} 2>/dev/null || true)\" == \"btrfs\" ]] && [[ \"$(stat -c %%i {q_ws} 2>/dev/null || true)\" == \"256\" ]]; then\n    \
            SAFETY_SNAP={safety_path_expr}\n    \
            [ -e \"$SAFETY_SNAP\" ] || btrfs subvolume snapshot -r {q_ws} \"$SAFETY_SNAP\" >/dev/null\n\
        fi\n\
        if [ -e {q_tmp} ]; then\n    \
            {del_tmp}\n\
        fi\n\
        btrfs subvolume snapshot {q_snap} {q_tmp} >/dev/null\n\
        if [ -e {q_ws} ]; then\n    \
            mv {q_ws} {q_old}\n\
        fi\n\
        if ! mv {q_tmp} {q_ws}; then\n    \
            [ -e {q_old} ] && mv {q_old} {q_ws} || true\n    \
            exit 1\n\
        fi\n\
        {del_old}"
    )
}

/// Full shell script that converts an existing plain workspace directory into a Btrfs subvolume.
pub fn convert_workspace_to_subvolume_script(env_name: &str, workspace_path: &str) -> String {
    let clean_ws = clean_path(workspace_path.trim());
    let q_name = shell_quote(env_name);
    let q_ws = shell_quote(&clean_ws);
    let q_tmp = shell_quote(&format!("{clean_ws}.subvol-tmp"));
    let q_pre = shell_quote(&format!("{clean_ws}.pre-subvol"));
    let q_stop_msg = shell_quote(&format!(
        "Stop {q_name} before converting its workspace to a Btrfs subvolume."
    ));
    let q_pre_exists_msg = shell_quote(&format!(
        "Backup directory {q_pre} already exists. Remove or rename it first."
    ));
    format!(
        "set -euo pipefail\n\
        echo '@@STAGE:10:Checking container & Btrfs status...'\n\
        STATUS=$(incus info {q_name} 2>/dev/null | awk '/^Status:/ {{print $2}}')\n\
        if [[ \"$STATUS\" == \"RUNNING\" ]]; then\n    \
            echo {q_stop_msg} >&2\n    \
            exit 1\n\
        fi\n\
        if ! command -v btrfs >/dev/null 2>&1; then\n    \
            echo \"btrfs command not found.\" >&2\n    \
            exit 1\n\
        fi\n\
        mkdir -p \"$(dirname {q_ws})\"\n\
        if [[ \"$(stat -f -c %%T \"$(dirname {q_ws})\" 2>/dev/null || true)\" != \"btrfs\" ]]; then\n    \
            echo \"Workspace filesystem is not Btrfs.\" >&2\n    \
            exit 1\n\
        fi\n\
        if [ -d {q_ws} ] && [[ \"$(stat -c %%i {q_ws} 2>/dev/null || true)\" == \"256\" ]]; then\n    \
            echo '@@STAGE:100:Workspace is already a Btrfs subvolume.'\n    \
            exit 0\n\
        fi\n\
        if [ -e {q_pre} ]; then\n    \
            echo {q_pre_exists_msg} >&2\n    \
            exit 1\n\
        fi\n\
        echo '@@STAGE:30:Creating Btrfs subvolume...'\n\
        if [ -e {q_tmp} ]; then rm -rf {q_tmp}; fi\n\
        btrfs subvolume create {q_tmp} >/dev/null\n\
        if [ -d {q_ws} ]; then\n    \
            echo '@@STAGE:55:Copying workspace files with reflinks...'\n    \
            cp -a --reflink=always {q_ws}/. {q_tmp}/ 2>/dev/null || cp -a {q_ws}/. {q_tmp}/\n\
        fi\n\
        if command -v setfacl >/dev/null 2>&1; then\n    \
            ACL_USER=\"${{SUDO_USER:-${{USER:-$(id -un)}}}}\"\n    \
            setfacl -R -m \"u:${{ACL_USER}}:rwx\" -m \"m::rwx\" -d -m \"u:${{ACL_USER}}:rwx\" -d -m \"m::rwx\" {q_tmp} 2>/dev/null || true\n\
        fi\n\
        echo '@@STAGE:85:Swapping workspace into place...'\n\
        if [ -d {q_ws} ]; then\n    \
            mv {q_ws} {q_pre}\n\
        fi\n\
        if ! mv {q_tmp} {q_ws}; then\n    \
            [ -e {q_pre} ] && mv {q_pre} {q_ws} || true\n    \
            exit 1\n\
        fi\n\
        echo '@@STAGE:100:Workspace converted to Btrfs subvolume.'"
    )
}

/// Parses `Generation:` from `btrfs subvolume show` output.
pub fn parse_btrfs_subvolume_generation(output: &str) -> Option<u64> {
    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Generation:") {
            if let Ok(gen_id) = rest.trim().parse::<u64>() {
                return Some(gen_id);
            }
        }
    }
    None
}

fn btrfs_subvolume_generation(path: &str) -> Option<u64> {
    let out = Command::new("btrfs")
        .args(["subvolume", "show", path])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    parse_btrfs_subvolume_generation(&stdout)
}

/// Walks `root` up to `max_entries` filesystem entries and returns the newest mtime.
/// Returns `None` if `root` cannot be read or exceeds `max_entries` (conservative fallback).
fn max_tree_mtime(root: &Path, max_entries: usize) -> Option<std::time::SystemTime> {
    let root_meta = fs::symlink_metadata(root).ok()?;
    let mut latest = root_meta.modified().ok()?;
    let mut stack = vec![root.to_path_buf()];
    let mut visited = 0usize;

    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).ok()?;
        for entry in entries {
            visited += 1;
            if visited > max_entries {
                return None;
            }
            let Ok(entry) = entry else {
                continue;
            };
            let Ok(meta) = fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if let Ok(mtime) = meta.modified() {
                if mtime > latest {
                    latest = mtime;
                }
            }
            if meta.file_type().is_dir() {
                stack.push(entry.path());
            }
        }
    }
    Some(latest)
}

/// Returns `true` when `workspace_path` exists, has at least one existing snapshot in
/// `.snapshots/<env_name>/`, and has not been modified since the newest snapshot
/// (comparing Btrfs subvolume generation when available, otherwise filesystem mtime).
pub fn workspace_unchanged_since_last_snapshot(
    workspace_path: &str,
    env_name: &str,
    snapshots: &[String],
) -> bool {
    let clean_ws = clean_path(workspace_path.trim());
    if clean_ws.is_empty() || !Path::new(&clean_ws).is_dir() {
        return false;
    }
    let snap_dir = workspace_snapshot_dir(&clean_ws, env_name);
    if snap_dir.is_empty() {
        return false;
    }

    let mut newest_snap: Option<(std::time::SystemTime, String)> = None;
    for snap in snapshots {
        let snap_path = format!("{snap_dir}/{snap}");
        let p = Path::new(&snap_path);
        if !p.is_dir() {
            continue;
        }
        let Ok(meta) = fs::symlink_metadata(p) else {
            continue;
        };
        let Ok(mtime) = meta.modified() else {
            continue;
        };
        if newest_snap.as_ref().is_none_or(|(t, _)| mtime > *t) {
            newest_snap = Some((mtime, snap_path));
        }
    }

    let Some((snap_mtime, snap_path)) = newest_snap else {
        return false;
    };

    if is_btrfs_subvolume(&clean_ws) && is_btrfs_subvolume(&snap_path) {
        if let (Some(ws_gen), Some(snap_gen)) = (
            btrfs_subvolume_generation(&clean_ws),
            btrfs_subvolume_generation(&snap_path),
        ) {
            return ws_gen <= snap_gen;
        }
    }

    let Some(ws_mtime) = max_tree_mtime(Path::new(&clean_ws), 20_000) else {
        return false;
    };
    ws_mtime <= snap_mtime
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_snapshot_paths_and_orphan_detection_match_qt_test() {
        let tmp = std::env::temp_dir().join(format!(
            "aibox-ws-snap-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let ws_base = tmp.join("workspaces");
        let ws_path = ws_base.join("dev box");
        fs::create_dir_all(&ws_path).expect("mkdir ws");

        let ws_path_str = ws_path.to_string_lossy().to_string();
        let snap_dir = workspace_snapshot_dir(&ws_path_str, "dev-box");
        assert_eq!(
            snap_dir,
            clean_path(&format!("{}/.snapshots/dev-box", ws_base.display()))
        );

        fs::create_dir_all(format!("{snap_dir}/snap-1")).expect("snap-1");
        fs::create_dir_all(format!("{snap_dir}/pre-restore-123")).expect("pre-restore");
        fs::create_dir_all(format!("{snap_dir}/auto-20261113-100000")).expect("auto");

        let ws_snaps = list_workspace_snapshots(&ws_path_str, "dev-box");
        assert_eq!(
            ws_snaps,
            vec![
                "auto-20261113-100000".to_string(),
                "pre-restore-123".to_string(),
                "snap-1".to_string(),
            ]
        );

        let orphans = find_orphan_workspace_snapshots(
            &["snap-1".to_string(), "auto-20261113-100000".to_string()],
            &ws_snaps,
        );
        assert_eq!(orphans, vec!["pre-restore-123".to_string()]);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn btrfs_checks_return_false_on_tmpfs_and_missing_paths() {
        assert!(!is_btrfs_filesystem(""));
        assert!(!is_btrfs_subvolume(""));
        // /tmp is typically tmpfs in test environments; only assert false when it isn't btrfs.
        if !path_is_btrfs(Path::new("/tmp")) {
            assert!(!is_btrfs_filesystem("/tmp/nonexistent-aibox-dir/sub"));
            assert!(!is_btrfs_subvolume("/tmp"));
        }
    }

    #[test]
    fn mount_migration_and_container_workspace_path_output() {
        let migrate = workspace_mount_migration_script("box");
        assert!(migrate.contains("incus config device get 'box' workspace path"));
        assert!(migrate.contains("incus config device set 'box' workspace path='/home/workspace'"));

        assert_eq!(
            parse_container_workspace_path_output(true, "/workspace\n"),
            "/workspace"
        );
        assert_eq!(
            parse_container_workspace_path_output(false, "/workspace\n"),
            "/home/workspace"
        );
        assert_eq!(
            parse_container_workspace_path_output(true, "relative"),
            "/home/workspace"
        );
    }

    #[test]
    fn parses_btrfs_subvolume_generation() {
        let sample = "\
\tName: \t\t\tdev-box
\tUUID: \t\t\t1234
\tGeneration: \t\t4219
\tGen at creation: \t4100
";
        assert_eq!(parse_btrfs_subvolume_generation(sample), Some(4219));
        assert_eq!(parse_btrfs_subvolume_generation("not a subvolume"), None);
    }

    #[test]
    fn non_btrfs_scripts_degrade_with_clear_messages() {
        let convert = convert_workspace_to_subvolume_script("dev-box", "/tmp/ws/dev-box");
        assert!(convert.contains("Workspace filesystem is not Btrfs."));
        assert!(convert.contains("btrfs command not found."));

        let restore = workspace_restore_snippet(
            "/tmp/ws/dev-box",
            "/tmp/ws/.snapshots/dev-box",
            "snap-1",
            100,
        );
        assert!(restore.contains("btrfs tool is required for workspace snapshot restore."));
        assert!(restore.contains("does not exist."));
    }
}
