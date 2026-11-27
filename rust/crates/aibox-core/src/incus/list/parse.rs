use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use chrono::Utc;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

use crate::health::swap::host_swap_info;
use crate::secrets::store::parse_env_secret_names;
use crate::session::effective_record_sessions;
use crate::snapshot::auto::is_auto_snapshot_name;
use crate::snapshot::workspace::{
    find_orphan_workspace_snapshots, is_btrfs_filesystem, list_workspace_snapshots,
    workspace_snapshot_dir,
};
use crate::util::format::{format_byte_rate, format_byte_total, format_bytes};
use crate::util::paths::CONTAINER_WORKSPACE_PATH;
use crate::util::size::parse_size_bytes;

use super::metrics::{
    build_cpu_core_percents, compute_cpu_percent, compute_net_rates, compute_usage_percent,
    format_percent, parse_cpu_count, read_host_core_weights, MetricPollContext,
};
use super::model::{CounterState, Environment, ParseDefaults};
use super::quota::{extract_port_forwards_and_mounts, extract_root_limits};

#[derive(Debug, Clone, Copy, Default)]
struct FlexibleI64(i64);

impl<'de> Deserialize<'de> for FlexibleI64 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct I64Visitor;
        impl<'de> Visitor<'de> for I64Visitor {
            type Value = FlexibleI64;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an integer, float, string, or null")
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(FlexibleI64(v))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(FlexibleI64(v as i64))
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                Ok(FlexibleI64(v as i64))
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(FlexibleI64(v.trim().parse::<i64>().unwrap_or(0)))
            }

            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(FlexibleI64(0))
            }

            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(FlexibleI64(0))
            }
        }
        deserializer.deserialize_any(I64Visitor)
    }
}

#[derive(Debug, Clone, Default)]
struct FlexibleString(String);

impl<'de> Deserialize<'de> for FlexibleString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct StrVisitor;
        impl<'de> Visitor<'de> for StrVisitor {
            type Value = FlexibleString;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a string, scalar, or null")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(FlexibleString(v.to_string()))
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(FlexibleString(v))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(FlexibleString(v.to_string()))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(FlexibleString(v.to_string()))
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                Ok(FlexibleString(v.to_string()))
            }

            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(FlexibleString(v.to_string()))
            }

            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(FlexibleString(String::new()))
            }

            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(FlexibleString(String::new()))
            }
        }
        deserializer.deserialize_any(StrVisitor)
    }
}

fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    T: Default + Deserialize<'de>,
    D: Deserializer<'de>,
{
    let opt = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

fn deserialize_string_map<'de, D>(deserializer: D) -> Result<HashMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct MapVisitor;
    impl<'de> Visitor<'de> for MapVisitor {
        type Value = HashMap<String, String>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a map of string keys to scalar values, or null")
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(HashMap::new())
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(HashMap::new())
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut out = HashMap::new();
            while let Some((k, v)) = map.next_entry::<String, FlexibleString>()? {
                out.insert(k, v.0);
            }
            Ok(out)
        }
    }
    deserializer.deserialize_any(MapVisitor)
}

fn deserialize_btreemap_string<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct MapVisitor;
    impl<'de> Visitor<'de> for MapVisitor {
        type Value = BTreeMap<String, String>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a map of string keys to scalar values, or null")
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(BTreeMap::new())
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(BTreeMap::new())
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut out = BTreeMap::new();
            while let Some((k, v)) = map.next_entry::<String, FlexibleString>()? {
                out.insert(k, v.0);
            }
            Ok(out)
        }
    }
    deserializer.deserialize_any(MapVisitor)
}

