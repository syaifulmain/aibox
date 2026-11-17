use aibox_core::backup::{
    admin_delete_script_text, available_disk_bytes, backups_dir_path, build_stale_report_json,
    check_export_space_json, classify_environment_staleness, delete_backup_files,
    delete_orphan_script, detect_orphans_json, directory_size_bytes,
    export_and_wipe_environment_script, export_environment_script, has_sufficient_disk_space,
    import_environment_script, prepare_delete_orphan_as_admin, scan_backup_archives_json,
    validate_export_request, validate_import_request, validate_orphan_path,
    validate_orphan_request,
};
use aibox_core::health::idle::parse_active_incus_exec_envs;

#[cxx::bridge(namespace = "aibox::ffi")]
mod ffi {
    struct StalenessResultFfi {
        classification: i32,
        days_unused: i64,
        status_key: String,
    }

    struct ImportValidationFfi {
        error: String,
        target_name: String,
        clean_backup: String,
        clean_ws_archive: String,
    }

    struct OrphanValidationFfi {
        error: String,
        clean_path: String,
    }

    struct AdminDeletePrepFfi {
        error: String,
        command: String,
        file_name: String,
    }

    extern "Rust" {
        fn backup_backups_dir_path(os_storage_path: &str) -> String;
        fn backup_has_sufficient_disk_space(required_bytes: i64, available_bytes: i64) -> bool;
        fn backup_directory_size_bytes(dir_path: &str) -> i64;
        fn backup_available_disk_bytes(path: &str) -> i64;
        fn backup_scan_archives_json(backups_dir: &str) -> String;
        fn backup_export_environment_script(
            name: &str,
            backup_file: &str,
            workspace_path: &str,
            workspace_archive_file: &str,
        ) -> String;
        fn backup_export_and_wipe_environment_script(
            name: &str,
            backup_file: &str,
            workspace_path: &str,
            workspace_archive_file: &str,
            delete_workspace_files: bool,
            sessions_env_dir: &str,
            environment_secrets_path: &str,
        ) -> String;
        fn backup_import_environment_script(
            backup_file: &str,
            target_name: &str,
            workspace_path: &str,
            workspace_archive_file: &str,
        ) -> String;
        fn backup_check_export_space_json(
            disk_usage_bytes: i64,
            disk_usage_str: &str,
            workspace_path: &str,
            include_workspace: bool,
            demo_mode: bool,
            backups_dir: &str,
        ) -> String;
        fn backup_validate_export_request(
            busy: bool,
            name: &str,
            env_exists: bool,
            clean_name: &mut String,
        ) -> String;
        fn backup_validate_import_request(
            busy: bool,
            new_name: &str,
            target_exists: bool,
            backup_file: &str,
            workspace_archive_file: &str,
            demo_mode: bool,
        ) -> ImportValidationFfi;
        fn backup_delete_files(backup_file: &str, workspace_archive_file: &str);
        fn health_parse_active_incus_exec_envs(pgrep_output: &str) -> Vec<String>;
        fn backup_classify_environment_staleness(
            last_used_epoch: i64,
            is_template: bool,
            stale_days: i32,
            now_epoch: i64,
        ) -> StalenessResultFfi;
        fn backup_detect_orphans_json(
            environments_json: &str,
            workspace_base: &str,
            backups_dir: &str,
        ) -> String;
        fn backup_build_stale_report_json(
            environments_json: &str,
            workspace_base: &str,
            backups_dir: &str,
            stale_days: i32,
            now_epoch: i64,
        ) -> String;
        fn backup_delete_orphan_script(kind: &str, target_path: &str) -> String;
        fn backup_admin_delete_script_text(kind: &str, target_path: &str) -> String;
        fn backup_validate_orphan_path(
            kind: &str,
            clean_path: &str,
            workspace_base: &str,
            backups_dir: &str,
        ) -> String;
        fn backup_validate_orphan_request(
            kind: &str,
            path: &str,
            workspace_base: &str,
            backups_dir: &str,
        ) -> OrphanValidationFfi;
        fn backup_prepare_delete_orphan_as_admin(
            kind: &str,
            path: &str,
            workspace_base: &str,
            backups_dir: &str,
            state_dir: &str,
            has_sudo: bool,
        ) -> AdminDeletePrepFfi;
    }
}

fn backup_backups_dir_path(os_storage_path: &str) -> String {
    backups_dir_path(os_storage_path)
}

fn backup_has_sufficient_disk_space(required_bytes: i64, available_bytes: i64) -> bool {
    has_sufficient_disk_space(required_bytes, available_bytes)
}

fn backup_directory_size_bytes(dir_path: &str) -> i64 {
    directory_size_bytes(dir_path)
}

fn backup_available_disk_bytes(path: &str) -> i64 {
    available_disk_bytes(path)
}

fn backup_scan_archives_json(backups_dir: &str) -> String {
    scan_backup_archives_json(backups_dir)
}

fn backup_export_environment_script(
    name: &str,
    backup_file: &str,
    workspace_path: &str,
    workspace_archive_file: &str,
) -> String {
    export_environment_script(name, backup_file, workspace_path, workspace_archive_file)
}

