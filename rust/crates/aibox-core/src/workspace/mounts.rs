use std::fs;
use std::path::Path;

use crate::util::names::sanitize_name;
use crate::util::paths::{clean_path, expand_tilde, home_dir, CONTAINER_WORKSPACE_PATH};
use crate::util::shell::shell_quote;

const EXACT_BLOCKED: &[&str] = &[
    "/", "/bin", "/boot", "/dev", "/etc", "/home", "/lib", "/lib64", "/proc", "/root", "/run",
    "/sbin", "/sys", "/usr", "/var",
];

const PREFIX_BLOCKED: &[&str] = &[
    "/etc/", "/boot/", "/dev/", "/proc/", "/sys/", "/root/", "/run/", "/bin/", "/sbin/", "/lib/",
    "/lib64/", "/usr/",
];

const SENSITIVE_COMPONENTS: &[&str] = &[
    ".ssh",
    ".gnupg",
    ".aws",
    ".kube",
    ".docker",
    ".aibox",
    ".password-store",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedExtraMount {
    pub resolved_source: String,
    pub device_name: String,
    pub container_path: String,
    pub script: String,
}

pub fn validate_extra_mount_path(
    raw_path: &str,
    forbidden_config_paths: &[String],
) -> Result<(), String> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err("Choose a host directory to mount.".to_string());
    }
    let expanded = expand_tilde(trimmed);
    let path = Path::new(&expanded);
    if !path.is_dir() {
        return Err(format!(
            "Host path '{expanded}' does not exist or is not a directory."
        ));
    }

    let canonical = match fs::canonicalize(path) {
        Ok(p) => p.to_string_lossy().into_owned(),
        Err(_) => clean_path(&expanded),
    };

    let home_clean = clean_path(&home_dir().to_string_lossy());
    if EXACT_BLOCKED.contains(&canonical.as_str()) || canonical == home_clean {
        return Err(format!(
            "Refusing to mount system root or home directory '{canonical}'. Pick a specific project/data subfolder."
        ));
    }

    for prefix in PREFIX_BLOCKED {
        if canonical.starts_with(prefix) {
            return Err(format!("Refusing to mount system path '{canonical}'."));
        }
    }

    for part in canonical.split('/').filter(|p| !p.is_empty()) {
        if SENSITIVE_COMPONENTS.contains(&part) {
            return Err(format!(
                "Refusing to mount sensitive directory component '{part}'."
            ));
        }
    }

    for cfg in forbidden_config_paths {
        let clean_cfg = clean_path(cfg);
        if !clean_cfg.is_empty()
            && (canonical == clean_cfg || canonical.starts_with(&format!("{clean_cfg}/")))
        {
            return Err(format!(
                "Refusing to mount AIBox configuration directory '{canonical}'."
            ));
        }
    }

    Ok(())
}

pub fn extra_mount_device_name(raw_name: &str) -> String {
    let clean = sanitize_name(raw_name);
    if clean.is_empty() {
        String::new()
    } else {
        format!("mnt-{clean}")
    }
}

