//! Path normalization and environment directory lookup (without symlink resolution).

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::util::error::IoError;
use crate::util::json::to_json_indented;

/// Container-side mount path for the shared workspace directory.
pub const CONTAINER_WORKSPACE_PATH: &str = "/home/workspace";

/// Reimplements `QDir::cleanPath` on POSIX paths without resolving symlinks.
pub fn clean_path(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    let is_abs = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if let Some(&last) = parts.last() {
                    if last != ".." {
                        parts.pop();
                    } else if !is_abs {
                        parts.push("..");
                    }
                } else if !is_abs {
                    parts.push("..");
                }
            }
            other => parts.push(other),
        }
    }
    if is_abs {
        if parts.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", parts.join("/"))
        }
    } else if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

/// Returns `$HOME` (falling back to `/`).
pub fn home_dir() -> PathBuf {
    match env::var_os("HOME") {
        Some(val) if !val.is_empty() => PathBuf::from(val),
        _ => PathBuf::from("/"),
    }
}

/// Returns `$XDG_CONFIG_HOME` if set and non-empty, otherwise `$HOME/.config`.
pub fn xdg_config_dir() -> PathBuf {
    match env::var_os("XDG_CONFIG_HOME") {
        Some(val) if !val.is_empty() => PathBuf::from(val),
        _ => home_dir().join(".config"),
    }
}

/// Expands a leading `~` or `~/` using `home_dir()`.
pub fn expand_tilde(raw: &str) -> String {
    if raw == "~" {
        home_dir().to_string_lossy().into_owned()
    } else if let Some(rest) = raw.strip_prefix("~/") {
        format!("{}/{}", home_dir().to_string_lossy(), rest)
    } else {
        raw.to_string()
    }
}

/// Atomically writes `data` to `path` by writing a sibling temp file, syncing, and renaming.
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<(), IoError> {
    atomic_write_with_mode(path, data, None)
}

/// Like [`atomic_write`], but applies the Unix permission `mode` (e.g. `0o600`)
/// to the temp file before any data is written, so the final file is never
/// readable with looser permissions.
///
/// The temp file is fsynced before the rename and the parent directory after
/// it, so the new contents survive a crash once this returns `Ok`.
pub fn atomic_write_with_mode(path: &Path, data: &[u8], mode: Option<u32>) -> Result<(), IoError> {
    atomic_write_with(path, mode, |file| {
        file.write_all(data)?;
        Ok(true)
    })?;
    Ok(())
}

/// Streams data to a sibling temp file via `write_fn` and atomically renames it
/// over `path` if `write_fn` returns `Ok(true)`.
///
/// Returning `Ok(false)` aborts the replacement cleanly without touching `path`.
pub fn atomic_write_with<F>(path: &Path, mode: Option<u32>, write_fn: F) -> Result<bool, IoError>
where
    F: FnOnce(&mut fs::File) -> std::io::Result<bool>,
{
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|e| IoError::new("create directory", parent, e))?;

    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("tmp");
    let pid = std::process::id();
    static ATOMIC_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    let (mut file, tmp_path) = {
        let mut opened = None;
        let mut last_err = None;
        for _ in 0..16 {
            let seq = ATOMIC_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let candidate = parent.join(format!(".{file_name}.tmp.{pid}.{seq}"));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            if let Some(mode) = mode {
                options.mode(mode);
            }
            match options.open(&candidate) {
                Ok(f) => {
                    opened = Some((f, candidate));
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    last_err = Some((candidate, e));
                }
                Err(e) => return Err(IoError::new("open", &candidate, e)),
            }
        }
        match opened {
            Some(pair) => pair,
            None => {
                let (p, e) = last_err.unwrap_or_else(|| {
                    (
                        parent.join(format!(".{file_name}.tmp.{pid}")),
                        std::io::Error::new(
                            std::io::ErrorKind::AlreadyExists,
                            "temp file collision",
                        ),
                    )
                });
                return Err(IoError::new("open", &p, e));
            }
        }
    };

    let write_result = (|| -> Result<bool, IoError> {
        if let Some(mode) = mode {
            // The umask may have stripped bits from `mode`, so set them explicitly.
            file.set_permissions(fs::Permissions::from_mode(mode))
                .map_err(|e| IoError::new("set permissions on", &tmp_path, e))?;
        }
        let commit = write_fn(&mut file).map_err(|e| IoError::new("write", &tmp_path, e))?;
        if !commit {
            return Ok(false);
        }
        file.sync_all()
            .map_err(|e| IoError::new("sync", &tmp_path, e))?;
        Ok(true)
    })();

    let commit = match write_result {
        Ok(commit) => commit,
        Err(err) => {
            let _ = fs::remove_file(&tmp_path);
            return Err(err);
        }
    };

    if !commit {
        let _ = fs::remove_file(&tmp_path);
        return Ok(false);
    }

    if let Err(e) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(IoError::new("rename", path, e));
    }

    // Persist the rename itself; some file systems do not support fsync on
    // directories, so a failure here is not fatal once the rename succeeded.
    if let Ok(dir) = fs::File::open(parent) {
        let _ = dir.sync_all();
    }
    Ok(true)
}

