// TODO(P17c): remove once all signals and task/diff fields are wired.
#![allow(dead_code)]
#![allow(clippy::too_many_arguments)]

use std::path::Path;
use std::pin::Pin;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QList, QMap, QMapPair_QString_QVariant, QString, QStringList, QUrl, QVariant};

use aibox_core::backup::{build_stale_report, scan_backup_archives};
use aibox_core::config::defaults::{
    default_cpu_options, default_disk_options, default_presets, default_ram_options,
};
use aibox_core::config::settings::stale_day_options;
use aibox_core::engine::Engine;
use aibox_core::health::pool::{
    parse_incus_storage_info, sum_root_disk_quotas_bytes, StoragePoolInfo,
};
use aibox_core::history::{
    load_history_seen_marker, load_operation_history, recent_failed_history_count,
};
use aibox_core::incus::catalog::catalog_for_entries;
use aibox_core::incus::demo::build_demo_state_now;
use aibox_core::incus::list::{
    parse as parse_incus_list, templates as extract_templates, ParseDefaults,
};
use aibox_core::session::scan_recorded_sessions;
use aibox_core::util::paths::{
    clean_path, default_os_storage_path, default_system_path, default_workspace_path,
    CONTAINER_WORKSPACE_PATH,
};
use aibox_core::workspace::diff::default_diff_excludes;
use aibox_core::workspace::file_access::{apply_file_access_state, FileAccessEnvInput};

use crate::variant::{
    i32s_to_qvariant_list, json_array_to_qvariant_list, json_object_to_qvariant_map,
    serialize_to_qvariant_list, serialize_to_qvariant_map, strings_to_qstringlist,
};

