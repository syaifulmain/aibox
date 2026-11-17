//! Env-file formatting, 0600 temp-file creation, and container push for secrets.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::secrets::store::{is_valid_secret_name, unique_id128};
use crate::util::paths::clean_path;
use crate::util::shell::shell_quote;

/// Formats `NAME='value'\n` lines for `/bin/sh` sourcing without `export`.
pub fn build_secret_env_file_content(secrets: &[(String, String)]) -> String {
    let mut out = String::new();
    for (name, value) in secrets {
        if !is_valid_secret_name(name) {
            continue;
        }
        out.push_str(name);
        out.push('=');
        out.push_str(&shell_quote(value));
        out.push('\n');
    }
    out
}

/// Creates a `0600` (`O_CREAT | O_EXCL`) temporary `.env` file in `$XDG_RUNTIME_DIR`
/// (falling back to `std::env::temp_dir()`) and writes `content` to it.
pub fn create_host_temp_secret_file(content: &str) -> Result<String, String> {
    let runtime_dir = match env::var("XDG_RUNTIME_DIR") {
        Ok(val) => {
            let trimmed = val.trim();
            let p = Path::new(trimmed);
            if !trimmed.is_empty() && p.is_dir() && is_writable_dir(p) {
                trimmed.to_string()
            } else {
                env::temp_dir().to_string_lossy().into_owned()
            }
        }
        Err(_) => env::temp_dir().to_string_lossy().into_owned(),
    };

    let path = clean_path(&format!("{runtime_dir}/aibox-{}.env", unique_id128()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|_| "Could not create temporary runtime secret file.".to_string())?;

    let _ = file.set_permissions(fs::Permissions::from_mode(0o600));
    if file.write_all(content.as_bytes()).is_err() {
        drop(file);
        let _ = fs::remove_file(&path);
        return Err("Could not write temporary runtime secret file.".to_string());
    }
    Ok(path)
}

fn is_writable_dir(dir: &Path) -> bool {
    let probe = dir.join(format!(".aibox-probe-{}", unique_id128()));
    if OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&probe)
        .is_ok()
    {
        let _ = fs::remove_file(probe);
        true
    } else {
        false
    }
}

/// Generates a `/dev/shm/aibox-<id>.env` path inside the container.
pub fn generate_remote_env_path() -> String {
    format!("/dev/shm/aibox-{}.env", unique_id128())
}

/// Builds the `incus` argument list for pushing a host secret file with `--mode 0400`.
pub fn build_incus_file_push_args(
    host_file: &str,
    env_name: &str,
    remote_path: &str,
) -> Vec<String> {
    vec![
        "file".to_string(),
        "push".to_string(),
        "--mode".to_string(),
        "0400".to_string(),
        host_file.to_string(),
        format!("{env_name}{remote_path}"),
    ]
}

/// Pushes `pairs` into `env_name:/dev/shm/aibox-<id>.env` with mode 0400, returning `None`
/// if `pairs` is empty or `Some(remote_path)` on success. Always deletes the host temp file.
pub fn push_environment_secrets_to_container(
    env_name: &str,
    pairs: &[(String, String)],
) -> Result<Option<String>, String> {
    if pairs.is_empty() {
        return Ok(None);
    }

    let host_file = create_host_temp_secret_file(&build_secret_env_file_content(pairs))?;
    let remote_path = generate_remote_env_path();
    let args = build_incus_file_push_args(&host_file, env_name, &remote_path);

    let mut child = match Command::new("incus")
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => {
            let _ = fs::remove_file(&host_file);
            return Err("Failed to inject secrets into container.".to_string());
        }
    };

    let deadline = Instant::now() + Duration::from_millis(15_000);
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Some(st),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                thread::sleep(Duration::from_millis(15));
            }
            Err(_) => break None,
        }
    };

    let _ = fs::remove_file(&host_file);

    match status {
        Some(st) if st.success() => Ok(Some(remote_path)),
        _ => {
            let mut err_txt = String::new();
            if let Some(mut stderr) = child.stderr.take() {
                use std::io::Read;
                let _ = stderr.read_to_string(&mut err_txt);
            }
            let trimmed = err_txt.trim();
            if trimmed.is_empty() {
                Err("Failed to inject secrets into container.".to_string())
            } else {
                Err(trimmed.to_string())
            }
        }
    }
}
