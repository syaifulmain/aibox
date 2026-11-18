//! Orphan workspace, snapshot, and backup archive detection and cleanup.

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use serde_json::{json, Value};

use crate::backup::{directory_size_bytes, expand_tilde};
use crate::snapshot::workspace_delete_snapshot_snippet;
use crate::util::format::format_bytes;
use crate::util::paths::{clean_path, home_dir};
use crate::util::shell::shell_quote;

fn absolutize_clean(path: &str) -> String {
    let p = Path::new(path);
    if p.is_absolute() {
        clean_path(path)
    } else if let Ok(cwd) = std::env::current_dir() {
        clean_path(&cwd.join(p).to_string_lossy())
    } else {
        clean_path(path)
    }
}

fn sorted_subdirs_no_symlinks(dir: &Path) -> Vec<(String, String)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if ft.is_symlink() || !ft.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "." || name == ".." {
            continue;
        }
        let abs = clean_path(&entry.path().to_string_lossy());
        out.push((name, abs));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Scans `workspace_base`, `workspace_base/.snapshots`, and `backups_dir` for orphans
/// not belonging to any known environment in `environments`.
pub fn detect_orphans(
    environments: &[Value],
    workspace_base: &str,
    backups_dir: &str,
) -> Vec<Value> {
    let mut known_env_names = BTreeSet::new();
    let mut known_workspace_paths = BTreeSet::new();
    let home = home_dir();

    for v in environments {
        let Some(env) = v.as_object() else {
            continue;
        };
        if let Some(name) = env.get("name").and_then(Value::as_str) {
            if !name.is_empty() {
                known_env_names.insert(name.to_string());
            }
        }
        if let Some(ws_raw) = env.get("workspacePath").and_then(Value::as_str) {
            let ws = expand_tilde(ws_raw.trim(), &home);
            if !ws.is_empty() {
                known_workspace_paths.insert(absolutize_clean(&ws));
            }
        }
    }

    let mut orphans = Vec::new();
    let clean_ws_base = if workspace_base.trim().is_empty() {
        String::new()
    } else {
        clean_path(workspace_base.trim())
    };

    if !clean_ws_base.is_empty() && Path::new(&clean_ws_base).is_dir() {
        for (dir_name, abs_path) in sorted_subdirs_no_symlinks(Path::new(&clean_ws_base)) {
            if dir_name.starts_with('.') {
                continue;
            }
            let env_candidate = dir_name.strip_suffix(".pre-subvol").unwrap_or(&dir_name);
            if !known_env_names.contains(&dir_name)
                && !known_env_names.contains(env_candidate)
                && !known_workspace_paths.contains(&abs_path)
            {
                let bytes = directory_size_bytes(&abs_path);
                orphans.push(json!({
                    "kind": "workspace",
                    "kindLabel": "Orphan workspace",
                    "name": dir_name,
                    "path": abs_path,
                    "openPath": abs_path,
                    "sizeBytes": bytes,
                    "sizeDisplay": format_bytes(bytes),
                }));
            }
        }

        let snap_root = format!("{clean_ws_base}/.snapshots");
        if Path::new(&snap_root).is_dir() {
            for (env_name, abs_path) in sorted_subdirs_no_symlinks(Path::new(&snap_root)) {
                if known_env_names.contains(&env_name) {
                    continue;
                }
                let bytes = directory_size_bytes(&abs_path);
                orphans.push(json!({
                    "kind": "workspace-snapshot",
                    "kindLabel": "Orphan ws snapshots",
                    "name": env_name,
                    "path": abs_path,
                    "openPath": abs_path,
                    "sizeBytes": bytes,
                    "sizeDisplay": format_bytes(bytes),
                }));
            }
        }
    }

    let clean_backups_dir = if backups_dir.trim().is_empty() {
        String::new()
    } else {
        clean_path(backups_dir.trim())
    };
    if !clean_backups_dir.is_empty() && Path::new(&clean_backups_dir).is_dir() {
        if let Ok(entries) = fs::read_dir(&clean_backups_dir) {
            let mut tar_files: Vec<(String, String, i64)> = Vec::new();
            for entry in entries.flatten() {
                let Ok(meta) = fs::metadata(entry.path()) else {
                    continue;
                };
                if !meta.is_file() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                if !name.ends_with(".tar.gz") && !name.ends_with(".tar.zst") {
                    continue;
                }
                let abs = clean_path(&entry.path().to_string_lossy());
                tar_files.push((name, abs, meta.len() as i64));
            }
            tar_files.sort_by(|a, b| a.0.cmp(&b.0));

            let mut instance_bases = BTreeSet::new();
            for (fn_name, _, _) in &tar_files {
                let is_ws = fn_name.ends_with("-workspace.tar.gz")
                    || fn_name.ends_with("-workspace.tar.zst");
                if is_ws {
                    continue;
                }
                if let Some(base) = fn_name
                    .strip_suffix(".tar.gz")
                    .or_else(|| fn_name.strip_suffix(".tar.zst"))
                {
                    instance_bases.insert(base.to_string());
                }
            }

            for (fn_name, abs_path, bytes) in tar_files {
                let Some(base) = fn_name
                    .strip_suffix("-workspace.tar.gz")
                    .or_else(|| fn_name.strip_suffix("-workspace.tar.zst"))
                else {
                    continue;
                };
                if !instance_bases.contains(base) {
                    orphans.push(json!({
                        "kind": "backup-workspace",
                        "kindLabel": "Orphan backup workspace",
                        "name": fn_name,
                        "path": abs_path,
                        "openPath": clean_backups_dir,
                        "sizeBytes": bytes,
                        "sizeDisplay": format_bytes(bytes),
                    }));
                }
            }
        }
    }

    orphans
}

