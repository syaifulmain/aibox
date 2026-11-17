//! Default OS catalog, presets, resource options, and diff excludes.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OsEntry {
    pub id: String,
    pub alias: String,
    pub name: String,
    pub badge: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub title: String,
    pub sub: String,
    pub flags: String,
    pub x11: bool,
}

pub fn default_os_entries() -> Vec<OsEntry> {
    let entry = |id: &str, alias: &str, name: &str, badge: &str, description: &str| OsEntry {
        id: id.to_string(),
        alias: alias.to_string(),
        name: name.to_string(),
        badge: badge.to_string(),
        description: description.to_string(),
    };

    vec![
        entry(
            "ubuntu-24.04",
            "ubuntu/24.04",
            "Ubuntu 24.04 LTS (Noble)",
            "Recommended",
            "Best compatibility for AI tools, Desktop (Electron/Qt/Flutter), and Android SDK.",
        ),
        entry(
            "ubuntu-22.04",
            "ubuntu/22.04",
            "Ubuntu 22.04 LTS (Jammy)",
            "LTS",
            "Proven long-term support release for legacy C++, Python, and mobile toolchains.",
        ),
        entry(
            "debian-12",
            "debian/12",
            "Debian 12 (Bookworm)",
            "Lightweight",
            "Rock-solid stability with low RAM overhead (~60 MB idle). Ideal for backend & databases.",
        ),
        entry(
            "archlinux",
            "archlinux",
            "Arch Linux",
            "Rolling",
            "Bleeding-edge packages matching Omarchy. Great for native C++, Rust, and Qt6 builds.",
        ),
        entry(
            "fedora-41",
            "fedora/41",
            "Fedora 41",
            "Modern",
            "Up-to-date workstation & server stack with modern GCC, Podman/Docker, and Wayland.",
        ),
        entry(
            "alpine-3.21",
            "alpine/3.21",
            "Alpine Linux 3.21",
            "Ultra-Light",
            "Featherweight (~5 MB) musl/BusyBox system. Boots in 0.5s for rapid CLI/microservice tests.",
        ),
        entry(
            "kali",
            "kali",
            "Kali Linux Rolling",
            "Security",
            "Pre-configured distribution for security auditing and penetration testing projects.",
        ),
        entry(
            "rockylinux-9",
            "rockylinux/9",
            "Rocky Linux 9",
            "Enterprise",
            "100% bug-for-bug RHEL 9 compatible environment for production server testing.",
        ),
    ]
}

pub fn default_presets() -> Vec<Preset> {
    let preset = |id: &str, title: &str, sub: &str, flags: &str, x11: bool| Preset {
        id: id.to_string(),
        title: title.to_string(),
        sub: sub.to_string(),
        flags: flags.to_string(),
        x11,
    };

    vec![
        preset(
            "full",
            "Desktop & Mobile",
            "GPU + KVM + GUI Bridge",
            "-p default -p ai-gui",
            true,
        ),
        preset(
            "standard",
            "Standard AI / Web",
            "Docker + Isolated OS",
            "-p default -c security.nesting=true",
            false,
        ),
        preset(
            "vm",
            "Virtual Machine",
            "Full KVM Kernel (--vm)",
            "--vm -p default",
            false,
        ),
    ]
}

pub fn default_disk_options() -> Vec<String> {
    vec![
        "20GiB".to_string(),
        "35GiB".to_string(),
        "60GiB".to_string(),
        "100GiB".to_string(),
    ]
}

pub fn default_ram_options() -> Vec<String> {
    vec![
        "4GiB".to_string(),
        "6GiB".to_string(),
        "8GiB".to_string(),
        "12GiB".to_string(),
    ]
}

pub fn default_cpu_options() -> Vec<i32> {
    vec![2, 4, 6, 8]
}

pub fn default_diff_excludes() -> Vec<String> {
    vec![
        ".git/".to_string(),
        "node_modules/".to_string(),
        "target/".to_string(),
        ".venv/".to_string(),
        "__pycache__/".to_string(),
        "build/".to_string(),
        ".aibox-paste/".to_string(),
    ]
}
