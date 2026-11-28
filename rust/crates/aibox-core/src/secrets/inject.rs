//! Env-file formatting, 0600 temp-file creation, and container push for secrets.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::secrets::error::SecretsError;
use crate::secrets::redact::redact_secrets;
use crate::secrets::store::{is_valid_secret_name, unique_id128};
use crate::secrets::zeroize_string;
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
pub fn create_host_temp_secret_file(content: &str) -> Result<String, SecretsError> {
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
        .map_err(|_| SecretsError::TempFile("Could not create temporary runtime secret file."))?;

    let _ = file.set_permissions(fs::Permissions::from_mode(0o600));
    if file.write_all(content.as_bytes()).is_err() {
        drop(file);
        let _ = fs::remove_file(&path);
        return Err(SecretsError::TempFile(
            "Could not write temporary runtime secret file.",
        ));
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
    build_incus_file_push_args_for_owner(host_file, env_name, remote_path, None)
}

/// Like [`build_incus_file_push_args`], optionally setting `--uid`/`--gid` ownership.
pub fn build_incus_file_push_args_for_owner(
    host_file: &str,
    env_name: &str,
    remote_path: &str,
    owner: Option<(u32, u32)>,
) -> Vec<String> {
    let mut args = vec![
        "file".to_string(),
        "push".to_string(),
        "--mode".to_string(),
        "0400".to_string(),
    ];
    if let Some((uid, gid)) = owner {
        args.push("--uid".to_string());
        args.push(uid.to_string());
        args.push("--gid".to_string());
        args.push(gid.to_string());
    }
    args.push(host_file.to_string());
    args.push(format!("{env_name}{remote_path}"));
    args
}

pub(crate) fn wait_child_with_timeout(child: Child, timeout: Duration) -> Option<Output> {
    let pid = child.id();
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let res = child.wait_with_output();
        let _ = tx.send(res);
    });

    match rx.recv_timeout(timeout) {
        Ok(res) => {
            let _ = handle.join();
            res.ok()
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            if let Ok(res) = rx.try_recv() {
                let _ = handle.join();
                return res.ok();
            }
            // SAFETY: `pid` is the live child process spawned above and not yet reaped
            // because `child.wait_with_output()` is still blocked in `handle`.
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGKILL);
            }
            let _ = handle.join();
            None
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = handle.join();
            None
        }
    }
}

/// Pushes `pairs` into `env_name:/dev/shm/aibox-<id>.env` with mode 0400, returning `None`
/// if `pairs` is empty or `Some(remote_path)` on success. Always deletes the host temp file.
pub fn push_environment_secrets_to_container(
    env_name: &str,
    pairs: &[(String, String)],
) -> Result<Option<String>, SecretsError> {
    push_environment_secrets_to_container_as(env_name, pairs, None)
}

/// Like [`push_environment_secrets_to_container`], optionally setting `--uid`/`--gid` on the pushed file.
pub fn push_environment_secrets_to_container_as(
    env_name: &str,
    pairs: &[(String, String)],
    owner: Option<(u32, u32)>,
) -> Result<Option<String>, SecretsError> {
    if pairs.is_empty() {
        return Ok(None);
    }

    let mut content = build_secret_env_file_content(pairs);
    let host_file_res = create_host_temp_secret_file(&content);
    zeroize_string(&mut content);
    let host_file = host_file_res?;

    let remote_path = generate_remote_env_path();
    let args = build_incus_file_push_args_for_owner(&host_file, env_name, &remote_path, owner);

    let child = match Command::new("incus")
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => {
            let _ = fs::remove_file(&host_file);
            return Err(SecretsError::InjectFailed(
                "Failed to inject secrets into container.".to_string(),
            ));
        }
    };

    let output = wait_child_with_timeout(child, Duration::from_millis(15_000));
    let _ = fs::remove_file(&host_file);

    match output {
        Some(out) if out.status.success() => Ok(Some(remote_path)),
        Some(out) => {
            let err_txt = String::from_utf8_lossy(&out.stderr);
            let trimmed = err_txt.trim();
            if trimmed.is_empty() {
                Err(SecretsError::InjectFailed(
                    "Failed to inject secrets into container.".to_string(),
                ))
            } else {
                let secret_vals: Vec<String> = pairs.iter().map(|(_, v)| v.clone()).collect();
                Err(SecretsError::InjectFailed(redact_secrets(
                    trimmed,
                    &secret_vals,
                )))
            }
        }
        None => Err(SecretsError::InjectFailed(
            "Failed to inject secrets into container.".to_string(),
        )),
    }
}
