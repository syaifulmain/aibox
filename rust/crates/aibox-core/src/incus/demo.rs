use chrono::{Local, TimeZone, Utc};
use serde::Serialize;
use serde_json::{json, Value};

use crate::backup::backups_dir_path;
use crate::health::pool::StoragePoolInfo;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DemoState {
    pub environments: Vec<Value>,
    pub operation_history: Vec<Value>,
    pub default_secrets: Vec<(String, String)>,
    pub backup_archives: Vec<Value>,
    pub pool_info: Value,
    pub pool_valid: bool,
    pub pool_name: String,
    pub pool_driver: String,
    pub pool_used_bytes: i64,
    pub pool_total_bytes: i64,
    pub pool_used_percent: i32,
    pub pool_size_text: String,
    pub pool_resizable: bool,
    pub pool_allocated_bytes: i64,
    pub pool_unbounded_count: i32,
    pub recorded_sessions: Vec<Value>,
}

pub fn build_demo_state(record_sessions: bool, os_storage_path: &str, now_epoch: i64) -> DemoState {
    let backups_dir = backups_dir_path(os_storage_path);

    let mut e1 = serde_json::Map::new();
    e1.insert("name".into(), json!("flutter-pos"));
    e1.insert("status".into(), json!("RUNNING"));
    e1.insert("running".into(), json!(true));
    e1.insert("type".into(), json!("CONTAINER"));
    e1.insert("osName".into(), json!("Ubuntu 24.04 LTS"));
    e1.insert("preset".into(), json!("Desktop & Mobile (GPU + KVM)"));
    e1.insert("cpuLimit".into(), json!("4 Cores"));
    e1.insert("cpuCores".into(), json!(4));
    e1.insert("cpuUsage".into(), json!("18%"));
    e1.insert("cpuPercent".into(), json!(18));
    e1.insert("cpuCorePercents".into(), json!([24, 19, 16, 13]));
    e1.insert("memoryLimit".into(), json!("8GiB"));
    e1.insert("memoryUsage".into(), json!("1.4 GB"));
    e1.insert("memoryPercent".into(), json!(18));
    e1.insert("diskLimit".into(), json!("60GiB"));
    e1.insert("diskUsage".into(), json!("8.2 GB"));
    e1.insert("diskUsageBytes".into(), json!(8_804_682_956_i64));
    e1.insert("diskPercent".into(), json!(14));
    e1.insert("processLimit".into(), json!("512"));
    e1.insert("diskReadLimit".into(), json!("200MB"));
    e1.insert("diskWriteLimit".into(), json!("200MB"));
    e1.insert("ipv4".into(), json!("10.143.44.18"));
    e1.insert("netRxBytes".into(), json!(39_012_732_109_i64));
    e1.insert("netTxBytes".into(), json!(7_365_862_195_i64));
    e1.insert("netRxRate".into(), json!(153_293_i64));
    e1.insert("netTxRate".into(), json!(14_234_i64));
    e1.insert("netRxRateText".into(), json!("149.7 KB/s"));
    e1.insert("netTxRateText".into(), json!("13.9 KB/s"));
    e1.insert("netRxTotalText".into(), json!("36.33 GB"));
    e1.insert("netTxTotalText".into(), json!("6.86 GB"));
    e1.insert("hostInterface".into(), json!("veth-demo1"));
    e1.insert("workspacePath".into(), json!("~/ai-workspaces/flutter-pos"));
    e1.insert("workspaceOnBtrfs".into(), json!(true));
    e1.insert("workspaceIsSubvolume".into(), json!(true));
    e1.insert("workspaceSnapshotOverride".into(), json!(""));
    e1.insert("workspaceSnapshotEnabled".into(), json!(true));
    e1.insert(
        "workspaceSnapshotDir".into(),
        json!("~/ai-workspaces/.snapshots/flutter-pos"),
    );
    e1.insert(
        "workspaceSnapshots".into(),
        json!(["clean-sdk", "before-ai-refactor", "auto-20261113-120000"]),
    );
    e1.insert("orphanWorkspaceSnapshots".into(), json!([]));
    e1.insert(
        "workspaceSnapshotSizes".into(),
        json!({
            "clean-sdk": "145 MB",
            "before-ai-refactor": "152 MB",
            "auto-20261113-120000": "152 MB"
        }),
    );
    e1.insert("hasPreSubvolBackup".into(), json!(false));
    e1.insert("snapshotCount".into(), json!(3));
    e1.insert("autoSnapshotCount".into(), json!(1));
    e1.insert("manualSnapshotCount".into(), json!(2));
    e1.insert(
        "snapshots".into(),
        json!(["clean-sdk", "before-ai-refactor", "auto-20261113-120000"]),
    );
    e1.insert(
        "snapshotSizes".into(),
        json!({
            "clean-sdk": "420 MB",
            "before-ai-refactor": "1.1 GB",
            "auto-20261113-120000": "185 MB"
        }),
    );
    e1.insert("autoSnapshotOverride".into(), json!(""));
    e1.insert("autoSnapshotEnabled".into(), json!(true));
    e1.insert("autoStopOverride".into(), json!(""));
    e1.insert("autoStopOnCritical".into(), json!(false));
    e1.insert("idleStopOverride".into(), json!(""));
    e1.insert("idleStopIgnorePorts".into(), json!(false));
    e1.insert("diskAlertLevel".into(), json!(0));
    e1.insert("isTemplate".into(), json!(false));
    e1.insert("networkMode".into(), json!("online"));
    e1.insert("networkAllowlist".into(), json!(""));
    e1.insert("networkBlocked".into(), json!(false));
    e1.insert("egressRecording".into(), json!(true));
    e1.insert(
        "portForwards".into(),
        json!([
            {"deviceName": "fwd-3000", "hostPort": 3000, "guestPort": 3000},
            {"deviceName": "fwd-8080", "hostPort": 8080, "guestPort": 8080}
        ]),
    );
    e1.insert("portForwardCount".into(), json!(2));
    e1.insert(
        "extraMounts".into(),
        json!([
            {"deviceName": "mnt-datasets", "source": "/home/user/datasets", "path": "/mnt/datasets", "readonly": true}
        ]),
    );
    e1.insert("extraMountCount".into(), json!(1));
    e1.insert("secretNames".into(), json!(["ANTHROPIC_API_KEY"]));
    e1.insert("snapshotTotalBytes".into(), json!(1_787_822_080_i64));
    e1.insert("snapshotTotalDisplay".into(), json!("1.66 GB"));
    e1.insert("lastUsedEpoch".into(), json!(now_epoch - (2 * 86400)));
    e1.insert("recordOverride".into(), json!(""));
    e1.insert("recordEnabled".into(), json!(record_sessions));
    let e1 = Value::Object(e1);

    let mut e2 = serde_json::Map::new();
    e2.insert("name".into(), json!("desktop-studio"));
    e2.insert("status".into(), json!("RUNNING"));
    e2.insert("running".into(), json!(true));
    e2.insert("type".into(), json!("CONTAINER"));
    e2.insert("osName".into(), json!("Arch Linux"));
    e2.insert("preset".into(), json!("Desktop & Mobile (GPU + KVM)"));
    e2.insert("cpuLimit".into(), json!("6 Cores"));
    e2.insert("cpuCores".into(), json!(6));
    e2.insert("cpuUsage".into(), json!("9%"));
    e2.insert("cpuPercent".into(), json!(9));
    e2.insert("cpuCorePercents".into(), json!([14, 11, 9, 8, 7, 5]));
    e2.insert("memoryLimit".into(), json!("6GiB"));
    e2.insert("memoryUsage".into(), json!("620 MB"));
    e2.insert("memoryPercent".into(), json!(10));
    e2.insert("diskLimit".into(), json!("35GiB"));
    e2.insert("diskUsage".into(), json!("32.2 GB"));
    e2.insert("diskPercent".into(), json!(92));
    e2.insert("processLimit".into(), json!("1024"));
    e2.insert("diskReadLimit".into(), json!(""));
    e2.insert("diskWriteLimit".into(), json!(""));
    e2.insert("ipv4".into(), json!("10.143.44.42"));
    e2.insert("netRxBytes".into(), json!(2_587_711_488_i64));
    e2.insert("netTxBytes".into(), json!(934_281_216_i64));
    e2.insert("netRxRate".into(), json!(24_883_i64));
    e2.insert("netTxRate".into(), json!(3_174_i64));
    e2.insert("netRxRateText".into(), json!("24.3 KB/s"));
    e2.insert("netTxRateText".into(), json!("3.1 KB/s"));
    e2.insert("netRxTotalText".into(), json!("2.41 GB"));
    e2.insert("netTxTotalText".into(), json!("891.0 MB"));
    e2.insert("hostInterface".into(), json!("veth-demo2"));
    e2.insert(
        "workspacePath".into(),
        json!("~/ai-workspaces/desktop-studio"),
    );
    e2.insert("snapshotCount".into(), json!(1));
    e2.insert("autoSnapshotCount".into(), json!(0));
    e2.insert("manualSnapshotCount".into(), json!(1));
    e2.insert("snapshots".into(), json!(["initial-setup"]));
    e2.insert("snapshotSizes".into(), json!({"initial-setup": "640 MB"}));
    e2.insert("autoSnapshotOverride".into(), json!(""));
    e2.insert("autoSnapshotEnabled".into(), json!(true));
    e2.insert("autoStopOverride".into(), json!("true"));
    e2.insert("autoStopOnCritical".into(), json!(true));
    e2.insert("diskAlertLevel".into(), json!(1));
    e2.insert("isTemplate".into(), json!(false));
    e2.insert("networkMode".into(), json!("allowlist"));
    e2.insert(
        "networkAllowlist".into(),
        json!("github.com, api.anthropic.com"),
    );
    e2.insert("networkBlocked".into(), json!(false));
    e2.insert("egressRecording".into(), json!(true));
    e2.insert("snapshotTotalBytes".into(), json!(671_088_640_i64));
    e2.insert("snapshotTotalDisplay".into(), json!("640 MB"));
    e2.insert("lastUsedEpoch".into(), json!(now_epoch - (45 * 86400)));
    e2.insert("recordOverride".into(), json!("true"));
    e2.insert("recordEnabled".into(), json!(true));
    let e2 = Value::Object(e2);

    let mut e3 = serde_json::Map::new();
    e3.insert("name".into(), json!("saas-backend"));
    e3.insert("status".into(), json!("STOPPED"));
    e3.insert("running".into(), json!(false));
    e3.insert("type".into(), json!("CONTAINER"));
    e3.insert("osName".into(), json!("Debian 12"));
    e3.insert("preset".into(), json!("Standard"));
    e3.insert("cpuLimit".into(), json!("2 Cores"));
    e3.insert("cpuCores".into(), json!(2));
    e3.insert("cpuUsage".into(), json!("0%"));
    e3.insert("cpuPercent".into(), json!(0));
    e3.insert("cpuCorePercents".into(), json!([0, 0]));
    e3.insert("memoryLimit".into(), json!("4GiB"));
    e3.insert("memoryUsage".into(), json!("0 MB"));
    e3.insert("memoryPercent".into(), json!(0));
    e3.insert("diskLimit".into(), json!("20GiB"));
    e3.insert("diskUsage".into(), json!("1.1 GB"));
    e3.insert("diskPercent".into(), json!(6));
    e3.insert("processLimit".into(), json!("512"));
    e3.insert("diskReadLimit".into(), json!(""));
    e3.insert("diskWriteLimit".into(), json!(""));
    e3.insert("ipv4".into(), json!("\u{2014}"));
    e3.insert("hostInterface".into(), json!(""));
    e3.insert(
        "workspacePath".into(),
        json!("~/ai-workspaces/saas-backend"),
    );
    e3.insert("snapshotCount".into(), json!(0));
    e3.insert("autoSnapshotCount".into(), json!(0));
    e3.insert("manualSnapshotCount".into(), json!(0));
    e3.insert("snapshots".into(), json!([]));
    e3.insert("snapshotSizes".into(), json!({}));
    e3.insert("autoSnapshotOverride".into(), json!("false"));
    e3.insert("autoSnapshotEnabled".into(), json!(false));
    e3.insert("autoStopOverride".into(), json!(""));
    e3.insert("autoStopOnCritical".into(), json!(false));
    e3.insert("diskAlertLevel".into(), json!(0));
    e3.insert("isTemplate".into(), json!(true));
    e3.insert("networkMode".into(), json!("offline"));
    e3.insert("networkAllowlist".into(), json!(""));
    e3.insert("networkBlocked".into(), json!(true));
    e3.insert("egressRecording".into(), json!(false));
    e3.insert("recordOverride".into(), json!("false"));
    e3.insert("recordEnabled".into(), json!(false));
    let e3 = Value::Object(e3);

    let operation_history = vec![
        json!({
            "id": "demo-1",
            "description": "Updating network mode for 'desktop-studio'",
            "envName": "desktop-studio",
            "status": "success",
            "exitCode": 0,
            "timestamp": "2026-11-13 13:10:02",
            "startedAt": "2026-11-13T13:10:00",
            "durationMs": 2000,
            "logFile": ""
        }),
        json!({
            "id": "demo-2",
            "description": "Saving snapshot 'before-ai-refactor' for 'flutter-pos'",
            "envName": "flutter-pos",
            "status": "success",
            "exitCode": 0,
            "timestamp": "2026-11-13 12:45:13",
            "startedAt": "2026-11-13T12:45:12",
            "durationMs": 1000,
            "logFile": ""
        }),
    ];

    let default_secrets = vec![
        (
            "ANTHROPIC_API_KEY".to_string(),
            "sk-ant-demo-0000".to_string(),
        ),
        ("GITHUB_TOKEN".to_string(), "ghp_demo_0000".to_string()),
    ];

    let backup_archives = vec![json!({
        "fileName": "flutter-pos-20261112-194000.tar.gz",
        "filePath": format!("{backups_dir}/flutter-pos-20261112-194000.tar.gz"),
        "envName": "flutter-pos",
        "sizeBytes": 2_415_919_104_i64,
        "sizeDisplay": "2.2 GB",
        "createdAt": "2026-11-12 19:40",
        "hasWorkspaceArchive": true,
        "workspaceArchivePath": format!("{backups_dir}/flutter-pos-20261112-194000-workspace.tar.gz"),
        "workspaceArchiveSizeDisplay": "148 MB"
    })];

    let pool = StoragePoolInfo {
        valid: true,
        name: "default".to_string(),
        driver: "btrfs".to_string(),
        used_bytes: 19_756_849_562_i64,
        total_bytes: 85_899_345_920_i64,
        used_percent: 23,
        size_text: "80GiB".to_string(),
        resizable: true,
    };
    let pool_allocated_bytes = 80_i64 * 1024 * 1024 * 1024;
    let pool_unbounded_count = 0;
    let pool_info: Value =
        serde_json::from_str(&pool.to_json(pool_allocated_bytes, 0)).unwrap_or_else(|_| json!({}));

    let demo_session_epoch = 1_794_400_000_i64;
    let session_ts = Local
        .timestamp_opt(demo_session_epoch, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default();
    let recorded_sessions = vec![json!({
        "id": "desktop-studio/1794400000",
        "envName": "desktop-studio",
        "epoch": demo_session_epoch,
        "timestamp": session_ts,
        "logPath": "demo://desktop-studio/1794400000.log",
        "timingPath": "demo://desktop-studio/1794400000.timing",
        "hasTiming": true,
        "sizeBytes": 4280_i64,
        "sizeDisplay": "4 KB"
    })];

    DemoState {
        environments: vec![e1, e2, e3],
        operation_history,
        default_secrets,
        backup_archives,
        pool_info,
        pool_valid: pool.valid,
        pool_name: pool.name,
        pool_driver: pool.driver,
        pool_used_bytes: pool.used_bytes,
        pool_total_bytes: pool.total_bytes,
        pool_used_percent: pool.used_percent,
        pool_size_text: pool.size_text,
        pool_resizable: pool.resizable,
        pool_allocated_bytes,
        pool_unbounded_count,
        recorded_sessions,
    }
}

pub fn build_demo_state_now(record_sessions: bool, os_storage_path: &str) -> DemoState {
    build_demo_state(record_sessions, os_storage_path, Utc::now().timestamp())
}

pub fn value_list_to_json(values: &[Value]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| "[]".to_string())
}

pub fn value_to_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_state_has_three_environments_and_one_template() {
        let state = build_demo_state(false, "/tmp/os-storage", 1_800_000_000);
        assert_eq!(state.environments.len(), 3);
        let template_count = state
            .environments
            .iter()
            .filter(|e| e.get("isTemplate").and_then(Value::as_bool) == Some(true))
            .count();
        assert_eq!(template_count, 1);
        assert_eq!(state.operation_history.len(), 2);
        assert_eq!(state.recorded_sessions.len(), 1);
        assert_eq!(state.backup_archives.len(), 1);
    }
}
