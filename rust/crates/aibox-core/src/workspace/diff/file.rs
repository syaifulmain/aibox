use std::fs;
use std::io::Read;
use std::path::Path;

use crate::snapshot::workspace::workspace_snapshot_dir;
use crate::util::format::format_bytes;
use crate::util::paths::clean_path;

pub const DEFAULT_MAX_DIFF_LINES: i32 = 2000;
pub const MAX_PREVIEW_BYTES: u64 = 512 * 1024;
pub const BINARY_SAMPLE_BYTES: usize = 8192;

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

pub(crate) fn is_safe_snapshot_name(snapshot_name: &str) -> bool {
    !snapshot_name.is_empty()
        && snapshot_name != "."
        && snapshot_name != ".."
        && !snapshot_name.contains('/')
        && !snapshot_name.contains('\0')
}

pub fn has_intermediate_symlink_escape(root_dir: &Path, relative_path: &str) -> bool {
    let Ok(canon_root) = fs::canonicalize(root_dir) else {
        return false;
    };
    let mut cur = root_dir.to_path_buf();
    let parts: Vec<&str> = relative_path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() <= 1 {
        return false;
    }
    for seg in &parts[..parts.len() - 1] {
        cur.push(seg);
        match fs::symlink_metadata(&cur) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return true;
                }
                if let Ok(canon_cur) = fs::canonicalize(&cur) {
                    if !canon_cur.starts_with(&canon_root) {
                        return true;
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(_) => return true,
        }
    }
    false
}

pub fn prepare_file_diff(
    name: &str,
    snapshot_name: &str,
    ws_path: &str,
    relative_path: &str,
    status: &str,
) -> FileDiffPrep {
    if name.is_empty()
        || !is_safe_snapshot_name(snapshot_name)
        || !is_safe_relative_path(relative_path)
    {
        return FileDiffPrep::Immediate {
            text: String::new(),
            info: "Invalid file path.".to_string(),
        };
    }

    let snap_dir = workspace_snapshot_dir(ws_path, name);
    let snap_root = format!("{snap_dir}/{snapshot_name}");
    let clean_ws = clean_path(ws_path);
    let snap_file = format!("{snap_root}/{relative_path}");
    let ws_file = format!("{clean_ws}/{relative_path}");

    let check_snap = status != "added";
    let check_ws = status != "deleted";

    if (check_snap && has_intermediate_symlink_escape(Path::new(&snap_root), relative_path))
        || (check_ws && has_intermediate_symlink_escape(Path::new(&clean_ws), relative_path))
    {
        return FileDiffPrep::Immediate {
            text: String::new(),
            info: "Invalid file path (symlink escape).".to_string(),
        };
    }

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

    if snap_meta.as_ref().is_some_and(|m| !m.is_file())
        || ws_meta.as_ref().is_some_and(|m| !m.is_file())
    {
        return FileDiffPrep::Immediate {
            text: String::new(),
            info: "Special file — text diff unavailable.".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_check_and_line_capping() {
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
}
