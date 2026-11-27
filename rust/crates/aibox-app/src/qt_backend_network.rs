use std::pin::Pin;
use std::process::Command;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

use aibox_core::incus::list::environment_by_name;
use aibox_core::network::acl::{
    add_egress_destination_allowlist, prepare_set_egress_recording, prepare_set_network_mode,
};
use aibox_core::network::allowlist::{
    extract_allowlist_hostnames, parse_allowlist_entries, SystemResolver,
};
use aibox_core::network::egress::parse_kernel_egress_logs;
use aibox_core::network::ports::{
    add_port_forward_script_with_bind, local_domain_setup_hint, open_forwarded_port_url,
    remove_port_forward_script, validate_add_port_forward_with_bind,
};

use crate::qt_backend::{qobject, spawn_detached};
use crate::qt_backend_task::NoopResolver;
use crate::qt_convert::{json_array_to_qvariant_list, serialize_to_qvariant_list};

impl qobject::Backend {
    pub fn set_network_mode(
        mut self: Pin<&mut Self>,
        name: &QString,
        mode: &QString,
        allowlist: &QString,
    ) {
        let name_s = name.to_string();
        if name_s.is_empty() || *self.busy() {
            return;
        }
        let mode_s = mode.to_string();
        let allow_s = allowlist.to_string();
        let egress_logging = environment_by_name(&self.engine.environments, &name_s)
            .is_some_and(|e| e.egress_recording);

        let needs_dns = mode_s.trim().eq_ignore_ascii_case("allowlist")
            && !extract_allowlist_hostnames(&parse_allowlist_entries(&allow_s)).is_empty();
        if !needs_dns {
            match prepare_set_network_mode(
                &name_s,
                &mode_s,
                &allow_s,
                egress_logging,
                &NoopResolver,
            ) {
                Ok(plan) => {
                    self.as_mut()
                        .run_shell_task(&plan.description, &plan.script, "", &name_s);
                }
                Err(err) => {
                    self.as_mut().set_error_message(&err);
                }
            }
            return;
        }

        self.as_mut()
            .set_busy(true, "Resolving network allowlist...");
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let res = prepare_set_network_mode(
                &name_s,
                &mode_s,
                &allow_s,
                egress_logging,
                &SystemResolver,
            );
            let _ = qt_thread.queue(move |mut backend| {
                backend.as_mut().set_busy(false, "");
                match res {
                    Ok(plan) => {
                        backend.as_mut().run_shell_task(
                            &plan.description,
                            &plan.script,
                            "",
                            &name_s,
                        );
                    }
                    Err(err) => {
                        backend.as_mut().set_error_message(&err);
                    }
                }
            });
        });
    }

    pub fn refresh_network_allowlist(self: Pin<&mut Self>, name: &QString) {
        let name_s = name.to_string();
        let Some(env) = environment_by_name(&self.engine.environments, &name_s) else {
            return;
        };
        let mode = QString::from("allowlist");
        let allowlist = QString::from(env.network_allowlist.as_str());
        self.set_network_mode(name, &mode, &allowlist);
    }

    pub fn set_egress_recording(mut self: Pin<&mut Self>, name: &QString, enabled: bool) {
        let name_s = name.to_string();
        if name_s.is_empty() || *self.busy() {
            return;
        }
        let (net_mode, net_allowlist) = {
            let Some(env) = environment_by_name(&self.engine.environments, &name_s) else {
                return;
            };
            (env.network_mode.clone(), env.network_allowlist.clone())
        };
        let needs_dns = net_mode.trim().eq_ignore_ascii_case("allowlist")
            && !extract_allowlist_hostnames(&parse_allowlist_entries(&net_allowlist)).is_empty();
        if !needs_dns {
            match prepare_set_egress_recording(
                &name_s,
                &net_mode,
                &net_allowlist,
                enabled,
                &NoopResolver,
            ) {
                Ok(plan) => {
                    self.as_mut()
                        .run_shell_task(&plan.description, &plan.script, "", &name_s);
                }
                Err(err) => {
                    self.as_mut().set_error_message(&err);
                }
            }
            return;
        }

        self.as_mut()
            .set_busy(true, "Resolving network allowlist...");
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let res = prepare_set_egress_recording(
                &name_s,
                &net_mode,
                &net_allowlist,
                enabled,
                &SystemResolver,
            );
            let _ = qt_thread.queue(move |mut backend| {
                backend.as_mut().set_busy(false, "");
                match res {
                    Ok(plan) => {
                        backend.as_mut().run_shell_task(
                            &plan.description,
                            &plan.script,
                            "",
                            &name_s,
                        );
                    }
                    Err(err) => {
                        backend.as_mut().set_error_message(&err);
                    }
                }
            });
        });
    }

    pub fn refresh_egress_log(mut self: Pin<&mut Self>, name: &QString) {
        let name_s = name.to_string();
        if name_s.is_empty() || !self.env_exists_active(&name_s) {
            return;
        }

        let switched_env = self.egress_logs_env_name.to_string() != name_s;
        {
            let mut rust = self.as_mut().rust_mut();
            rust.egress_logs_env_name = QString::from(name_s.as_str());
            if switched_env {
                rust.egress_logs = Default::default();
            }
        }
        if switched_env && !self.demo_mode {
            self.as_mut().egress_logs_changed();
        }

        let gen = self.egress_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let gen_ref = Arc::clone(&self.egress_generation);

        if self.demo_mode {
            let demo_rows = serde_json::json!([
                {"dstIp": "140.82.121.4", "proto": "TCP", "port": "443", "count": 18, "lastSeen": "13:18:42"},
                {"dstIp": "160.79.104.10", "proto": "TCP", "port": "443", "count": 9, "lastSeen": "13:17:15"},
                {"dstIp": "104.16.25.34", "proto": "TCP", "port": "443", "count": 4, "lastSeen": "13:15:09"}
            ]);
            let q_rows = json_array_to_qvariant_list(demo_rows.as_array().unwrap_or(&Vec::new()));
            {
                let mut rust = self.as_mut().rust_mut();
                rust.egress_logs = q_rows;
            }
            self.as_mut().egress_logs_changed();
            return;
        }

        let (container_ip, host_iface) = {
            let Some(env) = environment_by_name(&self.engine.environments, &name_s) else {
                return;
            };
            (env.ipv4.clone(), env.host_interface.clone())
        };

        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let out = Command::new("journalctl")
                .args(["-k", "-n", "4000", "--no-pager", "--output=short-iso"])
                .output();
            let stdout_s = out
                .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
                .unwrap_or_default();
            let entries = parse_kernel_egress_logs(&stdout_s, &container_ip, &host_iface, 100);
            let _ = qt_thread.queue(move |mut qobject| {
                if gen_ref.load(Ordering::SeqCst) != gen
                    || qobject.egress_logs_env_name.to_string() != name_s
                {
                    return;
                }
                let q_rows = serialize_to_qvariant_list(&entries);
                {
                    let mut rust = qobject.as_mut().rust_mut();
                    rust.egress_logs = q_rows;
                }
                qobject.as_mut().egress_logs_changed();
            });
        });
    }

    pub fn add_egress_destination_to_allowlist(
        self: Pin<&mut Self>,
        name: &QString,
        destination_ip: &QString,
    ) {
        let name_s = name.to_string();
        if name_s.is_empty() || *self.busy() {
            return;
        }
        let current = environment_by_name(&self.engine.environments, &name_s)
            .map(|e| e.network_allowlist.clone())
            .unwrap_or_default();
        let Some(updated) = add_egress_destination_allowlist(&current, &destination_ip.to_string())
        else {
            return;
        };
        let mode = QString::from("allowlist");
        let allowlist = QString::from(updated.as_str());
        self.set_network_mode(name, &mode, &allowlist);
    }

    pub fn add_port_forward(
        self: Pin<&mut Self>,
        name: &QString,
        host_port: i32,
        guest_port: i32,
    ) -> QString {
        self.add_port_forward_with_bind(name, host_port, guest_port, false, false)
    }

    pub fn add_port_forward_with_bind(
        mut self: Pin<&mut Self>,
        name: &QString,
        host_port: i32,
        guest_port: i32,
        bind_lan: bool,
        confirm_lan: bool,
    ) -> QString {
        let name_s = name.to_string();
        let env = if name_s.is_empty() {
            None
        } else {
            environment_by_name(&self.engine.environments, &name_s)
        };
        let existing_ports: Vec<i32> = env
            .as_ref()
            .map(|e| e.port_forwards.iter().map(|p| p.host_port).collect())
            .unwrap_or_default();

        if let Err(err) = validate_add_port_forward_with_bind(
            &name_s,
            *self.busy(),
            host_port,
            guest_port,
            bind_lan,
            confirm_lan,
            env.is_some(),
            &existing_ports,
        ) {
            return QString::from(err.as_str());
        }

        let bind_addr = if bind_lan { "0.0.0.0" } else { "127.0.0.1" };
        let script = add_port_forward_script_with_bind(&name_s, host_port, guest_port, bind_lan);
        self.as_mut().run_shell_task(
            &format!("Forwarding {bind_addr}:{host_port} -> {name_s}:{guest_port}"),
            &script,
            "",
            &name_s,
        );
        QString::default()
    }

    pub fn local_domain_setup_command(&self) -> QString {
        QString::from(local_domain_setup_hint().as_str())
    }

    pub fn remove_port_forward(mut self: Pin<&mut Self>, name: &QString, device_name: &QString) {
        let name_s = name.to_string();
        let dev_s = device_name.to_string();
        if name_s.is_empty() || dev_s.is_empty() || *self.busy() {
            return;
        }
        let script = remove_port_forward_script(&name_s, &dev_s);
        self.as_mut().run_shell_task(
            &format!("Removing port forward '{dev_s}' on '{name_s}'"),
            &script,
            "",
            &name_s,
        );
    }

    pub fn open_forwarded_port(self: Pin<&mut Self>, host_port: i32) {
        if let Some(url) = open_forwarded_port_url(host_port) {
            let _ = spawn_detached("xdg-open", &[&url], None);
        }
    }
}
