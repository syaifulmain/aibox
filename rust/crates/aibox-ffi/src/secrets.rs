use aibox_core::secrets::{
    self,
    state::{
        build_clear_global_secret_script, build_set_environment_secrets_script, EnvSecretRow,
        SecretsState,
    },
};
use aibox_core::util::json::to_json_compact;

#[cxx::bridge(namespace = "aibox::ffi")]
mod ffi {
    pub struct SecretPairFfi {
        pub name: String,
        pub value: String,
    }

    pub struct SecretsLoadResultFfi {
        pub pairs: Vec<SecretPairFfi>,
        pub error: String,
    }

    pub struct SecretsTempFileResultFfi {
        pub path: String,
        pub error: String,
    }

    pub struct SecretsPushResultFfi {
        pub ok: bool,
        pub remote_env_path: String,
        pub error: String,
    }

    pub struct EnvSecretUpdateFfi {
        pub env_name: String,
        pub remaining_names: Vec<String>,
    }

    pub struct SecretsStateInputFfi {
        pub secrets_path: String,
        pub secrets: Vec<SecretPairFfi>,
        pub secrets_error: String,
        pub hidden_global_secrets: Vec<String>,
        pub instance_secret_values: Vec<String>,
        pub environment_secrets_env: String,
        pub environment_secrets_json: String,
        pub environment_secrets_error: String,
        pub demo_mode: bool,
    }

    pub struct SecretsStateOutputFfi {
        pub error: String,
        pub secrets: Vec<SecretPairFfi>,
        pub secrets_error: String,
        pub hidden_global_secrets: Vec<String>,
        pub instance_secret_values: Vec<String>,
        pub environment_secrets_env: String,
        pub environment_secrets_json: String,
        pub environment_secrets_error: String,
    }

    extern "Rust" {
        fn secrets_default_store_path() -> String;
        fn secrets_is_safe_environment_file_name(name: &str) -> bool;
        fn secrets_instance_dir_path(secrets_path: &str) -> String;
        fn secrets_environment_store_path(secrets_path: &str, env_name: &str) -> String;
        fn secrets_is_valid_name(name: &str) -> bool;
        fn secrets_parse_env_names(raw_csv: &str) -> Vec<String>;
        fn secrets_load_store(file_path: &str) -> SecretsLoadResultFfi;
        fn secrets_load_name_list(file_path: &str, key: &str) -> Vec<String>;
        fn secrets_save_store(
            file_path: &str,
            secrets: &[SecretPairFfi],
            disabled_names: &[String],
            hidden_in_instances: &[String],
        ) -> String;
        fn secrets_build_env_file_content(secrets: &[SecretPairFfi]) -> String;
        fn secrets_redact_str(text: &str, values: &[String]) -> String;
        fn secrets_redact_bytes(data: &[u8], values: &[String]) -> Vec<u8>;
        fn secrets_create_host_temp_file(content: &str) -> SecretsTempFileResultFfi;

        fn secrets_list_json(secrets: &[SecretPairFfi], hidden_globals: &[String]) -> String;
        fn secrets_all_values(secrets: &[SecretPairFfi], instance_values: &[String])
            -> Vec<String>;

        fn secrets_reload(input: &SecretsStateInputFfi) -> SecretsStateOutputFfi;
        fn secrets_refresh_instance_cache(input: &SecretsStateInputFfi) -> Vec<String>;
        fn secrets_reload_environment(
            input: &SecretsStateInputFfi,
            env_name: &str,
            assigned_global_names: &[String],
        ) -> SecretsStateOutputFfi;
        fn secrets_update_env_assignments(
            env_secrets_json: &str,
            current_env: &str,
            env_name: &str,
            names: &[String],
        ) -> SecretsTempFileResultFfi;

        fn secrets_save_global(
            input: &SecretsStateInputFfi,
            name: &str,
            value: &str,
        ) -> SecretsStateOutputFfi;
        fn secrets_delete_global(input: &SecretsStateInputFfi, name: &str)
            -> SecretsStateOutputFfi;
        fn secrets_set_global_visible(
            input: &SecretsStateInputFfi,
            name: &str,
            visible: bool,
        ) -> SecretsStateOutputFfi;
        fn secrets_save_environment(
            input: &SecretsStateInputFfi,
            env_name: &str,
            name: &str,
            value: &str,
            assigned_global_names: &[String],
        ) -> SecretsStateOutputFfi;
        fn secrets_delete_environment(
            input: &SecretsStateInputFfi,
            env_name: &str,
            name: &str,
            assigned_global_names: &[String],
        ) -> SecretsStateOutputFfi;
        fn secrets_set_local_enabled(
            input: &SecretsStateInputFfi,
            env_name: &str,
            name: &str,
            enabled: bool,
            assigned_global_names: &[String],
        ) -> SecretsStateOutputFfi;

        fn secrets_resolve_environment(
            input: &SecretsStateInputFfi,
            env_name: &str,
            assigned_global_names: &[String],
        ) -> SecretsLoadResultFfi;
        fn secrets_push_to_container(
            input: &SecretsStateInputFfi,
            env_name: &str,
            assigned_global_names: &[String],
        ) -> SecretsPushResultFfi;

        fn secrets_build_clear_global_script(updates: Vec<EnvSecretUpdateFfi>) -> String;
        fn secrets_build_set_env_script(env_name: &str, clean_names: &[String]) -> String;
    }
}

