//! Incus storage pool parsing, overcommit math, and resize helpers.

use crate::util::format::format_bytes;
use crate::util::shell::shell_quote;
use crate::util::size::parse_size_bytes;
use regex::Regex;
use serde_json::{Map, Value};
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoragePoolInfo {
    pub valid: bool,
    pub name: String,
    pub driver: String,
    pub used_bytes: i64,
    pub total_bytes: i64,
    pub used_percent: i32,
    pub size_text: String,
    pub resizable: bool,
}

impl Default for StoragePoolInfo {
    fn default() -> Self {
        Self {
            valid: false,
            name: "default".to_string(),
            driver: String::new(),
            used_bytes: 0,
            total_bytes: 0,
            used_percent: 0,
            size_text: String::new(),
            resizable: false,
        }
    }
}

impl StoragePoolInfo {
    /// Serializes pool metrics into the JSON `Value` expected by `poolInfoToVariantMap`.
    pub fn to_value(&self, allocated_bytes: i64, unbounded_count: i32) -> Value {
        let mut map = Map::new();
        map.insert("valid".to_string(), Value::Bool(self.valid));
        map.insert("name".to_string(), Value::String(self.name.clone()));
        map.insert("driver".to_string(), Value::String(self.driver.clone()));
        map.insert(
            "usedBytes".to_string(),
            Value::Number(self.used_bytes.into()),
        );
        map.insert(
            "totalBytes".to_string(),
            Value::Number(self.total_bytes.into()),
        );
        map.insert(
            "usedText".to_string(),
            Value::String(if self.valid {
                format_bytes(self.used_bytes)
            } else {
                String::new()
            }),
        );
        map.insert(
            "totalText".to_string(),
            Value::String(if self.valid {
                format_bytes(self.total_bytes)
            } else {
                String::new()
            }),
        );
        map.insert(
            "usedPercent".to_string(),
            Value::Number(self.used_percent.into()),
        );
        map.insert(
            "allocatedBytes".to_string(),
            Value::Number(allocated_bytes.into()),
        );
        map.insert(
            "allocatedText".to_string(),
            Value::String(format_bytes(allocated_bytes)),
        );
        map.insert(
            "unboundedCount".to_string(),
            Value::Number(unbounded_count.into()),
        );
        map.insert(
            "sizeText".to_string(),
            Value::String(self.size_text.clone()),
        );
        map.insert("resizable".to_string(), Value::Bool(self.resizable));
        map.insert(
            "overcommitted".to_string(),
            Value::Bool(self.valid && allocated_bytes > self.total_bytes),
        );
        let alert_level = if !self.valid {
            0
        } else if self.used_percent > 95 {
            2
        } else if self.used_percent > 85 || allocated_bytes > self.total_bytes {
            1
        } else {
            0
        };
        map.insert("alertLevel".to_string(), Value::Number(alert_level.into()));
        Value::Object(map)
    }

    /// Serializes pool metrics into the JSON shape expected by `poolInfoToVariantMap`.
    pub fn to_json(&self, allocated_bytes: i64, unbounded_count: i32) -> String {
        self.to_value(allocated_bytes, unbounded_count).to_string()
    }
}

fn parse_storage_metric_bytes(raw_val: &str) -> i64 {
    let mut clean = raw_val.trim();
    if clean.len() >= 2
        && ((clean.starts_with('"') && clean.ends_with('"'))
            || (clean.starts_with('\'') && clean.ends_with('\'')))
    {
        clean = clean[1..clean.len() - 1].trim();
    }
    if clean.is_empty() {
        return 0;
    }
    if let Ok(exact_bytes) = clean.parse::<i64>() {
        return exact_bytes.max(0);
    }
    parse_size_bytes(clean)
}

