//! Kernel egress log (`journalctl -k`) parsing, incremental tailing, and aggregation.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::net::IpAddr;
use std::path::Path;

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

#[derive(Debug, Clone)]
struct AggEntry {
    dst_ip: String,
    proto: String,
    port: String,
    last_seen: String,
    count: i32,
    seq: i64,
}

/// Per-environment incremental egress log state tracking byte offset and aggregated rows.
#[derive(Debug, Default, Clone)]
pub struct EgressEnvState {
    last_offset: u64,
    container_ip: String,
    host_iface: String,
    agg: HashMap<String, AggEntry>,
    seq_counter: i64,
    key_buf: String,
}

impl EgressEnvState {
    /// Returns the last processed byte offset for this environment.
    pub fn last_offset(&self) -> u64 {
        self.last_offset
    }

    /// Clears all cached aggregation state and resets the byte offset to zero.
    pub fn reset(&mut self) {
        self.last_offset = 0;
        self.agg.clear();
        self.seq_counter = 0;
    }

    /// Incrementally reads new log lines from `reader` starting at `last_offset`.
    /// Resets offset and state if `reader` shrank (log rotation) or filters changed.
    pub fn parse_reader<R: Read + Seek>(
        &mut self,
        reader: &mut R,
        container_ip: &str,
        host_iface: &str,
        max_entries: i32,
    ) -> io::Result<Vec<EgressLogRow>> {
        let (clean_ip, clean_iface) = normalize_filters(container_ip, host_iface);
        if clean_ip.is_empty() && clean_iface.is_empty() {
            self.reset();
            self.container_ip.clear();
            self.host_iface.clear();
            return Ok(Vec::new());
        }

        if clean_ip != self.container_ip || clean_iface != self.host_iface {
            self.reset();
            self.container_ip = clean_ip.to_string();
            self.host_iface = clean_iface.to_string();
        }

        let total_len = reader.seek(SeekFrom::End(0))?;
        if total_len < self.last_offset {
            // Log rotated or truncated: restart from byte 0.
            self.reset();
        }

        if total_len > self.last_offset {
            reader.seek(SeekFrom::Start(self.last_offset))?;
            let mut buf_reader = BufReader::new(reader);
            let mut byte_buf = Vec::new();
            let mut bytes_read = 0u64;
            loop {
                byte_buf.clear();
                let n = buf_reader.read_until(b'\n', &mut byte_buf)?;
                if n == 0 || !byte_buf.ends_with(b"\n") {
                    break;
                }
                bytes_read += n as u64;
                let line = String::from_utf8_lossy(&byte_buf);
                let trimmed = line.trim_end_matches(['\r', '\n']);
                self.ingest_line(trimmed, clean_ip, clean_iface, max_entries);
            }
            self.last_offset += bytes_read;
        }

        Ok(self.snapshot_rows(max_entries))
    }

    fn ingest_line(&mut self, line: &str, clean_ip: &str, clean_iface: &str, max_entries: i32) {
        if line.is_empty() || !line.contains("eth0-egress") {
            return;
        }

        let mut in_if = "";
        let mut src_ip = "";
        let mut dst_ip = "";
        let mut proto = "";
        let mut port = "";

        for token in line.split_ascii_whitespace() {
            if let Some(v) = token.strip_prefix("IN=") {
                in_if = v;
            } else if let Some(v) = token.strip_prefix("SRC=") {
                src_ip = v;
            } else if let Some(v) = token.strip_prefix("DST=") {
                dst_ip = v;
            } else if let Some(v) = token.strip_prefix("PROTO=") {
                proto = v;
            } else if let Some(v) = token.strip_prefix("DPT=") {
                if !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()) {
                    port = v;
                }
            }
        }

        let match_iface = !clean_iface.is_empty() && !in_if.is_empty() && in_if == clean_iface;
        let match_ip =
            !clean_ip.is_empty() && !src_ip.is_empty() && ip_str_matches(src_ip, clean_ip);
        if !match_iface && !match_ip {
            return;
        }