fn pairs_from_ffi(pairs: &[ffi::SecretPairFfi]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|p| (p.name.clone(), p.value.clone()))
        .collect()
}

fn pairs_to_ffi(pairs: Vec<(String, String)>) -> Vec<ffi::SecretPairFfi> {
    pairs
        .into_iter()
        .map(|(name, value)| ffi::SecretPairFfi { name, value })
        .collect()
}

fn state_from_ffi(input: &ffi::SecretsStateInputFfi) -> SecretsState {
    let env_rows = EnvSecretRow::parse_list_json(&input.environment_secrets_json);
    SecretsState {
        secrets_path: input.secrets_path.clone(),
        secrets: pairs_from_ffi(&input.secrets),
        secrets_error: input.secrets_error.clone(),
        hidden_global_secrets: input.hidden_global_secrets.clone(),
        instance_secret_values: input.instance_secret_values.clone(),
        environment_secrets_env: input.environment_secrets_env.clone(),
        environment_secrets: env_rows,
        environment_secrets_error: input.environment_secrets_error.clone(),
    }
}

fn state_to_ffi(state: SecretsState, error: String) -> ffi::SecretsStateOutputFfi {
    let env_json = to_json_compact(&state.environment_secrets).unwrap_or_else(|_| "[]".to_string());
    ffi::SecretsStateOutputFfi {
        error,
        secrets: pairs_to_ffi(state.secrets),
        secrets_error: state.secrets_error,
        hidden_global_secrets: state.hidden_global_secrets,
        instance_secret_values: state.instance_secret_values,
        environment_secrets_env: state.environment_secrets_env,
        environment_secrets_json: env_json,
        environment_secrets_error: state.environment_secrets_error,
    }
}

fn secrets_default_store_path() -> String {
    secrets::default_secrets_store_path()
}

fn secrets_is_safe_environment_file_name(name: &str) -> bool {
    secrets::is_safe_environment_file_name(name)
}

fn secrets_instance_dir_path(secrets_path: &str) -> String {
    secrets::instance_secrets_dir_path(secrets_path)
}

fn secrets_environment_store_path(secrets_path: &str, env_name: &str) -> String {
    secrets::environment_secrets_store_path(secrets_path, env_name)
}

fn secrets_is_valid_name(name: &str) -> bool {
    secrets::is_valid_secret_name(name)
}

fn secrets_parse_env_names(raw_csv: &str) -> Vec<String> {
    secrets::parse_env_secret_names(raw_csv)
}

fn secrets_load_store(file_path: &str) -> ffi::SecretsLoadResultFfi {
    match secrets::load_secrets_store(file_path) {
        Ok(pairs) => ffi::SecretsLoadResultFfi {
            pairs: pairs_to_ffi(pairs),
            error: String::new(),
        },
        Err(error) => ffi::SecretsLoadResultFfi {
            pairs: Vec::new(),
            error,
        },
    }
}

fn secrets_load_name_list(file_path: &str, key: &str) -> Vec<String> {
    secrets::load_secret_name_list(file_path, key)
}