type QListQVariant = QList<QVariant>;
type QMapQStringQVariant = QMap<QMapPair_QString_QVariant>;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qlist.h");
        type QList_QVariant = super::QListQVariant;

        include!("cxx-qt-lib/qmap.h");
        type QMap_QString_QVariant = super::QMapQStringQVariant;

        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;

        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;

        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;

        include!("shim/clipboard_shim.h");
        fn aibox_clipboard_set_text(text: &QString) -> bool;
        fn aibox_clipboard_image_png() -> Vec<u8>;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qproperty(i32, count, READ, NOTIFY = count_changed)]
        #[qproperty(QList_QVariant, environments, READ, NOTIFY = environments_changed)]
        #[qproperty(QList_QVariant, os_catalog, cxx_name = "osCatalog", READ, NOTIFY = os_catalog_changed)]
        #[qproperty(i32, selected_index, cxx_name = "selectedIndex", READ, WRITE = set_selected_index, NOTIFY = selected_index_changed)]
        #[qproperty(QMap_QString_QVariant, selected_environment, cxx_name = "selectedEnvironment", READ = selected_environment, NOTIFY = selected_index_changed)]
        #[qproperty(bool, busy, READ, NOTIFY = busy_changed)]
        #[qproperty(i32, task_progress, cxx_name = "taskProgress", READ, NOTIFY = task_progress_changed)]
        #[qproperty(QString, task_stage, cxx_name = "taskStage", READ, NOTIFY = task_progress_changed)]
        #[qproperty(QString, task_log, cxx_name = "taskLog", READ, NOTIFY = task_progress_changed)]
        #[qproperty(QString, pending_env_name, cxx_name = "pendingEnvName", READ, NOTIFY = busy_changed)]
        #[qproperty(QString, status_message, cxx_name = "statusMessage", READ, NOTIFY = status_message_changed)]
        #[qproperty(QString, error_message, cxx_name = "errorMessage", READ, NOTIFY = error_message_changed)]
        #[qproperty(QString, downloading_os_id, cxx_name = "downloadingOsId", READ, NOTIFY = os_catalog_changed)]
        #[qproperty(QString, system_path, cxx_name = "systemPath", READ, NOTIFY = system_path_changed)]
        #[qproperty(
            QString,
            default_system_path,
            cxx_name = "defaultSystemPath",
            READ,
            CONSTANT
        )]
        #[qproperty(QString, workspace_base, cxx_name = "workspaceBase", READ, NOTIFY = workspace_base_changed)]
        #[qproperty(
            QString,
            container_workspace_path,
            cxx_name = "containerWorkspacePath",
            READ,
            CONSTANT
        )]
        #[qproperty(QString, os_storage_path, cxx_name = "osStoragePath", READ, NOTIFY = os_storage_path_changed)]
        #[qproperty(
            QString,
            default_workspace_base,
            cxx_name = "defaultWorkspaceBase",
            READ,
            CONSTANT
        )]
        #[qproperty(
            QString,
            default_os_storage_path,
            cxx_name = "defaultOsStoragePath",
            READ,
            CONSTANT
        )]
        #[qproperty(QString, config_file_path, cxx_name = "configFilePath", READ, NOTIFY = config_file_path_changed)]
        #[qproperty(bool, first_run, cxx_name = "firstRun", READ, NOTIFY = first_run_changed)]
        #[qproperty(QList_QVariant, presets, READ, NOTIFY = create_options_changed)]
        #[qproperty(QStringList, disk_options, cxx_name = "diskOptions", READ, NOTIFY = create_options_changed)]
        #[qproperty(QStringList, ram_options, cxx_name = "ramOptions", READ, NOTIFY = create_options_changed)]
        #[qproperty(QList_QVariant, cpu_options, cxx_name = "cpuOptions", READ, NOTIFY = create_options_changed)]
        #[qproperty(QString, default_preset_id, cxx_name = "defaultPresetId", READ, NOTIFY = create_options_changed)]
        #[qproperty(QString, default_os_id, cxx_name = "defaultOsId", READ, NOTIFY = os_catalog_changed)]
        #[qproperty(QString, default_disk, cxx_name = "defaultDisk", READ, NOTIFY = create_options_changed)]
        #[qproperty(QString, default_ram, cxx_name = "defaultRam", READ, NOTIFY = create_options_changed)]
        #[qproperty(i32, default_cpu, cxx_name = "defaultCpu", READ, NOTIFY = create_options_changed)]
        #[qproperty(bool, auto_snapshot_enabled, cxx_name = "autoSnapshotEnabled", READ, NOTIFY = auto_snapshot_settings_changed)]
        #[qproperty(i32, auto_snapshot_keep, cxx_name = "autoSnapshotKeep", READ, NOTIFY = auto_snapshot_settings_changed)]
        #[qproperty(QList_QVariant, templates, READ = templates, NOTIFY = environments_changed)]
        #[qproperty(
            QStringList,
            process_limit_options,
            cxx_name = "processLimitOptions",
            READ,
            CONSTANT
        )]
        #[qproperty(
            QStringList,
            io_limit_options,
            cxx_name = "ioLimitOptions",
            READ,
            CONSTANT
        )]
        #[qproperty(QString, default_process_limit, cxx_name = "defaultProcessLimit", READ, NOTIFY = create_options_changed)]
        #[qproperty(QString, default_io_limit, cxx_name = "defaultIoLimit", READ, NOTIFY = create_options_changed)]
        #[qproperty(bool, watchdog_enabled, cxx_name = "watchdogEnabled", READ, NOTIFY = watchdog_settings_changed)]
        #[qproperty(i32, watchdog_disk_warn, cxx_name = "watchdogDiskWarn", READ, NOTIFY = watchdog_settings_changed)]
        #[qproperty(i32, watchdog_disk_critical, cxx_name = "watchdogDiskCritical", READ, NOTIFY = watchdog_settings_changed)]
        #[qproperty(bool, watchdog_auto_stop, cxx_name = "watchdogAutoStop", READ, NOTIFY = watchdog_settings_changed)]
        #[qproperty(i32, idle_stop_minutes, cxx_name = "idleStopMinutes", READ, NOTIFY = idle_stop_settings_changed)]
        #[qproperty(
            QList_QVariant,
            idle_stop_options,
            cxx_name = "idleStopOptions",
            READ,
            CONSTANT
        )]
        #[qproperty(i32, stale_days, cxx_name = "staleDays", READ, NOTIFY = stale_report_settings_changed)]
        #[qproperty(
            QList_QVariant,
            stale_day_options,
            cxx_name = "staleDayOptions",
            READ,
            CONSTANT
        )]
        #[qproperty(QMap_QString_QVariant, stale_report, cxx_name = "staleReport", READ, NOTIFY = stale_report_changed)]
        #[qproperty(bool, record_sessions, cxx_name = "recordSessions", READ, NOTIFY = record_sessions_settings_changed)]
        #[qproperty(QList_QVariant, recorded_sessions, cxx_name = "recordedSessions", READ, NOTIFY = recorded_sessions_changed)]
        #[qproperty(QList_QVariant, operation_history, cxx_name = "operationHistory", READ, NOTIFY = operation_history_changed)]
        #[qproperty(i32, recent_failed_history_count, cxx_name = "recentFailedHistoryCount", READ = recent_failed_history_count, NOTIFY = operation_history_changed)]
        #[qproperty(QList_QVariant, egress_logs, cxx_name = "egressLogs", READ, NOTIFY = egress_logs_changed)]
        #[qproperty(QString, egress_logs_env_name, cxx_name = "egressLogsEnvName", READ, NOTIFY = egress_logs_changed)]
        #[qproperty(QString, backups_dir_path, cxx_name = "backupsDirPath", READ = backups_dir_path, NOTIFY = os_storage_path_changed)]
        #[qproperty(QList_QVariant, backup_archives, cxx_name = "backupArchives", READ, NOTIFY = backup_archives_changed)]
        #[qproperty(QStringList, diff_excludes, cxx_name = "diffExcludes", READ, NOTIFY = diff_excludes_changed)]
        #[qproperty(bool, diff_loading, cxx_name = "diffLoading", READ, NOTIFY = workspace_diff_changed)]
        #[qproperty(QList_QVariant, diff_entries, cxx_name = "diffEntries", READ, NOTIFY = workspace_diff_changed)]
        #[qproperty(QMap_QString_QVariant, diff_summary, cxx_name = "diffSummary", READ, NOTIFY = workspace_diff_changed)]
        #[qproperty(QString, selected_diff_text, cxx_name = "selectedDiffText", READ, NOTIFY = selected_diff_changed)]
        #[qproperty(QString, selected_diff_info, cxx_name = "selectedDiffInfo", READ, NOTIFY = selected_diff_changed)]
        #[qproperty(QList_QVariant, secrets_list, cxx_name = "secretsList", READ = secrets_list, NOTIFY = secrets_changed)]
        #[qproperty(QString, secrets_error, cxx_name = "secretsError", READ, NOTIFY = secrets_changed)]
        #[qproperty(
            QString,
            secrets_store_path,
            cxx_name = "secretsStorePath",
            READ,
            CONSTANT
        )]
        #[qproperty(QList_QVariant, environment_secrets, cxx_name = "environmentSecrets", READ, NOTIFY = environment_secrets_changed)]
        #[qproperty(QString, environment_secrets_error, cxx_name = "environmentSecretsError", READ, NOTIFY = environment_secrets_changed)]
        #[qproperty(QString, environment_secrets_env_name, cxx_name = "environmentSecretsEnvName", READ, NOTIFY = environment_secrets_changed)]
        #[qproperty(QMap_QString_QVariant, pool_info, cxx_name = "poolInfo", READ, NOTIFY = pool_info_changed)]
        type Backend = super::BackendRust;

        fn selected_environment(self: &Backend) -> QMap_QString_QVariant;
        fn templates(self: &Backend) -> QList_QVariant;
        fn recent_failed_history_count(self: &Backend) -> i32;
        fn backups_dir_path(self: &Backend) -> QString;
        fn secrets_list(self: &Backend) -> QList_QVariant;

        #[qsignal]
        #[cxx_name = "countChanged"]
        fn count_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "environmentsChanged"]
        fn environments_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "osCatalogChanged"]
        fn os_catalog_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "selectedIndexChanged"]
        fn selected_index_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "busyChanged"]
        fn busy_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "taskProgressChanged"]
        fn task_progress_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "statusMessageChanged"]
        fn status_message_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "errorMessageChanged"]
        fn error_message_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "systemPathChanged"]
        fn system_path_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "workspaceBaseChanged"]
        fn workspace_base_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "osStoragePathChanged"]
        fn os_storage_path_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "configFilePathChanged"]
        fn config_file_path_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "firstRunChanged"]
        fn first_run_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "createOptionsChanged"]
        fn create_options_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "autoSnapshotSettingsChanged"]
        fn auto_snapshot_settings_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "watchdogSettingsChanged"]
        fn watchdog_settings_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "idleStopSettingsChanged"]
        fn idle_stop_settings_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "staleReportSettingsChanged"]
        fn stale_report_settings_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "staleReportChanged"]
        fn stale_report_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "recordSessionsSettingsChanged"]
        fn record_sessions_settings_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "recordedSessionsChanged"]
        fn recorded_sessions_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "operationHistoryChanged"]
        fn operation_history_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "egressLogsChanged"]
        fn egress_logs_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "backupArchivesChanged"]
        fn backup_archives_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "diffExcludesChanged"]
        fn diff_excludes_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "workspaceDiffChanged"]
        fn workspace_diff_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "selectedDiffChanged"]
        fn selected_diff_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "secretsChanged"]
        fn secrets_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "environmentSecretsChanged"]
        fn environment_secrets_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "poolInfoChanged"]
        fn pool_info_changed(self: Pin<&mut Backend>);
        #[qsignal]
        #[cxx_name = "environmentCreated"]
        fn environment_created(self: Pin<&mut Backend>, name: &QString);

        #[qinvokable]
        fn configure(self: Pin<&mut Backend>, path: &QString, project_mode: bool);

        #[qinvokable]
        #[cxx_name = "setSelectedIndex"]
        fn set_selected_index(self: Pin<&mut Backend>, index: i32);

        #[qinvokable]
        #[cxx_name = "primaryAction"]
        fn primary_action(self: Pin<&mut Backend>);

        #[qinvokable]
        fn refresh(self: Pin<&mut Backend>);

        #[qinvokable]
        #[cxx_name = "clearError"]
        fn clear_error(self: Pin<&mut Backend>);

        #[qinvokable]
        #[cxx_name = "cancelTask"]
        fn cancel_task(self: Pin<&mut Backend>);

        #[qinvokable]
        #[cxx_name = "createEnvironment"]
        fn create_environment(
            self: Pin<&mut Backend>,
            name: &QString,
            os_id: &QString,
            preset: &QString,
            disk_gib: &QString,
            ram_gib: &QString,
            cpu_cores: i32,
            process_limit: &QString,
            io_limit: &QString,
            workspace_parent: &QString,
        );

        #[qinvokable]
        #[cxx_name = "cloneEnvironment"]
        fn clone_environment_simple(
            self: Pin<&mut Backend>,
            source_name: &QString,
            new_name: &QString,
        );

        #[qinvokable]
        #[cxx_name = "cloneEnvironment"]
        fn clone_environment(
            self: Pin<&mut Backend>,
            source_name: &QString,
            new_name: &QString,
            disk_gib: &QString,
            ram_gib: &QString,
            cpu_cores: i32,
            start_after: bool,
            process_limit: &QString,
            io_limit: &QString,
            workspace_parent: &QString,
        );

        #[qinvokable]
        #[cxx_name = "pathToUrl"]
        fn path_to_url(self: &Backend, path: &QString) -> QUrl;

        #[qinvokable]
        #[cxx_name = "localPath"]
        fn local_path(self: &Backend, url: &QUrl) -> QString;

        #[qinvokable]
        #[cxx_name = "environmentSecretsStorePath"]
        fn environment_secrets_store_path(self: &Backend, env_name: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "adminFixCommand"]
        fn admin_fix_command(self: &Backend, message: &QString) -> QString;

        // Stubs for P17b/P17c
        #[qinvokable]
        #[cxx_name = "enterEnvironment"]
        fn enter_environment(self: Pin<&mut Backend>, name: &QString);

        #[qinvokable]
        #[cxx_name = "openWorkspaceFolder"]
        fn open_workspace_folder(self: Pin<&mut Backend>, name: &QString);

        #[qinvokable]
        #[cxx_name = "enableFileAccess"]
        fn enable_file_access(self: Pin<&mut Backend>, name: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "disableFileAccess"]
        fn disable_file_access(self: Pin<&mut Backend>, name: &QString);

        #[qinvokable]
        #[cxx_name = "openFileAccess"]
        fn open_file_access(self: Pin<&mut Backend>, name: &QString);

        #[qinvokable]
        #[cxx_name = "toggleStartStop"]
        fn toggle_start_stop(self: Pin<&mut Backend>, name: &QString);

        #[qinvokable]
        #[cxx_name = "createSnapshot"]
        fn create_snapshot(self: Pin<&mut Backend>, name: &QString, snapshot_name: &QString);

        #[qinvokable]
        #[cxx_name = "restoreSnapshot"]
        fn restore_snapshot(
            self: Pin<&mut Backend>,
            name: &QString,
            snapshot_name: &QString,
            mode: &QString,
        );

        #[qinvokable]
        #[cxx_name = "deleteSnapshot"]
        fn delete_snapshot(self: Pin<&mut Backend>, name: &QString, snapshot_name: &QString);

        #[qinvokable]
        #[cxx_name = "setEnvironmentWorkspaceSnapshot"]
        fn set_environment_workspace_snapshot(
            self: Pin<&mut Backend>,
            name: &QString,
            enabled: bool,
        );

        #[qinvokable]
        #[cxx_name = "convertWorkspaceToSubvolume"]
        fn convert_workspace_to_subvolume(self: Pin<&mut Backend>, name: &QString);

        #[qinvokable]
        #[cxx_name = "deletePreSubvolBackup"]
        fn delete_pre_subvol_backup(self: Pin<&mut Backend>, name: &QString);

        #[qinvokable]
        #[cxx_name = "loadWorkspaceDiff"]
        fn load_workspace_diff(self: Pin<&mut Backend>, name: &QString, snapshot_name: &QString);

        #[qinvokable]
        #[cxx_name = "loadFileDiff"]
        fn load_file_diff(
            self: Pin<&mut Backend>,
            name: &QString,
            snapshot_name: &QString,
            relative_path: &QString,
            status: &QString,
        );

        #[qinvokable]
        #[cxx_name = "restoreSingleFile"]
        fn restore_single_file(
            self: Pin<&mut Backend>,
            name: &QString,
            snapshot_name: &QString,
            relative_path: &QString,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "saveDiffExcludes"]
        fn save_diff_excludes(self: Pin<&mut Backend>, raw_excludes: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "wipeEnvironment"]
        fn wipe_environment(self: Pin<&mut Backend>, name: &QString, delete_workspace_files: bool);

        #[qinvokable]
        #[cxx_name = "setEnvironmentTemplate"]
        fn set_environment_template(self: Pin<&mut Backend>, name: &QString, is_template: bool);

        #[qinvokable]
        #[cxx_name = "setEnvironmentAutoSnapshot"]
        fn set_environment_auto_snapshot(self: Pin<&mut Backend>, name: &QString, enabled: bool);

        #[qinvokable]
        #[cxx_name = "setEnvironmentAutoStop"]
        fn set_environment_auto_stop(self: Pin<&mut Backend>, name: &QString, enabled: bool);

        #[qinvokable]
        #[cxx_name = "setEnvironmentIdleStop"]
        fn set_environment_idle_stop(
            self: Pin<&mut Backend>,
            name: &QString,
            value: &QString,
            allow_with_port_forwards: bool,
        );

        #[qinvokable]
        #[cxx_name = "setNetworkMode"]
        fn set_network_mode(
            self: Pin<&mut Backend>,
            name: &QString,
            mode: &QString,
            allowlist: &QString,
        );

        #[qinvokable]
        #[cxx_name = "refreshNetworkAllowlist"]
        fn refresh_network_allowlist(self: Pin<&mut Backend>, name: &QString);

        #[qinvokable]
        #[cxx_name = "setEgressRecording"]
        fn set_egress_recording(self: Pin<&mut Backend>, name: &QString, enabled: bool);

        #[qinvokable]
        #[cxx_name = "refreshEgressLog"]
        fn refresh_egress_log(self: Pin<&mut Backend>, name: &QString);

        #[qinvokable]
        #[cxx_name = "addEgressDestinationToAllowlist"]
        fn add_egress_destination_to_allowlist(
            self: Pin<&mut Backend>,
            name: &QString,
            destination_ip: &QString,
        );

        #[qinvokable]
        #[cxx_name = "addPortForward"]
        fn add_port_forward(
            self: Pin<&mut Backend>,
            name: &QString,
            host_port: i32,
            guest_port: i32,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "removePortForward"]
        fn remove_port_forward(self: Pin<&mut Backend>, name: &QString, device_name: &QString);

        #[qinvokable]
        #[cxx_name = "openForwardedPort"]
        fn open_forwarded_port(self: Pin<&mut Backend>, host_port: i32);

        #[qinvokable]
        #[cxx_name = "addExtraMount"]
        fn add_extra_mount(
            self: Pin<&mut Backend>,
            name: &QString,
            host_path: &QString,
            mount_name: &QString,
            read_only: bool,
            confirm_read_write: bool,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "removeExtraMount"]
        fn remove_extra_mount(self: Pin<&mut Backend>, name: &QString, device_name: &QString);

        #[qinvokable]
        #[cxx_name = "validateExtraMountCandidate"]
        fn validate_extra_mount_candidate(self: &Backend, raw_path: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "updateEnvironmentResources"]
        fn update_environment_resources(
            self: Pin<&mut Backend>,
            name: &QString,
            cpu_cores: i32,
            ram_gib: &QString,
            disk_gib: &QString,
            process_limit: &QString,
            io_limit: &QString,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "refreshBackups"]
        fn refresh_backups(self: Pin<&mut Backend>);

        #[qinvokable]
        #[cxx_name = "checkExportSpace"]
        fn check_export_space(
            self: &Backend,
            name: &QString,
            include_workspace: bool,
        ) -> QMap_QString_QVariant;

        #[qinvokable]
        #[cxx_name = "exportEnvironment"]
        fn export_environment(
            self: Pin<&mut Backend>,
            name: &QString,
            include_workspace: bool,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "importEnvironment"]
        fn import_environment(
            self: Pin<&mut Backend>,
            backup_file: &QString,
            new_name: &QString,
            workspace_archive_file: &QString,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "deleteBackup"]
        fn delete_backup(
            self: Pin<&mut Backend>,
            backup_file: &QString,
            workspace_archive_file: &QString,
        );

        #[qinvokable]
        #[cxx_name = "readOperationLog"]
        fn read_operation_log(self: &Backend, id: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "clearOperationHistory"]
        fn clear_operation_history(self: Pin<&mut Backend>);

        #[qinvokable]
        #[cxx_name = "acknowledgeOperationHistory"]
        fn acknowledge_operation_history(self: Pin<&mut Backend>);

        #[qinvokable]
        #[cxx_name = "copyAiCommand"]
        fn copy_ai_command(self: Pin<&mut Backend>, name: &QString);

        #[qinvokable]
        #[cxx_name = "copyText"]
        fn copy_text(self: Pin<&mut Backend>, text: &QString, label: &QString) -> bool;

        #[qinvokable]
        #[cxx_name = "pasteImageToWorkspace"]
        fn paste_image_to_workspace(self: Pin<&mut Backend>, name: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "saveSecret"]
        fn save_secret(self: Pin<&mut Backend>, name: &QString, value: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "deleteSecret"]
        fn delete_secret(self: Pin<&mut Backend>, name: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "copySecret"]
        fn copy_secret(self: Pin<&mut Backend>, name: &QString) -> bool;

        #[qinvokable]
        #[cxx_name = "reloadSecrets"]
        fn reload_secrets(self: Pin<&mut Backend>);

        #[qinvokable]
        #[cxx_name = "setEnvironmentSecrets"]
        fn set_environment_secrets(
            self: Pin<&mut Backend>,
            name: &QString,
            secret_names: &QStringList,
        );

        #[qinvokable]
        #[cxx_name = "reloadEnvironmentSecrets"]
        fn reload_environment_secrets(self: Pin<&mut Backend>, env_name: &QString);

        #[qinvokable]
        #[cxx_name = "saveEnvironmentSecret"]
        fn save_environment_secret(
            self: Pin<&mut Backend>,
            env_name: &QString,
            name: &QString,
            value: &QString,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "deleteEnvironmentSecret"]
        fn delete_environment_secret(
            self: Pin<&mut Backend>,
            env_name: &QString,
            name: &QString,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "copyEnvironmentSecret"]
        fn copy_environment_secret(
            self: Pin<&mut Backend>,
            env_name: &QString,
            name: &QString,
        ) -> bool;

        #[qinvokable]
        #[cxx_name = "setEnvironmentLocalSecretEnabled"]
        fn set_environment_local_secret_enabled(
            self: Pin<&mut Backend>,
            env_name: &QString,
            name: &QString,
            enabled: bool,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "setGlobalSecretInstanceVisible"]
        fn set_global_secret_instance_visible(
            self: Pin<&mut Backend>,
            name: &QString,
            visible: bool,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "poolWarningForCreate"]
        fn pool_warning_for_create(self: &Backend, disk_spec: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "poolWarningForQuotaEdit"]
        fn pool_warning_for_quota_edit(
            self: &Backend,
            env_name: &QString,
            disk_spec: &QString,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "validateStoragePoolResize"]
        fn validate_storage_pool_resize(self: &Backend, size_spec: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "resizeStoragePool"]
        fn resize_storage_pool(self: Pin<&mut Backend>, size_spec: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "saveStaleSettings"]
        fn save_stale_settings(self: Pin<&mut Backend>, days: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "refreshStaleReport"]
        fn refresh_stale_report(self: Pin<&mut Backend>);

        #[qinvokable]
        #[cxx_name = "exportAndWipeEnvironment"]
        fn export_and_wipe_environment(
            self: Pin<&mut Backend>,
            name: &QString,
            include_workspace: bool,
            delete_workspace_files: bool,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "openOrphanFolder"]
        fn open_orphan_folder(self: Pin<&mut Backend>, path: &QString);

        #[qinvokable]
        #[cxx_name = "deleteOrphan"]
        fn delete_orphan(self: Pin<&mut Backend>, kind: &QString, path: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "deleteOrphanAsAdmin"]
        fn delete_orphan_as_admin(
            self: Pin<&mut Backend>,
            kind: &QString,
            path: &QString,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "saveSessionRecordingSettings"]
        fn save_session_recording_settings(self: Pin<&mut Backend>, enabled: bool) -> QString;

        #[qinvokable]
        #[cxx_name = "setEnvironmentRecordSessions"]
        fn set_environment_record_sessions(self: Pin<&mut Backend>, name: &QString, mode: &QString);

        #[qinvokable]
        #[cxx_name = "refreshRecordedSessions"]
        fn refresh_recorded_sessions(self: Pin<&mut Backend>);

        #[qinvokable]
        #[cxx_name = "readRecordedSessionLog"]
        fn read_recorded_session_log(self: &Backend, log_path: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "replayRecordedSession"]
        fn replay_recorded_session(
            self: Pin<&mut Backend>,
            log_path: &QString,
            timing_path: &QString,
        );

        #[qinvokable]
        #[cxx_name = "deleteRecordedSession"]
        fn delete_recorded_session(
            self: Pin<&mut Backend>,
            log_path: &QString,
            timing_path: &QString,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "downloadOsImage"]
        fn download_os_image(self: Pin<&mut Backend>, os_id: &QString);

        #[qinvokable]
        #[cxx_name = "removeOsImage"]
        fn remove_os_image(self: Pin<&mut Backend>, os_id: &QString);

        #[qinvokable]
        #[cxx_name = "completeFirstRun"]
        fn complete_first_run(self: Pin<&mut Backend>, system_path: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "saveSettings"]
        fn save_settings(
            self: Pin<&mut Backend>,
            workspace_path: &QString,
            os_storage_path: &QString,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "saveOsEntry"]
        fn save_os_entry(
            self: Pin<&mut Backend>,
            original_id: &QString,
            id: &QString,
            alias: &QString,
            name: &QString,
            badge: &QString,
            description: &QString,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "removeOsEntry"]
        fn remove_os_entry(self: Pin<&mut Backend>, id: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "resetOsCatalog"]
        fn reset_os_catalog(self: Pin<&mut Backend>) -> QString;

        #[qinvokable]
        #[cxx_name = "saveCreateOptions"]
        fn save_create_options(
            self: Pin<&mut Backend>,
            disk_csv: &QString,
            ram_csv: &QString,
            cpu_csv: &QString,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "savePreset"]
        fn save_preset(
            self: Pin<&mut Backend>,
            id: &QString,
            title: &QString,
            subtitle: &QString,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "resetCreateOptions"]
        fn reset_create_options(self: Pin<&mut Backend>) -> QString;

        #[qinvokable]
        #[cxx_name = "saveAutoSnapshotSettings"]
        fn save_auto_snapshot_settings(
            self: Pin<&mut Backend>,
            enabled: bool,
            keep_count: i32,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "saveWatchdogSettings"]
        fn save_watchdog_settings(
            self: Pin<&mut Backend>,
            enabled: bool,
            disk_warn: i32,
            disk_critical: i32,
            auto_stop: bool,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "saveIdleStopSettings"]
        fn save_idle_stop_settings(self: Pin<&mut Backend>, minutes: i32) -> QString;
    }

    impl cxx_qt::Threading for Backend {}
    impl cxx_qt::Initialize for Backend {}
}

