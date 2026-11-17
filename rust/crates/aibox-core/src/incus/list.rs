use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

use chrono::{DateTime, Datelike, NaiveDateTime, Utc};
use serde::Serialize;
use serde_json::Value;

use crate::secrets::store::parse_env_secret_names;
use crate::session::effective_record_sessions;
use crate::snapshot::auto::is_auto_snapshot_name;
use crate::snapshot::workspace::{
    find_orphan_workspace_snapshots, is_btrfs_filesystem, is_btrfs_subvolume,
    list_workspace_snapshots, workspace_snapshot_dir,
};
use crate::util::format::{format_byte_rate, format_byte_total, format_bytes};
use crate::util::paths::CONTAINER_WORKSPACE_PATH;
use crate::util::size::parse_size_bytes;

#[derive(Debug, Clone, Copy, Default)]
pub struct ParseDefaults {
    pub auto_snapshot: bool,
    pub auto_stop: bool,
    pub record_sessions: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CounterState {
    pub cpu_snapshot_ns: HashMap<String, i64>,
    pub net_counters: HashMap<String, (i64, i64)>,
    prev_host_idle: Vec<i64>,
    prev_host_total: Vec<i64>,
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

pub fn parse_cpu_count(cpu_limit_text: &str) -> i32 {
    let trimmed = cpu_limit_text.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("All") {
        return std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(1)
            .max(1);
    }

    if let Ok(direct) = trimmed.parse::<i32>() {
        if direct > 0 {
            return direct;
        }
    }

    let mut count = 0i32;
    for part in trimmed.split(',') {
        if part.is_empty() {
            continue;
        }
        let p = part.trim();
        if let Some(dash) = p.find('-') {
            if dash > 0 {
                let start = p[..dash].trim().parse::<i32>();
                let end = p[dash + 1..].trim().parse::<i32>();
                if let (Ok(s), Ok(e)) = (start, end) {
                    if e >= s {
                        count += e - s + 1;
                    }
                }
                continue;
            }
        }
        count += 1;
    }
    count.max(1)
}

pub fn format_percent(percent: i32) -> String {
    format!("{}%", percent.clamp(0, 100))
}

pub fn build_cpu_core_percents(
    core_count: i32,
    avg_percent: i32,
    running: bool,
    host_weights: &[i32],
) -> Vec<i32> {
    let n = core_count.clamp(1, 64) as usize;
    if !running || avg_percent <= 0 {
        return vec![0; n];
    }

    let mut weights = vec![1.0_f64; n];
    let mut weight_sum = 0.0_f64;
    for i in 0..n {
        let w = if i < host_weights.len() && host_weights[i] > 0 {
            f64::from(host_weights[i]) + 5.0
        } else {
            1.0 + 0.12 * (((i * 3) % n) as f64 - (n - 1) as f64 / 2.0)
        };
        let clamped = w.max(0.2);
        weights[i] = clamped;
        weight_sum += clamped;
    }

    let target_sum = avg_percent * n as i32;
    let mut current_sum = 0i32;
    let mut values = vec![0i32; n];
    for i in 0..n {
        let v = (((weights[i] / weight_sum) * f64::from(target_sum)).round() as i32).clamp(0, 100);
        values[i] = v;
        current_sum += v;
    }

    let mut diff = target_sum - current_sum;
    for v in &mut values {
        if diff == 0 {
            break;
        }
        if diff > 0 && *v < 100 {
            let add = diff.min(100 - *v);
            *v += add;
            diff -= add;
        } else if diff < 0 && *v > 0 {
            let sub = (-diff).min(*v);
            *v -= sub;
            diff += sub;
        }
    }

    values
}

fn read_host_proc_stat_idle_and_total() -> (Vec<i64>, Vec<i64>) {
    let mut idle = Vec::new();
    let mut totals = Vec::new();
    let Ok(contents) = fs::read_to_string("/proc/stat") else {
        return (idle, totals);
    };

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        let bytes = line.as_bytes();
        if !line.starts_with("cpu") || bytes.len() < 4 || !bytes[3].is_ascii_digit() {
            continue;
        }
        let mut total = 0i64;
        let mut idle_val = 0i64;
        let mut col = 0i32;
        for part in line.split(' ').skip(1) {
            if part.is_empty() {
                continue;
            }
            let v = part.parse::<i64>().unwrap_or(0);
            total += v;
            if col == 3 || col == 4 {
                idle_val += v;
            }
            col += 1;
        }
        idle.push(idle_val);
        totals.push(total);
    }
    (idle, totals)
}

fn json_i64(v: Option<&Value>) -> i64 {
    let Some(val) = v else {
        return 0;
    };
    if let Some(n) = val.as_i64() {
        n
    } else if let Some(u) = val.as_u64() {
        u as i64
    } else if let Some(f) = val.as_f64() {
        f as i64
    } else if let Some(s) = val.as_str() {
        s.trim().parse::<i64>().unwrap_or(0)
    } else {
        0
    }
}

fn json_str<'a>(obj: &'a serde_json::Map<String, Value>, key: &str) -> &'a str {
    obj.get(key).and_then(Value::as_str).unwrap_or("")
}

