# CATALOG.md — System Architecture, File Inventory, & Changelog

This document tracks all host system configurations, project files, architecture decisions, and the changelog of **AIBox** for easy maintenance, auditing, and future re-installation.

---

## 1. Host System Modifications (Omarchy / Arch Linux)

All host-level configurations are automated in [bin/setup-system](file:///home/user/Work/aibox/bin/setup-system) and can be reverted cleanly using [bin/uninstall](file:///home/user/Work/aibox/bin/uninstall).

| System Component | Path / Name | Purpose |
| :--- | :--- | :--- |
| **System Packages** | `incus`, `dnsmasq`, `xorg-xhost`, `acl`, `qemu-system-x86`, `edk2-ovmf` | System container & VM manager, local DHCP/DNS server, X11/Wayland display bridge, and POSIX ACL utilities. |
| **Subordinate UID/GID** | `/etc/subuid` & `/etc/subgid` (`root:1000000:1000000000`) | Maps container `root` to an unprivileged UID range on the Host so autonomous AI agents cannot damage the host OS. |
| **User Groups** | `incus-admin`, `kvm`, `render`, `video` (for user `user`) | Grants passwordless access to Incus, hardware virtualization (`/dev/kvm`), and Intel Iris Xe GPU acceleration (`/dev/dri`). |
| **Systemd Drop-in ACL** | `/etc/systemd/system/incus.service.d/acl.conf` | Automatically applies read/write ACL permissions on `/var/lib/incus/unix.socket` when `incus.service` starts. |
| **Btrfs Storage Pool** | `default` (`/var/lib/incus/storage-pools/default`) | Native Btrfs subvolume storage pool with **Btrfs Quotas** enabled for strict per-environment disk size limits and <1s copy-on-write snapshots. |
| **Network Bridge** | `incusbr0` (`10.143.44.1/24`, IPv4 NAT enabled, IPv6 disabled) | Managed private bridge providing automatic DHCP IPv4 addresses and internet access to all environments. |
| **UFW Firewall Rules** | `ufw allow in on incusbr0` & `ufw route allow in/out on incusbr0` | Permits DHCP, DNS, and routed internet traffic from `incusbr0` through the active host UFW firewall. |
| **GPU & KVM Profile** | Incus profile `ai-gui` (`security.nesting=true`, `gpu` & `kvm` devices) | Enables nested Docker, hardware-accelerated Desktop GUI apps, and full-speed Android Emulators inside containers. |
| **Shared Workspaces** | `~/aibox/workspaces/<name>` (configurable) ⟷ `/workspace` | Two-way shared directory between Host and container using POSIX default ACLs (`setfacl`) and UID shifting (`shift=true`). Existing environments keep the workspace path they were created with. |
| **App Configuration** | `~/aibox/config.json` & `~/.config/aibox/location.json` | Persists the active system root location (`location.json`), workspace and OS image storage locations, the editable OS catalog, preset titles, disk/RAM/CPU choices, defaults, and first-run completion (`config.json`). |
| **OS Image Storage** | `~/aibox/os-images` (configurable) | Offline OS image exports (`<id>` metadata + `<id>.root`) used as a fallback import source when the local Incus image is missing. |
| **Background Task State** | `~/aibox/state/` (`task.pid`, `task.meta`, `task.log`, `task.sh`) | Tracks detached background operations (`setsid`) so tasks survive GUI window closure and automatically re-attach on startup. |

---

## 2. Project File Inventory (`~/Work/aibox`)

Built following the **Omarchy** native desktop application architecture (C++17 + Qt Quick 6 + `qmake6` + `makepkg`).

| File / Path | Category | Description |
| :--- | :--- | :--- |
| [aibox.pro](file:///home/user/Work/aibox/aibox.pro) | Build Config | `qmake6` project file defining Qt6 modules, C++ sources, and compiled resources. |
| [src/main.cpp](file:///home/user/Work/aibox/src/main.cpp) | Source (C++) | Application entry point dispatching headless CLI (`Cli::run` under `QCoreApplication`) and QML GUI (`QGuiApplication` + `QQmlApplicationEngine`). |
| [src/cli.h](file:///home/user/Work/aibox/src/cli.h) & [src/cli.cpp](file:///home/user/Work/aibox/src/cli.cpp) | Source (C++) | Headless CLI parser, help/usage formatter, and subcommand dispatcher (`list`, `check`, `enter`, `exec`, `start`, `stop`, `snapshot`, `snapshots`, `restore`, `diff`, `help`) sharing `Backend` script builders and task lock. |
| [src/backend.h](file:///home/user/Work/aibox/src/backend.h) | Source (C++) | `Backend` class declaration exposing environments, OS catalog, configuration state, first-run state, and all `Q_INVOKABLE` actions to QML. |
| [src/backend.cpp](file:///home/user/Work/aibox/src/backend.cpp) | Source (C++) | Incus JSON parser (including per-environment workspace device paths), dynamic OS catalog, JSON configuration persistence and validation, OS image export/import cache, PTY progress stream parser (`script -q -e -c`), process-group cancellation (`cancelTask`), and background task persistence. |
| [src/theme.h](file:///home/user/Work/aibox/src/theme.h) & [src/theme.cpp](file:///home/user/Work/aibox/src/theme.cpp) | Source (C++) | Live file watcher for Omarchy's active theme (`~/.local/state/omarchy/current/theme/colors.toml`) and Hyprland's window rounding. |
| [src/translator.h](file:///home/user/Work/aibox/src/translator.h) & [src/translator.cpp](file:///home/user/Work/aibox/src/translator.cpp) | Source (C++) | Live translation loader following `~/.config/omarchy/language`. |
| [src/Main.qml](file:///home/user/Work/aibox/src/Main.qml) | Source (QML) | Main application window, custom `FlatInput`, `ChipInput` (Enter-to-add quota/core chips, max 4), `ActionButton`, and monochrome `SlidersIcon` components, first-run workspace dialog, modern Configuration menu (locations, storage pool, OS catalog CRUD, create options, safety), environment inspector, OS catalog, creation dialog, live progress bar, and keyboard shortcuts. |
| [src/resources.qrc](file:///home/user/Work/aibox/src/resources.qrc) | Resource | Bundles QML and translation catalogs directly into the `/usr/bin/aibox` binary. |
| [src/i18n/en.json](file:///home/user/Work/aibox/src/i18n/en.json) & [da.json](file:///home/user/Work/aibox/src/i18n/da.json) | Localization | English plural rules and localization catalogs. |
| [tests/aibox_tests.pro](file:///home/user/Work/aibox/tests/aibox_tests.pro) & [aibox_tests.cpp](file:///home/user/Work/aibox/tests/aibox_tests.cpp) | Unit Tests | 69 offscreen Qt Test cases covering Incus JSON parsing, workspace device paths, name sanitization, first-run/config persistence, storage settings, OS catalog CRUD, OS image cache detection, create-option validation, background task state, Phase 1 (auto-snapshots, templates, allowlists), Phase 2 (watchdog hysteresis, snapshot sizes, runaway limits, egress log parser, operation history), Phase 3 (port forwards, extra mounts, live resource editing, provisioning bundles), Phase 4 (backup archive scanning, disk free-space margin check, export/import round-trip scripts, `aibox check` & `--fix` health checks), network monitoring parsers (gateway/ping/loss and counter rates), `Theme`, and `Translator`. |
| [pkgbuild/PKGBUILD](file:///home/user/Work/aibox/pkgbuild/PKGBUILD) | Packaging | Arch Linux package definition installing `/usr/bin/aibox`, license, SVG icon, and `.desktop` entry. |
| [pkgbuild/aibox.desktop](file:///home/user/Work/aibox/pkgbuild/aibox.desktop) | Launcher | Desktop entry for the Omarchy App Launcher (`Super + Space`). |
| [pkgbuild/aibox.svg](file:///home/user/Work/aibox/pkgbuild/aibox.svg) | Asset | Scalable SVG application icon. |
| [bin/setup-system](file:///home/user/Work/aibox/bin/setup-system) | Script | Automated host setup script for Incus, Btrfs storage pool, UFW rules, and GPU/KVM profile. |
| [bin/build](file:///home/user/Work/aibox/bin/build) | Script | Compiles the application into `build/aibox`. |
| [bin/test](file:///home/user/Work/aibox/bin/test) | Script | Builds and runs the offscreen Qt Test suite. |
| [bin/install](file:///home/user/Work/aibox/bin/install) | Script | Standard installation entry point; builds the application and installs the Arch Linux package via `makepkg -fsic --noconfirm`. |
| [bin/uninstall](file:///home/user/Work/aibox/bin/uninstall) | Script | Cleanly removes the `aibox` package (with `--full` option to also remove Incus, UFW rules, and all containers). |
| [AGENTS.md](file:///home/user/Work/aibox/AGENTS.md) | Agent Policy | Mandatory instructions and workflow rules for AI agents (enforces automatic build, test, and system update). |

---

## 3. Changelog

### Iteration 33 — Aligned Network Monitor & Click-to-Copy IP
- **Resource Cards (`src/Main.qml`)**:
  - Removed the **IPv4 Address** card (with its `ONLINE`/`OFFLINE`/`ALLOWLIST` badge and connection bar) from the task-manager row; the row is now three equal cards (**Disk Usage**, **Memory Usage**, **CPU Usage**, `columns: 3`), so IP status lives only in the **Network Monitor**.
- **Network Monitor (`src/Main.qml`)**:
  - Replaced the `GridLayout` of eight alternating rows with two equal-width `ColumnLayout`s (`Layout.fillWidth` + `Layout.preferredWidth: 0`) so labels and values line up in true left/right pairs: *Ping / Packet Loss*, *Receiving / Sending*, *Downloaded / Uploaded*, *IP Address / Gateway*.
  - **IP Address** and **Gateway** values are now clickable: a `HoverHandler` shows a pointing cursor plus a monochrome copy glyph (two overlapping outlined rectangles, no emoji) and a `TapHandler` copies the value via the new `backend.copyText(text, label)`; the value momentarily renders `Copied` in the theme accent and the status bar reports the copy.
- **Backend (`src/backend.{h,cpp}`)**:
  - Added `Q_INVOKABLE bool copyText(const QString &text, const QString &label = QString())` — a generic clipboard helper that ignores empty/`—` values, sets `statusMessage` (auto-cleared after 3 s) and returns whether the copy happened.

### Iteration 32 — Chip-Based Create Option Editor
- **Create Options Editor (`src/Main.qml`)**:
  - Replaced the three comma-separated text fields (disk quota, RAM limit, CPU cores) with a new `ChipInput` component: type a number and press `Enter` to store it as a removable chip, capped at **4 choices** per list.
  - Units are appended automatically (`20` → `20GiB`); RAM and disk share the `GiB` suffix, while CPU cores stay plain numbers. Each chip renders the amount in mono text with a muted unit, and `×` removes it.
  - Edits persist immediately through `backend.saveCreateOptions()` (chips are sorted numerically and duplicates, non-numeric input, zero, and over-limit values are rejected with the existing config notice). At least one choice per list is kept, and `Backspace` on an empty input removes the last chip.
  - Added per-field placeholders (e.g. *Type a size, then Enter*) plus a `4 / 4` counter that turns into *Limit 4 — remove one first* on hover, and compacted the card so `Save Options` stays visible at the default window height.
  - Added `diskChoiceDraft`, `ramChoiceDraft`, `cpuChoiceDraft`, `syncCreateChoiceDrafts()`, `sortedChoices()`, and `updateCreateChoices()` helpers; `Restore Create Defaults` now re-syncs the chip drafts.
- **Localization (`src/i18n/da.json`)**:
  - Added Danish entries for the new chip placeholders, limit counter, and validation notices.

### Iteration 31 — Compact Environment List Card
- **Environment Sidebar (`src/Main.qml`)**:
  - Removed the secondary description line (`<osName> · <preset>`) under the environment name; everything else (status dot, name, `TEMPLATE`/`RUNNING` badges, disk/memory quotas, and network/IP) is unchanged.
  - Card height reduced from `78px` to `64px` and column spacing raised to `8px` so the title row and quota row stay balanced now that only two lines remain, showing more environments per screen.

### Iteration 30 — Split Locations & Storage Pool Settings
- **Configuration Sidebar (`src/Main.qml`)**:
  - Split the former combined **Storage & Locations** page into two responsibility-focused pages: **01 Locations** (system root, workspace folder, OS image storage + `Save Locations`) and **02 Storage Pool** (Incus pool usage, allocated quotas, and loop-file growth).
  - Shifted the remaining pages to **03 OS Catalog**, **04 Create Options**, and **05 Safety**; `settingsSection` indices were renumbered accordingly (`AIBOX_SETTINGS_SECTION` now maps `0` = Locations, `1` = Storage Pool, `2` = OS Catalog, `3` = Create Options, `4` = Safety).
  - Added a **Pool details unavailable** fallback card with a `Refresh` action when `poolInfo.valid` is false, so the new page never renders blank.
- **Localization (`src/i18n/da.json`)**:
  - Added Danish entries for `Locations`, `Storage Pool`, `Storage Locations`, `Incus Storage Pool`, `Incus capacity & growth`, and the new pool page subtitle/fallback strings.

### Iteration 29 — Viewed History Acknowledgement
- **Unread-Only Badge (`src/backend.{h,cpp}`)**:
  - Added `acknowledgeOperationHistory()` plus a persisted `history-seen` marker (`<stateDir>/history-seen`) that stores the newest viewed history id. `recentFailedHistoryCount` now counts only failed entries newer than that marker, so the red `History (N!)` badge clears once the viewer is opened instead of requiring `Clear History`.
  - The marker is loaded on startup and after a settings/location change, survives restarts, is reset by `clearOperationHistory`, and new failures recorded afterwards raise the badge again.
- **History Dialog (`src/Main.qml`)**:
  - `openHistoryDialog()` now calls `backend.acknowledgeOperationHistory()`, so opening History via `H`/the toolbar clears the notification while all entries and logs stay intact.
- **Tests**:
  - Extended `operationHistoryTruncationAndPersistence` to assert the counter drops to 0 after acknowledgement, history size stays intact across a restart, and a newer failed entry raises the count again (69 passed, 0 failed).

### Iteration 28 — Live Network Monitor Card
- **Network Metrics (`src/backend.{h,cpp}`)**:
  - `parseIncusListJson` now sums per-interface Incus `state.network.*.counters` (`bytes_received` / `bytes_sent`) and computes sampling-window rates from a `QHash<QString, QPair<qint64,qint64>>` snapshot, exposing `netRxBytes`, `netTxBytes`, `netRxRate`, `netTxRate`, `netRxRateText`, `netTxRateText`, `netRxTotalText`, and `netTxTotalText` per environment (zero extra processes).
  - Added `parseNetworkProbeOutput` (gateway from `ip route`, `/proc/net/route` little-endian hex fallback, `ping` packet-loss and `rtt` avg) and `networkStatsFromEnvironment`; a 10s `refreshNetworkStats` probe runs `incus exec <env> -- sh -c` for the selected running environment and exposes a `networkStats` property (ping, loss, rates, totals, IP, gateway, `active`).
  - Probes are cancelled when the environment stops, goes offline, or the selection changes; the process is guarded against overlap and killed after 9s, and the timer is skipped in demo mode.
- **Network Monitor Card (`src/Main.qml`)**:
  - Added a simple flat panel under the Network Control bar with a live/idle dot, a manual `Probe` button, and a two-column eight-metric grid (Ping, Packet Loss, Receiving, Sending, Downloaded, Uploaded, IP Address, Gateway) using mono values, muted labels, and green/accent traffic accents.
- **Demo & Tests**:
  - Demo environments now carry sample network counters/probe values so the card renders in `AIBOX_DEMO_MODE`.
  - Added `parseNetworkProbeOutputReadsPingGatewayAndLoss`, `parseNetworkProbeOutputHandlesBlockedNetworkAndHexGateway`, `networkCountersProduceRatesAndTotals`, and `networkStatsFollowEnvironmentState` (69 passed, 0 failed).

### Iteration 27 — In-App Storage Pool Growth & Binary Size Labels
- **Grow Loop-Backed Pools (`src/backend.{h,cpp}`)**:
  - Extended `StoragePoolInfo` with `sizeText` / `resizable`; `refresh()` now also reads `incus storage get default size` (NUL-separated 4th part, failure-tolerant) so AIBox knows whether the pool is a loop file (`30GiB`) or a directory/subvolume pool that follows host free space.
  - Added `parsePoolSizeSpec` (bare numbers mean GiB, accepts `GiB`/`TiB`/`MiB` with `,` or `.` decimals), `validateStoragePoolResize` (grow-only, rejects garbage/unchanged/shrinking sizes, verifies host free space via `availableDiskBytes("/var/lib/incus")`, blocks during running tasks) and `resizeStoragePool` → `resizeStoragePoolScript` running `incus storage set <pool> size=<new>` (Incus truncates the loop file, refreshes the loop device, and runs `btrfs filesystem resize max` itself).
  - Exposed `resizable` / `sizeText` in `poolInfo` so QML can hide growth controls for non-resizable pools.
- **Settings → Storage Pool Card (`src/Main.qml`)**:
  - Added an amber/red-aware **Incus storage pool** card under Settings → **Storage** with usage bar, `used / total`, driver + loop-file size (or directory-backed note), allocated quota sum with unbounded count, and a `Grow pool` row (`FlatInput` + button) that validates first and requires an inline confirmation noting running environments and that pools cannot shrink.
  - Made the sidebar pool footer clickable (`Manage`) to jump straight to the pool card, and reset the resize draft/confirmation whenever the settings modal opens.
- **Binary Size Units (`src/backend.cpp`, `src/Main.qml`)**:
  - `formatBytes()` now reports `KiB` / `MiB` / `GiB` to match its 1024-based math and the `GiB` quotas shown by Incus, so the Disk Usage card no longer mixes `25.2 GB` with `100GiB` next to a `30.0 GB` pool.
- **Tests (`tests/aibox_tests.cpp`)**:
  - Added `poolResizeSpecValidationAndScript` covering spec parsing (bare GiB, `TiB`/`MiB`, invalid/negative/zero), grow-only validation, non-resizable pools, quoted script generation, and refusal without starting a task; updated size assertions to binary units (65 passed, 0 failed).

### Iteration 26 — Minimal Dark Application Icon Refresh
- **Application Icon (`pkgbuild/aibox.svg`)**:
  - Replaced the bright yellow icon with a dark slate squircle (`512x512` viewBox), subtle rim highlight, and a minimalist blue-accented container frame holding the `>_` prompt to match AIBox and Omarchy's visual style.

### Iteration 25 — Default `~/aibox` System Storage, First-Run Location Setup & Modal UI Consistency
- **System Storage Location (`src/backend.{h,cpp}`, `src/cli.cpp`)**:
  - Changed the default system storage root from `.aibox/` to `~/aibox/` (`workspaces/`, `os-images/`, `state/`, `config.json`) without creating `~/aibox` prematurely before first-run confirmation.
  - Added `~/.config/aibox/location.json` (overridable via `AIBOX_LOCATION_FILE`) to persist the user's chosen system storage root across launches, while keeping `--project <dir>` resolution (`<dir>/aibox/`, with legacy fallback to `<dir>/.aibox/` if already present).
- **First-Run Custom Storage Dialog (`src/Main.qml`)**:
  - Updated the initial setup dialog (`firstRunOverlay`) to let users enter or browse (`Browse...` / `Default`) a custom system storage directory before clicking **Continue**, showing a live preview of `Workspaces`, `OS Images`, and `State & Cfg` paths.
- **UI Spacing & Modal Header Unification (`src/Main.qml`)**:
  - Added a reusable `ModalHeader` component with uniform top-right `Close [Esc]` buttons, standardized `FlatInput` height (`34px`) and `10px` action row gaps, and unified margins (`24px`, `22px` in `createOverlay`) and corner radii across modals and inspector bars.
  - Fixed horizontal overflow and alignment issues in Clone Environment (`Shared at:` path elision), Environment Backups (`Browse...` / `Import Backup` alignment and archive name elision), Operation History (`Close [Esc]` button), Settings (removed duplicate close buttons and added System Root card), and the `Shared Workspace` bar.

### Iteration 24 — Roadmap Phase 6: H3 Session Recording (Opt-in)
- **Session Recording Config & Per-Environment Override (`src/backend.{h,cpp}`)**:
  - Added global `recordSessions` (`false` default) in `config.json` and `Backend::saveSessionRecordingSettings`.
  - Added per-environment Incus config key `user.aibox.record` (`inherit`/`1`/`0`), parsed into `recordOverride` / `recordEnabled` in `parseIncusListJson` and updated via `Backend::setEnvironmentRecordSessions`.
  - Refactored terminal launch via `launchCommandInTerminal` and added `Backend::buildSessionWrapCommand` wrapping interactive Enter shells with `script -q -f --log-out <log> --log-timing <timing> -c <cmd>` (util-linux) with automatic unrecorded fallback when `script` is unavailable.
- **Post-Session Secret Redaction, Retention Pruning & Wipe Cleanup (`src/main.cpp`, `src/cli.cpp`, `src/backend.{h,cpp}`)**:
  - Added fast internal CLI path `aibox --redact-session <logPath> [sessionsDir]` in `src/main.cpp` and `Backend::redactSessionLogFile` so exited sessions scrub S3 secrets from `secrets.json` without placing secret values on shell scripts or argv (`0600` permissions preserved).
  - Updated CLI `runEnter` (`src/cli.cpp`) to also wrap interactive shells via `fork()` + `execvp("script", ...)` when enabled and redact/prune immediately on exit.
  - Added `Backend::pruneRecordedSessions` (retaining last 20 sessions per environment and 200 MB total under `<stateDir>/sessions/<env>/<epoch>.{log,timing}`), invoked on GUI startup and after session completion.
  - Updated `wipeEnvironmentScript` and `exportAndWipeEnvironmentScript` (plus sync calls) to remove `<stateDir>/sessions/<env>` on wipe.
- **Sessions Viewer, Terminal Replay & i18n (`src/Main.qml`, `src/i18n/da.json`)**:
  - Added **Session Recording (Opt-in)** toggle in Settings → **Safety** and per-environment `Inherit` / `On` / `Off` pills in the Guardrails card.
  - Added **Tasks** / **Sessions** segmented tabs inside **Operation History (`H`)** (`MODAL 6`) showing ANSI-stripped logs (`Backend::stripAnsiSequences`, `Backend::formatRecordedSessionText` capped at 200 KB), per-session delete, and **Replay in terminal** (`scriptreplay -t <timing> -O <log>`).
  - Added Danish translations in `src/i18n/da.json`.
- **Tests (`tests/aibox_tests.cpp`)**:
  - Added `sessionRecordingConfigOverridesAndWrapBuilder` and `sessionRecordingRetentionAnsiAndRedaction` covering `recordSessions` persistence, `user.aibox.record` overrides, `buildSessionWrapCommand` quoting and missing-`script` fallback, ANSI CSI/OSC/cr/backspace stripping, 200 KB truncation, `redactSessionLogFile`, and count/byte retention pruning (64 passed, 0 failed).

### Iteration 23 — Roadmap Phase 6: H4 Stale Environment Report (`U`)
- **Last-Used Tracking & Staleness Classifier (`src/backend.{h,cpp}`)**:
  - Recorded Unix epoch timestamps into `user.aibox.lastused` across GUI Enter (`prepareEnvironmentScript`), GUI headless execution (`runHeadlessCommand`), CLI `enter` / `exec` (`prepareEnvironmentScript`), and environment Start (`startEnvironmentScript`).
  - Added global `staleDays` in `config.json` (`30` default, valid `1..3650`, presets `7/14/30/60/90/180`) and `Backend::saveStaleSettings`.
  - Parsed `lastUsedEpoch` and `snapshotTotalBytes` in `Backend::parseIncusListJson` and implemented `Backend::classifyEnvironmentStaleness` (`Fresh`, `Stale`, `Unknown`, `Template`).
- **Orphan Scanner, Verified Export & Wipe, & Safe Cleanup (`src/backend.{h,cpp}`)**:
  - Added `Backend::detectOrphans` and `Backend::buildStaleReport` listing unused (`>= staleDays`) and unknown-age non-template environments (with disk usage, snapshot count/size, and workspace size) alongside orphan workspace folders, orphan `.snapshots/<env>` directories, and unpaired `-workspace.tar.gz` backup archives.
  - Added `Backend::exportAndWipeEnvironment` / `exportAndWipeEnvironmentScript` chaining `export` with an explicit `[ -s <archive> ]` verification gate so `wipe` aborts automatically if export fails or produces a 0-byte archive.
  - Added `Backend::openOrphanFolder` and path-validated `Backend::deleteOrphan` / `deleteOrphanScript` supporting Btrfs subvolume cleanup for orphaned snapshot dirs.
- **UI & i18n (`src/Main.qml`, `src/i18n/da.json`)**:
  - Added `U` shortcut, `?` Help overlay entry, Sidebar/Empty-state **Stale & Orphans Report (`U`)** buttons, Settings → **Safety** `staleDays` selector, and **MODAL 11** listing stale/unknown environments (**Export & Wipe**, **Wipe**) and orphans (**Open folder**, **Delete** with size confirmation).
  - Added Danish translations in `src/i18n/da.json`.
- **Tests (`tests/aibox_tests.cpp`)**:
  - Added `staleClassificationAndConfigPersistence` and `staleOrphanDetectionAndExportWipeSafety` covering `staleDays` persistence, JSON parsing, `user.aibox.lastused` snippets, fixture tree orphan detection, path-safety guards, and Export & Wipe abort behavior on failed or 0-byte exports (62 passed, 0 failed).

### Iteration 22 — Roadmap Phase 6: H1 Idle Auto-Stop (Opt-in)
- **Global & Per-Environment Idle Policy (`src/backend.{h,cpp}`)**:
  - Added global `idleStopMinutes` (`0` = Off default, options `0/15/30/60/120`, clamped `0..1440`) in `config.json` and `Backend::saveIdleStopSettings`.
  - Added per-environment Incus keys `user.aibox.idlestop` (`inherit`, `off`, or minutes) and `user.aibox.idlestop_ports` (`1`/`0`), parsed in `parseIncusListJson` and updated via `Backend::setEnvironmentIdleStop`.
- **Idle Evaluator, Blockers & Auto-Snapshot Stop (`src/backend.{h,cpp}`)**:
  - Added static pure evaluator `Backend::evaluateIdleDecision` plus `parseIdleStopOverride`, `effectiveIdleStopMinutes`, and `parseActiveIncusExecEnvs` (parses `pgrep -u $USER -af "incus exec"` while skipping value-taking Incus flags such as `--cwd`, `--env`, `--user`, `--group`, `--mode`, `--project`, `--target`).
  - Integrated `evaluateIdleAutoStops()` inside `Backend::refresh()` so running non-template, non-busy environments require average CPU `< 3%` over the full window, no active user `incus exec` session, and no port forwards (unless `user.aibox.idlestop_ports` permits it).
  - Sends a 1-minute advance `notify-send` warning before executing a normal stop (`stopEnvironmentScript(name, true)` prepending `buildAutoSnapshotIfNeeded` when enabled, without `--force`), and records the reason `"idle"` in `operation_history.json` (`reason` field propagated through `runShellTask`, detached `task.meta`, and `recordOperationHistory`).
- **UI & i18n (`src/Main.qml`, `src/i18n/da.json`)**:
  - Added **Idle Auto-Stop (Opt-in)** selector card (`Off` / `15m` / `30m` / `60m` / `120m`) in Settings → **Safety** with explicit notice that it works only while the AIBox GUI is running (no background daemon).
  - Added per-environment `Idle Auto-Stop:` override pills (`Inherit`, `Off`, `15m`, `30m`, `60m`) and `Ignore ports: ON/OFF` toggle in the Snapshots & Guardrails card, plus amber `IDLE` pill in Operation History (`H`).
  - Added Danish translations in `src/i18n/da.json`.
- **Tests (`tests/aibox_tests.cpp`)**:
  - Added `idleStopOverridesAndConfigPersistence` and `idleDecisionSyntheticSamplesAndBlockers` covering config defaults/persistence, override precedence, CPU spike reset, 1-minute advance warning, active `incus exec` flag/session parsing, port forward blocking/override, template skipping, and normal auto-snapshot stop script generation (60 passed, 0 failed).

### Iteration 21 — Roadmap Phase 6: H2 Storage Pool Overcommit Guard
- **Pool Capacity & Quota Sum Diagnostics (`src/backend.{h,cpp}`, `src/cli.cpp`)**:
  - Added `Backend::parseIncusStorageInfo` supporting `incus storage info default --bytes` (quoted byte counts), human-readable YAML (`1.24GiB` / `30.00GiB`), and JSON fixtures while ignoring `used by:` entries.
  - Added `Backend::sumRootDiskQuotasBytes` reading `expanded_devices.root` (falling back to direct `devices.root`) across instances on pool `default`, excluding a target environment during quota edits and counting unquoted/`Shared` environments; saved `diskQuotaBytes` in `parseIncusListJson`.
  - Refreshed `poolInfo` automatically inside `Backend::refresh()` and added `poolWarningForCreate` / `poolWarningForQuotaEdit` via `evaluatePoolOvercommitWarning` (warns when projected quotas exceed pool capacity or pool usage exceeds 85%).
  - Added `pool_capacity` (`evaluatePoolCapacityHealth`) as check #8 in `Backend::runHealthChecks` and `aibox check [--json]` (`[WARN]` above 85% pool usage, `[FAIL]` above 95% with bytes, percentages, allocated quota sum, and `[OVERCOMMITTED]` indicator in `detail`).
- **UI & i18n (`src/Main.qml`, `src/i18n/da.json`)**:
  - Displayed live `Pool <name>: used / total (pct%)` and `Alloc` in the sidebar storage footer and inside the **Disk Usage** resource card, plus non-blocking amber warning banners in **Create Environment** and **Edit Quotas (`E`)** when overcommitted or >85% full; also fixed `win.borderHover` QML color warning in Secrets checkbox.
  - Added Danish translations in `src/i18n/da.json`.
- **Tests (`tests/aibox_tests.cpp`)**:
  - Added `poolStorageInfoParsingRealFixtures` (using real host `incus storage info default [--bytes]` outputs), `poolOvercommitMathAndUnboundedQuotas`, and `poolCapacityHealthCheckThresholds`, and updated health check count assertions to 8 (58 passed, 0 failed).

### Iteration 20 — Roadmap Phase 5: S3 Secrets Injection (`A`)
- **Secrets Store & Runtime-Only Injection (`src/backend.{h,cpp}`, `src/cli.cpp`)**:
  - Added global secret storage at `~/.config/aibox/secrets.json` (overridable via `AIBOX_SECRETS_PATH`), enforcing `0700` directory and `0600` file permissions (refusing group/world-readable files on load and fixing permissions to `0600` on save).
  - Enforced environment variable name validation (`^[A-Z_][A-Z0-9_]*$`) and stored only comma-separated key **names** per environment in `user.aibox.secrets` (never secret values).
  - Documented injection variant: `incus exec --help` selects interactive TTY mode only when both stdin and stdout are terminals, so stdin redirection breaks interactive shells; implemented the `/dev/shm` `tmpfs` fallback across GUI Enter (`Space`), `runHeadlessCommand`, and CLI `enter` / `exec` (`0600` host file in `$XDG_RUNTIME_DIR` -> `incus file push --mode 0400` to `/dev/shm/aibox-<uuid>.env` -> immediate host unlink -> `set -a; . "$AIBOX_ENV"; set +a; rm -f "$AIBOX_ENV"` via positional parameter `$1` inside container).
  - Added `Backend::redactSecrets` / `redactSecretsBytes` (longest-first replacement with `[REDACTED]`) across live task output streaming, detached task logs (`task.log`), and Operation History persistence; updated `aiExecCommandText` (`C`) to refer to `aibox exec <name> -- bash` instead of embedding values.
- **UI & i18n (`src/Main.qml`, `src/i18n/da.json`)**:
  - Added `secretsVisible` modal (`A` shortcut, header **Secrets (A)** button, clickable **SECRETS: N** card badge, `?` overlay entry) with add/edit/delete for global secrets, masked values with per-row reveal toggle, permission error banner, and per-environment secret checkboxes.
  - Added Danish translations in `src/i18n/da.json`.
- **Tests (`tests/aibox_tests.cpp`)**:
  - Added `secretsValidationQuotingAndRedaction`, `secretsFilePermissionsEnforcedAndFixed`, and `secretsExecInjectionRedactsHistoryAndNeverTouchesBackups` (verifying quoting with `/bin/sh`, longest-first redaction, `0600`/`0700` enforcement and repair, CLI `exec` injection + host cleanup, and non-exposure in export/import/clone scripts; 55 passed, 0 failed).

### Iteration 19 — Roadmap Phase 5: S4 Headless CLI
- **Headless CLI (`src/cli.{h,cpp}`, `src/main.cpp`)**:
  - Moved CLI parsing and dispatch out of `src/main.cpp` into `Cli::parseArgs`, `Cli::helpText`, `Cli::execute`, and `Cli::run`, and added `src/cli.{h,cpp}` to `aibox.pro` and `tests/aibox_tests.pro`.
  - Runs headless commands under `QCoreApplication` (with `QT_QPA_PLATFORMTHEME` unset) so CLI works in plain/SSH terminals without X11, Wayland, or GTK initialization errors.
  - Added subcommands: `enter <env>` (runs GUI pre-steps then replaces process via `execvp("incus", ...)` for clean TTY pass-through), `exec <env> -- <cmd...>` (forwards command exit code), `start <env>`, `stop <env>`, `snapshot <env> [name]`, `snapshots <env> [--json]`, `restore <env> <snap> [--system-only|--workspace-only] [--yes]`, `diff <env> [snap] [--json]`, and `help [cmd]`, alongside existing `list [--json]` and `check [--fix] [--json]`.
  - Added global `--project <dir>` option (selecting the `<dir>/.aibox` directory, matching GUI defaults) and exit code mapping (`0` ok / command exit code, `1` failure, `2` usage error, `3` `task.pid` lock active, `4` env not found or template).
- **Shared Backend Refactoring (`src/backend.{h,cpp}`)**:
  - Added `Backend::PathMode::ProjectDir` constructor and `Backend::resolveProjectAiboxDir`, `refreshSync`, `isTaskLockActive`, `acquireCliTaskLock` / `releaseCliTaskLock` (enforced in `runShellTask` too), and shared script/diff helpers (`prepareEnvironmentScript`, `startEnvironmentScript`, `stopEnvironmentScript`, `enterIncusExecArgs`, `buildWorkspaceDiffArgs`, `processWorkspaceDiffOutput`) so GUI and CLI never duplicate shell scripts.
- **Tests (`tests/aibox_tests.cpp`)**:
  - Added `cliParsesAllSubcommandsAndFlags`, `cliProjectResolutionAndLockCheck`, and `cliExecAndExitCodesWithStubIncus` (52 passed, 0 failed).

### Iteration 18 — Roadmap Phase 5: S2 Change Review (`V`)
- **Workspace Change Review (`V`)** (`src/backend.{h,cpp}`):
  - Added `diffExcludes` (persisted in `config.json`, defaulting to `.git/`, `node_modules/`, `target/`, `.venv/`, `__pycache__/`, `build/`), `diffLoading`, `diffEntries`, `diffSummary`, `selectedDiffText`, and `selectedDiffInfo`.
  - Added `loadWorkspaceDiff(envName, snapName)` running `rsync -a -n --delete --itemize-changes --out-format='%i %l %n'` asynchronously with a 30s timeout and 5000-entry cap (`truncated` flag).
  - Added `loadFileDiff(envName, snapName, relPath, status)` validating safe relative paths, skipping files over 512 KB and binary files (NUL byte in first 8192 bytes), handling symlinks, running `diff -u --label snapshot --label current`, and capping output at 2000 lines.
  - Added `restoreSingleFile(envName, snapName, relPath)` copying the selected file or symlink back from `<workspaceBase>/.snapshots/<env>/<snapName>/<relPath>`, logging `"success"` in Operation History, and refreshing the diff list.
- **UI & i18n** (`src/Main.qml`, `src/i18n/da.json`):
  - Added `changeReviewVisible` modal (`V` shortcut, `?` help overlay entry, **Changes** button on workspace/paired snapshot rows, snapshot selector strip, editable exclude rules, split-pane file list + monospace unified diff viewer + **Restore this file** action).
  - Added Danish translations in `src/i18n/da.json`.
- **Tests** (`tests/aibox_tests.cpp`):
  - Added `parseRsyncItemizeHandlesFixturesAndCap`, `diffExcludesBinaryCheckAndLineCapping`, and `realRsyncDetectsAddedModifiedDeletedAndHonorsExcludes` (49 passed, 0 failed).

### Iteration 17 — Roadmap Phase 5: S0 Btrfs Spike & S1 Workspace Snapshots
* **S0 — Host Btrfs Permission Spike**:
  * Verified on `7.2.15-1-omarchy` (`/home` Btrfs without `user_subvol_rm_allowed`): `btrfs subvolume create` and `btrfs subvolume snapshot -r` succeed as unprivileged user; `btrfs subvolume delete` fails with `EPERM`, while `btrfs property set -ts <snap> ro false && rm -rf <snap>` removes subvolumes cleanly when ACLs permit (falling back to printing `sudo btrfs subvolume delete` otherwise).
  * Verified container writes under `shift=true` inherit `user:<host>:rwx` default ACLs and stopped-container workspace swaps work cleanly.
  * Added check #7 (`workspace_subvol_rm`) to `aibox check` suggesting `user_subvol_rm_allowed` when the workspace Btrfs mount lacks it.
* **S1 — Paired `/workspace` Btrfs Subvolume Snapshots (`src/backend.{h,cpp}`, `src/Main.qml`)**:
  * New, cloned, and imported environments automatically create `/workspace` as a Btrfs subvolume when hosted on Btrfs (`stat -f -c %T` == `btrfs`), falling back to plain directories on non-Btrfs filesystems.
  * Added **Convert to Subvolume** and **Delete `.pre-subvol`** actions for existing plain-directory workspaces on Btrfs.
  * Manual snapshots (`P`) and auto-snapshots before Enter/Start now take paired read-only workspace snapshots under `<workspaceBase>/.snapshots/<env>/<snapName>` (non-fatal warning logged to Operation History on failure), enforce `user.aibox.wssnap` (`WS: ON/OFF`), and prune/delete both halves together.
  * Added the **Restore Snapshot** confirmation dialog offering **Both (System + Workspace)**, **System only**, and **Workspace only**, taking an automatic `pre-restore-<epoch>` safety snapshot and stopping running environments before swapping `/workspace`.
  * Snapshot list rows show a monochrome `W` badge, combined system + workspace sizes, and a **Workspace-only snapshots** (`WS ONLY`) section for safety/orphan snapshots; wiping with workspace deletion removes `.snapshots/<env>/`.
* **Tests & Localization (`tests/aibox_tests.cpp`, `src/i18n/da.json`)**: Added 3 S1 unit test suites and updated health check tests for `workspace_subvol_rm` (46 total tests passing); added Danish translations for all new UI strings.

### Iteration 16 — Roadmap Phase 4: Resilience (Environment Export/Import & `aibox check --fix`)
* **R1 — Environment Export & Import (`src/backend.{h,cpp}`, `src/Main.qml`)**:
  * Added full instance archive export (`incus export <name> <file>.tar.gz --compression gzip`) and optional separate host `/workspace` archive (`tar -czf ...-workspace.tar.gz`) stored under `<osStoragePath>/backups/`.
  * Added pre-flight disk-space estimation and verification (`checkExportSpace`, `hasSufficientDiskSpace`, `QStorageInfo` with 100 MiB safety margin) before starting export tasks.
  * Added `importEnvironment` (`incus import` to a new name, automatic `user.aibox.template` unset, target workspace binding, and optional workspace tarball extraction) and `deleteBackup`.
  * Added the **Backups (`B`)** header action and modal with archive scanner, workspace inclusion toggle, size/free-space preview, external archive `FileDialog`, and progress/cancel background task execution.
* **R2 — Host Health Diagnostics & Safe Auto-Repair (`src/backend.{h,cpp}`, `src/main.cpp`)**:
  * Refactored `aibox check` into 6 modular checks (`groups`, `socket_acl`, `incus_daemon`, `bridge_incusbr0`, `ai_gui_profile`, `ufw_rules` reading `/etc/ufw/user.rules`), supporting both human-readable output and `--json` while preserving `aibox list` unchanged.
  * Added `aibox check --fix` to automatically repair non-root issues (creating `incusbr0`, creating/repairing `ai-gui` nesting/`gpu`/`kvm` devices when the Incus daemon is reachable) and print exact `sudo` remediation commands for privileged fixes.
* **Tests & Localization (`tests/aibox_tests.cpp`, `src/i18n/da.json`)**: Added 6 unit tests covering disk-space safety margins, backup archive pairing, stubbed `incus` export/import round-trip scripts with workspace archives, healthy host stub checks, non-root `--fix` repairs, and root command hints; updated Danish translations and `?` shortcuts overlay.

### Iteration 15 — Roadmap Phase 3: Daily Workflow (Ports, Extra Mounts, Live Quotas, & Provisioning)
* **W1 — Port Forwarding (`src/backend.{h,cpp}`, `src/Main.qml`)**:
  * Added Incus `proxy` device management bound strictly to `127.0.0.1` (`fwd-<hostPort>`), host port conflict checking via `QTcpServer` (`isHostPortAvailable`), parsed `portForwards` from `incus list --format=json`, and added the **Ports** modal with **Open** (`xdg-open http://127.0.0.1:<hostPort>`) and **Remove**.
* **W2 — Read-Only Extra Mounts (`src/backend.{h,cpp}`, `src/Main.qml`)**:
  * Added extra host folder mounts under `/mnt/<name>` (`mnt-<name>`, `readonly=true` by default, `shift=true` with fallback) parsed as `extraMounts` in `parseIncusListJson`.
  * Enforced `validateExtraMountPath` blocking `/`, `/etc`, `/home`, `$HOME`, system prefixes, `.ssh`, `.gnupg`, `.aws`, `.kube`, `.docker`, `.aibox`, and AIBox's config/state directories; read-write mounts require an explicit second confirmation toggle (`confirmReadWrite`).
* **W3 — Live Resource Editing (`src/backend.{h,cpp}`, `src/Main.qml`)**:
  * Added `updateEnvironmentResources` (`E` shortcut & **Quotas** button) to edit CPU cores, RAM limit, disk quota, container `limits.processes`, and `root` I/O bandwidth (`limits.read`/`write`) on existing environments, with a disk-shrink guard preventing resizing below current `diskUsageBytes` and a VM restart note.
* **W4 — Provisioning Presets (`src/backend.{h,cpp}`, `src/Main.qml`)**:
  * Added selectable **Provisioning Presets** (`Dev Essentials`, `Node.js LTS`, `Python 3`, `Rust`) in **Create Environment** for Debian/Ubuntu (`apt`) and Arch (`pacman`), executed non-fatally via `incus exec` after boot and customizable under `<configDir>/provision/`.
* **Tests & Localization (`tests/aibox_tests.cpp`, `src/i18n/da.json`)**: Added 5 unit tests covering port validation & loopback socket conflict detection, extra mount path deny-list & RW confirmation, live resource script & byte-size parsing, non-fatal provisioning bundle execution, and `parseIncusListJson` extraction of `portForwards`/`extraMounts`; updated Danish translations and `?` shortcuts overlay.

### Iteration 14 — Roadmap Phase 2: Guardrails & Visibility
* **Disk & Resource Watchdog + Snapshot Sizes (`src/backend.{h,cpp}`, `src/Main.qml`)**:
  * Evaluated per-environment disk thresholds (`watchdog.diskWarn` default `90%`, `watchdog.diskCritical` default `97%`) and sustained CPU/RAM high streaks inside the 4-second `refresh()` loop with a 5-point hysteresis/debounce state machine (`evaluateWatchdog`) so `notify-send` alerts fire once per escalation.
  * Added opt-in critical disk auto-stop (`watchdog.autoStopOnCritical`, per-environment override via `user.aibox.autostop` and `Auto-stop: ON/OFF` toggle), highlighted Disk Usage cards in amber/red on warning/critical levels, and added Watchdog controls to **Configuration → Safety**.
  * Lazily fetches snapshot disk consumption via `/1.0/instances/<name>/snapshots?recursion=1` (`parseSnapshotSizesJson`) for the selected environment and displays each snapshot's size in the rollback list when available.
* **Runaway Limits (`src/backend.{h,cpp}`, `src/Main.qml`)**:
  * Added **Process Limit** (`limits.processes`, containers only, skipped for `--vm` presets) and **Disk I/O Limit** (`limits.read` / `limits.write` on `root`) selectors in the **Create Environment** dialog, validation helpers (`isValidProcessLimit`, `isValidIoLimit`), and Inspector limit badges (`Pids ... · I/O ...`).
* **Operation History (`src/backend.{h,cpp}`, `src/Main.qml`)**:
  * Recorded every detached background task (`checkBackgroundTask`, `attachExistingTask`, `cancelTask`) as individual `<id>.json` metadata + tail-capped `200 KB` `<id>.log` files under `<stateDir>/history/` (keeping the newest 50 operations).
  * Added the **History** header button (`H` shortcut, showing failed count badge) and the **Operation History** viewer modal with `OK` / `FAILED` / `CANCELLED` badges, full scrollable task logs, and **Clear History**.
* **Egress Log (`src/backend.{h,cpp}`, `src/Main.qml`)**:
  * Implemented per-environment kernel bridge ACL egress logging (`aibox-log-<name>` for online mode and `state=logged` rules for allowlist mode), **Record: ON/OFF** toggle (`user.aibox.egresslog`), and **Egress Log** viewer parsing `journalctl -k` (`parseKernelEgressLogs`) aggregated by `(dstIp, proto, port)` with hit counts, last-seen timestamps, and one-click **+ Allow**.
* **Tests & Localization (`tests/aibox_tests.cpp`, `src/i18n/da.json`)**: Added 5 unit tests covering watchdog hysteresis/auto-stop, snapshot sizes & watchdog config, container vs VM runaway limits, egress ACL scripts & kernel log parsing, and operation history truncation/persistence; updated Danish translations and shortcuts overlay (`?`).

### Iteration 13 — Roadmap Phase 1: Auto-Snapshots, Clone & Templates, & Network Control
* **Auto-Snapshots & Retention (`src/backend.{h,cpp}`, `src/Main.qml`)**:
  * Added automatic `auto-<timestamp>` Btrfs snapshots before **Enter OS Terminal** and **Start OS** with a 5-minute minimum interval (`shouldCreateAutoSnapshot`) and automatic pruning of older `auto-` snapshots while keeping manual snapshots intact (`autoSnapshotsToPrune`).
  * Enabled by default (keeping the last 5 auto-snapshots); configurable globally in **Configuration → Safety** (`autoSnapshotEnabled`, `autoSnapshotKeep` in `config.json`) and overridden per environment via `user.aibox.autosnap` (`Auto: ON` / `Auto: OFF` toggle).
  * Snapshot entries prefixed with `auto-` display a monochrome `AUTO` tag in the rollback list.
* **Clone & Reusable Templates (`src/backend.{h,cpp}`, `src/Main.qml`)**:
  * Added `cloneEnvironment` (`D` shortcut & **Clone** dialog) using `incus copy --stateless`, creating an isolated host workspace path, unsetting template flags, applying optional quotas, and isolating/copying network ACL configuration.
  * Added **Template** toggle (`T` shortcut & `user.aibox.template=true`) to mark stopped environments as reusable templates with a `TEMPLATE` badge, disabling direct start/enter actions and adding a **Fresh OS / From Template** mode selector in the **Create Environment** dialog.
* **Per-Environment Network Control (`src/backend.{h,cpp}`, `src/Main.qml`, `aibox.pro`)**:
  * Added `Online`, `Allowlist`, and `Offline` modes plus a one-click **Kill Network / Restore Online** action (`K` shortcut) persisted via `user.aibox.netmode` and `user.aibox.netallow`.
  * `Offline` detaches `eth0` (`type=none`) live while keeping `/workspace` functional; `Allowlist` resolves allowed domains to public IPv4 CIDRs (filtering private/reserved DNS responses against DNS rebinding) and applies a per-instance `aibox-<name>` Incus Network ACL (`security.acls.default.egress.action=drop`), refreshing IPs on start and via **Refresh IPs**, and cleaning up the ACL on wipe.
* **Tests & Localization (`tests/aibox_tests.cpp`, `src/i18n/da.json`)**: Added 5 new unit tests covering auto-snapshot persistence, pruning/interval rules, template/network JSON parsing, clone script workspace isolation, and allowlist DNS-rebinding protection; updated Danish translations and keyboard shortcuts overlay (`?`).

### Iteration 12 — Task Manager Resource Cards, Per-Core CPU View, & Right-Aligned Actions
* **Shared Workspace & Snapshot Right-Aligned Actions (`src/Main.qml`)**: Moved the **Open Shared Folder** (`F`) button into the right side of the **Shared Workspace** box beside the host ⟷ `/workspace` path, and anchored the **Snapshot label** input + **+ Save Snapshot** (`P`) button to the right edge of the Btrfs Snapshots header.
* **Task Manager Resource Monitor Cards (`src/backend.h`, `src/backend.cpp`, `src/Main.qml`)**:
  * **Disk Usage**: Extracts `state.disk.root.usage` and `state.disk.root.total` (alongside the configured quota) from `incus list --format=json` to display live disk consumption (`<used> / <quota>`), usage percentage badge, and progress bar.
  * **Memory Usage**: Calculates memory usage percentage against `limits.memory` / `state.memory.total` and displays a live utilization badge and progress bar alongside `<used> / <limit>`.
  * **Per-Core CPU Usage**: Samples container/VM `state.cpu.usage` nanosecond counters across refresh intervals and `/proc/stat` core weights to render individual per-core percentage labels (`C1`, `C2`, `C3`, ...) and mini utilization bars inside the **CPU Usage** card.
  * **Network Status**: Enhances the **IPv4 Address** card with an `ONLINE` / `OFFLINE` status indicator and connection bar.
* **Unit Tests & Localization (`tests/aibox_tests.cpp`, `src/i18n/da.json`)**: Extended `parseIncusJsonExtractsSpecsAndSnapshots` to verify disk usage, memory percentage, CPU delta percentage, and `cpuCorePercents` list generation, and added translation keys for the new resource monitor labels.

### Iteration 11 — Create Environment Script Quoting Fix
* **Preset & Quota Arguments (`src/backend.cpp`, `src/backend.h`)**: The generated `task.sh` used to assign the preset flags and quota arguments unquoted (`PROFILE_FLAGS=-p default -p ai-gui`), so bash tried to run `default` as a command and the creation aborted with `line 11: default: command not found`. The script now stores both as properly quoted bash arrays (`PROFILE_ARGS=(...)`, `QUOTA_ARGS=(...)`) that are expanded as `${PROFILE_ARGS[*]}` / `${QUOTA_ARGS[*]}` when calling `incus launch`, and the quota values are no longer double-quoted into the Incus argument. The script builder was extracted into the testable `Backend::createEnvironmentScript()` plus the `shellArrayWords()` helper.
* **ACL user fallback**: `setfacl` now uses `${SUDO_USER:-${USER:-$(id -un)}}` so the workspace ACL step cannot fail under `set -u` when `$USER` is unset.
* **Regression Test (`tests/aibox_tests.cpp`)**: New `createEnvironmentScriptPassesPresetFlagsAsArguments` test runs the generated script against stub `incus`/`script`/`sleep` binaries and asserts the launch arguments arrive as separate `-p default -p ai-gui -c limits.cpu=4 -c limits.memory=6GiB -d root,size=60GiB` tokens.

### Iteration 10 — Repository Maintenance & Clean Setup/Installation
* **Repository Cleanup**: Removed unused root screenshot files (`config.png`, `first-run.png`), legacy `~/.local/bin/ai-box` helper script, and stale translation keys from `src/i18n/da.json` while adding the missing `%n OS entries` plural form to `src/i18n/en.json`.
* **Single-Pass Clean Installer (`bin/install` & `pkgbuild/PKGBUILD`)**: Removed the duplicate `./bin/build` invocation before `makepkg` in `bin/install`, added `-c` (`--clean`) to automatically clean up `pkgbuild/pkg/` and `pkgbuild/src/` after installation, and declared explicit runtime/build dependencies (`acl`, `util-linux`, `qt6-base`) in `PKGBUILD`.
* **Idempotent Host Setup & Uninstaller (`bin/setup-system` & `bin/uninstall`)**: Ensured all `incus` initialization commands in `bin/setup-system` run via `sudo` so first-time setup succeeds before user group re-login, and updated `bin/uninstall --full` to also clean up `incusbr0` UFW firewall rules.

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
* **Agent Guidelines & Permanent Rules ([AGENTS.md](file:///home/user/Work/aibox/AGENTS.md))**: Added workspace rules and global rules enforcing that any AI agent adding a feature, modifying the UI, or fixing bugs must build (`./bin/build`), test (`./bin/test`), build the Arch package (`makepkg -f`), and update the system (`./bin/install`) so the device is always up to date.
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
* Standardized all project documentation ([README.md](file:///home/user/Work/aibox/README.md), [CATALOG.md](file:///home/user/Work/aibox/CATALOG.md)), helper scripts ([bin/setup-system](file:///home/user/Work/aibox/bin/setup-system), [bin/uninstall](file:///home/user/Work/aibox/bin/uninstall), [~/.local/bin/ai-box](file:///home/user/.local/bin/ai-box)), desktop metadata, and UI catalogs to English.

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
