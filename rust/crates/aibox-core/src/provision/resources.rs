use crate::config::defaults::OsEntry;
use crate::provision::templates::{render_template, PREFETCH_IMAGE_TEMPLATE};
use crate::provision::{
    is_host_swap_value, is_valid_size_spec, swap_config_value, validate_limits,
    validate_swap_limit, ProvisionError,
};
use crate::util::paths::cache_base_path;
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

#[allow(clippy::too_many_arguments)]
pub fn update_resources_script(
    name: &str,
    cpu_cores: i32,
    ram_gib: &str,
    disk_gib: &str,
    process_limit: &str,
    io_limit: &str,
    swap_limit: &str,
    is_vm: bool,
) -> String {
    let q_name = shell_quote(name);
    let mut lines = Vec::new();
    lines.push("set -euo pipefail".to_string());
    lines.push("echo '@@STAGE:25:Updating CPU and memory quotas...'".to_string());
    if cpu_cores > 0 {
        lines.push(format!(
            "incus config set {q_name} limits.cpu={}",
            shell_quote(&cpu_cores.to_string())
        ));
    }
    let trimmed_ram = ram_gib.trim();
    if !trimmed_ram.is_empty() {
        lines.push(format!(
            "incus config set {q_name} limits.memory={}",
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
                "incus config set {q_name} limits.processes={}",
                shell_quote(clean_proc)
            ));
        }

        let clean_swap = swap_limit.trim();
        if !clean_swap.is_empty() {
            if is_host_swap_value(clean_swap) {
                lines.push(format!(
                    "incus config unset {q_name} limits.memory.swap >/dev/null 2>&1 || true"
                ));
            } else if let Some(swap_value) = swap_config_value(clean_swap) {
                lines.push(format!(
                    "incus config set {q_name} limits.memory.swap={}",
                    shell_quote(&swap_value)
                ));
            }
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
    swap_limit: &str,
    env_exists: bool,
    used_bytes: i64,
    disk_usage_label: &str,
    is_vm: bool,
) -> Result<UpdateResourcesPlan, ProvisionError> {
    if name.is_empty() {
        return Err(ProvisionError::EmptySelection);
    }
    if busy {
        return Err(ProvisionError::Busy);
    }
    if !(1..=1024).contains(&cpu_cores) {
        return Err(ProvisionError::InvalidCpuCores);
    }
    if !is_valid_size_spec(ram_gib) {
        return Err(ProvisionError::InvalidRamLimit);
    }
    if !is_valid_size_spec(disk_gib) {
        return Err(ProvisionError::InvalidDiskQuota);
    }

    let (clean_proc, clean_io) = validate_limits(process_limit, io_limit)?;
    validate_swap_limit(swap_limit)?;

    if !env_exists {
        return Err(ProvisionError::NotFound(name.to_string()));
    }

    let trimmed_disk = disk_gib.trim();
    let target_bytes = parse_size_bytes(trimmed_disk);
    if used_bytes > 0 && target_bytes > 0 && target_bytes < used_bytes {
        return Err(ProvisionError::DiskShrinkBelowUsage {
            target: trimmed_disk.to_string(),
            current: disk_usage_label.to_string(),
        });
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
            swap_limit,
            is_vm,
        ),
    })
}

