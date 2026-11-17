use aibox_core::network::{acl, allowlist, egress, ports};

#[cxx::bridge(namespace = "aibox::ffi")]
mod ffi {
    struct ResolveAllowlistResultFfi {
        cidrs: Vec<String>,
        warnings: Vec<String>,
    }

    struct NetworkTaskPlanFfi {
        description: String,
        script: String,
        error: String,
    }

    unsafe extern "C++" {
        include!("host_resolver.h");
        type HostResolverBridge;
        fn resolve(&self, host: &str) -> Vec<String>;
    }

    extern "Rust" {
        fn parse_allowlist_entries(raw_text: &str) -> Vec<String>;
        fn extract_allowlist_hostnames(entries: &[String]) -> Vec<String>;
        fn resolve_allowlist_cidrs(
            entries: &[String],
            has_resolver: bool,
            resolver: &HostResolverBridge,
        ) -> ResolveAllowlistResultFfi;
        fn acl_apply_shell_snippet(
            name: &str,
            allowlist_raw: &str,
            resolved_cidrs: &[String],
            egress_logging: bool,
        ) -> String;
        fn network_control_script(
            name: &str,
            mode: &str,
            allowlist_raw: &str,
            resolved_cidrs: &[String],
            egress_logging: bool,
        ) -> String;
        fn build_allowlist_refresh_if_needed(
            name: &str,
            network_mode: &str,
            network_allowlist: &str,
            egress_recording: bool,
        ) -> String;
        fn prepare_set_network_mode(
            name: &str,
            mode: &str,
            allowlist: &str,
            egress_recording: bool,
        ) -> NetworkTaskPlanFfi;
        fn prepare_set_egress_recording(
            name: &str,
            mode: &str,
            allowlist: &str,
            enabled: bool,
        ) -> NetworkTaskPlanFfi;
        fn add_egress_destination_allowlist(
            existing_allowlist: &str,
            destination_ip: &str,
        ) -> String;
        fn parse_kernel_egress_logs_json(
            journal_output: &str,
            container_ip: &str,
            host_iface: &str,
            max_entries: i32,
        ) -> String;
        fn proxy_device_name(host_port: i32) -> String;
        fn is_host_port_available(host_port: i32) -> bool;
        fn add_port_forward_script(name: &str, host_port: i32, guest_port: i32) -> String;
        fn remove_port_forward_script(name: &str, device_name: &str) -> String;
        fn validate_add_port_forward(
            name: &str,
            busy: bool,
            host_port: i32,
            guest_port: i32,
            env_found: bool,
            existing_host_ports: &[i32],
        ) -> String;
        fn open_forwarded_port_url(host_port: i32) -> String;
    }
}

struct FfiHostResolver<'a>(&'a ffi::HostResolverBridge);

impl allowlist::HostResolver for FfiHostResolver<'_> {
    fn resolve_ipv4(&self, host: &str) -> Vec<String> {
        self.0.resolve(host)
    }
}

fn parse_allowlist_entries(raw_text: &str) -> Vec<String> {
    allowlist::parse_allowlist_entries(raw_text)
}

fn extract_allowlist_hostnames(entries: &[String]) -> Vec<String> {
    allowlist::extract_allowlist_hostnames(entries)
}

fn resolve_allowlist_cidrs(
    entries: &[String],
    has_resolver: bool,
    resolver: &ffi::HostResolverBridge,
) -> ffi::ResolveAllowlistResultFfi {
    let (cidrs, warnings) = if has_resolver {
        allowlist::resolve_allowlist_cidrs(entries, &FfiHostResolver(resolver))
    } else {
        allowlist::resolve_allowlist_cidrs(entries, &allowlist::SystemResolver)
    };
    ffi::ResolveAllowlistResultFfi { cidrs, warnings }
}

fn acl_apply_shell_snippet(
    name: &str,
    allowlist_raw: &str,
    resolved_cidrs: &[String],
    egress_logging: bool,
) -> String {
    acl::acl_apply_shell_snippet(name, allowlist_raw, resolved_cidrs, egress_logging)
}

fn network_control_script(
    name: &str,
    mode: &str,
    allowlist_raw: &str,
    resolved_cidrs: &[String],
    egress_logging: bool,
) -> String {
    acl::network_control_script(name, mode, allowlist_raw, resolved_cidrs, egress_logging)
}

fn build_allowlist_refresh_if_needed(
    name: &str,
    network_mode: &str,
    network_allowlist: &str,
    egress_recording: bool,
) -> String {
    acl::build_allowlist_refresh_if_needed(
        name,
        network_mode,
        network_allowlist,
        egress_recording,
        &allowlist::SystemResolver,
    )
}

fn plan_to_ffi(res: Result<acl::NetworkTaskPlan, String>) -> ffi::NetworkTaskPlanFfi {
    match res {
        Ok(plan) => ffi::NetworkTaskPlanFfi {
            description: plan.description,
            script: plan.script,
            error: String::new(),
        },
        Err(error) => ffi::NetworkTaskPlanFfi {
            description: String::new(),
            script: String::new(),
            error,
        },
    }
}

fn prepare_set_network_mode(
    name: &str,
    mode: &str,
    allowlist: &str,
    egress_recording: bool,
) -> ffi::NetworkTaskPlanFfi {
    plan_to_ffi(acl::prepare_set_network_mode(
        name,
        mode,
        allowlist,
        egress_recording,
        &allowlist::SystemResolver,
    ))
}

fn prepare_set_egress_recording(
    name: &str,
    mode: &str,
    allowlist: &str,
    enabled: bool,
) -> ffi::NetworkTaskPlanFfi {
    plan_to_ffi(acl::prepare_set_egress_recording(
        name,
        mode,
        allowlist,
        enabled,
        &allowlist::SystemResolver,
    ))
}

fn add_egress_destination_allowlist(existing_allowlist: &str, destination_ip: &str) -> String {
    acl::add_egress_destination_allowlist(existing_allowlist, destination_ip).unwrap_or_default()
}

fn parse_kernel_egress_logs_json(
    journal_output: &str,
    container_ip: &str,
    host_iface: &str,
    max_entries: i32,
) -> String {
    egress::parse_kernel_egress_logs_json(journal_output, container_ip, host_iface, max_entries)
}

fn proxy_device_name(host_port: i32) -> String {
    ports::proxy_device_name(host_port)
}

fn is_host_port_available(host_port: i32) -> bool {
    ports::is_host_port_available(host_port)
}

fn add_port_forward_script(name: &str, host_port: i32, guest_port: i32) -> String {
    ports::add_port_forward_script(name, host_port, guest_port)
}

fn remove_port_forward_script(name: &str, device_name: &str) -> String {
    ports::remove_port_forward_script(name, device_name)
}

fn validate_add_port_forward(
    name: &str,
    busy: bool,
    host_port: i32,
    guest_port: i32,
    env_found: bool,
    existing_host_ports: &[i32],
) -> String {
    ports::validate_add_port_forward(
        name,
        busy,
        host_port,
        guest_port,
        env_found,
        existing_host_ports,
    )
    .err()
    .unwrap_or_default()
}

fn open_forwarded_port_url(host_port: i32) -> String {
    ports::open_forwarded_port_url(host_port).unwrap_or_default()
}
