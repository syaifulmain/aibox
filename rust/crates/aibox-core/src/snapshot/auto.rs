//! Auto-snapshot naming, interval checks, pruning, and shell snippet generation.

use std::collections::{BTreeSet, HashSet};

use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveDateTime, TimeZone};

use crate::snapshot::workspace::{
    workspace_delete_snapshot_snippet, workspace_snapshot_dir, workspace_snapshot_snippet,
    workspace_unchanged_since_last_snapshot,
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

/// Tiered retention policy for auto-snapshots (recent + daily + weekly buckets).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// Always keep the N newest auto-snapshots regardless of age.
    pub keep_recent: usize,
    /// Keep 1 auto-snapshot per calendar day for up to `keep_daily` distinct days.
    pub keep_daily: usize,
    /// Keep 1 auto-snapshot per ISO week for up to `keep_weekly` distinct weeks.
    pub keep_weekly: usize,
}

impl RetentionPolicy {
    /// Builds a policy keeping `keep_recent` recent, `7` daily, and `4` weekly snapshots.
    pub fn tiered(keep_recent: i32) -> Self {
        Self {
            keep_recent: keep_recent.max(0) as usize,
            keep_daily: 7,
            keep_weekly: 4,
        }
    }
}

/// Returns oldest `auto-*` snapshot names to prune under a tiered recent/daily/weekly policy.
///
/// Manual snapshots (without `auto-` prefix) are never pruned. For each day/week bucket within
/// the retention window, the newest snapshot in that bucket is kept.
pub fn auto_snapshots_to_prune_tiered(
    snapshots: &[String],
    policy: RetentionPolicy,
    now_epoch: i64,
) -> Vec<String> {
    let mut autos: Vec<(i64, Option<DateTime<Local>>, String)> = snapshots
        .iter()
        .filter(|s| is_auto_snapshot_name(s))
        .map(|s| {
            let dt = parse_auto_snapshot_timestamp(s);
            let epoch = dt.map(|d| d.timestamp()).unwrap_or(0);
            (epoch, dt, s.clone())
        })
        .collect();

    if autos.is_empty() {
        return Vec::new();
    }

    // Sort newest-first so we greedily keep the latest snapshot in each bucket.
    autos.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.2.cmp(&a.2)));

    let now_dt = Local
        .timestamp_opt(now_epoch, 0)
        .earliest()
        .unwrap_or_else(Local::now);
    let today = now_dt.date_naive();

    let mut kept: HashSet<String> = HashSet::new();
    let mut daily_buckets: BTreeSet<NaiveDate> = BTreeSet::new();
    let mut weekly_buckets: BTreeSet<(i32, u32)> = BTreeSet::new();

    for (idx, (_epoch, dt_opt, name)) in autos.iter().enumerate() {
        if idx < policy.keep_recent {
            kept.insert(name.clone());
            continue;
        }
        let Some(dt) = dt_opt else {
            continue;
        };
        let date = dt.date_naive();
        let age_days = (today - date).num_days();
        if age_days < 0 {
            // Future-dated snapshots are kept in recent/daily if within bounds.
            continue;
        }
        let mut keep_this = false;
        if policy.keep_daily > 0
            && (age_days as usize) < policy.keep_daily
            && !daily_buckets.contains(&date)
            && daily_buckets.len() < policy.keep_daily
        {
            daily_buckets.insert(date);
            keep_this = true;
        }
        let iso_week = date.iso_week();
        let week_key = (iso_week.year(), iso_week.week());
        let age_weeks = (age_days / 7) as usize;
        if policy.keep_weekly > 0
            && age_weeks < policy.keep_weekly
            && !weekly_buckets.contains(&week_key)
            && weekly_buckets.len() < policy.keep_weekly
        {
            weekly_buckets.insert(week_key);
            keep_this = true;
        }
        if keep_this {
            kept.insert(name.clone());
        }
    }

    // Return pruned names oldest-first, matching `auto_snapshots_to_prune`.
    autos
        .into_iter()
        .rev()
        .filter(|(_, _, name)| !kept.contains(name))
        .map(|(_, _, name)| name)
        .collect()
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
    if workspace_snapshot_enabled
        && workspace_unchanged_since_last_snapshot(workspace_path, name, snapshots)
    {
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

    struct FakeClock {
        epoch: i64,
    }

    impl FakeClock {
        fn new(epoch: i64) -> Self {
            Self { epoch }
        }
        fn advance(&mut self, secs: i64) -> i64 {
            self.epoch += secs;
            self.epoch
        }
        fn stamp(&self) -> String {
            Local
                .timestamp_opt(self.epoch, 0)
                .earliest()
                .expect("valid epoch")
                .format("%Y%m%d-%H%M%S")
                .to_string()
        }
    }

    #[test]
    fn min_interval_seconds_with_fake_clock() {
        let base_dt =
            NaiveDateTime::parse_from_str("20261113-100000", "%Y%m%d-%H%M%S").expect("valid dt");
        let base_epoch = Local
            .from_local_datetime(&base_dt)
            .earliest()
            .expect("local")
            .timestamp();
        let mut clock = FakeClock::new(base_epoch);
        let mut snaps = vec![format!("auto-{}", clock.stamp())];

        // 299s later: still within 300s interval
        let t1 = clock.advance(299);
        assert!(!should_create_auto_snapshot(&snaps, t1, 300));
        assert!(decide_auto_snapshot(
            "dev-box",
            false,
            true,
            &snaps,
            "",
            false,
            5,
            None,
            t1,
            &clock.stamp(),
        )
        .is_none());

        // +1s (300s total): boundary reached -> allowed
        let t2 = clock.advance(1);
        assert!(should_create_auto_snapshot(&snaps, t2, 300));
        // Unless last_attempt_epoch was 60s ago
        assert!(decide_auto_snapshot(
            "dev-box",
            false,
            true,
            &snaps,
            "",
            false,
            5,
            Some(t2 - 60),
            t2,
            &clock.stamp(),
        )
        .is_none());
        assert!(decide_auto_snapshot(
            "dev-box",
            false,
            true,
            &snaps,
            "",
            false,
            5,
            Some(t2 - 300),
            t2,
            &clock.stamp(),
        )
        .is_some());

        // Negative or zero min_interval_seconds always allows creation
        assert!(should_create_auto_snapshot(&snaps, base_epoch, 0));
        assert!(should_create_auto_snapshot(&snaps, base_epoch, -10));

        // After adding the new snapshot, another 299s must elapse before the next one
        snaps.push(format!("auto-{}", clock.stamp()));
        let t3 = clock.advance(299);
        assert!(!should_create_auto_snapshot(&snaps, t3, 300));
        let t4 = clock.advance(1);
        assert!(should_create_auto_snapshot(&snaps, t4, 300));
    }

    #[test]
    fn tiered_retention_keeps_recent_daily_and_weekly_snapshots() {
        let now_dt =
            NaiveDateTime::parse_from_str("20261118-180000", "%Y%m%d-%H%M%S").expect("valid dt");
        let now_epoch = Local
            .from_local_datetime(&now_dt)
            .earliest()
            .expect("local")
            .timestamp();

        let snaps: Vec<String> = vec![
            "manual-keep",
            // Week -3 (21 days ago, outside 7-day daily window, inside 4-week weekly)
            "auto-20261028-090000",
            "auto-20261028-150000", // newer in same week -> kept as weekly
            // Day -2 (inside 7-day daily window): 2 on same day
            "auto-20261116-090000",
            "auto-20261116-170000", // newer on day -2 -> kept as daily
            // Day -1: 2 on same day
            "auto-20261117-080000",
            "auto-20261117-200000", // newer on day -1 -> kept as daily
            // Today: 2 recent
            "auto-20261118-100000",
            "auto-20261118-160000",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let policy = RetentionPolicy {
            keep_recent: 2,
            keep_daily: 7,
            keep_weekly: 4,
        };
        let prune = auto_snapshots_to_prune_tiered(&snaps, policy, now_epoch);
        assert_eq!(
            prune,
            vec![
                "auto-20261028-090000".to_string(),
                "auto-20261116-090000".to_string(),
                "auto-20261117-080000".to_string(),
            ]
        );
        assert!(!prune.iter().any(|s| s == "manual-keep"));
    }

    #[test]
    fn smart_auto_snapshot_skips_unchanged_workspace_and_triggers_on_change() {
        let tmp = std::env::temp_dir().join(format!(
            "aibox-smart-autosnap-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let ws = tmp.join("dev-box");
        let snap_dir = tmp.join(".snapshots/dev-box");
        let old_snap = snap_dir.join("auto-20261113-100000");
        std::fs::create_dir_all(&ws).expect("ws");
        std::fs::write(ws.join("code.rs"), "fn main() {}\n").expect("write");
        // Ensure snapshot dir mtime is >= workspace mtime.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::create_dir_all(&old_snap).expect("snap");

        let snaps = vec!["auto-20261113-100000".to_string()];
        let old_epoch = parse_auto_snapshot_timestamp_epoch("auto-20261113-100000").expect("epoch");

        // Workspace is unchanged since old_snap -> skipped even after 600s.
        assert!(decide_auto_snapshot(
            "dev-box",
            false,
            true,
            &snaps,
            &ws.to_string_lossy(),
            true,
            5,
            None,
            old_epoch + 600,
            "20261113-101000",
        )
        .is_none());

        // Modify a file in workspace -> mtime advances -> auto-snapshot created.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(ws.join("code.rs"), "fn main() { println!(\"hi\"); }\n").expect("modify");
        assert!(decide_auto_snapshot(
            "dev-box",
            false,
            true,
            &snaps,
            &ws.to_string_lossy(),
            true,
            5,
            None,
            old_epoch + 600,
            "20261113-101000",
        )
        .is_some());

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
