use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::util::format::format_bytes;
use crate::util::paths::clean_path;

pub const DEFAULT_MAX_DIFF_ENTRIES: i32 = 5000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffEntry {
    pub path: String,
    pub status: String,
    #[serde(rename = "type")]
    pub entry_type: String,
    pub size_bytes: i64,
    pub size_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedRsyncItemize {
    pub entries: Vec<DiffEntry>,
    pub truncated: bool,
    pub added_count: i32,
    pub modified_count: i32,
    pub deleted_count: i32,
    pub total_count: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffSummary {
    pub snapshot_name: String,
    pub added_count: i32,
    pub modified_count: i32,
    pub deleted_count: i32,
    pub total_count: i32,
    pub truncated: bool,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDiffResult {
    pub summary: DiffSummary,
    pub entries: Vec<DiffEntry>,
}

pub fn build_rsync_exclude_args(excludes: &[String]) -> Vec<String> {
    excludes
        .iter()
        .filter_map(|raw| {
            let clean = raw.trim();
            if clean.is_empty() {
                None
            } else {
                Some(format!("--exclude={clean}"))
            }
        })
        .collect()
}

pub fn build_workspace_diff_args(
    snap_path: &str,
    ws_path: &str,
    excludes: &[String],
) -> Vec<String> {
    let mut args = vec![
        "-a".to_string(),
        "-n".to_string(),
        "--delete".to_string(),
        "--itemize-changes".to_string(),
        "--out-format=%i %l %n".to_string(),
    ];
    args.extend(build_rsync_exclude_args(excludes));
    args.push(format!("{}/", clean_path(snap_path)));
    args.push(format!("{}/", clean_path(ws_path)));
    args
}

fn rsync_line_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(\*deleting\s+|[<>ch.*][fdLDS][^\s]{9})\s+(\d+)\s+(.+)$")
            .expect("valid rsync itemize regex")
    })
}

fn enrich_added_entry(ws_path: &str, row: &mut DiffEntry) {
    if row.status == "added" && row.entry_type != "dir" && !ws_path.is_empty() {
        let full_path = format!("{ws_path}/{}", row.path);
        let p = Path::new(&full_path);
        if let Ok(sym_meta) = fs::symlink_metadata(p) {
            if sym_meta.file_type().is_symlink() {
                row.entry_type = "symlink".to_string();
            } else if sym_meta.file_type().is_file() {
                let sz = sym_meta.len() as i64;
                row.size_bytes = sz;
                row.size_text = format_bytes(sz);
            }
        }
    }
}

/// Incremental line-by-line parser for `rsync --itemize-changes --out-format=%i %l %n`.
pub struct StreamingRsyncParser {
    ws_path: String,
    cap: usize,
    entries: Vec<DiffEntry>,
    added_count: i32,
    modified_count: i32,
    deleted_count: i32,
    truncated: bool,
}

impl StreamingRsyncParser {
    pub fn new(ws_path: &str, max_entries: i32) -> Self {
        Self {
            ws_path: ws_path.to_string(),
            cap: max_entries.max(1) as usize,
            entries: Vec::new(),
            added_count: 0,
            modified_count: 0,
            deleted_count: 0,
            truncated: false,
        }
    }

    pub fn is_truncated(&self) -> bool {
        self.truncated
    }

    /// Feeds one line of rsync stdout. Returns `true` if a new entry was added.
    pub fn push_line(&mut self, raw_line: &str) -> bool {
        if self.truncated || raw_line.is_empty() {
            return false;
        }
        let line = raw_line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            return false;
        }
        let Some(caps) = rsync_line_regex().captures(line) else {
            return false;
        };

        let code = &caps[1];
        let size_bytes = caps[2].parse::<i64>().unwrap_or(0);
        let mut rel_path = caps[3].to_string();

        if rel_path == "./" || rel_path == "." {
            return false;
        }
        if code.starts_with(".d") {
            return false;
        }

