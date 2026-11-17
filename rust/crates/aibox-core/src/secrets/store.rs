//! Secret name validation and 0600-enforced JSON store IO.

use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;
use serde_json::{json, Value};

use crate::util::json::to_json_indented;
use crate::util::paths::{clean_path, xdg_config_dir};

static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

pub(crate) fn unique_id128() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id() as u128;
    let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed) as u128;
    let mixed = nanos ^ (pid << 64) ^ (seq << 32);
    format!("{mixed:032x}")
}

/// Returns the default host path for the global secrets store (`~/.config/aibox/secrets.json`).
pub fn default_secrets_store_path() -> String {
    let raw = xdg_config_dir().join("aibox/secrets.json");
    clean_path(&raw.to_string_lossy())
}

/// Returns true if `name` is safe to use as a per-environment secrets JSON filename.
pub fn is_safe_environment_file_name(name: &str) -> bool {
    if name.len() > 64 {
        return false;
    }
    let bytes = name.as_bytes();
    let Some(&first) = bytes.first() else {
        return false;
    };
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Returns the `secrets.d` directory next to `secrets_path`, or empty if `secrets_path` is blank.
pub fn instance_secrets_dir_path(secrets_path: &str) -> String {
    let trimmed = secrets_path.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let abs = if Path::new(trimmed).is_absolute() {
        PathBuf::from(trimmed)
    } else {
        env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("/"))
            .join(trimmed)
    };
    let clean_file = clean_path(&abs.to_string_lossy());
    let parent = Path::new(&clean_file)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/".to_string());
    clean_path(&format!("{parent}/secrets.d"))
}

/// Returns the per-environment secrets store path (`<secrets.d>/<env>.json`), or empty if invalid.
pub fn environment_secrets_store_path(secrets_path: &str, env_name: &str) -> String {
    let clean_name = env_name.trim();
    let dir_path = instance_secrets_dir_path(secrets_path);
    if dir_path.is_empty() || !is_safe_environment_file_name(clean_name) {
        return String::new();
    }
    clean_path(&format!("{dir_path}/{clean_name}.json"))
}

/// Returns true if `name` matches `^[A-Z_][A-Z0-9_]*$`.
pub fn is_valid_secret_name(name: &str) -> bool {
    // Use \A..\z so trailing newlines are rejected like QRegularExpression::match on single line,
    // wait: QRegularExpression "^...$" without MultilineOption still matches before trailing \n!
    // Check ASCII directly for exact ^[A-Z_][A-Z0-9_]*$ behaviour without regex overhead.
    let re = Regex::new(r"^[A-Z_][A-Z0-9_]*$").expect("valid secret name regex");
    re.is_match(name)
}

/// Parses a comma-separated list of secret names, keeping valid unique names in order.
pub fn parse_env_secret_names(raw_csv: &str) -> Vec<String> {
    let mut names = Vec::new();
    for part in raw_csv.split(',') {
        if part.is_empty() {
            continue;
        }
        let clean = part.trim();
        if is_valid_secret_name(clean) && !names.iter().any(|n| n == clean) {
            names.push(clean.to_string());
        }
    }
    names
}

/// Loads `(name, value)` pairs from `file_path`, refusing group/world-accessible files.
pub fn load_secrets_store(file_path: &str) -> Result<Vec<(String, String)>, String> {
    if file_path.trim().is_empty() {
        return Ok(Vec::new());
    }

    let meta = match fs::metadata(file_path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => {
            return Err(format!(
                "Could not inspect secrets file permissions: {file_path}"
            ))
        }
    };

    let mode = meta.permissions().mode();
    if (mode & 0o077) != 0 {
        return Err(format!(
            "Refusing to read {file_path} with insecure permissions ({:o}). Run: chmod 600 {file_path}",
            mode & 0o777
        ));
    }

    let bytes =
        fs::read(file_path).map_err(|_| format!("Could not open secrets file: {file_path}"))?;
    let doc: Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("Invalid JSON in secrets file: {e}"))?;

    let arr = if let Some(obj) = doc.as_object() {
        obj.get("secrets").and_then(|v| v.as_array())
    } else {
        doc.as_array()
    };

    let Some(arr) = arr else {
        return Ok(Vec::new());
    };

    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for val in arr {
        let Some(obj) = val.as_object() else {
            continue;
        };
        let name = obj
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let secret_val = obj
            .get("value")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if !is_valid_secret_name(&name) || !seen.insert(name.clone()) {
            continue;
        }
        result.push((name, secret_val));
    }
    Ok(result)
}

