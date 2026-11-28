use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::pin::Pin;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QList, QString, QVariant};

use aibox_core::history::{record_operation_history, RecordOperationParams};
use aibox_core::incus::list::environment_by_name;
use aibox_core::snapshot::workspace::workspace_snapshot_dir;
use aibox_core::util::paths::home_dir;
use aibox_core::util::time::now_utc_iso;
use aibox_core::workspace::diff::{
    build_workspace_diff_args, is_safe_relative_path, prepare_file_diff, restore_single_file,
    shape_file_diff_output, workspace_root_mtime_nanos, FileDiffPrep, StreamingRsyncParser,
    DEFAULT_MAX_DIFF_ENTRIES,
};
use aibox_core::workspace::file_access::{
    build_file_access_mount_script, choose_file_access_port, collect_reserved_file_access_ports,
    file_access_dir_path, file_access_log_path, file_access_process_alive, file_access_url,
    is_host_port_available, is_local_tcp_port_listening, load_file_access_state,
    read_file_access_failure_detail, stop_file_access_process, validate_enable_file_access,
    write_file_access_state,
};
use aibox_core::workspace::mounts::{
    prepare_add_extra_mount, remove_extra_mount_script, validate_extra_mount_path,
};
use aibox_core::workspace::paste::{
    pasted_image_container_path, prepare_pasted_image_path, prune_stale_pasted_images,
};

use crate::qt_backend::{find_executable, qobject, spawn_detached};
use crate::qt_convert::{
    json_array_to_qvariant_list, serialize_to_qvariant_list, serialize_to_qvariant_map,
};

type QListQVariant = QList<QVariant>;

impl qobject::Backend {
    pub fn open_workspace_folder(self: Pin<&mut Self>, name: &QString) {
        let name_s = name.to_string();
        if name_s.is_empty() {
            return;
        }
        let path = self.workspace_path_for(&name_s);
        let _ = std::fs::create_dir_all(&path);
        let _ = spawn_detached("xdg-open", &[&path], None);
    }