        let status: &str;
        let mut entry_type = "file";

        if code.starts_with("*deleting") {
            status = "added";
            if let Some(stripped) = rel_path.strip_suffix('/') {
                entry_type = "dir";
                rel_path = stripped.to_string();
            }
        } else {
            let type_char = code.as_bytes().get(1).copied().unwrap_or(b'f');
            if type_char == b'd' {
                entry_type = "dir";
                if let Some(stripped) = rel_path.strip_suffix('/') {
                    rel_path = stripped.to_string();
                }
            } else if type_char == b'L' {
                entry_type = "symlink";
            }

            if code.get(2..11) == Some("+++++++++") {
                status = "deleted";
            } else {
                status = "modified";
            }
        }

        if rel_path.is_empty() {
            return false;
        }

        if self.entries.len() >= self.cap {
            self.truncated = true;
            return false;
        }

        match status {
            "added" => self.added_count += 1,
            "deleted" => self.deleted_count += 1,
            _ => self.modified_count += 1,
        }

        let size_text = if entry_type == "dir" {
            "dir".to_string()
        } else {
            format_bytes(size_bytes)
        };

        let mut entry = DiffEntry {
            path: rel_path,
            status: status.to_string(),
            entry_type: entry_type.to_string(),
            size_bytes,
            size_text,
        };
        enrich_added_entry(&self.ws_path, &mut entry);
        self.entries.push(entry);
        true
    }

    pub fn snapshot_result(&self, snapshot_name: &str) -> WorkspaceDiffResult {
        WorkspaceDiffResult {
            summary: DiffSummary {
                snapshot_name: snapshot_name.to_string(),
                added_count: self.added_count,
                modified_count: self.modified_count,
                deleted_count: self.deleted_count,
                total_count: self.entries.len() as i32,
                truncated: self.truncated,
                error: String::new(),
            },
            entries: self.entries.clone(),
        }
    }

    pub fn into_parsed(self) -> ParsedRsyncItemize {
        let total_count = self.entries.len() as i32;
        ParsedRsyncItemize {
            entries: self.entries,
            truncated: self.truncated,
            added_count: self.added_count,
            modified_count: self.modified_count,
            deleted_count: self.deleted_count,
            total_count,
        }
    }
}

pub fn parse_rsync_itemize(output: &str, max_entries: i32) -> ParsedRsyncItemize {
    let mut parser = StreamingRsyncParser::new("", max_entries);
    for raw_line in output.split('\n') {
        parser.push_line(raw_line);
        if parser.is_truncated() {
            break;
        }
    }
    parser.into_parsed()
}

pub fn process_workspace_diff_output(
    snapshot_name: &str,
    ws_path: &str,
    rsync_stdout: &str,
) -> WorkspaceDiffResult {
    let mut parser = StreamingRsyncParser::new(ws_path, DEFAULT_MAX_DIFF_ENTRIES);
    for raw_line in rsync_stdout.split('\n') {
        parser.push_line(raw_line);
        if parser.is_truncated() {
            break;
        }
    }
    parser.snapshot_result(snapshot_name)
}

/// Returns the modification timestamp of `path` in nanoseconds since epoch.
pub fn workspace_root_mtime_nanos(ws_path: &str) -> Option<u128> {
    let meta = fs::metadata(ws_path).ok()?;
    let modified = meta.modified().ok()?;
    modified
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_nanos())
}

const DIFF_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Debug, Clone)]
struct CachedDiffEntry {
    ws_mtime_nanos: u128,
    snap_mtime_nanos: u128,
    excludes: Vec<String>,
    cached_at: std::time::Instant,
    result: WorkspaceDiffResult,
}

/// Caches workspace diff results keyed by `(env_name, snapshot_name)` and validated
/// against workspace root mtime, snapshot dir mtime, excludes, and a short TTL so
/// nested subdirectory edits are never hidden indefinitely.
#[derive(Debug, Default)]
pub struct WorkspaceDiffCache {
    entries: HashMap<(String, String), CachedDiffEntry>,
}

