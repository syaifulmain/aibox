//! Snapshot management: auto-snapshot policy, Btrfs workspace snapshots, sizes, and scripts.

pub mod auto;
pub mod scripts;
pub mod sizes;
pub mod workspace;

pub use auto::{
    auto_snapshot_shell_snippet, auto_snapshots_to_prune, auto_snapshots_to_prune_tiered,
    decide_auto_snapshot, is_auto_snapshot_name, parse_auto_snapshot_timestamp,
    parse_auto_snapshot_timestamp_epoch, parse_auto_snapshot_timestamp_iso,
    should_create_auto_snapshot, RetentionPolicy,
};
pub use scripts::{
    create_snapshot_script, delete_snapshot_script, restore_snapshot_script,
    wipe_environment_script,
};
pub use sizes::{
    apply_cached_snapshot_sizes, compute_workspace_snapshot_sizes, fetch_incus_snapshot_sizes,
    merge_snapshot_size_map, merge_snapshot_size_map_json, parse_btrfs_qgroup_bytes,
    parse_du_bytes_output, parse_snapshot_sizes_json, SnapshotSizeCache,
};
pub use workspace::{
    container_workspace_path_for, convert_workspace_to_subvolume_script,
    find_orphan_workspace_snapshots, is_btrfs_filesystem, is_btrfs_subvolume,
    list_workspace_snapshots, parse_btrfs_subvolume_generation,
    parse_container_workspace_path_output, workspace_create_snippet,
    workspace_delete_snapshot_snippet, workspace_mount_migration_script, workspace_restore_snippet,
    workspace_snapshot_dir, workspace_snapshot_snippet, workspace_unchanged_since_last_snapshot,
};
