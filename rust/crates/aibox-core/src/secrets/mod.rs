pub mod inject;
pub mod redact;
pub mod state;
pub mod store;

pub use inject::{
    build_incus_file_push_args, build_secret_env_file_content, create_host_temp_secret_file,
    generate_remote_env_path, push_environment_secrets_to_container,
};
pub use redact::{redact_secrets, redact_secrets_bytes};
pub use state::{EnvSecretRow, SecretsState};
pub use store::{
    default_secrets_store_path, environment_secrets_store_path, instance_secrets_dir_path,
    is_safe_environment_file_name, is_valid_secret_name, load_secret_name_list, load_secrets_store,
    parse_env_secret_names, save_secrets_store,
};

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
        assert_eq!(save_secrets_store(&store_str, &initial, &[], &[]), Ok(()));

        let dir_mode = fs::metadata(&store_dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode & 0o077, 0);

        let file_mode = fs::metadata(&store_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(file_mode, 0o600);

        assert_eq!(load_secrets_store(&store_str), Ok(initial));

        // Group/world-readable files must be refused on load.
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o604)).unwrap();
        assert!(load_secrets_store(&store_str).is_err());

        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o640)).unwrap();
        assert!(load_secrets_store(&store_str).is_err());

        // Saving again through SecretsState fixes permissions back to 0600.
        let mut state = SecretsState::default();
        state.set_secrets_store_path(&store_str, false);
        assert!(!state.secrets_error.is_empty());
        assert_eq!(
            state.save_secret("API_KEY", "sk-test-rotated", false),
            Ok(())
        );
        assert!(state.secrets_error.is_empty());

        let fixed_mode = fs::metadata(&store_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(fixed_mode, 0o600);
        assert_eq!(
            load_secrets_store(&store_str),
            Ok(vec![("API_KEY".to_string(), "sk-test-rotated".to_string())])
        );
    }

    #[test]
    fn environment_secrets_are_scoped_and_global_secrets_are_read_only() {
        let tmp = TempDirGuard::new("scoped");
        let store_path = tmp.0.join("secrets.json");
        let store_str = store_path.to_string_lossy().into_owned();

        let mut state = SecretsState::default();
        state.set_secrets_store_path(&store_str, false);

        assert_eq!(
            state.save_secret("SHARED_KEY", "sk-test-global", false),
            Ok(())
        );
        assert_eq!(
            state.save_environment_secret("alpha", "LOCAL_KEY", "sk-test-local", &[], false),
            Ok(())
        );

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
            .contains("global secret"));
        assert!(state
            .delete_environment_secret("alpha", "SHARED_KEY", &[], false)
            .unwrap_err()
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
        assert_eq!(
            state.set_global_secret_instance_visible("SHARED_KEY", false, false),
            Ok(())
        );
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
        assert_eq!(
            state.save_secret("SHARED_KEY", "sk-test-global-2", false),
            Ok(())
        );
        assert!(!state.secrets_list()[0].visible_in_environments);

        assert_eq!(
            state.set_global_secret_instance_visible("SHARED_KEY", true, false),
            Ok(())
        );
        state.reload_environment_secrets("alpha", &[], false);
        assert_eq!(state.environment_secrets.len(), 2);

        // Local secrets can be disabled for their own environment.
        assert_eq!(
            state.set_environment_local_secret_enabled("alpha", "LOCAL_KEY", false, &[], false),
            Ok(())
        );
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

        assert_eq!(
            state.set_environment_local_secret_enabled("alpha", "LOCAL_KEY", true, &[], false),
            Ok(())
        );
        assert_eq!(
            state
                .resolve_environment_secrets("alpha", &[], false)
                .unwrap()
                .len(),
            1
        );

        assert_eq!(
            state.delete_environment_secret("alpha", "LOCAL_KEY", &[], false),
            Ok(())
        );
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
}
