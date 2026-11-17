//! Auto-snapshot naming, interval checks, pruning, and shell snippet generation.

use chrono::{DateTime, Local, NaiveDateTime, TimeZone};

use crate::snapshot::workspace::{
    workspace_delete_snapshot_snippet, workspace_snapshot_dir, workspace_snapshot_snippet,
};
use crate::util::shell::shell_quote;

/// Returns `true` if `snapshot_name` uses the `auto-` prefix.
pub fn is_auto_snapshot_name(snapshot_name: &str) -> bool {
    snapshot_name.starts_with("auto-")
}

/// Parses `auto-yyyyMMdd-HHmmss` in local time, matching `QDateTime::fromString(..., "yyyyMMdd-HHmmss")`.
pub fn parse_auto_snapshot_timestamp(snapshot_name: &str) -> Option<DateTime<Local>> {
    if !is_auto_snapshot_name(snapshot_name) {
        return None;
    }
    let ts = &snapshot_name[5..];
    let ndt = NaiveDateTime::parse_from_str(ts, "%Y%m%d-%H%M%S").ok()?;
    Local
        .from_local_datetime(&ndt)
        .earliest()
        .or_else(|| Local.from_local_datetime(&ndt).latest())
}

/// Formats a parsed auto-snapshot timestamp as `Qt::ISODate` (local time, no offset) or empty.
pub fn parse_auto_snapshot_timestamp_iso(snapshot_name: &str) -> String {
    match parse_auto_snapshot_timestamp(snapshot_name) {
        Some(dt) => dt.format("%Y-%m-%dT%H:%M:%S").to_string(),
        None => String::new(),
    }
}

/// Returns `Some(unix_epoch_seconds)` if `snapshot_name` has a valid auto-snapshot timestamp.
pub fn parse_auto_snapshot_timestamp_epoch(snapshot_name: &str) -> Option<i64> {
    parse_auto_snapshot_timestamp(snapshot_name).map(|dt| dt.timestamp())
}

/// Returns `true` when no valid auto-snapshot exists or the newest is at least `min_interval_seconds` old.
pub fn should_create_auto_snapshot(
    snapshots: &[String],
    now_epoch: i64,
    min_interval_seconds: i64,
) -> bool {
    let mut newest: Option<i64> = None;
    for snap in snapshots {
        if let Some(dt) = parse_auto_snapshot_timestamp(snap) {
            let epoch = dt.timestamp();
            if newest.is_none_or(|n| epoch > n) {
                newest = Some(epoch);
            }
        }
    }
    match newest {
        None => true,
        Some(n) => (now_epoch - n) >= min_interval_seconds.max(0),
    }
}

/// Returns oldest `auto-*` snapshot names to prune so at most `keep_count` remain.
pub fn auto_snapshots_to_prune(snapshots: &[String], keep_count: i32) -> Vec<String> {
    let mut autos: Vec<(i64, String)> = snapshots
        .iter()
        .filter(|s| is_auto_snapshot_name(s))
        .map(|s| {
            let epoch = parse_auto_snapshot_timestamp(s)
                .map(|dt| dt.timestamp())
                .unwrap_or(0);
            (epoch, s.clone())
        })
        .collect();

    let keep = keep_count.max(0) as usize;
    if autos.len() <= keep {
        return Vec::new();
    }

    autos.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let drop_count = autos.len() - keep;
    autos.into_iter().take(drop_count).map(|(_, n)| n).collect()
}

/// Builds the shell snippet that creates an auto-snapshot and prunes older ones.
pub fn auto_snapshot_shell_snippet(
    name: &str,
    new_snap_name: &str,
    prune_names: &[String],
    workspace_path: &str,
    include_workspace: bool,
) -> String {
    if name.is_empty() || new_snap_name.is_empty() {
        return String::new();
    }
    let mut lines = Vec::new();
    lines.push(format!(
        "incus snapshot create {} {} >/dev/null 2>&1 || true",
        shell_quote(name),
        shell_quote(new_snap_name)
    ));

    let snap_dir = if include_workspace {
        workspace_snapshot_dir(workspace_path, name)
    } else {
        String::new()
    };
    if include_workspace && !snap_dir.is_empty() {
        let ws_snap = workspace_snapshot_snippet(workspace_path, &snap_dir, new_snap_name);
        if !ws_snap.is_empty() {
            let warn =
                format!("Warning: workspace auto-snapshot failed for {name}/{new_snap_name}");
            lines.push(format!(
                "(\n{ws_snap}\n) || echo {} >&2",
                shell_quote(&warn)
            ));
        }
    }
    for old_snap in prune_names {
        lines.push(format!(
            "incus snapshot delete {} {} >/dev/null 2>&1 || true",
            shell_quote(name),
            shell_quote(old_snap)
        ));
        if include_workspace && !snap_dir.is_empty() {
            lines.push(workspace_delete_snapshot_snippet(
                &format!("{snap_dir}/{old_snap}"),
                true,
            ));
        }
    }
    lines.join("\n")
}

