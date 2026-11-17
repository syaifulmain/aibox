//! Byte-level log truncation helpers.

/// Keeps at most `max_bytes` from the tail of `data`, aligning to the next
/// newline when possible and prepending `... [truncated] ...\n`.
pub fn truncate_log_tail(data: &[u8], max_bytes: i64) -> Vec<u8> {
    if max_bytes <= 0 || data.len() <= max_bytes as usize {
        return data.to_vec();
    }
    let keep = max_bytes as usize;
    let mut tail = &data[data.len() - keep..];
    if let Some(newline_idx) = tail.iter().position(|&b| b == b'\n') {
        if newline_idx > 0 && newline_idx < tail.len() - 1 {
            tail = &tail[newline_idx + 1..];
        }
    }
    const PREFIX: &[u8] = b"... [truncated] ...\n";
    let mut out = Vec::with_capacity(PREFIX.len() + tail.len());
    out.extend_from_slice(PREFIX);
    out.extend_from_slice(tail);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_tail_on_byte_boundary() {
        let mut big = Vec::new();
        for i in 0..30 {
            big.extend_from_slice(format!("line-{i}\n").as_bytes());
        }
        let truncated = truncate_log_tail(&big, 80);
        assert!(truncated.starts_with(b"... [truncated] ...\n"));
        assert!(truncated.ends_with(b"line-29\n"));
        assert_eq!(truncate_log_tail(b"abc", 0), b"abc");
    }
}
