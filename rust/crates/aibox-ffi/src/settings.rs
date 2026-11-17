use aibox_core::{config, util};

#[cxx::bridge(namespace = "aibox::ffi")]
mod ffi {
    struct SettingsResult {
        error: String,
        config_json: String,
    }

    struct FirstRunPathsResult {
        error: String,
        system_path: String,
        workspace_base: String,
        os_storage_path: String,
        state_dir: String,
        config_path: String,
    }

    extern "Rust" {
        #[allow(clippy::too_many_arguments)]
        fn settings_complete_first_run(
            system_path: &str,
            current_system_path: &str,
            current_workspace_base: &str,
            current_os_storage_path: &str,
            current_state_dir: &str,
            current_config_path: &str,
            is_project_dir_mode: bool,
            remember_system_location: bool,
        ) -> FirstRunPathsResult;

        fn settings_save_settings(
            config_json: &str,
            workspace_path: &str,
            os_storage_path: &str,
            default_workspace_base: &str,
            default_os_storage_path: &str,
        ) -> SettingsResult;

        fn settings_save_os_entry(
            config_json: &str,
            original_id: &str,
            id: &str,
            alias: &str,
            name: &str,
            badge: &str,
            description: &str,
        ) -> SettingsResult;

        fn settings_remove_os_entry(config_json: &str, id: &str) -> SettingsResult;
        fn settings_reset_os_catalog(config_json: &str) -> SettingsResult;

        fn settings_save_create_options(
            config_json: &str,
            disk_csv: &str,
            ram_csv: &str,
            cpu_csv: &str,
        ) -> SettingsResult;

        fn settings_save_preset(
            config_json: &str,
            id: &str,
            title: &str,
            subtitle: &str,
        ) -> SettingsResult;

        fn settings_reset_create_options(config_json: &str) -> SettingsResult;

        fn settings_save_auto_snapshot(
            config_json: &str,
            enabled: bool,
            keep_count: i32,
        ) -> SettingsResult;

        fn settings_save_watchdog(
            config_json: &str,
            enabled: bool,
            disk_warn: i32,
            disk_critical: i32,
            auto_stop: bool,
        ) -> SettingsResult;

        fn settings_stale_day_options() -> Vec<i32>;

        fn settings_save_idle_stop(config_json: &str, minutes: i32) -> SettingsResult;
        fn settings_save_stale(config_json: &str, days: i32) -> SettingsResult;
        fn settings_save_session_recording(config_json: &str, enabled: bool) -> SettingsResult;
        fn settings_save_diff_excludes(config_json: &str, raw_excludes: &str) -> SettingsResult;
    }
}

fn parse_cfg(config_json: &str) -> Result<config::Config, String> {
    config::settings::parse_config_json(config_json)
}