    pub fn enable_file_access(mut self: Pin<&mut Self>, name: &QString) -> QString {
        let name_s = name.to_string();
        let env = if name_s.is_empty() {
            None
        } else {
            environment_by_name(&self.engine.environments, &name_s)
        };
        let env_exists = env.is_some();
        let is_template = env.as_ref().is_some_and(|e| e.is_template);
        let is_running = env.as_ref().is_some_and(|e| e.running);

        if let Err(err) =
            validate_enable_file_access(&name_s, *self.busy(), env_exists, is_template, is_running)
        {
            self.as_mut().set_error_message(&err);
            return QString::from(err.as_str());
        }

        let state_dir = self.engine.paths.state_dir.clone();
        if let Some(existing) = load_file_access_state(&state_dir, &name_s) {
            if file_access_process_alive(existing.pid, &name_s) {
                // The listener already runs; resync so the toggle reflects it.
                self.as_mut().sync_environments_from_engine();
                return QString::default();
            }
        }

        let fa_dir = file_access_dir_path(&state_dir);
        if std::fs::create_dir_all(&fa_dir).is_err() {
            let msg = format!("Could not create the root file access state directory: {fa_dir}");
            self.as_mut().set_error_message(&msg);
            return QString::from(msg.as_str());
        }

        let reserved: HashSet<i32> = collect_reserved_file_access_ports(&state_dir)
            .into_iter()
            .collect();
        let port = choose_file_access_port(&reserved, Some(is_host_port_available), 2222, 2299);
        if port <= 0 {
            let msg = "No free local port is available for root file access (tried 2222-2299).";
            self.as_mut().set_error_message(msg);
            return QString::from(msg);
        }

        let log_path = file_access_log_path(&state_dir, &name_s);
        let script = build_file_access_mount_script(&name_s, port, &log_path);
        let home = home_dir().to_string_lossy().to_string();
        let pid = match spawn_detached("/bin/sh", &["-c", &script], Some(&home)) {
            Ok(p) if p > 0 => p as i64,
            _ => {
                let msg = "Could not start the local SFTP listener.";
                self.as_mut().set_error_message(msg);
                return QString::from(msg);
            }
        };

        let started_at = now_utc_iso();
        if write_file_access_state(&state_dir, &name_s, pid, port, &started_at).is_err() {
            // SAFETY: `pid` is a valid child PID returned by `spawn_detached`.
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGTERM);
            }
            let msg = "Could not save the root file access state file.";
            self.as_mut().set_error_message(msg);
            return QString::from(msg);
        }

        self.as_mut()
            .set_status_message(&format!("Starting root file access on 127.0.0.1:{port}..."));
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let mut listening = false;
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline {
                if !file_access_process_alive(pid, &name_s) {
                    break;
                }
                if is_local_tcp_port_listening(port) {
                    listening = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(60));
            }

            if !listening {
                let detail = read_file_access_failure_detail(&log_path);
                stop_file_access_process(&state_dir, &name_s);
                let msg = if detail.is_empty() {
                    format!("Root file access failed to start on 127.0.0.1:{port}.")
                } else {
                    format!("Root file access failed to start: {detail}")
                };
                let _ = qt_thread.queue(move |mut backend| {
                    backend.as_mut().set_status_message("");
                    backend.as_mut().set_error_message(&msg);
                });
                return;
            }

            let url = file_access_url(port);
            let _ = qt_thread.queue(move |mut backend| {
                backend.as_mut().sync_environments_from_engine();
                backend.as_mut().set_error_message("");
                backend
                    .as_mut()
                    .set_status_message(&format!("Root file access enabled: {url}"));
                backend
                    .as_mut()
                    .schedule_status_clear(Duration::from_millis(3000));
            });
        });
        QString::default()
    }

    pub fn disable_file_access(mut self: Pin<&mut Self>, name: &QString) {
        let name_s = name.to_string();
        if name_s.is_empty() {
            return;
        }
        let state_dir = self.engine.paths.state_dir.clone();
        stop_file_access_process(&state_dir, &name_s);
        self.as_mut().sync_environments_from_engine();
        self.as_mut()
            .set_status_message(&format!("Root file access disabled for '{name_s}'."));
        self.as_mut()
            .schedule_status_clear(Duration::from_millis(3000));
    }

    pub fn open_file_access(self: Pin<&mut Self>, name: &QString) {
        let name_s = name.to_string();
        if name_s.is_empty() {
            return;
        }
        let url = load_file_access_state(&self.engine.paths.state_dir, &name_s)
            .filter(|s| file_access_process_alive(s.pid, &name_s) && s.port > 0)
            .map(|s| file_access_url(s.port))
            .unwrap_or_default();
        if url.is_empty() {
            return;
        }

        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let has_handler = Command::new("gio")
                .args(["mime", "x-scheme-handler/sftp"])
                .env("LC_ALL", "C")
                .output()
                .is_ok_and(|out| {
                    out.status.success()
                        && !String::from_utf8_lossy(&out.stdout).contains("No default applications")
                });
            if has_handler && spawn_detached("xdg-open", &[&url], None).is_ok() {
                return;
            }

            for fm in ["nautilus", "dolphin", "thunar", "nemo", "pcmanfm", "caja"] {
                if find_executable(fm).is_none() {
                    continue;
                }
                let res = if fm == "nautilus" {
                    spawn_detached(fm, &["-w", &url], None)
                } else {
                    spawn_detached(fm, &[&url], None)
                };
                if res.is_ok() {
                    return;
                }
            }

            let _ = qt_thread.queue(move |mut backend| {
                backend
                    .as_mut()
                    .copy_text_with_status(&url, "Copied SFTP URL to clipboard.");
                backend.as_mut().set_error_message(
                    "No file manager accepted the SFTP URL; it has been copied to the clipboard.",
                );
            });
        });
    }

    pub fn paste_image_to_workspace(mut self: Pin<&mut Self>, name: &QString) -> QString {
        let env_name = name.to_string().trim().to_string();
        if env_name.is_empty() {
            self.as_mut()
                .set_error_message("Select an environment before pasting an image.");
            return QString::default();
        }
        let env = environment_by_name(&self.engine.environments, &env_name);
        if env.is_none() {
            self.as_mut()
                .set_error_message(&format!("The environment '{env_name}' is not available."));
            return QString::default();
        }
        let png_bytes = qobject::aibox_clipboard_image_png();
        if png_bytes.is_empty() {
            self.as_mut().set_error_message(
                "The clipboard does not contain an image. Copy a screenshot or an image file first.",
            );
            return QString::default();
        }

        let ws_path = self.workspace_path_for(&env_name);
        let host_path = match prepare_pasted_image_path(&ws_path) {
            Ok(p) => p,
            Err(e) => {
                self.as_mut().set_error_message(&e);
                return QString::default();
            }
        };
        if std::fs::write(&host_path, &png_bytes).is_err() {
            self.as_mut().set_error_message(&format!(
                "Could not save the clipboard image to '{host_path}'."
            ));
            return QString::default();
        }
        prune_stale_pasted_images(&ws_path, 7);

        let container_ws = env
            .map(|e| e.workspace_mount_path.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| aibox_core::util::paths::CONTAINER_WORKSPACE_PATH.to_string());
        let container_path = pasted_image_container_path(&container_ws, &host_path);

        let q_container_path = QString::from(container_path.as_str());
        qobject::aibox_clipboard_set_text(&q_container_path);
        self.as_mut().set_status_message(&format!(
            "Saved clipboard image as '{container_path}'. Path is on the clipboard; paste it in the terminal."
        ));
        self.as_mut()
            .schedule_status_clear(Duration::from_millis(6000));
        q_container_path
    }

    pub fn validate_extra_mount_candidate(&self, raw_path: &QString) -> QString {
        let cfg_dir = Path::new(&self.engine.paths.config_file_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let forbidden = vec![cfg_dir, self.engine.paths.state_dir.clone()];
        match validate_extra_mount_path(&raw_path.to_string(), &forbidden) {
            Ok(()) => QString::default(),
            Err(err) => QString::from(err.as_str()),
        }
    }

    pub fn add_extra_mount(
        mut self: Pin<&mut Self>,
        name: &QString,
        host_path: &QString,
        mount_name: &QString,
        read_only: bool,
        confirm_read_write: bool,
    ) -> QString {
        let name_s = name.to_string();
        let env = if name_s.is_empty() {
            None
        } else {
            environment_by_name(&self.engine.environments, &name_s)
        };
        let cfg_dir = Path::new(&self.engine.paths.config_file_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let forbidden = vec![cfg_dir, self.engine.paths.state_dir.clone()];
        let existing_mounts: Vec<(String, String)> = env
            .as_ref()
            .map(|e| {
                e.extra_mounts
                    .iter()
                    .map(|m| (m.device_name.clone(), m.path.clone()))
                    .collect()
            })
            .unwrap_or_default();

        let prepared = match prepare_add_extra_mount(
            &name_s,
            *self.busy(),
            &host_path.to_string(),
            &mount_name.to_string(),
            read_only,
            confirm_read_write,
            &forbidden,
            env.is_some(),
            &existing_mounts,
        ) {
            Ok(p) => p,
            Err(err) => return QString::from(err.as_str()),
        };
        let mode_label = if read_only { "RO" } else { "RW" };
        self.as_mut().run_shell_task(
            &format!(
                "Mounting {} at {} ({mode_label})",
                prepared.resolved_source, prepared.container_path
            ),
            &prepared.script,
            "",
            &name_s,
        );
        QString::default()
    }

    pub fn remove_extra_mount(mut self: Pin<&mut Self>, name: &QString, device_name: &QString) {
        let name_s = name.to_string();
        let dev_s = device_name.to_string();
        if name_s.is_empty() || dev_s.is_empty() || *self.busy() {
            return;
        }
        let script = remove_extra_mount_script(&name_s, &dev_s);
        self.as_mut().run_shell_task(
            &format!("Removing mount '{dev_s}' on '{name_s}'"),
            &script,
            "",
            &name_s,
        );
    }

    pub fn load_workspace_diff(mut self: Pin<&mut Self>, name: &QString, snapshot_name: &QString) {
        let name_s = name.to_string();
        let snap_s = snapshot_name.to_string();
        if name_s.is_empty() || snap_s.is_empty() {
            return;
        }

        // Invalidate any in-flight diff or file-diff workers before early-return paths.
        let gen = self.diff_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let gen_ref = self.diff_generation.clone();
        self.file_diff_generation.fetch_add(1, Ordering::SeqCst);

        {
            let mut rust = self.as_mut().rust_mut();
            rust.selected_diff_text = QString::default();
            rust.selected_diff_info = QString::default();
        }
        self.as_mut().selected_diff_changed();

        if self.demo_mode {
            let entries = serde_json::json!([
                {"path": "lib/main.dart", "status": "modified", "type": "file", "sizeBytes": 4120, "sizeText": "4 KB"},
                {"path": "lib/services/auth.dart", "status": "added", "type": "file", "sizeBytes": 1840, "sizeText": "2 KB"},
                {"path": "lib/legacy_helper.dart", "status": "deleted", "type": "file", "sizeBytes": 920, "sizeText": "1 KB"}
            ]);
            let summary = serde_json::json!({
                "snapshotName": snap_s,
                "addedCount": 1,
                "modifiedCount": 1,
                "deletedCount": 1,
                "totalCount": 3,
                "truncated": false,
                "error": ""
            });
            let q_entries = json_array_to_qvariant_list(entries.as_array().unwrap_or(&Vec::new()));
            let q_summary = serialize_to_qvariant_map(&summary);
            {
                let mut rust = self.as_mut().rust_mut();
                rust.diff_loading = false;
                rust.diff_entries = q_entries;
                rust.diff_summary = q_summary;
            }
            self.as_mut().workspace_diff_changed();
            return;
        }

        let env = environment_by_name(&self.engine.environments, &name_s);
        let ws_path = env
            .map(|e| e.workspace_path.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.workspace_path_for(&name_s));
        let snap_dir = workspace_snapshot_dir(&ws_path, &name_s);
        let snap_path = format!("{snap_dir}/{snap_s}");

        if snap_dir.is_empty() || !Path::new(&snap_path).is_dir() {
            let summary = serde_json::json!({
                "snapshotName": snap_s,
                "addedCount": 0,
                "modifiedCount": 0,
                "deletedCount": 0,
                "totalCount": 0,
                "truncated": false,
                "error": format!("Workspace snapshot '{snap_s}' was not found.")
            });
            let q_summary = serialize_to_qvariant_map(&summary);
            {
                let mut rust = self.as_mut().rust_mut();
                rust.diff_loading = false;
                rust.diff_entries = QListQVariant::default();
                rust.diff_summary = q_summary;
            }
            self.as_mut().workspace_diff_changed();
            return;
        }

        let excludes = self.engine.config.diff_excludes.clone();
        let ws_mtime = workspace_root_mtime_nanos(&ws_path);
        let snap_mtime = workspace_root_mtime_nanos(&snap_path);
        if let Some(cached) = self
            .diff_cache
            .get(&name_s, &snap_s, ws_mtime, snap_mtime, &excludes)
            .cloned()
        {
            let q_entries = serialize_to_qvariant_list(&cached.entries);
            let q_summary = serialize_to_qvariant_map(&cached.summary);
            {
                let mut rust = self.as_mut().rust_mut();
                rust.diff_loading = false;
                rust.diff_entries = q_entries;
                rust.diff_summary = q_summary;
            }
            self.as_mut().workspace_diff_changed();
            return;
        }

        let args = build_workspace_diff_args(&snap_path, &ws_path, &excludes);

        let initial_summary = serde_json::json!({
            "snapshotName": snap_s,
            "addedCount": 0,
            "modifiedCount": 0,
            "deletedCount": 0,
            "totalCount": 0,
            "truncated": false,
            "error": ""
        });
        let q_initial_summary = serialize_to_qvariant_map(&initial_summary);
        {
            let mut rust = self.as_mut().rust_mut();
            rust.diff_loading = true;
            rust.diff_entries = QListQVariant::default();
            rust.diff_summary = q_initial_summary;
        }
        self.as_mut().workspace_diff_changed();

        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let push_error = |err_msg: String| {
                let snap_err = snap_s.clone();
                let gen_err = gen_ref.clone();
                let _ = qt_thread.queue(move |mut qobject| {
                    if gen_err.load(Ordering::SeqCst) != gen {
                        return;
                    }
                    let summary = serde_json::json!({
                        "snapshotName": snap_err,
                        "addedCount": 0,
                        "modifiedCount": 0,
                        "deletedCount": 0,
                        "totalCount": 0,
                        "truncated": false,
                        "error": err_msg
                    });
                    let q_summary = serialize_to_qvariant_map(&summary);
                    {
                        let mut rust = qobject.as_mut().rust_mut();
                        rust.diff_loading = false;
                        rust.diff_entries = QListQVariant::default();
                        rust.diff_summary = q_summary;
                    }
                    qobject.as_mut().workspace_diff_changed();
                });
            };

            let mut child = match Command::new("rsync")
                .args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(c) => c,
                Err(e) => {
                    push_error(format!("Failed to start rsync: {e}"));
                    return;
                }
            };

            let stdout_pipe = child.stdout.take();
            let stderr_pipe = child.stderr.take();

            let stderr_handle = std::thread::spawn(move || {
                let mut buf = String::new();
                if let Some(mut err) = stderr_pipe {
                    let _ = err.read_to_string(&mut buf);
                }
                buf
            });

            let (tx, rx) = mpsc::channel::<String>();
            let stdout_handle = std::thread::spawn(move || {
                if let Some(out) = stdout_pipe {
                    let mut reader = BufReader::new(out);
                    let mut line = String::new();
                    loop {
                        line.clear();
                        match reader.read_line(&mut line) {
                            Ok(0) | Err(_) => break,
                            Ok(_) => {
                                if tx.send(std::mem::take(&mut line)).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                }
            });

            let mut parser = StreamingRsyncParser::new(&ws_path, DEFAULT_MAX_DIFF_ENTRIES);
            let deadline = Instant::now() + Duration::from_secs(30);
            let mut timed_out = false;
            let mut pending_entries = 0_usize;
            let mut last_emit = Instant::now();
            let partial_seq = Arc::new(AtomicU64::new(0));

            loop {
                if gen_ref.load(Ordering::SeqCst) != gen {
                    let _ = child.kill();
                    let _ = child.wait();
                    return;
                }
                if Instant::now() >= deadline {
                    timed_out = true;
                    let _ = child.kill();
                    break;
                }

                let mut disconnected = false;
                match rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(line) => {
                        if parser.push_line(&line) {
                            pending_entries += 1;
                        }
                        while let Ok(more) = rx.try_recv() {
                            if parser.push_line(&more) {
                                pending_entries += 1;
                            }
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => {
                        disconnected = true;
                    }
                }

                if pending_entries > 0 && last_emit.elapsed() >= Duration::from_millis(120) {
                    pending_entries = 0;
                    last_emit = Instant::now();
                    let partial = parser.snapshot_result(&snap_s);
                    let entries_json = match serde_json::to_value(&partial.entries) {
                        Ok(serde_json::Value::Array(arr)) => arr,
                        _ => Vec::new(),
                    };
                    let summary_json = serde_json::to_value(&partial.summary).ok();
                    let seq = partial_seq.fetch_add(1, Ordering::SeqCst) + 1;
                    let seq_ref = Arc::clone(&partial_seq);
                    let gen_partial = gen_ref.clone();
                    let _ = qt_thread.queue(move |mut qobject| {
                        if gen_partial.load(Ordering::SeqCst) != gen
                            || seq_ref.load(Ordering::SeqCst) != seq
                        {
                            return;
                        }
                        let q_entries = json_array_to_qvariant_list(&entries_json);
                        let q_summary = match summary_json {
                            Some(serde_json::Value::Object(ref map)) => {
                                crate::qt_convert::json_object_to_qvariant_map(map)
                            }
                            _ => Default::default(),
                        };
                        {
                            let mut rust = qobject.as_mut().rust_mut();
                            rust.diff_loading = true;
                            rust.diff_entries = q_entries;
                            rust.diff_summary = q_summary;
                        }
                        qobject.as_mut().workspace_diff_changed();
                    });
                }

                if disconnected {
                    break;
                }
            }

            partial_seq.fetch_add(1, Ordering::SeqCst);
            let status = child.wait();
            let _ = stdout_handle.join();
            let stderr_text = stderr_handle.join().unwrap_or_default();

            if gen_ref.load(Ordering::SeqCst) != gen {
                return;
            }
            if timed_out {
                push_error("Diff scan timed out after 30 seconds.".to_string());
                return;
            }

            let code = match status {
                Ok(s) => s.code().unwrap_or(-1),
                Err(e) => {
                    push_error(format!("Failed to read rsync output: {e}"));
                    return;
                }
            };
            if code != 0 && code != 24 {
                let err_txt = stderr_text.trim().to_string();
                let msg = if err_txt.is_empty() {
                    "Failed to compare workspace with snapshot.".to_string()
                } else {
                    err_txt
                };
                push_error(msg);
                return;
            }

            let res = parser.snapshot_result(&snap_s);
            let entries_json = match serde_json::to_value(&res.entries) {
                Ok(serde_json::Value::Array(arr)) => arr,
                _ => Vec::new(),
            };
            let summary_json = serde_json::to_value(&res.summary).ok();
            let _ = qt_thread.queue(move |mut qobject| {
                if gen_ref.load(Ordering::SeqCst) != gen {
                    return;
                }
                let q_entries = json_array_to_qvariant_list(&entries_json);
                let q_summary = match summary_json {
                    Some(serde_json::Value::Object(ref map)) => {
                        crate::qt_convert::json_object_to_qvariant_map(map)
                    }
                    _ => Default::default(),
                };
                {
                    let mut rust = qobject.as_mut().rust_mut();
                    rust.diff_cache
                        .insert(&name_s, &snap_s, ws_mtime, snap_mtime, excludes, res);
                    rust.diff_loading = false;
                    rust.diff_entries = q_entries;
                    rust.diff_summary = q_summary;
                }
                qobject.as_mut().workspace_diff_changed();
            });
        });
    }

    pub fn load_file_diff(
        mut self: Pin<&mut Self>,
        name: &QString,
        snapshot_name: &QString,
        relative_path: &QString,
        status: &QString,
    ) {
        let name_s = name.to_string();
        let snap_s = snapshot_name.to_string();
        let rel_s = relative_path.to_string();
        let status_s = status.to_string();
        let file_gen = self.file_diff_generation.fetch_add(1, Ordering::SeqCst) + 1;

        if name_s.is_empty() || snap_s.is_empty() || !is_safe_relative_path(&rel_s) {
            {
                let mut rust = self.as_mut().rust_mut();
                rust.selected_diff_text = QString::default();
                rust.selected_diff_info = QString::from("Invalid file path.");
            }
            self.as_mut().selected_diff_changed();
            return;
        }

        if self.demo_mode {
            let text = match status_s.as_str() {
                "added" => "--- snapshot\n+++ current\n@@ -0,0 +1,4 @@\n+class AuthService {\n+  Future<bool> signIn() async => true;\n+}\n",
                "deleted" => "--- snapshot\n+++ current\n@@ -1,3 +0,0 @@\n-String legacyHelper() {\n-  return 'deprecated';\n-}\n",
                _ => "--- snapshot\n+++ current\n@@ -10,4 +10,5 @@\n void main() {\n-  runApp(const OldApp());\n+  // Updated by AI agent\n+  runApp(const NewApp());\n }\n",
            };
            {
                let mut rust = self.as_mut().rust_mut();
                rust.selected_diff_text = QString::from(text);
                rust.selected_diff_info = QString::from(rel_s.as_str());
            }
            self.as_mut().selected_diff_changed();
            return;
        }

        let ws_path = environment_by_name(&self.engine.environments, &name_s)
            .map(|e| e.workspace_path.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.workspace_path_for(&name_s));

        let args = match prepare_file_diff(&name_s, &snap_s, &ws_path, &rel_s, &status_s) {
            FileDiffPrep::Immediate { text, info } => {
                {
                    let mut rust = self.as_mut().rust_mut();
                    rust.selected_diff_text = QString::from(text.as_str());
                    rust.selected_diff_info = QString::from(info.as_str());
                }
                self.as_mut().selected_diff_changed();
                return;
            }
            FileDiffPrep::RunDiff { args } => args,
        };
        let file_gen_ref = Arc::clone(&self.file_diff_generation);
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let out = Command::new("diff").args(&args).output();
            let (text, info) = match out {
                Ok(output) => {
                    let code = output.status.code().unwrap_or(-1);
                    let raw = String::from_utf8_lossy(&output.stdout);
                    let shaped = shape_file_diff_output(code, &raw, &rel_s);
                    (shaped.text, shaped.info)
                }
                Err(e) => (String::new(), format!("Failed to run diff: {e}")),
            };
            let _ = qt_thread.queue(move |mut qobject| {
                if file_gen_ref.load(Ordering::SeqCst) != file_gen {
                    return;
                }
                {
                    let mut rust = qobject.as_mut().rust_mut();
                    rust.selected_diff_text = QString::from(text.as_str());
                    rust.selected_diff_info = QString::from(info.as_str());
                }
                qobject.as_mut().selected_diff_changed();
            });
        });
    }

    pub fn restore_single_file(
        mut self: Pin<&mut Self>,
        name: &QString,
        snapshot_name: &QString,
        relative_path: &QString,
    ) -> QString {
        let name_s = name.to_string();
        let snap_s = snapshot_name.to_string();
        let rel_s = relative_path.to_string();
        if name_s.is_empty() || snap_s.is_empty() || !is_safe_relative_path(&rel_s) {
            return QString::from("Invalid file restore request.");
        }

        let state_dir = self.engine.paths.state_dir.clone();
        let secrets = self.engine.secrets.all_secret_values();

        if self.demo_mode {
            if let Some(entries) = record_operation_history(&RecordOperationParams {
                state_dir: &state_dir,
                max_entries: 50,
                description: &format!("Restored file '{rel_s}' from snapshot '{snap_s}'"),
                env_name: &name_s,
                status: "success",
                exit_code: 0,
                started_at_iso: &now_utc_iso(),
                log_bytes: b"Single-file restore (demo)\n",
                reason: "",
                secret_values: &secrets,
            }) {
                let q_hist = json_array_to_qvariant_list(&entries);
                {
                    let mut rust = self.as_mut().rust_mut();
                    rust.engine.set_history_cache(entries);
                    rust.operation_history = q_hist;
                }
                self.as_mut().operation_history_changed();
            }
            self.as_mut()
                .set_status_message(&format!("[Demo] Restored '{rel_s}' from '{snap_s}'."));
            return QString::default();
        }

        let ws_path = environment_by_name(&self.engine.environments, &name_s)
            .map(|e| e.workspace_path.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.workspace_path_for(&name_s));

        let log_msg = match restore_single_file(&name_s, &snap_s, &ws_path, &rel_s) {
            Ok(msg) => msg,
            Err(err) => return QString::from(err.as_str()),
        };

        if let Some(entries) = record_operation_history(&RecordOperationParams {
            state_dir: &state_dir,
            max_entries: 50,
            description: &format!("Restored file '{rel_s}' from snapshot '{snap_s}'"),
            env_name: &name_s,
            status: "success",
            exit_code: 0,
            started_at_iso: &now_utc_iso(),
            log_bytes: log_msg.as_bytes(),
            reason: "",
            secret_values: &secrets,
        }) {
            let q_hist = json_array_to_qvariant_list(&entries);
            {
                let mut rust = self.as_mut().rust_mut();
                rust.engine.set_history_cache(entries);
                rust.operation_history = q_hist;
            }
            self.as_mut().operation_history_changed();
        }
        self.as_mut()
            .set_status_message(&format!("Restored '{rel_s}' from snapshot '{snap_s}'."));
        self.as_mut().rust_mut().diff_cache.invalidate_env(&name_s);
        self.as_mut().load_workspace_diff(name, snapshot_name);
        QString::default()
    }
}
