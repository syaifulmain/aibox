//! Host swap discovery for container swap limits.
//!
//! Incus containers do not own a swap device; their `limits.memory.swap`
//! allowance is backed by the swap active on the host. This module only reads
//! host state. It never creates or changes swap.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HostSwapInfo {
    pub total_bytes: i64,
    pub entries: usize,
}

impl HostSwapInfo {
    pub fn available(&self) -> bool {
        self.total_bytes > 0 && self.entries > 0
    }
}

/// Parses the contents of `/proc/swaps`. The size column is in KiB.
pub fn parse_proc_swaps(contents: &str) -> HostSwapInfo {
    let mut total_bytes = 0i64;
    let mut entries = 0usize;
    for line in contents.lines().skip(1) {
        let mut fields = line.split_whitespace();
        let Some(_filename) = fields.next() else {
            continue;
        };
        let Some(_swap_type) = fields.next() else {
            continue;
        };
        let Some(size_kib) = fields.next().and_then(|value| value.parse::<i64>().ok()) else {
            continue;
        };
        if size_kib <= 0 {
            continue;
        }
        entries += 1;
        total_bytes = total_bytes.saturating_add(size_kib.saturating_mul(1024));
    }
    HostSwapInfo {
        total_bytes,
        entries,
    }
}

/// Reads the host swap state from `/proc/swaps`. Missing or unreadable files
/// are treated as "no swap" rather than an error.
pub fn host_swap_info() -> HostSwapInfo {
    std::fs::read_to_string("/proc/swaps")
        .map(|contents| parse_proc_swaps(&contents))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_swap_partition_and_swapfile_sizes() {
        let info = parse_proc_swaps(
            "Filename\t\t\t\tType\t\tSize\t\tUsed\t\tPriority\n\
             /dev/nvme0n1p3                          partition\t8388604\t\t120\t\t-2\n\
             /swapfile                               file\t\t2097148\t\t0\t\t-3\n",
        );
        assert_eq!(info.entries, 2);
        assert_eq!(
            info.total_bytes,
            (8_388_604i64 + 2_097_148i64).saturating_mul(1024)
        );
        assert!(info.available());
    }

    #[test]
    fn empty_or_header_only_means_no_host_swap() {
        assert_eq!(parse_proc_swaps(""), HostSwapInfo::default());
        assert_eq!(
            parse_proc_swaps("Filename\t\t\t\tType\t\tSize\t\tUsed\t\tPriority\n"),
            HostSwapInfo::default()
        );
    }
}