/// Validates that `clean_path_in` sits directly under the allowed directory for `kind`,
/// is not a symlink escape, and matches the expected naming rules.
pub fn validate_orphan_path(
    kind: &str,
    clean_path_in: &str,
    workspace_base: &str,
    backups_dir: &str,
) -> Result<(), String> {
    let clean = clean_path(clean_path_in);
    if let Ok(meta) = fs::symlink_metadata(&clean) {
        if meta.file_type().is_symlink() {
            return Err(format!(
                "Refusing to delete symlink path outside allowed directory: {clean}"
            ));
        }
    }

    let p = Path::new(&clean);
    let file_name = p
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let parent_dir = p
        .parent()
        .map(|par| clean_path(&par.to_string_lossy()))
        .unwrap_or_default();

    let clean_ws_base = clean_path(workspace_base);
    let clean_snap_root = clean_path(&format!("{workspace_base}/.snapshots"));
    let clean_backups = clean_path(backups_dir);

    if kind == "workspace"
        && (parent_dir != clean_ws_base || file_name == ".snapshots" || file_name.starts_with('.'))
    {
        return Err(format!(
            "Refusing to delete path outside workspace directory: {clean}"
        ));
    }
    if kind == "workspace-snapshot" && parent_dir != clean_snap_root {
        return Err(format!(
            "Refusing to delete path outside snapshot root: {clean}"
        ));
    }
    if kind == "backup-workspace"
        && (parent_dir != clean_backups
            || (!file_name.ends_with("-workspace.tar.gz")
                && !file_name.ends_with("-workspace.tar.zst")))
    {
        return Err(format!(
            "Refusing to delete non-orphan archive outside backups directory: {clean}"
        ));
    }
    Ok(())
}

/// Validates `kind` and `path` before orphan deletion. Returns the cleaned path on success.
pub fn validate_orphan_request(
    kind: &str,
    path: &str,
    workspace_base: &str,
    backups_dir: &str,
) -> Result<String, String> {
    let clean = clean_path(path.trim());
    if clean.is_empty() {
        return Err("Orphan path is empty.".to_string());
    }
    if kind != "workspace" && kind != "workspace-snapshot" && kind != "backup-workspace" {
        return Err(format!("Unknown orphan kind '{kind}'."));
    }
    validate_orphan_path(kind, &clean, workspace_base, backups_dir)?;
    Ok(clean)
}