/// Pure decision logic for `Backend::buildAutoSnapshotIfNeeded`.
#[allow(clippy::too_many_arguments)]
pub fn decide_auto_snapshot(
    name: &str,
    is_template: bool,
    auto_snapshot_enabled: bool,
    snapshots: &[String],
    workspace_path: &str,
    workspace_snapshot_enabled: bool,
    keep_count: i32,
    last_attempt_epoch: Option<i64>,
    now_epoch: i64,
    now_local_stamp: &str,
) -> Option<String> {
    if name.is_empty() || is_template || !auto_snapshot_enabled {
        return None;
    }
    if let Some(last) = last_attempt_epoch {
        if now_epoch - last < 300 {
            return None;
        }
    }
    if !should_create_auto_snapshot(snapshots, now_epoch, 300) {
        return None;
    }
    let new_snap = format!("auto-{now_local_stamp}");
    if snapshots.iter().any(|s| s == &new_snap) {
        return None;
    }
    let mut all_snaps = snapshots.to_vec();
    all_snaps.push(new_snap.clone());
    let prune_names = auto_snapshots_to_prune(&all_snaps, keep_count);
    Some(auto_snapshot_shell_snippet(
        name,
        &new_snap,
        &prune_names,
        workspace_path,
        workspace_snapshot_enabled,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_snapshot_pruning_and_interval_match_qt_test() {
        assert!(is_auto_snapshot_name("auto-20261113-100000"));
        assert!(!is_auto_snapshot_name("snap-manual"));

        let snaps: Vec<String> = vec![
            "manual-initial",
            "auto-20261113-100000",
            "auto-20261113-101000",
            "before-refactor",
            "auto-20261113-102000",
            "auto-20261113-103000",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let prune = auto_snapshots_to_prune(&snaps, 2);
        assert_eq!(
            prune,
            vec![
                "auto-20261113-100000".to_string(),
                "auto-20261113-101000".to_string()
            ]
        );
        assert!(!prune.iter().any(|s| s == "manual-initial"));
        assert!(!prune.iter().any(|s| s == "before-refactor"));

        let newest = parse_auto_snapshot_timestamp("auto-20261113-103000")
            .expect("valid timestamp")
            .timestamp();
        assert!(!should_create_auto_snapshot(&snaps, newest + 120, 300));
        assert!(should_create_auto_snapshot(&snaps, newest + 300, 300));
        assert!(should_create_auto_snapshot(
            &["manual-only".to_string()],
            newest,
            300
        ));

        let snippet =
            auto_snapshot_shell_snippet("dev-box", "auto-20261113-103500", &prune, "", false);
        assert!(snippet.contains("incus snapshot create 'dev-box' 'auto-20261113-103500'"));
        assert!(snippet.contains("incus snapshot delete 'dev-box' 'auto-20261113-100000'"));
    }

    #[test]
    fn workspace_auto_snapshot_snippet_pairs_create_and_prune() {
        let ws = "/tmp/my workspaces/dev box";
        let prune = vec![
            "auto-20261113-090000".to_string(),
            "auto-20261113-100000".to_string(),
        ];
        let snippet =
            auto_snapshot_shell_snippet("dev-box", "auto-20261113-120000", &prune, ws, true);

        assert!(snippet.contains("incus snapshot create 'dev-box' 'auto-20261113-120000'"));
        assert!(snippet.contains(
            "btrfs subvolume snapshot -r '/tmp/my workspaces/dev box' '/tmp/my workspaces/.snapshots/dev-box/auto-20261113-120000'"
        ));
        assert!(snippet.contains("incus snapshot delete 'dev-box' 'auto-20261113-090000'"));
        assert!(snippet.contains(
            "btrfs subvolume delete '/tmp/my workspaces/.snapshots/dev-box/auto-20261113-090000'"
        ));
        assert!(snippet.contains(
            "btrfs subvolume delete '/tmp/my workspaces/.snapshots/dev-box/auto-20261113-100000'"
        ));
        assert!(snippet.contains("Warning: workspace auto-snapshot failed"));
    }
}
