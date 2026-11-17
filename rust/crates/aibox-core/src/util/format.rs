//! Byte-size and transfer-rate formatting matching Qt's `QString::arg(..., 'f', prec)`.

/// Rounds `value` half-away-from-zero to `decimals` places and formats it
/// with fixed precision, matching `QLocale`/`QString::arg(double, 0, 'f', n)`.
fn format_fixed(value: f64, decimals: i32) -> String {
    let factor = 10f64.powi(decimals);
    let rounded = (value * factor).round() / factor;
    format!("{rounded:.prec$}", prec = decimals as usize)
}

/// Formats a byte count using KiB/MiB/GiB units, matching `formatBytes` in C++.
pub fn format_bytes(bytes: i64) -> String {
    if bytes <= 0 {
        return "0 MiB".to_string();
    }
    let mb = bytes as f64 / (1024.0 * 1024.0);
    if mb >= 1024.0 {
        return format!("{} GiB", format_fixed(mb / 1024.0, 1));
    }
    if mb < 1.0 {
        return format!("{} KiB", format_fixed(bytes as f64 / 1024.0, 0));
    }
    format!("{} MiB", format_fixed(mb, 0))
}

/// Formats a byte rate using B/s, KB/s, MB/s, GB/s, matching `formatByteRate` in C++.
pub fn format_byte_rate(bytes_per_second: i64) -> String {
    if bytes_per_second <= 0 {
        return "0 KB/s".to_string();
    }
    let kb = bytes_per_second as f64 / 1024.0;
    if kb < 1.0 {
        return format!("{bytes_per_second} B/s");
    }
    if kb < 1024.0 {
        return format!("{} KB/s", format_fixed(kb, 1));
    }
    let mb = kb / 1024.0;
    if mb < 1024.0 {
        return format!("{} MB/s", format_fixed(mb, 2));
    }
    format!("{} GB/s", format_fixed(mb / 1024.0, 2))
}

/// Formats a cumulative byte total using B, KB, MB, GB, TB, matching `formatByteTotal` in C++.
pub fn format_byte_total(bytes: i64) -> String {
    if bytes <= 0 {
        return "0 B".to_string();
    }
    let kb = bytes as f64 / 1024.0;
    if kb < 1024.0 {
        return format!("{} KB", format_fixed(kb, 1));
    }
    let mb = kb / 1024.0;
    if mb < 1024.0 {
        return format!("{} MB", format_fixed(mb, 2));
    }
    let gb = mb / 1024.0;
    if gb < 1024.0 {
        return format!("{} GB", format_fixed(gb, 2));
    }
    format!("{} TB", format_fixed(gb / 1024.0, 2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_boundaries() {
        assert_eq!(format_bytes(0), "0 MiB");
        assert_eq!(format_bytes(-1), "0 MiB");
        assert_eq!(format_bytes(512), "1 KiB");
        assert_eq!(format_bytes(1024), "1 KiB");
        assert_eq!(format_bytes(1536), "2 KiB");
        assert_eq!(format_bytes(1024 * 1024), "1 MiB");
        assert_eq!(format_bytes(1_127_428_915), "1.0 GiB");
        assert_eq!(format_bytes(1_342_177_280), "1.3 GiB");
    }

    #[test]
    fn format_rate_and_total() {
        assert_eq!(format_byte_rate(0), "0 KB/s");
        assert_eq!(format_byte_rate(512), "512 B/s");
        assert_eq!(format_byte_rate(1536), "1.5 KB/s");
        assert_eq!(format_byte_total(0), "0 B");
        assert_eq!(format_byte_total(512), "0.5 KB");
        assert_eq!(format_byte_total(1536), "1.5 KB");
    }
}
