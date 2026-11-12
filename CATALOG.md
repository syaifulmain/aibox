# CATALOG.md — System Architecture, File Inventory, & Changelog

This document tracks all host system configurations, project files, architecture decisions, and the changelog of **AIBox** for easy maintenance, auditing, and future re-installation.

---

## 1. Host System Modifications (Omarchy / Arch Linux)

All host-level configurations are automated in [bin/setup-system](file:///home/saep/Work/aibox/bin/setup-system) and can be reverted cleanly using [bin/uninstall](file:///home/saep/Work/aibox/bin/uninstall).

| System Component | Path / Name | Purpose |
| :--- | :--- | :--- |
| **System Packages** | `incus`, `dnsmasq`, `xorg-xhost`, `acl`, `qemu-system-x86`, `edk2-ovmf` | System container & VM manager, local DHCP/DNS server, X11/Wayland display bridge, and POSIX ACL utilities. |
| **Subordinate UID/GID** | `/etc/subuid` & `/etc/subgid` (`root:1000000:1000000000`) | Maps container `root` to an unprivileged UID range on the Host so autonomous AI agents cannot damage the host OS. |
| **User Groups** | `incus-admin`, `kvm`, `render`, `video` (for user `saep`) | Grants passwordless access to Incus, hardware virtualization (`/dev/kvm`), and Intel Iris Xe GPU acceleration (`/dev/dri`). |
| **Systemd Drop-in ACL** | `/etc/systemd/system/incus.service.d/acl.conf` | Automatically applies read/write ACL permissions on `/var/lib/incus/unix.socket` when `incus.service` starts. |
| **Btrfs Storage Pool** | `default` (`/var/lib/incus/storage-pools/default`) | Native Btrfs subvolume storage pool with **Btrfs Quotas** enabled for strict per-environment disk size limits and <1s copy-on-write snapshots. |
| **Network Bridge** | `incusbr0` (`10.143.44.1/24`, IPv4 NAT enabled, IPv6 disabled) | Managed private bridge providing automatic DHCP IPv4 addresses and internet access to all environments. |
| **UFW Firewall Rules** | `ufw allow in on incusbr0` & `ufw route allow in/out on incusbr0` | Permits DHCP, DNS, and routed internet traffic from `incusbr0` through the active host UFW firewall. |
| **GPU & KVM Profile** | Incus profile `ai-gui` (`security.nesting=true`, `gpu` & `kvm` devices) | Enables nested Docker, hardware-accelerated Desktop GUI apps, and full-speed Android Emulators inside containers. |
| **Shared Workspaces** | `<project>/.aibox/workspaces/<name>` (configurable) ⟷ `/workspace` | Two-way shared directory between Host and container using POSIX default ACLs (`setfacl`) and UID shifting (`shift=true`). Existing environments keep the workspace path they were created with. |
| **App Configuration** | `<project>/.aibox/config.json` | Persists workspace and OS image storage locations, the editable OS catalog, preset titles, disk/RAM/CPU choices, defaults, and first-run completion. |
| **OS Image Storage** | `<project>/.aibox/os-images` (configurable) | Offline OS image exports (`<id>` metadata + `<id>.root`) used as a fallback import source when the local Incus image is missing. |
| **Background Task State** | `<project>/.aibox/state/` (`task.pid`, `task.meta`, `task.log`, `task.sh`) | Tracks detached background operations (`setsid`) so tasks survive GUI window closure and automatically re-attach on startup. |

---

## 2. Project File Inventory (`~/Work/aibox`)

Built following the **Omarchy** native desktop application architecture (C++17 + Qt Quick 6 + `qmake6` + `makepkg`).

| File / Path | Category | Description |
| :--- | :--- | :--- |
| [aibox.pro](file:///home/saep/Work/aibox/aibox.pro) | Build Config | `qmake6` project file defining Qt6 modules, C++ sources, and compiled resources. |
| [src/main.cpp](file:///home/saep/Work/aibox/src/main.cpp) | Source (C++) | Application entry point supporting both the QML GUI and headless CLI inspection (`aibox list`, `aibox check`). |
| [src/backend.h](file:///home/saep/Work/aibox/src/backend.h) | Source (C++) | `Backend` class declaration exposing environments, OS catalog, configuration state, first-run state, and all `Q_INVOKABLE` actions to QML. |
| [src/backend.cpp](file:///home/saep/Work/aibox/src/backend.cpp) | Source (C++) | Incus JSON parser (including per-environment workspace device paths), dynamic OS catalog, JSON configuration persistence and validation, OS image export/import cache, PTY progress stream parser (`script -q -e -c`), process-group cancellation (`cancelTask`), and background task persistence. |
| [src/theme.h](file:///home/saep/Work/aibox/src/theme.h) & [src/theme.cpp](file:///home/saep/Work/aibox/src/theme.cpp) | Source (C++) | Live file watcher for Omarchy's active theme (`~/.local/state/omarchy/current/theme/colors.toml`) and Hyprland's window rounding. |
| [src/translator.h](file:///home/saep/Work/aibox/src/translator.h) & [src/translator.cpp](file:///home/saep/Work/aibox/src/translator.cpp) | Source (C++) | Live translation loader following `~/.config/omarchy/language`. |
| [src/Main.qml](file:///home/saep/Work/aibox/src/Main.qml) | Source (QML) | Main application window, custom `FlatInput`, `ActionButton`, and monochrome `SlidersIcon` components, first-run workspace dialog, modern Configuration menu (storage, OS catalog CRUD, create options), environment inspector, OS catalog, creation dialog, live progress bar, and keyboard shortcuts. |
| [src/resources.qrc](file:///home/saep/Work/aibox/src/resources.qrc) | Resource | Bundles QML and translation catalogs directly into the `/usr/bin/aibox` binary. |
| [src/i18n/en.json](file:///home/saep/Work/aibox/src/i18n/en.json) & [da.json](file:///home/saep/Work/aibox/src/i18n/da.json) | Localization | English plural rules and localization catalogs. |
| [tests/aibox_tests.pro](file:///home/saep/Work/aibox/tests/aibox_tests.pro) & [aibox_tests.cpp](file:///home/saep/Work/aibox/tests/aibox_tests.cpp) | Unit Tests | 20 offscreen Qt Test cases covering Incus JSON parsing, workspace device paths, name sanitization, first-run/config persistence, storage settings, OS catalog CRUD, OS image cache detection, create-option validation, background task state, `Theme`, and `Translator`. |
| [pkgbuild/PKGBUILD](file:///home/saep/Work/aibox/pkgbuild/PKGBUILD) | Packaging | Arch Linux package definition installing `/usr/bin/aibox`, license, SVG icon, and `.desktop` entry. |
| [pkgbuild/aibox.desktop](file:///home/saep/Work/aibox/pkgbuild/aibox.desktop) | Launcher | Desktop entry for the Omarchy App Launcher (`Super + Space`). |
| [pkgbuild/aibox.svg](file:///home/saep/Work/aibox/pkgbuild/aibox.svg) | Asset | Scalable SVG application icon. |
| [bin/setup-system](file:///home/saep/Work/aibox/bin/setup-system) | Script | Automated host setup script for Incus, Btrfs storage pool, UFW rules, and GPU/KVM profile. |
| [bin/build](file:///home/saep/Work/aibox/bin/build) | Script | Compiles the application into `build/aibox`. |
| [bin/test](file:///home/saep/Work/aibox/bin/test) | Script | Builds and runs the offscreen Qt Test suite. |
| [bin/install](file:///home/saep/Work/aibox/bin/install) | Script | Standard installation entry point; builds the application and installs the Arch Linux package via `makepkg -fsi`. |
| [bin/uninstall](file:///home/saep/Work/aibox/bin/uninstall) | Script | Cleanly removes the `aibox` package (with `--full` option to also remove Incus and all containers). |
| [AGENTS.md](file:///home/saep/Work/aibox/AGENTS.md) | Agent Policy | Mandatory instructions and workflow rules for AI agents (enforces automatic build, test, and system update). |

---

## 3. Changelog

### Iteration 9 — Project-Local Environment & Data Defaults
* **Unified Installer**: Use `./bin/install` as the standard operation to build and install the Arch package.
* **Project-Local Data**: Defaults now place workspaces, OS image copies, configuration, and detached task state under `<project>/.aibox/`, anchored to the process working directory. `.aibox/` is gitignored to prevent environment data and large OS image caches from entering version control.
* **First-Run Welcome**: Retains its focused single **Continue** button; data defaults are project-local when AIBox is launched from the project directory.

### Iteration 8 — Simplified Welcome & AIBox-Prefixed Storage Defaults
* **First-Run Welcome**: Removed the workspace-location instructions, path field, browse button, default action, and footer from the welcome screen. It now presents a single **Continue** button.
* **AIBox Storage Defaults**: At that time, new installations defaulted to `~/AIBox/workspaces` and `~/AIBox/os-images`.

### Iteration 7 — Configuration Menu, First-Run Workspace Setup, Editable OS Catalog & Create Options
* **First-Run Workspace Dialog**: On the first launch (no configuration file yet), AIBox shows a focused welcome modal to choose the shared workspace location. Leaving the field empty or pressing **Use Default & Continue** keeps the current default (`~/AIBox/workspaces`). Completion is persisted in `<project>/.aibox/config.json` by default.
* **Modern Configuration Menu (`3` / monochrome sliders button)**:
  * **Storage & Locations**: change the workspace folder and the OS image storage folder, with native folder pickers, default/reset actions, and validation. Existing environments keep their original workspace path because the actual path is read back from each Incus workspace device.
  * **OS Catalog**: add, edit, and remove distributions (ID, image alias, display name, badge, description) with duplicate/format validation and a restore-defaults action. Cached image files are preserved when an entry is removed.
  * **Create Options**: edit capability preset titles/subtitles and the comma-separated disk quota, RAM limit, and CPU core choices used by the creation dialog. Defaults automatically fall back when an edited option disappears.
* **Editable OS Image Storage**: Downloads were exported to `~/ai-os-images` (configurable) as Incus metadata/rootfs files. If the local Incus image is missing, both downloads and new environments import the saved copy instead of re-downloading.
* **Monochrome Omarchy-Style UI**: Replaced emoji iconography with a one-color QML `SlidersIcon`; the Configuration menu uses a clean sidebar, flat cards, and the active Omarchy accent.
* **Backend & Tests**: Added JSON configuration load/save with `QSaveFile`, dynamic catalog builders, 7 new offscreen tests (20 total), and new keyboard shortcut help entry.

### Iteration 6 — Mandatory System Update Rule for AI Agents & Clean Package Rebuild
* **Agent Guidelines & Permanent Rules ([AGENTS.md](file:///home/saep/Work/aibox/AGENTS.md))**: Added workspace rules and global rules enforcing that any AI agent adding a feature, modifying the UI, or fixing bugs must build (`./bin/build`), test (`./bin/test`), build the Arch package (`makepkg -f`), and update the system (`./bin/install`) so the device is always up to date.
* **Symlink Interoperability**: Created `GEMINI.md` and `CLAUDE.md` symlinks pointing to `AGENTS.md` for cross-tool compatibility.
* **Full Rebuild & Test Verification**: Clean recompiled `build/aibox`, verified all 13 offscreen Qt tests passed, and prepared the Arch Linux package in `pkgbuild/`.

### Iteration 5 — Visual Polish, Status Alignment, Dynamic OS Theme Accent for Start OS, & Uniform Button Metrics
* **Inspector Title & Status Alignment**: Fixed layout calculation issue where the status badge squished and overlapped long environment titles. Given explicit preferred dimensions (`Layout.preferredWidth`, `Layout.preferredHeight: 22`), 12px horizontal separation, and an active status dot indicator.
* **Start OS & Stop OS Lifecycle Color System**:
  * **Start OS** now dynamically renders using the current active Omarchy theme accent (`theme.accent`), matching the exact visual style of the **Wipe OS** button (subtle tinted background using `Qt.rgba()`, 1px theme accent border, theme accent text, and tinted shortcut badge) instead of a hardcoded static green. When the desktop theme changes via `omarchy theme`, the button color adapts live.
  * **Stop OS** renders in prominent dark-red (`theme.red` tinted background and border, red text and shortcut badge) matching the **Wipe OS** button when the environment is running.
* **Top-Right Primary Action**: Positioned the **Enter OS Terminal** button in the top-right corner of the inspector pane with a flex spacer (`Item { Layout.fillWidth: true }`).
* **Strict Button Metrics & Spacing**:
  * Enforced a uniform height of `34px` across all action buttons, search/command inputs, and the segmented tab switcher.
  * Enforced uniform `10px` spacing across button rows and toolbars.
  * Added `minWidth` constraints across action button categories to eliminate visual jumping during state toggles.

### Iteration 4 — Full English Standardization
* Standardized all project documentation ([README.md](file:///home/saep/Work/aibox/README.md), [CATALOG.md](file:///home/saep/Work/aibox/CATALOG.md)), helper scripts ([bin/setup-system](file:///home/saep/Work/aibox/bin/setup-system), [bin/uninstall](file:///home/saep/Work/aibox/bin/uninstall), [~/.local/bin/ai-box](file:///home/saep/.local/bin/ai-box)), desktop metadata, and UI catalogs to English.

### Iteration 3 — Professional UI Refinement, Live Progress, Cancellation, & Background Persistence (Commit `4500970`)
* **Custom Flat Inputs (`FlatInput`)**: Replaced Qt Material's notched floating-label `TextField` with a sleek 1px-bordered dark input component and static labels.
* **Typography Overhaul**: Removed heavy bold text across buttons and cards; paired **`Adwaita Sans`** (`Normal` / `Medium`) for UI labels with **`JetBrainsMono Nerd Font`** for technical specs (disk/RAM quotas, IPv4 addresses, container names, and keybinding hints).
* **Refined Selector Cards**: Replaced bright solid-fill selection blocks with subtle accent-tinted dark cards (`#192136`), `1px` accent borders, and indicator dots.
* **Real-Time Progress & Log Stream**: Wrapped `incus image copy` and `incus launch` in a pseudo-terminal (`script -q -e -c`) to stream live percentage progress (`0%–100%`), stage indicators (*Step 1/4* to *Step 4/4*), and terminal output lines.
* **Task Cancellation & Concurrent Creation Lock**:
  * Added a **Cancel** button that terminates the active process group (`SIGTERM`/`SIGKILL`) and cleans up partially created containers/folders.
  * Locked all creation buttons and shortcuts while an operation is running (`backend.busy`).
* **Detached Background Execution & Auto Re-attachment**:
  * Tasks ran in an independent session (`setsid`) with state persisted in `~/.cache/aibox/`.
  * Closing the GUI window during environment creation does not interrupt the task; it sends a desktop notification (`notify-send`) when done and automatically re-attaches to the live progress bar if AIBox is reopened while still running.

### Iteration 2 — Native Omarchy GUI Application `AIBox` (Commit `80e4505`)
* Built the native C++17 & Qt Quick 6 desktop app in `~/Work/aibox`.
* Implemented master-detail environment inspector, 1-click terminal launch (`Space`), shared folder opener (`F`), 1-second Btrfs snapshots (`P`), headless Xvfb test runner, and 1-second OS wipe (`Delete`).
* Added an 8-distro OS catalog with one-tap local image caching.
* Packaged with `PKGBUILD` and installed into the Omarchy App Launcher (`Super + Space`).

### Iteration 1 — Base Incus Infrastructure & `ai-box` CLI
* Installed and configured Incus with a Btrfs storage pool and disk quotas enabled.
* Configured `incusbr0` bridge networking and UFW firewall rules.
* Created the `ai-gui` profile for Intel Iris Xe GPU (`/dev/dri`) and KVM (`/dev/kvm`) passthrough.
* Created the interactive CLI wrapper at `~/.local/bin/ai-box`.