pub fn download_os_image_script(os_id: &str, alias: &str, cache_base: &str) -> OsImageTaskPlan {
    let local_alias = format!("aibox-{os_id}");
    let remote_image = format!("images:{alias}");
    let cache_root = format!("{cache_base}.root");
    let q_dl_msg = shell_quote(&format!(
        "@@STAGE:5:Downloading {os_id} from the official image server..."
    ));
    let script = format!(
        "\nset -euo pipefail\nLOCAL_ALIAS={}\nREMOTE_IMAGE={}\nCACHE={}\nCACHE_ROOT={}\n\nmkdir -p \"$(dirname \"$CACHE\")\"\nif incus image info \"$LOCAL_ALIAS\" >/dev/null 2>&1; then\n    echo \"@@STAGE:45:OS image is already in the local Incus store...\"\nelif [ -s \"$CACHE\" ] && [ -s \"$CACHE_ROOT\" ]; then\n    echo \"@@STAGE:35:Importing OS image from OS storage...\"\n    incus image import \"$CACHE\" \"$CACHE_ROOT\" --alias \"$LOCAL_ALIAS\"\nelse\n    echo {q_dl_msg}\n    script -q -e -c \"incus image copy \\\"$REMOTE_IMAGE\\\" local: --alias \\\"$LOCAL_ALIAS\\\" --auto-update\" /dev/null\nfi\n\nif [ ! -s \"$CACHE\" ] || [ ! -s \"$CACHE_ROOT\" ]; then\n    echo \"@@STAGE:85:Saving OS image to OS storage...\"\n    rm -f \"$CACHE\" \"$CACHE_ROOT\"\n    incus image export \"$LOCAL_ALIAS\" \"$CACHE\" >/dev/null\nfi\necho \"@@STAGE:100:Download complete!\"\n",
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

/// Builds a quiet, idempotent background script to pre-fetch an OS image into Incus and OS storage.
pub fn prefetch_os_image_script(os_id: &str, alias: &str, cache_base: &str) -> OsImageTaskPlan {
    let local_alias = format!("aibox-{os_id}");
    let remote_image = format!("images:{alias}");
    let cache_root = format!("{cache_base}.root");
    let script = render_template(
        PREFETCH_IMAGE_TEMPLATE,
        &[
            ("LOCAL_ALIAS", &shell_quote(&local_alias)),
            ("REMOTE_IMAGE", &shell_quote(&remote_image)),
            ("CACHE", &shell_quote(cache_base)),
            ("CACHE_ROOT", &shell_quote(&cache_root)),
        ],
    );
    OsImageTaskPlan {
        title: format!("Pre-fetching OS image '{os_id}'"),
        script,
    }
}

/// Validates and builds a background pre-fetch plan when an OS catalog entry is selected.
/// Returns `Ok(None)` if the image is already cached locally.
pub fn prepare_prefetch_os_image(
    os_id: &str,
    os_entries: &[OsEntry],
    os_storage_path: &str,
    already_cached: bool,
) -> Result<Option<OsImageTaskPlan>, ProvisionError> {
    let clean_id = os_id.trim();
    if clean_id.is_empty() {
        return Err(ProvisionError::EmptySelection);
    }
    let entry = os_entries
        .iter()
        .find(|e| e.id == clean_id)
        .ok_or_else(|| ProvisionError::UnknownOsEntry(clean_id.to_string()))?;
    if already_cached {
        return Ok(None);
    }
    let cache_base = cache_base_path(os_storage_path, &entry.id);
    Ok(Some(prefetch_os_image_script(
        &entry.id,
        &entry.alias,
        &cache_base,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_resource_editing_and_shrink_guard_match_qt_test() {
        assert_eq!(parse_size_bytes("10GiB"), 10_737_418_240);
        assert_eq!(parse_size_bytes("512MiB"), 536_870_912);

        let ct_update =
            update_resources_script("box1", 6, "12GiB", "80GiB", "2048", "200MB", "4GiB", false);
        assert!(ct_update.contains("limits.cpu='6'"));
        assert!(ct_update.contains("limits.memory='12GiB'"));
        assert!(ct_update.contains("limits.processes='2048'"));
        assert!(ct_update.contains("limits.memory.swap='4GiB'"));
        assert!(ct_update.contains("root size='80GiB'"));
        assert!(ct_update.contains("limits.read='200MB' limits.write='200MB'"));

        let vm_update =
            update_resources_script("vm1", 4, "8GiB", "60GiB", "2048", "Unlimited", "4GiB", true);
        assert!(!vm_update.contains("limits.processes"));
        assert!(!vm_update.contains("limits.memory.swap"));
        assert!(vm_update.contains("device unset 'vm1' root limits.read"));

        let shrink_reset =
            update_resources_script("box1", 2, "4GiB", "20GiB", "Unlimited", "", "Host", false);
        assert!(shrink_reset.contains("config unset 'box1' limits.processes"));
        assert!(shrink_reset.contains("config unset 'box1' limits.memory.swap"));

        let shrink_err = validate_and_prepare_update_resources(
            "box1",
            false,
            2,
            "4GiB",
            "10GiB",
            "Unlimited",
            "",
            "",
            true,
            20 * 1024 * 1024 * 1024,
            "20.0 GiB",
            false,
        )
        .unwrap_err();
        assert_eq!(
            shrink_err.to_string(),
            "Cannot shrink disk quota (10GiB) below current usage (20.0 GiB)."
        );

        assert_eq!(
            validate_and_prepare_update_resources(
                "box1",
                false,
                2,
                "4GiB",
                "20GiB",
                "Unlimited",
                "",
                "bogus",
                true,
                0,
                "",
                false,
            )
            .unwrap_err(),
            ProvisionError::InvalidSwapLimit
        );
    }

    #[test]
    fn prefetch_os_image_validates_and_skips_when_cached() {
        let catalog = vec![OsEntry {
            id: "archlinux".into(),
            alias: "archlinux".into(),
            name: "Arch Linux".into(),
            badge: "Rolling".into(),
            description: "Arch".into(),
        }];

        assert_eq!(
            prepare_prefetch_os_image("", &catalog, "/tmp/os", false).unwrap_err(),
            ProvisionError::EmptySelection
        );
        assert_eq!(
            prepare_prefetch_os_image("unknown", &catalog, "/tmp/os", false).unwrap_err(),
            ProvisionError::UnknownOsEntry("unknown".into())
        );
        assert!(
            prepare_prefetch_os_image("archlinux", &catalog, "/tmp/os", true)
                .unwrap()
                .is_none()
        );

        let plan = prepare_prefetch_os_image("archlinux", &catalog, "/tmp/os", false)
            .unwrap()
            .expect("expected prefetch plan");
        assert!(plan.title.contains("archlinux"));
        assert!(plan.script.contains("incus image copy"));
        assert!(plan.script.contains("incus image export"));
    }
}