fn json_metric_bytes(val: Option<&Value>) -> i64 {
    match val {
        Some(Value::String(s)) => parse_storage_metric_bytes(s),
        Some(Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                i.max(0)
            } else if let Some(u) = n.as_u64() {
                i64::try_from(u).unwrap_or(i64::MAX)
            } else if let Some(f) = n.as_f64() {
                (f as i64).max(0)
            } else {
                0
            }
        }
        _ => 0,
    }
}

fn finalize_used_percent(info: &mut StoragePoolInfo) {
    if info.total_bytes > 0 {
        let pct = ((info.used_bytes as f64) * 100.0) / (info.total_bytes as f64);
        info.used_percent = (pct.round() as i32).clamp(0, 100);
        info.valid = true;
    }
}

/// Parses `incus storage info <pool>` YAML or JSON output.
pub fn parse_incus_storage_info(output: &str) -> StoragePoolInfo {
    let mut info = StoragePoolInfo::default();
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return info;
    }

    if trimmed.starts_with('{') {
        if let Ok(Value::Object(mut obj)) = serde_json::from_str::<Value>(trimmed) {
            if let Some(Value::Object(inner)) = obj.get("info") {
                obj = inner.clone();
            }
            if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
                if !name.is_empty() {
                    info.name = name.to_string();
                }
            }
            if let Some(driver) = obj.get("driver").and_then(|v| v.as_str()) {
                info.driver = driver.to_string();
            }
            let used_val = obj.get("space used").or_else(|| obj.get("used"));
            let total_val = obj.get("total space").or_else(|| obj.get("total"));
            info.used_bytes = json_metric_bytes(used_val);
            info.total_bytes = json_metric_bytes(total_val);
            finalize_used_percent(&mut info);
            return info;
        }
    }

    let mut in_used_by = false;
    for raw_line in trimmed.split('\n') {
        if !raw_line.starts_with(' ') && !raw_line.starts_with('\t') {
            let top_key = raw_line.trim();
            if top_key.starts_with("used by:") {
                in_used_by = true;
                continue;
            }
            if top_key.starts_with("info:") {
                in_used_by = false;
                continue;
            }
        }
        if in_used_by {
            continue;
        }
        let Some(colon) = raw_line.find(':') else {
            continue;
        };
        if colon == 0 {
            continue;
        }
        let key = raw_line[..colon].trim().to_lowercase();
        let val = raw_line[colon + 1..].trim();
        match key.as_str() {
            "name" if !val.is_empty() => info.name = val.to_string(),
            "driver" => info.driver = val.to_string(),
            "space used" => info.used_bytes = parse_storage_metric_bytes(val),
            "total space" => info.total_bytes = parse_storage_metric_bytes(val),
            _ => {}
        }
    }

    finalize_used_percent(&mut info);
    info
}

/// Sums root disk quotas in bytes across already-parsed `Environment` structs without
/// re-parsing JSON, returning `(total_bytes, unbounded_count)`.
pub fn sum_env_root_disk_quotas(
    envs: &[crate::incus::list::Environment],
    exclude_env_name: &str,
    pool_name: &str,
) -> (i64, i32) {
    let mut total: i64 = 0;
    let mut unbounded: i32 = 0;

    for env in envs {
        if env.name.is_empty() || (!exclude_env_name.is_empty() && env.name == exclude_env_name) {
            continue;
        }
        if !pool_name.is_empty() && !env.root_pool.is_empty() && env.root_pool != pool_name {
            continue;
        }
        if env.disk_quota_bytes > 0 {
            total += env.disk_quota_bytes;
        } else {
            unbounded += 1;
        }
    }

    (total, unbounded)
}

