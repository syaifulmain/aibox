//! Secret name validation and 0600-enforced JSON store IO.

use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::secrets::error::SecretsError;
use crate::secrets::{zeroize_bytes, zeroize_string};
use crate::util::error::IoError;
use crate::util::json::to_json_indented;
use crate::util::paths::{atomic_write_with_mode, clean_path, xdg_config_dir};

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
    if let Ok(override_path) = std::env::var("AIBOX_SECRETS_PATH") {
        let trimmed = override_path.trim();
        if !trimmed.is_empty() {
            return clean_path(trimmed);
        }
    }
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
    let bytes = name.as_bytes();
    let Some(&first) = bytes.first() else {
        return false;
    };
    if !(first.is_ascii_uppercase() || first == b'_') {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|&b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
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

#[derive(Debug, Clone, Default)]
pub(crate) struct SecretsDocument {
    pub pairs: Vec<(String, String)>,
    pub disabled: Vec<String>,
    pub hidden_in_instances: Vec<String>,
}

fn extract_name_list(doc: &Value, key: &str) -> Vec<String> {
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

fn read_and_parse_secrets_file(
    path: &Path,
    mut file: File,
) -> Result<SecretsDocument, SecretsError> {
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|e| IoError::new("read", path, e))?;
    let parse_res: Result<Value, _> = serde_json::from_slice(&bytes);
    zeroize_bytes(&mut bytes);
    let doc = parse_res.map_err(|source| SecretsError::Parse {
        path: path.to_path_buf(),
        source,
    })?;

    let disabled = extract_name_list(&doc, "disabled");
    let hidden_in_instances = extract_name_list(&doc, "hiddenInInstances");

    let arr = if let Some(obj) = doc.as_object() {
        obj.get("secrets").and_then(|v| v.as_array())
    } else {
        doc.as_array()
    };

    let mut pairs = Vec::new();
    if let Some(arr) = arr {
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
            pairs.push((name, secret_val));
        }
    }

    Ok(SecretsDocument {
        pairs,
        disabled,
        hidden_in_instances,
    })
}

/// Single-pass loader returning pairs and name lists while enforcing `0600` permissions via `fstat`.
pub(crate) fn load_secrets_document(file_path: &str) -> Result<SecretsDocument, SecretsError> {
    if file_path.trim().is_empty() {
        return Ok(SecretsDocument::default());
    }

    let path = Path::new(file_path);
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(SecretsDocument::default());
        }
        Err(e) => return Err(IoError::new("open", path, e).into()),
    };

    let meta = file
        .metadata()
        .map_err(|e| IoError::new("inspect permissions on", path, e))?;
    let mode = meta.permissions().mode();
    if (mode & 0o077) != 0 {
        return Err(SecretsError::InsecurePermissions {
            path: path.to_path_buf(),
            mode: mode & 0o777,
        });
    }

    read_and_parse_secrets_file(path, file)
}

/// Reads an existing secrets document when recovering from `InsecurePermissions` during `save_secret`.
pub(crate) fn load_secrets_document_for_permission_fix(
    file_path: &str,
) -> Result<SecretsDocument, SecretsError> {
    if file_path.trim().is_empty() {
        return Ok(SecretsDocument::default());
    }
    let path = Path::new(file_path);
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(SecretsDocument::default());
        }
        Err(e) => return Err(IoError::new("open", path, e).into()),
    };
    read_and_parse_secrets_file(path, file)
}

/// Loads `(name, value)` pairs from `file_path`, refusing group/world-accessible files.
pub fn load_secrets_store(file_path: &str) -> Result<Vec<(String, String)>, SecretsError> {
    load_secrets_document(file_path).map(|doc| doc.pairs)
}

/// Reads a named array of valid unique secret names (e.g. `"disabled"` or `"hiddenInInstances"`).
pub fn load_secret_name_list(file_path: &str, key: &str) -> Vec<String> {
    if key.trim().is_empty() || file_path.trim().is_empty() {
        return Vec::new();
    }
    let Ok(mut bytes) = fs::read(file_path) else {
        return Vec::new();
    };
    let parse_res = serde_json::from_slice::<Value>(&bytes);
    zeroize_bytes(&mut bytes);
    let Ok(doc) = parse_res else {
        return Vec::new();
    };
    extract_name_list(&doc, key)
}

/// Atomically writes `secrets` (plus optional `disabled` and `hiddenInInstances` lists)
/// to `file_path` with directory mode 0700 and file mode 0600.
pub fn save_secrets_store(
    file_path: &str,
    secrets: &[(String, String)],
    disabled_names: &[String],
    hidden_in_instances: &[String],
) -> Result<(), SecretsError> {
    let clean = clean_path(file_path.trim());
    if clean.is_empty() {
        return Err(SecretsError::InvalidPath);
    }

    let mut arr = Vec::new();
    let mut seen = HashSet::new();
    for (name, value) in secrets {
        if !is_valid_secret_name(name) {
            return Err(SecretsError::InvalidSecretName {
                name: name.to_string(),
            });
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

    fs::create_dir_all(&dir_path)
        .map_err(|e| IoError::new("create secrets directory", &dir_path, e))?;
    fs::set_permissions(&dir_path, fs::Permissions::from_mode(0o700))
        .map_err(|e| IoError::new("set 0700 permissions on", &dir_path, e))?;

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

    let mut payload = to_json_indented(&root).map_err(SecretsError::Serialize)?;
    let write_res = atomic_write_with_mode(Path::new(&clean), payload.as_bytes(), Some(0o600));
    zeroize_string(&mut payload);
    write_res?;

    Ok(())
}
