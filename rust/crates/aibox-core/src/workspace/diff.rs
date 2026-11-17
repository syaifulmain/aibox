use std::fs;
use std::io::Read;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

pub use crate::config::defaults::default_diff_excludes;
pub use crate::config::settings::parse_diff_excludes;
use crate::snapshot::workspace::workspace_snapshot_dir;
use crate::util::format::format_bytes;
use crate::util::paths::clean_path;

pub const DEFAULT_MAX_DIFF_ENTRIES: i32 = 5000;
pub const DEFAULT_MAX_DIFF_LINES: i32 = 2000;
pub const MAX_PREVIEW_BYTES: u64 = 512 * 1024;
pub const BINARY_SAMPLE_BYTES: usize = 8192;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileDiffPrep {
    Immediate { text: String, info: String },
    RunDiff { args: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiffOutput {
    pub text: String,
    pub info: String,
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

pub fn is_safe_relative_path(relative_path: &str) -> bool {
    if relative_path.is_empty() || relative_path.starts_with('/') || relative_path.contains('\0') {
        return false;
    }
    let mut has_parts = false;
    for seg in relative_path.split('/').filter(|s| !s.is_empty()) {
        has_parts = true;
        if seg == "." || seg == ".." {
            return false;
        }
    }
    has_parts
}

pub fn is_likely_binary_data(sample: &[u8]) -> bool {
    let limit = sample.len().min(BINARY_SAMPLE_BYTES);
    sample[..limit].contains(&0)
}

pub fn cap_diff_lines(diff_output: &str, max_lines: i32) -> (String, bool) {
    let limit = max_lines.max(1) as usize;
    let lines: Vec<&str> = diff_output.split('\n').collect();
    if lines.len() <= limit {
        return (diff_output.to_string(), false);
    }
    (lines[..limit].join("\n"), true)
}

fn rsync_line_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(\*deleting\s+|[<>ch.*][fdLDS][^\s]{9})\s+(\d+)\s+(.+)$")
            .expect("valid rsync itemize regex")
    })
}

pub fn parse_rsync_itemize(output: &str, max_entries: i32) -> ParsedRsyncItemize {
    let re = rsync_line_regex();
    let cap = max_entries.max(1) as usize;

    let mut entries = Vec::new();
    let mut added_count = 0_i32;
    let mut modified_count = 0_i32;
    let mut deleted_count = 0_i32;
    let mut truncated = false;

    for raw_line in output.split('\n').filter(|s| !s.is_empty()) {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        let Some(caps) = re.captures(line) else {
            continue;
        };

        let code = &caps[1];
        let size_bytes = caps[2].parse::<i64>().unwrap_or(0);
        let mut rel_path = caps[3].to_string();

        if rel_path == "./" || rel_path == "." {
            continue;
        }
        if code.starts_with(".d") {
            continue;
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
            continue;
        }

        if entries.len() >= cap {
            truncated = true;
            break;
        }

        match status {
            "added" => added_count += 1,
            "deleted" => deleted_count += 1,
            _ => modified_count += 1,
        }

        let size_text = if entry_type == "dir" {
            "dir".to_string()
        } else {
            format_bytes(size_bytes)
        };

        entries.push(DiffEntry {
            path: rel_path,
            status: status.to_string(),
            entry_type: entry_type.to_string(),
            size_bytes,
            size_text,
        });
    }

    let total_count = entries.len() as i32;
    ParsedRsyncItemize {
        entries,
        truncated,
        added_count,
        modified_count,
        deleted_count,
        total_count,
    }
}

pub fn process_workspace_diff_output(
    snapshot_name: &str,
    ws_path: &str,
    rsync_stdout: &str,
) -> WorkspaceDiffResult {
    let parsed = parse_rsync_itemize(rsync_stdout, DEFAULT_MAX_DIFF_ENTRIES);
    let mut entries = parsed.entries;

    for row in &mut entries {
        if row.status == "added" && row.entry_type != "dir" {
            let full_path = format!("{ws_path}/{}", row.path);
            let p = Path::new(&full_path);
            if let Ok(sym_meta) = fs::symlink_metadata(p) {
                if sym_meta.file_type().is_symlink() {
                    row.entry_type = "symlink".to_string();
                } else if let Ok(meta) = fs::metadata(p) {
                    let sz = meta.len() as i64;
                    row.size_bytes = sz;
                    row.size_text = format_bytes(sz);
                }
            }
        }
    }

    WorkspaceDiffResult {
        summary: DiffSummary {
            snapshot_name: snapshot_name.to_string(),
            added_count: parsed.added_count,
            modified_count: parsed.modified_count,
            deleted_count: parsed.deleted_count,
            total_count: parsed.total_count,
            truncated: parsed.truncated,
            error: String::new(),
        },
        entries,
    }
}

