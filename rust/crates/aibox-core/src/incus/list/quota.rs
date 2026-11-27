use std::collections::BTreeMap;

use crate::util::size::parse_size_bytes;

use super::model::{ExtraMountEntry, PortForwardEntry};

pub(crate) struct RootLimits<'a> {
    pub disk_limit: &'a str,
    pub disk_quota_bytes: i64,
    pub disk_read_limit: &'a str,
    pub disk_write_limit: &'a str,
    pub root_pool: &'a str,
}

pub(crate) fn extract_root_limits<'a>(
    dev_root: Option<&'a BTreeMap<String, String>>,
    exp_root: Option<&'a BTreeMap<String, String>>,
) -> RootLimits<'a> {
    let pick = |key: &str, default: &'a str| -> &'a str {
        let from_dev = dev_root
            .and_then(|m| m.get(key))
            .map(String::as_str)
            .unwrap_or("");
        if !from_dev.is_empty() {
            return from_dev;
        }
        let from_exp = exp_root
            .and_then(|m| m.get(key))
            .map(String::as_str)
            .unwrap_or("");
        if !from_exp.is_empty() {
            return from_exp;
        }
        default
    };

    let disk_limit = pick("size", "Shared");
    let disk_quota_bytes = if disk_limit.eq_ignore_ascii_case("Shared") {
        0
    } else {
        parse_size_bytes(disk_limit)
    };
    let root_dev = exp_root.filter(|m| !m.is_empty()).or(dev_root);
    let root_pool = root_dev
        .and_then(|r| r.get("pool"))
        .map(String::as_str)
        .unwrap_or("");

    RootLimits {
        disk_limit,
        disk_quota_bytes,
        disk_read_limit: pick("limits.read", "Unlimited"),
        disk_write_limit: pick("limits.write", "Unlimited"),
        root_pool,
    }
}

pub(crate) fn extract_port_forwards_and_mounts(
    devices: &BTreeMap<String, BTreeMap<String, String>>,
) -> (Vec<PortForwardEntry>, Vec<ExtraMountEntry>) {
    let mut port_forwards = Vec::new();
    let mut extra_mounts = Vec::new();

    for (dev_name, dev_map) in devices {
        let dev_type = dev_map.get("type").map(String::as_str).unwrap_or("");
        if dev_type == "proxy" {
            let listen_addr = dev_map.get("listen").map(String::as_str).unwrap_or("");
            let connect_addr = dev_map.get("connect").map(String::as_str).unwrap_or("");
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
            let src = dev_map.get("source").cloned().unwrap_or_default();
            let mnt_path = dev_map.get("path").cloned().unwrap_or_default();
            let ro_str = dev_map
                .get("readonly")
                .map(|s| s.trim().to_lowercase())
                .unwrap_or_default();
            let is_ro = ro_str == "true" || ro_str == "1";
            extra_mounts.push(ExtraMountEntry {
                device_name: dev_name.clone(),
                source: src,
                path: mnt_path,
                readonly: is_ro,
            });
        }
    }

    (port_forwards, extra_mounts)
}