/// Reads a named array of valid unique secret names (e.g. `"disabled"` or `"hiddenInInstances"`).
pub fn load_secret_name_list(file_path: &str, key: &str) -> Vec<String> {
    if key.trim().is_empty() {
        return Vec::new();
    }
    let Ok(bytes) = fs::read(file_path) else {
        return Vec::new();
    };
    let Ok(doc) = serde_json::from_slice::<Value>(&bytes) else {
        return Vec::new();
    };
    let Some(obj) = doc.as_object() else {
        return Vec::new();
    };
    let Some(arr) = obj.get(key).and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    let mut names = Vec::new();
    for val in arr {
        let Some(name) = val.as_str() else {
            continue;
        };
        if is_valid_secret_name(name) && !names.iter().any(|n| n == name) {
            names.push(name.to_string());
        }
    }
    names
}

/// Atomically writes `secrets` (plus optional `disabled` and `hiddenInInstances` lists)
/// to `file_path` with directory mode 0700 and file mode 0600.
pub fn save_secrets_store(
    file_path: &str,
    secrets: &[(String, String)],
    disabled_names: &[String],
    hidden_in_instances: &[String],
) -> Result<(), String> {
    let clean = clean_path(file_path.trim());
    if clean.is_empty() {
        return Err("Invalid secrets store path.".to_string());
    }

    let mut arr = Vec::new();
    let mut seen = HashSet::new();
    for (name, value) in secrets {
        if !is_valid_secret_name(name) {
            return Err(format!(
                "Invalid secret name '{name}'. Use ^[A-Z_][A-Z0-9_]*$."
            ));
        }
        if !seen.insert(name.clone()) {
            continue;
        }
        arr.push(json!({
            "name": name,
            "value": value,
        }));
    }

    let abs_path = if Path::new(&clean).is_absolute() {
        PathBuf::from(&clean)
    } else {
        env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("/"))
            .join(&clean)
    };
    let dir_path = abs_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("/"));
    let dir_str = dir_path.to_string_lossy();

    fs::create_dir_all(&dir_path)
        .map_err(|_| format!("Could not create secrets directory: {dir_str}"))?;
    fs::set_permissions(&dir_path, fs::Permissions::from_mode(0o700))
        .map_err(|_| format!("Could not set 0700 permissions on directory: {dir_str}"))?;

    let mut root = BTreeMap::new();
    root.insert("secrets", Value::Array(arr));

    let disabled_arr: Vec<Value> = {
        let mut out = Vec::new();
        let mut d_seen = HashSet::new();
        for name in disabled_names {
            if is_valid_secret_name(name) && d_seen.insert(name.clone()) {
                out.push(Value::String(name.clone()));
            }
        }
        out
    };
    if !disabled_arr.is_empty() {
        root.insert("disabled", Value::Array(disabled_arr));
    }

    let hidden_arr: Vec<Value> = {
        let mut out = Vec::new();
        let mut h_seen = HashSet::new();
        for name in hidden_in_instances {
            if is_valid_secret_name(name) && h_seen.insert(name.clone()) {
                out.push(Value::String(name.clone()));
            }
        }
        out
    };
    if !hidden_arr.is_empty() {
        root.insert("hiddenInInstances", Value::Array(hidden_arr));
    }

    let payload =
        to_json_indented(&root).map_err(|e| format!("Could not serialize secrets store: {e}"))?;

    let tmp_path = format!("{clean}.tmp.{}", unique_id128());
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp_path)
        .map_err(|_| format!("Could not create temporary secrets file: {tmp_path}"))?;

    let _ = file.set_permissions(fs::Permissions::from_mode(0o600));
    if file.write_all(payload.as_bytes()).is_err() {
        drop(file);
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("Could not write secrets file: {clean}"));
    }
    drop(file);

    if fs::rename(&tmp_path, &clean).is_err() {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("Could not replace secrets file: {clean}"));
    }
    fs::set_permissions(&clean, fs::Permissions::from_mode(0o600))
        .map_err(|_| format!("Could not set 0600 permissions on secrets file: {clean}"))?;

    Ok(())
}