impl WorkspaceDiffCache {
    pub fn get(
        &self,
        env_name: &str,
        snapshot_name: &str,
        ws_mtime_nanos: Option<u128>,
        snap_mtime_nanos: Option<u128>,
        excludes: &[String],
    ) -> Option<&WorkspaceDiffResult> {
        let ws_mtime = ws_mtime_nanos?;
        let snap_mtime = snap_mtime_nanos?;
        let key = (env_name.to_string(), snapshot_name.to_string());
        let cached = self.entries.get(&key)?;
        if cached.ws_mtime_nanos == ws_mtime
            && cached.snap_mtime_nanos == snap_mtime
            && cached.excludes == excludes
            && cached.cached_at.elapsed() <= DIFF_CACHE_TTL
        {
            Some(&cached.result)
        } else {
            None
        }
    }

    pub fn insert(
        &mut self,
        env_name: &str,
        snapshot_name: &str,
        ws_mtime_nanos: Option<u128>,
        snap_mtime_nanos: Option<u128>,
        excludes: Vec<String>,
        result: WorkspaceDiffResult,
    ) {
        let (Some(ws_mtime), Some(snap_mtime)) = (ws_mtime_nanos, snap_mtime_nanos) else {
            return;
        };
        if !result.summary.error.is_empty() {
            return;
        }
        self.entries.insert(
            (env_name.to_string(), snapshot_name.to_string()),
            CachedDiffEntry {
                ws_mtime_nanos: ws_mtime,
                snap_mtime_nanos: snap_mtime,
                excludes,
                cached_at: std::time::Instant::now(),
                result,
            },
        );
    }

