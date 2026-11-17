//! Kernel egress log (`journalctl -k`) parsing and aggregation.

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;

/// Aggregated egress connection row matching the QML `egressLogs` model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EgressLogRow {
    pub dst_ip: String,
    pub proto: String,
    pub port: String,
    pub count: i32,
    pub last_seen: String,
}

fn iso_ts_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(\d{4}-\d{2}-\d{2}T(\d{2}:\d{2}:\d{2}))\S*").expect("valid iso_ts_re")
    })
}

fn sys_ts_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^[A-Z][a-z]{2}\s+\d+\s+(\d{2}:\d{2}:\d{2})").expect("valid sys_ts_re")
    })
}

fn in_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\bIN=(\S+)").expect("valid in_re"))
}

fn src_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\bSRC=(\S+)").expect("valid src_re"))
}

fn dst_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\bDST=(\S+)").expect("valid dst_re"))
}

fn proto_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\bPROTO=(\S+)").expect("valid proto_re"))
}

fn dpt_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\bDPT=(\d+)").expect("valid dpt_re"))
}

fn capture_first(re: &Regex, line: &str, group: usize) -> String {
    re.captures(line)
        .and_then(|caps| caps.get(group))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default()
}

/// Parses `journalctl -k` output for `eth0-egress` lines matching `container_ip` or `host_iface`.
pub fn parse_kernel_egress_logs(
    journal_output: &str,
    container_ip: &str,
    host_iface: &str,
    max_entries: i32,
) -> Vec<EgressLogRow> {
    let trimmed_ip = container_ip.trim();
    let clean_ip = if trimmed_ip == "—" { "" } else { trimmed_ip };
    let clean_iface = host_iface.trim();
    if clean_ip.is_empty() && clean_iface.is_empty() {
        return Vec::new();
    }

    struct AggEntry {
        dst_ip: String,
        proto: String,
        port: String,
        last_seen: String,
        count: i32,
        seq: i64,
    }

    let mut agg: HashMap<String, AggEntry> = HashMap::new();
    let mut seq_counter: i64 = 0;

    for line in journal_output.split('\n') {
        if line.is_empty() || !line.contains("eth0-egress") {
            continue;
        }

        let in_if = capture_first(in_re(), line, 1);
        let src_ip = capture_first(src_re(), line, 1);
        let match_iface = !clean_iface.is_empty() && !in_if.is_empty() && in_if == clean_iface;
        let match_ip = !clean_ip.is_empty() && !src_ip.is_empty() && src_ip == clean_ip;
        if !match_iface && !match_ip {
            continue;
        }

        let dst_ip = capture_first(dst_re(), line, 1);
        if dst_ip.is_empty() {
            continue;
        }
        let proto = capture_first(proto_re(), line, 1);
        let port = capture_first(dpt_re(), line, 1);

        let seen = {
            let iso = capture_first(iso_ts_re(), line, 2);
            if !iso.is_empty() {
                iso
            } else {
                capture_first(sys_ts_re(), line, 1)
            }
        };

        let key = format!("{dst_ip}|{proto}|{port}");
        seq_counter += 1;
        if let Some(entry) = agg.get_mut(&key) {
            entry.count += 1;
            if !seen.is_empty() {
                entry.last_seen = seen;
            }
            entry.seq = seq_counter;
        } else {
            agg.insert(
                key,
                AggEntry {
                    dst_ip,
                    proto: if proto.is_empty() {
                        "IP".to_string()
                    } else {
                        proto
                    },
                    port,
                    last_seen: seen,
                    count: 1,
                    seq: seq_counter,
                },
            );
        }
    }

    let mut list: Vec<AggEntry> = agg.into_values().collect();
    list.sort_by_key(|a| std::cmp::Reverse(a.seq));

    let limit = max_entries.max(1) as usize;
    list.into_iter()
        .take(limit)
        .map(|e| EgressLogRow {
            dst_ip: e.dst_ip,
            proto: e.proto,
            port: if e.port.is_empty() {
                "—".to_string()
            } else {
                e.port
            },
            count: e.count,
            last_seen: if e.last_seen.is_empty() {
                "—".to_string()
            } else {
                e.last_seen
            },
        })
        .collect()
}

/// Serializes `parse_kernel_egress_logs` output to a compact JSON array for FFI.
pub fn parse_kernel_egress_logs_json(
    journal_output: &str,
    container_ip: &str,
    host_iface: &str,
    max_entries: i32,
) -> String {
    let rows = parse_kernel_egress_logs(journal_output, container_ip, host_iface, max_entries);
    serde_json::to_string(&rows).unwrap_or_else(|_| "[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn egress_journal_parsing_matches_qt_test() {
        let journal_sample = "\
2026-11-13T13:10:01+0700 host kernel: eth0-egress-0 IN=veth-box1 OUT=incusbr0 SRC=10.143.44.18 DST=140.82.121.4 LEN=60 PROTO=TCP SPT=50001 DPT=443 \n\
2026-11-13T13:10:03+0700 host kernel: eth0-egress-0 IN=veth-other OUT=incusbr0 SRC=10.143.44.99 DST=9.9.9.9 LEN=60 PROTO=UDP SPT=50002 DPT=53 \n\
2026-11-13T13:10:05+0700 host kernel: eth0-egress-default IN=veth-box1 OUT=incusbr0 SRC=10.143.44.18 DST=104.16.25.34 LEN=60 PROTO=TCP SPT=50003 DPT=443 \n\
2026-11-13T13:10:09+0700 host kernel: eth0-egress-0 IN=veth-old OUT=incusbr0 SRC=10.143.44.18 DST=140.82.121.4 LEN=60 PROTO=TCP SPT=50004 DPT=443 \n";

        let logs = parse_kernel_egress_logs(journal_sample, "10.143.44.18", "veth-box1", 100);
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].dst_ip, "140.82.121.4");
        assert_eq!(logs[0].count, 2);
        assert_eq!(logs[0].last_seen, "13:10:09");
        assert_eq!(logs[1].dst_ip, "104.16.25.34");
        assert_eq!(logs[1].count, 1);
    }
}
