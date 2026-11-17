use crate::health::limits::{is_valid_io_limit, is_valid_process_limit};
use crate::network::acl::acl_apply_shell_snippet;
use crate::network::allowlist::{parse_allowlist_entries, resolve_allowlist_cidrs, HostResolver};
use crate::provision::{clean_limit_value, is_valid_size_spec};
use crate::snapshot::workspace::workspace_create_snippet;
use crate::util::names::sanitize_name;
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

    format!(
        "\nset -euo pipefail\nSRC={}\nDST={}\nWORKSPACE={}\nWS_PATH={}\n\necho \"@@STAGE:12:Step 1/3 — Preparing fresh workspace directory...\"\n{}\nif command -v setfacl >/dev/null 2>&1; then\n    ACL_USER=\"${{SUDO_USER:-${{USER:-$(id -un)}}}}\"\n    setfacl -R -m \"u:${{ACL_USER}}:rwx\" -m \"m::rwx\" -d -m \"u:${{ACL_USER}}:rwx\" -d -m \"m::rwx\" \"$WORKSPACE\" 2>/dev/null || true\nfi\n\necho \"@@STAGE:35:Step 2/3 — Cloning environment with Btrfs copy-on-write...\"\nscript -q -e -c \"incus copy \\\"$SRC\\\" \\\"$DST\\\" --stateless\" /dev/null\n\necho \"@@STAGE:70:Step 3/3 — Configuring isolated workspace & limits...\"\nincus config unset \"$DST\" user.aibox.template >/dev/null 2>&1 || true\nincus config device remove \"$DST\" workspace >/dev/null 2>&1 || true\nif ! incus config device add \"$DST\" workspace disk source=\"$WORKSPACE\" path=\"$WS_PATH\" shift=true >/dev/null 2>&1; then\n    incus config device add \"$DST\" workspace disk source=\"$WORKSPACE\" path=\"$WS_PATH\" >/dev/null\nfi\n{}\n{}\n{}\necho \"@@STAGE:100:Clone ready!\"\n",
        shell_quote(source_name),
        shell_quote(new_name),
        shell_quote(workspace),
        shell_quote(CONTAINER_WORKSPACE_PATH),
        workspace_create_snippet(workspace),
        quota_lines.trim(),
        net_snippet,
        start_snippet,
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
    raw_new_name: &str,
    existing_names: &[String],
    disk_gib: &str,
    ram_gib: &str,
    cpu_cores: i32,
    start_after: bool,
    process_limit: &str,
    io_limit: &str,
    workspace_parent: &str,
    workspace_base: &str,
    resolver: &R,
) -> Result<CloneEnvironmentPlan, String> {
    if busy {
        return Err(
            "Another operation is currently in progress. Please wait or cancel it first."
                .to_string(),
        );
    }
    if !source_exists {
        return Err(format!("Source environment '{source_name}' was not found."));
    }
    let new_name = sanitize_name(raw_new_name);
    if new_name.is_empty() {
        return Err(
            "Please enter a valid environment name (letters, numbers, hyphens).".to_string(),
        );
    }
    if existing_names.iter().any(|n| n == &new_name) {
        return Err(format!("Environment '{new_name}' already exists."));
    }

    if !disk_gib.trim().is_empty() && !is_valid_size_spec(disk_gib) {
        return Err("The disk quota is invalid. Use a value such as 35GiB.".to_string());
    }
    if !ram_gib.trim().is_empty() && !is_valid_size_spec(ram_gib) {
        return Err("The RAM limit is invalid. Use a value such as 6GiB.".to_string());
    }

    let clean_proc = clean_limit_value(process_limit);
    let clean_io = clean_limit_value(io_limit);
    if !is_valid_process_limit(&clean_proc) {
        return Err(
            "The process limit is invalid. Use an integer >= 10 (e.g. 512) or leave unlimited."
                .to_string(),
        );
    }
    if !is_valid_io_limit(&clean_io) {
        return Err(
            "The disk I/O limit is invalid. Use a rate such as 200MB or 2500iops.".to_string(),
        );
    }

    let resolved_cidrs = if source_net_mode == "allowlist" {
        let entries = parse_allowlist_entries(source_net_allowlist);
        resolve_allowlist_cidrs(&entries, resolver).0
    } else {
        Vec::new()
    };

    let is_vm = source_type.eq_ignore_ascii_case("VM");
    let host_workspace = resolve_workspace_path(workspace_parent, &new_name, workspace_base)?;
    let script = clone_environment_script(
        source_name,
        &new_name,
        &host_workspace,
        disk_gib,
        ram_gib,
        cpu_cores,
        start_after,
        source_net_mode,
        source_net_allowlist,
        &resolved_cidrs,
        &clean_proc,
        &clean_io,
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
        assert!(log.contains("copy node-tpl cloned-box --stateless"));
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
            false,
        );
        assert!(clone_script.contains("limits.processes '1024'"));
        assert!(clone_script.contains("limits.read='100MB' limits.write='100MB'"));
    }
}
