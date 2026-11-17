use aibox_core::snapshot;
use aibox_core::util::json::to_json_compact;

#[cxx::bridge(namespace = "aibox::ffi")]
mod ffi {
    extern "Rust" {
        fn parse_snapshot_sizes_json(json: &[u8]) -> String;
        fn parse_du_bytes_output_json(output: &str) -> String;
        fn merge_snapshot_size_map_json(existing_json: &str, cached_json: &str) -> String;

        fn is_auto_snapshot_name(snapshot_name: &str) -> bool;
        fn parse_auto_snapshot_timestamp_iso(snapshot_name: &str) -> String;
        fn should_create_auto_snapshot(
            snapshots: &[String],
            now_epoch: i64,
            min_interval_seconds: i64,
        ) -> bool;
        fn auto_snapshots_to_prune(snapshots: &[String], keep_count: i32) -> Vec<String>;
        fn auto_snapshot_shell_snippet(
            name: &str,
            new_snap_name: &str,
            prune_names: &[String],
            workspace_path: &str,
            include_workspace: bool,
        ) -> String;
        #[allow(clippy::too_many_arguments)]
        fn decide_auto_snapshot(
            name: &str,
            is_template: bool,
            auto_snapshot_enabled: bool,
            snapshots: &[String],
            workspace_path: &str,
            workspace_snapshot_enabled: bool,
            keep_count: i32,
            has_last_attempt: bool,
            last_attempt_epoch: i64,
            now_epoch: i64,
            now_local_stamp: &str,
        ) -> String;

        fn workspace_snapshot_dir(workspace_path: &str, env_name: &str) -> String;
        fn is_btrfs_filesystem(path: &str) -> bool;
        fn is_btrfs_subvolume(path: &str) -> bool;
        fn list_workspace_snapshots(workspace_path: &str, env_name: &str) -> Vec<String>;
        fn find_orphan_workspace_snapshots(
            incus_snapshots: &[String],
            ws_snapshots: &[String],
        ) -> Vec<String>;
        fn workspace_create_snippet(workspace_path: &str) -> String;
        fn workspace_mount_migration_script(env_name: &str) -> String;
        fn container_workspace_path_for(name: &str, demo_mode: bool) -> String;
        fn workspace_snapshot_snippet(
            workspace_path: &str,
            snap_dir: &str,
            snap_name: &str,
        ) -> String;
        fn workspace_delete_snapshot_snippet(snap_path: &str, warn_only: bool) -> String;
        fn workspace_restore_snippet(
            workspace_path: &str,
            snap_dir: &str,
            snap_name: &str,
            safety_epoch: i64,
        ) -> String;
        fn convert_workspace_to_subvolume_script(env_name: &str, workspace_path: &str) -> String;

        fn create_snapshot_script(
            name: &str,
            snap_name: &str,
            workspace_path: &str,
            include_workspace: bool,
        ) -> String;
        fn restore_snapshot_script(
            name: &str,
            snap_name: &str,
            workspace_path: &str,
            mode: &str,
            include_workspace: bool,
            safety_epoch: i64,
        ) -> String;
        fn delete_snapshot_script(
            name: &str,
            snap_name: &str,
            workspace_path: &str,
            has_incus_snap: bool,
            has_workspace_snap: bool,
        ) -> String;
        fn wipe_environment_script(
            name: &str,
            workspace_path: &str,
            delete_workspace_files: bool,
            sessions_env_dir: &str,
            environment_secrets_path: &str,
        ) -> String;
    }
}

fn parse_snapshot_sizes_json(json: &[u8]) -> String {
    let map = snapshot::parse_snapshot_sizes_json(json);
    to_json_compact(&map).unwrap_or_else(|_| "{}".to_string())
}

fn parse_du_bytes_output_json(output: &str) -> String {
    let map = snapshot::parse_du_bytes_output(output);
    to_json_compact(&map).unwrap_or_else(|_| "{}".to_string())
}

fn merge_snapshot_size_map_json(existing_json: &str, cached_json: &str) -> String {
    snapshot::merge_snapshot_size_map_json(existing_json, cached_json)
}

fn is_auto_snapshot_name(snapshot_name: &str) -> bool {
    snapshot::is_auto_snapshot_name(snapshot_name)
}

fn parse_auto_snapshot_timestamp_iso(snapshot_name: &str) -> String {
    snapshot::parse_auto_snapshot_timestamp_iso(snapshot_name)
}

fn should_create_auto_snapshot(
    snapshots: &[String],
    now_epoch: i64,
    min_interval_seconds: i64,
) -> bool {
    snapshot::should_create_auto_snapshot(snapshots, now_epoch, min_interval_seconds)
}