/// Builds the shell script to delete an orphan workspace, snapshot directory, or backup tarball.
pub fn delete_orphan_script(kind: &str, target_path: &str) -> String {
    let clean_target = clean_path(target_path.trim());
    let q_target = shell_quote(&clean_target);
    if kind == "backup-workspace" {
        return format!(
            "set -euo pipefail\n\
             echo '@@STAGE:40:Removing orphan backup archive...'\n\
             rm -f {q_target}\n\
             echo '@@STAGE:100:Orphan removed.'"
        );
    }
    if kind == "workspace-snapshot" {
        return format!(
            "set -euo pipefail\n\
             echo '@@STAGE:35:Removing orphan workspace snapshots...'\n\
             if [ -d {q_target} ]; then\n    \
             for S in {q_target}/*; do\n        \
             [ -e \"$S\" ] || continue\n        \
             if ! btrfs subvolume delete \"$S\" >/dev/null 2>&1; then\n            \
             btrfs property set -ts \"$S\" ro false >/dev/null 2>&1 || true\n            \
             rm -rf \"$S\" >/dev/null 2>&1 || true\n        \
             fi\n    \
             done\n    \
             rmdir {q_target} >/dev/null 2>&1 || rm -rf {q_target} >/dev/null 2>&1 || true\n    \
             if [ -e {q_target} ] && command -v sudo >/dev/null 2>&1 && sudo -n true >/dev/null 2>&1; then\n        \
             echo '@@STAGE:70:Retrying removal with admin rights...'\n        \
             sudo -n rm -rf {q_target} >/dev/null 2>&1 || true\n    \
             fi\n\
             fi\n\
             if [ -e {q_target} ]; then\n    \
             echo 'Could not completely remove orphan workspace snapshots (root-owned files or Btrfs subvolumes).' >&2\n    \
             echo \"Fix (admin): sudo rm -rf {q_target}\" >&2\n    \
             exit 1\n\
             fi\n\
             echo '@@STAGE:100:Orphan workspace snapshots removed.'"
        );
    }
    let del_snaps = workspace_delete_snapshot_snippet(&clean_target, true);
    format!(
        "set -euo pipefail\n\
         echo '@@STAGE:35:Removing orphan workspace folder...'\n\
         {del_snaps}\n\
         rm -rf {q_target} >/dev/null 2>&1 || true\n\
         if [ -e {q_target} ] && command -v sudo >/dev/null 2>&1 && sudo -n true >/dev/null 2>&1; then\n    \
         echo '@@STAGE:70:Retrying removal with admin rights...'\n    \
         sudo -n rm -rf {q_target} >/dev/null 2>&1 || true\n\
         fi\n\
         if [ -e {q_target} ]; then\n    \
         echo 'Could not completely remove orphan workspace folder (root-owned files or Btrfs subvolumes).' >&2\n    \
         echo \"Fix (admin): sudo rm -rf {q_target}\" >&2\n    \
         exit 1\n\
         fi\n\
         echo '@@STAGE:100:Orphan workspace removed.'"
    )
}

