//! Top-level snapshot and wipe shell script builders.

use crate::snapshot::workspace::{
    workspace_delete_snapshot_snippet, workspace_restore_snippet, workspace_snapshot_dir,
    workspace_snapshot_snippet,
};
use crate::util::paths::clean_path;
use crate::util::shell::shell_quote;

/// Builds the shell script for creating an Incus snapshot (and optional workspace Btrfs snapshot).
pub fn create_snapshot_script(
    name: &str,
    snap_name: &str,
    workspace_path: &str,
    include_workspace: bool,
) -> String {
    let q_name = shell_quote(name);
    let q_snap = shell_quote(snap_name);
    let mut script = format!(
        "set -euo pipefail\n\
echo '@@STAGE:40:Creating OS container snapshot...'\n\
incus snapshot create {q_name} {q_snap}\n"
    );
    let snap_dir = if include_workspace {
        workspace_snapshot_dir(workspace_path, name)
    } else {
        String::new()
    };
    if include_workspace && !snap_dir.is_empty() {
        let ws_snap = workspace_snapshot_snippet(workspace_path, &snap_dir, snap_name);
        if !ws_snap.is_empty() {
            let warn = format!("Warning: workspace snapshot failed for {name}/{snap_name}");
            script.push_str(&format!(
                "echo '@@STAGE:75:Creating workspace Btrfs snapshot...'\n\
(\n{ws_snap}\n) || echo {} >&2\n",
                shell_quote(&warn)
            ));
        }
    }
    script.push_str("echo '@@STAGE:100:Saved.'\n");
    script
}

/// Builds the shell script for restoring an Incus and/or workspace snapshot.
pub fn restore_snapshot_script(
    name: &str,
    snap_name: &str,
    workspace_path: &str,
    mode: &str,
    include_workspace: bool,
    safety_epoch: i64,
) -> String {
    let normalized = mode.trim().to_lowercase();
    let do_system = normalized != "workspace";
    let do_workspace = normalized == "workspace" || (normalized != "system" && include_workspace);
    let q_name = shell_quote(name);
    let q_snap = shell_quote(snap_name);

    let mut script = String::from("set -euo pipefail\n");
    if do_workspace {
        let snap_dir = workspace_snapshot_dir(workspace_path, name);
        if snap_dir.is_empty() {
            script.push_str("echo \"Workspace path is unavailable.\" >&2\nexit 1\n");
            return script;
        }
        script.push_str(&format!(
            "echo '@@STAGE:15:Checking environment state...'\n\
            STATUS=$(incus info {q_name} 2>/dev/null | awk '/^Status:/ {{print $2}}')\n\
            if [[ \"$STATUS\" == \"RUNNING\" ]]; then\n    \
                echo '@@STAGE:30:Stopping environment before workspace swap...'\n    \
                incus stop {q_name}\n\
            fi\n"
        ));
        if do_system {
            script.push_str(&format!(
                "echo '@@STAGE:55:Restoring OS snapshot...'\n\
incus snapshot restore {q_name} {q_snap}\n"
            ));
        }
        script.push_str(
            "echo '@@STAGE:78:Restoring workspace snapshot (with pre-restore safety snapshot)...'\n",
        );
        script.push_str(&workspace_restore_snippet(
            workspace_path,
            &snap_dir,
            snap_name,
            safety_epoch,
        ));
        script.push('\n');
    } else if do_system {
        script.push_str(&format!(
            "echo '@@STAGE:50:Restoring OS snapshot...'\n\
incus snapshot restore {q_name} {q_snap}\n"
        ));
    }
    script.push_str("echo '@@STAGE:100:Restored.'\n");
    script
}

/// Builds the shell script for deleting an Incus and/or workspace snapshot.
pub fn delete_snapshot_script(
    name: &str,
    snap_name: &str,
    workspace_path: &str,
    has_incus_snap: bool,
    has_workspace_snap: bool,
) -> String {
    let mut script = String::from("set -euo pipefail\n");
    if has_incus_snap {
        script.push_str(&format!(
            "echo '@@STAGE:40:Deleting OS snapshot...'\n\
incus snapshot delete {} {}\n",
            shell_quote(name),
            shell_quote(snap_name)
        ));
    }
    if has_workspace_snap {
        let snap_dir = workspace_snapshot_dir(workspace_path, name);
        if !snap_dir.is_empty() {
            script.push_str("echo '@@STAGE:75:Deleting workspace snapshot...'\n");
            script.push_str(&workspace_delete_snapshot_snippet(
                &format!("{snap_dir}/{snap_name}"),
                false,
            ));
            script.push('\n');
        }
    }
    script.push_str("echo '@@STAGE:100:Deleted.'\n");
    script
}