/// Default global AIBox root directory (`$HOME/aibox`).
pub fn default_system_path() -> String {
    clean_path(&format!("{}/aibox", home_dir().to_string_lossy()))
}

/// Default workspace base directory (`$HOME/aibox/workspaces`).
pub fn default_workspace_path() -> String {
    format!("{}/workspaces", default_system_path())
}

/// Default OS image storage directory (`$HOME/aibox/os-images`).
pub fn default_os_storage_path() -> String {
    format!("{}/os-images", default_system_path())
}

/// Path to `location.json` (overridable via `AIBOX_LOCATION_FILE`).
pub fn default_location_file_path() -> String {
    if let Some(env_path) = env::var_os("AIBOX_LOCATION_FILE") {
        let trimmed = env_path.to_string_lossy().trim().to_string();
        if !trimmed.is_empty() {
            return clean_path(&trimmed);
        }
    }
    let cfg = xdg_config_dir();
    let base = cfg.to_string_lossy();
    let base_trimmed = base.trim();
    let resolved_base = if base_trimmed.is_empty() {
        format!("{}/.config", home_dir().to_string_lossy())
    } else {
        base_trimmed.to_string()
    };
    clean_path(&format!("{resolved_base}/aibox/location.json"))
}

/// Reads the configured system path from `location.json`, falling back to `default_system_path()`.
pub fn configured_system_path() -> String {
    let loc_path = default_location_file_path();
    if let Ok(bytes) = fs::read(&loc_path) {
        if let Ok(Value::Object(obj)) = serde_json::from_slice::<Value>(&bytes) {
            if let Some(raw_val) = obj.get("systemPath").and_then(|v| v.as_str()) {
                let trimmed = raw_val.trim();
                if !trimmed.is_empty() {
                    let mut raw = expand_tilde(trimmed);
                    if !raw.starts_with('/') {
                        raw = format!("{}/{}", home_dir().to_string_lossy(), raw);
                    }
                    return clean_path(&raw);
                }
            }
        }
    }
    default_system_path()
}

/// Saves `system_path` to `location.json`.
pub fn save_location_file(system_path: &str) -> Result<(), String> {
    let loc_path = default_location_file_path();
    let payload = json!({
        "version": 1,
        "systemPath": clean_path(system_path),
    });
    let json_text = to_json_indented(&payload)
        .map_err(|e| format!("failed to serialize location.json: {e}"))?;
    atomic_write(Path::new(&loc_path), json_text.as_bytes()).map_err(String::from)
}

/// Resolves the `.aibox` / `aibox` directory for a project or global mode.
pub fn resolve_project_aibox_dir(project_dir: &str, base_cwd: &str) -> String {
    let dir_trimmed = project_dir.trim();
    if dir_trimmed.is_empty() {
        let cwd_trimmed = base_cwd.trim();
        if cwd_trimmed.is_empty() {
            return configured_system_path();
        }
        return clean_path(&format!("{}/aibox", clean_path(cwd_trimmed)));
    }

    let cwd = if base_cwd.trim().is_empty() {
        env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("/"))
            .to_string_lossy()
            .into_owned()
    } else {
        clean_path(base_cwd.trim())
    };

    let mut dir = expand_tilde(dir_trimmed);
    if !dir.starts_with('/') {
        dir = format!("{cwd}/{dir}");
    }
    dir = clean_path(&dir);

    let base_name = Path::new(&dir)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if base_name == "aibox" || base_name == ".aibox" {
        return dir;
    }

    let legacy_dot_dir = clean_path(&format!("{dir}/.aibox"));
    let std_dir = clean_path(&format!("{dir}/aibox"));
    if Path::new(&legacy_dot_dir).is_dir() && !Path::new(&std_dir).is_dir() {
        return legacy_dot_dir;
    }
    std_dir
}

/// Normalizes `raw` (expanding `~` and resolving relative paths against `$HOME`), or uses `fallback`.
pub fn normalized_path(raw: &str, fallback: &str) -> String {
    let mut path = raw.trim().to_string();
    if path.is_empty() {
        path = fallback.to_string();
    }
    path = expand_tilde(&path);
    if !path.starts_with('/') {
        path = format!("{}/{}", home_dir().to_string_lossy(), path);
    }
    clean_path(&path)
}