fn deserialize_devices_map<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, BTreeMap<String, String>>, D::Error>
where
    D: Deserializer<'de>,
{
    struct DevicesVisitor;
    impl<'de> Visitor<'de> for DevicesVisitor {
        type Value = BTreeMap<String, BTreeMap<String, String>>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a map of device maps, or null")
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(BTreeMap::new())
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(BTreeMap::new())
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            #[derive(Deserialize)]
            struct DevWrap(
                #[serde(deserialize_with = "deserialize_btreemap_string")] BTreeMap<String, String>,
            );

            let mut out = BTreeMap::new();
            while let Some((k, v)) = map.next_entry::<String, DevWrap>()? {
                out.insert(k, v.0);
            }
            Ok(out)
        }
    }
    deserializer.deserialize_any(DevicesVisitor)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawIncusInstance {
    #[serde(deserialize_with = "deserialize_null_default")]
    name: String,
    #[serde(deserialize_with = "deserialize_null_default")]
    status: String,
    #[serde(rename = "type", deserialize_with = "deserialize_null_default")]
    env_type: String,
    #[serde(deserialize_with = "deserialize_null_default")]
    profiles: Vec<FlexibleString>,
    #[serde(deserialize_with = "deserialize_string_map")]
    config: HashMap<String, String>,
    #[serde(deserialize_with = "deserialize_devices_map")]
    devices: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(deserialize_with = "deserialize_devices_map")]
    expanded_devices: BTreeMap<String, BTreeMap<String, String>>,
    state: Option<RawState>,
    #[serde(deserialize_with = "deserialize_null_default")]
    snapshots: Vec<RawSnapshot>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawState {
    #[serde(deserialize_with = "deserialize_null_default")]
    started_at: String,
    #[serde(deserialize_with = "deserialize_null_default")]
    memory: RawMemoryState,
    #[serde(deserialize_with = "deserialize_null_default")]
    disk: RawDiskMap,
    #[serde(deserialize_with = "deserialize_null_default")]
    cpu: RawCpuState,
    #[serde(deserialize_with = "deserialize_null_default")]
    network: BTreeMap<String, RawNetInterface>,
}

impl RawState {
    fn is_empty(&self) -> bool {
        self.started_at.is_empty()
            && self.memory.usage.0 == 0
            && self.memory.total.0 == 0
            && self.memory.swap_usage.0 == 0
            && self.disk.root.usage.0 == 0
            && self.disk.root.total.0 == 0
            && self.cpu.usage.0 == 0
            && self.network.is_empty()
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawMemoryState {
    usage: FlexibleI64,
    total: FlexibleI64,
    swap_usage: FlexibleI64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawDiskMap {
    #[serde(deserialize_with = "deserialize_null_default")]
    root: RawDiskState,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawDiskState {
    usage: FlexibleI64,
    total: FlexibleI64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawCpuState {
    usage: FlexibleI64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawNetInterface {
    #[serde(deserialize_with = "deserialize_null_default")]
    host_name: String,
    #[serde(deserialize_with = "deserialize_null_default")]
    counters: RawNetCounters,
    #[serde(deserialize_with = "deserialize_null_default")]
    addresses: Vec<RawNetAddress>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawNetCounters {
    bytes_received: FlexibleI64,
    bytes_sent: FlexibleI64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawNetAddress {
    #[serde(deserialize_with = "deserialize_null_default")]
    family: String,
    #[serde(deserialize_with = "deserialize_null_default")]
    scope: String,
    #[serde(deserialize_with = "deserialize_null_default")]
    address: String,
}

#[derive(Debug, Default)]
struct RawSnapshot {
    name: String,
    size: FlexibleI64,
}

impl<'de> Deserialize<'de> for RawSnapshot {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SnapVisitor;
        impl<'de> Visitor<'de> for SnapVisitor {
            type Value = RawSnapshot;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a snapshot object or ignored scalar")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut snap = RawSnapshot::default();
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "name" => {
                            let val: FlexibleString = map.next_value()?;
                            snap.name = val.0;
                        }
                        "size" => {
                            snap.size = map.next_value()?;
                        }
                        _ => {
                            let _: de::IgnoredAny = map.next_value()?;
                        }
                    }
                }
                Ok(snap)
            }

            fn visit_str<E: de::Error>(self, _v: &str) -> Result<Self::Value, E> {
                Ok(RawSnapshot::default())
            }

            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(RawSnapshot::default())
            }

            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(RawSnapshot::default())
            }
        }
        deserializer.deserialize_any(SnapVisitor)
    }
}

fn map_str<'a>(map: &'a HashMap<String, String>, key: &str) -> &'a str {
    map.get(key).map(String::as_str).unwrap_or("")
}