pub struct BackendRust {
    count: i32,
    environments: QListQVariant,
    os_catalog: QListQVariant,
    pub(crate) selected_index: i32,
    busy: bool,
    task_progress: i32,
    task_stage: QString,
    task_log: QString,
    pending_env_name: QString,
    status_message: QString,
    error_message: QString,
    pub(crate) downloading_os_id: QString,
    system_path: QString,
    default_system_path: QString,
    workspace_base: QString,
    container_workspace_path: QString,
    os_storage_path: QString,
    pub(crate) default_workspace_base: QString,
    pub(crate) default_os_storage_path: QString,
    config_file_path: QString,
    first_run: bool,
    presets: QListQVariant,
    disk_options: QStringList,
    ram_options: QStringList,
    cpu_options: QListQVariant,
    default_preset_id: QString,
    default_os_id: QString,
    default_disk: QString,
    default_ram: QString,
    default_cpu: i32,
    auto_snapshot_enabled: bool,
    auto_snapshot_keep: i32,
    process_limit_options: QStringList,
    io_limit_options: QStringList,
    default_process_limit: QString,
    default_io_limit: QString,
    watchdog_enabled: bool,
    watchdog_disk_warn: i32,
    watchdog_disk_critical: i32,
    watchdog_auto_stop: bool,
    idle_stop_minutes: i32,
    idle_stop_options: QListQVariant,
    stale_days: i32,
    stale_day_options: QListQVariant,
    pub(crate) stale_report: QMapQStringQVariant,
    record_sessions: bool,
    recorded_sessions: QListQVariant,
    pub(crate) operation_history: QListQVariant,
    pub(crate) egress_logs: QListQVariant,
    pub(crate) egress_logs_env_name: QString,
    pub(crate) backup_archives: QListQVariant,
    diff_excludes: QStringList,
    pub(crate) diff_loading: bool,
    pub(crate) diff_entries: QListQVariant,
    pub(crate) diff_summary: QMapQStringQVariant,
    pub(crate) selected_diff_text: QString,
    pub(crate) selected_diff_info: QString,
    pub(crate) secrets_error: QString,
    secrets_store_path: QString,
    pub(crate) environment_secrets: QListQVariant,
    pub(crate) environment_secrets_error: QString,
    pub(crate) environment_secrets_env_name: QString,
    pool_info: QMapQStringQVariant,

