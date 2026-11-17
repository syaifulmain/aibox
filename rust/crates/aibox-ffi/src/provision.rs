use aibox_core::config::defaults::{OsEntry, Preset};
use aibox_core::provision::{clone, create, lifecycle, resources};

#[cxx::bridge(namespace = "aibox::ffi")]
mod ffi {
    struct ProvisionTaskPlanFfi {
        ok: bool,
        error: String,
        name: String,
        title: String,
        script: String,
    }

    struct ToggleStartStopPlanFfi {
        ok: bool,
        error: String,
        is_stopping: bool,
        title: String,
        script: String,
    }

    struct OsEntryInputFfi {
        id: String,
        alias: String,
    }

    struct PresetInputFfi {
        id: String,
        flags: String,
        x11: bool,
    }

    extern "Rust" {
        fn update_resources_script(
            name: &str,
            cpu_cores: i32,
            ram_gib: &str,
            disk_gib: &str,
            process_limit: &str,
            io_limit: &str,
            is_vm: bool,
        ) -> String;

        #[allow(clippy::too_many_arguments)]
        fn validate_and_prepare_update_resources(
            name: &str,
            busy: bool,
            cpu_cores: i32,
            ram_gib: &str,
            disk_gib: &str,
            process_limit: &str,
            io_limit: &str,
            env_exists: bool,
            used_bytes: i64,
            disk_usage_label: &str,
            is_vm: bool,
        ) -> ProvisionTaskPlanFfi;

        fn download_os_image_script(
            os_id: &str,
            alias: &str,
            cache_base: &str,
        ) -> ProvisionTaskPlanFfi;
        fn remove_os_image_script(os_id: &str, cache_base: &str) -> ProvisionTaskPlanFfi;

        #[allow(clippy::too_many_arguments)]
        fn create_environment_script(
            name: &str,
            local_alias: &str,
            remote_image: &str,
            workspace: &str,
            cache_base: &str,
            profile_flags: &str,
            ram_gib: &str,
            disk_gib: &str,
            cpu_cores: i32,
            x11: bool,
            process_limit: &str,
            io_limit: &str,
            is_vm: bool,
        ) -> String;

        #[allow(clippy::too_many_arguments)]
        fn prepare_create_environment(
            busy: bool,
            raw_name: &str,
            existing_names: &[String],
            os_id: &str,
            os_entries: &[OsEntryInputFfi],
            preset_id: &str,
            default_preset_id: &str,
            presets: &[PresetInputFfi],
            disk_gib: &str,
            ram_gib: &str,
            cpu_cores: i32,
            process_limit: &str,
            io_limit: &str,
            workspace_parent: &str,
            workspace_base: &str,
            os_storage_path: &str,
        ) -> ProvisionTaskPlanFfi;

        #[allow(clippy::too_many_arguments)]
        fn clone_environment_script(
            source_name: &str,
            new_name: &str,
            workspace: &str,
            disk_gib: &str,
            ram_gib: &str,
            cpu_cores: i32,
            start_after: bool,
            net_mode: &str,
            net_allowlist: &str,
            resolved_cidrs: &[String],
            process_limit: &str,
            io_limit: &str,
            is_vm: bool,
        ) -> String;

        #[allow(clippy::too_many_arguments)]
        fn prepare_clone_environment(
            busy: bool,
            source_name: &str,
            source_exists: bool,
            source_net_mode: &str,
            source_net_allowlist: &str,
            source_type: &str,
            raw_new_name: &str,
            existing_names: &[String],
            disk_gib: &str,
            ram_gib: &str,
            cpu_cores: i32,
            start_after: bool,
            resolved_cidrs: &[String],
            process_limit: &str,
            io_limit: &str,
            workspace_parent: &str,
            workspace_base: &str,
        ) -> ProvisionTaskPlanFfi;

        fn build_prepare_script(name: &str, auto_snap: &str, allow_refresh: &str) -> String;
        fn start_environment_script(name: &str, auto_snap: &str, allow_refresh: &str) -> String;
        fn stop_environment_script(
            name: &str,
            include_auto_snapshot: bool,
            auto_snap: &str,
        ) -> String;

        fn enter_incus_exec_args(
            name: &str,
            remote_env_path: &str,
            container_workspace_path: &str,
            display: &str,
            term: &str,
        ) -> Vec<String>;

        fn exec_incus_command_args(
            name: &str,
            cmd: &[String],
            remote_env_path: &str,
            container_workspace_path: &str,
        ) -> Vec<String>;

        fn build_enter_command(name: &str, remote_env_path: &str) -> String;
        fn ai_exec_command_text(
            name: &str,
            has_secrets: bool,
            workspace_mount_path: &str,
        ) -> String;

        fn prepare_toggle_start_stop(
            name: &str,
            is_template: bool,
            is_running: bool,
            auto_snap: &str,
            allow_refresh: &str,
        ) -> ToggleStartStopPlanFfi;

        fn set_environment_template_script(
            name: &str,
            is_running: bool,
            is_template: bool,
        ) -> ProvisionTaskPlanFfi;
    }
}

