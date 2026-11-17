//! Localhost port forwarding validation, script builders, and port probing.

use crate::util::shell::shell_quote;
pub use crate::workspace::file_access::is_host_port_available;

/// Returns the Incus proxy device name for `host_port`.
pub fn proxy_device_name(host_port: i32) -> String {
    format!("fwd-{host_port}")
}

/// Builds the shell script to attach a localhost proxy device to an environment.
pub fn add_port_forward_script(name: &str, host_port: i32, guest_port: i32) -> String {
    let dev = proxy_device_name(host_port);
    let q_name = shell_quote(name);
    let q_dev = shell_quote(&dev);
    let q_listen = shell_quote(&format!("tcp:127.0.0.1:{host_port}"));
    let q_connect = shell_quote(&format!("tcp:127.0.0.1:{guest_port}"));
    format!(
        "set -euo pipefail\n\
         echo '@@STAGE:45:Adding localhost port forward ({host_port} -> {guest_port})...'\n\
         incus config device add {q_name} {q_dev} proxy listen={q_listen} connect={q_connect}\n\
         echo '@@STAGE:100:Port forward active.'"
    )
}

/// Builds the shell script to remove a port-forward device from an environment.
pub fn remove_port_forward_script(name: &str, device_name: &str) -> String {
    let q_name = shell_quote(name);
    let q_dev = shell_quote(device_name);
    format!(
        "set -euo pipefail\n\
         echo '@@STAGE:50:Removing port forward...'\n\
         incus config device remove {q_name} {q_dev}\n\
         echo '@@STAGE:100:Port forward removed.'"
    )
}

/// Validates parameters and host port availability for adding a port forward.
pub fn validate_add_port_forward(
    name: &str,
    busy: bool,
    host_port: i32,
    guest_port: i32,
    env_found: bool,
    existing_host_ports: &[i32],
) -> Result<(), String> {
    if name.is_empty() {
        return Err("Select an environment first.".to_string());
    }
    if busy {
        return Err("Another operation is currently in progress.".to_string());
    }
    if !(1024..=65535).contains(&host_port) {
        return Err("Host port must be between 1024 and 65535 (127.0.0.1 bind).".to_string());
    }
    if !(1..=65535).contains(&guest_port) {
        return Err("Container port must be between 1 and 65535.".to_string());
    }
    if !env_found {
        return Err(format!("Environment '{name}' was not found."));
    }
    if existing_host_ports.contains(&host_port) {
        return Err(format!(
            "Host port {host_port} is already forwarded on '{name}'."
        ));
    }
    if !is_host_port_available(host_port) {
        return Err(format!(
            "Host port 127.0.0.1:{host_port} is already in use on this machine."
        ));
    }
    Ok(())
}

/// Returns the localhost HTTP URL for an open forwarded port if `host_port` is valid.
pub fn open_forwarded_port_url(host_port: i32) -> Option<String> {
    if (1..=65535).contains(&host_port) {
        Some(format!("http://127.0.0.1:{host_port}"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    use super::*;

    #[test]
    fn port_forward_validation_and_conflict_detection_matches_qt_test() {
        assert_eq!(proxy_device_name(3000), "fwd-3000");
        assert!(!is_host_port_available(80));
        assert!(!is_host_port_available(70000));

        let blocker = TcpListener::bind(("127.0.0.1", 0)).expect("bind ephemeral port");
        let busy_port = i32::from(blocker.local_addr().unwrap().port());
        assert!(busy_port >= 1024);
        assert!(!is_host_port_available(busy_port));
        drop(blocker);
        assert!(is_host_port_available(busy_port));

        let script = add_port_forward_script("web-box", 3000, 8080);
        assert!(script
            .contains("'fwd-3000' proxy listen='tcp:127.0.0.1:3000' connect='tcp:127.0.0.1:8080'"));

        assert!(validate_add_port_forward("web-box", false, 80, 80, true, &[]).is_err());
        assert!(validate_add_port_forward("web-box", false, 3000, 0, true, &[]).is_err());
        assert!(validate_add_port_forward("web-box", false, 3000, 80, false, &[]).is_err());
        assert!(validate_add_port_forward("web-box", false, 3000, 80, true, &[3000]).is_err());
    }
}