    // Non-QML internal state
    pub(crate) engine: Engine,
    pub(crate) project_mode: bool,
    pub(crate) remember_system_location: bool,
    pub(crate) demo_mode: bool,
    demo_environments: Vec<serde_json::Value>,
    pub(crate) history_seen_id: String,
    last_images_json: Vec<u8>,
    pub(crate) last_list_json: Vec<u8>,
    last_refresh_at: Option<Instant>,
    refresh_in_flight: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
    poll_started: bool,
    task_watch_active: Arc<AtomicBool>,
    pub(crate) diff_generation: Arc<AtomicU64>,
}

impl Default for BackendRust {
    fn default() -> Self {
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());
        let engine = Engine::new(None, true, &cwd);
        let default_sys = default_system_path();
        let default_ws = default_workspace_path();
        let default_os = default_os_storage_path();

        Self {
            count: 0,
            environments: QListQVariant::default(),
            os_catalog: QListQVariant::default(),
            selected_index: 0,
            busy: false,
            task_progress: 0,
            task_stage: QString::default(),
            task_log: QString::default(),
            pending_env_name: QString::default(),
            status_message: QString::default(),
            error_message: QString::default(),
            downloading_os_id: QString::default(),
            system_path: QString::from(engine.paths.system_path.as_str()),
            default_system_path: QString::from(default_sys.as_str()),
            workspace_base: QString::from(engine.config.workspace_base.as_str()),
            container_workspace_path: QString::from(CONTAINER_WORKSPACE_PATH),
            os_storage_path: QString::from(engine.config.os_storage_path.as_str()),
            default_workspace_base: QString::from(default_ws.as_str()),
            default_os_storage_path: QString::from(default_os.as_str()),
            config_file_path: QString::from(engine.paths.config_file_path.as_str()),
            first_run: !engine.config.first_run_completed,
            presets: serialize_to_qvariant_list(&default_presets()),
            disk_options: strings_to_qstringlist(&default_disk_options()),
            ram_options: strings_to_qstringlist(&default_ram_options()),
            cpu_options: i32s_to_qvariant_list(&default_cpu_options()),
            default_preset_id: QString::from("full"),
            default_os_id: QString::from("ubuntu-24.04"),
            default_disk: QString::from("35GiB"),
            default_ram: QString::from("6GiB"),
            default_cpu: 4,
            auto_snapshot_enabled: true,
            auto_snapshot_keep: 5,
            process_limit_options: strings_to_qstringlist(&[
                "2048".to_string(),
                "4096".to_string(),
                "8192".to_string(),
                "Unlimited".to_string(),
            ]),
            io_limit_options: strings_to_qstringlist(&[
                "Unlimited".to_string(),
                "50MB".to_string(),
                "100MB".to_string(),
                "250MB".to_string(),
            ]),
            default_process_limit: QString::from("4096"),
            default_io_limit: QString::from("Unlimited"),
            watchdog_enabled: true,
            watchdog_disk_warn: 90,
            watchdog_disk_critical: 97,
            watchdog_auto_stop: false,
            idle_stop_minutes: 0,
            idle_stop_options: i32s_to_qvariant_list(&[0, 15, 30, 60, 120]),
            stale_days: 30,
            stale_day_options: i32s_to_qvariant_list(&stale_day_options()),
            stale_report: QMapQStringQVariant::default(),
            record_sessions: false,
            recorded_sessions: QListQVariant::default(),
            operation_history: QListQVariant::default(),
            egress_logs: QListQVariant::default(),
            egress_logs_env_name: QString::default(),
            backup_archives: QListQVariant::default(),
            diff_excludes: strings_to_qstringlist(&default_diff_excludes()),
            diff_loading: false,
            diff_entries: QListQVariant::default(),
            diff_summary: QMapQStringQVariant::default(),
            selected_diff_text: QString::default(),
            selected_diff_info: QString::default(),
            secrets_error: QString::default(),
            secrets_store_path: QString::from(engine.secrets.secrets_path.as_str()),
            environment_secrets: QListQVariant::default(),
            environment_secrets_error: QString::default(),
            environment_secrets_env_name: QString::default(),
            pool_info: QMapQStringQVariant::default(),
            engine,
            project_mode: true,
            remember_system_location: false,
            demo_mode: false,
            demo_environments: Vec::new(),
            history_seen_id: String::new(),
            last_images_json: Vec::new(),
            last_list_json: Vec::new(),
            last_refresh_at: None,
            refresh_in_flight: Arc::new(AtomicBool::new(false)),
            stop_flag: Arc::new(AtomicBool::new(false)),
            poll_started: false,
            task_watch_active: Arc::new(AtomicBool::new(false)),
            diff_generation: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl Drop for BackendRust {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }
}

impl cxx_qt::Initialize for qobject::Backend {
    fn initialize(self: Pin<&mut Self>) {}
}

impl qobject::Backend {
    pub fn selected_environment(&self) -> QMapQStringQVariant {
        if self.selected_index >= 0 {
            if self.demo_mode {
                if let Some(serde_json::Value::Object(map)) =
                    self.demo_environments.get(self.selected_index as usize)
                {
                    return json_object_to_qvariant_map(map);
                }
            } else if let Some(env) = self.engine.environments.get(self.selected_index as usize) {
                return serialize_to_qvariant_map(env);
            }
        }
        QMapQStringQVariant::default()
    }

