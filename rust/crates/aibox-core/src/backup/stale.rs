//! Environment staleness classification and stale report builder.

use chrono::{Local, TimeZone};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::backup::orphans::detect_orphans;
use crate::backup::{directory_size_bytes, expand_tilde};
use crate::util::format::format_bytes;
use crate::util::paths::home_dir;
use crate::util::size::parse_size_bytes;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum StalenessClass {
    Fresh = 0,
    Stale = 1,
    Unknown = 2,
    Template = 3,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StalenessResult {
    pub classification: StalenessClass,
    pub days_unused: i64,
    pub status_key: String,
}

/// Classifies an environment's staleness from its `last_used_epoch` and `stale_days` threshold.
pub fn classify_environment_staleness(
    last_used_epoch: i64,
    is_template: bool,
    stale_days: i32,
    now_epoch: i64,
) -> StalenessResult {
    if is_template {
        return StalenessResult {
            classification: StalenessClass::Template,
            days_unused: -1,
            status_key: "template".to_string(),
        };
    }
    if last_used_epoch <= 0 {
        return StalenessResult {
            classification: StalenessClass::Unknown,
            days_unused: -1,
            status_key: "unknown".to_string(),
        };
    }
    let threshold_days = stale_days.clamp(1, 3650) as i64;
    let ref_now = if now_epoch > 0 {
        now_epoch
    } else {
        Local::now().timestamp()
    };
    let elapsed = (ref_now - last_used_epoch).max(0);
    let days_unused = elapsed / 86_400;
    if elapsed >= threshold_days * 86_400 {
        StalenessResult {
            classification: StalenessClass::Stale,
            days_unused,
            status_key: "stale".to_string(),
        }
    } else {
        StalenessResult {
            classification: StalenessClass::Fresh,
            days_unused,
            status_key: "fresh".to_string(),
        }
    }
}

fn format_local_date(epoch_secs: i64) -> String {
    Local
        .timestamp_opt(epoch_secs, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

/// Builds the full stale environment and orphan report map matching `Backend::buildStaleReport`.
pub fn build_stale_report(
    environments: &[Value],
    workspace_base: &str,
    backups_dir: &str,
    stale_days: i32,
    now_epoch: i64,
) -> Value {
    let threshold_days = stale_days.clamp(1, 3650);
    let ref_now = if now_epoch > 0 {
        now_epoch
    } else {
        Local::now().timestamp()
    };
    let home = home_dir();

    let mut stale_entries: Vec<Value> = Vec::new();
    let mut unknown_entries: Vec<Value> = Vec::new();
    let mut fresh_count = 0i32;
    let mut template_count = 0i32;
    let mut reclaimable_bytes = 0i64;

    for v in environments {
        let Some(env) = v.as_object() else {
            continue;
        };
        let is_tpl = env
            .get("isTemplate")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let last_used = env
            .get("lastUsedEpoch")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let cls = classify_environment_staleness(last_used, is_tpl, threshold_days, ref_now);

        match cls.classification {
            StalenessClass::Template => {
                template_count += 1;
                continue;
            }
            StalenessClass::Fresh => {
                fresh_count += 1;
                continue;
            }
            StalenessClass::Stale | StalenessClass::Unknown => {}
        }

        let disk_usage_str = env.get("diskUsage").and_then(Value::as_str).unwrap_or("");
        let mut disk_bytes = env
            .get("diskUsageBytes")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        if disk_bytes <= 0 {
            disk_bytes = parse_size_bytes(disk_usage_str);
        }

        let mut snap_total_bytes = env
            .get("snapshotTotalBytes")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        if snap_total_bytes <= 0 {
            if let Some(snap_sizes) = env.get("snapshotSizes").and_then(Value::as_object) {
                for val in snap_sizes.values() {
                    let s = val.as_str().unwrap_or("");
                    snap_total_bytes += parse_size_bytes(s).max(0);
                }
            }
        }

        let raw_ws_path = env
            .get("workspacePath")
            .and_then(Value::as_str)
            .unwrap_or("");
        let ws_path = expand_tilde(raw_ws_path.trim(), &home);
        let ws_bytes = if ws_path.is_empty() {
            0
        } else {
            directory_size_bytes(&ws_path)
        };

        let total_bytes = disk_bytes.max(0) + snap_total_bytes.max(0) + ws_bytes.max(0);
        reclaimable_bytes += total_bytes;

        let last_used_display = if cls.classification == StalenessClass::Stale {
            format!(
                "{}d ago ({})",
                cls.days_unused,
                format_local_date(last_used)
            )
        } else {
            "Unknown (never recorded)".to_string()
        };
        let disk_usage_display = if disk_bytes > 0 {
            format_bytes(disk_bytes)
        } else {
            disk_usage_str.to_string()
        };
        let snap_total_display = if snap_total_bytes > 0 {
            format_bytes(snap_total_bytes)
        } else {
            "—".to_string()
        };

        let item = json!({
            "name": env.get("name").and_then(Value::as_str).unwrap_or(""),
            "status": cls.status_key,
            "running": env.get("running").and_then(Value::as_bool).unwrap_or(false),
            "type": env.get("type").and_then(Value::as_str).unwrap_or(""),
            "osName": env.get("osName").and_then(Value::as_str).unwrap_or(""),
            "lastUsedEpoch": last_used,
            "daysUnused": cls.days_unused,
            "lastUsedDisplay": last_used_display,
            "diskUsageBytes": disk_bytes,
            "diskUsageDisplay": disk_usage_display,
            "snapshotCount": env.get("snapshotCount").and_then(Value::as_i64).unwrap_or(0) as i32,
            "snapshotTotalBytes": snap_total_bytes,
            "snapshotTotalDisplay": snap_total_display,
            "workspacePath": raw_ws_path,
            "workspaceSizeBytes": ws_bytes,
            "workspaceSizeDisplay": format_bytes(ws_bytes),
            "totalFootprintBytes": total_bytes,
            "totalFootprintDisplay": format_bytes(total_bytes),
        });

        if cls.classification == StalenessClass::Stale {
            stale_entries.push(item);
        } else {
            unknown_entries.push(item);
        }
    }

    stale_entries.sort_by(|a, b| {
        let da = a.get("daysUnused").and_then(Value::as_i64).unwrap_or(0);
        let db = b.get("daysUnused").and_then(Value::as_i64).unwrap_or(0);
        db.cmp(&da)
    });

    let stale_count = stale_entries.len() as i32;
    let unknown_count = unknown_entries.len() as i32;
    let mut entries = stale_entries;
    entries.extend(unknown_entries);

    let orphans = detect_orphans(environments, workspace_base, backups_dir);
    for ov in &orphans {
        let sb = ov.get("sizeBytes").and_then(Value::as_i64).unwrap_or(0);
        reclaimable_bytes += sb.max(0);
    }
    let orphan_count = orphans.len() as i32;

    let generated_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    json!({
        "staleDays": threshold_days,
        "generatedAt": generated_at,
        "entries": entries,
        "orphans": orphans,
        "staleCount": stale_count,
        "unknownCount": unknown_count,
        "freshCount": fresh_count,
        "templateCount": template_count,
        "orphanCount": orphan_count,
        "totalReclaimableBytes": reclaimable_bytes,
        "totalReclaimableDisplay": format_bytes(reclaimable_bytes),
    })
}
