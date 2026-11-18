//! Secret value redaction for logs and byte streams using Aho-Corasick.

use std::cmp::Reverse;
use std::collections::VecDeque;

const REDACTED_MARKER: &[u8] = b"[REDACTED]";
const REDACTED_STR: &str = "[REDACTED]";

#[derive(Default)]
struct AcNode {
    children: Vec<(u8, usize)>,
    fail: usize,
    best_len: usize,
}

impl AcNode {
    fn child(&self, byte: u8) -> Option<usize> {
        self.children
            .iter()
            .find_map(|&(b, idx)| if b == byte { Some(idx) } else { None })
    }
}

/// Single-pass O(n) multi-pattern secret redactor using Aho-Corasick.
///
/// Resolves overlapping matches leftmost-longest so shared prefixes never leave
/// trailing fragments behind.
pub struct SecretRedactor {
    nodes: Vec<AcNode>,
    max_len: usize,
}

impl SecretRedactor {
    /// Builds an automaton over all non-empty unique values in `values`.
    pub fn new(values: &[String]) -> Self {
        let mut unique: Vec<&[u8]> = Vec::with_capacity(values.len());
        let mut max_len = 0usize;
        for val in values {
            if !val.is_empty() {
                let bytes = val.as_bytes();
                if !unique.contains(&bytes) {
                    max_len = max_len.max(bytes.len());
                    unique.push(bytes);
                }
            }
        }

        if unique.is_empty() {
            return Self {
                nodes: Vec::new(),
                max_len: 0,
            };
        }

        let mut nodes = vec![AcNode::default()];
        for pat in unique {
            let mut cur = 0usize;
            for &b in pat {
                if let Some(next) = nodes[cur].child(b) {
                    cur = next;
                } else {
                    let next = nodes.len();
                    nodes.push(AcNode::default());
                    nodes[cur].children.push((b, next));
                    cur = next;
                }
            }
            nodes[cur].best_len = nodes[cur].best_len.max(pat.len());
        }

        let mut queue = VecDeque::new();
        let root_children: Vec<(u8, usize)> = nodes[0].children.clone();
        for (_, child) in root_children {
            nodes[child].fail = 0;
            queue.push_back(child);
        }

        while let Some(u) = queue.pop_front() {
            let children: Vec<(u8, usize)> = nodes[u].children.clone();
            for (b, v) in children {
                let mut f = nodes[u].fail;
                while f != 0 && nodes[f].child(b).is_none() {
                    f = nodes[f].fail;
                }
                let fail_target = nodes[f].child(b).unwrap_or(0);
                nodes[v].fail = fail_target;
                nodes[v].best_len = nodes[v].best_len.max(nodes[fail_target].best_len);
                queue.push_back(v);
            }
        }

        Self { nodes, max_len }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Finds non-overlapping leftmost-longest match intervals `(start, end)` in `haystack`.
    fn find_redaction_ranges(&self, haystack: &[u8]) -> Vec<(usize, usize)> {
        if self.nodes.is_empty() || haystack.is_empty() {
            return Vec::new();
        }

        let mut raw_matches: Vec<(usize, usize)> = Vec::new();
        let mut state = 0usize;
        for (i, &b) in haystack.iter().enumerate() {
            while state != 0 && self.nodes[state].child(b).is_none() {
                state = self.nodes[state].fail;
            }
            state = self.nodes[state].child(b).unwrap_or(0);
            let len = self.nodes[state].best_len;
            if len > 0 {
                raw_matches.push((i + 1 - len, i + 1));
            }
        }

        if raw_matches.is_empty() {
            return Vec::new();
        }

        raw_matches.sort_unstable_by_key(|&(start, end)| (start, Reverse(end)));
        let mut selected = Vec::with_capacity(raw_matches.len());
        let mut last_end = 0usize;
        for (start, end) in raw_matches {
            if start >= last_end {
                selected.push((start, end));
                last_end = end;
            }
        }
        selected
    }

    /// Redacts all matches in `text` in a single pass.
    pub fn redact_str(&self, text: &str) -> String {
        if self.is_empty() || text.is_empty() {
            return text.to_string();
        }
        let ranges = self.find_redaction_ranges(text.as_bytes());
        if ranges.is_empty() {
            return text.to_string();
        }

        let mut out = String::with_capacity(text.len());
        let mut cursor = 0usize;
        for (start, end) in ranges {
            out.push_str(&text[cursor..start]);
            out.push_str(REDACTED_STR);
            cursor = end;
        }
        out.push_str(&text[cursor..]);
        out
    }

    /// Redacts all matches in `data` in a single pass.
    pub fn redact_bytes(&self, data: &[u8]) -> Vec<u8> {
        if self.is_empty() || data.is_empty() {
            return data.to_vec();
        }
        let ranges = self.find_redaction_ranges(data);
        if ranges.is_empty() {
            return data.to_vec();
        }

        let mut out = Vec::with_capacity(data.len());
        let mut cursor = 0usize;
        for (start, end) in ranges {
            out.extend_from_slice(&data[cursor..start]);
            out.extend_from_slice(REDACTED_MARKER);
            cursor = end;
        }
        out.extend_from_slice(&data[cursor..]);
        out
    }
}

/// Chunked redactor that holds back at most `max_secret_len - 1` trailing bytes
/// so secrets split across arbitrary buffer boundaries are still redacted.
pub struct StreamingRedactor {
    redactor: SecretRedactor,
    buf: Vec<u8>,
    modified: bool,
}

impl StreamingRedactor {
    pub fn new(values: &[String]) -> Self {
        Self {
            redactor: SecretRedactor::new(values),
            buf: Vec::new(),
            modified: false,
        }
    }

