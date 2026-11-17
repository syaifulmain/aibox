use std::collections::HashSet;

use aibox_core::workspace::{file_access, mounts, paste};

#[cxx::bridge(namespace = "aibox::ffi")]
mod ffi {
    struct FileAccessStateFfi {
        found: bool,
        name: String,
        pid: i64,
        port: i32,
        started_at: String,
    }

    struct FileAccessEnvInputFfi {
        name: String,
        running: bool,
    }

    struct FileAccessEnvOutputFfi {
        active: bool,
        port: i32,
        url: String,
    }

    struct PendingFileAccessKillFfi {
        pid: i64,
        name: String,
    }

    struct ApplyFileAccessResultFfi {
        envs: Vec<FileAccessEnvOutputFfi>,
        pending_kills: Vec<PendingFileAccessKillFfi>,
    }

    struct PreparedExtraMountFfi {
        error: String,
        resolved_source: String,
        device_name: String,
        container_path: String,
        script: String,
    }

    struct ExtraMountPairFfi {
        device_name: String,
        path: String,
    }

    struct StringResultFfi {
        value: String,
        error: String,
    }

    unsafe extern "C++" {
        include!("port_predicate.h");
        type PortPredicate;
        fn check(&self, port: i32) -> bool;
    }

    extern "Rust" {
        fn file_access_dir_path(state_dir: &str) -> String;
        fn file_access_state_path(state_dir: &str, name: &str) -> String;
        fn file_access_log_path(state_dir: &str, name: &str) -> String;
        fn load_file_access_state(state_dir: &str, name: &str) -> FileAccessStateFfi;
        fn file_access_process_alive(pid: i64, name: &str) -> bool;
        fn kill_file_access_if_alive(pid: i64, name: &str, signal: i32);
        fn stop_file_access_process(state_dir: &str, name: &str) -> i64;
        fn apply_file_access_state(
            state_dir: &str,
            envs: Vec<FileAccessEnvInputFfi>,
        ) -> ApplyFileAccessResultFfi;
        fn file_access_url(port: i32) -> String;
        fn choose_file_access_port(
            reserved_ports: &[i32],
            has_predicate: bool,
            predicate: &PortPredicate,
            start_port: i32,
            max_port: i32,
        ) -> i32;
        fn is_local_tcp_port_listening(port: i32) -> bool;
        fn validate_enable_file_access(
            name: &str,
            busy: bool,
            env_found: bool,
            is_template: bool,
            running: bool,
        ) -> String;
        fn collect_reserved_file_access_ports(state_dir: &str) -> Vec<i32>;
        fn build_file_access_mount_script(name: &str, port: i32, log_path: &str) -> String;
        fn write_file_access_state(
            state_dir: &str,
            name: &str,
            pid: i64,
            port: i32,
            started_at_iso: &str,
        ) -> String;
        fn read_file_access_failure_detail(log_path: &str) -> String;

        fn validate_extra_mount_path(raw_path: &str, forbidden_config_paths: &[String]) -> String;
        fn extra_mount_device_name(raw_name: &str) -> String;
        fn add_extra_mount_script(
            name: &str,
            device_name: &str,
            host_path: &str,
            container_path: &str,
            read_only: bool,
        ) -> String;
        #[allow(clippy::too_many_arguments)]
        fn prepare_add_extra_mount(
            name: &str,
            busy: bool,
            host_path: &str,
            mount_name: &str,
            read_only: bool,
            confirm_read_write: bool,
            forbidden_config_paths: &[String],
            env_found: bool,
            existing_mounts: Vec<ExtraMountPairFfi>,
        ) -> PreparedExtraMountFfi;
        fn remove_extra_mount_script(name: &str, device_name: &str) -> String;

        fn prepare_pasted_image_path(workspace_path: &str) -> StringResultFfi;
        fn prune_stale_pasted_images(workspace_path: &str, max_age_days: i64);
        fn pasted_image_container_path(container_workspace_path: &str, host_path: &str) -> String;
    }
}

fn file_access_dir_path(state_dir: &str) -> String {
    file_access::file_access_dir_path(state_dir)
}

fn file_access_state_path(state_dir: &str, name: &str) -> String {
    file_access::file_access_state_path(state_dir, name)
}

fn file_access_log_path(state_dir: &str, name: &str) -> String {
    file_access::file_access_log_path(state_dir, name)
}

fn load_file_access_state(state_dir: &str, name: &str) -> ffi::FileAccessStateFfi {
    match file_access::load_file_access_state(state_dir, name) {
        Some(s) => ffi::FileAccessStateFfi {
            found: true,
            name: s.name,
            pid: s.pid,
            port: s.port,
            started_at: s.started_at,
        },
        None => ffi::FileAccessStateFfi {
            found: false,
            name: String::new(),
            pid: 0,
            port: 0,
            started_at: String::new(),
        },
    }
}

fn file_access_process_alive(pid: i64, name: &str) -> bool {
    file_access::file_access_process_alive(pid, name)
}

fn kill_file_access_if_alive(pid: i64, name: &str, signal: i32) {
    file_access::kill_file_access_if_alive(pid, name, signal);
}

fn stop_file_access_process(state_dir: &str, name: &str) -> i64 {
    file_access::stop_file_access_process(state_dir, name).unwrap_or(0)
}