/// Sums root disk quotas in bytes from `incus list --format=json`, returning `(total_bytes, unbounded_count)`.
pub fn sum_root_disk_quotas_bytes(
    incus_list_json: &str,
    exclude_env_name: &str,
    pool_name: &str,
) -> (i64, i32) {
    let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(incus_list_json) else {
        return (0, 0);
    };

    let mut total: i64 = 0;
    let mut unbounded: i32 = 0;

    for val in arr {
        let Some(obj) = val.as_object() else {
            continue;
        };
        let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.is_empty() || (!exclude_env_name.is_empty() && name == exclude_env_name) {
            continue;
        }

        let expanded = obj.get("expanded_devices").and_then(|v| v.as_object());
        let direct = obj.get("devices").and_then(|v| v.as_object());

        let expanded_root = expanded
            .and_then(|m| m.get("root"))
            .and_then(|v| v.as_object())
            .filter(|m| !m.is_empty());
        let direct_root = direct
            .and_then(|m| m.get("root"))
            .and_then(|v| v.as_object());
        let root_dev = expanded_root.or(direct_root);

        let root_pool = root_dev
            .and_then(|r| r.get("pool"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !pool_name.is_empty() && !root_pool.is_empty() && root_pool != pool_name {
            continue;
        }

        let mut size_str = root_dev
            .and_then(|r| r.get("size"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .unwrap_or("");
        if size_str.is_empty() {
            size_str = direct_root
                .and_then(|r| r.get("size"))
                .and_then(|v| v.as_str())
                .map(str::trim)
                .unwrap_or("");
        }

        let quota = parse_size_bytes(size_str);
        if quota > 0 {
            total += quota;
        } else {
            unbounded += 1;
        }
    }

    (total, unbounded)
}

/// Evaluates whether a storage pool is near capacity (>85%) or overcommitted by quotas.
pub fn evaluate_pool_overcommit_warning(
    pool: &StoragePoolInfo,
    existing_quotas_bytes: i64,
    requested_bytes: i64,
    unbounded_count: i32,
) -> String {
    if !pool.valid || pool.total_bytes <= 0 {
        return String::new();
    }

    let req = requested_bytes.max(0);
    let projected = existing_quotas_bytes + req;
    let overcommit = projected > pool.total_bytes;
    let high_pool = pool.used_percent > 85;
    if !overcommit && !high_pool {
        return String::new();
    }

    let mut parts = Vec::new();
    if high_pool {
        parts.push(format!(
            "Pool '{}' is {}% full ({} / {}).",
            pool.name,
            pool.used_percent,
            format_bytes(pool.used_bytes),
            format_bytes(pool.total_bytes)
        ));
    }
    if overcommit {
        parts.push(format!(
            "Total allocated quotas ({}) will exceed pool capacity ({}).",
            format_bytes(projected),
            format_bytes(pool.total_bytes)
        ));
    }
    if unbounded_count > 0 {
        parts.push(format!(
            "{unbounded_count} environment(s) have no disk quota."
        ));
    }
    parts.join(" ")
}

/// Returns an overcommit/capacity warning for creating a new environment with `disk_spec`.
pub fn pool_warning_for_create(
    pool: &StoragePoolInfo,
    allocated_bytes: i64,
    unbounded_count: i32,
    disk_spec: &str,
) -> String {
    if !pool.valid {
        return String::new();
    }
    let req_bytes = parse_size_bytes(disk_spec);
    evaluate_pool_overcommit_warning(pool, allocated_bytes, req_bytes, unbounded_count)
}

/// Returns an overcommit/capacity warning when editing `env_name`'s disk quota to `disk_spec`.
pub fn pool_warning_for_quota_edit(
    pool: &StoragePoolInfo,
    incus_list_json: &str,
    fallback_env_quotas: &[(String, i64)],
    env_name: &str,
    disk_spec: &str,
) -> String {
    if !pool.valid {
        return String::new();
    }
    let new_bytes = parse_size_bytes(disk_spec);
    if !incus_list_json.is_empty() {
        let (other_allocated, unbounded) =
            sum_root_disk_quotas_bytes(incus_list_json, env_name, &pool.name);
        return evaluate_pool_overcommit_warning(pool, other_allocated, new_bytes, unbounded);
    }

    let mut other_allocated: i64 = 0;
    let mut unbounded: i32 = 0;
    for (name, quota) in fallback_env_quotas {
        if name == env_name {
            continue;
        }
        if *quota > 0 {
            other_allocated += *quota;
        } else {
            unbounded += 1;
        }
    }
    evaluate_pool_overcommit_warning(pool, other_allocated, new_bytes, unbounded)
}

/// Generates the shell script to grow an Incus storage pool.
pub fn resize_storage_pool_script(pool_name: &str, size_spec: &str) -> String {
    let trimmed_size = size_spec.trim();
    let q_pool = shell_quote(pool_name);
    let q_size = shell_quote(trimmed_size);
    let q_stage20 = shell_quote(&format!(
        "@@STAGE:20:Growing storage pool '{pool_name}' to '{trimmed_size}'..."
    ));
    let q_stage100 = shell_quote(&format!(
        "@@STAGE:100:Storage pool grown to '{trimmed_size}'."
    ));
    format!(
        "set -euo pipefail\n\
         echo {q_stage20}\n\
         incus storage set {q_pool} size={q_size}\n\
         echo '@@STAGE:80:Verifying new pool capacity...'\n\
         incus storage info {q_pool} --bytes 2>/dev/null || incus storage info {q_pool}\n\
         echo {q_stage100}"
    )
}

/// Parses a grow-only storage pool size spec (`"60"`, `"60GiB"`, `"1.5TiB"`, etc.) into bytes.
pub fn parse_pool_size_spec(spec: &str) -> Option<i64> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)^\s*(\d+(?:[\.,]\d+)?)\s*(TiB|TB|GiB|GB|MiB|MB)?\s*$")
            .expect("valid pool size regex")
    });
    let caps = re.captures(spec)?;
    let number = caps.get(1)?.as_str().replace(',', ".");
    let value: f64 = number.parse().ok()?;
    if value <= 0.0 {
        return None;
    }

    // Incus treats bare storage sizes as GiB, so "60" means 60GiB here too.
    let unit = caps
        .get(2)
        .map(|m| m.as_str().to_uppercase())
        .unwrap_or_default();
    let scale = match unit.as_str() {
        "TIB" | "TB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        "MIB" | "MB" => 1024.0 * 1024.0,
        _ => 1024.0 * 1024.0 * 1024.0,
    };

    let bytes = value * scale;
    if !(1.0..9.0e18).contains(&bytes) {
        return None;
    }
    Some(bytes.round() as i64)
}