fn secrets_save_store(
    file_path: &str,
    secrets_in: &[ffi::SecretPairFfi],
    disabled_names: &[String],
    hidden_in_instances: &[String],
) -> String {
    match secrets::save_secrets_store(
        file_path,
        &pairs_from_ffi(secrets_in),
        disabled_names,
        hidden_in_instances,
    ) {
        Ok(()) => String::new(),
        Err(e) => e,
    }
}

fn secrets_build_env_file_content(secrets_in: &[ffi::SecretPairFfi]) -> String {
    secrets::build_secret_env_file_content(&pairs_from_ffi(secrets_in))
}

fn secrets_redact_str(text: &str, values: &[String]) -> String {
    secrets::redact_secrets(text, values)
}

fn secrets_redact_bytes(data: &[u8], values: &[String]) -> Vec<u8> {
    secrets::redact_secrets_bytes(data, values)
}

fn secrets_create_host_temp_file(content: &str) -> ffi::SecretsTempFileResultFfi {
    match secrets::create_host_temp_secret_file(content) {
        Ok(path) => ffi::SecretsTempFileResultFfi {
            path,
            error: String::new(),
        },
        Err(error) => ffi::SecretsTempFileResultFfi {
            path: String::new(),
            error,
        },
    }
}

fn secrets_list_json(secrets_in: &[ffi::SecretPairFfi], hidden_globals: &[String]) -> String {
    let state = SecretsState {
        secrets: pairs_from_ffi(secrets_in),
        hidden_global_secrets: hidden_globals.to_vec(),
        ..SecretsState::default()
    };
    to_json_compact(&state.secrets_list()).unwrap_or_else(|_| "[]".to_string())
}

fn secrets_all_values(
    secrets_in: &[ffi::SecretPairFfi],
    instance_values: &[String],
) -> Vec<String> {
    let state = SecretsState {
        secrets: pairs_from_ffi(secrets_in),
        instance_secret_values: instance_values.to_vec(),
        ..SecretsState::default()
    };
    state.all_secret_values()
}

fn secrets_reload(input: &ffi::SecretsStateInputFfi) -> ffi::SecretsStateOutputFfi {
    let mut state = state_from_ffi(input);
    state.reload_secrets(input.demo_mode);
    state_to_ffi(state, String::new())
}

fn secrets_refresh_instance_cache(input: &ffi::SecretsStateInputFfi) -> Vec<String> {
    let mut state = state_from_ffi(input);
    state.refresh_instance_secret_values_cache(input.demo_mode);
    state.instance_secret_values
}

fn secrets_reload_environment(
    input: &ffi::SecretsStateInputFfi,
    env_name: &str,
    assigned_global_names: &[String],
) -> ffi::SecretsStateOutputFfi {
    let mut state = state_from_ffi(input);
    state.reload_environment_secrets(env_name, assigned_global_names, input.demo_mode);
    state_to_ffi(state, String::new())
}

fn secrets_update_env_assignments(
    env_secrets_json: &str,
    current_env: &str,
    env_name: &str,
    names: &[String],
) -> ffi::SecretsTempFileResultFfi {
    let mut state = SecretsState {
        environment_secrets_env: current_env.to_string(),
        environment_secrets: EnvSecretRow::parse_list_json(env_secrets_json),
        ..SecretsState::default()
    };
    let changed = state.update_environment_secret_assignments(env_name, names);
    let json = if changed {
        to_json_compact(&state.environment_secrets).unwrap_or_else(|_| "[]".to_string())
    } else {
        String::new()
    };
    ffi::SecretsTempFileResultFfi {
        path: json,
        error: if changed {
            "1".to_string()
        } else {
            String::new()
        },
    }
}

fn secrets_save_global(
    input: &ffi::SecretsStateInputFfi,
    name: &str,
    value: &str,
) -> ffi::SecretsStateOutputFfi {
    let mut state = state_from_ffi(input);
    let err = state
        .save_secret(name, value, input.demo_mode)
        .err()
        .unwrap_or_default();
    state_to_ffi(state, err)
}