/// Validates and resolves a workspace path for `env_name` under `raw_parent` or `default_base`.
pub fn resolve_workspace_path(
    raw_parent: &str,
    env_name: &str,
    default_base: &str,
) -> Result<String, String> {
    let mut parent = raw_parent.trim().to_string();
    if parent.is_empty() {
        parent = default_base.trim().to_string();
        if parent.is_empty() {
            return Err("The default shared location is not configured.".to_string());
        }
    } else {
        parent = expand_tilde(&parent);
        if !parent.starts_with('/') {
            return Err("The custom shared location must be an absolute path.".to_string());
        }
        parent = clean_path(&parent);

        let home_clean = clean_path(&home_dir().to_string_lossy());
        const BLOCKED_EXACT: &[&str] = &[
            "/", "/bin", "/boot", "/dev", "/etc", "/home", "/lib", "/lib64", "/proc", "/root",
            "/run", "/sbin", "/sys", "/usr", "/var",
        ];
        if BLOCKED_EXACT.contains(&parent.as_str()) || parent == home_clean {
            return Err(format!(
                "Refusing to use system root or home directory '{parent}' as shared location. Pick a specific subfolder."
            ));
        }

        const BLOCKED_PREFIXES: &[&str] = &[
            "/bin/", "/boot/", "/dev/", "/etc/", "/lib/", "/lib64/", "/proc/", "/root/", "/run/",
            "/sbin/", "/sys/", "/usr/",
        ];
        if BLOCKED_PREFIXES.iter().any(|p| parent.starts_with(p)) {
            return Err(format!(
                "Refusing to use system path '{parent}' as shared location."
            ));
        }
    }

    let clean_name = env_name.trim();
    if clean_name.is_empty()
        || clean_name == "."
        || clean_name == ".."
        || clean_name.contains('/')
        || clean_name.contains('\0')
    {
        return Err(
            "Please enter a valid environment name (letters, numbers, hyphens).".to_string(),
        );
    }

    Ok(clean_path(&format!("{parent}/{clean_name}")))
}

/// Replaces every character outside `[A-Za-z0-9._-]` with `_`.
fn safe_path_component(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Returns the cache directory for `os_id` under `os_storage_path`.
pub fn cache_base_path(os_storage_path: &str, os_id: &str) -> String {
    format!("{os_storage_path}/{}", safe_path_component(os_id))
}

/// Returns the workspace path for `name`, preferring an explicit override in `env_paths`.
pub fn workspace_path_for(workspace_base: &str, env_paths: &[(&str, &str)], name: &str) -> String {
    for (env_name, ws_path) in env_paths {
        if *env_name == name && !ws_path.is_empty() {
            return (*ws_path).to_string();
        }
    }
    format!("{workspace_base}/{name}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_path_matches_qdir_semantics() {
        assert_eq!(clean_path(""), "");
        assert_eq!(clean_path("."), ".");
        assert_eq!(clean_path("/"), "/");
        assert_eq!(clean_path("/../../a//b/./c/.."), "/a/b");
        assert_eq!(clean_path("a/b/../c/"), "a/c");
        assert_eq!(clean_path("../a/../../b"), "../../b");
    }

    #[test]
    fn cache_base_path_replaces_unsafe_characters() {
        assert_eq!(
            cache_base_path("/os", "ubuntu/24.04 lts:amd64"),
            "/os/ubuntu_24.04_lts_amd64"
        );
        assert_eq!(cache_base_path("/os", "a-b_c.d"), "/os/a-b_c.d");
        // Each non-ASCII char maps to exactly one underscore, like the old regex.
        assert_eq!(cache_base_path("/os", "\u{e6}\u{f8}x"), "/os/__x");
        assert_eq!(cache_base_path("/os", "../etc"), "/os/.._etc");
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("aibox-util-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn atomic_write_replaces_contents_and_leaves_no_temp_file() {
        let dir = temp_dir("atomic");
        let target = dir.join("state.json");
        atomic_write(&target, b"one").unwrap();
        atomic_write(&target, b"two").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"two");
        let entries: Vec<_> = fs::read_dir(&dir).unwrap().collect();
        assert_eq!(entries.len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn atomic_write_with_mode_sets_permissions() {
        let dir = temp_dir("mode");
        let target = dir.join("secret.json");
        atomic_write_with_mode(&target, b"{}", Some(0o600)).unwrap();
        let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn atomic_write_in_read_only_dir_returns_error() {
        let dir = temp_dir("readonly");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
        let target = dir.join("state.json");
        // Root ignores directory permissions; only assert when the probe fails.
        let probe_denied = fs::write(dir.join("probe"), b"x").is_err();
        let result = atomic_write(&target, b"data");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
        if probe_denied {
            let err = result.unwrap_err();
            assert_eq!(err.op, "open");
            assert!(err.to_string().contains("failed to open"));
            assert!(!target.exists());
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