/// Builds the standalone POSIX shell script run via `sudo -- sh <script>` for admin orphan deletion.
pub fn admin_delete_script_text(kind: &str, target_path: &str) -> String {
    let clean_target = clean_path(target_path.trim());
    let q_target = shell_quote(&clean_target);
    if kind == "backup-workspace" {
        return format!("#!/bin/sh\nset -u\nrm -f {q_target}\n");
    }
    format!(
        "#!/bin/sh\n\
         set -u\n\
         TARGET={q_target}\n\
         rm -rf \"$TARGET\" 2>/dev/null || true\n\
         if [ -e \"$TARGET\" ]; then\n    \
         find \"$TARGET\" -depth -type d -exec sh -c 'btrfs subvolume show \"$1\" >/dev/null 2>&1 && \
         btrfs subvolume delete \"$1\" >/dev/null 2>&1' sh {{}} \\; 2>/dev/null || true\n    \
         rm -rf \"$TARGET\" 2>/dev/null || true\n\
         fi\n\
         if [ -e \"$TARGET\" ]; then\n    \
         echo \"Failed to remove: $TARGET\" >&2\n    \
         exit 1\n\
         fi\n\
         echo \"Removed: $TARGET\"\n"
    )
}

/// Validates and writes the admin-delete helper script into `state_dir/admin-delete.sh`,
/// returning the terminal command to execute on success.
pub fn prepare_delete_orphan_as_admin(
    kind: &str,
    path: &str,
    workspace_base: &str,
    backups_dir: &str,
    state_dir: &str,
    has_sudo: bool,
) -> Result<(String, String), String> {
    let clean = validate_orphan_request(kind, path, workspace_base, backups_dir)?;
    if !Path::new(&clean).exists() {
        return Err(format!("Orphan path '{clean}' no longer exists."));
    }
    if !has_sudo {
        return Err("sudo is not available on this system.".to_string());
    }
    let script_path = format!("{state_dir}/admin-delete.sh");
    if let Err(e) = fs::create_dir_all(state_dir) {
        return Err(format!(
            "Could not write admin delete script to {script_path}: {e}"
        ));
    }
    let text = admin_delete_script_text(kind, &clean);
    if fs::write(&script_path, text.as_bytes()).is_err() {
        return Err(format!(
            "Could not write admin delete script to {script_path}."
        ));
    }
    let _ = fs::set_permissions(&script_path, fs::Permissions::from_mode(0o700));
    let q_script = shell_quote(&script_path);
    let cmd = format!(
        "sudo -- sh {q_script}; RC=$?; echo; \
         if [ $RC -eq 0 ]; then echo 'AIBox: orphan removed.'; \
         else echo 'AIBox: orphan removal failed.'; fi; \
         echo 'Refresh the stale report to update this list.'; \
         echo 'Press Enter to close...'; read -r _"
    );
    let file_name = Path::new(&clean)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok((cmd, file_name))
}

/// Execution mode for orphan cleanup. Defaults to `DryRun` for safety.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OrphanCleanupMode {
    #[default]
    DryRun,
    Execute,
}

/// A single validated orphan item scheduled for confirmation or deletion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrphanPlanItem {
    pub kind: String,
    pub name: String,
    pub path: String,
    pub size_bytes: i64,
    pub size_display: String,
}

/// Explicit orphan cleanup plan with default dry-run mode and confirmation list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrphanCleanupPlan {
    pub dry_run: bool,
    pub items: Vec<OrphanPlanItem>,
    pub total_bytes: i64,
    pub total_display: String,
    pub confirmation_list: Vec<String>,
    pub requires_confirmation: bool,
}

/// Result of running `execute_orphan_cleanup` in `DryRun` or `Execute` mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrphanCleanupReport {
    pub dry_run: bool,
    pub would_delete: Vec<String>,
    pub deleted: Vec<String>,
    pub skipped_active: Vec<String>,
    pub delete_scripts: Vec<(String, String)>,
    pub reclaimed_bytes: i64,
}

