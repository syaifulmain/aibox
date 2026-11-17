//! Backup export, export-and-wipe, and import shell script builders.

use crate::snapshot::{wipe_environment_script, workspace_create_snippet};
use crate::util::paths::CONTAINER_WORKSPACE_PATH;
use crate::util::shell::shell_quote;

/// Builds the common `incus export` and optional workspace tarball steps.
pub fn build_export_steps(
    name: &str,
    backup_file: &str,
    workspace_path: &str,
    workspace_archive_file: &str,
) -> String {
    let q_name = shell_quote(name);
    let q_backup = shell_quote(backup_file);
    let mut script = format!(
        "set -e\n\
         mkdir -p \"$(dirname {q_backup})\"\n\
         echo '@@STAGE:15:Exporting instance...'\n\
         incus export {q_name} {q_backup} --compression gzip\n\
         echo '@@STAGE:60:Instance archive written.'"
    );

    if !workspace_path.trim().is_empty() && !workspace_archive_file.trim().is_empty() {
        let q_ws = shell_quote(workspace_path);
        let q_ws_out = shell_quote(workspace_archive_file);
        script.push_str(&format!(
            "\nif [ -d {q_ws} ]; then\n    \
             echo '@@STAGE:72:Archiving workspace directory...'\n    \
             tar -czf {q_ws_out} -C {q_ws} .\n\
             fi"
        ));
    }
    script
}

/// Builds the shell script for exporting an environment (and optionally its workspace).
pub fn export_environment_script(
    name: &str,
    backup_file: &str,
    workspace_path: &str,
    workspace_archive_file: &str,
) -> String {
    let mut script = build_export_steps(name, backup_file, workspace_path, workspace_archive_file);
    script.push_str("\necho '@@STAGE:100:Backup export complete.'\n");
    script
}

/// Builds the shell script that exports, verifies non-empty archives, and then wipes the environment.
pub fn export_and_wipe_environment_script(
    name: &str,
    backup_file: &str,
    workspace_path: &str,
    workspace_archive_file: &str,
    delete_workspace_files: bool,
    sessions_env_dir: &str,
    environment_secrets_path: &str,
) -> String {
    let q_backup = shell_quote(backup_file);
    let mut script = build_export_steps(name, backup_file, workspace_path, workspace_archive_file);
    script.push_str(&format!(
        "\necho '@@STAGE:78:Verifying exported archive before wipe...'\n\
         if [ ! -s {q_backup} ]; then\n    \
         echo 'Export verification failed: instance backup archive is missing or empty; aborting wipe.' >&2\n    \
         exit 1\n\
         fi\n"
    ));
    if !workspace_path.trim().is_empty() && !workspace_archive_file.trim().is_empty() {
        let q_ws = shell_quote(workspace_path);
        let q_ws_out = shell_quote(workspace_archive_file);
        script.push_str(&format!(
            "if [ -d {q_ws} ] && [ ! -s {q_ws_out} ]; then\n    \
             echo 'Export verification failed: workspace archive is missing or empty; aborting wipe.' >&2\n    \
             exit 1\n\
             fi\n"
        ));
    }
    script.push_str(&wipe_environment_script(
        name,
        workspace_path,
        delete_workspace_files,
        sessions_env_dir,
        environment_secrets_path,
    ));
    script
}

/// Builds the shell script for importing an environment backup archive and optional workspace tarball.
pub fn import_environment_script(
    backup_file: &str,
    target_name: &str,
    workspace_path: &str,
    workspace_archive_file: &str,
) -> String {
    let q_backup = shell_quote(backup_file);
    let q_name = shell_quote(target_name);
    let q_ws = shell_quote(workspace_path);
    let ws_create = workspace_create_snippet(workspace_path);
    let q_container_ws = shell_quote(CONTAINER_WORKSPACE_PATH);

    let mut script = format!(
        "set -e\n\
         echo '@@STAGE:15:Preparing target workspace...'\n\
         {ws_create}\n\
         if command -v setfacl >/dev/null 2>&1; then\n    \
         ACL_USER=\"${{SUDO_USER:-${{USER:-$(id -un)}}}}\"\n    \
         setfacl -R -m \"u:${{ACL_USER}}:rwx\" -m \"m::rwx\" -d -m \"u:${{ACL_USER}}:rwx\" -d -m \"m::rwx\" {q_ws} 2>/dev/null || true\n\
         fi\n\
         echo '@@STAGE:35:Importing archive as {q_name}...'\n\
         incus import {q_backup} {q_name}\n\
         incus config unset {q_name} user.aibox.template >/dev/null 2>&1 || true\n\
         incus config set {q_name} user.aibox.workspace {q_ws} >/dev/null 2>&1 || true\n\
         incus config device set {q_name} workspace source={q_ws} path={q_container_ws} >/dev/null 2>&1 || \
         incus config device add {q_name} workspace disk source={q_ws} path={q_container_ws} >/dev/null 2>&1 || true"
    );

    if !workspace_archive_file.trim().is_empty() {
        let q_ws_arc = shell_quote(workspace_archive_file);
        script.push_str(&format!(
            "\nif [ -f {q_ws_arc} ]; then\n    \
             echo '@@STAGE:82:Extracting workspace archive...'\n    \
             tar -xzf {q_ws_arc} -C {q_ws}\n\
             fi"
        ));
    }

    script.push_str("\necho '@@STAGE:100:Environment import complete.'\n");
    script
}
