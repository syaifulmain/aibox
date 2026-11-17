use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use chrono::Local;

pub fn paste_dir_path(workspace_path: &str) -> String {
    Path::new(workspace_path)
        .join(".aibox-paste")
        .to_string_lossy()
        .into_owned()
}

pub fn prepare_pasted_image_path(workspace_path: &str) -> Result<String, String> {
    let paste_dir = paste_dir_path(workspace_path);
    fs::create_dir_all(&paste_dir)
        .map_err(|_| format!("Could not create the paste folder '{paste_dir}'."))?;

    let stamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let dir_path = Path::new(&paste_dir);
    let mut candidate = dir_path.join(format!("paste-{stamp}.png"));
    let mut suffix = 2u32;
    while candidate.exists() {
        candidate = dir_path.join(format!("paste-{stamp}-{suffix}.png"));
        suffix += 1;
    }
    Ok(candidate.to_string_lossy().into_owned())
}

pub fn prune_stale_pasted_images(workspace_path: &str, max_age_days: i64) {
    let paste_dir = paste_dir_path(workspace_path);
    let Ok(entries) = fs::read_dir(&paste_dir) else {
        return;
    };
    let max_secs = max_age_days.max(0) as u64 * 86_400;
    let Some(cutoff) = SystemTime::now().checked_sub(Duration::from_secs(max_secs)) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !name.starts_with("paste-") || !name.ends_with(".png") {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            if let Ok(modified) = meta.modified() {
                if modified < cutoff {
                    let _ = fs::remove_file(&path);
                }
            }
        }
    }
}

pub fn pasted_image_container_path(container_workspace_path: &str, host_path: &str) -> String {
    let root = container_workspace_path.trim_end_matches('/');
    let file_name = Path::new(host_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    format!("{root}/.aibox-paste/{file_name}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::diff::default_diff_excludes;

    #[test]
    fn pasted_images_stored_in_paste_dir_and_excluded_from_diff() {
        let base = format!("/tmp/aibox-rust-test-paste-{}", std::process::id());
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();

        let p1 = prepare_pasted_image_path(&base).unwrap();
        assert_eq!(
            Path::new(&p1).parent().unwrap(),
            Path::new(&base).join(".aibox-paste")
        );
        fs::write(&p1, b"png1").unwrap();

        let p2 = prepare_pasted_image_path(&base).unwrap();
        assert_ne!(p1, p2);
        fs::write(&p2, b"png2").unwrap();

        assert_eq!(
            pasted_image_container_path("/home/workspace/", &p1),
            format!(
                "/home/workspace/.aibox-paste/{}",
                Path::new(&p1).file_name().unwrap().to_str().unwrap()
            )
        );
        assert!(default_diff_excludes().contains(&".aibox-paste/".to_string()));

        let _ = fs::remove_dir_all(&base);
    }
}
