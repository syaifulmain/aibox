//! First-run setup and storage location settings.

use std::fs;

use crate::config::error::ConfigError;
use crate::config::model::Config;
use crate::util::paths::{
    default_os_storage_path as default_os_storage_path_fn, default_system_path,
    default_workspace_path, normalized_path,
};

#[derive(Debug, Clone)]
pub struct FirstRunRequest<'a> {
    pub system_path: &'a str,
    pub current_system_path: &'a str,
    pub current_workspace_base: &'a str,
    pub current_os_storage_path: &'a str,
    pub current_state_dir: &'a str,
    pub current_config_path: &'a str,
    pub is_project_dir_mode: bool,
    pub remember_system_location: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirstRunPaths {
    pub system_path: String,
    pub workspace_base: String,
    pub os_storage_path: String,
    pub state_dir: String,
    pub config_path: String,
}

fn ensure_writable_dir(
    field: &'static str,
    path: &str,
    create_err: &str,
    write_err: &str,
) -> Result<(), ConfigError> {
    if fs::create_dir_all(path).is_err() {
        return Err(ConfigError::invalid(field, format!("{create_err}: {path}")));
    }
    match fs::metadata(path) {
        Ok(meta) if meta.is_dir() && !meta.permissions().readonly() => Ok(()),
        _ => Err(ConfigError::invalid(field, format!("{write_err}: {path}"))),
    }
}

pub fn complete_first_run(req: &FirstRunRequest<'_>) -> Result<FirstRunPaths, ConfigError> {
    let fallback = if req.current_system_path.is_empty() {
        default_system_path()
    } else {
        req.current_system_path.to_string()
    };
    let target_root = normalized_path(req.system_path, &fallback);
    ensure_writable_dir(
        "systemPath",
        &target_root,
        "Could not create the system directory",
        "The system directory is not writable",
    )?;

    let use_structured = req.is_project_dir_mode
        || req.remember_system_location
        || !req.system_path.trim().is_empty();

    let next_workspace = if use_structured {
        format!("{target_root}/workspaces")
    } else {
        req.current_workspace_base.to_string()
    };
    let next_os_storage = if use_structured {
        format!("{target_root}/os-images")
    } else {
        req.current_os_storage_path.to_string()
    };
    let next_state_dir = if use_structured {
        format!("{target_root}/state")
    } else {
        req.current_state_dir.to_string()
    };
    let next_config_path = if use_structured {
        format!("{target_root}/config.json")
    } else {
        req.current_config_path.to_string()
    };

    ensure_writable_dir(
        "workspaceBase",
        &next_workspace,
        "Could not create the workspace directory",
        "The workspace directory is not writable",
    )?;
    ensure_writable_dir(
        "osStoragePath",
        &next_os_storage,
        "Could not create the OS storage directory",
        "The OS storage directory is not writable",
    )?;
    ensure_writable_dir(
        "stateDir",
        &next_state_dir,
        "Could not create the state directory",
        "The state directory is not writable",
    )?;

    Ok(FirstRunPaths {
        system_path: target_root,
        workspace_base: next_workspace,
        os_storage_path: next_os_storage,
        state_dir: next_state_dir,
        config_path: next_config_path,
    })
}