fn secrets_delete_global(
    input: &ffi::SecretsStateInputFfi,
    name: &str,
) -> ffi::SecretsStateOutputFfi {
    let mut state = state_from_ffi(input);
    let err = state
        .delete_secret(name, input.demo_mode)
        .err()
        .unwrap_or_default();
    state_to_ffi(state, err)
}

fn secrets_set_global_visible(
    input: &ffi::SecretsStateInputFfi,
    name: &str,
    visible: bool,
) -> ffi::SecretsStateOutputFfi {
    let mut state = state_from_ffi(input);
    let err = state
        .set_global_secret_instance_visible(name, visible, input.demo_mode)
        .err()
        .unwrap_or_default();
    state_to_ffi(state, err)
}

fn secrets_save_environment(
    input: &ffi::SecretsStateInputFfi,
    env_name: &str,
    name: &str,
    value: &str,
    assigned_global_names: &[String],
) -> ffi::SecretsStateOutputFfi {
    let mut state = state_from_ffi(input);
    let err = state
        .save_environment_secret(
            env_name,
            name,
            value,
            assigned_global_names,
            input.demo_mode,
        )
        .err()
        .unwrap_or_default();
    state_to_ffi(state, err)
}

fn secrets_delete_environment(
    input: &ffi::SecretsStateInputFfi,
    env_name: &str,
    name: &str,
    assigned_global_names: &[String],
) -> ffi::SecretsStateOutputFfi {
    let mut state = state_from_ffi(input);
    let err = state
        .delete_environment_secret(env_name, name, assigned_global_names, input.demo_mode)
        .err()
        .unwrap_or_default();
    state_to_ffi(state, err)
}

fn secrets_set_local_enabled(
    input: &ffi::SecretsStateInputFfi,
    env_name: &str,
    name: &str,
    enabled: bool,
    assigned_global_names: &[String],
) -> ffi::SecretsStateOutputFfi {
    let mut state = state_from_ffi(input);
    let err = state
        .set_environment_local_secret_enabled(
            env_name,
            name,
            enabled,
            assigned_global_names,
            input.demo_mode,
        )
        .err()
        .unwrap_or_default();
    state_to_ffi(state, err)
}

fn secrets_resolve_environment(
    input: &ffi::SecretsStateInputFfi,
    env_name: &str,
    assigned_global_names: &[String],
) -> ffi::SecretsLoadResultFfi {
    let state = state_from_ffi(input);
    match state.resolve_environment_secrets(env_name, assigned_global_names, input.demo_mode) {
        Ok(pairs) => ffi::SecretsLoadResultFfi {
            pairs: pairs_to_ffi(pairs),
            error: String::new(),
        },
        Err(error) => ffi::SecretsLoadResultFfi {
            pairs: Vec::new(),
            error,
        },
    }
}

fn secrets_push_to_container(
    input: &ffi::SecretsStateInputFfi,
    env_name: &str,
    assigned_global_names: &[String],
) -> ffi::SecretsPushResultFfi {
    let state = state_from_ffi(input);
    let pairs =
        match state.resolve_environment_secrets(env_name, assigned_global_names, input.demo_mode) {
            Ok(p) => p,
            Err(error) => {
                return ffi::SecretsPushResultFfi {
                    ok: false,
                    remote_env_path: String::new(),
                    error,
                }
            }
        };
    match secrets::push_environment_secrets_to_container(env_name, &pairs) {
        Ok(Some(remote_env_path)) => ffi::SecretsPushResultFfi {
            ok: true,
            remote_env_path,
            error: String::new(),
        },
        Ok(None) => ffi::SecretsPushResultFfi {
            ok: true,
            remote_env_path: String::new(),
            error: String::new(),
        },
        Err(error) => ffi::SecretsPushResultFfi {
            ok: false,
            remote_env_path: String::new(),
            error,
        },
    }
}

fn secrets_build_clear_global_script(updates: Vec<ffi::EnvSecretUpdateFfi>) -> String {
    let mapped: Vec<(String, Vec<String>)> = updates
        .into_iter()
        .map(|u| (u.env_name, u.remaining_names))
        .collect();
    build_clear_global_secret_script(&mapped)
}

fn secrets_build_set_env_script(env_name: &str, clean_names: &[String]) -> String {
    build_set_environment_secrets_script(env_name, clean_names)
}
