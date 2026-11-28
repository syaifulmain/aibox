pub mod error;
pub mod inject;
pub mod redact;
pub mod state;
pub mod store;

pub use error::SecretsError;
pub use inject::{
    build_incus_file_push_args, build_incus_file_push_args_for_owner,
    build_secret_env_file_content, create_host_temp_secret_file, generate_remote_env_path,
    push_environment_secrets_to_container, push_environment_secrets_to_container_as,
};
pub use redact::{redact_secrets, redact_secrets_bytes, SecretRedactor, StreamingRedactor};
pub use state::{EnvSecretRow, SecretsState};
pub use store::{
    default_secrets_store_path, environment_secrets_store_path, instance_secrets_dir_path,
    is_safe_environment_file_name, is_valid_secret_name, load_secret_name_list, load_secrets_store,
    parse_env_secret_names, save_secrets_store,
};

/// Overwrites `buf` with zeros using volatile writes so the compiler cannot elide it.
pub fn zeroize_bytes(buf: &mut [u8]) {
    for b in buf.iter_mut() {
        // SAFETY: `b` is a valid, aligned, mutable reference to a `u8` in `buf`.
        unsafe { std::ptr::write_volatile(b, 0) };
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

/// Overwrites the backing buffer of `s` with zeros and clears its length.
pub fn zeroize_string(s: &mut String) {
    // SAFETY: Replacing all bytes with 0x00 leaves valid UTF-8 (NUL bytes),
    // and `clear()` sets length to 0 immediately after.
    unsafe { zeroize_bytes(s.as_bytes_mut()) };
    s.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::process::Command;

    struct TempDirGuard(PathBuf);

    impl TempDirGuard {
        fn new(prefix: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "aibox-secrets-test-{prefix}-{}",
                store::unique_id128()
            ));
            fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn secrets_validation_quoting_and_redaction() {
        assert!(is_valid_secret_name("OPENAI_API_KEY"));
        assert!(is_valid_secret_name("_PRIVATE_1"));
        assert!(!is_valid_secret_name(""));
        assert!(!is_valid_secret_name("lowercase"));
        assert!(!is_valid_secret_name("1INVALID"));
        assert!(!is_valid_secret_name("BAD-NAME"));
        assert!(!is_valid_secret_name("BAD=VAL"));

        assert_eq!(
            parse_env_secret_names(
                "OPENAI_API_KEY, bad_name, KEY=val, GITHUB_TOKEN, OPENAI_API_KEY"
            ),
            vec!["OPENAI_API_KEY", "GITHUB_TOKEN"]
        );

        let weird_val = "it's $HOME $(id) `uname`\nline2";
        let pairs = vec![
            ("OPENAI_API_KEY".to_string(), "sk-test-short".to_string()),
            (
                "OPENAI_API_KEY_LONG".to_string(),
                "sk-test-short-extra".to_string(),
            ),
            ("TRICKY_VAL".to_string(), weird_val.to_string()),
        ];
        let env_script = build_secret_env_file_content(&pairs);
        assert!(env_script.contains("OPENAI_API_KEY="));
        assert!(env_script.contains("TRICKY_VAL="));
        assert!(!env_script.contains("export "));

        // Verify /bin/sh sources quoted values verbatim without expanding $, backticks, or newlines.
        let tmp = TempDirGuard::new("quoting");
        let script_path = tmp.0.join("test.env");
        fs::write(&script_path, &env_script).expect("write test.env");
        let out = Command::new("/bin/sh")
            .args([
                "-c",
                ". \"$1\" && printf '%s' \"$TRICKY_VAL\"",
                "sh",
                &script_path.to_string_lossy(),
            ])
            .output()
            .expect("run /bin/sh");
        assert!(out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stdout), weird_val);

        // Redaction replaces longest secret values first so prefixes don't leave tails behind.
        let raw_log = "using sk-test-short-extra and sk-test-short inside output";
        let vals = vec![
            "sk-test-short".to_string(),
            "sk-test-short-extra".to_string(),
        ];
        assert_eq!(
            redact_secrets(raw_log, &vals),
            "using [REDACTED] and [REDACTED] inside output"
        );
        assert_eq!(
            String::from_utf8(redact_secrets_bytes(raw_log.as_bytes(), &vals)).unwrap(),
            "using [REDACTED] and [REDACTED] inside output"
        );
    }

    #[test]
    fn secrets_file_permissions_enforced_and_fixed() {
        let tmp = TempDirGuard::new("perms");
        let store_dir = tmp.0.join("cfg");
        let store_path = store_dir.join("secrets.json");
        let store_str = store_path.to_string_lossy().into_owned();

        let initial = vec![("API_KEY".to_string(), "sk-test-super-secret".to_string())];
        assert!(save_secrets_store(&store_str, &initial, &[], &[]).is_ok());

        let dir_mode = fs::metadata(&store_dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode & 0o077, 0);

        let file_mode = fs::metadata(&store_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(file_mode, 0o600);

        assert_eq!(load_secrets_store(&store_str).unwrap(), initial);

        // Group/world-readable files must be refused on load.
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o604)).unwrap();
        assert!(load_secrets_store(&store_str).is_err());

        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o640)).unwrap();
        assert!(load_secrets_store(&store_str).is_err());

        // Saving again through SecretsState fixes permissions back to 0600.
        let mut state = SecretsState::default();
        state.set_secrets_store_path(&store_str, false);
        assert!(!state.secrets_error.is_empty());
        assert!(state
            .save_secret("API_KEY", "sk-test-rotated", false)
            .is_ok());
        assert!(state.secrets_error.is_empty());

        let fixed_mode = fs::metadata(&store_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(fixed_mode, 0o600);
        assert_eq!(
            load_secrets_store(&store_str).unwrap(),
            vec![("API_KEY".to_string(), "sk-test-rotated".to_string())]
        );
    }

    #[test]
    fn environment_secrets_are_scoped_and_global_secrets_are_read_only() {
        let tmp = TempDirGuard::new("scoped");
        let store_path = tmp.0.join("secrets.json");
        let store_str = store_path.to_string_lossy().into_owned();

        let mut state = SecretsState::default();
        state.set_secrets_store_path(&store_str, false);

        assert!(state
            .save_secret("SHARED_KEY", "sk-test-global", false)
            .is_ok());
        assert!(state
            .save_environment_secret("alpha", "LOCAL_KEY", "sk-test-local", &[], false)
            .is_ok());

        let env_store = environment_secrets_store_path(&state.secrets_path, "alpha");
        assert!(PathBuf::from(&env_store).exists());
        let env_mode = fs::metadata(&env_store).unwrap().permissions().mode() & 0o777;
        assert_eq!(env_mode & 0o077, 0);

        state.reload_environment_secrets("alpha", &[], false);
        assert!(state.environment_secrets_error.is_empty());
        assert_eq!(state.environment_secrets_env, "alpha");
        assert_eq!(state.environment_secrets.len(), 2);

        let local_row = state
            .environment_secrets
            .iter()
            .find(|r| r.name == "LOCAL_KEY")
            .unwrap();
        let global_row = state
            .environment_secrets
            .iter()
            .find(|r| r.name == "SHARED_KEY")
            .unwrap();
        assert_eq!(local_row.scope, "environment");
        assert!(local_row.assigned);
        assert_eq!(global_row.scope, "global");

        // Global names cannot be shadowed or deleted from an environment.
        assert!(state
            .save_environment_secret("alpha", "SHARED_KEY", "sk-test-overwrite", &[], false)
            .unwrap_err()
            .to_string()
            .contains("global secret"));
        assert!(state
            .delete_environment_secret("alpha", "SHARED_KEY", &[], false)
            .unwrap_err()
            .to_string()
            .contains("cannot be deleted"));

        let alpha_secrets = state
            .resolve_environment_secrets("alpha", &[], false)
            .unwrap();
        assert_eq!(alpha_secrets.len(), 1);
        assert_eq!(alpha_secrets[0].0, "LOCAL_KEY");
        assert!(state
            .all_secret_values()
            .contains(&"sk-test-local".to_string()));
        assert!(state
            .resolve_environment_secrets("beta", &[], false)
            .unwrap()
            .is_empty());

        // Global visibility is independent from per-environment use.
        assert!(state
            .set_global_secret_instance_visible("SHARED_KEY", false, false)
            .is_ok());
        assert!(!state.secrets_list()[0].visible_in_environments);
        state.reload_environment_secrets("alpha", &[], false);
        assert_eq!(state.environment_secrets.len(), 1);
        assert_eq!(state.environment_secrets[0].scope, "environment");
        assert_eq!(
            state
                .resolve_environment_secrets("alpha", &[], false)
                .unwrap()
                .len(),
            1
        );

        // Editing a global secret keeps its visibility state.
        assert!(state
            .save_secret("SHARED_KEY", "sk-test-global-2", false)
            .is_ok());
        assert!(!state.secrets_list()[0].visible_in_environments);

        assert!(state
            .set_global_secret_instance_visible("SHARED_KEY", true, false)
            .is_ok());
        state.reload_environment_secrets("alpha", &[], false);
        assert_eq!(state.environment_secrets.len(), 2);

        // Local secrets can be disabled for their own environment.
        assert!(state
            .set_environment_local_secret_enabled("alpha", "LOCAL_KEY", false, &[], false)
            .is_ok());
        state.reload_environment_secrets("alpha", &[], false);
        assert!(
            !state
                .environment_secrets
                .iter()
                .find(|r| r.name == "LOCAL_KEY")
                .unwrap()
                .assigned
        );
        assert!(state
            .resolve_environment_secrets("alpha", &[], false)
            .unwrap()
            .is_empty());

        assert!(state
            .set_environment_local_secret_enabled("alpha", "LOCAL_KEY", true, &[], false)
            .is_ok());
        assert_eq!(
            state
                .resolve_environment_secrets("alpha", &[], false)
                .unwrap()
                .len(),
            1
        );

        assert!(state
            .delete_environment_secret("alpha", "LOCAL_KEY", &[], false)
            .is_ok());
        state.reload_environment_secrets("alpha", &[], false);
        assert_eq!(state.environment_secrets.len(), 1);
        assert_eq!(state.environment_secrets[0].scope, "global");
        assert!(state
            .resolve_environment_secrets("alpha", &[], false)
            .unwrap()
            .is_empty());
        assert!(!state
            .all_secret_values()
            .contains(&"sk-test-local".to_string()));
    }

    #[test]
    fn hiding_global_secret_excludes_it_from_resolution() {
        let mut state = SecretsState::default();
        state
            .save_secret("ANTHROPIC_API_KEY", "sk-test-demo", true)
            .unwrap();
        assert_eq!(
            state
                .resolve_environment_secrets(
                    "flutter-pos",
                    &["ANTHROPIC_API_KEY".to_string()],
                    true
                )
                .unwrap()
                .len(),
            1
        );
        state
            .set_global_secret_instance_visible("ANTHROPIC_API_KEY", false, true)
            .unwrap();
        assert!(state
            .resolve_environment_secrets("flutter-pos", &[], true)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn temp_secret_file_is_0600_and_push_args_use_0400() {
        let content =
            build_secret_env_file_content(&[("MY_TOKEN".to_string(), "sk-test-999".to_string())]);
        let path = create_host_temp_secret_file(&content).expect("create temp secret file");
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let read_back = fs::read_to_string(&path).unwrap();
        assert_eq!(read_back, "MY_TOKEN='sk-test-999'\n");
        let _ = fs::remove_file(&path);

        let args = build_incus_file_push_args(&path, "secbox", "/dev/shm/aibox-123.env");
        assert_eq!(
            args,
            vec![
                "file",
                "push",
                "--mode",
                "0400",
                path.as_str(),
                "secbox/dev/shm/aibox-123.env",
            ]
        );
    }

    #[test]
    fn secrets_files_created_with_0600_under_permissive_umask() {
        static UMASK_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        struct UmaskGuard(libc::mode_t);
        impl Drop for UmaskGuard {
            fn drop(&mut self) {
                unsafe { libc::umask(self.0) };
            }
        }

        let _lock = UMASK_LOCK.lock().unwrap();
        let prev = unsafe { libc::umask(0o000) };
        let _guard = UmaskGuard(prev);

        let tmp = TempDirGuard::new("umask");
        let store_dir = tmp.0.join("cfg");
        let store_path = store_dir.join("secrets.json");
        let store_str = store_path.to_string_lossy().into_owned();

        let pairs = vec![("UMASK_KEY".to_string(), "sk-umask-secret".to_string())];
        save_secrets_store(&store_str, &pairs, &[], &[]).unwrap();

        let dir_mode = fs::metadata(&store_dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700);
        let file_mode = fs::metadata(&store_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(file_mode, 0o600);

        let env_content = build_secret_env_file_content(&pairs);
        let temp_env = create_host_temp_secret_file(&env_content).unwrap();
        let temp_mode = fs::metadata(&temp_env).unwrap().permissions().mode() & 0o777;
        let _ = fs::remove_file(&temp_env);
        assert_eq!(temp_mode, 0o600);
    }

    #[test]
    fn streaming_redaction_masks_secrets_split_across_buffer_boundaries() {
        let values = vec![
            "sk-split-secret".to_string(),
            "sk-split-secret-longer".to_string(),
            "tok_12345".to_string(),
        ];
        let input = b"prefix sk-split-secret-longer mid sk-split-secret end tok_12345!";

        for chunk_size in [1usize, 2, 3, 5, 7, 13] {
            let mut redactor = StreamingRedactor::new(&values);
            let mut out = Vec::new();
            for chunk in input.chunks(chunk_size) {
                out.extend_from_slice(&redactor.feed(chunk));
            }
            out.extend_from_slice(&redactor.finish());
            assert_eq!(
                String::from_utf8(out).unwrap(),
                "prefix [REDACTED] mid [REDACTED] end [REDACTED]!",
                "failed for chunk_size={chunk_size}"
            );
        }

        // Also verify line-buffered streaming in task::progress across 1-byte chunks.
        let mut progress = crate::task::progress::TaskProgressState::default();
        let line = b"Auth with sk-split-secret-longer\n";
        for byte in line {
            crate::task::progress::feed_process_output_streaming(
                &mut progress,
                std::slice::from_ref(byte),
                &values,
            );
        }
        crate::task::progress::flush_process_output_streaming(&mut progress, &values);
        assert!(!progress.task_log.contains("sk-split-secret"));
        assert!(progress.task_log.contains("[REDACTED]"));
    }

    #[test]
    fn secrets_never_appear_in_history_session_or_errors() {
        let secret_val = "sk-never-leak-9876543210";
        let secret_vals = vec![secret_val.to_string()];

        // 1. SecretsError Display must never contain secret values.
        let errors: Vec<SecretsError> = vec![
            SecretsError::InvalidPath,
            SecretsError::InvalidSecretName {
                name: "bad-name".to_string(),
            },
            SecretsError::InvalidSecretNameFormat,
            SecretsError::InsecurePermissions {
                path: PathBuf::from("/tmp/secrets.json"),
                mode: 0o644,
            },
            SecretsError::NoEnvironmentSelected {
                action: "adding a secret",
            },
            SecretsError::GlobalSecretReadOnly {
                name: "API_KEY".to_string(),
                message: "'API_KEY' is a global secret.".to_string(),
            },
            SecretsError::SecretNotFound {
                name: "MISSING_KEY".to_string(),
                env: Some("alpha".to_string()),
            },
            SecretsError::TempFile("Could not write temporary runtime secret file."),
            SecretsError::InjectFailed("incus failed".to_string()),
        ];
        for err in errors {
            assert!(!err.to_string().contains(secret_val));
        }

        // 2. History records must redact secret values from description and logs.
        let tmp = TempDirGuard::new("no-leak");
        let state_dir = tmp.0.join("state");
        fs::create_dir_all(&state_dir).unwrap();
        let state_str = state_dir.to_string_lossy().into_owned();

        crate::history::record_operation_history(&crate::history::RecordOperationParams {
            state_dir: &state_str,
            max_entries: 10,
            description: &format!("Run with {secret_val}"),
            env_name: "alpha",
            status: "failed",
            exit_code: 1,
            started_at_iso: "2026-01-01T00:00:00Z",
            log_bytes: format!("token={secret_val}\n").as_bytes(),
            reason: "",
            secret_values: &secret_vals,
        });

        // 3. Session log redaction must scrub secret values and keep 0600.
        let sess_log = state_dir.join("session.log");
        fs::write(&sess_log, format!("export KEY={secret_val}\n")).unwrap();
        fs::set_permissions(&sess_log, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(crate::session::redact_session_log_file(
            &sess_log.to_string_lossy(),
            &secret_vals
        ));
        assert_eq!(
            fs::metadata(&sess_log).unwrap().permissions().mode() & 0o777,
            0o600
        );

        assert_dir_contains_no_needle(&state_dir, secret_val);
    }

    #[test]
    fn wait_child_with_timeout_completes_and_kills_slow_children() {
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};

        let fast = Command::new("/bin/sh")
            .args(["-c", "printf ok"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let out = inject::wait_child_with_timeout(fast, Duration::from_secs(2)).unwrap();
        assert!(out.status.success());
        assert_eq!(out.stdout, b"ok");

        let slow = Command::new("/bin/sh")
            .args(["-c", "sleep 5"])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let start = Instant::now();
        let timed_out = inject::wait_child_with_timeout(slow, Duration::from_millis(50));
        assert!(timed_out.is_none());
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn zeroize_clears_buffers_and_strings() {
        let mut buf = b"super-secret-bytes".to_vec();
        zeroize_bytes(&mut buf);
        assert!(buf.iter().all(|&b| b == 0));

        let mut s = String::from("super-secret-string");
        zeroize_string(&mut s);
        assert!(s.is_empty());
    }

    #[test]
    fn aho_corasick_redacts_many_patterns_and_overlapping_prefixes() {
        let mut values: Vec<String> = (0..60)
            .map(|i| format!("secret-token-{i:02}-value"))
            .collect();
        values.push("secret-token-05".to_string());
        values.push("secret-token-05-value-extended".to_string());
        values.push("αβγ-utf8-secret".to_string());

        let redactor = SecretRedactor::new(&values);
        let input = "start secret-token-05-value-extended mid secret-token-05 end αβγ-utf8-secret secret-token-59-value!";
        let redacted = redactor.redact_str(input);
        assert_eq!(
            redacted,
            "start [REDACTED] mid [REDACTED] end [REDACTED] [REDACTED]!"
        );
        assert_eq!(redact_secrets(input, &values), redacted);
    }

    #[test]
    fn canary_secret_never_leaks_into_state_dir() {
        let canary = "aibox-canary-val-9f8e7d6c5b4a3210";
        let tmp = TempDirGuard::new("canary");
        let config_dir = tmp.0.join("config");
        let state_dir = tmp.0.join("state");
        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&state_dir).unwrap();

        let store_path = config_dir.join("secrets.json");
        let mut state = SecretsState::default();
        state.set_secrets_store_path(&store_path.to_string_lossy(), false);
        state.save_secret("CANARY_KEY", canary, false).unwrap();
        state
            .save_environment_secret("devbox", "CANARY_KEY", canary, &[], false)
            .unwrap_err(); // global name cannot be shadowed
        state
            .save_environment_secret("devbox", "LOCAL_CANARY", canary, &[], false)
            .unwrap();

        let resolved = state
            .resolve_environment_secrets("devbox", &["CANARY_KEY".to_string()], false)
            .unwrap();
        assert_eq!(resolved.len(), 2);

        // Env file content is only for transient container push, never written to state_dir.
        let mut env_content = build_secret_env_file_content(&resolved);
        assert!(env_content.contains(canary));
        zeroize_string(&mut env_content);

        // Scripts only reference secret names, never values.
        let clear_script =
            state::build_clear_global_secret_script(&[("devbox".to_string(), vec![])]);
        let set_script =
            state::build_set_environment_secrets_script("devbox", &["CANARY_KEY".to_string()]);
        fs::write(
            state_dir.join("scripts.sh"),
            format!("{clear_script}\n{set_script}"),
        )
        .unwrap();

        // Task progress streaming + history + session recording in state_dir.
        let secret_vals = state.all_secret_values();
        let mut progress = crate::task::progress::TaskProgressState::default();
        let raw_output = format!("@@STAGE:40:Running with {canary}\noutput: {canary}\n");
        crate::task::progress::feed_process_output_streaming(
            &mut progress,
            raw_output.as_bytes(),
            &secret_vals,
        );
        crate::task::progress::flush_process_output_streaming(&mut progress, &secret_vals);

        crate::history::record_operation_history(&crate::history::RecordOperationParams {
            state_dir: &state_dir.to_string_lossy(),
            max_entries: 10,
            description: &format!("Create devbox ({canary})"),
            env_name: "devbox",
            status: "success",
            exit_code: 0,
            started_at_iso: "2026-01-01T00:00:00Z",
            log_bytes: raw_output.as_bytes(),
            reason: "",
            secret_values: &secret_vals,
        });

        let sess_dir = state_dir.join("sessions/devbox");
        fs::create_dir_all(&sess_dir).unwrap();
        let sess_log = sess_dir.join("20260101.log");
        fs::write(&sess_log, raw_output.as_bytes()).unwrap();
        crate::session::redact_session_log_file(&sess_log.to_string_lossy(), &secret_vals);

        assert_dir_contains_no_needle(&state_dir, canary);
    }

    fn assert_dir_contains_no_needle(dir: &std::path::Path, needle: &str) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                assert_dir_contains_no_needle(&path, needle);
            } else {
                let bytes = fs::read(&path).unwrap();
                let text = String::from_utf8_lossy(&bytes);
                assert!(
                    !text.contains(needle),
                    "secret canary leaked into {}",
                    path.display()
                );
            }
        }
    }
}
