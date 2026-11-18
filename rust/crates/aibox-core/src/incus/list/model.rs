use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ParseDefaults {
    pub auto_snapshot: bool,
    pub auto_stop: bool,
    pub record_sessions: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CounterState {
    pub cpu_snapshot_ns: HashMap<String, i64>,
    pub net_counters: HashMap<String, (i64, i64)>,
    pub(crate) prev_host_idle: Vec<i64>,
    pub(crate) prev_host_total: Vec<i64>,
    pub(crate) last_list_hash: Option<u64>,
    pub(crate) last_parse_had_active_rates: bool,
}

impl CounterState {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortForwardEntry {
    pub device_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listen: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connect: Option<String>,
    pub host_port: i32,
    pub guest_port: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtraMountEntry {
    pub device_name: String,
    pub source: String,
    pub path: String,
    pub readonly: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
    pub name: String,
    pub status: String,
    pub running: bool,
    #[serde(rename = "type")]
    pub env_type: String,
    pub os_name: String,
    pub preset: String,
    pub cpu_limit: String,
    pub cpu_cores: i32,
    pub cpu_usage: String,
    pub cpu_percent: i32,
    pub cpu_core_percents: Vec<i32>,
    pub memory_limit: String,
    pub memory_usage: String,
    pub memory_percent: i32,
    pub disk_limit: String,
    pub disk_quota_bytes: i64,
    pub disk_usage: String,
    pub disk_usage_bytes: i64,
    pub disk_percent: i32,
    pub process_limit: String,
    pub disk_read_limit: String,
    pub disk_write_limit: String,
    pub ipv4: String,
    pub host_interface: String,
    pub net_rx_bytes: i64,
    pub net_tx_bytes: i64,
    pub net_rx_rate: i64,
    pub net_tx_rate: i64,
    pub net_rx_rate_text: String,
    pub net_tx_rate_text: String,
    pub net_rx_total_text: String,
    pub net_tx_total_text: String,
    pub workspace_path: String,
    pub workspace_mount_path: String,
    pub workspace_on_btrfs: bool,
    pub workspace_is_subvolume: bool,
    pub workspace_snapshot_override: String,
    pub workspace_snapshot_enabled: bool,
    pub workspace_snapshot_dir: String,
    pub workspace_snapshots: Vec<String>,
    pub orphan_workspace_snapshots: Vec<String>,
    pub workspace_snapshot_sizes: BTreeMap<String, String>,
    pub has_pre_subvol_backup: bool,
    pub secret_names: Vec<String>,
    pub port_forwards: Vec<PortForwardEntry>,
    pub port_forward_count: usize,
    pub extra_mounts: Vec<ExtraMountEntry>,
    pub extra_mount_count: usize,
    pub snapshot_count: usize,
    pub auto_snapshot_count: i32,
    pub manual_snapshot_count: i32,
    pub snapshots: Vec<String>,
    pub snapshot_sizes: BTreeMap<String, String>,
    pub snapshot_total_bytes: i64,
    pub snapshot_total_display: String,
    pub auto_snapshot_override: String,
    pub auto_snapshot_enabled: bool,
    pub auto_stop_override: String,
    pub auto_stop_on_critical: bool,
    pub idle_stop_override: String,
    pub idle_stop_ignore_ports: bool,
    pub record_override: String,
    pub record_enabled: bool,
    pub last_used_epoch: i64,
    pub disk_alert_level: i32,
    pub is_template: bool,
    pub network_mode: String,
    pub network_allowlist: String,
    pub network_blocked: bool,
    pub egress_recording: bool,
}

pub fn environment_by_name<'a>(envs: &'a [Environment], name: &str) -> Option<&'a Environment> {
    envs.iter().find(|env| env.name == name)
}

pub fn templates(envs: &[Environment]) -> Vec<&Environment> {
    envs.iter().filter(|env| env.is_template).collect()
}