    /// Feeds a chunk and returns the safely redacted prefix bytes.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<u8> {
        if self.redactor.is_empty() {
            return chunk.to_vec();
        }
        self.buf.extend_from_slice(chunk);
        if self.redactor.max_len <= 1 {
            let out = self.redactor.redact_bytes(&self.buf);
            if out != self.buf {
                self.modified = true;
            }
            self.buf.clear();
            return out;
        }

        let keep_tail = self.redactor.max_len - 1;
        let safe_end = self.buf.len().saturating_sub(keep_tail);
        if safe_end == 0 {
            return Vec::new();
        }

        // Any match starting at `start < safe_end` has at least `max_len` bytes
        // ahead of it in `self.buf`, so no longer secret can start at `start`.
        let ranges = self.redactor.find_redaction_ranges(&self.buf);
        let mut emit_until = safe_end;
        let mut out = Vec::with_capacity(safe_end);
        let mut cursor = 0usize;

        for (start, end) in ranges {
            if start >= safe_end {
                break;
            }
            if &self.buf[start..end] != REDACTED_MARKER {
                self.modified = true;
            }
            out.extend_from_slice(&self.buf[cursor..start]);
            out.extend_from_slice(REDACTED_MARKER);
            cursor = end;
            emit_until = emit_until.max(end);
        }

        if cursor < emit_until {
            out.extend_from_slice(&self.buf[cursor..emit_until]);
        }
        self.buf.drain(..emit_until);
        out
    }

    /// Flushes and redacts any remaining buffered bytes at EOF.
    pub fn finish(self) -> Vec<u8> {
        self.finish_with_status().0
    }

    /// Flushes and redacts any remaining buffered bytes at EOF, returning `(bytes, modified)`.
    pub fn finish_with_status(mut self) -> (Vec<u8>, bool) {
        if self.buf.is_empty() {
            return (Vec::new(), self.modified);
        }
        let out = self.redactor.redact_bytes(&self.buf);
        if out != self.buf {
            self.modified = true;
        }
        self.buf.clear();
        (out, self.modified)
    }
}

/// Replaces every non-empty value in `values` with `[REDACTED]`, resolving
/// overlaps leftmost-longest so shared prefixes never leave trailing fragments behind.
pub fn redact_secrets(text: &str, values: &[String]) -> String {
    if text.is_empty() || values.is_empty() {
        return text.to_string();
    }
    SecretRedactor::new(values).redact_str(text)
}

/// UTF-8 byte-stream wrapper around `redact_secrets`.
pub fn redact_secrets_bytes(data: &[u8], values: &[String]) -> Vec<u8> {
    if data.is_empty() || values.is_empty() {
        return data.to_vec();
    }
    let text = String::from_utf8_lossy(data);
    redact_secrets(&text, values).into_bytes()
}