fn btree_str<'a>(map: &'a BTreeMap<String, String>, key: &str) -> &'a str {
    map.get(key).map(String::as_str).unwrap_or("")
}

/// Computes a fast 64-bit FNV-1a hash of raw `incus list` bytes and parse inputs
/// so identical polls can skip rebuilding the environment model.
pub fn hash_list_inputs(json: &[u8], workspace_base: &str, defaults: ParseDefaults) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    let feed = |h: &mut u64, bytes: &[u8]| {
        for &b in bytes {
            *h ^= u64::from(b);
            *h = h.wrapping_mul(0x100000001b3);
        }
    };
    feed(&mut hash, json);
    feed(&mut hash, &[0xff]);
    feed(&mut hash, workspace_base.as_bytes());
    feed(
        &mut hash,
        &[
            u8::from(defaults.auto_snapshot),
            u8::from(defaults.auto_stop),
            u8::from(defaults.record_sessions),
        ],
    );
    hash
}

pub fn parse(
    json: &[u8],
    workspace_base: &str,
    state: &mut CounterState,
    elapsed_wall_ns: i64,
    defaults: ParseDefaults,
    track_cpu_and_net: bool,
) -> Vec<Environment> {
    parse_checked(
        json,
        workspace_base,
        state,
        elapsed_wall_ns,
        defaults,
        track_cpu_and_net,
    )
    .unwrap_or_default()
}

/// Parses `incus list` JSON in a single pass, returning a `serde_json::Error` on invalid JSON.
pub fn parse_checked(
    json: &[u8],
    workspace_base: &str,
    state: &mut CounterState,
    elapsed_wall_ns: i64,
    defaults: ParseDefaults,
    track_cpu_and_net: bool,
) -> Result<Vec<Environment>, serde_json::Error> {
    let now_utc_ms = Utc::now().timestamp_millis();
    parse_with_now_checked(
        json,
        workspace_base,
        state,
        elapsed_wall_ns,
        defaults,
        track_cpu_and_net,
        now_utc_ms,
    )
}

/// Parses `incus list` JSON only if the payload/defaults changed or the previous
/// poll had active CPU/network rates that need to settle. Returns `None` when skipped.
pub fn parse_if_changed(
    json: &[u8],
    workspace_base: &str,
    state: &mut CounterState,
    elapsed_wall_ns: i64,
    defaults: ParseDefaults,
    track_cpu_and_net: bool,
) -> Option<Vec<Environment>> {
    parse_if_changed_checked(
        json,
        workspace_base,
        state,
        elapsed_wall_ns,
        defaults,
        track_cpu_and_net,
    )
    .unwrap_or_else(|_| Some(Vec::new()))
}

/// Checked variant of `parse_if_changed` that validates/parses JSON in a single pass.
pub fn parse_if_changed_checked(
    json: &[u8],
    workspace_base: &str,
    state: &mut CounterState,
    elapsed_wall_ns: i64,
    defaults: ParseDefaults,
    track_cpu_and_net: bool,
) -> Result<Option<Vec<Environment>>, serde_json::Error> {
    let input_hash = hash_list_inputs(json, workspace_base, defaults);
    if state.last_list_hash == Some(input_hash) && !state.last_parse_had_active_rates {
        return Ok(None);
    }

    let envs = parse_checked(
        json,
        workspace_base,
        state,
        elapsed_wall_ns,
        defaults,
        track_cpu_and_net,
    )?;
    state.last_list_hash = Some(input_hash);
    state.last_parse_had_active_rates = envs
        .iter()
        .any(|e| e.running && (e.cpu_percent > 0 || e.net_rx_rate > 0 || e.net_tx_rate > 0));
    Ok(Some(envs))
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
    parse_with_now_checked(
        json,
        workspace_base,
        state,
        elapsed_wall_ns,
        defaults,
        track_cpu_and_net,
        now_utc_ms,
    )
    .unwrap_or_default()
}

