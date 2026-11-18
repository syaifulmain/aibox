//! Snapshot size parsing, `btrfs qgroup` / `du` measurement, and read-only size caching.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Map, Value};

use crate::incus::list::Environment;
use crate::snapshot::workspace::is_btrfs_filesystem;
use crate::util::format::format_bytes;
use crate::util::json::to_json_compact;

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

/// Parses `btrfs qgroup show -r --raw <subvol>` output and returns referenced (`rfer`) bytes.
pub fn parse_btrfs_qgroup_bytes(output: &str) -> Option<i64> {
    for line in output.lines().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("qgroupid")
            || trimmed.starts_with("Qgroupid")
            || trimmed.starts_with("---")
        {
            continue;
        }
        let cols: Vec<&str> = trimmed.split_whitespace().collect();
        if cols.len() >= 2 && cols[0].contains('/') {
            if let Ok(rfer) = cols[1].parse::<i64>() {
                if rfer >= 0 {
                    return Some(rfer);
                }
            }
        }
    }
    None
}

/// Fetches Incus snapshot sizes for `env_name` via `incus query`.
pub fn fetch_incus_snapshot_sizes(env_name: &str) -> BTreeMap<String, String> {
    if env_name.trim().is_empty() {
        return BTreeMap::new();
    }
    let query_path = format!("/1.0/instances/{env_name}/snapshots?recursion=1");
    let Ok(out) = Command::new("incus")
        .args(["query", &query_path])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    else {
        return BTreeMap::new();
    };
    if !out.status.success() {
        return BTreeMap::new();
    }
    parse_snapshot_sizes_json(&out.stdout)
}

/// Computes workspace snapshot sizes using `btrfs qgroup` when available, falling back to `du -sb`.
pub fn compute_workspace_snapshot_sizes(
    snap_dir: &str,
    snap_names: &[String],
) -> BTreeMap<String, String> {
    let mut sizes = BTreeMap::new();
    if snap_dir.trim().is_empty() || snap_names.is_empty() {
        return sizes;
    }

    let existing: Vec<(String, String)> = snap_names
        .iter()
        .filter_map(|name| {
            let p = format!("{snap_dir}/{name}");
            Path::new(&p).is_dir().then(|| (name.clone(), p))
        })
        .collect();
    if existing.is_empty() {
        return sizes;
    }

    let mut remaining_start = 0usize;
    if is_btrfs_filesystem(snap_dir) {
        for (idx, (name, path)) in existing.iter().enumerate() {
            let Ok(out) = Command::new("btrfs")
                .args(["qgroup", "show", "-r", "--raw", path])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output()
            else {
                break;
            };
            if !out.status.success() {
                break;
            }
            let stdout = String::from_utf8_lossy(&out.stdout);
            let Some(bytes) = parse_btrfs_qgroup_bytes(&stdout) else {
                break;
            };
            sizes.insert(name.clone(), format_bytes(bytes));
            remaining_start = idx + 1;
        }
    }

    if remaining_start < existing.len() {
        let mut cmd = Command::new("du");
        cmd.arg("-sb");
        for (_, path) in &existing[remaining_start..] {
            cmd.arg(path);
        }
        if let Ok(out) = cmd.stdout(Stdio::piped()).stderr(Stdio::null()).output() {
            if out.status.success() {
                let du_sizes = parse_du_bytes_output(&String::from_utf8_lossy(&out.stdout));
                for (k, v) in du_sizes {
                    sizes.insert(k, v);
                }
            }
        }
    }

    sizes
}

/// Per-environment cache of read-only Incus and workspace snapshot sizes.
#[derive(Debug, Clone, Default)]
pub struct SnapshotSizeCache {
    pub incus: BTreeMap<String, BTreeMap<String, String>>,
    pub workspace: BTreeMap<String, BTreeMap<String, String>>,
}

impl SnapshotSizeCache {
    /// Returns Incus snapshot names for `env_name` not yet present in the cache.
    pub fn missing_incus_snapshots(&self, env_name: &str, snapshots: &[String]) -> Vec<String> {
        let cached = self.incus.get(env_name);
        snapshots
            .iter()
            .filter(|s| !cached.is_some_and(|m| m.contains_key(s.as_str())))
            .cloned()
            .collect()
    }

    /// Returns workspace snapshot names for `env_name` not yet present in the cache.
    pub fn missing_workspace_snapshots(
        &self,
        env_name: &str,
        ws_snapshots: &[String],
    ) -> Vec<String> {
        let cached = self.workspace.get(env_name);
        ws_snapshots
            .iter()
            .filter(|s| !cached.is_some_and(|m| m.contains_key(s.as_str())))
            .cloned()
            .collect()
    }