fn read_sample_bytes(path: &Path) -> Vec<u8> {
    let Ok(mut f) = fs::File::open(path) else {
        return Vec::new();
    };
    let mut buf = vec![0_u8; BINARY_SAMPLE_BYTES];
    let n = f.read(&mut buf).unwrap_or(0);
    buf.truncate(n);
    buf
}

fn symlink_target_display(path: &Path) -> String {
    match fs::read_link(path) {
        Ok(target) => target.to_string_lossy().into_owned(),
        Err(_) => "—".to_string(),
    }
}

pub fn prepare_file_diff(
    name: &str,
    snapshot_name: &str,
    ws_path: &str,
    relative_path: &str,
    status: &str,
) -> FileDiffPrep {
    if name.is_empty() || snapshot_name.is_empty() || !is_safe_relative_path(relative_path) {
        return FileDiffPrep::Immediate {
            text: String::new(),
            info: "Invalid file path.".to_string(),
        };
    }

    let snap_dir = workspace_snapshot_dir(ws_path, name);
    let snap_file = format!("{snap_dir}/{snapshot_name}/{relative_path}");
    let ws_file = format!("{}/{relative_path}", clean_path(ws_path));

    let check_snap = status != "added";
    let check_ws = status != "deleted";

    let snap_path = Path::new(&snap_file);
    let ws_file_path = Path::new(&ws_file);

    let snap_sym = if check_snap {
        fs::symlink_metadata(snap_path).ok()
    } else {
        None
    };
    let ws_sym = if check_ws {
        fs::symlink_metadata(ws_file_path).ok()
    } else {
        None
    };

    let snap_is_link = snap_sym
        .as_ref()
        .is_some_and(|m| m.file_type().is_symlink());
    let ws_is_link = ws_sym.as_ref().is_some_and(|m| m.file_type().is_symlink());

    if snap_is_link || ws_is_link {
        let snap_target = if snap_is_link {
            symlink_target_display(snap_path)
        } else {
            "—".to_string()
        };
        let ws_target = if ws_is_link {
            symlink_target_display(ws_file_path)
        } else {
            "—".to_string()
        };
        return FileDiffPrep::Immediate {
            text: format!("Symlink target:\n  snapshot: {snap_target}\n  current:  {ws_target}"),
            info: relative_path.to_string(),
        };
    }

    let snap_meta = if check_snap {
        fs::metadata(snap_path).ok()
    } else {
        None
    };
    let ws_meta = if check_ws {
        fs::metadata(ws_file_path).ok()
    } else {
        None
    };

    if snap_meta.as_ref().is_some_and(|m| m.is_dir())
        || ws_meta.as_ref().is_some_and(|m| m.is_dir())
    {
        return FileDiffPrep::Immediate {
            text: String::new(),
            info: "Directory entry — select a file to view its diff.".to_string(),
        };
    }

    let snap_size = snap_meta.as_ref().map_or(0, |m| m.len());
    let ws_size = ws_meta.as_ref().map_or(0, |m| m.len());
    if (check_snap && snap_size > MAX_PREVIEW_BYTES) || (check_ws && ws_size > MAX_PREVIEW_BYTES) {
        let max_sz = snap_size.max(ws_size) as i64;
        return FileDiffPrep::Immediate {
            text: String::new(),
            info: format!(
                "File exceeds 512 KB preview limit ({}).",
                format_bytes(max_sz)
            ),
        };
    }

    if (check_snap && is_likely_binary_data(&read_sample_bytes(snap_path)))
        || (check_ws && is_likely_binary_data(&read_sample_bytes(ws_file_path)))
    {
        return FileDiffPrep::Immediate {
            text: String::new(),
            info: "Binary file — text diff unavailable.".to_string(),
        };
    }

    let left_arg = if check_snap {
        snap_file
    } else {
        "/dev/null".to_string()
    };
    let right_arg = if check_ws {
        ws_file
    } else {
        "/dev/null".to_string()
    };

    FileDiffPrep::RunDiff {
        args: vec![
            "-u".to_string(),
            "--label".to_string(),
            "snapshot".to_string(),
            "--label".to_string(),
            "current".to_string(),
            left_arg,
            right_arg,
        ],
    }
}