pub fn parse_with_now_checked(
    json: &[u8],
    workspace_base: &str,
    state: &mut CounterState,
    elapsed_wall_ns: i64,
    defaults: ParseDefaults,
    track_cpu_and_net: bool,
    now_utc_ms: i64,
) -> Result<Vec<Environment>, serde_json::Error> {
    let items = serde_json::from_slice::<Vec<RawIncusInstance>>(json)?;

    let mut next_cpu_snapshot = HashMap::new();
    let mut next_net_snapshot = HashMap::new();
    let mut btrfs_parent_cache: HashMap<String, bool> = HashMap::new();
    let mut cached_host_swap_total: Option<i64> = None;
    let host_core_weights = if track_cpu_and_net {
        read_host_core_weights(state)
    } else {
        Vec::new()
    };
    let poll_ctx = MetricPollContext {
        elapsed_wall_ns,
        now_utc_ms,
        track_cpu_and_net,
        prev_state: state,
    };

    let mut result = Vec::with_capacity(items.len());

    for item in &items {
        let name = item.name.as_str();
        if name.is_empty() {
            continue;
        }

        let status = item.status.to_uppercase();
        let running = status == "RUNNING";
        let env_type = item.env_type.to_uppercase();
        let config = &item.config;

        let mut os_name = map_str(config, "image.description").to_string();
        if os_name.is_empty() {
            let os = map_str(config, "image.os");
            let rel = map_str(config, "image.release");
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
            .map(String::as_str)
            .unwrap_or("All");
        let cpu_core_count = parse_cpu_count(cpu_limit_raw);
        let memory_limit = config
            .get("limits.memory")
            .map(String::as_str)
            .unwrap_or("Dynamic")
            .to_string();
        let process_limit = config
            .get("limits.processes")
            .map(String::as_str)
            .unwrap_or("Unlimited")
            .to_string();
        let swap_limit = if env_type == "VIRTUAL-MACHINE" {
            String::new()
        } else {
            let raw_swap = config
                .get("limits.memory.swap")
                .map(String::as_str)
                .unwrap_or("")
                .trim();
            if raw_swap.eq_ignore_ascii_case("false")
                || raw_swap.eq_ignore_ascii_case("off")
                || raw_swap == "0"
            {
                "Off".to_string()
            } else if raw_swap.is_empty()
                || raw_swap.eq_ignore_ascii_case("true")
                || raw_swap.eq_ignore_ascii_case("host")
                || raw_swap.eq_ignore_ascii_case("default")
            {
                String::new()
            } else {
                raw_swap.to_string()
            }
        };

        let limits =
            extract_root_limits(item.devices.get("root"), item.expanded_devices.get("root"));

        let mut preset = "Standard".to_string();
        if item.profiles.iter().any(|p| p.0 == "ai-gui") {
            preset = "Desktop & Mobile (GPU + KVM)".to_string();
        }
        if env_type == "VIRTUAL-MACHINE" {
            preset = "Full KVM Virtual Machine".to_string();
        }

        let mut ipv4 = "\u{2014}".to_string();
        let mut host_iface = map_str(config, "volatile.eth0.host_name").to_string();
        let mut memory_usage = "0 MB".to_string();
        let mut disk_usage = "0 MB".to_string();
        let mut mem_bytes = 0i64;
        let mut mem_total_bytes = parse_size_bytes(&memory_limit);
        let mut swap_bytes = 0i64;
        let mut disk_bytes = 0i64;
        let mut disk_total_bytes = parse_size_bytes(limits.disk_limit);
        let mut cpu_percent = 0i32;
        let mut net_rx_bytes = 0i64;
        let mut net_tx_bytes = 0i64;

        if let Some(st) = item.state.as_ref().filter(|s| !s.is_empty()) {
            mem_bytes = st.memory.usage.0;
            swap_bytes = st.memory.swap_usage.0;
            if mem_bytes > 0 {
                memory_usage = format_bytes(mem_bytes);
            }
            if mem_total_bytes <= 0 && st.memory.total.0 > 0 {
                mem_total_bytes = st.memory.total.0;
            }

            disk_bytes = st.disk.root.usage.0;
            if disk_bytes > 0 {
                disk_usage = format_bytes(disk_bytes);
            }
            if disk_total_bytes <= 0 && st.disk.root.total.0 > 0 {
                disk_total_bytes = st.disk.root.total.0;
            }

            cpu_percent = compute_cpu_percent(
                name,
                running,
                st.cpu.usage.0,
                &st.started_at,
                cpu_core_count,
                &poll_ctx,
                &mut next_cpu_snapshot,
            );

            for (iface_name, iface) in &st.network {
                if iface_name == "lo" {
                    continue;
                }
                if !iface.host_name.is_empty() && host_iface.is_empty() {
                    host_iface = iface.host_name.clone();
                }
                net_rx_bytes += iface.counters.bytes_received.0;
                net_tx_bytes += iface.counters.bytes_sent.0;
                if ipv4 != "\u{2014}" {
                    continue;
                }
                for addr in &iface.addresses {
                    if addr.family == "inet" && addr.scope == "global" {
                        ipv4 = addr.address.clone();
                        break;
                    }
                }
            }
        }

        let (net_rx_rate, net_tx_rate) = compute_net_rates(
            name,
            running,
            net_rx_bytes,
            net_tx_bytes,
            &poll_ctx,
            &mut next_net_snapshot,
        );

        let memory_percent = compute_usage_percent(mem_bytes, mem_total_bytes);
        let disk_percent = compute_usage_percent(disk_bytes, disk_total_bytes);

        // Container swap is host-provided: an explicit limit bounds it, otherwise
        // the host swap total is the only meaningful denominator. Read host swap
        // lazily (and at most once per refresh) so an idle list poll stays cheap.
        let (swap_usage, swap_percent) =
            if swap_bytes > 0 && env_type != "VIRTUAL-MACHINE" && swap_limit != "Off" {
                let swap_total_bytes = if swap_limit.is_empty() {
                    *cached_host_swap_total.get_or_insert_with(|| host_swap_info().total_bytes)
                } else {
                    parse_size_bytes(&swap_limit)
                };
                (
                    format_bytes(swap_bytes),
                    compute_usage_percent(swap_bytes, swap_total_bytes),
                )
            } else {
                (String::new(), 0)
            };

        let mut snapshots = Vec::new();
        let mut snapshot_sizes = BTreeMap::new();
        let mut snapshot_total_bytes = 0i64;
        let mut auto_snap_count = 0i32;
        for snap in &item.snapshots {
            if !snap.name.is_empty() {
                snapshots.push(snap.name.clone());
                if is_auto_snapshot_name(&snap.name) {
                    auto_snap_count += 1;
                }
                if snap.size.0 > 0 {
                    snapshot_sizes.insert(snap.name.clone(), format_bytes(snap.size.0));
                    snapshot_total_bytes += snap.size.0;
                }
            }
        }

        let auto_snap_override = map_str(config, "user.aibox.autosnap").trim().to_lowercase();
        let env_auto_snap = if auto_snap_override.is_empty() {
            defaults.auto_snapshot
        } else {
            auto_snap_override == "true"
        };
        let auto_stop_override = map_str(config, "user.aibox.autostop").trim().to_lowercase();
        let env_auto_stop = if auto_stop_override.is_empty() {
            defaults.auto_stop
        } else {
            auto_stop_override == "true"
        };
        let idle_stop_override = map_str(config, "user.aibox.idlestop").trim().to_lowercase();
        let idle_stop_ignore_ports = map_str(config, "user.aibox.idlestop_ports")
            .trim()
            .to_lowercase()
            == "true";
        let record_override = map_str(config, "user.aibox.record").trim().to_lowercase();
        let env_record_enabled =
            effective_record_sessions(defaults.record_sessions, &record_override);
        let last_used_epoch = map_str(config, "user.aibox.lastused")
            .trim()
            .parse::<i64>()
            .ok()
            .filter(|&v| v > 0)
            .unwrap_or(0);
        let is_template = map_str(config, "user.aibox.template").trim().to_lowercase() == "true";
        let egress_recording = map_str(config, "user.aibox.egresslog")
            .trim()
            .to_lowercase()
            == "true";

        let mut net_mode = map_str(config, "user.aibox.netmode").trim().to_lowercase();
        if net_mode != "online" && net_mode != "offline" && net_mode != "allowlist" {
            if let Some(eth0_dev) = item.devices.get("eth0") {
                let eth0_acls = btree_str(eth0_dev, "security.acls");
                if btree_str(eth0_dev, "type") == "none" {
                    net_mode = "offline".to_string();
                } else if !eth0_acls.is_empty() && eth0_acls != format!("aibox-log-{name}") {
                    net_mode = "allowlist".to_string();
                } else {
                    net_mode = "online".to_string();
                }
            } else {
                net_mode = "online".to_string();
            }
        }
        let net_allowlist = map_str(config, "user.aibox.netallow").to_string();

        let exp_ws = item.expanded_devices.get("workspace");
        let dev_ws = item.devices.get("workspace");

        let mut workspace_path = exp_ws
            .map(|m| btree_str(m, "source"))
            .filter(|s| !s.is_empty())
            .or_else(|| {
                dev_ws
                    .map(|m| btree_str(m, "source"))
                    .filter(|s| !s.is_empty())
            })
            .unwrap_or("")
            .to_string();
        if workspace_path.is_empty() {
            workspace_path = format!("{workspace_base}/{name}");
        }

        let mut workspace_mount_path = exp_ws
            .map(|m| btree_str(m, "path"))
            .filter(|s| !s.is_empty())
            .or_else(|| {
                dev_ws
                    .map(|m| btree_str(m, "path"))
                    .filter(|s| !s.is_empty())
            })
            .unwrap_or("")
            .to_string();
        if workspace_mount_path.is_empty() {
            workspace_mount_path = CONTAINER_WORKSPACE_PATH.to_string();
        }

        let ws_snap_override = map_str(config, "user.aibox.wssnap").trim().to_lowercase();
        let ws_exists = Path::new(&workspace_path).exists();
        let ws_on_btrfs = if ws_exists {
            is_btrfs_filesystem(&workspace_path)
        } else {
            let parent_key = Path::new(&workspace_path)
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            if let Some(&cached) = btrfs_parent_cache.get(&parent_key) {
                cached
            } else {
                let res = is_btrfs_filesystem(&workspace_path);
                btrfs_parent_cache.insert(parent_key, res);
                res
            }
        };
        let ws_is_subvolume = ws_exists
            && ws_on_btrfs
            && fs::symlink_metadata(&workspace_path)
                .is_ok_and(|meta| meta.file_type().is_dir() && meta.ino() == 256);
        let ws_snap_enabled = ws_is_subvolume && ws_snap_override != "false";
        let ws_snap_dir = workspace_snapshot_dir(&workspace_path, name);
        let ws_snapshots = list_workspace_snapshots(&workspace_path, name);
        let orphan_ws_snapshots = find_orphan_workspace_snapshots(&snapshots, &ws_snapshots);
        let has_pre_subvol_backup = Path::new(&format!("{workspace_path}.pre-subvol")).exists();
        let secret_names = parse_env_secret_names(map_str(config, "user.aibox.secrets"));

        let (port_forwards, extra_mounts) = extract_port_forwards_and_mounts(&item.devices);

        let cpu_limit_display = if cpu_limit_raw == "All" {
            "All Cores".to_string()
        } else {
            format!("{cpu_limit_raw} Cores")
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
            swap_usage,
            swap_percent,
            disk_limit: limits.disk_limit.to_string(),
            disk_quota_bytes: limits.disk_quota_bytes,
            disk_usage,
            disk_usage_bytes: disk_bytes,
            disk_percent,
            process_limit,
            disk_read_limit: limits.disk_read_limit.to_string(),
            disk_write_limit: limits.disk_write_limit.to_string(),
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
            swap_limit,
            root_pool: limits.root_pool.to_string(),
        });
    }

    if track_cpu_and_net {
        state.cpu_snapshot_ns = next_cpu_snapshot;
        state.net_counters = next_net_snapshot;
    }

    Ok(result)
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