/// Builds a dry-run-by-default orphan cleanup plan with an explicit confirmation list.
/// When `selected_paths` is `Some`, only orphans matching those cleaned paths are included.
pub fn plan_orphan_cleanup(
    environments: &[Value],
    workspace_base: &str,
    backups_dir: &str,
    selected_paths: Option<&[String]>,
) -> OrphanCleanupPlan {
    let detected = detect_orphans(environments, workspace_base, backups_dir);
    let selected_set: Option<BTreeSet<String>> =
        selected_paths.map(|paths| paths.iter().map(|p| clean_path(p.trim())).collect());

    let mut items = Vec::new();
    let mut confirmation_list = Vec::new();
    let mut total_bytes = 0i64;

    for v in detected {
        let Some(kind) = v.get("kind").and_then(Value::as_str) else {
            continue;
        };
        let Some(path) = v.get("path").and_then(Value::as_str) else {
            continue;
        };
        let clean = clean_path(path);
        if let Some(ref filter) = selected_set {
            if !filter.contains(&clean) {
                continue;
            }
        }
        if validate_orphan_request(kind, &clean, workspace_base, backups_dir).is_err() {
            continue;
        }
        let name = v
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(&clean)
            .to_string();
        let size_bytes = v.get("sizeBytes").and_then(Value::as_i64).unwrap_or(0);
        let size_display = v
            .get("sizeDisplay")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format_bytes(size_bytes));
        total_bytes = total_bytes.saturating_add(size_bytes);
        confirmation_list.push(format!("[{kind}] {clean} ({size_display})"));
        items.push(OrphanPlanItem {
            kind: kind.to_string(),
            name,
            path: clean,
            size_bytes,
            size_display,
        });
    }

    OrphanCleanupPlan {
        dry_run: true,
        requires_confirmation: !items.is_empty(),
        items,
        total_bytes,
        total_display: format_bytes(total_bytes),
        confirmation_list,
    }
}

/// Executes (or dry-runs) an `OrphanCleanupPlan`, re-verifying against `environments` so an
/// active environment's workspace or snapshots can never be deleted.
pub fn execute_orphan_cleanup(
    plan: &OrphanCleanupPlan,
    mode: OrphanCleanupMode,
    environments: &[Value],
    workspace_base: &str,
    backups_dir: &str,
) -> Result<OrphanCleanupReport, String> {
    if mode == OrphanCleanupMode::DryRun {
        return Ok(OrphanCleanupReport {
            dry_run: true,
            would_delete: plan.confirmation_list.clone(),
            deleted: Vec::new(),
            skipped_active: Vec::new(),
            delete_scripts: Vec::new(),
            reclaimed_bytes: 0,
        });
    }

    if plan.items.is_empty() || plan.confirmation_list.is_empty() {
        return Err("Orphan cleanup requires a non-empty explicit confirmation list.".to_string());
    }

    // Re-scan live orphans so any newly-active environment is protected.
    let current_orphans: BTreeSet<String> =
        detect_orphans(environments, workspace_base, backups_dir)
            .into_iter()
            .filter_map(|v| {
                v.get("path")
                    .and_then(Value::as_str)
                    .map(|p| clean_path(p.trim()))
            })
            .collect();

    let mut deleted = Vec::new();
    let mut skipped_active = Vec::new();
    let mut delete_scripts = Vec::new();
    let mut reclaimed_bytes = 0i64;

    for item in &plan.items {
        let clean = validate_orphan_request(&item.kind, &item.path, workspace_base, backups_dir)?;
        if !current_orphans.contains(&clean) {
            skipped_active.push(clean);
            continue;
        }
        if item.kind == "backup-workspace" {
            if Path::new(&clean).exists() {
                fs::remove_file(&clean)
                    .map_err(|e| format!("Could not delete orphan archive '{clean}': {e}"))?;
            }
            deleted.push(clean);
            reclaimed_bytes = reclaimed_bytes.saturating_add(item.size_bytes);
        } else {
            let script = delete_orphan_script(&item.kind, &clean);
            delete_scripts.push((clean.clone(), script));
            deleted.push(clean);
            reclaimed_bytes = reclaimed_bytes.saturating_add(item.size_bytes);
        }
    }

    Ok(OrphanCleanupReport {
        dry_run: false,
        would_delete: Vec::new(),
        deleted,
        skipped_active,
        delete_scripts,
        reclaimed_bytes,
    })
}
