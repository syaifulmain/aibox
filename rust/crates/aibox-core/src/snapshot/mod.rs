//! Snapshot management: auto-snapshot policy, Btrfs workspace snapshots, sizes, and scripts.

pub mod auto;
pub mod scripts;
pub mod workspace;

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::util::format::format_bytes;
use crate::util::json::to_json_compact;

pub use auto::{
    auto_snapshot_shell_snippet, auto_snapshots_to_prune, decide_auto_snapshot,
    is_auto_snapshot_name, parse_auto_snapshot_timestamp, parse_auto_snapshot_timestamp_epoch,
    parse_auto_snapshot_timestamp_iso, should_create_auto_snapshot,
};
pub use scripts::{
    create_snapshot_script, delete_snapshot_script, restore_snapshot_script,
    wipe_environment_script,
};
pub use workspace::{
    container_workspace_path_for, convert_workspace_to_subvolume_script,
    find_orphan_workspace_snapshots, is_btrfs_filesystem, is_btrfs_subvolume,
    list_workspace_snapshots, parse_container_workspace_path_output, workspace_create_snippet,
    workspace_delete_snapshot_snippet, workspace_mount_migration_script, workspace_restore_snippet,
    workspace_snapshot_dir, workspace_snapshot_snippet,
};

/// Parses Incus `/1.0/instances/<name>/snapshots?recursion=1` JSON into `{name: formattedSize}`.
pub fn parse_snapshot_sizes_json(json: &[u8]) -> BTreeMap<String, String> {
    let mut sizes = BTreeMap::new();
    if String::from_utf8_lossy(json).trim().is_empty() {
        return sizes;
    }
    let Ok(Value::Array(arr)) = serde_json::from_slice::<Value>(json) else {
        return sizes;
    };
    for val in arr {
        let Some(obj) = val.as_object() else {
            continue;
        };
        let snap_name = obj.get("name").and_then(Value::as_str).unwrap_or("");
        let snap_bytes = obj.get("size").and_then(Value::as_i64).unwrap_or(-1);
        if !snap_name.is_empty() && snap_bytes > 0 {
            sizes.insert(snap_name.to_string(), format_bytes(snap_bytes));
        }
    }
    sizes
}

/// Parses `du -sb` output (`<bytes>\t<path>`) into `{basename: formattedSize}`.
pub fn parse_du_bytes_output(output: &str) -> BTreeMap<String, String> {
    let mut sizes = BTreeMap::new();
    for line in output.split('\n').filter(|l| !l.is_empty()) {
        let Some(tab_idx) = line.find('\t') else {
            continue;
        };
        if tab_idx == 0 {
            continue;
        }
        let Ok(bytes) = line[..tab_idx].trim().parse::<i64>() else {
            continue;
        };
        let path = line[tab_idx + 1..].trim();
        let name = path.rsplit('/').next().unwrap_or("");
        if bytes >= 0 && !name.is_empty() {
            sizes.insert(name.to_string(), format_bytes(bytes));
        }
    }
    sizes
}

/// Merges cached `snapshotSizes` and `workspaceSnapshotSizes` maps into an environment JSON list.
/// Returns `true` if any environment was modified.
pub fn apply_cached_snapshot_sizes(
    envs: &mut [Value],
    snapshot_sizes_cache: &BTreeMap<String, BTreeMap<String, String>>,
    ws_snapshot_sizes_cache: &BTreeMap<String, BTreeMap<String, String>>,
) -> bool {
    let mut any_changed = false;
    for env_val in envs.iter_mut() {
        let Some(env) = env_val.as_object_mut() else {
            continue;
        };
        let name = env
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let mut changed = false;
        if let Some(cached) = snapshot_sizes_cache.get(&name) {
            if !cached.is_empty() {
                let mut merged = env
                    .get("snapshotSizes")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                for (k, v) in cached {
                    merged.insert(k.clone(), Value::String(v.clone()));
                }
                env.insert("snapshotSizes".to_string(), Value::Object(merged));
                changed = true;
            }
        }
        if let Some(ws_cached) = ws_snapshot_sizes_cache.get(&name) {
            if !ws_cached.is_empty() {
                let mut merged = env
                    .get("workspaceSnapshotSizes")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                for (k, v) in ws_cached {
                    merged.insert(k.clone(), Value::String(v.clone()));
                }
                env.insert("workspaceSnapshotSizes".to_string(), Value::Object(merged));
                changed = true;
            }
        }
        if changed {
            any_changed = true;
        }
    }
    any_changed
}

/// Merges `existing` and `cached` snapshot size maps (used by the Stage 1 C++ facade).
pub fn merge_snapshot_size_map(
    existing: &BTreeMap<String, String>,
    cached: &BTreeMap<String, String>,
) -> Map<String, Value> {
    let mut merged = Map::new();
    for (k, v) in existing {
        merged.insert(k.clone(), Value::String(v.clone()));
    }
    for (k, v) in cached {
        merged.insert(k.clone(), Value::String(v.clone()));
    }
    merged
}

/// JSON-string wrapper around `merge_snapshot_size_map` for the Stage 1 C++ facade.
pub fn merge_snapshot_size_map_json(existing_json: &str, cached_json: &str) -> String {
    let existing: BTreeMap<String, String> =
        serde_json::from_str(existing_json).unwrap_or_default();
    let cached: BTreeMap<String, String> = serde_json::from_str(cached_json).unwrap_or_default();
    let merged = merge_snapshot_size_map(&existing, &cached);
    to_json_compact(&merged).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_sizes_json_matches_qt_test() {
        let snap_json = br#"[
            { "name": "clean-sdk", "size": 52428800 },
            { "name": "empty-unknown", "size": -1 }
        ]"#;
        let sizes = parse_snapshot_sizes_json(snap_json);
        assert_eq!(sizes.get("clean-sdk").map(String::as_str), Some("50 MiB"));
        assert!(!sizes.contains_key("empty-unknown"));
    }

    #[test]
    fn du_bytes_output_matches_qt_test() {
        let du = "1048576\t/tmp/ws/.snapshots/dev-box/snap-1\n2048\t/tmp/ws/.snapshots/dev-box/pre-restore-123\n";
        let sizes = parse_du_bytes_output(du);
        assert_eq!(sizes.get("snap-1").map(String::as_str), Some("1 MiB"));
        assert_eq!(
            sizes.get("pre-restore-123").map(String::as_str),
            Some("2 KiB")
        );
    }

    #[test]
    fn apply_cached_snapshot_sizes_merges_without_dropping_existing() {
        let mut envs = vec![serde_json::json!({
            "name": "dev-box",
            "snapshotSizes": { "old": "1 KiB" }
        })];
        let mut snap_cache = BTreeMap::new();
        let mut dev_snaps = BTreeMap::new();
        dev_snaps.insert("new".to_string(), "2 MiB".to_string());
        snap_cache.insert("dev-box".to_string(), dev_snaps);
        let ws_cache = BTreeMap::new();

        assert!(apply_cached_snapshot_sizes(
            &mut envs,
            &snap_cache,
            &ws_cache
        ));
        let obj = envs[0].as_object().expect("obj");
        let sizes = obj["snapshotSizes"].as_object().expect("sizes");
        assert_eq!(sizes["old"], "1 KiB");
        assert_eq!(sizes["new"], "2 MiB");
    }
}