    /// Inserts computed Incus snapshot sizes for `env_name`. Returns `true` if any entry changed.
    pub fn insert_incus(&mut self, env_name: &str, sizes: BTreeMap<String, String>) -> bool {
        if sizes.is_empty() {
            return false;
        }
        let entry = self.incus.entry(env_name.to_string()).or_default();
        let mut changed = false;
        for (k, v) in sizes {
            if entry.get(&k) != Some(&v) {
                entry.insert(k, v);
                changed = true;
            }
        }
        changed
    }

    /// Inserts computed workspace snapshot sizes for `env_name`. Returns `true` if any entry changed.
    pub fn insert_workspace(&mut self, env_name: &str, sizes: BTreeMap<String, String>) -> bool {
        if sizes.is_empty() {
            return false;
        }
        let entry = self.workspace.entry(env_name.to_string()).or_default();
        let mut changed = false;
        for (k, v) in sizes {
            if entry.get(&k) != Some(&v) {
                entry.insert(k, v);
                changed = true;
            }
        }
        changed
    }

    /// Drops cached entries for snapshots that no longer exist on `env_name`.
    pub fn prune_removed(&mut self, env_name: &str, live_incus: &[String], live_ws: &[String]) {
        if let Some(map) = self.incus.get_mut(env_name) {
            let live: BTreeSet<&str> = live_incus.iter().map(String::as_str).collect();
            map.retain(|k, _| live.contains(k.as_str()));
        }
        if let Some(map) = self.workspace.get_mut(env_name) {
            let live: BTreeSet<&str> = live_ws.iter().map(String::as_str).collect();
            map.retain(|k, _| live.contains(k.as_str()));
        }
    }

    /// Merges cached sizes into a typed `Environment` struct. Returns `true` if modified.
    pub fn apply_to_environment(&self, env: &mut Environment) -> bool {
        let mut changed = false;
        if let Some(cached) = self.incus.get(&env.name) {
            for (k, v) in cached {
                if env.snapshot_sizes.get(k) != Some(v) {
                    env.snapshot_sizes.insert(k.clone(), v.clone());
                    changed = true;
                }
            }
        }
        if let Some(ws_cached) = self.workspace.get(&env.name) {
            for (k, v) in ws_cached {
                if env.workspace_snapshot_sizes.get(k) != Some(v) {
                    env.workspace_snapshot_sizes.insert(k.clone(), v.clone());
                    changed = true;
                }
            }
        }
        changed
    }

    /// Merges cached sizes into a JSON environment list.
    pub fn apply_to_envs(&self, envs: &mut [Value]) -> bool {
        apply_cached_snapshot_sizes(envs, &self.incus, &self.workspace)
    }
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
    use std::fs;

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
    fn parses_btrfs_qgroup_output_and_handles_missing_quotas() {
        let qgroup_out = "\
qgroupid         rfer         excl
--------         ----         ----
0/258        52428800      4194304
";
        assert_eq!(parse_btrfs_qgroup_bytes(qgroup_out), Some(52_428_800));
        assert_eq!(parse_btrfs_qgroup_bytes(""), None);
        assert_eq!(
            parse_btrfs_qgroup_bytes("ERROR: quotas not enabled\n"),
            None
        );
    }

    #[test]
    fn snapshot_size_cache_tracks_missing_prunes_and_computes_du() {
        let tmp = std::env::temp_dir().join(format!(
            "aibox-size-cache-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let snap_a = tmp.join("snap-a");
        fs::create_dir_all(&snap_a).expect("mkdir");
        fs::write(snap_a.join("file.bin"), vec![0u8; 4096]).expect("write");

        let mut cache = SnapshotSizeCache::default();
        let missing = cache
            .missing_workspace_snapshots("dev-box", &["snap-a".to_string(), "snap-b".to_string()]);
        assert_eq!(missing, vec!["snap-a".to_string(), "snap-b".to_string()]);

        let computed = compute_workspace_snapshot_sizes(&tmp.to_string_lossy(), &missing);
        assert!(computed.contains_key("snap-a"));
        assert!(!computed.contains_key("snap-b"));

        assert!(cache.insert_workspace("dev-box", computed));
        assert_eq!(
            cache.missing_workspace_snapshots(
                "dev-box",
                &["snap-a".to_string(), "snap-b".to_string()]
            ),
            vec!["snap-b".to_string()]
        );

        cache.prune_removed("dev-box", &[], &[]);
        assert!(cache.workspace.get("dev-box").is_some_and(|m| m.is_empty()));

        let _ = fs::remove_dir_all(&tmp);
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