fn auto_snapshots_to_prune(snapshots: &[String], keep_count: i32) -> Vec<String> {
    snapshot::auto_snapshots_to_prune(snapshots, keep_count)
}

fn auto_snapshot_shell_snippet(
    name: &str,
    new_snap_name: &str,
    prune_names: &[String],
    workspace_path: &str,
    include_workspace: bool,
) -> String {
    snapshot::auto_snapshot_shell_snippet(
        name,
        new_snap_name,
        prune_names,
        workspace_path,
        include_workspace,
    )
}

#[allow(clippy::too_many_arguments)]
fn decide_auto_snapshot(
    name: &str,
    is_template: bool,
    auto_snapshot_enabled: bool,
    snapshots: &[String],
    workspace_path: &str,
    workspace_snapshot_enabled: bool,
    keep_count: i32,
    has_last_attempt: bool,
    last_attempt_epoch: i64,
    now_epoch: i64,
    now_local_stamp: &str,
) -> String {
    let last_attempt = if has_last_attempt {
        Some(last_attempt_epoch)
    } else {
        None
    };
    snapshot::decide_auto_snapshot(
        name,
        is_template,
        auto_snapshot_enabled,
        snapshots,
        workspace_path,
        workspace_snapshot_enabled,
        keep_count,
        last_attempt,
        now_epoch,
        now_local_stamp,
    )
    .unwrap_or_default()
}

fn workspace_snapshot_dir(workspace_path: &str, env_name: &str) -> String {
    snapshot::workspace_snapshot_dir(workspace_path, env_name)
}

fn is_btrfs_filesystem(path: &str) -> bool {
    snapshot::is_btrfs_filesystem(path)
}

fn is_btrfs_subvolume(path: &str) -> bool {
    snapshot::is_btrfs_subvolume(path)
}

fn list_workspace_snapshots(workspace_path: &str, env_name: &str) -> Vec<String> {
    snapshot::list_workspace_snapshots(workspace_path, env_name)
}

fn find_orphan_workspace_snapshots(
    incus_snapshots: &[String],
    ws_snapshots: &[String],
) -> Vec<String> {
    snapshot::find_orphan_workspace_snapshots(incus_snapshots, ws_snapshots)
}

fn workspace_create_snippet(workspace_path: &str) -> String {
    snapshot::workspace_create_snippet(workspace_path)
}

fn workspace_mount_migration_script(env_name: &str) -> String {
    snapshot::workspace_mount_migration_script(env_name)
}

fn container_workspace_path_for(name: &str, demo_mode: bool) -> String {
    snapshot::container_workspace_path_for(name, demo_mode)
}

fn workspace_snapshot_snippet(workspace_path: &str, snap_dir: &str, snap_name: &str) -> String {
    snapshot::workspace_snapshot_snippet(workspace_path, snap_dir, snap_name)
}

fn workspace_delete_snapshot_snippet(snap_path: &str, warn_only: bool) -> String {
    snapshot::workspace_delete_snapshot_snippet(snap_path, warn_only)
}

fn workspace_restore_snippet(
    workspace_path: &str,
    snap_dir: &str,
    snap_name: &str,
    safety_epoch: i64,
) -> String {
    snapshot::workspace_restore_snippet(workspace_path, snap_dir, snap_name, safety_epoch)
}

fn convert_workspace_to_subvolume_script(env_name: &str, workspace_path: &str) -> String {
    snapshot::convert_workspace_to_subvolume_script(env_name, workspace_path)
}

fn create_snapshot_script(
    name: &str,
    snap_name: &str,
    workspace_path: &str,
    include_workspace: bool,
) -> String {
    snapshot::create_snapshot_script(name, snap_name, workspace_path, include_workspace)
}

fn restore_snapshot_script(
    name: &str,
    snap_name: &str,
    workspace_path: &str,
    mode: &str,
    include_workspace: bool,
    safety_epoch: i64,
) -> String {
    snapshot::restore_snapshot_script(
        name,
        snap_name,
        workspace_path,
        mode,
        include_workspace,
        safety_epoch,
    )
}

fn delete_snapshot_script(
    name: &str,
    snap_name: &str,
    workspace_path: &str,
    has_incus_snap: bool,
    has_workspace_snap: bool,
) -> String {
    snapshot::delete_snapshot_script(
        name,
        snap_name,
        workspace_path,
        has_incus_snap,
        has_workspace_snap,
    )
}

fn wipe_environment_script(
    name: &str,
    workspace_path: &str,
    delete_workspace_files: bool,
    sessions_env_dir: &str,
    environment_secrets_path: &str,
) -> String {
    snapshot::wipe_environment_script(
        name,
        workspace_path,
        delete_workspace_files,
        sessions_env_dir,
        environment_secrets_path,
    )
}
