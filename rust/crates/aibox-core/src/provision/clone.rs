use crate::network::acl::acl_apply_shell_snippet;
use crate::network::allowlist::{parse_allowlist_entries, resolve_allowlist_cidrs, HostResolver};
use crate::provision::templates::{render_template, rollback_trap_snippet, CLONE_SCRIPT_TEMPLATE};
use crate::provision::{
    is_host_swap_value, is_valid_size_spec, swap_config_value, validate_limits,
    validate_new_env_name, validate_swap_limit, ProvisionError,
};
use crate::snapshot::workspace::workspace_create_snippet;
use crate::util::paths::{resolve_workspace_path, CONTAINER_WORKSPACE_PATH};
use crate::util::shell::shell_quote;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloneEnvironmentPlan {
    pub new_name: String,
    pub title: String,
    pub script: String,
}

#[allow(clippy::too_many_arguments)]
pub fn clone_environment_script(
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
    swap_limit: &str,
    is_vm: bool,
) -> String {
    clone_environment_script_with_source(
        source_name,
        new_name,
        workspace,
        "",
        disk_gib,
        ram_gib,
        cpu_cores,
        start_after,
        net_mode,
        net_allowlist,
        resolved_cidrs,
        process_limit,
        io_limit,
        swap_limit,
        is_vm,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn clone_environment_script_with_source(
    source_name: &str,
    new_name: &str,
    workspace: &str,
    source_workspace: &str,
    disk_gib: &str,
    ram_gib: &str,
    cpu_cores: i32,
    start_after: bool,
    net_mode: &str,
    net_allowlist: &str,
    resolved_cidrs: &[String],
    process_limit: &str,
    io_limit: &str,
    swap_limit: &str,
    is_vm: bool,
) -> String {
    let q_new = shell_quote(new_name);
    let mut quota_lines = String::new();
    if cpu_cores > 0 {
        quota_lines.push_str(&format!(
            "incus config set {q_new} limits.cpu {}\n",
            shell_quote(&cpu_cores.to_string())
        ));
    }
    let trimmed_ram = ram_gib.trim();
    if !trimmed_ram.is_empty() {
        quota_lines.push_str(&format!(
            "incus config set {q_new} limits.memory {}\n",
            shell_quote(trimmed_ram)
        ));
    }
    let clean_proc = process_limit.trim();
    if !is_vm && !clean_proc.is_empty() && !clean_proc.eq_ignore_ascii_case("unlimited") {
        quota_lines.push_str(&format!(
            "incus config set {q_new} limits.processes {}\n",
            shell_quote(clean_proc)
        ));
    }
    let clean_swap = swap_limit.trim();
    if !is_vm && !clean_swap.is_empty() {
        if is_host_swap_value(clean_swap) {
            quota_lines.push_str(&format!("incus config unset {q_new} limits.memory.swap\n"));
        } else if let Some(swap_value) = swap_config_value(clean_swap) {
            quota_lines.push_str(&format!(
                "incus config set {q_new} limits.memory.swap {}\n",
                shell_quote(&swap_value)
            ));
        }
    }
    let trimmed_disk = disk_gib.trim();
    if !trimmed_disk.is_empty() {
        let q_disk = shell_quote(trimmed_disk);
        quota_lines.push_str(&format!(
            "incus config device set {q_new} root size={q_disk} >/dev/null 2>&1 || incus config device override {q_new} root size={q_disk}\n"
        ));
    }
    let clean_io = io_limit.trim();
    if !clean_io.is_empty() && !clean_io.eq_ignore_ascii_case("unlimited") {
        let q_io = shell_quote(clean_io);
        quota_lines.push_str(&format!(
            "incus config device set {q_new} root limits.read={q_io} limits.write={q_io} >/dev/null 2>&1 || incus config device override {q_new} root limits.read={q_io} limits.write={q_io}\n"
        ));
    }

    let mut net_snippet = if net_mode == "allowlist" {
        acl_apply_shell_snippet(new_name, net_allowlist, resolved_cidrs, false)
    } else if net_mode == "offline" {
        format!(
            "incus config device remove {q_new} eth0 >/dev/null 2>&1 || true\nincus config device add {q_new} eth0 none >/dev/null 2>&1 || true"
        )
    } else {
        format!("incus config device remove {q_new} eth0 >/dev/null 2>&1 || true")
    };
    net_snippet.push_str(&format!(
        "\nincus config unset {q_new} user.aibox.egresslog >/dev/null 2>&1 || true"
    ));

    let start_snippet = if start_after {
        "\necho \"@@STAGE:85:Starting cloned environment...\"\nincus start \"$DST\"\nfor _ in 1 2 3 4 5; do\n    if incus list \"$DST\" -f csv -c 4 | grep -q \"[0-9]\"; then\n        break\n    fi\n    sleep 1\ndone\n"
    } else {
        ""
    };

    let rollback_trap = rollback_trap_snippet("DST");
    let ws_create = workspace_create_snippet(workspace);

    render_template(
        CLONE_SCRIPT_TEMPLATE,
        &[
            ("SRC", &shell_quote(source_name)),
            ("DST", &shell_quote(new_name)),
            ("WORKSPACE", &shell_quote(workspace)),
            ("SRC_WORKSPACE", &shell_quote(source_workspace)),
            ("WS_PATH", &shell_quote(CONTAINER_WORKSPACE_PATH)),
            ("ROLLBACK_TRAP", &rollback_trap),
            ("WORKSPACE_CREATE", &ws_create),
            ("QUOTA_LINES", quota_lines.trim()),
            ("NET_SNIPPET", &net_snippet),
            ("START_SNIPPET", start_snippet),
        ],
    )
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_clone_environment<R: HostResolver>(
    busy: bool,
    source_name: &str,
    source_exists: bool,
    source_net_mode: &str,
    source_net_allowlist: &str,
    source_type: &str,
    source_workspace: &str,
    raw_new_name: &str,
    existing_names: &[String],
    disk_gib: &str,
    ram_gib: &str,
    cpu_cores: i32,
    start_after: bool,
    process_limit: &str,
    io_limit: &str,
    swap_limit: &str,
    workspace_parent: &str,
    workspace_base: &str,
    resolver: &R,
) -> Result<CloneEnvironmentPlan, ProvisionError> {
    if busy {
        return Err(ProvisionError::Busy);
    }
    if !source_exists {
        return Err(ProvisionError::SourceNotFound(source_name.to_string()));
    }
    let new_name = validate_new_env_name(raw_new_name, existing_names)?;

    if !(0..=1024).contains(&cpu_cores) {
        return Err(ProvisionError::InvalidCpuCores);
    }
    if !disk_gib.trim().is_empty() && !is_valid_size_spec(disk_gib) {
        return Err(ProvisionError::InvalidDiskQuota);
    }
    if !ram_gib.trim().is_empty() && !is_valid_size_spec(ram_gib) {
        return Err(ProvisionError::InvalidRamLimit);
    }

    let (clean_proc, clean_io) = validate_limits(process_limit, io_limit)?;
    validate_swap_limit(swap_limit)?;

    let resolved_cidrs = if source_net_mode == "allowlist" {
        let entries = parse_allowlist_entries(source_net_allowlist);
        resolve_allowlist_cidrs(&entries, resolver).0
    } else {
        Vec::new()
    };

    let is_vm = source_type.eq_ignore_ascii_case("VM");
    let host_workspace = resolve_workspace_path(workspace_parent, &new_name, workspace_base)
        .map_err(ProvisionError::InvalidWorkspacePath)?;
    let script = clone_environment_script_with_source(
        source_name,
        &new_name,
        &host_workspace,
        source_workspace,
        disk_gib,
        ram_gib,
        cpu_cores,
        start_after,
        source_net_mode,
        source_net_allowlist,
        &resolved_cidrs,
        &clean_proc,
        &clean_io,
        swap_limit,
        is_vm,
    );

    Ok(CloneEnvironmentPlan {
        title: format!("Cloning '{source_name}' to '{new_name}'"),
        new_name,
        script,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    fn write_executable(path: &std::path::Path, content: &str) {
        fs::write(path, content).unwrap();
        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).unwrap();
    }

    #[test]
    fn clone_environment_script_isolates_workspace_and_unsets_template_match_qt_test() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("aibox-prov-clone-{nanos}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let stub_dir = dir.join("stubs");
        fs::create_dir_all(&stub_dir).unwrap();
        let log_path = dir.join("incus.log");

        write_executable(
            &stub_dir.join("incus"),
            "#!/bin/sh\necho \"$@\" >> \"$AIBOX_STUB_LOG\"\nexit 0\n",
        );
        write_executable(&stub_dir.join("script"), "#!/bin/sh\nexec bash -c \"$4\"\n");
        write_executable(&stub_dir.join("sleep"), "#!/bin/sh\nexit 0\n");

        let workspace = dir.join("ws/cloned-box");
        let workspace_str = workspace.to_string_lossy().to_string();
        let script_path = dir.join("clone.sh");
        let script = clone_environment_script(
            "node-tpl",
            "cloned-box",
            &workspace_str,
            "40GiB",
            "8GiB",
            4,
            false,
            "online",
            "",
            &[],
            "",
            "",
            "",
            false,
        );
        fs::write(&script_path, script).unwrap();

        let path_env = format!(
            "{}:{}",
            stub_dir.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let output = Command::new("/bin/bash")
            .arg(&script_path)
            .env("PATH", path_env)
            .env("AIBOX_STUB_LOG", &log_path)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");

        let log = fs::read_to_string(&log_path).unwrap();
        assert!(log.contains("copy node-tpl cloned-box --instance-only --stateless"));
        assert!(log.contains("config unset cloned-box user.aibox.template"));
        assert!(log.contains("config device remove cloned-box workspace"));
        assert!(log.contains(&format!(
            "config device add cloned-box workspace disk source={workspace_str} path=/home/workspace shift=true"
        )));
        assert!(log.contains("config set cloned-box limits.cpu 4"));
        assert!(log.contains("config set cloned-box limits.memory 8GiB"));
        assert!(workspace.is_dir());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn runaway_limits_clone_script() {
        let clone_script = clone_environment_script(
            "src",
            "dst",
            "/tmp/ws",
            "40GiB",
            "8GiB",
            4,
            false,
            "online",
            "",
            &[],
            "1024",
            "100MB",
            "Off",
            false,
        );
        assert!(clone_script.contains("limits.processes '1024'"));
        assert!(clone_script.contains("limits.read='100MB' limits.write='100MB'"));
        assert!(clone_script.contains("limits.memory.swap 'false'"));
    }

    #[test]
    fn prepare_clone_environment_validates_inputs_with_typed_errors() {
        struct NoopResolver;
        impl HostResolver for NoopResolver {
            fn resolve_ipv4(&self, _host: &str) -> Vec<String> {
                Vec::new()
            }
        }
        let resolver = NoopResolver;
        let existing = vec!["taken".to_string()];

        assert_eq!(
            prepare_clone_environment(
                true,
                "src",
                true,
                "online",
                "",
                "CT",
                "/tmp/ws/src",
                "dst",
                &existing,
                "40GiB",
                "8GiB",
                4,
                false,
                "512",
                "100MB",
                "",
                "",
                "/tmp/ws",
                &resolver,
            )
            .unwrap_err(),
            ProvisionError::Busy
        );
        assert_eq!(
            prepare_clone_environment(
                false, "missing", false, "online", "", "CT", "", "dst", &existing, "40GiB", "8GiB",
                4, false, "512", "100MB", "", "", "/tmp/ws", &resolver,
            )
            .unwrap_err(),
            ProvisionError::SourceNotFound("missing".into())
        );
        assert_eq!(
            prepare_clone_environment(
                false, "src", true, "online", "", "CT", "", "!!!", &existing, "40GiB", "8GiB", 4,
                false, "512", "100MB", "", "", "/tmp/ws", &resolver,
            )
            .unwrap_err(),
            ProvisionError::InvalidName
        );
        assert_eq!(
            prepare_clone_environment(
                false, "src", true, "online", "", "CT", "", "taken", &existing, "40GiB", "8GiB", 4,
                false, "512", "100MB", "", "", "/tmp/ws", &resolver,
            )
            .unwrap_err(),
            ProvisionError::AlreadyExists("taken".into())
        );
        assert_eq!(
            prepare_clone_environment(
                false, "src", true, "online", "", "CT", "", "dst", &existing, "40GiB", "8GiB", -1,
                false, "512", "100MB", "", "", "/tmp/ws", &resolver,
            )
            .unwrap_err(),
            ProvisionError::InvalidCpuCores
        );
        assert_eq!(
            prepare_clone_environment(
                false, "src", true, "online", "", "CT", "", "dst", &existing, "bad", "8GiB", 4,
                false, "512", "100MB", "", "", "/tmp/ws", &resolver,
            )
            .unwrap_err(),
            ProvisionError::InvalidDiskQuota
        );
        assert_eq!(
            prepare_clone_environment(
                false, "src", true, "online", "", "CT", "", "dst", &existing, "40GiB", "bad", 4,
                false, "512", "100MB", "", "", "/tmp/ws", &resolver,
            )
            .unwrap_err(),
            ProvisionError::InvalidRamLimit
        );
        assert_eq!(
            prepare_clone_environment(
                false, "src", true, "online", "", "CT", "", "dst", &existing, "40GiB", "8GiB", 4,
                false, "512", "100MB", "bogus", "", "/tmp/ws", &resolver,
            )
            .unwrap_err(),
            ProvisionError::InvalidSwapLimit
        );
    }

    #[test]
    fn clone_environment_script_uses_btrfs_snapshot_when_supported() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("aibox-prov-btrfs-clone-{nanos}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let stub_dir = dir.join("stubs");
        fs::create_dir_all(&stub_dir).unwrap();
        let src_ws = dir.join("ws/src-box");
        let dst_ws = dir.join("ws/dst-box");
        fs::create_dir_all(&src_ws).unwrap();
        let btrfs_log = dir.join("btrfs.log");

        write_executable(&stub_dir.join("incus"), "#!/bin/sh\nexit 0\n");
        write_executable(&stub_dir.join("script"), "#!/bin/sh\nexec bash -c \"$4\"\n");
        write_executable(&stub_dir.join("sleep"), "#!/bin/sh\nexit 0\n");
        write_executable(
            &stub_dir.join("stat"),
            "#!/bin/sh\ncase \"$*\" in\n    *%T*) echo btrfs; exit 0;;\n    *%i*) echo 256; exit 0;;\nesac\nexec /usr/bin/stat \"$@\"\n",
        );
        write_executable(
            &stub_dir.join("btrfs"),
            "#!/bin/sh\necho \"$@\" >> \"$AIBOX_BTRFS_LOG\"\nif [ \"$1\" = subvolume ] && [ \"$2\" = snapshot ]; then mkdir -p \"$4\"; fi\nexit 0\n",
        );

        let script_path = dir.join("clone.sh");
        let script = clone_environment_script_with_source(
            "src-box",
            "dst-box",
            &dst_ws.to_string_lossy(),
            &src_ws.to_string_lossy(),
            "40GiB",
            "8GiB",
            4,
            false,
            "online",
            "",
            &[],
            "",
            "",
            "",
            false,
        );
        fs::write(&script_path, script).unwrap();

        let path_env = format!(
            "{}:{}",
            stub_dir.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let output = Command::new("/bin/bash")
            .arg(&script_path)
            .env("PATH", path_env)
            .env("AIBOX_BTRFS_LOG", &btrfs_log)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");

        let log = fs::read_to_string(&btrfs_log).unwrap();
        assert!(
            log.contains(&format!(
                "subvolume snapshot {} {}",
                src_ws.display(),
                dst_ws.display()
            )),
            "expected btrfs subvolume snapshot, got: {log}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn clone_environment_script_rolls_back_at_every_stage() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("aibox-prov-clone-rb-{nanos}"));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        let stub_dir = base.join("stubs");
        fs::create_dir_all(&stub_dir).unwrap();

        write_executable(
            &stub_dir.join("incus"),
            r#"#!/bin/sh
echo "$@" >> "$AIBOX_STUB_LOG"
if [ "$1" = copy ] && [ "$FAIL_AT" = "copy" ]; then exit 2; fi
if [ "$1" = config ] && [ "$2" = device ] && [ "$3" = add ] && [ "$FAIL_AT" = "device_add" ]; then exit 3; fi
if [ "$1" = start ] && [ "$FAIL_AT" = "start" ]; then exit 4; fi
exit 0
"#,
        );
        write_executable(&stub_dir.join("script"), "#!/bin/sh\nexec bash -c \"$4\"\n");
        write_executable(&stub_dir.join("sleep"), "#!/bin/sh\nexit 0\n");

        let path_env = format!(
            "{}:{}",
            stub_dir.display(),
            std::env::var("PATH").unwrap_or_default()
        );

        for fail_at in ["copy", "device_add", "start"] {
            let ws = base.join(format!("ws-{fail_at}"));
            let log_path = base.join(format!("incus-{fail_at}.log"));
            let script_path = base.join(format!("clone-{fail_at}.sh"));
            let script = clone_environment_script(
                "src-tpl",
                "dst-rb",
                &ws.to_string_lossy(),
                "40GiB",
                "8GiB",
                4,
                fail_at == "start",
                "online",
                "",
                &[],
                "",
                "",
                "",
                false,
            );
            fs::write(&script_path, script).unwrap();

            let output = Command::new("/bin/bash")
                .arg(&script_path)
                .env("PATH", &path_env)
                .env("AIBOX_STUB_LOG", &log_path)
                .env("FAIL_AT", fail_at)
                .output()
                .unwrap();
            assert!(!output.status.success(), "expected failure at {fail_at}");
            assert!(
                !ws.exists(),
                "workspace should have been rolled back on failure at {fail_at}"
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("[rollback]"),
                "expected rollback log at {fail_at}, got: {stderr}"
            );
            let log = fs::read_to_string(&log_path).unwrap();
            if fail_at == "copy" {
                assert!(!log.contains("delete dst-rb --force"), "log: {log}");
            } else {
                assert!(log.contains("delete dst-rb --force"), "log: {log}");
                assert!(
                    log.contains("network acl delete aibox-dst-rb"),
                    "log: {log}"
                );
            }
        }

        let _ = fs::remove_dir_all(&base);
    }
}
