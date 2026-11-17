use crate::config::defaults::{OsEntry, Preset};
use crate::health::limits::{is_valid_io_limit, is_valid_process_limit};
use crate::provision::{clean_limit_value, is_valid_size_spec};
use crate::snapshot::workspace::workspace_create_snippet;
use crate::util::names::sanitize_name;
use crate::util::paths::{cache_base_path, resolve_workspace_path, CONTAINER_WORKSPACE_PATH};
use crate::util::shell::{shell_array_words, shell_quote};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateEnvironmentPlan {
    pub name: String,
    pub title: String,
    pub script: String,
}

#[allow(clippy::too_many_arguments)]
pub fn create_environment_script(
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
    let mut root_opts = format!("size={}", disk_gib.trim());
    let clean_io = io_limit.trim();
    if !clean_io.is_empty() && !clean_io.eq_ignore_ascii_case("unlimited") {
        root_opts.push_str(&format!(",limits.read={clean_io},limits.write={clean_io}"));
    }

    let mut quota_args = format!(
        "-c limits.cpu={} -c limits.memory={} -d root,{root_opts}",
        cpu_cores.max(1),
        ram_gib.trim()
    );
    let clean_proc = process_limit.trim();
    if !is_vm && !clean_proc.is_empty() && !clean_proc.eq_ignore_ascii_case("unlimited") {
        quota_args.push_str(&format!(" -c limits.processes={clean_proc}"));
    }

    let ws_snippet = workspace_create_snippet(workspace);
    let cache_root = format!("{cache_base}.root");

    format!(
        "\nset -euo pipefail\nNAME={}\nLOCAL_ALIAS={}\nREMOTE_IMAGE={}\nWORKSPACE={}\nWS_PATH={}\nCACHE={}\nCACHE_ROOT={}\nPROFILE_ARGS=({})\nQUOTA_ARGS=({})\nX11={}\n\necho \"@@STAGE:8:Step 1/4 — Preparing shared workspace directory...\"\n{}\nif command -v setfacl >/dev/null 2>&1; then\n    ACL_USER=\"${{SUDO_USER:-${{USER:-$(id -un)}}}}\"\n    setfacl -R -m \"u:${{ACL_USER}}:rwx\" -m \"m::rwx\" -d -m \"u:${{ACL_USER}}:rwx\" -d -m \"m::rwx\" \"$WORKSPACE\" 2>/dev/null || true\nfi\n\nif incus image info \"$LOCAL_ALIAS\" >/dev/null 2>&1; then\n    echo \"@@STAGE:60:Step 2/4 — Using cached OS image ($LOCAL_ALIAS)...\"\n    SOURCE_IMAGE=\"$LOCAL_ALIAS\"\nelif [ -s \"$CACHE\" ] && [ -s \"$CACHE_ROOT\" ]; then\n    echo \"@@STAGE:30:Step 2/4 — Importing OS image from OS storage...\"\n    incus image import \"$CACHE\" \"$CACHE_ROOT\" --alias \"$LOCAL_ALIAS\"\n    SOURCE_IMAGE=\"$LOCAL_ALIAS\"\nelse\n    echo \"@@STAGE:12:Step 2/4 — Downloading OS image ($REMOTE_IMAGE)...\"\n    script -q -e -c \"incus image copy \\\"$REMOTE_IMAGE\\\" local: --alias \\\"$LOCAL_ALIAS\\\" --auto-update\" /dev/null\n    SOURCE_IMAGE=\"$LOCAL_ALIAS\"\nfi\n\necho \"@@STAGE:72:Step 3/4 — Launching isolated OS container & applying quotas...\"\nscript -q -e -c \"incus launch \\\"$SOURCE_IMAGE\\\" \\\"$NAME\\\" ${{PROFILE_ARGS[*]}} ${{QUOTA_ARGS[*]}}\" /dev/null\n\necho \"@@STAGE:86:Step 4/4 — Mounting $WS_PATH & waiting for network IP...\"\nsleep 1\n\nif ! incus config device add \"$NAME\" workspace disk source=\"$WORKSPACE\" path=\"$WS_PATH\" shift=true >/dev/null 2>&1; then\n    incus config device add \"$NAME\" workspace disk source=\"$WORKSPACE\" path=\"$WS_PATH\" >/dev/null\nfi\n\nif [[ \"$X11\" == \"1\" ]] && [[ -d /tmp/.X11-unix ]]; then\n    if command -v xhost >/dev/null 2>&1 && [[ -n \"${{DISPLAY:-}}\" ]]; then\n        xhost +local: >/dev/null 2>&1 || true\n    fi\n    incus config device add \"$NAME\" x11 disk source=/tmp/.X11-unix path=/tmp/.X11-unix >/dev/null 2>&1 || true\nfi\n\nincus exec \"$NAME\" -- sh -c '\n    mkdir -p \"$1\" && chmod 777 \"$1\" || true\n    if [ -f /etc/dhcpcd.conf ] && ! grep -q \"^noarp\" /etc/dhcpcd.conf; then\n        echo \"noarp\" >> /etc/dhcpcd.conf\n        pkill -HUP dhcpcd 2>/dev/null || udhcpc -i eth0 -n -q 2>/dev/null || true\n    fi\n' aibox \"$WS_PATH\" >/dev/null 2>&1 || true\n\nfor _ in 1 2 3 4 5; do\n    if incus list \"$NAME\" -f csv -c 4 | grep -q \"[0-9]\"; then\n        break\n    fi\n    sleep 1\ndone\n{}\necho \"@@STAGE:100:Environment ready!\"\n",
        shell_quote(name),
        shell_quote(local_alias),
        shell_quote(remote_image),
        shell_quote(workspace),
        shell_quote(CONTAINER_WORKSPACE_PATH),
        shell_quote(cache_base),
        shell_quote(&cache_root),
        shell_array_words(profile_flags),
        shell_array_words(&quota_args),
        if x11 { "1" } else { "0" },
        ws_snippet,
        ws_snippet,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_create_environment(
    busy: bool,
    raw_name: &str,
    existing_names: &[String],
    os_id: &str,
    os_entries: &[OsEntry],
    preset_id: &str,
    default_preset_id: &str,
    presets: &[Preset],
    disk_gib: &str,
    ram_gib: &str,
    cpu_cores: i32,
    process_limit: &str,
    io_limit: &str,
    workspace_parent: &str,
    workspace_base: &str,
    os_storage_path: &str,
) -> Result<CreateEnvironmentPlan, String> {
    if busy {
        return Err(
            "Another operation is currently in progress. Please wait or cancel it first."
                .to_string(),
        );
    }

    let name = sanitize_name(raw_name);
    if name.is_empty() {
        return Err(
            "Please enter a valid environment name (letters, numbers, hyphens).".to_string(),
        );
    }

    if existing_names.iter().any(|n| n == &name) {
        return Err(format!("Environment '{name}' already exists."));
    }

    if !is_valid_size_spec(disk_gib) {
        return Err("The disk quota is invalid. Use a value such as 35GiB.".to_string());
    }
    if !is_valid_size_spec(ram_gib) {
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

    let os_entry = os_entries
        .iter()
        .find(|e| e.id == os_id)
        .or_else(|| os_entries.first())
        .ok_or_else(|| {
            "There is no OS in the catalog. Add one in Configuration first.".to_string()
        })?;

    let preset = presets
        .iter()
        .find(|p| p.id == preset_id)
        .or_else(|| presets.iter().find(|p| p.id == default_preset_id))
        .or_else(|| presets.first())
        .ok_or_else(|| {
            "There is no capability preset. Add one in Configuration first.".to_string()
        })?;

    let host_workspace = resolve_workspace_path(workspace_parent, &name, workspace_base)?;
    let local_alias = format!("aibox-{}", os_entry.id);
    let cache_base = cache_base_path(os_storage_path, &os_entry.id);
    let is_vm = preset.flags.contains("--vm");

    let script = create_environment_script(
        &name,
        &local_alias,
        &format!("images:{}", os_entry.alias),
        &host_workspace,
        &cache_base,
        &preset.flags,
        ram_gib,
        disk_gib,
        cpu_cores,
        preset.x11,
        &clean_proc,
        &clean_io,
        is_vm,
    );

    Ok(CreateEnvironmentPlan {
        title: format!("Creating environment '{name}'"),
        name,
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
    fn create_environment_script_passes_preset_flags_as_arguments_match_qt_test() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("aibox-prov-create-{nanos}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let stub_dir = dir.join("stubs");
        fs::create_dir_all(&stub_dir).unwrap();
        let log_path = dir.join("incus.log");

        write_executable(
            &stub_dir.join("incus"),
            "#!/bin/sh\necho \"$@\" >> \"$AIBOX_STUB_LOG\"\nif [ \"$1\" = image ] && [ \"$2\" = info ]; then exit 1; fi\nif [ \"$1\" = list ]; then echo \"10.0.0.5\"; fi\nexit 0\n",
        );
        write_executable(&stub_dir.join("script"), "#!/bin/sh\nexec bash -c \"$4\"\n");
        write_executable(&stub_dir.join("sleep"), "#!/bin/sh\nexit 0\n");

        let script_path = dir.join("task.sh");
        let script = create_environment_script(
            "mobile-porting",
            "aibox-archlinux",
            "images:archlinux",
            &dir.join("workspace").to_string_lossy(),
            &dir.join("os-images/aibox-archlinux").to_string_lossy(),
            "-p default -p ai-gui",
            "6GiB",
            "60GiB",
            4,
            true,
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
            .env_remove("DISPLAY")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");

        let log = fs::read_to_string(&log_path).unwrap();
        assert!(
            log.contains(
                "launch aibox-archlinux mobile-porting -p default -p ai-gui -c limits.cpu=4 -c limits.memory=6GiB -d root,size=60GiB"
            ),
            "unexpected log: {log}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn runaway_limits_container_vs_vm_create_script() {
        let ct_script = create_environment_script(
            "ct-box",
            "aibox-ubuntu",
            "images:ubuntu/24.04",
            "/tmp/ws",
            "/tmp/cache",
            "",
            "6GiB",
            "35GiB",
            4,
            false,
            "512",
            "200MB",
            false,
        );
        assert!(ct_script.contains("limits.processes=512"));
        assert!(ct_script.contains("limits.read=200MB,limits.write=200MB"));

        let vm_script = create_environment_script(
            "vm-box",
            "aibox-ubuntu",
            "images:ubuntu/24.04",
            "/tmp/ws",
            "/tmp/cache",
            "--vm",
            "6GiB",
            "35GiB",
            4,
            false,
            "512",
            "200MB",
            true,
        );
        assert!(!vm_script.contains("limits.processes="));
        assert!(vm_script.contains("limits.read=200MB,limits.write=200MB"));
    }
}