fn finish(res: Result<config::Config, String>) -> ffi::SettingsResult {
    match res {
        Ok(cfg) => match util::json::to_json_compact(&cfg) {
            Ok(config_json) => ffi::SettingsResult {
                error: String::new(),
                config_json,
            },
            Err(e) => ffi::SettingsResult {
                error: e.to_string(),
                config_json: String::new(),
            },
        },
        Err(error) => ffi::SettingsResult {
            error,
            config_json: String::new(),
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn settings_complete_first_run(
    system_path: &str,
    current_system_path: &str,
    current_workspace_base: &str,
    current_os_storage_path: &str,
    current_state_dir: &str,
    current_config_path: &str,
    is_project_dir_mode: bool,
    remember_system_location: bool,
) -> ffi::FirstRunPathsResult {
    let req = config::settings::FirstRunRequest {
        system_path,
        current_system_path,
        current_workspace_base,
        current_os_storage_path,
        current_state_dir,
        current_config_path,
        is_project_dir_mode,
        remember_system_location,
    };
    match config::settings::complete_first_run(&req) {
        Ok(p) => ffi::FirstRunPathsResult {
            error: String::new(),
            system_path: p.system_path,
            workspace_base: p.workspace_base,
            os_storage_path: p.os_storage_path,
            state_dir: p.state_dir,
            config_path: p.config_path,
        },
        Err(error) => ffi::FirstRunPathsResult {
            error,
            system_path: String::new(),
            workspace_base: String::new(),
            os_storage_path: String::new(),
            state_dir: String::new(),
            config_path: String::new(),
        },
    }
}

fn settings_save_settings(
    config_json: &str,
    workspace_path: &str,
    os_storage_path: &str,
    default_workspace_base: &str,
    default_os_storage_path: &str,
) -> ffi::SettingsResult {
    finish(parse_cfg(config_json).and_then(|cfg| {
        config::settings::save_settings(
            &cfg,
            workspace_path,
            os_storage_path,
            default_workspace_base,
            default_os_storage_path,
        )
    }))
}

fn settings_save_os_entry(
    config_json: &str,
    original_id: &str,
    id: &str,
    alias: &str,
    name: &str,
    badge: &str,
    description: &str,
) -> ffi::SettingsResult {
    finish(parse_cfg(config_json).and_then(|cfg| {
        config::settings::save_os_entry(&cfg, original_id, id, alias, name, badge, description)
    }))
}

fn settings_remove_os_entry(config_json: &str, id: &str) -> ffi::SettingsResult {
    finish(parse_cfg(config_json).and_then(|cfg| config::settings::remove_os_entry(&cfg, id)))
}

fn settings_reset_os_catalog(config_json: &str) -> ffi::SettingsResult {
    finish(parse_cfg(config_json).map(|cfg| config::settings::reset_os_catalog(&cfg)))
}

fn settings_save_create_options(
    config_json: &str,
    disk_csv: &str,
    ram_csv: &str,
    cpu_csv: &str,
) -> ffi::SettingsResult {
    finish(
        parse_cfg(config_json).and_then(|cfg| {
            config::settings::save_create_options(&cfg, disk_csv, ram_csv, cpu_csv)
        }),
    )
}

fn settings_save_preset(
    config_json: &str,
    id: &str,
    title: &str,
    subtitle: &str,
) -> ffi::SettingsResult {
    finish(
        parse_cfg(config_json)
            .and_then(|cfg| config::settings::save_preset(&cfg, id, title, subtitle)),
    )
}

fn settings_reset_create_options(config_json: &str) -> ffi::SettingsResult {
    finish(parse_cfg(config_json).map(|cfg| config::settings::reset_create_options(&cfg)))
}

fn settings_save_auto_snapshot(
    config_json: &str,
    enabled: bool,
    keep_count: i32,
) -> ffi::SettingsResult {
    finish(
        parse_cfg(config_json).and_then(|cfg| {
            config::settings::save_auto_snapshot_settings(&cfg, enabled, keep_count)
        }),
    )
}

fn settings_save_watchdog(
    config_json: &str,
    enabled: bool,
    disk_warn: i32,
    disk_critical: i32,
    auto_stop: bool,
) -> ffi::SettingsResult {
    finish(parse_cfg(config_json).and_then(|cfg| {
        config::settings::save_watchdog_settings(&cfg, enabled, disk_warn, disk_critical, auto_stop)
    }))
}

fn settings_stale_day_options() -> Vec<i32> {
    config::settings::stale_day_options()
}

fn settings_save_idle_stop(config_json: &str, minutes: i32) -> ffi::SettingsResult {
    finish(
        parse_cfg(config_json)
            .and_then(|cfg| config::settings::save_idle_stop_settings(&cfg, minutes)),
    )
}

fn settings_save_stale(config_json: &str, days: i32) -> ffi::SettingsResult {
    finish(parse_cfg(config_json).and_then(|cfg| config::settings::save_stale_settings(&cfg, days)))
}

fn settings_save_session_recording(config_json: &str, enabled: bool) -> ffi::SettingsResult {
    finish(
        parse_cfg(config_json)
            .map(|cfg| config::settings::save_session_recording_settings(&cfg, enabled)),
    )
}

fn settings_save_diff_excludes(config_json: &str, raw_excludes: &str) -> ffi::SettingsResult {
    finish(
        parse_cfg(config_json).map(|cfg| config::settings::save_diff_excludes(&cfg, raw_excludes)),
    )
}