fn backup_export_and_wipe_environment_script(
    name: &str,
    backup_file: &str,
    workspace_path: &str,
    workspace_archive_file: &str,
    delete_workspace_files: bool,
    sessions_env_dir: &str,
    environment_secrets_path: &str,
) -> String {
    export_and_wipe_environment_script(
        name,
        backup_file,
        workspace_path,
        workspace_archive_file,
        delete_workspace_files,
        sessions_env_dir,
        environment_secrets_path,
    )
}

fn backup_import_environment_script(
    backup_file: &str,
    target_name: &str,
    workspace_path: &str,
    workspace_archive_file: &str,
) -> String {
    import_environment_script(
        backup_file,
        target_name,
        workspace_path,
        workspace_archive_file,
    )
}

fn backup_check_export_space_json(
    disk_usage_bytes: i64,
    disk_usage_str: &str,
    workspace_path: &str,
    include_workspace: bool,
    demo_mode: bool,
    backups_dir: &str,
) -> String {
    check_export_space_json(
        disk_usage_bytes,
        disk_usage_str,
        workspace_path,
        include_workspace,
        demo_mode,
        backups_dir,
    )
}

fn backup_validate_export_request(
    busy: bool,
    name: &str,
    env_exists: bool,
    clean_name: &mut String,
) -> String {
    match validate_export_request(busy, name, env_exists) {
        Ok(clean) => {
            *clean_name = clean;
            String::new()
        }
        Err(err) => {
            clean_name.clear();
            err
        }
    }
}

fn backup_validate_import_request(
    busy: bool,
    new_name: &str,
    target_exists: bool,
    backup_file: &str,
    workspace_archive_file: &str,
    demo_mode: bool,
) -> ffi::ImportValidationFfi {
    match validate_import_request(
        busy,
        new_name,
        target_exists,
        backup_file,
        workspace_archive_file,
        demo_mode,
    ) {
        Ok((target_name, clean_backup, clean_ws_archive)) => ffi::ImportValidationFfi {
            error: String::new(),
            target_name,
            clean_backup,
            clean_ws_archive,
        },
        Err(error) => ffi::ImportValidationFfi {
            error,
            target_name: String::new(),
            clean_backup: String::new(),
            clean_ws_archive: String::new(),
        },
    }
}

fn backup_delete_files(backup_file: &str, workspace_archive_file: &str) {
    delete_backup_files(backup_file, workspace_archive_file);
}

fn health_parse_active_incus_exec_envs(pgrep_output: &str) -> Vec<String> {
    parse_active_incus_exec_envs(pgrep_output)
        .into_iter()
        .collect()
}

fn backup_classify_environment_staleness(
    last_used_epoch: i64,
    is_template: bool,
    stale_days: i32,
    now_epoch: i64,
) -> ffi::StalenessResultFfi {
    let res = classify_environment_staleness(last_used_epoch, is_template, stale_days, now_epoch);
    ffi::StalenessResultFfi {
        classification: res.classification as i32,
        days_unused: res.days_unused,
        status_key: res.status_key,
    }
}

fn backup_detect_orphans_json(
    environments_json: &str,
    workspace_base: &str,
    backups_dir: &str,
) -> String {
    detect_orphans_json(environments_json, workspace_base, backups_dir)
}

fn backup_build_stale_report_json(
    environments_json: &str,
    workspace_base: &str,
    backups_dir: &str,
    stale_days: i32,
    now_epoch: i64,
) -> String {
    build_stale_report_json(
        environments_json,
        workspace_base,
        backups_dir,
        stale_days,
        now_epoch,
    )
}

fn backup_delete_orphan_script(kind: &str, target_path: &str) -> String {
    delete_orphan_script(kind, target_path)
}

fn backup_admin_delete_script_text(kind: &str, target_path: &str) -> String {
    admin_delete_script_text(kind, target_path)
}

fn backup_validate_orphan_path(
    kind: &str,
    clean_path: &str,
    workspace_base: &str,
    backups_dir: &str,
) -> String {
    validate_orphan_path(kind, clean_path, workspace_base, backups_dir)
        .err()
        .unwrap_or_default()
}

fn backup_validate_orphan_request(
    kind: &str,
    path: &str,
    workspace_base: &str,
    backups_dir: &str,
) -> ffi::OrphanValidationFfi {
    match validate_orphan_request(kind, path, workspace_base, backups_dir) {
        Ok(clean_path) => ffi::OrphanValidationFfi {
            error: String::new(),
            clean_path,
        },
        Err(error) => ffi::OrphanValidationFfi {
            error,
            clean_path: String::new(),
        },
    }
}

fn backup_prepare_delete_orphan_as_admin(
    kind: &str,
    path: &str,
    workspace_base: &str,
    backups_dir: &str,
    state_dir: &str,
    has_sudo: bool,
) -> ffi::AdminDeletePrepFfi {
    match prepare_delete_orphan_as_admin(
        kind,
        path,
        workspace_base,
        backups_dir,
        state_dir,
        has_sudo,
    ) {
        Ok((command, file_name)) => ffi::AdminDeletePrepFfi {
            error: String::new(),
            command,
            file_name,
        },
        Err(error) => ffi::AdminDeletePrepFfi {
            error,
            command: String::new(),
            file_name: String::new(),
        },
    }
}