pub fn add_extra_mount_script(
    name: &str,
    device_name: &str,
    host_path: &str,
    container_path: &str,
    read_only: bool,
) -> String {
    let ro_flag = if read_only { "true" } else { "false" };
    let q_name = shell_quote(name);
    let q_dev = shell_quote(device_name);
    let q_host = shell_quote(host_path);
    let q_container = shell_quote(container_path);
    format!(
        "set -euo pipefail\n\
         echo '@@STAGE:45:Adding extra mount ({q_container})...'\n\
         if ! incus config device add {q_name} {q_dev} disk source={q_host} path={q_container} readonly={ro_flag} shift=true >/dev/null 2>&1; then\n    \
             incus config device add {q_name} {q_dev} disk source={q_host} path={q_container} readonly={ro_flag}\n\
         fi\n\
         echo '@@STAGE:100:Mount attached.'"
    )
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_add_extra_mount(
    name: &str,
    busy: bool,
    host_path: &str,
    mount_name: &str,
    read_only: bool,
    confirm_read_write: bool,
    forbidden_config_paths: &[String],
    env_found: bool,
    existing_mounts: &[(String, String)],
) -> Result<PreparedExtraMount, String> {
    if name.is_empty() {
        return Err("Select an environment first.".to_string());
    }
    if busy {
        return Err("Another operation is currently in progress.".to_string());
    }
    if !read_only && !confirm_read_write {
        return Err("Read-write extra mounts require explicit second confirmation.".to_string());
    }

    let raw_mount_name = if mount_name.is_empty() {
        Path::new(host_path.trim())
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
    } else {
        mount_name
    };
    let clean_mount = sanitize_name(raw_mount_name);
    if clean_mount.is_empty()
        || clean_mount == "workspace"
        || clean_mount == "root"
        || clean_mount == "x11"
    {
        return Err(
            "Choose a valid mount name (cannot be 'workspace', 'root', or 'x11').".to_string(),
        );
    }

    let expanded = expand_tilde(host_path.trim());
    validate_extra_mount_path(&expanded, forbidden_config_paths)?;

    let resolved_source = fs::canonicalize(&expanded)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let device_name = extra_mount_device_name(&clean_mount);
    let container_path = format!("{CONTAINER_WORKSPACE_PATH}/{clean_mount}");

    if !env_found {
        return Err(format!("Environment '{name}' was not found."));
    }
    for (existing_dev, existing_path) in existing_mounts {
        if existing_dev == &device_name || existing_path == &container_path {
            return Err(format!(
                "Mount '{container_path}' already exists on '{name}'."
            ));
        }
    }

    let script = add_extra_mount_script(
        name,
        &device_name,
        &resolved_source,
        &container_path,
        read_only,
    );
    Ok(PreparedExtraMount {
        resolved_source,
        device_name,
        container_path,
        script,
    })
}

pub fn remove_extra_mount_script(name: &str, device_name: &str) -> String {
    format!(
        "set -euo pipefail\n\
         echo '@@STAGE:50:Detaching extra mount...'\n\
         incus config device remove {} {}\n\
         echo '@@STAGE:100:Mount removed.'",
        shell_quote(name),
        shell_quote(device_name)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn extra_mount_validation_and_script_match_qt_test() {
        let base = format!("/tmp/aibox-rust-test-mounts-{}", std::process::id());
        let _ = fs::remove_dir_all(&base);
        let safe_dir = format!("{base}/datasets");
        let ssh_dir = format!("{base}/home/.ssh/keys");
        let cfg_dir = format!("{base}/.aibox-cfg");
        fs::create_dir_all(&safe_dir).unwrap();
        fs::create_dir_all(&ssh_dir).unwrap();
        fs::create_dir_all(&cfg_dir).unwrap();

        assert!(validate_extra_mount_path("/", &[]).is_err());
        assert!(validate_extra_mount_path("/etc", &[]).is_err());
        assert!(validate_extra_mount_path("/home", &[]).is_err());
        if let Ok(home) = env::var("HOME") {
            if Path::new(&home).is_dir() {
                assert!(validate_extra_mount_path(&home, &[]).is_err());
            }
        }
        assert!(validate_extra_mount_path(&ssh_dir, &[]).is_err());
        assert!(validate_extra_mount_path(&cfg_dir, std::slice::from_ref(&cfg_dir)).is_err());
        assert!(validate_extra_mount_path(&format!("{base}/missing-dir"), &[]).is_err());
        assert!(validate_extra_mount_path(&safe_dir, std::slice::from_ref(&cfg_dir)).is_ok());

        assert_eq!(extra_mount_device_name("My Data"), "mnt-my-data");
        let ro_script =
            add_extra_mount_script("ct1", "mnt-data", &safe_dir, "/home/workspace/data", true);
        assert!(ro_script.contains("readonly=true shift=true"));

        let rw_err = prepare_add_extra_mount(
            "ct1",
            false,
            &safe_dir,
            "data",
            false,
            false,
            &[],
            false,
            &[],
        );
        assert!(rw_err.is_err());

        let _ = fs::remove_dir_all(&base);
    }
}