fn err_plan(error: String) -> ffi::ProvisionTaskPlanFfi {
    ffi::ProvisionTaskPlanFfi {
        ok: false,
        error,
        name: String::new(),
        title: String::new(),
        script: String::new(),
    }
}

fn update_resources_script(
    name: &str,
    cpu_cores: i32,
    ram_gib: &str,
    disk_gib: &str,
    process_limit: &str,
    io_limit: &str,
    is_vm: bool,
) -> String {
    resources::update_resources_script(
        name,
        cpu_cores,
        ram_gib,
        disk_gib,
        process_limit,
        io_limit,
        is_vm,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_and_prepare_update_resources(
    name: &str,
    busy: bool,
    cpu_cores: i32,
    ram_gib: &str,
    disk_gib: &str,
    process_limit: &str,
    io_limit: &str,
    env_exists: bool,
    used_bytes: i64,
    disk_usage_label: &str,
    is_vm: bool,
) -> ffi::ProvisionTaskPlanFfi {
    match resources::validate_and_prepare_update_resources(
        name,
        busy,
        cpu_cores,
        ram_gib,
        disk_gib,
        process_limit,
        io_limit,
        env_exists,
        used_bytes,
        disk_usage_label,
        is_vm,
    ) {
        Ok(plan) => ffi::ProvisionTaskPlanFfi {
            ok: true,
            error: String::new(),
            name: name.to_string(),
            title: plan.title,
            script: plan.script,
        },
        Err(e) => err_plan(e),
    }
}

fn download_os_image_script(
    os_id: &str,
    alias: &str,
    cache_base: &str,
) -> ffi::ProvisionTaskPlanFfi {
    let plan = resources::download_os_image_script(os_id, alias, cache_base);
    ffi::ProvisionTaskPlanFfi {
        ok: true,
        error: String::new(),
        name: os_id.to_string(),
        title: plan.title,
        script: plan.script,
    }
}

fn remove_os_image_script(os_id: &str, cache_base: &str) -> ffi::ProvisionTaskPlanFfi {
    let plan = resources::remove_os_image_script(os_id, cache_base);
    ffi::ProvisionTaskPlanFfi {
        ok: true,
        error: String::new(),
        name: os_id.to_string(),
        title: plan.title,
        script: plan.script,
    }
}

#[allow(clippy::too_many_arguments)]
fn create_environment_script(
    name: &str,
    local_alias: &str,
    remote_image: &str,
    workspace: &str,
    cache_base: &str,
    profile_flags: &str,
    ram_gib: &str,
    disk_gib: &str,
    cpu_cores: i32,
    x11: bool,
    process_limit: &str,
    io_limit: &str,
    is_vm: bool,
) -> String {
    create::create_environment_script(
        name,
        local_alias,
        remote_image,
        workspace,
        cache_base,
        profile_flags,
        ram_gib,
        disk_gib,
        cpu_cores,
        x11,
        process_limit,
        io_limit,
        is_vm,
    )
}

#[allow(clippy::too_many_arguments)]
fn prepare_create_environment(
    busy: bool,
    raw_name: &str,
    existing_names: &[String],
    os_id: &str,
    os_entries: &[ffi::OsEntryInputFfi],
    preset_id: &str,
    default_preset_id: &str,
    presets: &[ffi::PresetInputFfi],
    disk_gib: &str,
    ram_gib: &str,
    cpu_cores: i32,
    process_limit: &str,
    io_limit: &str,
    workspace_parent: &str,
    workspace_base: &str,
    os_storage_path: &str,
) -> ffi::ProvisionTaskPlanFfi {
    let os_vec: Vec<OsEntry> = os_entries
        .iter()
        .map(|e| OsEntry {
            id: e.id.clone(),
            alias: e.alias.clone(),
            name: String::new(),
            badge: String::new(),
            description: String::new(),
        })
        .collect();
    let preset_vec: Vec<Preset> = presets
        .iter()
        .map(|p| Preset {
            id: p.id.clone(),
            title: String::new(),
            sub: String::new(),
            flags: p.flags.clone(),
            x11: p.x11,
        })
        .collect();

    match create::prepare_create_environment(
        busy,
        raw_name,
        existing_names,
        os_id,
        &os_vec,
        preset_id,
        default_preset_id,
        &preset_vec,
        disk_gib,
        ram_gib,
        cpu_cores,
        process_limit,
        io_limit,
        workspace_parent,
        workspace_base,
        os_storage_path,
    ) {
        Ok(plan) => ffi::ProvisionTaskPlanFfi {
            ok: true,
            error: String::new(),
            name: plan.name,
            title: plan.title,
            script: plan.script,
        },
        Err(e) => err_plan(e),
    }
}

#[allow(clippy::too_many_arguments)]
fn clone_environment_script(
    source_name: &str,
    new_name: &str,
    workspace: &str,
    disk_gib: &str,
    ram_gib: &str,
    cpu_cores: i32,
    start_after: bool,
    net_mode: &str,
    net_allowlist: &str,
    resolved_cidrs: &[String],
    process_limit: &str,
    io_limit: &str,
    is_vm: bool,
) -> String {
    clone::clone_environment_script(
        source_name,
        new_name,
        workspace,
        disk_gib,
        ram_gib,
        cpu_cores,
        start_after,
        net_mode,
        net_allowlist,
        resolved_cidrs,
        process_limit,
        io_limit,
        is_vm,
    )
}

#[allow(clippy::too_many_arguments)]
fn prepare_clone_environment(
    busy: bool,
    source_name: &str,
    source_exists: bool,
    source_net_mode: &str,
    source_net_allowlist: &str,
    source_type: &str,
    raw_new_name: &str,
    existing_names: &[String],
    disk_gib: &str,
    ram_gib: &str,
    cpu_cores: i32,
    start_after: bool,
    resolved_cidrs: &[String],
    process_limit: &str,
    io_limit: &str,
    workspace_parent: &str,
    workspace_base: &str,
) -> ffi::ProvisionTaskPlanFfi {
    let resolver = aibox_core::network::allowlist::StaticResolver::default();
    match clone::prepare_clone_environment(
        busy,
        source_name,
        source_exists,
        if source_net_mode == "allowlist" {
            "online"
        } else {
            source_net_mode
        },
        source_net_allowlist,
        source_type,
        raw_new_name,
        existing_names,
        disk_gib,
        ram_gib,
        cpu_cores,
        start_after,
        process_limit,
        io_limit,
        workspace_parent,
        workspace_base,
        &resolver,
    ) {
        Ok(mut plan) => {
            if source_net_mode == "allowlist" {
                let host_workspace = aibox_core::util::paths::resolve_workspace_path(
                    workspace_parent,
                    &plan.new_name,
                    workspace_base,
                )
                .unwrap_or_default();
                let is_vm = source_type.eq_ignore_ascii_case("VM");
                let clean_proc = {
                    let t = process_limit.trim();
                    if t.eq_ignore_ascii_case("unlimited") {
                        ""
                    } else {
                        t
                    }
                };
                let clean_io = {
                    let t = io_limit.trim();
                    if t.eq_ignore_ascii_case("unlimited") {
                        ""
                    } else {
                        t
                    }
                };
                plan.script = clone::clone_environment_script(
                    source_name,
                    &plan.new_name,
                    &host_workspace,
                    disk_gib,
                    ram_gib,
                    cpu_cores,
                    start_after,
                    source_net_mode,
                    source_net_allowlist,
                    resolved_cidrs,
                    clean_proc,
                    clean_io,
                    is_vm,
                );
            }
            ffi::ProvisionTaskPlanFfi {
                ok: true,
                error: String::new(),
                name: plan.new_name,
                title: plan.title,
                script: plan.script,
            }
        }
        Err(e) => err_plan(e),
    }
}

fn build_prepare_script(name: &str, auto_snap: &str, allow_refresh: &str) -> String {
    lifecycle::build_prepare_script(name, auto_snap, allow_refresh)
}

fn start_environment_script(name: &str, auto_snap: &str, allow_refresh: &str) -> String {
    lifecycle::start_environment_script(name, auto_snap, allow_refresh)
}

fn stop_environment_script(name: &str, include_auto_snapshot: bool, auto_snap: &str) -> String {
    lifecycle::stop_environment_script(name, include_auto_snapshot, auto_snap)
}

fn enter_incus_exec_args(
    name: &str,
    remote_env_path: &str,
    container_workspace_path: &str,
    display: &str,
    term: &str,
) -> Vec<String> {
    lifecycle::enter_incus_exec_args(
        name,
        remote_env_path,
        container_workspace_path,
        display,
        term,
    )
}

fn exec_incus_command_args(
    name: &str,
    cmd: &[String],
    remote_env_path: &str,
    container_workspace_path: &str,
) -> Vec<String> {
    lifecycle::exec_incus_command_args(name, cmd, remote_env_path, container_workspace_path)
}

fn build_enter_command(name: &str, remote_env_path: &str) -> String {
    lifecycle::build_enter_command(name, remote_env_path)
}

fn ai_exec_command_text(name: &str, has_secrets: bool, workspace_mount_path: &str) -> String {
    lifecycle::ai_exec_command_text(name, has_secrets, workspace_mount_path)
}

fn prepare_toggle_start_stop(
    name: &str,
    is_template: bool,
    is_running: bool,
    auto_snap: &str,
    allow_refresh: &str,
) -> ffi::ToggleStartStopPlanFfi {
    match lifecycle::prepare_toggle_start_stop(
        name,
        is_template,
        is_running,
        auto_snap,
        allow_refresh,
    ) {
        Ok(plan) => ffi::ToggleStartStopPlanFfi {
            ok: true,
            error: String::new(),
            is_stopping: plan.is_stopping,
            title: plan.title,
            script: plan.script,
        },
        Err(e) => ffi::ToggleStartStopPlanFfi {
            ok: false,
            error: e,
            is_stopping: false,
            title: String::new(),
            script: String::new(),
        },
    }
}

fn set_environment_template_script(
    name: &str,
    is_running: bool,
    is_template: bool,
) -> ffi::ProvisionTaskPlanFfi {
    let plan = lifecycle::set_environment_template_script(name, is_running, is_template);
    ffi::ProvisionTaskPlanFfi {
        ok: true,
        error: String::new(),
        name: name.to_string(),
        title: plan.title,
        script: plan.script,
    }
}