    pub fn templates(&self) -> QListQVariant {
        if self.demo_mode {
            let tmpls: Vec<serde_json::Value> = self
                .demo_environments
                .iter()
                .filter(|v| v.get("isTemplate").and_then(|b| b.as_bool()) == Some(true))
                .cloned()
                .collect();
            return json_array_to_qvariant_list(&tmpls);
        }
        let tmpls: Vec<_> = extract_templates(&self.engine.environments)
            .into_iter()
            .cloned()
            .collect();
        serialize_to_qvariant_list(&tmpls)
    }

    pub fn recent_failed_history_count(&self) -> i32 {
        let history = load_operation_history(&self.engine.paths.state_dir);
        recent_failed_history_count(&history, &self.history_seen_id)
    }

    pub fn backups_dir_path(&self) -> QString {
        QString::from(
            aibox_core::backup::backups_dir_path(&self.engine.config.os_storage_path).as_str(),
        )
    }

    pub fn secrets_list(&self) -> QListQVariant {
        let list = self.engine.secrets.secrets_list();
        serialize_to_qvariant_list(&list)
    }

    pub fn configure(mut self: Pin<&mut Self>, path: &QString, project_mode: bool) {
        let path_str = path.to_string();
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());
        let path_opt = if path_str.trim().is_empty() {
            None
        } else {
            Some(path_str.as_str())
        };
        let demo_mode = std::env::var("AIBOX_DEMO_MODE")
            .map(|v| v.trim() == "1")
            .unwrap_or(false);
        let mut engine = Engine::new(path_opt, project_mode, &cwd);
        engine.secrets.reload_secrets(demo_mode);

        let remember_system_location = !project_mode && path_opt.is_none();
        {
            let mut rust = self.as_mut().rust_mut();
            rust.engine = engine;
            rust.project_mode = project_mode;
            rust.remember_system_location = remember_system_location;
            rust.demo_mode = demo_mode;
        }

        self.as_mut().sync_config_properties();
        self.as_mut().rebuild_os_catalog();

        let state_dir = self.engine.paths.state_dir.clone();
        let seen_id = load_history_seen_marker(&state_dir);
        let history = load_operation_history(&state_dir);
        {
            let mut rust = self.as_mut().rust_mut();
            rust.history_seen_id = seen_id;
            rust.operation_history = json_array_to_qvariant_list(&history);
        }
        self.as_mut().operation_history_changed();

