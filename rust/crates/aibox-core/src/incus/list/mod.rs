pub mod metrics;
pub mod model;
pub mod parse;
pub mod quota;

pub use metrics::{build_cpu_core_percents, format_percent, parse_cpu_count};
pub use model::{
    environment_by_name, templates, CounterState, Environment, ExtraMountEntry, ParseDefaults,
    PortForwardEntry,
};
pub use parse::{hash_list_inputs, parse, parse_if_changed, parse_json, parse_with_now};

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn parse_incus_json_extracts_specs_and_snapshots_match_qt_test() {
        let sample = br#"[
            {
                "name": "flutter-pos",
                "status": "Running",
                "type": "container",
                "profiles": ["default", "ai-gui"],
                "config": {
                    "image.description": "Ubuntu noble amd64 (20261110)",
                    "limits.cpu": "4",
                    "limits.memory": "6GiB"
                },
                "devices": {
                    "root": { "size": "35GiB" }
                },
                "state": {
                    "disk": { "root": { "usage": 10737418240, "total": 37580963840 } },
                    "memory": { "usage": 524288000, "total": 6442450944 },
                    "cpu": { "usage": 8000000000 },
                    "network": {
                        "eth0": {
                            "addresses": [
                                { "family": "inet", "scope": "global", "address": "10.143.44.55" }
                            ]
                        }
                    }
                },
                "snapshots": [
                    { "name": "snap-initial" },
                    { "name": "before-refactor" }
                ]
            }
        ]"#;

        let mut state = CounterState::new();
        state
            .cpu_snapshot_ns
            .insert("flutter-pos".to_string(), 4_000_000_000);
        let list = parse(
            sample,
            "/tmp/workspaces",
            &mut state,
            4_000_000_000,
            ParseDefaults {
                auto_snapshot: true,
                auto_stop: false,
                record_sessions: false,
            },
            true,
        );
        assert_eq!(list.len(), 1);
        let env = &list[0];
        assert_eq!(env.name, "flutter-pos");
        assert!(env.running);
        assert_eq!(env.os_name, "Ubuntu noble amd64");
        assert_eq!(env.preset, "Desktop & Mobile (GPU + KVM)");
        assert_eq!(env.disk_limit, "35GiB");
        assert_eq!(env.disk_usage, "10.0 GiB");
        assert_eq!(env.disk_percent, 29);
        assert_eq!(env.memory_limit, "6GiB");
        assert_eq!(env.memory_usage, "500 MiB");
        assert_eq!(env.memory_percent, 8);
        assert_eq!(env.cpu_limit, "4 Cores");
        assert_eq!(env.cpu_cores, 4);
        assert_eq!(env.cpu_usage, "25%");
        assert_eq!(env.cpu_percent, 25);
        assert_eq!(env.cpu_core_percents.len(), 4);
        assert_eq!(env.ipv4, "10.143.44.55");
        assert_eq!(env.snapshot_count, 2);
    }

    #[test]
    fn parse_incus_json_maps_container_swap_limits() {
        let sample = br#"[
            {
                "name": "swap-off",
                "status": "Stopped",
                "type": "container",
                "config": { "limits.memory.swap": "false" },
                "devices": {}
            },
            {
                "name": "swap-size",
                "status": "Stopped",
                "type": "container",
                "config": { "limits.memory.swap": "4GiB" },
                "devices": {}
            },
            {
                "name": "swap-host",
                "status": "Stopped",
                "type": "container",
                "config": { "limits.memory.swap": "true" },
                "devices": {}
            },
            {
                "name": "swap-vm",
                "status": "Stopped",
                "type": "virtual-machine",
                "config": { "limits.memory.swap": "4GiB" },
                "devices": {}
            }
        ]"#;
        let mut state = CounterState::new();
        let envs = parse(
            sample,
            "/tmp/ws",
            &mut state,
            0,
            ParseDefaults::default(),
            false,
        );
        assert_eq!(envs[0].swap_limit, "Off");
        assert_eq!(envs[1].swap_limit, "4GiB");
        assert_eq!(envs[2].swap_limit, "");
        assert_eq!(envs[3].swap_limit, "");
    }

    #[test]
    fn parse_incus_json_uses_workspace_device_source_match_qt_test() {
        let sample = br#"[
            {
                "name": "custom-box",
                "status": "Running",
                "type": "container",
                "config": {},
                "expanded_devices": {
                    "workspace": { "type": "disk", "source": "/data/custom-box" }
                }
            }
        ]"#;
        let mut state = CounterState::new();
        let list = parse(
            sample,
            "/tmp/default",
            &mut state,
            0,
            ParseDefaults::default(),
            false,
        );
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].workspace_path, "/data/custom-box");
    }

    #[test]
    fn parse_incus_json_extracts_template_and_network_state_match_qt_test() {
        let sample = br#"[
            {
                "name": "node-tpl",
                "status": "Stopped",
                "type": "container",
                "config": {
                    "user.aibox.template": "true",
                    "user.aibox.autosnap": "false",
                    "user.aibox.netmode": "allowlist",
                    "user.aibox.netallow": "github.com, 1.1.1.1"
                },
                "devices": {
                    "eth0": { "type": "nic", "security.acls": "aibox-node-tpl" }
                },
                "snapshots": [
                    { "name": "base" },
                    { "name": "auto-20261113-090000" }
                ]
            },
            {
                "name": "airgap-box",
                "status": "Running",
                "type": "container",
                "config": {},
                "devices": {
                    "eth0": { "type": "none" }
                }
            }
        ]"#;
        let mut state = CounterState::new();
        let list = parse(
            sample,
            "/tmp/ws",
            &mut state,
            0,
            ParseDefaults {
                auto_snapshot: true,
                auto_stop: false,
                record_sessions: false,
            },
            false,
        );
        assert_eq!(list.len(), 2);
        let tpl = &list[0];
        assert!(tpl.is_template);
        assert!(!tpl.auto_snapshot_enabled);
        assert_eq!(tpl.auto_snapshot_count, 1);
        assert_eq!(tpl.manual_snapshot_count, 1);
        assert_eq!(tpl.network_mode, "allowlist");
        assert_eq!(tpl.network_allowlist, "github.com, 1.1.1.1");

        let airgap = &list[1];
        assert!(!airgap.is_template);
        assert!(airgap.auto_snapshot_enabled);
        assert_eq!(airgap.network_mode, "offline");
        assert_eq!(templates(&list).len(), 1);
        assert!(environment_by_name(&list, "node-tpl").is_some());
    }

    #[test]
    fn parse_incus_json_extracts_port_forwards_and_extra_mounts_match_qt_test() {
        let sample = br#"[
            {
                "name": "dev-box",
                "status": "Running",
                "type": "container",
                "config": { "limits.cpu": "2", "limits.memory": "4GiB" },
                "devices": {
                    "root": { "type": "disk", "path": "/", "size": "40GiB" },
                    "workspace": { "type": "disk", "source": "/tmp/ws", "path": "/workspace" },
                    "fwd-3000": { "type": "proxy", "listen": "tcp:127.0.0.1:3000", "connect": "tcp:127.0.0.1:8000" },
                    "mnt-datasets": { "type": "disk", "source": "/tmp/datasets", "path": "/mnt/datasets", "readonly": "true" }
                },
                "state": {
                    "disk": { "root": { "usage": 5368709120, "total": 42949672960 } }
                }
            }
        ]"#;
        let mut state = CounterState::new();
        let envs = parse(
            sample,
            "/tmp",
            &mut state,
            0,
            ParseDefaults::default(),
            false,
        );
        assert_eq!(envs.len(), 1);
        let env = &envs[0];
        assert_eq!(env.disk_usage_bytes, 5_368_709_120);
        assert_eq!(env.port_forward_count, 1);
        assert_eq!(env.port_forwards[0].host_port, 3000);
        assert_eq!(env.port_forwards[0].guest_port, 8000);
        assert_eq!(env.extra_mount_count, 1);
        assert_eq!(env.extra_mounts[0].path, "/mnt/datasets");
        assert!(env.extra_mounts[0].readonly);
    }

    #[test]
    fn network_counters_produce_rates_and_totals_match_qt_test() {
        let make_json = |rx: i64, tx: i64| -> Vec<u8> {
            format!(
                r#"[{{
                    "name": "net-box",
                    "status": "Running",
                    "type": "container",
                    "config": {{}},
                    "state": {{
                        "network": {{
                            "eth0": {{
                                "addresses": [ {{ "family": "inet", "scope": "global", "address": "10.1.2.3" }} ],
                                "counters": {{ "bytes_received": {rx}, "bytes_sent": {tx} }}
                            }}
                        }}
                    }}
                }}]"#
            )
            .into_bytes()
        };

        let mut state = CounterState::new();
        let first = parse(
            &make_json(1_048_576, 524_288),
            "/tmp/ws",
            &mut state,
            2_000_000_000,
            ParseDefaults {
                auto_snapshot: true,
                auto_stop: false,
                record_sessions: false,
            },
            true,
        );
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].net_rx_bytes, 1_048_576);
        assert_eq!(first[0].net_tx_bytes, 524_288);
        assert_eq!(first[0].net_rx_rate, 0);
        assert_eq!(first[0].net_rx_total_text, "1.00 MB");

        let second = parse(
            &make_json(1_050_624, 525_312),
            "/tmp/ws",
            &mut state,
            2_000_000_000,
            ParseDefaults {
                auto_snapshot: true,
                auto_stop: false,
                record_sessions: false,
            },
            true,
        );
        assert_eq!(second[0].net_rx_rate, 1024);
        assert_eq!(second[0].net_tx_rate, 512);
        assert_eq!(second[0].net_rx_rate_text, "1.0 KB/s");
        assert_eq!(second[0].net_tx_rate_text, "512 B/s");
    }

    #[test]
    fn environment_serializes_every_qml_key() {
        let mut state = CounterState::new();
        let sample = br#"[{"name":"box","status":"Running","type":"container","config":{}}]"#;
        let envs = parse(
            sample,
            "/tmp/ws",
            &mut state,
            0,
            ParseDefaults::default(),
            false,
        );
        let json_val = serde_json::to_value(&envs[0]).expect("serialize");
        let obj = json_val.as_object().expect("object");
        for key in [
            "name",
            "status",
            "running",
            "type",
            "osName",
            "preset",
            "cpuLimit",
            "cpuCores",
            "cpuUsage",
            "cpuPercent",
            "cpuCorePercents",
            "memoryLimit",
            "memoryUsage",
            "memoryPercent",
            "diskLimit",
            "diskQuotaBytes",
            "diskUsage",
            "diskUsageBytes",
            "diskPercent",
            "processLimit",
            "diskReadLimit",
            "diskWriteLimit",
            "ipv4",
            "hostInterface",
            "netRxBytes",
            "netTxBytes",
            "netRxRate",
            "netTxRate",
            "netRxRateText",
            "netTxRateText",
            "netRxTotalText",
            "netTxTotalText",
            "workspacePath",
            "workspaceMountPath",
            "workspaceOnBtrfs",
            "workspaceIsSubvolume",
            "workspaceSnapshotOverride",
            "workspaceSnapshotEnabled",
            "workspaceSnapshotDir",
            "workspaceSnapshots",
            "orphanWorkspaceSnapshots",
            "workspaceSnapshotSizes",
            "hasPreSubvolBackup",
            "secretNames",
            "portForwards",
            "portForwardCount",
            "extraMounts",
            "extraMountCount",
            "snapshotCount",
            "autoSnapshotCount",
            "manualSnapshotCount",
            "snapshots",
            "snapshotSizes",
            "snapshotTotalBytes",
            "snapshotTotalDisplay",
            "autoSnapshotOverride",
            "autoSnapshotEnabled",
            "autoStopOverride",
            "autoStopOnCritical",
            "idleStopOverride",
            "idleStopIgnorePorts",
            "recordOverride",
            "recordEnabled",
            "lastUsedEpoch",
            "diskAlertLevel",
            "isTemplate",
            "networkMode",
            "networkAllowlist",
            "networkBlocked",
            "egressRecording",
        ] {
            assert!(obj.contains_key(key), "missing QML key: {key}");
        }
    }

    #[test]
    fn parse_ignores_unknown_future_incus_fields_and_null_containers() {
        let sample = br#"[
            {
                "name": "future-box",
                "status": "Stopped",
                "type": "container",
                "architecture": "x86_64",
                "ephemeral": false,
                "future_nested_field": { "a": [1, 2, 3], "b": true },
                "profiles": null,
                "config": { "limits.cpu": 2, "user.aibox.autosnap": null },
                "devices": null,
                "expanded_devices": null,
                "state": null,
                "snapshots": null
            }
        ]"#;
        let mut state = CounterState::new();
        let envs = parse(
            sample,
            "/tmp/ws",
            &mut state,
            0,
            ParseDefaults::default(),
            false,
        );
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].name, "future-box");
        assert_eq!(envs[0].cpu_cores, 2);
        assert!(!envs[0].running);
    }

    #[test]
    fn parse_if_changed_skips_identical_polls_and_detects_changes() {
        let sample = br#"[{"name":"idle-box","status":"Stopped","type":"container","config":{}}]"#;
        let mut state = CounterState::new();
        let first = parse_if_changed(
            sample,
            "/tmp/ws",
            &mut state,
            4_000_000_000,
            ParseDefaults::default(),
            true,
        );
        assert!(first.is_some());

        let second = parse_if_changed(
            sample,
            "/tmp/ws",
            &mut state,
            4_000_000_000,
            ParseDefaults::default(),
            true,
        );
        assert!(second.is_none());

        let changed = br#"[{"name":"idle-box","status":"Running","type":"container","config":{}}]"#;
        let third = parse_if_changed(
            changed,
            "/tmp/ws",
            &mut state,
            4_000_000_000,
            ParseDefaults::default(),
            true,
        );
        assert!(third.is_some());
        assert!(third.unwrap()[0].running);
    }

    #[test]
    fn benchmark_parse_20_instances_under_2ms() {
        let fixture = include_str!("../../../tests/fixtures/incus/incus_list_20_instances.json");
        let bytes = fixture.as_bytes();
        let mut state = CounterState::new();
        // Warm up host /proc/stat baseline.
        for _ in 0..5 {
            let envs = parse(
                bytes,
                "/tmp/aibox-bench-nonexistent",
                &mut state,
                4_000_000_000,
                ParseDefaults::default(),
                true,
            );
            assert_eq!(envs.len(), 20);
        }

        let iterations = 50;
        let start = Instant::now();
        for _ in 0..iterations {
            let envs = parse(
                bytes,
                "/tmp/aibox-bench-nonexistent",
                &mut state,
                4_000_000_000,
                ParseDefaults::default(),
                true,
            );
            assert_eq!(envs.len(), 20);
        }
        let avg_elapsed = start.elapsed() / iterations;
        #[cfg(not(debug_assertions))]
        assert!(
            avg_elapsed.as_micros() < 2000,
            "20-instance parse took {:?} (limit: 2ms)",
            avg_elapsed
        );
        let _ = avg_elapsed;
    }
}
