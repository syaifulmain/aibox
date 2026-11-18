//! Backup export, export-and-wipe, and import shell script builders.

use crate::backup::export::{manifest_path_for, CompressionCodec};
use crate::snapshot::{wipe_environment_script, workspace_create_snippet};
use crate::util::paths::CONTAINER_WORKSPACE_PATH;
use crate::util::shell::shell_quote;

/// Builds the common `incus export` and optional workspace tarball steps (legacy gzip defaults).
pub fn build_export_steps(
    name: &str,
    backup_file: &str,
    workspace_path: &str,
    workspace_archive_file: &str,
) -> String {
    build_export_steps_with_options(
        name,
        backup_file,
        workspace_path,
        workspace_archive_file,
        CompressionCodec::Gzip,
        false,
    )
}

/// Builds `incus export` and optional workspace tarball steps with selectable compression (`zstd -T0`
/// or `gzip`) and optional `<backup_file>.manifest.json` SHA-256 sidecar generation.
pub fn build_export_steps_with_options(
    name: &str,
    backup_file: &str,
    workspace_path: &str,
    workspace_archive_file: &str,
    compression: CompressionCodec,
    write_manifest: bool,
) -> String {
    let q_name = shell_quote(name);
    let q_backup = shell_quote(backup_file);
    let comp_flag = compression.incus_flag();
    let mut script = format!(
        "set -e\n\
         mkdir -p \"$(dirname {q_backup})\"\n\
         echo '@@STAGE:15:Exporting instance...'\n\
         incus export {q_name} {q_backup} --compression {comp_flag}\n\
         echo '@@STAGE:60:Instance archive written.'"
    );

    if !workspace_path.trim().is_empty() && !workspace_archive_file.trim().is_empty() {
        let q_ws = shell_quote(workspace_path);
        let q_ws_out = shell_quote(workspace_archive_file);
        let tar_cmd = match compression {
            CompressionCodec::Zstd => {
                format!("tar --use-compress-program='zstd -T0' -cf {q_ws_out} -C {q_ws} .")
            }
            CompressionCodec::Gzip => format!("tar -czf {q_ws_out} -C {q_ws} ."),
        };
        script.push_str(&format!(
            "\nif [ -d {q_ws} ]; then\n    \
             echo '@@STAGE:72:Archiving workspace directory...'\n    \
             {tar_cmd}\n\
             fi"
        ));
    }

    if write_manifest {
        let manifest_path = manifest_path_for(backup_file);
        let q_manifest = shell_quote(&manifest_path.to_string_lossy());
        let q_comp = shell_quote(compression.as_str());
        let ws_block = if !workspace_path.trim().is_empty()
            && !workspace_archive_file.trim().is_empty()
        {
            let q_ws_out = shell_quote(workspace_archive_file);
            format!(
                "WS_JSON=''\n\
                 if [ -f {q_ws_out} ]; then\n    \
                 WS_SHA=$(sha256sum {q_ws_out} | awk '{{print $1}}')\n    \
                 WS_SIZE=$(stat -c%s {q_ws_out})\n    \
                 WS_BASE=$(basename {q_ws_out})\n    \
                 WS_JSON=$(printf ',\\n  \"workspaceArchiveFile\": \"%s\",\\n  \"workspaceArchiveSha256\": \"%s\",\\n  \"workspaceArchiveSizeBytes\": %s' \"$WS_BASE\" \"$WS_SHA\" \"$WS_SIZE\")\n\
                 fi"
            )
        } else {
            "WS_JSON=''".to_string()
        };
        script.push_str(&format!(
            "\necho '@@STAGE:90:Writing backup manifest and SHA-256 checksum...'\n\
             ARC_SHA=$(sha256sum {q_backup} | awk '{{print $1}}')\n\
             ARC_SIZE=$(stat -c%s {q_backup})\n\
             ARC_BASE=$(basename {q_backup})\n\
             CREATED_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)\n\
             {ws_block}\n\
             printf '{{\\n  \"version\": 1,\\n  \"envName\": \"%s\",\\n  \"createdAt\": \"%s\",\\n  \"compression\": \"%s\",\\n  \"archiveFile\": \"%s\",\\n  \"archiveSha256\": \"%s\",\\n  \"archiveSizeBytes\": %s%s\\n}}\\n' {q_name} \"$CREATED_AT\" {q_comp} \"$ARC_BASE\" \"$ARC_SHA\" \"$ARC_SIZE\" \"$WS_JSON\" > {q_manifest}"
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

/// Builds the shell script for exporting an environment with explicit compression and manifest options.
pub fn export_environment_script_with_options(
    name: &str,
    backup_file: &str,
    workspace_path: &str,
    workspace_archive_file: &str,
    compression: CompressionCodec,
    write_manifest: bool,
) -> String {
    let mut script = build_export_steps_with_options(
        name,
        backup_file,
        workspace_path,
        workspace_archive_file,
        compression,
        write_manifest,
    );
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
    export_and_wipe_environment_script_with_options(
        name,
        backup_file,
        workspace_path,
        workspace_archive_file,
        delete_workspace_files,
        sessions_env_dir,
        environment_secrets_path,
        CompressionCodec::Gzip,
        false,
    )
}

/// Builds the export-and-wipe shell script with selectable compression and manifest generation.
#[allow(clippy::too_many_arguments)]
pub fn export_and_wipe_environment_script_with_options(
    name: &str,
    backup_file: &str,
    workspace_path: &str,
    workspace_archive_file: &str,
    delete_workspace_files: bool,
    sessions_env_dir: &str,
    environment_secrets_path: &str,
    compression: CompressionCodec,
    write_manifest: bool,
) -> String {
    let q_backup = shell_quote(backup_file);
    let mut script = build_export_steps_with_options(
        name,
        backup_file,
        workspace_path,
        workspace_archive_file,
        compression,
        write_manifest,
    );
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
    import_environment_script_with_options(
        backup_file,
        target_name,
        workspace_path,
        workspace_archive_file,
        false,
    )
}

/// Builds the shell script for importing an environment backup archive, supporting `.tar.zst`
/// and `.tar.gz` and optional shell-level manifest SHA-256 verification.
pub fn import_environment_script_with_options(
    backup_file: &str,
    target_name: &str,
    workspace_path: &str,
    workspace_archive_file: &str,
    verify_manifest: bool,
) -> String {
    let q_backup = shell_quote(backup_file);
    let q_name = shell_quote(target_name);
    let q_ws = shell_quote(workspace_path);
    let ws_create = workspace_create_snippet(workspace_path);
    let q_container_ws = shell_quote(CONTAINER_WORKSPACE_PATH);

    let mut script = String::from("set -e\n");
    if verify_manifest {
        let q_manifest = shell_quote(&manifest_path_for(backup_file).to_string_lossy());
        script.push_str(&format!(
            "if [ -f {q_manifest} ]; then\n    \
             echo '@@STAGE:10:Verifying backup archive checksum...'\n    \
             EXP_SHA=$(sed -n 's/.*\"archiveSha256\"[[:space:]]*:[[:space:]]*\"\\([^\"]*\\)\".*/\\1/p' {q_manifest} | head -n1)\n    \
             if [ -n \"$EXP_SHA\" ]; then\n        \
             ACT_SHA=$(sha256sum {q_backup} | awk '{{print $1}}')\n        \
             if [ \"$EXP_SHA\" != \"$ACT_SHA\" ]; then\n            \
             echo \"Backup archive checksum mismatch: expected $EXP_SHA, got $ACT_SHA\" >&2\n            \
             exit 1\n        \
             fi\n    \
             fi\n\
             fi\n"
        ));
    }

    script.push_str(&format!(
        "echo '@@STAGE:15:Preparing target workspace...'\n\
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
    ));

    if !workspace_archive_file.trim().is_empty() {
        let q_ws_arc = shell_quote(workspace_archive_file);
        let extract_cmd = if workspace_archive_file.ends_with(".tar.zst") {
            format!("tar --use-compress-program='zstd -d' -xf {q_ws_arc} -C {q_ws}")
        } else {
            format!("tar -xzf {q_ws_arc} -C {q_ws}")
        };
        script.push_str(&format!(
            "\nif [ -f {q_ws_arc} ]; then\n    \
             echo '@@STAGE:82:Extracting workspace archive...'\n    \
             {extract_cmd}\n\
             fi"
        ));
    }

    script.push_str("\necho '@@STAGE:100:Environment import complete.'\n");
    script
}