        if dst_ip.is_empty() {
            return;
        }

        let seen = extract_timestamp(line);
        let norm_proto = if proto.is_empty() { "IP" } else { proto };
        let canon_dst = canonical_ip_or_raw(dst_ip);

        self.key_buf.clear();
        self.key_buf.push_str(&canon_dst);
        self.key_buf.push('|');
        self.key_buf.push_str(norm_proto);
        self.key_buf.push('|');
        self.key_buf.push_str(port);

        self.seq_counter += 1;
        if let Some(entry) = self.agg.get_mut(&self.key_buf) {
            entry.count = entry.count.saturating_add(1);
            if !seen.is_empty() {
                entry.last_seen.clear();
                entry.last_seen.push_str(seen);
            }
            entry.seq = self.seq_counter;
        } else {
            let cap = (max_entries.max(1) as usize).saturating_mul(8).max(4096);
            if self.agg.len() >= cap {
                let mut seqs: Vec<i64> = self.agg.values().map(|e| e.seq).collect();
                let mid = seqs.len() / 2;
                let (_, &mut cutoff, _) = seqs.select_nth_unstable(mid);
                self.agg.retain(|_, e| e.seq > cutoff);
            }
            self.agg.insert(
                self.key_buf.clone(),
                AggEntry {
                    dst_ip: canon_dst,
                    proto: norm_proto.to_string(),
                    port: port.to_string(),
                    last_seen: seen.to_string(),
                    count: 1,
                    seq: self.seq_counter,
                },
            );
        }
    }

    fn snapshot_rows(&self, max_entries: i32) -> Vec<EgressLogRow> {
        let mut list: Vec<&AggEntry> = self.agg.values().collect();
        let limit = max_entries.max(1) as usize;
        if list.len() > limit {
            let (top, _, _) =
                list.select_nth_unstable_by_key(limit - 1, |a| std::cmp::Reverse(a.seq));
            top.sort_by_key(|a| std::cmp::Reverse(a.seq));
            list.truncate(limit);
        } else {
            list.sort_by_key(|a| std::cmp::Reverse(a.seq));
        }

        list.into_iter()
            .map(|e| EgressLogRow {
                dst_ip: e.dst_ip.clone(),
                proto: e.proto.clone(),
                port: if e.port.is_empty() {
                    "—".to_string()
                } else {
                    e.port.clone()
                },
                count: e.count,
                last_seen: if e.last_seen.is_empty() {
                    "—".to_string()
                } else {
                    e.last_seen.clone()
                },
            })
            .collect()
    }
}

/// Multi-environment incremental egress log cache keyed by environment name.
#[derive(Debug, Default)]
pub struct EgressLogCache {
    envs: HashMap<String, EgressEnvState>,
}

impl EgressLogCache {
    /// Incrementally parses new egress log bytes from `reader` for `env_name`.
    pub fn refresh_from_reader<R: Read + Seek>(
        &mut self,
        env_name: &str,
        reader: &mut R,
        container_ip: &str,
        host_iface: &str,
        max_entries: i32,
    ) -> io::Result<Vec<EgressLogRow>> {
        let state = self.envs.entry(env_name.to_string()).or_default();
        state.parse_reader(reader, container_ip, host_iface, max_entries)
    }

    /// Incrementally parses new egress log bytes from `path` for `env_name`.
    pub fn refresh_from_file(
        &mut self,
        env_name: &str,
        path: &Path,
        container_ip: &str,
        host_iface: &str,
        max_entries: i32,
    ) -> io::Result<Vec<EgressLogRow>> {
        let mut file = File::open(path)?;
        self.refresh_from_reader(env_name, &mut file, container_ip, host_iface, max_entries)
    }

    /// Returns the last processed byte offset for `env_name`.
    pub fn offset_for(&self, env_name: &str) -> u64 {
        self.envs
            .get(env_name)
            .map_or(0, EgressEnvState::last_offset)
    }

    /// Removes cached egress log state for `env_name`.
    pub fn clear_env(&mut self, env_name: &str) {
        self.envs.remove(env_name);
    }
}

