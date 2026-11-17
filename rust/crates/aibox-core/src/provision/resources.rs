use crate::health::limits::{is_valid_io_limit, is_valid_process_limit};
use crate::provision::{clean_limit_value, is_valid_size_spec};
use crate::util::shell::shell_quote;
use crate::util::size::parse_size_bytes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateResourcesPlan {
    pub title: String,
    pub script: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsImageTaskPlan {
    pub title: String,
    pub script: String,
}

pub fn update_resources_script(
    name: &str,
    cpu_cores: i32,
    ram_gib: &str,
    disk_gib: &str,
    process_limit: &str,
    io_limit: &str,
    is_vm: bool,
) -> String {
    let q_name = shell_quote(name);
    let mut lines = Vec::new();
    lines.push("set -euo pipefail".to_string());
    lines.push("echo '@@STAGE:25:Updating CPU and memory quotas...'".to_string());
    if cpu_cores > 0 {
        lines.push(format!(
            "incus config set {q_name} limits.cpu {}",
            shell_quote(&cpu_cores.to_string())
        ));
    }
    let trimmed_ram = ram_gib.trim();
    if !trimmed_ram.is_empty() {
        lines.push(format!(
            "incus config set {q_name} limits.memory {}",
            shell_quote(trimmed_ram)
        ));
    }

    if !is_vm {
        let clean_proc = process_limit.trim();
        if clean_proc.is_empty() || clean_proc.eq_ignore_ascii_case("unlimited") {
            lines.push(format!(
                "incus config unset {q_name} limits.processes >/dev/null 2>&1 || true"
            ));
        } else {
            lines.push(format!(
                "incus config set {q_name} limits.processes {}",
                shell_quote(clean_proc)
            ));
        }
    }

    lines.push("echo '@@STAGE:65:Updating root disk quota and I/O limits...'".to_string());
    let trimmed_disk = disk_gib.trim();
    if !trimmed_disk.is_empty() {
        let q_disk = shell_quote(trimmed_disk);
        lines.push(format!(
            "incus config device set {q_name} root size={q_disk} >/dev/null 2>&1 || incus config device override {q_name} root size={q_disk}"
        ));
    }

    let clean_io = io_limit.trim();
    if clean_io.is_empty() || clean_io.eq_ignore_ascii_case("unlimited") {
        lines.push(format!(
            "incus config device unset {q_name} root limits.read >/dev/null 2>&1 || true"
        ));
        lines.push(format!(
            "incus config device unset {q_name} root limits.write >/dev/null 2>&1 || true"
        ));
    } else {
        let q_io = shell_quote(clean_io);
        lines.push(format!(
            "incus config device set {q_name} root limits.read={q_io} limits.write={q_io} >/dev/null 2>&1 || incus config device override {q_name} root limits.read={q_io} limits.write={q_io}"
        ));
    }

    lines.push("echo '@@STAGE:100:Quotas updated.'".to_string());
    lines.join("\n")
}

#[allow(clippy::too_many_arguments)]
pub fn validate_and_prepare_update_resources(
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
) -> Result<UpdateResourcesPlan, String> {
    if name.is_empty() {
        return Err("Select an environment first.".to_string());
    }
    if busy {
        return Err("Another operation is currently in progress.".to_string());
    }
    if !(1..=1024).contains(&cpu_cores) {
        return Err("CPU cores must be between 1 and 1024.".to_string());
    }
    if !is_valid_size_spec(ram_gib) {
        return Err("The RAM limit is invalid. Use a value such as 8GiB.".to_string());
    }
    if !is_valid_size_spec(disk_gib) {
        return Err("The disk quota is invalid. Use a value such as 50GiB.".to_string());
    }

    let clean_proc = clean_limit_value(process_limit);
    let clean_io = clean_limit_value(io_limit);
    if !is_valid_process_limit(&clean_proc) {
        return Err(
            "The process limit is invalid. Use an integer >= 10 (e.g. 4096) or Unlimited."
                .to_string(),
        );
    }
    if !is_valid_io_limit(&clean_io) {
        return Err(
            "The disk I/O limit is invalid. Use a rate such as 200MB, 2500iops, or Unlimited."
                .to_string(),
        );
    }

    if !env_exists {
        return Err(format!("Environment '{name}' was not found."));
    }

    let trimmed_disk = disk_gib.trim();
    let target_bytes = parse_size_bytes(trimmed_disk);
    if used_bytes > 0 && target_bytes > 0 && target_bytes < used_bytes {
        return Err(format!(
            "Cannot shrink disk quota ({trimmed_disk}) below current usage ({disk_usage_label})."
        ));
    }

    Ok(UpdateResourcesPlan {
        title: format!("Updating resource quotas for '{name}'"),
        script: update_resources_script(
            name,
            cpu_cores,
            ram_gib,
            disk_gib,
            &clean_proc,
            &clean_io,
            is_vm,
        ),
    })
}

pub fn download_os_image_script(os_id: &str, alias: &str, cache_base: &str) -> OsImageTaskPlan {
    let local_alias = format!("aibox-{os_id}");
    let remote_image = format!("images:{alias}");
    let cache_root = format!("{cache_base}.root");
    let script = format!(
        "\nset -euo pipefail\nLOCAL_ALIAS={}\nREMOTE_IMAGE={}\nCACHE={}\nCACHE_ROOT={}\n\nmkdir -p \"$(dirname \"$CACHE\")\"\nif incus image info \"$LOCAL_ALIAS\" >/dev/null 2>&1; then\n    echo \"@@STAGE:45:OS image is already in the local Incus store...\"\nelif [ -s \"$CACHE\" ] && [ -s \"$CACHE_ROOT\" ]; then\n    echo \"@@STAGE:35:Importing OS image from OS storage...\"\n    incus image import \"$CACHE\" \"$CACHE_ROOT\" --alias \"$LOCAL_ALIAS\"\nelse\n    echo \"@@STAGE:5:Downloading {os_id} from the official image server...\"\n    script -q -e -c \"incus image copy \\\"$REMOTE_IMAGE\\\" local: --alias \\\"$LOCAL_ALIAS\\\" --auto-update\" /dev/null\nfi\n\nif [ ! -s \"$CACHE\" ] || [ ! -s \"$CACHE_ROOT\" ]; then\n    echo \"@@STAGE:85:Saving OS image to OS storage...\"\n    rm -f \"$CACHE\" \"$CACHE_ROOT\"\n    incus image export \"$LOCAL_ALIAS\" \"$CACHE\" >/dev/null\nfi\necho \"@@STAGE:100:Download complete!\"\n",
        shell_quote(&local_alias),
        shell_quote(&remote_image),
        shell_quote(cache_base),
        shell_quote(&cache_root),
    );
    OsImageTaskPlan {
        title: format!("Downloading OS image '{os_id}'"),
        script,
    }
}

pub fn remove_os_image_script(os_id: &str, cache_base: &str) -> OsImageTaskPlan {
    let local_alias = format!("aibox-{os_id}");
    let cache_root = format!("{cache_base}.root");
    let q_alias = shell_quote(&local_alias);
    let script = format!(
        "set -euo pipefail\n\
         echo '@@STAGE:50:Removing image cache...'\n\
         if incus image info {q_alias} >/dev/null 2>&1; then incus image delete {q_alias}; fi\n\
         rm -f {} {}\n\
         echo '@@STAGE:100:Removed.'",
        shell_quote(cache_base),
        shell_quote(&cache_root),
    );
    OsImageTaskPlan {
        title: format!("Removing cached OS image '{os_id}'"),
        script,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_resource_editing_and_shrink_guard_match_qt_test() {
        assert_eq!(parse_size_bytes("10GiB"), 10_737_418_240);
        assert_eq!(parse_size_bytes("512MiB"), 536_870_912);

        let ct_update =
            update_resources_script("box1", 6, "12GiB", "80GiB", "2048", "200MB", false);
        assert!(ct_update.contains("limits.cpu '6'"));
        assert!(ct_update.contains("limits.memory '12GiB'"));
        assert!(ct_update.contains("limits.processes '2048'"));
        assert!(ct_update.contains("root size='80GiB'"));
        assert!(ct_update.contains("limits.read='200MB' limits.write='200MB'"));

        let vm_update =
            update_resources_script("vm1", 4, "8GiB", "60GiB", "2048", "Unlimited", true);
        assert!(!vm_update.contains("limits.processes"));
        assert!(vm_update.contains("device unset 'vm1' root limits.read"));

        let shrink_reset =
            update_resources_script("box1", 2, "4GiB", "20GiB", "Unlimited", "", false);
        assert!(shrink_reset.contains("config unset 'box1' limits.processes"));

        let shrink_err = validate_and_prepare_update_resources(
            "box1",
            false,
            2,
            "4GiB",
            "10GiB",
            "Unlimited",
            "",
            true,
            20 * 1024 * 1024 * 1024,
            "20.0 GiB",
            false,
        )
        .unwrap_err();
        assert_eq!(
            shrink_err,
            "Cannot shrink disk quota (10GiB) below current usage (20.0 GiB)."
        );
    }
}
