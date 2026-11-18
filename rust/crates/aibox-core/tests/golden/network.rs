use crate::common;
use aibox_core::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn dispatch(fn_name: &str, _case: &str, args: &Value) -> Value {
    match fn_name {
        "Backend::parseAllowlistEntries" => {
            let raw = args["rawText"].as_str().expect("rawText");
            json!(network::allowlist::parse_allowlist_entries(raw))
        }
        "Backend::resolveAllowlistCidrs" => {
            let entries: Vec<String> = args["entries"]
                .as_array()
                .expect("entries")
                .iter()
                .map(|v| v.as_str().expect("str").to_string())
                .collect();
            let resolver = network::allowlist::StaticResolver::new(BTreeMap::from([
                (
                    "github.com".to_string(),
                    vec![
                        "140.82.121.4".to_string(),
                        "192.168.1.50".to_string(),
                        "140.82.121.4".to_string(),
                    ],
                ),
                (
                    "rebind.test".to_string(),
                    vec![
                        "0.0.0.1".to_string(),
                        "10.1.2.3".to_string(),
                        "100.64.0.1".to_string(),
                        "100.127.255.254".to_string(),
                        "127.0.0.1".to_string(),
                        "169.254.169.254".to_string(),
                        "172.16.0.1".to_string(),
                        "172.31.255.254".to_string(),
                        "192.168.0.1".to_string(),
                        "224.0.0.1".to_string(),
                        "255.255.255.255".to_string(),
                        "::1".to_string(),
                        "not-an-ip".to_string(),
                    ],
                ),
                (
                    "mixed.test".to_string(),
                    vec![
                        "100.63.255.254".to_string(),
                        "172.15.255.254".to_string(),
                        "172.32.0.1".to_string(),
                    ],
                ),
            ]));
            let (cidrs, warnings) =
                network::allowlist::resolve_allowlist_cidrs(&entries, &resolver);
            json!({ "cidrs": cidrs, "warnings": warnings })
        }
        "aclApplyShellSnippet" => {
            let name = args["name"].as_str().expect("name");
            let allowlist_raw = args["allowlistRaw"].as_str().expect("allowlistRaw");
            let resolved_cidrs: Vec<String> = args["resolvedCidrs"]
                .as_array()
                .expect("resolvedCidrs")
                .iter()
                .map(|v| v.as_str().expect("str").to_string())
                .collect();
            let egress_logging = args["egressLogging"].as_bool().expect("egressLogging");
            json!(network::acl::acl_apply_shell_snippet(
                name,
                allowlist_raw,
                &resolved_cidrs,
                egress_logging,
            ))
        }
        "Backend::networkControlScript" => {
            let name = args["name"].as_str().expect("name");
            let mode = args["mode"].as_str().expect("mode");
            let allowlist_raw = args["allowlistRaw"].as_str().expect("allowlistRaw");
            let resolved_cidrs: Vec<String> = args["resolvedCidrs"]
                .as_array()
                .expect("resolvedCidrs")
                .iter()
                .map(|v| v.as_str().expect("str").to_string())
                .collect();
            let egress_logging = args["egressLogging"].as_bool().expect("egressLogging");
            json!(network::acl::network_control_script(
                name,
                mode,
                allowlist_raw,
                &resolved_cidrs,
                egress_logging,
            ))
        }
        "Backend::parseKernelEgressLogs" => {
            let journal_output = args["journalOutput"].as_str().expect("journalOutput");
            let container_ip = args["containerIp"].as_str().expect("containerIp");
            let host_iface = args["hostIface"].as_str().expect("hostIface");
            let max_entries = args["maxEntries"].as_i64().expect("maxEntries") as i32;
            json!(network::egress::parse_kernel_egress_logs(
                journal_output,
                container_ip,
                host_iface,
                max_entries,
            ))
        }
        "Backend::proxyDeviceName" => {
            let host_port = args["hostPort"].as_i64().expect("hostPort") as i32;
            json!(network::ports::proxy_device_name(host_port))
        }
        "Backend::addPortForwardScript" => {
            let name = args["name"].as_str().expect("name");
            let host_port = args["hostPort"].as_i64().expect("hostPort") as i32;
            let guest_port = args["guestPort"].as_i64().expect("guestPort") as i32;
            json!(network::ports::add_port_forward_script(
                name, host_port, guest_port
            ))
        }
        "Backend::addPortForward" => {
            let name = args["name"].as_str().expect("name");
            let host_port = args["hostPort"].as_i64().expect("hostPort") as i32;
            let guest_port = args["guestPort"].as_i64().expect("guestPort") as i32;
            let res = network::ports::validate_add_port_forward(
                name,
                false,
                host_port,
                guest_port,
                false,
                &[],
            )
            .err()
            .unwrap_or_default();
            json!(res)
        }
        other => panic!("unregistered golden function: {other}"),
    }
}

#[test]
fn replay() {
    common::replay_jsonl("network", dispatch);
}