fn parse_started_at_ms(raw: &str) -> Option<i64> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        if dt.year() > 1970 {
            return Some(dt.with_timezone(&Utc).timestamp_millis());
        }
        return None;
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S"] {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(s, fmt) {
            if ndt.year() > 1970 {
                return Some(ndt.and_utc().timestamp_millis());
            }
            return None;
        }
    }
    None
}

pub fn parse(
    json: &[u8],
    workspace_base: &str,
    state: &mut CounterState,
    elapsed_wall_ns: i64,
    defaults: ParseDefaults,
    track_cpu_and_net: bool,
) -> Vec<Environment> {
    let now_utc_ms = Utc::now().timestamp_millis();
    parse_with_now(
        json,
        workspace_base,
        state,
        elapsed_wall_ns,
        defaults,
        track_cpu_and_net,
        now_utc_ms,
    )
}

pub fn parse_with_now(
    json: &[u8],
    workspace_base: &str,
    state: &mut CounterState,
    elapsed_wall_ns: i64,
    defaults: ParseDefaults,
    track_cpu_and_net: bool,
    now_utc_ms: i64,
) -> Vec<Environment> {
    let Ok(Value::Array(items)) = serde_json::from_slice::<Value>(json) else {
        return Vec::new();
    };

    let mut next_cpu_snapshot = HashMap::new();
    let mut next_net_snapshot = HashMap::new();

    let (curr_host_idle, curr_host_total) = read_host_proc_stat_idle_and_total();
    let mut host_core_weights = Vec::new();
    if curr_host_idle.len() == state.prev_host_idle.len()
        && curr_host_total.len() == state.prev_host_total.len()
    {
        for i in 0..curr_host_total.len() {
            let d_total = curr_host_total[i] - state.prev_host_total[i];
            let d_idle = curr_host_idle[i] - state.prev_host_idle[i];
            let pct = if d_total > 0 {
                (((d_total - d_idle) * 100) / d_total).clamp(0, 100) as i32
            } else {
                0
            };
            host_core_weights.push(pct);
        }
    }
    if !curr_host_total.is_empty() {
        state.prev_host_idle = curr_host_idle;
        state.prev_host_total = curr_host_total;
    }

    let mut result = Vec::with_capacity(items.len());

    for val in &items {
        let Some(obj) = val.as_object() else {
            continue;
        };
        let name = json_str(obj, "name");
        if name.is_empty() {
            continue;
        }

        let status = json_str(obj, "status").to_uppercase();
        let running = status == "RUNNING";
        let env_type = json_str(obj, "type").to_uppercase();

        let empty_map = serde_json::Map::new();
        let config = obj
            .get("config")
            .and_then(Value::as_object)
            .unwrap_or(&empty_map);

        let mut os_name = json_str(config, "image.description").to_string();
        if os_name.is_empty() {
            let os = json_str(config, "image.os");
            let rel = json_str(config, "image.release");
            os_name = if !os.is_empty() {
                format!("{os} {rel}").trim().to_string()
            } else {
                "Linux Environment".to_string()
            };
        }
        if let Some(paren_idx) = os_name.find('(') {
            if paren_idx > 0 {
                os_name = os_name[..paren_idx].trim().to_string();
            }
        }

        let cpu_limit_raw = config
            .get("limits.cpu")
            .and_then(Value::as_str)
            .unwrap_or("All");
        let cpu_core_count = parse_cpu_count(cpu_limit_raw);
        let memory_limit = config
            .get("limits.memory")
            .and_then(Value::as_str)
            .unwrap_or("Dynamic")
            .to_string();
        let process_limit = config
            .get("limits.processes")
            .and_then(Value::as_str)
            .unwrap_or("Unlimited")
            .to_string();

        let expanded_devices = obj
            .get("expanded_devices")
            .and_then(Value::as_object)
            .unwrap_or(&empty_map);
        let devices = obj
            .get("devices")
            .and_then(Value::as_object)
            .unwrap_or(&empty_map);

        let dev_root = devices
            .get("root")
            .and_then(Value::as_object)
            .unwrap_or(&empty_map);
        let exp_root = expanded_devices
            .get("root")
            .and_then(Value::as_object)
            .unwrap_or(&empty_map);

        let mut disk_limit = json_str(dev_root, "size");
        if disk_limit.is_empty() {
            disk_limit = exp_root
                .get("size")
                .and_then(Value::as_str)
                .unwrap_or("Shared");
        }
        let mut disk_read_limit = json_str(dev_root, "limits.read");
        if disk_read_limit.is_empty() {
            disk_read_limit = exp_root
                .get("limits.read")
                .and_then(Value::as_str)
                .unwrap_or("Unlimited");
        }
        let mut disk_write_limit = json_str(dev_root, "limits.write");
        if disk_write_limit.is_empty() {
            disk_write_limit = exp_root
                .get("limits.write")
                .and_then(Value::as_str)
                .unwrap_or("Unlimited");
        }

        let mut preset = "Standard".to_string();
        if let Some(profiles) = obj.get("profiles").and_then(Value::as_array) {
            if profiles.iter().any(|p| p.as_str() == Some("ai-gui")) {
                preset = "Desktop & Mobile (GPU + KVM)".to_string();
            }
        }
        if env_type == "VIRTUAL-MACHINE" {
            preset = "Full KVM Virtual Machine".to_string();
        }

        let mut ipv4 = "\u{2014}".to_string();
        let mut host_iface = json_str(config, "volatile.eth0.host_name").to_string();
        let mut memory_usage = "0 MB".to_string();
        let mut disk_usage = "0 MB".to_string();
        let mut mem_bytes = 0i64;
        let mut mem_total_bytes = parse_size_bytes(&memory_limit);
        let mut disk_bytes = 0i64;
        let mut disk_total_bytes = parse_size_bytes(disk_limit);
        let mut cpu_percent = 0i32;
        let mut net_rx_bytes = 0i64;
        let mut net_tx_bytes = 0i64;

        if let Some(state_obj) = obj.get("state").and_then(Value::as_object) {
            if !state_obj.is_empty() {
                let mem_obj = state_obj
                    .get("memory")
                    .and_then(Value::as_object)
                    .unwrap_or(&empty_map);
                mem_bytes = json_i64(mem_obj.get("usage"));
                if mem_bytes > 0 {
                    memory_usage = format_bytes(mem_bytes);
                }
                let state_mem_total = json_i64(mem_obj.get("total"));
                if mem_total_bytes <= 0 && state_mem_total > 0 {
                    mem_total_bytes = state_mem_total;
                }

                let root_disk_obj = state_obj
                    .get("disk")
                    .and_then(Value::as_object)
                    .and_then(|d| d.get("root"))
                    .and_then(Value::as_object)
                    .unwrap_or(&empty_map);
                disk_bytes = json_i64(root_disk_obj.get("usage"));
                if disk_bytes > 0 {
                    disk_usage = format_bytes(disk_bytes);
                }
                let state_disk_total = json_i64(root_disk_obj.get("total"));
                if disk_total_bytes <= 0 && state_disk_total > 0 {
                    disk_total_bytes = state_disk_total;
                }

                let cpu_obj = state_obj
                    .get("cpu")
                    .and_then(Value::as_object)
                    .unwrap_or(&empty_map);
                let cpu_usage_ns = json_i64(cpu_obj.get("usage"));
                if running && cpu_usage_ns >= 0 {
                    next_cpu_snapshot.insert(name.to_string(), cpu_usage_ns);
                    if track_cpu_and_net
                        && elapsed_wall_ns > 0
                        && state.cpu_snapshot_ns.contains_key(name)
                    {
                        let prev_ns = state.cpu_snapshot_ns[name];
                        let delta_ns = cpu_usage_ns - prev_ns;
                        if delta_ns >= 0 {
                            let capacity_ns =
                                (elapsed_wall_ns as f64) * f64::from(cpu_core_count.max(1));
                            cpu_percent = (((delta_ns as f64) * 100.0 / capacity_ns).round()
                                as i32)
                                .clamp(0, 100);
                        }
                    } else if let Some(started_ms) =
                        parse_started_at_ms(json_str(state_obj, "started_at"))
                    {
                        let uptime_ms = now_utc_ms - started_ms;
                        if uptime_ms > 500 && cpu_usage_ns > 0 {
                            let capacity_ns =
                                (uptime_ms as f64) * 1_000_000.0 * f64::from(cpu_core_count.max(1));
                            cpu_percent = (((cpu_usage_ns as f64) * 100.0 / capacity_ns).round()
                                as i32)
                                .clamp(0, 100);
                        }
                    }
                }

                if let Some(net) = state_obj.get("network").and_then(Value::as_object) {
                    for (iface_name, iface_val) in net {
                        if iface_name == "lo" {
                            continue;
                        }
                        let Some(iface_obj) = iface_val.as_object() else {
                            continue;
                        };
                        let iface_host = json_str(iface_obj, "host_name");
                        if !iface_host.is_empty() && host_iface.is_empty() {
                            host_iface = iface_host.to_string();
                        }
                        if let Some(counters) = iface_obj.get("counters").and_then(Value::as_object)
                        {
                            net_rx_bytes += json_i64(counters.get("bytes_received"));
                            net_tx_bytes += json_i64(counters.get("bytes_sent"));
                        }
                        if ipv4 != "\u{2014}" {
                            continue;
                        }
                        if let Some(addrs) = iface_obj.get("addresses").and_then(Value::as_array) {
                            for addr_val in addrs {
                                let Some(addr_obj) = addr_val.as_object() else {
                                    continue;
                                };
                                if json_str(addr_obj, "family") == "inet"
                                    && json_str(addr_obj, "scope") == "global"
                                {
                                    ipv4 = json_str(addr_obj, "address").to_string();
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut net_rx_rate = 0i64;
        let mut net_tx_rate = 0i64;
        if track_cpu_and_net && running {
            if elapsed_wall_ns > 0 {
                if let Some(&(prev_rx, prev_tx)) = state.net_counters.get(name) {
                    if net_rx_bytes >= prev_rx {
                        net_rx_rate = (((net_rx_bytes - prev_rx) as f64) * 1e9
                            / (elapsed_wall_ns as f64))
                            .round() as i64;
                    }
                    if net_tx_bytes >= prev_tx {
                        net_tx_rate = (((net_tx_bytes - prev_tx) as f64) * 1e9
                            / (elapsed_wall_ns as f64))
                            .round() as i64;
                    }
                }
            }
            next_net_snapshot.insert(name.to_string(), (net_rx_bytes, net_tx_bytes));
        }

        let memory_percent = if mem_total_bytes > 0 && mem_bytes > 0 {
            (((mem_bytes as f64) * 100.0 / (mem_total_bytes as f64)).round() as i32).clamp(0, 100)
        } else {
            0
        };
        let disk_percent = if disk_total_bytes > 0 && disk_bytes > 0 {
            (((disk_bytes as f64) * 100.0 / (disk_total_bytes as f64)).round() as i32).clamp(0, 100)
        } else {
            0
        };

        let mut snapshots = Vec::new();
        let mut snapshot_sizes = BTreeMap::new();
        let mut snapshot_total_bytes = 0i64;
        let mut auto_snap_count = 0i32;
        if let Some(snap_arr) = obj.get("snapshots").and_then(Value::as_array) {
            for snap_val in snap_arr {
                let Some(snap_obj) = snap_val.as_object() else {
                    continue;
                };
                let snap_name = json_str(snap_obj, "name");
                if !snap_name.is_empty() {
                    snapshots.push(snap_name.to_string());
                    if is_auto_snapshot_name(snap_name) {
                        auto_snap_count += 1;
                    }
                    let snap_size = json_i64(snap_obj.get("size"));
                    if snap_size > 0 {
                        snapshot_sizes.insert(snap_name.to_string(), format_bytes(snap_size));
                        snapshot_total_bytes += snap_size;
                    }
                }
            }
        }

        let auto_snap_override = json_str(config, "user.aibox.autosnap")
            .trim()
            .to_lowercase();
        let env_auto_snap = if auto_snap_override.is_empty() {
            defaults.auto_snapshot
        } else {
            auto_snap_override == "true"
        };
        let auto_stop_override = json_str(config, "user.aibox.autostop")
            .trim()
            .to_lowercase();
        let env_auto_stop = if auto_stop_override.is_empty() {
            defaults.auto_stop
        } else {
            auto_stop_override == "true"
        };
        let idle_stop_override = json_str(config, "user.aibox.idlestop")
            .trim()
            .to_lowercase();
        let idle_stop_ignore_ports = json_str(config, "user.aibox.idlestop_ports")
            .trim()
            .to_lowercase()
            == "true";
        let record_override = json_str(config, "user.aibox.record").trim().to_lowercase();
        let env_record_enabled =
            effective_record_sessions(defaults.record_sessions, &record_override);
        let last_used_epoch = json_str(config, "user.aibox.lastused")
            .trim()
            .parse::<i64>()
            .ok()
            .filter(|&v| v > 0)
            .unwrap_or(0);
        let is_template = json_str(config, "user.aibox.template")
            .trim()
            .to_lowercase()
            == "true";
        let egress_recording = json_str(config, "user.aibox.egresslog")
            .trim()
            .to_lowercase()
            == "true";

        let mut net_mode = json_str(config, "user.aibox.netmode").trim().to_lowercase();
        let eth0_dev = devices
            .get("eth0")
            .and_then(Value::as_object)
            .unwrap_or(&empty_map);
        if net_mode != "online" && net_mode != "offline" && net_mode != "allowlist" {
            let eth0_acls = json_str(eth0_dev, "security.acls");
            if json_str(eth0_dev, "type") == "none" {
                net_mode = "offline".to_string();
            } else if !eth0_acls.is_empty() && eth0_acls != format!("aibox-log-{name}") {
                net_mode = "allowlist".to_string();
            } else {
                net_mode = "online".to_string();
            }
        }
        let net_allowlist = json_str(config, "user.aibox.netallow").to_string();

        let exp_ws = expanded_devices
            .get("workspace")
            .and_then(Value::as_object)
            .unwrap_or(&empty_map);
        let dev_ws = devices
            .get("workspace")
            .and_then(Value::as_object)
            .unwrap_or(&empty_map);

        let mut workspace_path = json_str(exp_ws, "source").to_string();
        if workspace_path.is_empty() {
            workspace_path = json_str(dev_ws, "source").to_string();
        }
        if workspace_path.is_empty() {
            workspace_path = format!("{workspace_base}/{name}");
        }

        let mut workspace_mount_path = json_str(exp_ws, "path").to_string();
        if workspace_mount_path.is_empty() {
            workspace_mount_path = json_str(dev_ws, "path").to_string();
        }
        if workspace_mount_path.is_empty() {
            workspace_mount_path = CONTAINER_WORKSPACE_PATH.to_string();
        }

        let ws_snap_override = json_str(config, "user.aibox.wssnap").trim().to_lowercase();
        let ws_on_btrfs = is_btrfs_filesystem(&workspace_path);
        let ws_is_subvolume = is_btrfs_subvolume(&workspace_path);
        let ws_snap_enabled = ws_is_subvolume && ws_snap_override != "false";
        let ws_snap_dir = workspace_snapshot_dir(&workspace_path, name);
        let ws_snapshots = list_workspace_snapshots(&workspace_path, name);
        let orphan_ws_snapshots = find_orphan_workspace_snapshots(&snapshots, &ws_snapshots);
        let has_pre_subvol_backup = Path::new(&format!("{workspace_path}.pre-subvol")).exists();
        let secret_names = parse_env_secret_names(json_str(config, "user.aibox.secrets"));

        let mut port_forwards = Vec::new();
        let mut extra_mounts = Vec::new();
        for (dev_name, dev_val) in devices {
            let Some(dev_obj) = dev_val.as_object() else {
                continue;
            };
            let dev_type = json_str(dev_obj, "type");
            if dev_type == "proxy" {
                let listen_addr = json_str(dev_obj, "listen");
                let connect_addr = json_str(dev_obj, "connect");
                let host_port = listen_addr
                    .rsplit(':')
                    .next()
                    .and_then(|s| s.trim().parse::<i32>().ok())
                    .unwrap_or(0);
                let guest_port = connect_addr
                    .rsplit(':')
                    .next()
                    .and_then(|s| s.trim().parse::<i32>().ok())
                    .unwrap_or(0);
                if host_port > 0 {
                    port_forwards.push(PortForwardEntry {
                        device_name: dev_name.clone(),
                        listen: Some(listen_addr.to_string()),
                        connect: Some(connect_addr.to_string()),
                        host_port,
                        guest_port: if guest_port > 0 {
                            guest_port
                        } else {
                            host_port
                        },
                    });
                }
            } else if dev_type == "disk"
                && dev_name != "root"
                && dev_name != "workspace"
                && dev_name != "x11"
            {
                let src = json_str(dev_obj, "source").to_string();
                let mnt_path = json_str(dev_obj, "path").to_string();
                let ro_str = json_str(dev_obj, "readonly").trim().to_lowercase();
                let is_ro = ro_str == "true" || ro_str == "1";
                extra_mounts.push(ExtraMountEntry {
                    device_name: dev_name.clone(),
                    source: src,
                    path: mnt_path,
                    readonly: is_ro,
                });
            }
        }

        let cpu_limit_display = if cpu_limit_raw == "All" {
            "All Cores".to_string()
        } else {
            format!("{cpu_limit_raw} Cores")
        };
        let disk_quota_bytes = if disk_limit.eq_ignore_ascii_case("Shared") {
            0
        } else {
            parse_size_bytes(disk_limit)
        };

        let snapshot_count = snapshots.len();
        let manual_snapshot_count = snapshot_count as i32 - auto_snap_count;
        let snapshot_total_display = if snapshot_total_bytes > 0 {
            format_bytes(snapshot_total_bytes)
        } else {
            String::new()
        };
        let port_forward_count = port_forwards.len();
        let extra_mount_count = extra_mounts.len();
        let network_blocked = net_mode == "offline";

        result.push(Environment {
            name: name.to_string(),
            status,
            running,
            env_type,
            os_name,
            preset,
            cpu_limit: cpu_limit_display,
            cpu_cores: cpu_core_count,
            cpu_usage: format_percent(cpu_percent),
            cpu_percent,
            cpu_core_percents: build_cpu_core_percents(
                cpu_core_count,
                cpu_percent,
                running,
                &host_core_weights,
            ),
            memory_limit,
            memory_usage,
            memory_percent,
            disk_limit: disk_limit.to_string(),
            disk_quota_bytes,
            disk_usage,
            disk_usage_bytes: disk_bytes,
            disk_percent,
            process_limit,
            disk_read_limit: disk_read_limit.to_string(),
            disk_write_limit: disk_write_limit.to_string(),
            ipv4,
            host_interface: host_iface,
            net_rx_bytes,
            net_tx_bytes,
            net_rx_rate,
            net_tx_rate,
            net_rx_rate_text: format_byte_rate(net_rx_rate),
            net_tx_rate_text: format_byte_rate(net_tx_rate),
            net_rx_total_text: format_byte_total(net_rx_bytes),
            net_tx_total_text: format_byte_total(net_tx_bytes),
            workspace_path,
            workspace_mount_path,
            workspace_on_btrfs: ws_on_btrfs,
            workspace_is_subvolume: ws_is_subvolume,
            workspace_snapshot_override: ws_snap_override,
            workspace_snapshot_enabled: ws_snap_enabled,
            workspace_snapshot_dir: ws_snap_dir,
            workspace_snapshots: ws_snapshots,
            orphan_workspace_snapshots: orphan_ws_snapshots,
            workspace_snapshot_sizes: BTreeMap::new(),
            has_pre_subvol_backup,
            secret_names,
            port_forwards,
            port_forward_count,
            extra_mounts,
            extra_mount_count,
            snapshot_count,
            auto_snapshot_count: auto_snap_count,
            manual_snapshot_count,
            snapshots,
            snapshot_sizes,
            snapshot_total_bytes,
            snapshot_total_display,
            auto_snapshot_override: auto_snap_override,
            auto_snapshot_enabled: env_auto_snap,
            auto_stop_override,
            auto_stop_on_critical: env_auto_stop,
            idle_stop_override,
            idle_stop_ignore_ports,
            record_override,
            record_enabled: env_record_enabled,
            last_used_epoch,
            disk_alert_level: 0,
            is_template,
            network_mode: net_mode,
            network_allowlist: net_allowlist,
            network_blocked,
            egress_recording,
        });
    }

    if track_cpu_and_net {
        state.cpu_snapshot_ns = next_cpu_snapshot;
        state.net_counters = next_net_snapshot;
    }

    result
}

pub fn parse_json(
    json: &[u8],
    workspace_base: &str,
    state: &mut CounterState,
    elapsed_wall_ns: i64,
    defaults: ParseDefaults,
    track_cpu_and_net: bool,
) -> String {
    let envs = parse(
        json,
        workspace_base,
        state,
        elapsed_wall_ns,
        defaults,
        track_cpu_and_net,
    );
    serde_json::to_string(&envs).unwrap_or_else(|_| "[]".to_string())
}

pub fn environment_by_name<'a>(envs: &'a [Environment], name: &str) -> Option<&'a Environment> {
    envs.iter().find(|env| env.name == name)
}

pub fn templates(envs: &[Environment]) -> Vec<&Environment> {
    envs.iter().filter(|env| env.is_template).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
