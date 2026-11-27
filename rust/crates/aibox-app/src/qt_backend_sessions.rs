use std::path::Path;
use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QList, QString, QVariant};

use aibox_core::history::read_operation_log;
use aibox_core::session::{
    build_replay_command, delete_recorded_session, read_recorded_session_log, sessions_dir_path,
};
use aibox_core::util::shell::shell_quote;

use crate::qt_backend::{find_executable, launch_command_in_terminal, qobject};

type QListQVariant = QList<QVariant>;

impl qobject::Backend {
    pub fn read_operation_log(&self, id: &QString) -> QString {
        let log = read_operation_log(
            &self.engine.paths.state_dir,
            &id.to_string(),
            self.demo_mode,
        )
        .unwrap_or_else(|| "Log file not found.".to_string());
        QString::from(log.as_str())
    }

    pub fn clear_operation_history(mut self: Pin<&mut Self>) {
        if self.demo_mode {
            self.as_mut()
                .rust_mut()
                .engine
                .set_history_cache(Vec::new());
        } else {
            self.as_mut().rust_mut().engine.clear_history();
        }
        {
            let mut rust = self.as_mut().rust_mut();
            rust.history_seen_id.clear();
            rust.operation_history = QListQVariant::default();
        }
        self.as_mut().operation_history_changed();
    }

    pub fn acknowledge_operation_history(mut self: Pin<&mut Self>) {
        if self.demo_mode {
            return;
        }
        let next_seen = self.as_mut().rust_mut().engine.acknowledge_history();
        if let Some(next_seen) = next_seen {
            {
                let mut rust = self.as_mut().rust_mut();
                rust.history_seen_id = next_seen;
            }
            self.as_mut().operation_history_changed();
        }
    }

    pub fn set_environment_record_sessions(
        mut self: Pin<&mut Self>,
        name: &QString,
        mode: &QString,
    ) {
        let name_s = name.to_string();
        if name_s.is_empty() {
            return;
        }
        let norm = mode.to_string().trim().to_ascii_lowercase();
        let val = match norm.as_str() {
            "on" | "true" | "1" => "true",
            "off" | "false" | "0" => "false",
            _ => "inherit",
        };
        if self.demo_mode {
            let global_enabled = *self.record_sessions();
            let mut updated = false;
            {
                let mut rust = self.as_mut().rust_mut();
                let enabled_val = if val == "inherit" {
                    global_enabled
                } else {
                    val == "true"
                };
                for env in &mut rust.demo_environments {
                    if env.get("name").and_then(|v| v.as_str()) == Some(name_s.as_str()) {
                        if let Some(obj) = env.as_object_mut() {
                            obj.insert(
                                "recordOverride".to_string(),
                                serde_json::Value::String(val.to_string()),
                            );
                            obj.insert(
                                "recordEnabled".to_string(),
                                serde_json::Value::Bool(enabled_val),
                            );
                            updated = true;
                        }
                        break;
                    }
                }
            }
            if updated {
                self.as_mut().sync_demo_environments();
            }
            return;
        }

        if *self.busy() {
            return;
        }
        let q_name = shell_quote(&name_s);
        let script = if val == "inherit" {
            format!(
                "set -euo pipefail\necho '@@STAGE:50:Updating session recording setting...'\n\
                 incus config unset {q_name} user.aibox.record >/dev/null 2>&1 || true\n\
                 echo '@@STAGE:100:Session recording updated.'"
            )
        } else {
            format!(
                "set -euo pipefail\necho '@@STAGE:50:Updating session recording setting...'\n\
                 incus config set {q_name} user.aibox.record {val}\n\
                 echo '@@STAGE:100:Session recording updated.'"
            )
        };
        self.as_mut().run_shell_task(
            &format!("Updating session recording for '{name_s}'"),
            &script,
            "",
            &name_s,
        );
    }

    pub fn read_recorded_session_log(&self, log_path: &QString) -> QString {
        let log = read_recorded_session_log(&log_path.to_string(), self.demo_mode)
            .unwrap_or_else(|| "Session log not found.".to_string());
        QString::from(log.as_str())
    }

    pub fn replay_recorded_session(
        mut self: Pin<&mut Self>,
        log_path: &QString,
        timing_path: &QString,
    ) {
        if self.demo_mode {
            self.as_mut().set_status_message(
                "[Demo] Session replay opens in a terminal for real recordings.",
            );
            return;
        }
        let log_s = log_path.to_string();
        let timing_s = timing_path.to_string();
        if log_s.is_empty()
            || timing_s.is_empty()
            || !Path::new(&log_s).exists()
            || !Path::new(&timing_s).exists()
        {
            self.as_mut()
                .set_error_message("Timing file missing — cannot replay this session.");
            return;
        }
        if find_executable("scriptreplay").is_none() {
            self.as_mut()
                .set_error_message("'scriptreplay' is not installed on the host.");
            return;
        }
        let replay_cmd = build_replay_command(&log_s, &timing_s);
        launch_command_in_terminal(&replay_cmd);
    }

    pub fn delete_recorded_session(
        mut self: Pin<&mut Self>,
        log_path: &QString,
        timing_path: &QString,
    ) -> QString {
        if self.demo_mode {
            self.as_mut().set_status_message("[Demo] Session deleted.");
            return QString::default();
        }
        let sessions_dir = sessions_dir_path(&self.engine.paths.state_dir);
        if let Err(clean_root) = delete_recorded_session(
            &sessions_dir,
            &log_path.to_string(),
            &timing_path.to_string(),
        ) {
            return QString::from(format!("Refused to delete path outside {clean_root}").as_str());
        }
        self.as_mut().refresh_recorded_sessions();
        QString::default()
    }
}