        if let Ok(idx_str) = std::env::var("AIBOX_SELECT_INDEX") {
            if let Ok(idx) = idx_str.trim().parse::<i32>() {
                self.as_mut().set_selected_index(idx);
            }
        }

        if self.demo_mode {
            {
                let mut rust = self.as_mut().rust_mut();
                rust.first_run = false;
            }
            self.as_mut().first_run_changed();
            self.as_mut().load_demo_data();
            return;
        }

        if !self.first_run {
            let _ = std::fs::create_dir_all(&self.engine.config.workspace_base);
            let _ = std::fs::create_dir_all(&self.engine.config.os_storage_path);
            let _ = std::fs::create_dir_all(&self.engine.paths.state_dir);
            self.as_mut().refresh_backups();
            self.as_mut().refresh_recorded_sessions();
        }

        if find_executable("incus").is_some() {
            self.as_mut().start_background_workers();
            self.as_mut().refresh();
        }
    }

    pub(crate) fn sync_config_properties(mut self: Pin<&mut Self>) {
        let (
            system_path,
            workspace_base,
            os_storage_path,
            config_file_path,
            first_run,
            presets,
            disk_options,
            ram_options,
            cpu_options,
            default_preset_id,
            default_os_id,
            default_disk,
            default_ram,
            default_cpu,
            auto_snapshot_enabled,
            auto_snapshot_keep,
            watchdog_enabled,
            watchdog_disk_warn,
            watchdog_disk_critical,
            watchdog_auto_stop,
            idle_stop_minutes,
            stale_days,
            record_sessions,
            diff_excludes,
            secrets_store_path,
            secrets_error,
        ) = {
            let cfg = &self.engine.config;
            let paths = &self.engine.paths;
            (
                QString::from(paths.system_path.as_str()),
                QString::from(cfg.workspace_base.as_str()),
                QString::from(cfg.os_storage_path.as_str()),
                QString::from(paths.config_file_path.as_str()),
                !cfg.first_run_completed,
                serialize_to_qvariant_list(&cfg.presets),
                strings_to_qstringlist(&cfg.disk_options),
                strings_to_qstringlist(&cfg.ram_options),
                i32s_to_qvariant_list(&cfg.cpu_options),
                QString::from(cfg.default_preset_id.as_str()),
                QString::from(cfg.default_os_id.as_str()),
                QString::from(cfg.default_disk.as_str()),
                QString::from(cfg.default_ram.as_str()),
                cfg.default_cpu,
                cfg.auto_snapshot_enabled,
                cfg.auto_snapshot_keep,
                cfg.watchdog_enabled,
                cfg.watchdog_disk_warn,
                cfg.watchdog_disk_critical,
                cfg.watchdog_auto_stop,
                cfg.idle_stop_minutes,
                cfg.stale_days,
                cfg.record_sessions,
                strings_to_qstringlist(&cfg.diff_excludes),
                QString::from(self.engine.secrets.secrets_path.as_str()),
                QString::from(self.engine.secrets.secrets_error.as_str()),
            )
        };

        {
            let mut rust = self.as_mut().rust_mut();
            rust.system_path = system_path;
            rust.workspace_base = workspace_base;
            rust.os_storage_path = os_storage_path;
            rust.config_file_path = config_file_path;
            rust.first_run = first_run;
            rust.presets = presets;
            rust.disk_options = disk_options;
            rust.ram_options = ram_options;
            rust.cpu_options = cpu_options;
            rust.default_preset_id = default_preset_id;
            rust.default_os_id = default_os_id;
            rust.default_disk = default_disk;
            rust.default_ram = default_ram;
            rust.default_cpu = default_cpu;
            rust.auto_snapshot_enabled = auto_snapshot_enabled;
            rust.auto_snapshot_keep = auto_snapshot_keep;
            rust.watchdog_enabled = watchdog_enabled;
            rust.watchdog_disk_warn = watchdog_disk_warn;
            rust.watchdog_disk_critical = watchdog_disk_critical;
            rust.watchdog_auto_stop = watchdog_auto_stop;
            rust.idle_stop_minutes = idle_stop_minutes;
            rust.stale_days = stale_days;
            rust.record_sessions = record_sessions;
            rust.diff_excludes = diff_excludes;
            rust.secrets_store_path = secrets_store_path;
            rust.secrets_error = secrets_error;
        }

        self.as_mut().system_path_changed();
        self.as_mut().workspace_base_changed();
        self.as_mut().os_storage_path_changed();
        self.as_mut().config_file_path_changed();
        self.as_mut().first_run_changed();
        self.as_mut().create_options_changed();
        self.as_mut().auto_snapshot_settings_changed();
        self.as_mut().watchdog_settings_changed();
        self.as_mut().idle_stop_settings_changed();
        self.as_mut().stale_report_settings_changed();
        self.as_mut().record_sessions_settings_changed();
        self.as_mut().diff_excludes_changed();
        self.as_mut().secrets_changed();
    }

    pub(crate) fn rebuild_os_catalog(mut self: Pin<&mut Self>) {
        let downloading = self.downloading_os_id.to_string();
        let catalog = catalog_for_entries(
            &self.engine.config.os_catalog,
            &self.last_images_json,
            &downloading,
            &self.engine.config.os_storage_path,
        );
        let q_catalog = serialize_to_qvariant_list(&catalog);
        {
            let mut rust = self.as_mut().rust_mut();
            rust.engine.os_catalog = catalog;
            rust.os_catalog = q_catalog;
        }
        self.as_mut().os_catalog_changed();
    }

    fn load_demo_data(mut self: Pin<&mut Self>) {
        let state = build_demo_state_now(self.record_sessions, &self.engine.config.os_storage_path);
        let q_envs = json_array_to_qvariant_list(&state.environments);
        let count = state.environments.len() as i32;
        let q_history = json_array_to_qvariant_list(&state.operation_history);
        let q_backups = json_array_to_qvariant_list(&state.backup_archives);
        let q_pool = match &state.pool_info {
            serde_json::Value::Object(map) => json_object_to_qvariant_map(map),
            _ => QMapQStringQVariant::default(),
        };
        let q_sessions = json_array_to_qvariant_list(&state.recorded_sessions);

        {
            let mut rust = self.as_mut().rust_mut();
            rust.demo_environments = state.environments;
            rust.engine.pool_info = StoragePoolInfo {
                valid: state.pool_valid,
                name: state.pool_name,
                driver: state.pool_driver,
                used_bytes: state.pool_used_bytes,
                total_bytes: state.pool_total_bytes,
                used_percent: state.pool_used_percent,
                size_text: state.pool_size_text,
                resizable: state.pool_resizable,
            };
            if rust.engine.secrets.secrets.is_empty() {
                rust.engine.secrets.secrets = state.default_secrets;
            }
            rust.environments = q_envs;
            rust.count = count;
            rust.operation_history = q_history;
            rust.backup_archives = q_backups;
            rust.pool_info = q_pool;
            rust.recorded_sessions = q_sessions;
        }

        self.as_mut().environments_changed();
        self.as_mut().count_changed();
        self.as_mut().selected_index_changed();
        self.as_mut().operation_history_changed();
        self.as_mut().backup_archives_changed();
        self.as_mut().pool_info_changed();
        self.as_mut().recorded_sessions_changed();
        self.as_mut().secrets_changed();
        self.as_mut().refresh_stale_report();
    }

    pub(crate) fn save_config(&self) -> Result<(), String> {
        aibox_core::config::model::save(&self.engine.paths.config_file_path, &self.engine.config)
    }