/// Validates a storage pool resize request; returns an empty string when valid.
pub fn validate_storage_pool_resize(
    busy: bool,
    task_lock_active: bool,
    pool: &StoragePoolInfo,
    size_spec: &str,
    available_bytes: i64,
) -> String {
    if busy {
        return "Wait for the current AIBox task to finish before growing the storage pool."
            .to_string();
    }
    if task_lock_active {
        return "Another AIBox task is currently running.".to_string();
    }
    if !pool.valid || pool.total_bytes <= 0 {
        return "The Incus storage pool is not available yet. Refresh and try again.".to_string();
    }
    if !pool.resizable {
        return format!(
            "Pool '{}' is not loop-file backed, so its capacity follows the host filesystem and cannot be grown from AIBox.",
            pool.name
        );
    }

    let Some(target_bytes) = parse_pool_size_spec(size_spec) else {
        return "Enter a pool size such as 60GiB (plain numbers are treated as GiB).".to_string();
    };

    if target_bytes <= pool.total_bytes {
        return format!(
            "The new size ({}) must be larger than the current pool size ({}); Incus cannot shrink a pool.",
            format_bytes(target_bytes),
            format_bytes(pool.total_bytes)
        );
    }

    let delta = target_bytes - pool.total_bytes;
    if available_bytes > 0 && delta > available_bytes {
        return format!(
            "Growing the pool needs {} more space, but only {} is free on the host filesystem.",
            format_bytes(delta),
            format_bytes(available_bytes)
        );
    }

    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: i64 = 1024 * 1024 * 1024;

    #[test]
    fn pool_storage_info_parsing_real_fixtures() {
        let bytes_yaml = "\
info:
  description: Btrfs Storage Pool with disk quotas and instant snapshots
  driver: btrfs
  name: default
  space used: \"1329242112\"
  total space: \"32212254720\"
used by:
  instances:
    - mobile-porting
  profiles:
    - default
";
        let bytes_info = parse_incus_storage_info(bytes_yaml);
        assert!(bytes_info.valid);
        assert_eq!(bytes_info.name, "default");
        assert_eq!(bytes_info.driver, "btrfs");
        assert_eq!(bytes_info.used_bytes, 1_329_242_112);
        assert_eq!(bytes_info.total_bytes, 32_212_254_720);
        assert_eq!(bytes_info.used_percent, 4);

        let human_yaml = "\
info:
  driver: btrfs
  name: default
  space used: 1.24GiB
  total space: 30.00GiB
";
        let human_info = parse_incus_storage_info(human_yaml);
        assert!(human_info.valid);
        assert_eq!(human_info.total_bytes, 30 * GIB);
        assert_eq!(human_info.used_percent, 4);

        let json_out = r#"{"info":{"name":"default","driver":"btrfs","space used":26843545600,"total space":32212254720}}"#;
        let json_info = parse_incus_storage_info(json_out);
        assert!(json_info.valid);
        assert_eq!(json_info.used_percent, 83);

        assert!(!parse_incus_storage_info("invalid output").valid);
    }

    #[test]
    fn pool_overcommit_math_and_unbounded_quotas() {
        let list_json = r#"[
            {"name":"env-a","devices":{},"expanded_devices":{"root":{"path":"/","pool":"default","size":"20GiB"}}},
            {"name":"env-b","devices":{"root":{"size":"15GiB"}},"expanded_devices":{"root":{"path":"/","pool":"default","size":"15GiB"}}},
            {"name":"env-noquota","devices":{},"expanded_devices":{"root":{"path":"/","pool":"default"}}},
            {"name":"env-otherpool","devices":{},"expanded_devices":{"root":{"path":"/","pool":"ssd2","size":"100GiB"}}}
        ]"#;

        let (sum_all, unbounded) = sum_root_disk_quotas_bytes(list_json, "", "default");
        assert_eq!(sum_all, 35 * GIB);
        assert_eq!(unbounded, 1);

        let (sum_excl_a, unbounded_excl) =
            sum_root_disk_quotas_bytes(list_json, "env-a", "default");
        assert_eq!(sum_excl_a, 15 * GIB);
        assert_eq!(unbounded_excl, 1);

        let mut pool = StoragePoolInfo {
            valid: true,
            name: "default".to_string(),
            used_bytes: 10 * GIB,
            total_bytes: 40 * GIB,
            used_percent: 25,
            ..Default::default()
        };

        assert_eq!(
            evaluate_pool_overcommit_warning(&pool, sum_all, 4 * GIB, unbounded),
            ""
        );

        let over_warn = evaluate_pool_overcommit_warning(&pool, sum_all, 10 * GIB, unbounded);
        assert!(over_warn.contains("45.0 GiB"));
        assert!(over_warn.contains("40.0 GiB"));
        assert!(over_warn.contains("1 environment(s)"));

        pool.used_percent = 88;
        pool.used_bytes = 35 * GIB;
        assert!(!evaluate_pool_overcommit_warning(&pool, 0, GIB, 0).is_empty());

        pool.used_percent = 25;
        pool.used_bytes = 10 * GIB;
        assert_eq!(
            pool_warning_for_create(&pool, sum_all, unbounded, "4GiB"),
            ""
        );
        assert!(!pool_warning_for_create(&pool, sum_all, unbounded, "10GiB").is_empty());
        assert_eq!(
            pool_warning_for_quota_edit(&pool, list_json, &[], "env-a", "24GiB"),
            ""
        );
        assert!(!pool_warning_for_quota_edit(&pool, list_json, &[], "env-a", "30GiB").is_empty());
    }

    #[test]
    fn pool_resize_spec_validation_and_script() {
        assert_eq!(parse_pool_size_spec("60"), Some(60 * GIB));
        assert_eq!(parse_pool_size_spec(" 60GiB "), Some(60 * GIB));
        assert_eq!(parse_pool_size_spec("1.5TiB"), Some(1536 * GIB));
        assert_eq!(parse_pool_size_spec("512MiB"), Some(512 * 1024 * 1024));
        assert_eq!(parse_pool_size_spec(""), None);
        assert_eq!(parse_pool_size_spec("big"), None);
        assert_eq!(parse_pool_size_spec("-60GiB"), None);
        assert_eq!(parse_pool_size_spec("0"), None);

        let pool = StoragePoolInfo {
            valid: true,
            name: "default".to_string(),
            driver: "btrfs".to_string(),
            used_bytes: 512 * 1024 * 1024,
            total_bytes: GIB,
            used_percent: 50,
            size_text: "1GiB".to_string(),
            resizable: true,
        };

        assert!(!validate_storage_pool_resize(false, false, &pool, "", 10 * GIB).is_empty());
        assert!(!validate_storage_pool_resize(false, false, &pool, "nope", 10 * GIB).is_empty());
        assert!(!validate_storage_pool_resize(false, false, &pool, "1GiB", 10 * GIB).is_empty());
        assert!(!validate_storage_pool_resize(false, false, &pool, "512MiB", 10 * GIB).is_empty());
        assert_eq!(
            validate_storage_pool_resize(false, false, &pool, "2GiB", 10 * GIB),
            ""
        );
        assert_eq!(
            validate_storage_pool_resize(false, false, &pool, "2", 10 * GIB),
            ""
        );
        assert!(
            !validate_storage_pool_resize(false, false, &pool, "2GiB", 512 * 1024 * 1024)
                .is_empty()
        );

        let dir_pool = StoragePoolInfo {
            size_text: String::new(),
            resizable: false,
            ..pool
        };
        assert!(
            !validate_storage_pool_resize(false, false, &dir_pool, "2GiB", 10 * GIB).is_empty()
        );

        let script = resize_storage_pool_script("default", "2GiB");
        assert!(script.contains("incus storage set 'default' size='2GiB'"));
        assert!(script.contains("incus storage info 'default'"));
        assert!(script.contains("@@STAGE:100"));
    }

    #[test]
    fn sum_env_root_disk_quotas_matches_json_sum_and_filters_pool() {
        let list_json = br#"[
            {"name":"env-a","type":"container","status":"Stopped","devices":{},"expanded_devices":{"root":{"path":"/","pool":"default","size":"20GiB"}}},
            {"name":"env-b","type":"container","status":"Stopped","devices":{"root":{"size":"15GiB"}},"expanded_devices":{"root":{"path":"/","pool":"default","size":"15GiB"}}},
            {"name":"env-noquota","type":"container","status":"Stopped","devices":{},"expanded_devices":{"root":{"path":"/","pool":"default"}}},
            {"name":"env-otherpool","type":"container","status":"Stopped","devices":{},"expanded_devices":{"root":{"path":"/","pool":"ssd2","size":"100GiB"}}}
        ]"#;
        let mut state = crate::incus::list::CounterState::new();
        let envs = crate::incus::list::parse(
            list_json,
            "/tmp/ws",
            &mut state,
            0,
            crate::incus::list::ParseDefaults::default(),
            false,
        );
        let json_str = std::str::from_utf8(list_json).unwrap();
        assert_eq!(
            sum_env_root_disk_quotas(&envs, "", "default"),
            sum_root_disk_quotas_bytes(json_str, "", "default")
        );
        assert_eq!(
            sum_env_root_disk_quotas(&envs, "env-a", "default"),
            sum_root_disk_quotas_bytes(json_str, "env-a", "default")
        );
    }
}