pub fn shape_file_diff_output(exit_code: i32, stdout: &str, relative_path: &str) -> FileDiffOutput {
    if exit_code != 0 && exit_code != 1 {
        return FileDiffOutput {
            text: String::new(),
            info: format!("Could not generate diff for {relative_path}."),
        };
    }
    let (capped, truncated) = cap_diff_lines(stdout, DEFAULT_MAX_DIFF_LINES);
    let info = if truncated {
        format!("{relative_path} (diff capped at 2000 lines)")
    } else {
        relative_path.to_string()
    };
    FileDiffOutput { text: capped, info }
}

pub fn restore_single_file(
    name: &str,
    snapshot_name: &str,
    ws_path: &str,
    relative_path: &str,
) -> Result<String, String> {
    if name.is_empty() || snapshot_name.is_empty() || !is_safe_relative_path(relative_path) {
        return Err("Invalid file restore request.".to_string());
    }

    let snap_dir = workspace_snapshot_dir(ws_path, name);
    let snap_file = format!("{snap_dir}/{snapshot_name}/{relative_path}");
    let ws_file = format!("{}/{relative_path}", clean_path(ws_path));

    let src_path = Path::new(&snap_file);
    let dst_path = Path::new(&ws_file);

    let Ok(src_meta) = fs::symlink_metadata(src_path) else {
        return Err(format!(
            "File '{relative_path}' does not exist in snapshot '{snapshot_name}'."
        ));
    };

    if src_meta.is_dir() {
        return Err("Cannot single-restore a directory; use Snapshot Restore instead.".to_string());
    }

    if let Some(parent) = dst_path.parent() {
        if !parent.as_os_str().is_empty() && fs::create_dir_all(parent).is_err() {
            return Err(format!(
                "Could not create parent directory: {}",
                parent.display()
            ));
        }
    }

    if fs::symlink_metadata(dst_path).is_ok() && fs::remove_file(dst_path).is_err() {
        return Err(format!("Could not overwrite workspace file: {ws_file}"));
    }

    if src_meta.file_type().is_symlink() {
        let Ok(target) = fs::read_link(src_path) else {
            return Err(format!("Could not restore symlink: {relative_path}"));
        };
        if symlink(&target, dst_path).is_err() {
            return Err(format!("Could not restore symlink: {relative_path}"));
        }
    } else {
        if fs::copy(src_path, dst_path).is_err() {
            return Err(format!(
                "Could not copy file from snapshot: {relative_path}"
            ));
        }
        let _ = fs::set_permissions(dst_path, src_meta.permissions());
    }

    Ok(format!("Restored {snap_file} -> {ws_file}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    struct TempDirGuard(std::path::PathBuf);
    impl TempDirGuard {
        fn new(prefix: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "aibox-rust-diff-{prefix}-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
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
    fn diff_excludes_binary_check_and_line_capping() {
        let defaults = default_diff_excludes();
        assert!(defaults.contains(&".git/".to_string()));
        assert!(defaults.contains(&"node_modules/".to_string()));
        assert!(defaults.contains(&".aibox-paste/".to_string()));

        assert_eq!(
            parse_diff_excludes(".git/, dist/, *.log\ndist/"),
            vec![".git/", "dist/", "*.log"]
        );

        assert_eq!(
            build_rsync_exclude_args(&[".git/".into(), " ".into(), "build/".into()]),
            vec!["--exclude=.git/", "--exclude=build/"]
        );

        assert!(is_safe_relative_path("src/main.cpp"));
        assert!(!is_safe_relative_path("/etc/passwd"));
        assert!(!is_safe_relative_path("../secret.txt"));
        assert!(!is_safe_relative_path("a/../b"));

        assert!(!is_likely_binary_data(b"hello\nworld\n"));
        assert!(is_likely_binary_data(b"ELF\x00\x01\x02"));

        let big_lines: Vec<String> = (0..2050).map(|i| format!("+line {i}")).collect();
        let (capped, truncated) = cap_diff_lines(&big_lines.join("\n"), 2000);
        assert!(truncated);
        assert_eq!(capped.split('\n').count(), 2000);
    }

    #[test]
    fn hostile_relative_paths_are_rejected() {
        let hostile = [
            "",
            "/",
            "///",
            "/etc/passwd",
            ".",
            "..",
            "./a",
            "../a",
            "a/.",
            "a/..",
            "a/./b",
            "a/../b",
            "a\0b",
            "a/b\0",
        ];
        for path in hostile {
            assert!(!is_safe_relative_path(path), "expected rejected: {path:?}");
        }

        let safe = ["a", "a/b.txt", ".../file", "a..b/c.d", ".hidden/file"];
        for path in safe {
            assert!(is_safe_relative_path(path), "expected safe: {path:?}");
        }
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
    fn prepare_and_restore_single_file_handle_files_symlinks_and_errors() {
        let dir = TempDirGuard::new("restore");
        let ws = dir.path().join("workspaces/box1");
        let snap_dir = workspace_snapshot_dir(ws.to_str().unwrap(), "box1");
        let snap = Path::new(&snap_dir).join("s1");
        fs::create_dir_all(ws.join("sub")).unwrap();
        fs::create_dir_all(snap.join("sub")).unwrap();

        fs::write(snap.join("sub/a.txt"), "original\n").unwrap();
        fs::set_permissions(snap.join("sub/a.txt"), fs::Permissions::from_mode(0o640)).unwrap();
        fs::write(ws.join("sub/a.txt"), "modified\n").unwrap();
        symlink("a.txt", snap.join("sub/link")).unwrap();
        fs::write(ws.join("bin.dat"), b"ABC\x00DEF").unwrap();

        assert_eq!(
            prepare_file_diff("box1", "s1", ws.to_str().unwrap(), "../bad", "modified"),
            FileDiffPrep::Immediate {
                text: String::new(),
                info: "Invalid file path.".to_string(),
            }
        );

        assert_eq!(
            prepare_file_diff("box1", "s1", ws.to_str().unwrap(), "bin.dat", "added"),
            FileDiffPrep::Immediate {
                text: String::new(),
                info: "Binary file — text diff unavailable.".to_string(),
            }
        );

        match prepare_file_diff("box1", "s1", ws.to_str().unwrap(), "sub/a.txt", "modified") {
            FileDiffPrep::RunDiff { args } => {
                assert_eq!(args[0], "-u");
                assert_eq!(args[2], "snapshot");
                assert_eq!(args[4], "current");
            }
            other => panic!("expected RunDiff, got {other:?}"),
        }

        let ok = restore_single_file("box1", "s1", ws.to_str().unwrap(), "sub/a.txt").unwrap();
        assert!(ok.starts_with("Restored "));
        assert_eq!(
            fs::read_to_string(ws.join("sub/a.txt")).unwrap(),
            "original\n"
        );
        assert_eq!(
            fs::metadata(ws.join("sub/a.txt"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o640
        );

        restore_single_file("box1", "s1", ws.to_str().unwrap(), "sub/link").unwrap();
        assert!(fs::symlink_metadata(ws.join("sub/link"))
            .unwrap()
            .file_type()
            .is_symlink());

        assert!(restore_single_file("box1", "s1", ws.to_str().unwrap(), "sub").is_err());
        assert!(restore_single_file("box1", "s1", ws.to_str().unwrap(), "missing.txt").is_err());
        assert!(restore_single_file("box1", "s1", ws.to_str().unwrap(), "../escape").is_err());

        let shaped = shape_file_diff_output(2, "", "sub/a.txt");
        assert_eq!(shaped.info, "Could not generate diff for sub/a.txt.");
    }
}