/// Builds the shell script for wiping an environment and optionally its workspace, sessions, and secrets.
pub fn wipe_environment_script(
    name: &str,
    workspace_path: &str,
    delete_workspace_files: bool,
    sessions_env_dir: &str,
    environment_secrets_path: &str,
) -> String {
    let mut script = format!(
        "echo '@@STAGE:40:Wiping OS container...'; \
incus delete {} --force; \
incus network acl delete {} >/dev/null 2>&1 || true; \
incus network acl delete {} >/dev/null 2>&1 || true",
        shell_quote(name),
        shell_quote(&format!("aibox-{name}")),
        shell_quote(&format!("aibox-log-{name}"))
    );
    if delete_workspace_files && !workspace_path.trim().is_empty() {
        let clean_ws = clean_path(workspace_path.trim());
        let snap_dir = workspace_snapshot_dir(&clean_ws, name);
        script.push_str("\necho '@@STAGE:75:Removing workspace and workspace snapshots...'\n");
        if !snap_dir.is_empty() {
            let q_snap_dir = shell_quote(&snap_dir);
            script.push_str(&format!(
                "if [ -d {q_snap_dir} ]; then\n    \
                    for S in {q_snap_dir}/*; do\n        \
                        [ -e \"$S\" ] || continue\n        \
                        if ! btrfs subvolume delete \"$S\" >/dev/null 2>&1; then\n            \
                            btrfs property set -ts \"$S\" ro false >/dev/null 2>&1 || true\n            \
                            rm -rf \"$S\" >/dev/null 2>&1 || true\n        \
                        fi\n    \
                    done\n    \
                    rmdir {q_snap_dir} 2>/dev/null || rm -rf {q_snap_dir} 2>/dev/null || true\n\
                fi\n"
            ));
        }
        script.push_str(&workspace_delete_snapshot_snippet(&clean_ws, true));
        script.push('\n');
        script.push_str(&workspace_delete_snapshot_snippet(
            &format!("{clean_ws}.pre-subvol"),
            true,
        ));
        script.push('\n');
    }
    if !sessions_env_dir.trim().is_empty() {
        script.push_str(&format!(
            "\nrm -rf {} >/dev/null 2>&1 || true\n",
            shell_quote(&clean_path(sessions_env_dir.trim()))
        ));
    }
    if !environment_secrets_path.trim().is_empty() {
        // Instance-only secrets must not survive the environment they belong to.
        script.push_str(&format!(
            "\nrm -f {} >/dev/null 2>&1 || true\n",
            shell_quote(&clean_path(environment_secrets_path.trim()))
        ));
    }
    script.push_str("\necho '@@STAGE:100:Wiped.'");
    script
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    fn write_executable(path: &std::path::Path, content: &str) {
        fs::write(path, content).expect("write script");
        let mut perms = fs::metadata(path).expect("meta").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("chmod");
    }

    #[test]
    fn workspace_restore_and_delete_scripts_with_stub_btrfs_match_qt_test() {
        let tmp = std::env::temp_dir().join(format!(
            "aibox-stub-btrfs-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let stub_dir = tmp.join("stubs");
        let ws_base = tmp.join("workspaces");
        let ws_path = ws_base.join("dev-box");
        let log_path = tmp.join("calls.log");
        fs::create_dir_all(&stub_dir).expect("stub_dir");
        fs::create_dir_all(&ws_path).expect("ws_path");

        let ws_str = ws_path.to_string_lossy().to_string();
        let snap_dir = workspace_snapshot_dir(&ws_str, "dev-box");
        fs::create_dir_all(&snap_dir).expect("snap_dir");
        fs::write(ws_path.join("current.txt"), "modified").expect("write current");

        write_executable(
            &stub_dir.join("stat"),
            "#!/bin/sh\n\
if [ \"$1\" = -f ]; then echo btrfs; exit 0; fi\n\
if [ \"$1\" = -c ]; then echo 256; exit 0; fi\n\
exec /usr/bin/stat \"$@\"\n",
        );
        write_executable(
            &stub_dir.join("btrfs"),
            "#!/bin/sh\n\
echo \"btrfs $@\" >> \"$AIBOX_STUB_LOG\"\n\
if [ \"$1\" = subvolume ] && [ \"$2\" = snapshot ]; then\n\
    if [ \"$3\" = -r ]; then src=\"$4\"; dst=\"$5\"; else src=\"$3\"; dst=\"$4\"; fi\n\
    mkdir -p \"$dst\" && cp -a \"$src/.\" \"$dst/\" 2>/dev/null || true\n\
    exit 0\n\
fi\n\
if [ \"$1\" = subvolume ] && [ \"$2\" = create ]; then\n\
    mkdir -p \"$3\"\n\
    exit 0\n\
fi\n\
if [ \"$1\" = subvolume ] && [ \"$2\" = delete ]; then\n\
    rm -rf \"$3\"\n\
    exit 0\n\
fi\n\
exit 0\n",
        );
        write_executable(
            &stub_dir.join("incus"),
            "#!/bin/sh\n\
echo \"incus $@\" >> \"$AIBOX_STUB_LOG\"\n\
if [ \"$1\" = info ]; then echo \"Status: RUNNING\"; fi\n\
exit 0\n",
        );

        let orig_path = std::env::var("PATH").unwrap_or_default();
        let test_path = format!("{}:{orig_path}", stub_dir.display());
        let run_bash = |script: &str| {
            let script_file = tmp.join("run.sh");
            fs::write(&script_file, script).expect("write run.sh");
            let out = Command::new("/bin/bash")
                .arg(&script_file)
                .env("PATH", &test_path)
                .env("AIBOX_STUB_LOG", &log_path)
                .output()
                .expect("run bash");
            assert!(
                out.status.success(),
                "bash failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        };

        // 1. Paired createSnapshot
        run_bash(&create_snapshot_script("dev-box", "snap-a", &ws_str, true));
        assert!(std::path::Path::new(&format!("{snap_dir}/snap-a/current.txt")).exists());

        // 2. Restore Both stops container, restores, takes pre-restore safety snap, swaps
        fs::write(ws_path.join("current.txt"), "broken").expect("broken");
        fs::write(format!("{snap_dir}/snap-a/current.txt"), "clean").expect("clean");
        let _ = fs::remove_file(&log_path);
        run_bash(&restore_snapshot_script(
            "dev-box", "snap-a", &ws_str, "both", true, 1700000123,
        ));
        let restore_log = fs::read_to_string(&log_path).expect("read log");
        let stop_idx = restore_log.find("incus stop dev-box").expect("stop");
        let incus_restore_idx = restore_log
            .find("incus snapshot restore dev-box snap-a")
            .expect("incus restore");
        let safety_snap_idx = restore_log
            .find(&format!(
                "btrfs subvolume snapshot -r {ws_str} {snap_dir}/pre-restore-1700000123"
            ))
            .expect("safety snap");
        let ws_restore_idx = restore_log
            .find(&format!(
                "btrfs subvolume snapshot {snap_dir}/snap-a {ws_str}.restore-tmp"
            ))
            .expect("ws restore");
        assert!(
            incus_restore_idx > stop_idx
                && safety_snap_idx > incus_restore_idx
                && ws_restore_idx > safety_snap_idx
        );
        assert!(
            std::path::Path::new(&format!("{snap_dir}/pre-restore-1700000123/current.txt"))
                .exists()
        );
        assert_eq!(
            fs::read_to_string(ws_path.join("current.txt")).expect("restored"),
            "clean"
        );

        // 3. System-only restore leaves workspace untouched
        fs::write(ws_path.join("current.txt"), "keep-me").expect("keep-me");
        run_bash(&restore_snapshot_script(
            "dev-box", "snap-a", &ws_str, "system", true, 1700000999,
        ));
        assert_eq!(
            fs::read_to_string(ws_path.join("current.txt")).expect("kept"),
            "keep-me"
        );

        // 4. Delete paired snapshot
        run_bash(&delete_snapshot_script(
            "dev-box", "snap-a", &ws_str, true, true,
        ));
        assert!(!std::path::Path::new(&format!("{snap_dir}/snap-a")).exists());

        // 5. Wipe with deleteWorkspaceFiles
        run_bash(&wipe_environment_script("dev-box", &ws_str, true, "", ""));
        assert!(!std::path::Path::new(&snap_dir).exists());
        assert!(!ws_path.exists());

        let _ = fs::remove_dir_all(&tmp);
    }
}
