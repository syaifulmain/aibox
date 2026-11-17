use aibox_core::{config, util};

#[cxx::bridge(namespace = "aibox::ffi")]
mod ffi {
    struct WorkspacePathResult {
        path: String,
        error: String,
    }

    extern "Rust" {
        fn default_system_path() -> String;
        fn default_location_file_path() -> String;
        fn configured_system_path() -> String;
        fn save_location_file(system_path: &str) -> bool;
        fn resolve_project_aibox_dir(project_dir: &str, base_cwd: &str) -> String;
        fn default_workspace_path() -> String;
        fn default_os_storage_path() -> String;
        fn normalized_path(raw: &str, fallback: &str) -> String;
        fn resolve_workspace_path(
            raw_parent: &str,
            env_name: &str,
            default_base: &str,
        ) -> WorkspacePathResult;
        fn cache_base_path(os_storage_path: &str, os_id: &str) -> String;

        fn default_os_entries_json() -> String;
        fn default_presets_json() -> String;
        fn default_disk_options() -> Vec<String>;
        fn default_ram_options() -> Vec<String>;
        fn default_cpu_options() -> Vec<i32>;

        fn config_load(
            path: &str,
            default_workspace_base: &str,
            default_os_storage_path: &str,
        ) -> String;
        fn config_save(path: &str, json: &str) -> String;
        fn config_normalize_json(json: &str) -> String;
    }
}

fn default_system_path() -> String {
    util::paths::default_system_path()
}

fn default_location_file_path() -> String {
    util::paths::default_location_file_path()
}

fn configured_system_path() -> String {
    util::paths::configured_system_path()
}

fn save_location_file(system_path: &str) -> bool {
    util::paths::save_location_file(system_path).is_ok()
}

fn resolve_project_aibox_dir(project_dir: &str, base_cwd: &str) -> String {
    util::paths::resolve_project_aibox_dir(project_dir, base_cwd)
}

fn default_workspace_path() -> String {
    util::paths::default_workspace_path()
}

fn default_os_storage_path() -> String {
    util::paths::default_os_storage_path()
}

fn normalized_path(raw: &str, fallback: &str) -> String {
    util::paths::normalized_path(raw, fallback)
}

fn resolve_workspace_path(
    raw_parent: &str,
    env_name: &str,
    default_base: &str,
) -> ffi::WorkspacePathResult {
    match util::paths::resolve_workspace_path(raw_parent, env_name, default_base) {
        Ok(path) => ffi::WorkspacePathResult {
            path,
            error: String::new(),
        },
        Err(error) => ffi::WorkspacePathResult {
            path: String::new(),
            error,
        },
    }
}

fn cache_base_path(os_storage_path: &str, os_id: &str) -> String {
    util::paths::cache_base_path(os_storage_path, os_id)
}

fn default_os_entries_json() -> String {
    util::json::to_json_compact(&config::default_os_entries()).unwrap_or_else(|_| "[]".to_string())
}

fn default_presets_json() -> String {
    util::json::to_json_compact(&config::default_presets()).unwrap_or_else(|_| "[]".to_string())
}

fn default_disk_options() -> Vec<String> {
    config::default_disk_options()
}

fn default_ram_options() -> Vec<String> {
    config::default_ram_options()
}

fn default_cpu_options() -> Vec<i32> {
    config::default_cpu_options()
}

fn config_load(path: &str, default_workspace_base: &str, default_os_storage_path: &str) -> String {
    let cfg = config::load(path, default_workspace_base, default_os_storage_path);
    util::json::to_json_compact(&cfg).unwrap_or_else(|_| "{}".to_string())
}

fn config_save(path: &str, json: &str) -> String {
    match config::save_json_str(path, json) {
        Ok(()) => String::new(),
        Err(err) => err,
    }
}

fn config_normalize_json(json: &str) -> String {
    config::normalize_json_str(json).unwrap_or_else(|_| json.to_string())
}
