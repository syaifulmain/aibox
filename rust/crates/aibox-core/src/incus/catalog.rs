use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::defaults::{default_os_entries, OsEntry};
use crate::util::format::format_bytes;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OsCatalogEntry {
    pub id: String,
    pub alias: String,
    pub name: String,
    pub badge: String,
    pub description: String,
    pub downloaded: bool,
    pub downloading: bool,
    pub size_text: String,
}

fn json_i64(v: Option<&Value>) -> i64 {
    let Some(val) = v else {
        return 0;
    };
    if let Some(n) = val.as_i64() {
        n
    } else if let Some(u) = val.as_u64() {
        u as i64
    } else if let Some(f) = val.as_f64() {
        f as i64
    } else {
        0
    }
}

pub fn catalog_for_entries(
    entries: &[OsEntry],
    images_json: &[u8],
    downloading_id: &str,
    storage_path: &str,
) -> Vec<OsCatalogEntry> {
    let mut cached_aliases: HashMap<String, i64> = HashMap::new();
    if let Ok(Value::Array(arr)) = serde_json::from_slice::<Value>(images_json) {
        for val in &arr {
            let Some(img) = val.as_object() else {
                continue;
            };
            let size = json_i64(img.get("size"));
            if let Some(aliases) = img.get("aliases").and_then(Value::as_array) {
                for alias_val in aliases {
                    if let Some(alias_name) = alias_val
                        .as_object()
                        .and_then(|o| o.get("name"))
                        .and_then(Value::as_str)
                    {
                        if !alias_name.is_empty() {
                            cached_aliases.insert(alias_name.to_string(), size);
                        }
                    }
                }
            }
            if let Some(props) = img.get("properties").and_then(Value::as_object) {
                let os = props
                    .get("os")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_lowercase();
                let rel = props
                    .get("release")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_lowercase();
                if !os.is_empty() {
                    cached_aliases.insert(format!("{os}/{rel}"), size);
                }
            }
        }
    }

    let mut list = Vec::with_capacity(entries.len());
    for entry in entries {
        if entry.id.is_empty() {
            continue;
        }
        let local_alias = format!("aibox-{}", entry.id);
        let in_incus = cached_aliases.contains_key(&local_alias);
        let mut size_bytes = cached_aliases.get(&local_alias).copied().unwrap_or(0);

        let mut saved_on_disk = false;
        if !storage_path.is_empty() {
            let meta_path = Path::new(storage_path).join(&entry.id);
            let rootfs_path = Path::new(storage_path).join(format!("{}.root", entry.id));
            if let (Ok(m1), Ok(m2)) = (fs::metadata(&meta_path), fs::metadata(&rootfs_path)) {
                saved_on_disk = m1.is_file() && m1.len() > 0 && m2.is_file() && m2.len() > 0;
                if !in_incus && saved_on_disk {
                    size_bytes = (m1.len() + m2.len()) as i64;
                }
            }
        }

        let downloaded = in_incus || saved_on_disk;
        let cache_text = if in_incus {
            format!("Cached \u{00b7} {}", format_bytes(size_bytes))
        } else if saved_on_disk {
            format!("Saved \u{00b7} {}", format_bytes(size_bytes))
        } else {
            "Not Cached".to_string()
        };

        list.push(OsCatalogEntry {
            id: entry.id.clone(),
            alias: entry.alias.clone(),
            name: entry.name.clone(),
            badge: entry.badge.clone(),
            description: entry.description.clone(),
            downloaded,
            downloading: entry.id == downloading_id,
            size_text: cache_text,
        });
    }

    list
}

pub fn build_os_catalog(images_json: &[u8], downloading_id: &str) -> Vec<OsCatalogEntry> {
    catalog_for_entries(&default_os_entries(), images_json, downloading_id, "")
}

pub fn catalog_for_entries_json(
    entries: &[OsEntry],
    images_json: &[u8],
    downloading_id: &str,
    storage_path: &str,
) -> String {
    let list = catalog_for_entries(entries, images_json, downloading_id, storage_path);
    serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string())
}

pub fn build_os_catalog_json(images_json: &[u8], downloading_id: &str) -> String {
    let list = build_os_catalog(images_json, downloading_id);
    serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn os_catalog_marks_downloaded_images_match_qt_test() {
        let images_json = br#"[
            {
                "size": 146800640,
                "aliases": [ { "name": "aibox-ubuntu-24.04" } ]
            }
        ]"#;
        let catalog = build_os_catalog(images_json, "");
        assert!(catalog.len() >= 6);
        let first = &catalog[0];
        assert_eq!(first.id, "ubuntu-24.04");
        assert!(first.downloaded);
    }

    #[test]
    fn os_storage_cache_marks_entry_downloaded_match_qt_test() {
        let dir = unique_temp_dir("aibox-os-cache-test");
        fs::write(dir.join("cached-os"), b"metadata").expect("write metadata");
        fs::write(dir.join("cached-os.root"), b"rootfs").expect("write rootfs");

        let entries = vec![OsEntry {
            id: "cached-os".to_string(),
            alias: "cached/os".to_string(),
            name: "Cached OS".to_string(),
            badge: "Test".to_string(),
            description: String::new(),
        }];
        let catalog = catalog_for_entries(&entries, b"[]", "", &dir.to_string_lossy());
        assert_eq!(catalog.len(), 1);
        assert!(catalog[0].downloaded);
        assert!(catalog[0].size_text.starts_with("Saved"));

        let _ = fs::remove_dir_all(&dir);
    }
}