    pub(crate) fn set_status_message(mut self: Pin<&mut Self>, msg: &str) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.status_message = QString::from(msg);
        }
        self.as_mut().status_message_changed();
    }

    pub(crate) fn set_error_message(mut self: Pin<&mut Self>, msg: &str) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.error_message = QString::from(msg);
        }
        self.as_mut().error_message_changed();
    }

    pub(crate) fn copy_text_with_status(
        mut self: Pin<&mut Self>,
        text: &str,
        status_msg: &str,
    ) -> bool {
        if text.is_empty() {
            return false;
        }
        let q_text = QString::from(text);
        if !qobject::aibox_clipboard_set_text(&q_text) {
            return false;
        }
        self.as_mut().set_status_message(status_msg);
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(3000));
            let _ = qt_thread.queue(|mut qobject| {
                if !*qobject.busy() {
                    qobject.as_mut().set_status_message("");
                }
            });
        });
        true
    }

    pub(crate) fn workspace_path_for(&self, name: &str) -> String {
        let pairs: Vec<(&str, &str)> = self
            .engine
            .environments
            .iter()
            .map(|e| (e.name.as_str(), e.workspace_path.as_str()))
            .collect();
        aibox_core::util::paths::workspace_path_for(
            &self.engine.config.workspace_base,
            &pairs,
            name,
        )
    }

    pub(crate) fn run_shell_task(
        self: Pin<&mut Self>,
        stage: &str,
        _script: &str,
        _on_success_msg: &str,
        _pending_env: &str,
    ) {
        eprintln!("not yet ported (P17c): runShellTask: {stage}");
    }

    pub(crate) fn sync_environments_from_engine(mut self: Pin<&mut Self>) {
        let state_dir = self.engine.paths.state_dir.clone();
        let fa_inputs: Vec<FileAccessEnvInput> = self
            .engine
            .environments
            .iter()
            .map(|e| FileAccessEnvInput {
                name: e.name.clone(),
                running: e.running,
            })
            .collect();
        let fa_result = apply_file_access_state(&state_dir, &fa_inputs);
        let mut env_values: Vec<serde_json::Value> = self
            .engine
            .environments
            .iter()
            .map(|e| serde_json::to_value(e).unwrap_or(serde_json::Value::Null))
            .collect();
        for (val, fa) in env_values.iter_mut().zip(fa_result.envs.iter()) {
            if let Some(obj) = val.as_object_mut() {
                obj.insert("fileAccessEnabled".into(), serde_json::json!(fa.active));
                obj.insert("fileAccessPort".into(), serde_json::json!(fa.port));
                obj.insert("fileAccessUrl".into(), serde_json::json!(fa.url));
            }
        }
        let q_envs = json_array_to_qvariant_list(&env_values);
        let count = self.engine.environments.len() as i32;
        {
            let mut rust = self.as_mut().rust_mut();
            rust.environments = q_envs;
            rust.count = count;
        }
        self.as_mut().environments_changed();
        self.as_mut().count_changed();
        self.as_mut().selected_index_changed();
    }

    pub(crate) fn start_background_workers(mut self: Pin<&mut Self>) {
        if self.poll_started {
            return;
        }
        {
            let mut rust = self.as_mut().rust_mut();
            rust.poll_started = true;
        }

        // 4s refresh poll thread
        let qt_thread_poll = self.qt_thread();
        let stop_poll = Arc::clone(&self.stop_flag);
        std::thread::spawn(move || {
            while !stop_poll.load(Ordering::Relaxed) {
                for _ in 0..20 {
                    if stop_poll.load(Ordering::Relaxed) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(200));
                }
                let _ = qt_thread_poll.queue(|mut backend| {
                    if !backend.busy {
                        backend.as_mut().refresh();
                    }
                });
            }
        });

        // 250ms task watch & 30s diff timeout thread
        let qt_thread_watch = self.qt_thread();
        let stop_watch = Arc::clone(&self.stop_flag);
        let task_watch_active = Arc::clone(&self.task_watch_active);
        std::thread::spawn(move || {
            while !stop_watch.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(250));
                if stop_watch.load(Ordering::Relaxed) {
                    return;
                }
                if task_watch_active.load(Ordering::Relaxed) {
                    let _ = qt_thread_watch.queue(|backend| {
                        // Task watcher hook (wired in P17c)
                        let _ = backend;
                    });
                }
            }
        });
    }

    pub fn set_selected_index(mut self: Pin<&mut Self>, index: i32) {
        if index == self.selected_index {
            return;
        }
        {
            let mut rust = self.as_mut().rust_mut();
            rust.selected_index = index;
        }
        self.as_mut().selected_index_changed();
    }

    pub fn primary_action(mut self: Pin<&mut Self>) {
        let next_count = self.count + 1;
        {
            let mut rust = self.as_mut().rust_mut();
            rust.count = next_count;
        }
        self.as_mut().count_changed();
        if let Some(env) = self.engine.selected_environment(self.selected_index) {
            let name = QString::from(env.name.as_str());
            self.as_mut().enter_environment(&name);
        }
    }

    pub fn clear_error(mut self: Pin<&mut Self>) {
        if !self.error_message.is_empty() {
            {
                let mut rust = self.as_mut().rust_mut();
                rust.error_message = QString::default();
            }
            self.as_mut().error_message_changed();
        }
    }

    pub fn refresh(mut self: Pin<&mut Self>) {
        if self.demo_mode {
            return;
        }
        if self
            .refresh_in_flight
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let in_flight = Arc::clone(&self.refresh_in_flight);
        let qt_thread = self.qt_thread();
        let elapsed_wall_ns = self
            .last_refresh_at
            .map(|t| t.elapsed().as_nanos() as i64)
            .unwrap_or(0);
        {
            let mut rust = self.as_mut().rust_mut();
            rust.last_refresh_at = Some(Instant::now());
        }

        std::thread::spawn(move || {
            let script = "incus list --format=json 2>/dev/null; printf '\\0'; \
                          incus image list --format=json 2>/dev/null; printf '\\0'; \
                          (incus storage info default --bytes 2>/dev/null || incus storage info default 2>/dev/null); printf '\\0'; \
                          (incus storage get default size 2>/dev/null || true)";
            let output = Command::new("/bin/sh").arg("-c").arg(script).output();
            in_flight.store(false, Ordering::SeqCst);

            let Ok(out) = output else {
                return;
            };
            if !out.status.success() {
                return;
            }
            let stdout = out.stdout;
            let _ = qt_thread.queue(move |mut backend| {
                backend
                    .as_mut()
                    .apply_refresh_output(&stdout, elapsed_wall_ns);
            });
        });
    }

    fn apply_refresh_output(mut self: Pin<&mut Self>, raw: &[u8], elapsed_wall_ns: i64) {
        let mut parts = raw.split(|&b| b == 0);
        let list_json = parts.next().unwrap_or(&[]);
        let img_json = parts.next().unwrap_or(&[]);
        let storage_info_text = String::from_utf8_lossy(parts.next().unwrap_or(&[])).to_string();
        let pool_size_text = String::from_utf8_lossy(parts.next().unwrap_or(&[]))
            .trim()
            .to_string();

        let mut pool_info = parse_incus_storage_info(&storage_info_text);
        if pool_info.valid {
            pool_info.size_text = pool_size_text.clone();
            pool_info.resizable = !pool_size_text.is_empty();
        }
        let list_bytes_saved = list_json.to_vec();
        let list_str = String::from_utf8_lossy(list_json);
        let (allocated_bytes, unbounded_count) =
            sum_root_disk_quotas_bytes(&list_str, "", "default");
        let pool_json_str = pool_info.to_json(allocated_bytes, unbounded_count);
        let pool_json: serde_json::Value =
            serde_json::from_str(&pool_json_str).unwrap_or(serde_json::Value::Null);
        let q_pool = match &pool_json {
            serde_json::Value::Object(map) => json_object_to_qvariant_map(map),
            _ => QMapQStringQVariant::default(),
        };

        let defaults = ParseDefaults {
            auto_snapshot: self.auto_snapshot_enabled,
            auto_stop: self.watchdog_auto_stop,
            record_sessions: self.record_sessions,
        };

        let (envs_changed, _clamped_index) = {
            let mut rust = self.as_mut().rust_mut();
            rust.last_images_json = img_json.to_vec();
            rust.engine.pool_info = pool_info;
            rust.last_list_json = list_bytes_saved;
            rust.pool_info = q_pool;

            let workspace_base = rust.engine.config.workspace_base.clone();
            let state_dir = rust.engine.paths.state_dir.clone();
            let warn = rust.watchdog_disk_warn;
            let crit = rust.watchdog_disk_critical;

            let mut envs = parse_incus_list(
                list_json,
                &workspace_base,
                &mut rust.engine.counter_state,
                elapsed_wall_ns,
                defaults,
                true,
            );

            for env in &mut envs {
                env.disk_alert_level = if env.disk_percent >= crit {
                    2
                } else if env.disk_percent >= warn {
                    1
                } else {
                    0
                };
            }

            let fa_inputs: Vec<FileAccessEnvInput> = envs
                .iter()
                .map(|e| FileAccessEnvInput {
                    name: e.name.clone(),
                    running: e.running,
                })
                .collect();
            let fa_result = apply_file_access_state(&state_dir, &fa_inputs);

            let mut env_values: Vec<serde_json::Value> = envs
                .iter()
                .map(|e| serde_json::to_value(e).unwrap_or(serde_json::Value::Null))
                .collect();
            for (val, fa) in env_values.iter_mut().zip(fa_result.envs.iter()) {
                if let Some(obj) = val.as_object_mut() {
                    obj.insert("fileAccessEnabled".into(), serde_json::json!(fa.active));
                    obj.insert("fileAccessPort".into(), serde_json::json!(fa.port));
                    obj.insert("fileAccessUrl".into(), serde_json::json!(fa.url));
                }
            }

            let changed = envs != rust.engine.environments;
            let q_envs = json_array_to_qvariant_list(&env_values);
            if changed {
                rust.engine.environments = envs.clone();
                rust.environments = q_envs;
                rust.count = envs.len() as i32;
            }
            let max_idx = (envs.len() as i32 - 1).max(0);
            let clamped = if rust.selected_index >= envs.len() as i32 {
                max_idx
            } else {
                rust.selected_index
            };
            rust.selected_index = clamped;
            (changed, clamped)
        };

        self.as_mut().pool_info_changed();
        if envs_changed {
            self.as_mut().environments_changed();
            self.as_mut().count_changed();
            self.as_mut().selected_index_changed();
            if !self.stale_report.is_empty() {
                self.as_mut().refresh_stale_report();
            }
        }
        self.as_mut().rebuild_os_catalog();
    }

    pub fn path_to_url(&self, path: &QString) -> QUrl {
        let raw = path.to_string();
        let home = aibox_core::util::paths::home_dir()
            .to_string_lossy()
            .to_string();
        let mut p = if raw.trim().is_empty() {
            home.clone()
        } else {
            clean_path(&raw)
        };
        while !p.is_empty() && !Path::new(&p).exists() {
            let parent = Path::new(&p)
                .parent()
                .map(|pp| pp.to_string_lossy().to_string())
                .unwrap_or_default();
            if parent == p || parent.is_empty() {
                break;
            }
            p = parent;
        }
        if p.is_empty() || !Path::new(&p).exists() {
            p = home;
        }
        QUrl::from_local_file(&QString::from(p.as_str()))
    }

    pub fn local_path(&self, url: &QUrl) -> QString {
        url.to_local_file().unwrap_or_default()
    }

    pub fn environment_secrets_store_path(&self, env_name: &QString) -> QString {
        QString::from(
            aibox_core::secrets::store::environment_secrets_store_path(
                &self.engine.secrets.secrets_path,
                &env_name.to_string(),
            )
            .as_str(),
        )
    }

    pub fn admin_fix_command(&self, message: &QString) -> QString {
        QString::from(aibox_core::task::files::admin_fix_command(&message.to_string()).as_str())
    }

    pub fn refresh_backups(mut self: Pin<&mut Self>) {
        let dir = aibox_core::backup::backups_dir_path(&self.engine.config.os_storage_path);
        let archives = scan_backup_archives(&dir);
        let q_archives = json_array_to_qvariant_list(&archives);
        {
            let mut rust = self.as_mut().rust_mut();
            rust.backup_archives = q_archives;
        }
        self.as_mut().backup_archives_changed();
    }

    pub fn refresh_recorded_sessions(mut self: Pin<&mut Self>) {
        let dir = aibox_core::session::sessions_dir_path(&self.engine.paths.state_dir);
        let sessions = scan_recorded_sessions(&dir);
        let q_sessions = json_array_to_qvariant_list(&sessions);
        {
            let mut rust = self.as_mut().rust_mut();
            rust.recorded_sessions = q_sessions;
        }
        self.as_mut().recorded_sessions_changed();
    }

    pub fn refresh_stale_report(mut self: Pin<&mut Self>) {
        let envs_arr = if self.demo_mode {
            self.demo_environments.clone()
        } else {
            let envs_val = serde_json::to_value(&self.engine.environments).unwrap_or_default();
            envs_val.as_array().cloned().unwrap_or_default()
        };
        let backups_dir = aibox_core::backup::backups_dir_path(&self.engine.config.os_storage_path);
        let report = build_stale_report(
            &envs_arr,
            &self.engine.config.workspace_base,
            &backups_dir,
            self.stale_days,
            0,
        );
        let q_report = match report {
            serde_json::Value::Object(map) => json_object_to_qvariant_map(&map),
            _ => QMapQStringQVariant::default(),
        };
        {
            let mut rust = self.as_mut().rust_mut();
            rust.stale_report = q_report;
        }
        self.as_mut().stale_report_changed();
    }

    // ---- Stubs for P17b / P17c ----
    pub fn cancel_task(self: Pin<&mut Self>) {
        eprintln!("not yet ported: cancelTask");
    }
    pub fn enter_environment(self: Pin<&mut Self>, _name: &QString) {
        eprintln!("not yet ported: enterEnvironment");
    }
    pub fn read_operation_log(&self, _id: &QString) -> QString {
        eprintln!("not yet ported: readOperationLog");
        QString::default()
    }
    pub fn clear_operation_history(self: Pin<&mut Self>) {
        eprintln!("not yet ported: clearOperationHistory");
    }
    pub fn acknowledge_operation_history(self: Pin<&mut Self>) {
        eprintln!("not yet ported: acknowledgeOperationHistory");
    }
    pub fn set_environment_record_sessions(self: Pin<&mut Self>, _name: &QString, _mode: &QString) {
        eprintln!("not yet ported: setEnvironmentRecordSessions");
    }
    pub fn read_recorded_session_log(&self, _log_path: &QString) -> QString {
        eprintln!("not yet ported: readRecordedSessionLog");
        QString::default()
    }
    pub fn replay_recorded_session(
        self: Pin<&mut Self>,
        _log_path: &QString,
        _timing_path: &QString,
    ) {
        eprintln!("not yet ported: replayRecordedSession");
    }
    pub fn delete_recorded_session(
        self: Pin<&mut Self>,
        _log_path: &QString,
        _timing_path: &QString,
    ) -> QString {
        eprintln!("not yet ported: deleteRecordedSession");
        QString::default()
    }
}

pub(crate) fn spawn_detached(
    program: &str,
    args: &[&str],
    cwd: Option<&str>,
) -> Result<u32, String> {
    use std::os::unix::process::CommandExt;
    let mut cmd = Command::new(program);
    cmd.args(args);
    if let Some(dir) = cwd {
        if !dir.is_empty() {
            cmd.current_dir(dir);
        }
    }
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    // SAFETY: `setsid()` is async-signal-safe and detaches the child into its own session.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    cmd.spawn()
        .map(|child| child.id())
        .map_err(|e| e.to_string())
}

pub(crate) fn launch_command_in_terminal(shell_cmd: &str) {
    if find_executable("xdg-terminal-exec").is_some() {
        let _ = spawn_detached("xdg-terminal-exec", &["sh", "-c", shell_cmd], None);
    } else if find_executable("foot").is_some() {
        let _ = spawn_detached("foot", &["sh", "-c", shell_cmd], None);
    }
}

pub(crate) fn find_executable(name: &str) -> Option<String> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}
