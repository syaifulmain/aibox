use aibox_core::config::defaults::OsEntry;
use aibox_core::incus::catalog::{
    build_os_catalog_json, catalog_for_entries_json as core_catalog_for_entries_json,
};
use aibox_core::incus::demo::{build_demo_state_now, value_list_to_json, value_to_json};
use aibox_core::incus::list::{parse_json, CounterState, ParseDefaults};

pub struct CounterStateHandle {
    inner: CounterState,
}

#[cxx::bridge(namespace = "aibox::ffi")]
mod ffi {
    struct CpuSnapshotEntryFfi {
        name: String,
        usage_ns: i64,
    }

    struct NetCounterEntryFfi {
        name: String,
        rx_bytes: i64,
        tx_bytes: i64,
    }

    struct ParseIncusListResultFfi {
        envs_json: String,
        cpu_snapshots: Vec<CpuSnapshotEntryFfi>,
        net_counters: Vec<NetCounterEntryFfi>,
    }

    struct OsEntryFullFfi {
        id: String,
        alias: String,
        name: String,
        badge: String,
        description: String,
    }

    struct DemoSecretFfi {
        name: String,
        value: String,
    }

    struct DemoStateFfi {
        environments_json: String,
        operation_history_json: String,
        default_secrets: Vec<DemoSecretFfi>,
        backup_archives_json: String,
        pool_info_json: String,
        pool_valid: bool,
        pool_name: String,
        pool_driver: String,
        pool_used_bytes: i64,
        pool_total_bytes: i64,
        pool_used_percent: i32,
        pool_size_text: String,
        pool_resizable: bool,
        pool_allocated_bytes: i64,
        pool_unbounded_count: i32,
        recorded_sessions_json: String,
    }

    extern "Rust" {
        type CounterStateHandle;

        fn new_counter_state() -> Box<CounterStateHandle>;

        #[allow(clippy::too_many_arguments)]
        fn parse_incus_list_with_state(
            state: &mut CounterStateHandle,
            json: &[u8],
            workspace_base: &str,
            elapsed_wall_ns: i64,
            default_auto_snapshot: bool,
            default_auto_stop: bool,
            default_record_sessions: bool,
            track_cpu_and_net: bool,
        ) -> String;

        #[allow(clippy::too_many_arguments)]
        fn parse_incus_list_stateless(
            json: &[u8],
            workspace_base: &str,
            cpu_snapshots: &[CpuSnapshotEntryFfi],
            net_counters: &[NetCounterEntryFfi],
            elapsed_wall_ns: i64,
            default_auto_snapshot: bool,
            default_auto_stop: bool,
            default_record_sessions: bool,
            track_cpu_and_net: bool,
        ) -> ParseIncusListResultFfi;

        fn catalog_for_entries_json(
            entries: &[OsEntryFullFfi],
            images_json: &[u8],
            downloading_id: &str,
            storage_path: &str,
        ) -> String;

        fn build_os_catalog_json(images_json: &[u8], downloading_id: &str) -> String;

        fn build_demo_state_ffi(record_sessions: bool, os_storage_path: &str) -> DemoStateFfi;
    }
}

fn new_counter_state() -> Box<CounterStateHandle> {
    Box::new(CounterStateHandle {
        inner: CounterState::new(),
    })
}

#[allow(clippy::too_many_arguments)]
fn parse_incus_list_with_state(
    state: &mut CounterStateHandle,
    json: &[u8],
    workspace_base: &str,
    elapsed_wall_ns: i64,
    default_auto_snapshot: bool,
    default_auto_stop: bool,
    default_record_sessions: bool,
    track_cpu_and_net: bool,
) -> String {
    parse_json(
        json,
        workspace_base,
        &mut state.inner,
        elapsed_wall_ns,
        ParseDefaults {
            auto_snapshot: default_auto_snapshot,
            auto_stop: default_auto_stop,
            record_sessions: default_record_sessions,
        },
        track_cpu_and_net,
    )
}

#[allow(clippy::too_many_arguments)]
fn parse_incus_list_stateless(
    json: &[u8],
    workspace_base: &str,
    cpu_snapshots: &[ffi::CpuSnapshotEntryFfi],
    net_counters: &[ffi::NetCounterEntryFfi],
    elapsed_wall_ns: i64,
    default_auto_snapshot: bool,
    default_auto_stop: bool,
    default_record_sessions: bool,
    track_cpu_and_net: bool,
) -> ffi::ParseIncusListResultFfi {
    let mut state = CounterState::new();
    for entry in cpu_snapshots {
        state
            .cpu_snapshot_ns
            .insert(entry.name.clone(), entry.usage_ns);
    }
    for entry in net_counters {
        state
            .net_counters
            .insert(entry.name.clone(), (entry.rx_bytes, entry.tx_bytes));
    }
    let envs_json = parse_json(
        json,
        workspace_base,
        &mut state,
        elapsed_wall_ns,
        ParseDefaults {
            auto_snapshot: default_auto_snapshot,
            auto_stop: default_auto_stop,
            record_sessions: default_record_sessions,
        },
        track_cpu_and_net,
    );
    let out_cpu = state
        .cpu_snapshot_ns
        .into_iter()
        .map(|(name, usage_ns)| ffi::CpuSnapshotEntryFfi { name, usage_ns })
        .collect();
    let out_net = state
        .net_counters
        .into_iter()
        .map(|(name, (rx_bytes, tx_bytes))| ffi::NetCounterEntryFfi {
            name,
            rx_bytes,
            tx_bytes,
        })
        .collect();
    ffi::ParseIncusListResultFfi {
        envs_json,
        cpu_snapshots: out_cpu,
        net_counters: out_net,
    }
}

fn catalog_for_entries_json(
    entries: &[ffi::OsEntryFullFfi],
    images_json: &[u8],
    downloading_id: &str,
    storage_path: &str,
) -> String {
    let core_entries: Vec<OsEntry> = entries
        .iter()
        .map(|e| OsEntry {
            id: e.id.clone(),
            alias: e.alias.clone(),
            name: e.name.clone(),
            badge: e.badge.clone(),
            description: e.description.clone(),
        })
        .collect();
    core_catalog_for_entries_json(&core_entries, images_json, downloading_id, storage_path)
}

fn build_demo_state_ffi(record_sessions: bool, os_storage_path: &str) -> ffi::DemoStateFfi {
    let state = build_demo_state_now(record_sessions, os_storage_path);
    ffi::DemoStateFfi {
        environments_json: value_list_to_json(&state.environments),
        operation_history_json: value_list_to_json(&state.operation_history),
        default_secrets: state
            .default_secrets
            .into_iter()
            .map(|(name, value)| ffi::DemoSecretFfi { name, value })
            .collect(),
        backup_archives_json: value_list_to_json(&state.backup_archives),
        pool_info_json: value_to_json(&state.pool_info),
        pool_valid: state.pool_valid,
        pool_name: state.pool_name,
        pool_driver: state.pool_driver,
        pool_used_bytes: state.pool_used_bytes,
        pool_total_bytes: state.pool_total_bytes,
        pool_used_percent: state.pool_used_percent,
        pool_size_text: state.pool_size_text,
        pool_resizable: state.pool_resizable,
        pool_allocated_bytes: state.pool_allocated_bytes,
        pool_unbounded_count: state.pool_unbounded_count,
        recorded_sessions_json: value_list_to_json(&state.recorded_sessions),
    }
}