fn apply_file_access_state(
    state_dir: &str,
    envs: Vec<ffi::FileAccessEnvInputFfi>,
) -> ffi::ApplyFileAccessResultFfi {
    let inputs: Vec<file_access::FileAccessEnvInput> = envs
        .into_iter()
        .map(|e| file_access::FileAccessEnvInput {
            name: e.name,
            running: e.running,
        })
        .collect();
    let res = file_access::apply_file_access_state(state_dir, &inputs);
    ffi::ApplyFileAccessResultFfi {
        envs: res
            .envs
            .into_iter()
            .map(|e| ffi::FileAccessEnvOutputFfi {
                active: e.active,
                port: e.port,
                url: e.url,
            })
            .collect(),
        pending_kills: res
            .pending_kills
            .into_iter()
            .map(|k| ffi::PendingFileAccessKillFfi {
                pid: k.pid,
                name: k.name,
            })
            .collect(),
    }
}

fn file_access_url(port: i32) -> String {
    file_access::file_access_url(port)
}

fn choose_file_access_port(
    reserved_ports: &[i32],
    has_predicate: bool,
    predicate: &ffi::PortPredicate,
    start_port: i32,
    max_port: i32,
) -> i32 {
    let reserved: HashSet<i32> = reserved_ports.iter().copied().collect();
    if has_predicate {
        file_access::choose_file_access_port(
            &reserved,
            Some(|port| predicate.check(port)),
            start_port,
            max_port,
        )
    } else {
        file_access::choose_file_access_port::<fn(i32) -> bool>(
            &reserved, None, start_port, max_port,
        )
    }
}

fn is_local_tcp_port_listening(port: i32) -> bool {
    file_access::is_local_tcp_port_listening(port)
}

fn validate_enable_file_access(
    name: &str,
    busy: bool,
    env_found: bool,
    is_template: bool,
    running: bool,
) -> String {
    file_access::validate_enable_file_access(name, busy, env_found, is_template, running)
        .err()
        .unwrap_or_default()
}

fn collect_reserved_file_access_ports(state_dir: &str) -> Vec<i32> {
    file_access::collect_reserved_file_access_ports(state_dir)
}

fn build_file_access_mount_script(name: &str, port: i32, log_path: &str) -> String {
    file_access::build_file_access_mount_script(name, port, log_path)
}

fn write_file_access_state(
    state_dir: &str,
    name: &str,
    pid: i64,
    port: i32,
    started_at_iso: &str,
) -> String {
    file_access::write_file_access_state(state_dir, name, pid, port, started_at_iso)
        .err()
        .unwrap_or_default()
}

fn read_file_access_failure_detail(log_path: &str) -> String {
    file_access::read_file_access_failure_detail(log_path)
}

fn validate_extra_mount_path(raw_path: &str, forbidden_config_paths: &[String]) -> String {
    mounts::validate_extra_mount_path(raw_path, forbidden_config_paths)
        .err()
        .unwrap_or_default()
}

fn extra_mount_device_name(raw_name: &str) -> String {
    mounts::extra_mount_device_name(raw_name)
}

fn add_extra_mount_script(
    name: &str,
    device_name: &str,
    host_path: &str,
    container_path: &str,
    read_only: bool,
) -> String {
    mounts::add_extra_mount_script(name, device_name, host_path, container_path, read_only)
}

#[allow(clippy::too_many_arguments)]
fn prepare_add_extra_mount(
    name: &str,
    busy: bool,
    host_path: &str,
    mount_name: &str,
    read_only: bool,
    confirm_read_write: bool,
    forbidden_config_paths: &[String],
    env_found: bool,
    existing_mounts: Vec<ffi::ExtraMountPairFfi>,
) -> ffi::PreparedExtraMountFfi {
    let pairs: Vec<(String, String)> = existing_mounts
        .into_iter()
        .map(|m| (m.device_name, m.path))
        .collect();
    match mounts::prepare_add_extra_mount(
        name,
        busy,
        host_path,
        mount_name,
        read_only,
        confirm_read_write,
        forbidden_config_paths,
        env_found,
        &pairs,
    ) {
        Ok(p) => ffi::PreparedExtraMountFfi {
            error: String::new(),
            resolved_source: p.resolved_source,
            device_name: p.device_name,
            container_path: p.container_path,
            script: p.script,
        },
        Err(e) => ffi::PreparedExtraMountFfi {
            error: e,
            resolved_source: String::new(),
            device_name: String::new(),
            container_path: String::new(),
            script: String::new(),
        },
    }
}

fn remove_extra_mount_script(name: &str, device_name: &str) -> String {
    mounts::remove_extra_mount_script(name, device_name)
}

fn prepare_pasted_image_path(workspace_path: &str) -> ffi::StringResultFfi {
    match paste::prepare_pasted_image_path(workspace_path) {
        Ok(v) => ffi::StringResultFfi {
            value: v,
            error: String::new(),
        },
        Err(e) => ffi::StringResultFfi {
            value: String::new(),
            error: e,
        },
    }
}

fn prune_stale_pasted_images(workspace_path: &str, max_age_days: i64) {
    paste::prune_stale_pasted_images(workspace_path, max_age_days);
}

fn pasted_image_container_path(container_workspace_path: &str, host_path: &str) -> String {
    paste::pasted_image_container_path(container_workspace_path, host_path)
}