fn normalize_filters<'a>(container_ip: &'a str, host_iface: &'a str) -> (&'a str, &'a str) {
    let trimmed_ip = container_ip.trim();
    let clean_ip = if trimmed_ip == "—" { "" } else { trimmed_ip };
    let clean_iface = host_iface.trim();
    (clean_ip, clean_iface)
}

fn ip_str_matches(raw: &str, filter: &str) -> bool {
    if raw == filter {
        return true;
    }
    match (raw.parse::<IpAddr>(), filter.parse::<IpAddr>()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn canonical_ip_or_raw(raw: &str) -> String {
    match raw.parse::<IpAddr>() {
        Ok(ip) => ip.to_string(),
        Err(_) => raw.to_string(),
    }
}

fn is_hh_mm_ss_bytes(b: &[u8]) -> bool {
    b.len() == 8
        && b[0].is_ascii_digit()
        && b[1].is_ascii_digit()
        && b[2] == b':'
        && b[3].is_ascii_digit()
        && b[4].is_ascii_digit()
        && b[5] == b':'
        && b[6].is_ascii_digit()
        && b[7].is_ascii_digit()
}

fn is_hh_mm_ss(s: &str) -> bool {
    is_hh_mm_ss_bytes(s.as_bytes())
}

fn extract_timestamp(line: &str) -> &str {
    // ISO-8601: YYYY-MM-DDTHH:MM:SS...
    let b = line.as_bytes();
    if b.len() >= 19
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[8..10].iter().all(u8::is_ascii_digit)
        && is_hh_mm_ss_bytes(&b[11..19])
    {
        return &line[11..19];
    }

    // Syslog: Mmm DD HH:MM:SS ...
    let mut parts = line.split_ascii_whitespace();
    if let (Some(mon), Some(day), Some(time_part)) = (parts.next(), parts.next(), parts.next()) {
        let mb = mon.as_bytes();
        let valid_mon = mb.len() == 3
            && mb[0].is_ascii_uppercase()
            && mb[1].is_ascii_lowercase()
            && mb[2].is_ascii_lowercase();
        let valid_day = !day.is_empty() && day.bytes().all(|c| c.is_ascii_digit());
        if valid_mon && valid_day && is_hh_mm_ss(time_part) {
            return time_part;
        }
    }

    ""
}

/// Parses `journalctl -k` output for `eth0-egress` lines matching `container_ip` or `host_iface`.
pub fn parse_kernel_egress_logs(
    journal_output: &str,
    container_ip: &str,
    host_iface: &str,
    max_entries: i32,
) -> Vec<EgressLogRow> {
    let (clean_ip, clean_iface) = normalize_filters(container_ip, host_iface);
    if clean_ip.is_empty() && clean_iface.is_empty() {
        return Vec::new();
    }

    let mut state = EgressEnvState::default();
    for line in journal_output.split('\n') {
        state.ingest_line(line, clean_ip, clean_iface, max_entries);
    }
    state.snapshot_rows(max_entries)
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
    use std::io::Write;
    use std::time::Instant;

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

    #[test]
    fn egress_parser_supports_ipv6_addresses() {
        let journal_sample = "\
2026-11-13T13:10:01+0700 host kernel: eth0-egress-0 IN=veth-box1 OUT=incusbr0 SRC=2001:0db8::10 DST=2606:4700:4700::1111 LEN=80 PROTO=TCP SPT=50001 DPT=443 \n\
2026-11-13T13:10:02+0700 host kernel: eth0-egress-0 IN=veth-box1 OUT=incusbr0 SRC=2001:db8::10 DST=2606:4700:4700::1111 LEN=80 PROTO=TCP SPT=50002 DPT=443 \n";

        let logs = parse_kernel_egress_logs(journal_sample, "2001:db8::10", "", 100);
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].dst_ip, "2606:4700:4700::1111");
        assert_eq!(logs[0].proto, "TCP");
        assert_eq!(logs[0].port, "443");
        assert_eq!(logs[0].count, 2);
        assert_eq!(logs[0].last_seen, "13:10:02");
    }

    fn temp_test_file(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("aibox-egress-test-{}-{}", std::process::id(), tag));
        std::fs::create_dir_all(&dir).expect("create temp test dir");
        dir.join("egress.log")
    }

    #[test]
    fn incremental_egress_handles_append_and_log_rotation() {
        let log_path = temp_test_file("append-rotate");
        std::fs::write(
            &log_path,
            "2026-11-13T13:10:01+0700 host kernel: eth0-egress-0 IN=veth-1 OUT=incusbr0 SRC=10.0.0.2 DST=1.1.1.1 PROTO=TCP DPT=443\n",
        )
        .expect("write initial log");

        let mut cache = EgressLogCache::default();
        let first = cache
            .refresh_from_file("box1", &log_path, "10.0.0.2", "veth-1", 100)
            .expect("first refresh");
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].count, 1);
        let offset_after_first = cache.offset_for("box1");
        assert!(offset_after_first > 0);

        // Append a new line; only new bytes should be read and aggregated.
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&log_path)
            .expect("open append");
        writeln!(
            file,
            "2026-11-13T13:10:05+0700 host kernel: eth0-egress-0 IN=veth-1 OUT=incusbr0 SRC=10.0.0.2 DST=1.1.1.1 PROTO=TCP DPT=443"
        )
        .expect("append line");
        drop(file);

        let second = cache
            .refresh_from_file("box1", &log_path, "10.0.0.2", "veth-1", 100)
            .expect("second refresh");
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].count, 2);
        assert_eq!(second[0].last_seen, "13:10:05");
        assert!(cache.offset_for("box1") > offset_after_first);

        // Simulate log rotation (smaller file): offset resets and old state clears.
        std::fs::write(
            &log_path,
            "2026-11-13T13:11:00+0700 host kernel: eth0-egress-0 IN=veth-1 OUT=incusbr0 SRC=10.0.0.2 DST=8.8.8.8 PROTO=UDP DPT=53\n",
        )
        .expect("write rotated log");

        let rotated = cache
            .refresh_from_file("box1", &log_path, "10.0.0.2", "veth-1", 100)
            .expect("rotated refresh");
        assert_eq!(rotated.len(), 1);
        assert_eq!(rotated[0].dst_ip, "8.8.8.8");
        assert_eq!(rotated[0].count, 1);
        if let Some(parent) = log_path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }

    #[test]
    fn incremental_egress_100k_lines_second_refresh_under_5ms() {
        let log_path = temp_test_file("100k");
        {
            let file = File::create(&log_path).expect("create 100k fixture");
            let mut writer = io::BufWriter::new(file);
            for i in 0..100_000 {
                let dst_octet = (i % 20) + 1;
                let dpt = if (i / 20) % 2 == 0 { 443 } else { 80 };
                writeln!(
                    writer,
                    "2026-11-13T13:10:01+0700 host kernel: eth0-egress-0 IN=veth-box1 OUT=incusbr0 SRC=10.143.44.18 DST=140.82.121.{dst_octet} LEN=60 PROTO=TCP SPT=50001 DPT={dpt}"
                )
                .expect("write line");
            }
            writer.flush().expect("flush");
        }

        let mut cache = EgressLogCache::default();
        let initial = cache
            .refresh_from_file("box1", &log_path, "10.143.44.18", "veth-box1", 100)
            .expect("initial 100k refresh");
        assert_eq!(initial.len(), 40);
        let total_count: i32 = initial.iter().map(|r| r.count).sum();
        assert_eq!(total_count, 100_000);

        let started = Instant::now();
        let second = cache
            .refresh_from_file("box1", &log_path, "10.143.44.18", "veth-box1", 100)
            .expect("second refresh");
        let elapsed = started.elapsed();

        assert_eq!(second, initial);
        assert!(
            elapsed < std::time::Duration::from_millis(5),
            "second refresh without new data took {elapsed:?}, expected < 5ms"
        );
        if let Some(parent) = log_path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }
}