    pub fn invalidate_env(&mut self, env_name: &str) {
        self.entries.retain(|(env, _), _| env != env_name);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::defaults::default_diff_excludes;
    use crate::config::settings::parse_diff_excludes;
    use std::process::Command;
    use std::time::Instant;

    struct TempDirGuard(std::path::PathBuf);
    impl TempDirGuard {
        fn new(prefix: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "aibox-rust-rsync-{prefix}-{}",
                std::time::SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn parse_rsync_itemize_handles_fixtures_and_cap() {
        let fixture = "\
.d..t...... 40 ./\n\
.d..t...... 40 sub dir/\n\
*deleting   0 added file with spaces.txt\n\
*deleting   0 new dir/\n\
>f.s....... 42 src/main file.cpp\n\
>f+++++++++ 128 removed.txt\n\
cd+++++++++ 0 old dir/\n\
cL+++++++++ 12 deleted_link\n\
cLc........ 8 changed_link\n";

        let parsed = parse_rsync_itemize(fixture, 5000);
        assert!(!parsed.truncated);
        assert_eq!(parsed.added_count, 2);
        assert_eq!(parsed.modified_count, 2);
        assert_eq!(parsed.deleted_count, 3);
        assert_eq!(parsed.total_count, 7);

        let entries = &parsed.entries;
        assert_eq!(entries[0].path, "added file with spaces.txt");
        assert_eq!(entries[0].status, "added");
        assert_eq!(entries[0].entry_type, "file");

        assert_eq!(entries[1].path, "new dir");
        assert_eq!(entries[1].status, "added");
        assert_eq!(entries[1].entry_type, "dir");

        assert_eq!(entries[2].path, "src/main file.cpp");
        assert_eq!(entries[2].status, "modified");
        assert_eq!(entries[2].size_bytes, 42);

        assert_eq!(entries[3].path, "removed.txt");
        assert_eq!(entries[3].status, "deleted");

        assert_eq!(entries[4].path, "old dir");
        assert_eq!(entries[4].entry_type, "dir");
        assert_eq!(entries[4].status, "deleted");

        assert_eq!(entries[5].entry_type, "symlink");
        assert_eq!(entries[5].status, "deleted");
        assert_eq!(entries[6].entry_type, "symlink");
        assert_eq!(entries[6].status, "modified");

        let capped = parse_rsync_itemize(fixture, 3);
        assert!(capped.truncated);
        assert_eq!(capped.total_count, 3);
    }

    #[test]
    fn diff_excludes_and_args_building() {
        let defaults = default_diff_excludes();
        assert!(defaults.contains(&".git/".to_string()));
        assert!(defaults.contains(&"node_modules/".to_string()));
        assert!(defaults.contains(&"target/".to_string()));
        assert!(defaults.contains(&".venv/".to_string()));
        assert!(defaults.contains(&".aibox-paste/".to_string()));

        assert_eq!(
            parse_diff_excludes(".git/, dist/, *.log\ndist/"),
            vec![".git/", "dist/", "*.log"]
        );

        assert_eq!(
            build_rsync_exclude_args(&[".git/".into(), " ".into(), "build/".into()]),
            vec!["--exclude=.git/", "--exclude=build/"]
        );
    }

    #[test]
    fn streaming_parser_produces_incremental_batches_matching_batch_parse() {
        let lines = [
            "*deleting   0 a.txt\n",
            ">f.s....... 10 b.txt\n",
            ">f+++++++++ 20 c.txt\n",
        ];
        let mut parser = StreamingRsyncParser::new("", 5000);
        assert!(parser.push_line(lines[0]));
        let snap1 = parser.snapshot_result("s1");
        assert_eq!(snap1.entries.len(), 1);
        assert_eq!(snap1.summary.added_count, 1);

        assert!(parser.push_line(lines[1]));
        assert!(parser.push_line(lines[2]));
        let snap_final = parser.snapshot_result("s1");
        let batch = parse_rsync_itemize(&lines.concat(), 5000);
        assert_eq!(snap_final.entries, batch.entries);
        assert_eq!(snap_final.summary.total_count, batch.total_count);
    }

    #[test]
    fn workspace_diff_cache_hits_on_unchanged_mtime_and_invalidates() {
        let mut cache = WorkspaceDiffCache::default();
        let excludes = default_diff_excludes();
        let res = WorkspaceDiffResult {
            summary: DiffSummary {
                snapshot_name: "s1".to_string(),
                added_count: 1,
                modified_count: 0,
                deleted_count: 0,
                total_count: 1,
                truncated: false,
                error: String::new(),
            },
            entries: vec![],
        };

        cache.insert(
            "box1",
            "s1",
            Some(1000),
            Some(5000),
            excludes.clone(),
            res.clone(),
        );
        assert!(cache
            .get("box1", "s1", Some(1000), Some(5000), &excludes)
            .is_some());
        // Mtime change misses
        assert!(cache
            .get("box1", "s1", Some(2000), Some(5000), &excludes)
            .is_none());
        assert!(cache
            .get("box1", "s1", Some(1000), Some(6000), &excludes)
            .is_none());
        // Excludes change misses
        assert!(cache
            .get("box1", "s1", Some(1000), Some(5000), &[".git/".to_string()])
            .is_none());
        // Invalidate env clears
        cache.invalidate_env("box1");
        assert!(cache
            .get("box1", "s1", Some(1000), Some(5000), &excludes)
            .is_none());
    }

    #[test]
    fn real_rsync_detects_added_modified_deleted_and_honors_excludes() {
        let Ok(rsync_check) = Command::new("rsync").arg("--version").output() else {
            return; // skip when rsync is not installed
        };
        if !rsync_check.status.success() {
            return;
        }

        let dir = TempDirGuard::new("rsync");
        let snap = dir.path().join("snap");
        let ws = dir.path().join("ws");
        fs::create_dir_all(snap.join(".git")).unwrap();
        fs::create_dir_all(ws.join(".git")).unwrap();
        fs::create_dir_all(ws.join("node_modules")).unwrap();

        fs::write(snap.join("same.txt"), "same\n").unwrap();
        fs::write(ws.join("same.txt"), "same\n").unwrap();
        let perms = fs::metadata(snap.join("same.txt")).unwrap().permissions();
        fs::set_permissions(ws.join("same.txt"), perms).unwrap();
        let _ = Command::new("touch")
            .arg("-r")
            .arg(snap.join("same.txt"))
            .arg(ws.join("same.txt"))
            .status();

        fs::write(snap.join("edit.txt"), "v1\n").unwrap();
        fs::write(ws.join("edit.txt"), "v2-edited\n").unwrap();
        fs::write(snap.join("deleted.txt"), "bye\n").unwrap();
        fs::write(ws.join("added.txt"), "hi\n").unwrap();
        fs::write(ws.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(ws.join("node_modules/pkg.js"), "module.exports = {}\n").unwrap();

        let args = build_workspace_diff_args(
            snap.to_str().unwrap(),
            ws.to_str().unwrap(),
            &default_diff_excludes(),
        );
        let out = Command::new("rsync").args(&args).output().unwrap();
        assert_eq!(out.status.code(), Some(0));

        let res = parse_rsync_itemize(
            &String::from_utf8_lossy(&out.stdout),
            DEFAULT_MAX_DIFF_ENTRIES,
        );
        assert_eq!(res.added_count, 1);
        assert_eq!(res.modified_count, 1);
        assert_eq!(res.deleted_count, 1);
        assert_eq!(res.total_count, 3);
    }

    #[test]
    fn large_workspace_10k_files_diff_under_3_seconds_with_default_excludes() {
        let Ok(rsync_check) = Command::new("rsync").arg("--version").output() else {
            return;
        };
        if !rsync_check.status.success() {
            return;
        }

        let dir = TempDirGuard::new("10k");
        let snap = dir.path().join("snap");
        let ws = dir.path().join("ws");

        // Create 10,000 files across excluded heavy directories and active source files.
        let excluded_dirs = ["node_modules", "target", ".git/objects", ".venv"];
        for d in excluded_dirs {
            fs::create_dir_all(ws.join(d)).unwrap();
            fs::create_dir_all(snap.join(d)).unwrap();
        }
        fs::create_dir_all(ws.join("src")).unwrap();
        fs::create_dir_all(snap.join("src")).unwrap();

        for i in 0..9500 {
            let bucket = excluded_dirs[i % excluded_dirs.len()];
            fs::write(ws.join(format!("{bucket}/f_{i}.dat")), "x").unwrap();
        }
        for i in 0..500 {
            let rel = format!("src/mod_{i}.rs");
            fs::write(snap.join(&rel), "fn f() {}\n").unwrap();
            fs::write(ws.join(&rel), "fn f() {}\n").unwrap();
        }
        fs::write(ws.join("src/mod_0.rs"), "fn f() { let _ = 1; }\n").unwrap();
        fs::write(ws.join("src/new_file.rs"), "pub fn added() {}\n").unwrap();

        let args = build_workspace_diff_args(
            snap.to_str().unwrap(),
            ws.to_str().unwrap(),
            &default_diff_excludes(),
        );

        let started = Instant::now();
        let out = Command::new("rsync").args(&args).output().unwrap();
        let elapsed = started.elapsed();

        assert_eq!(out.status.code(), Some(0));
        assert!(
            elapsed.as_secs_f64() < 3.0,
            "10k workspace diff took {elapsed:?} (expected < 3s)"
        );

        let res = process_workspace_diff_output(
            "s1",
            ws.to_str().unwrap(),
            &String::from_utf8_lossy(&out.stdout),
        );
        assert!(res.entries.iter().any(|e| e.path == "src/new_file.rs"));
        assert!(!res
            .entries
            .iter()
            .any(|e| e.path.starts_with("node_modules/")
                || e.path.starts_with("target/")
                || e.path.starts_with(".git/")
                || e.path.starts_with(".venv/")));
    }
}