pub fn save_settings(
    cfg: &Config,
    workspace_path: &str,
    os_storage_path: &str,
    default_workspace_base: &str,
    default_os_storage_path: &str,
) -> Result<Config, ConfigError> {
    let ws_fallback = if default_workspace_base.is_empty() {
        default_workspace_path()
    } else {
        default_workspace_base.to_string()
    };
    let os_fallback = if default_os_storage_path.is_empty() {
        default_os_storage_path_fn()
    } else {
        default_os_storage_path.to_string()
    };

    let workspace = normalized_path(workspace_path, &ws_fallback);
    let os_storage = normalized_path(os_storage_path, &os_fallback);

    ensure_writable_dir(
        "workspaceBase",
        &workspace,
        "Could not create the workspace directory",
        "The workspace directory is not writable",
    )?;
    ensure_writable_dir(
        "osStoragePath",
        &os_storage,
        "Could not create the OS storage directory",
        "The OS storage directory is not writable",
    )?;

    let mut next = cfg.clone();
    next.workspace_base = workspace;
    next.os_storage_path = os_storage;
    next.first_run_completed = true;
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("aibox-settings-loc-{label}-{nanos}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn first_run_completes_and_persists() {
        let dir = temp_dir("firstrun");
        let custom_sys = dir.join("custom-system");
        let cur_ws = dir.join("ws");
        let cur_os = dir.join("os");
        let cur_state = dir.join("state");
        let cur_cfg = dir.join("config.json");
        let req = FirstRunRequest {
            system_path: custom_sys.to_str().unwrap(),
            current_system_path: dir.to_str().unwrap(),
            current_workspace_base: cur_ws.to_str().unwrap(),
            current_os_storage_path: cur_os.to_str().unwrap(),
            current_state_dir: cur_state.to_str().unwrap(),
            current_config_path: cur_cfg.to_str().unwrap(),
            is_project_dir_mode: false,
            remember_system_location: false,
        };
        let paths = complete_first_run(&req).expect("first run");
        assert_eq!(paths.system_path, custom_sys.to_str().unwrap());
        assert_eq!(
            paths.workspace_base,
            custom_sys.join("workspaces").to_str().unwrap()
        );
        assert_eq!(
            paths.os_storage_path,
            custom_sys.join("os-images").to_str().unwrap()
        );
        assert_eq!(paths.state_dir, custom_sys.join("state").to_str().unwrap());
        assert_eq!(
            paths.config_path,
            custom_sys.join("config.json").to_str().unwrap()
        );
        assert!(custom_sys.join("workspaces").is_dir());
        assert!(custom_sys.join("os-images").is_dir());
        assert!(custom_sys.join("state").is_dir());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn first_run_empty_keeps_current_workspace() {
        let dir = temp_dir("firstrun-empty");
        let custom_ws = dir.join("custom-ws");
        let os_dir = dir.join("os-images");
        let state_dir = dir.join("state");
        let cfg_path = dir.join("config.json");
        let req = FirstRunRequest {
            system_path: "",
            current_system_path: dir.to_str().unwrap(),
            current_workspace_base: custom_ws.to_str().unwrap(),
            current_os_storage_path: os_dir.to_str().unwrap(),
            current_state_dir: state_dir.to_str().unwrap(),
            current_config_path: cfg_path.to_str().unwrap(),
            is_project_dir_mode: false,
            remember_system_location: false,
        };
        let paths = complete_first_run(&req).expect("empty first run");
        assert_eq!(paths.workspace_base, custom_ws.to_str().unwrap());
        assert_eq!(paths.os_storage_path, os_dir.to_str().unwrap());
        assert_eq!(paths.config_path, cfg_path.to_str().unwrap());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn storage_settings_validate_and_persist() {
        let dir = temp_dir("storage");
        let cfg = Config::new_defaults(
            dir.join("ws-default").to_str().unwrap(),
            dir.join("os-default").to_str().unwrap(),
        );
        let new_ws = dir.join("ws-new");
        let new_os = dir.join("os-new");
        let updated = save_settings(
            &cfg,
            new_ws.to_str().unwrap(),
            new_os.to_str().unwrap(),
            &cfg.workspace_base,
            &cfg.os_storage_path,
        )
        .expect("save settings");
        assert!(updated.first_run_completed);
        assert_eq!(updated.workspace_base, new_ws.to_str().unwrap());
        assert_eq!(updated.os_storage_path, new_os.to_str().unwrap());
        assert!(new_ws.is_dir());
        assert!(new_os.is_dir());

        let blocking = dir.join("blocking-file");
        fs::write(&blocking, "x").expect("write blocking file");
        let err = save_settings(
            &cfg,
            blocking.to_str().unwrap(),
            new_os.to_str().unwrap(),
            &cfg.workspace_base,
            &cfg.os_storage_path,
        )
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("Could not create the workspace directory"));
        let _ = fs::remove_dir_all(&dir);
    }
}
