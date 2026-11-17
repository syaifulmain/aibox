//! Path normalization and environment directory lookup (without symlink resolution).

use std::env;
use std::path::PathBuf;

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
