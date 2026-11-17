//! Path normalization and environment directory lookup (without symlink resolution).

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde_json::{json, Value};

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
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|e| format!("failed to create directory {}: {e}", parent.display()))?;

    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("tmp");
    let pid = std::process::id();
    let tmp_path = parent.join(format!(".{file_name}.tmp.{pid}"));

    let write_result = (|| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_path)
            .map_err(|e| format!("failed to open {}: {e}", tmp_path.display()))?;
        file.write_all(data)
            .map_err(|e| format!("failed to write {}: {e}", tmp_path.display()))?;
        file.sync_all()
            .map_err(|e| format!("failed to sync {}: {e}", tmp_path.display()))?;
        Ok(())
    })();

    if let Err(err) = write_result {
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }

    if let Err(e) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("failed to rename {}: {e}", path.display()));
    }
    Ok(())
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
    atomic_write(Path::new(&loc_path), json_text.as_bytes())
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
    if clean_name.is_empty() {
        return Err(
            "Please enter a valid environment name (letters, numbers, hyphens).".to_string(),
        );
    }

    Ok(clean_path(&format!("{parent}/{clean_name}")))
}

/// Returns the cache directory for `os_id` under `os_storage_path`.
pub fn cache_base_path(os_storage_path: &str, os_id: &str) -> String {
    let re = Regex::new(r"[^A-Za-z0-9._-]").expect("valid regex");
    let safe = re.replace_all(os_id, "_");
    format!("{os_storage_path}/{safe}")
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
}
