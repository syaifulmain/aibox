# CATALOG.md — System Architecture, File Inventory, & Changelog

This document tracks all host system configurations, project files, architecture decisions, and the changelog of **AIBox** for easy maintenance, auditing, and future re-installation.

---

## 1. Host System Modifications (Omarchy / Arch Linux)

All host-level configurations are automated in [bin/setup-system](bin/setup-system) and can be reverted cleanly using [bin/uninstall](bin/uninstall).

| System Component | Path / Name | Purpose |
| :--- | :--- | :--- |
| **System Packages** | `incus`, `dnsmasq`, `xorg-xhost`, `acl`, `qemu-system-x86`, `edk2-ovmf` | System container & VM manager, local DHCP/DNS server, X11/Wayland display bridge, and POSIX ACL utilities. |
| **Subordinate UID/GID** | `/etc/subuid` & `/etc/subgid` (`root:1000000:1000000000`) | Maps container `root` to an unprivileged UID range on the Host so autonomous AI agents cannot damage the host OS. |
| **User Groups** | `incus-admin`, `kvm`, `render`, `video` (for the current user) | Grants passwordless access to Incus, hardware virtualization (`/dev/kvm`), and Intel Iris Xe GPU acceleration (`/dev/dri`). |
| **Systemd Drop-in ACL** | `/etc/systemd/system/incus.service.d/acl.conf` | Automatically applies read/write ACL permissions on `/var/lib/incus/unix.socket` when `incus.service` starts. |
| **Btrfs Storage Pool** | `default` (`/var/lib/incus/storage-pools/default`) | Native Btrfs subvolume storage pool with **Btrfs Quotas** enabled for strict per-environment disk size limits and <1s copy-on-write snapshots. |
| **Network Bridge** | `incusbr0` (`10.143.44.1/24`, IPv4 NAT enabled, IPv6 disabled) | Managed private bridge providing automatic DHCP IPv4 addresses and internet access to all environments. |
| **UFW Firewall Rules** | `ufw allow in on incusbr0` & `ufw route allow in/out on incusbr0` | Permits DHCP, DNS, and routed internet traffic from `incusbr0` through the active host UFW firewall. |
| **GPU & KVM Profile** | Incus profile `ai-gui` (`security.nesting=true`, `gpu` & `kvm` devices) | Enables nested Docker, hardware-accelerated Desktop GUI apps, and full-speed Android Emulators inside containers. |
| **Shared Workspaces** | `~/aibox/workspaces/<name>` (configurable) ⟷ `/home/workspace` | Two-way shared directory between Host and container using POSIX default ACLs (`setfacl`) and UID shifting (`shift=true`). Existing environments keep the workspace path they were created with, and their container mount is moved from `/workspace` to `/home/workspace` automatically on the next Start/Enter. |
| **App Configuration** | `~/aibox/config.json` & `~/.config/aibox/location.json` | Persists the active system root location (`location.json`), workspace and OS image storage locations, the editable OS catalog, preset titles, disk/RAM/CPU choices, defaults, and first-run completion (`config.json`). |
| **OS Image Storage** | `~/aibox/os-images` (configurable) | Offline OS image exports (`<id>` metadata + `<id>.root`) used as a fallback import source when the local Incus image is missing. |
| **Background Task State** | `~/aibox/state/` (`task.pid`, `task.meta`, `task.log`, `task.sh`) | Tracks detached background operations (`setsid`) so tasks survive GUI window closure and automatically re-attach on startup. |
| **Root File Access State** | `~/aibox/state/file-access/` (`<env>.json`, `<env>.log`) | PID/port of the per-environment localhost-only SFTP listener (`incus file mount --listen --no-auth`), so **Root Files** survives AIBox restarts and dead listeners are pruned on refresh. |

---

## 2. Project File Inventory (`~/Program/aibox`)

Built with Rust (edition 2021) + `cxx-qt` + Qt Quick 6, compiled via Cargo (`makepkg` for Arch packaging).
**Architecture decision:** Core rewritten in Rust (`aibox-core`); QML unchanged (`src/qml/`); `cxx-qt` binding (`aibox-app`) with tiny C++ shims for Qt APIs `cxx-qt` cannot express directly.

| File / Path | Category | Description |
| :--- | :--- | :--- |
| [rust/Cargo.toml](rust/Cargo.toml) | Workspace | Cargo workspace (`aibox-core`, `aibox-app`), version `0.3.0`. |
| [rust/crates/aibox-core/src/](rust/crates/aibox-core/src) | Source (Rust) | Pure Rust application core: `util`, `config`, `theme`, `i18n`, `secrets`, `session`, `history`, `snapshot`, `workspace`, `network`, `backup`, `health`, `provision`, `incus`, `task`, `engine`, and headless `cli`. |
| [rust/crates/aibox-core/tests/golden/](rust/crates/aibox-core/tests/golden) | Golden Fixtures | Committed `.jsonl` parity fixtures replayed by `aibox-core` `golden.rs`. |
| [rust/crates/aibox-app/src/main.rs](rust/crates/aibox-app/src/main.rs) & [lib.rs](rust/crates/aibox-app/src/lib.rs) | Source (Rust) | Process entry point dispatching `--redact-session` and headless CLI in Rust, and calling `aibox_gui_main` for the QML GUI. |
| [rust/crates/aibox-app/src/qt_backend*.rs](rust/crates/aibox-app/src/qt_backend.rs) | Source (Rust) | `cxx-qt` `Backend` QObject wrapping `Engine` and GUI state, split by feature module (`config`, `secrets`, `snapshot`, `workspace`, `network`, `backup`, `health`, `provision`, `sessions`, `task`). |
| [rust/crates/aibox-app/src/qt_theme.rs](rust/crates/aibox-app/src/qt_theme.rs) & [qt_translator.rs](rust/crates/aibox-app/src/qt_translator.rs) | Source (Rust) | `cxx-qt` `Theme` and `TranslatorState` QObjects with live file watching (`notify`) and Hyprland rounding polling. |
| [rust/crates/aibox-app/src/variant.rs](rust/crates/aibox-app/src/variant.rs) | Source (Rust) | `serde_json::Value` to `QVariant` / `QVariantList` / `QVariantMap` / `QStringList` converters. |
| [rust/crates/aibox-app/cpp/main.cpp](rust/crates/aibox-app/cpp/main.cpp) | Source (C++) | QML GUI entry point (`aibox_gui_main`) setting up `QGuiApplication` + `QQmlApplicationEngine`. |
| [rust/crates/aibox-app/cpp/shim/](rust/crates/aibox-app/cpp/shim) | Source (C++) | Tiny C++ shims (`translator_shim.{h,cpp}`, `clipboard_shim.{h,cpp}`, `qml_helpers.{h,cpp}`, `test_helpers.{h,cpp}`). |
| [rust/crates/aibox-app/tests/](rust/crates/aibox-app/tests) | Integration Tests | Offscreen Qt/QML integration tests (`backend_qobject.rs` for QObject/signal/async tests, `qml_smoke.rs` loading `qrc:/Main.qml`, all settings sections, and all dialogs with zero-warning enforcement, `qml_color_lint.rs` enforcing zero hardcoded colors or legacy tokens in `src/qml/`, and `theme_matrix.rs` for ignored offscreen screenshot matrix generation). |
| [src/qml/Main.qml](src/qml/Main.qml) | Source (QML) | Application window shell: state properties, dialog visibility, keyboard shortcuts, first-run overlay, and composition of the extracted panels and dialogs. |
| [src/qml/components/](src/qml/components) | Source (QML) | Reusable controls and design tokens. `Style.qml` (registered by `qmldir`) is the single source for surfaces, spacing, type scale, radii, control sizes and modal metrics. `ModalDialog` is the shared modal shell; `ActionButton`, `FlatInput`, `ChipInput`, `ModalHeader`, `MenuRow`, `TagBadge`, `MeterBar`, `SegmentedTabs`, `SelectChip`, `SelectCard`, `CheckRow`, `ResourceMeterCard`, `BrandMark` and `SlidersIcon` are shared building blocks. |
| [src/qml/dialogs/](src/qml/dialogs) | Source (QML) | Modal overlays: Create, Clone, Environment Actions, First Run, Wipe, Restore, Change Review, Help, History, Egress, Ports, Mounts, Edit Quotas, Backups, Secrets, Stale Report, Tools. |
| [src/qml/panels/](src/qml/panels) | Source (QML) | Main layout: `AppHeader`, `StatusBanner`, `MainContent` composition, `EnvironmentList`, `EnvironmentInspector`, `NetworkModeCard`, `SnapshotsCard`, `OsCatalogPanel`, and `SettingsPanel` with its `settings/` sections (locations, storage pool, OS catalog, create options, safety). |
| [src/resources.qrc](src/resources.qrc) | Resource | Bundles QML and translation catalogs directly into the `aibox` binary. |
| [src/i18n/en.json](src/i18n/en.json) & [da.json](src/i18n/da.json) | Localization | English plural rules and localization catalogs. |
| [pkgbuild/PKGBUILD](pkgbuild/PKGBUILD) | Packaging | Arch Linux package definition (`0.3.0`) installing `/usr/bin/aibox`, license, SVG icon, and `.desktop` entry. |
| [pkgbuild/aibox.desktop](pkgbuild/aibox.desktop) | Launcher | Desktop entry for the Omarchy App Launcher (`Super + Space`). |
| [pkgbuild/aibox.svg](pkgbuild/aibox.svg) | Asset | Scalable SVG application icon. |
| [bin/setup-system](bin/setup-system) | Script | Automated host setup script for Incus, Btrfs storage pool, UFW rules, and GPU/KVM profile. |
| [bin/build](bin/build) | Script | Compiles the `aibox-app` binary into `rust/target/release/aibox` via `cargo build --release --locked -p aibox-app`. |
| [bin/test](bin/test) | Script | Runs `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --workspace --release --locked` with `QT_QPA_PLATFORM=offscreen`. |
| [bin/install](bin/install) | Script | Standard installation entry point; builds the application and installs it into `~/.local/` (`~/.local/bin/aibox`, desktop entry, icon, and license) without `sudo`, or builds/installs the Arch Linux package with `--system`. |
| [bin/uninstall](bin/uninstall) | Script | Cleanly removes the user-local `aibox` installation and any legacy `aibox` pacman package (with `--full` option to also remove Incus, UFW rules, and all containers). |
| [AGENTS.md](AGENTS.md) | Agent Policy | Mandatory workflow, repository layout, style rules, and maintenance guidance for human and AI contributors (English, no emoji). |

---

## 3. Changelog

### Core Health: `crate::process`, Explicit `socket_writable`, Check Cadence, Single-Query Idle Stats & `checks/` Split (`rust/crates/aibox-core/src/process.rs`, `health/`)

- **Extracted `crate::process` (`process.rs`, `engine.rs`, `cli/`, `golden.rs`):** Moved `CommandOutput`, `CommandRunner`, and `SystemRunner` out of `health::checks` into a dedicated `aibox_core::process` module and updated all core, CLI, and golden test callers.
- **Explicit `CommandRunner::socket_writable` (`process.rs`, `health/checks/incus.rs`):** Replaced the `__socket_rw__` pseudo-program convention in `SystemRunner` with a typed `fn socket_writable(&self, path: &str) -> bool` method on `CommandRunner` (using `libc::access` in `SystemRunner`).
- **Modular `checks/` split (`health/checks/mod.rs`, `host.rs`, `incus.rs`, `devices.rs`):** Split `health/checks.rs` into focused submodules for host checks (`host.rs`), Incus/pool checks (`incus.rs`), and device/profile checks (`devices.rs`), preserving all `aibox_core::health::checks` exports.
- **Per-check cadence & result caching (`health/checks/mod.rs`):** Added `CheckCadence` (`Static` vs `Dynamic`), `CheckRunMode` (`StartupOrManual` vs `Poll`), `CachedHealthCheck`, and `HealthCheckCache::run_with_cadence` so background polls re-evaluate only dynamic checks (`incus_daemon`, `pool_capacity`) while preserving cached static check results and timestamps.
- **Single-query idle stats (`health/idle.rs`):** Added `IdleInstanceStats`, `extract_idle_stats_from_envs`, and `extract_idle_stats_from_incus_list` to extract CPU and network activity for all instances from a single `incus list --format=json` payload without per-instance queries.
- **Verification tests (`health/checks/mod.rs`, `health/idle.rs`, `process.rs`):** Added tests for fake-runner failure combinations (daemon unreachable, socket permission denied, missing KVM/GPU), cadence caching (`Poll` skipping static checks), multi-instance CPU/net extraction from one `incus list` payload, and `SystemRunner::socket_writable`.

### Core Backup: Manifest Verification, Safe Orphan Cleanup, `mod.rs` Split, Parallel Compression, Cached Scan & Scheduled Backups (`rust/crates/aibox-core/src/backup/`, `aibox-app`)

- **Modular split (`backup/mod.rs`, `export.rs`, `import.rs`, `archive_scan.rs`, `space.rs`):** Split `backup/mod.rs` into focused submodules for export/manifest/scheduled backups (`export.rs`), import/verification (`import.rs`), cached archive scanning (`archive_scan.rs`), and disk space checks (`space.rs`), preserving all `aibox_core::backup` exports.
- **Archive manifest & SHA-256 verification (`backup/export.rs`, `backup/import.rs`, `backup/scripts.rs`, `qt_backend_backup.rs`):** Added `BackupManifest`, streaming pure-Rust `sha256_file`, `write_backup_manifest`, and `verify_backup_archive` returning structured `BackupError` variants (`NotFound`, `ManifestCorrupt`, `SizeMismatch`, `ChecksumMismatch`, `ArchiveCorrupt`). Export scripts now write `<archive>.manifest.json` sidecars and `Backend::import_environment` verifies checksums and legacy archive headers before importing.
- **Safe orphan cleanup (`backup/orphans.rs`):** Added `OrphanCleanupMode` (default `DryRun`), `plan_orphan_cleanup` with an explicit `confirmation_list`, and `execute_orphan_cleanup` which re-verifies against live environments so active workspaces and snapshots are never deleted.
- **Parallel `zstd -T0` compression & cached archive scanning (`backup/export.rs`, `backup/scripts.rs`, `backup/archive_scan.rs`, `qt_backend_backup.rs`):** Added `CompressionCodec::detect()` (`zstd -T0` with `gzip` fallback) for exports and `.archive-index.json` metadata caching in `scan_backup_archives_with_stats` supporting both `.tar.gz` and `.tar.zst` archives.
- **Scheduled backups (`backup/export.rs`):** Added `ScheduledBackupPolicy`, `should_create_scheduled_backup`, `scheduled_backups_to_prune`, and `decide_scheduled_backup` for opt-in periodic exports integrated with `auto_snapshot_enabled`.
- **Verification tests (`backup/mod.rs`):** Added unit tests for NIST SHA-256 vectors, export-manifest-verify-import round-trips, corrupt/truncated/missing archive structured errors, dry-run and active-environment-protected orphan cleanup, `.archive-index.json` cache hits and `zstd` scripts, and scheduled backup decisions/pruning.

### Core Network: Incremental Egress Tailing, IPv6 Allowlist/Egress, ACL Diff & Safe IP Parsing (`rust/crates/aibox-core/src/network/`)

- **Safe IP parsing (`network/allowlist.rs`):** Removed `.unwrap()` calls on IP parsing and ensured invalid CIDRs, malformed hostnames, and garbage DNS responses are skipped with warnings without panicking.
- **Incremental egress parsing & aggregation (`network/egress.rs`):** Added `EgressEnvState` and `EgressLogCache` to track per-environment byte offsets (`Seek`), detect log rotation/truncation (file size shrinking resets offset), and aggregate `(DST, DPT, PROTO)` counts using a single-pass token parser instead of per-line regexes.
- **IPv6 support (`network/allowlist.rs`, `network/egress.rs`, `network/acl.rs`):** Added `is_private_or_reserved_ipv6`, `parse_ipv6_subnet`, `HostResolver::resolve_ips`, `resolve_allowlist_cidrs_with_ipv6`, IPv6 `SRC`/`DST` handling in kernel egress logs, and `/128` deduplication in `add_egress_destination_allowlist`.
- **Incremental ACL diff (`network/acl.rs`):** Added `diff_acl_cidrs`, `parse_acl_egress_destinations`, and `acl_diff_shell_snippet` to compute minimal rule additions/removals instead of recreating the entire Incus ACL.
- **Verification tests (`network/`):** Added tests for a 100k-line egress log fixture (second refresh without new data completing in `< 5 ms`), log append and rotation, IPv6 allowlist/egress parsing, ACL diffing, and invalid allowlist entry handling.

### Core Workspace: Symlink-Safe Restore, Streaming Diff, Diff Cache & `diff/` Split (`rust/crates/aibox-core/src/workspace/diff/`, `aibox-app`)

- **Modular split (`workspace/diff/mod.rs`, `rsync.rs`, `file.rs`, `restore.rs`):** Split `workspace/diff.rs` into focused submodules for rsync arg building/parsing/caching (`rsync.rs`), per-file diff preparation and binary/line checks (`file.rs`), and symlink-safe single-file restore (`restore.rs`), preserving all `aibox_core::workspace::diff` exports.
- **Symlink-safe single-file restore (`workspace/diff/restore.rs`):** Replaced path-based `create_dir_all` / `remove_file` / `copy` in `restore_single_file` with `openat` + `O_NOFOLLOW` directory traversal, `fstatat(AT_SYMLINK_NOFOLLOW)` + `readlinkat` root-escape checks, and `openat(O_EXCL | O_NOFOLLOW)` + `renameat` atomic replacement.
- **Streaming incremental diff & diff cache (`workspace/diff/rsync.rs`, `qt_backend.rs`, `qt_backend_workspace.rs`):** Added `StreamingRsyncParser` and `WorkspaceDiffCache` (keyed by `(env, snapshot, workspace root mtime, excludes)`). `Backend::load_workspace_diff` now streams `rsync` stdout line-by-line with batched UI updates while `diffLoading` is active, caches completed results, and invalidates on `restore_single_file`.
- **Verification tests (`workspace/diff/`):** Added tests for target/parent symlink escape rejection, concurrent symlink-swap race safety, incremental streaming parity, diff cache hit/invalidation, and a 10,000-file workspace diff benchmark completing in `< 3 s` with default excludes.

### Core Snapshot: Centralized Quoting, Async Size Cache, Smart Auto-Snapshot & Tiered Retention (`rust/crates/aibox-core/src/snapshot/`, `aibox-app`)

- **Centralized shell quoting (`snapshot/workspace.rs`, `snapshot/scripts.rs`, `golden/snapshot.jsonl`, `golden/backup.jsonl`):** Routed all error/warning `echo` messages in `workspace_delete_snapshot_snippet`, `workspace_restore_snippet`, and `convert_workspace_to_subvolume_script` through `util::shell::shell_quote` so `$` and `"` inside hostile names/paths are never expanded or broken. Updated reviewed golden fixtures and added hostile-name (` `, `'`, `$`, `\n`) unit and golden tests.
- **Async snapshot size calculation & cache (`snapshot/sizes.rs`, `qt_backend.rs`):** Extracted snapshot size parsing (`parse_snapshot_sizes_json`, `parse_du_bytes_output`, `parse_btrfs_qgroup_bytes`), added `compute_workspace_snapshot_sizes` (`btrfs qgroup show -r --raw` with `du -sb` fallback) and `SnapshotSizeCache`, and wired `Backend` to compute missing read-only snapshot sizes on a background worker thread and prune deleted entries.
- **Smart auto-snapshot skip (`snapshot/workspace.rs`, `snapshot/auto.rs`):** Added `workspace_unchanged_since_last_snapshot` (comparing Btrfs subvolume `Generation` when available and falling back to filesystem tree mtime) so `decide_auto_snapshot` skips creating redundant workspace auto-snapshots when nothing has changed since the last snapshot.
- **Tiered retention policy (`snapshot/auto.rs`):** Added `RetentionPolicy` and `auto_snapshots_to_prune_tiered` supporting recent, daily, and weekly retention buckets while preserving manual snapshots.
- **Verification tests (`snapshot/auto.rs`, `snapshot/scripts.rs`, `snapshot/sizes.rs`, `snapshot/workspace.rs`):** Added unit tests for `min_interval_seconds` with a fake clock, tiered retention pruning, smart auto-snapshot skip/trigger, `btrfs qgroup` parsing and cache pruning, non-Btrfs degradation messages, and hostile-name script syntax/execution checks.

### Core History: Atomic Writes, Monthly JSONL Storage, In-Memory Cache & Pruning (`rust/crates/aibox-core/src/history.rs`, `engine.rs`, `aibox-app`)

- **Atomic writes (`history.rs`):** Switched history log writes, monthly metadata updates, and the `history-seen` marker (`acknowledge_operation_history`) to `atomic_write` so no partial files or leftover temp files occur on crash.
- **Monthly append-only JSONL format (`history.rs`):** Persisted new operation history metadata in monthly `<YYYY-MM>.jsonl` files while keeping backwards-compatible loading and pruning for legacy per-operation `*.json` files. Added `prune_operation_history` supporting both count-based and age-based pruning.
- **In-memory history cache (`engine.rs`, `qt_backend*.rs`):** Added `HistoryCache` on `Engine` (`reload_history`, `set_history_cache`, `invalidate_history`, `acknowledge_history`, `clear_history`, `recent_failed_history_count`) so QML `recentFailedHistoryCount` bindings and `acknowledge_operation_history` read from memory instead of scanning disk on every evaluation.
- **Verification tests (`history.rs`, `engine.rs`):** Added unit tests for corrupted JSON/JSONL line skipping, count and age pruning, unread failure counting, `Engine::history_cache` tracking, and a 1000-entry list + count benchmark completing in `< 20 ms`.

### Core Session: Streaming Redaction, Atomic Write, Single-Pass ANSI & `sessions.idx` (`rust/crates/aibox-core/src/session.rs`)

- **Streaming atomic redaction (`util/paths.rs`, `secrets/redact.rs`, `session.rs`):** Added `atomic_write_with` in `util::paths` (shared with `atomic_write_with_mode`) and `StreamingRedactor::finish_with_status` in `secrets::redact`. `redact_session_log_file` now streams 64 KiB chunks (`BufReader`/`BufWriter`) directly into a `0600` sibling temp file and atomically commits only when secrets were redacted, keeping memory constant for large logs.
- **Single-pass streaming ANSI stripper (`session.rs`):** Replaced the 4-pass regex pipeline in `strip_ansi_sequences` with `StreamingAnsiStripper`, a single-pass byte state machine handling OSC (`\x07` / `\x1b\\`), CSI, 2-byte ESC, short ESC, `\r`, and `\x07` across arbitrary chunk boundaries. `read_recorded_session_log` now tail-seeks to the 400 KiB raw budget and streams 64 KiB chunks through `StreamingAnsiStripper`.
- **Indexed retention pruning (`session.rs`):** Added a lightweight `<sessions_dir>/sessions.idx` cache (`SessionIndex` keyed by `env_name` and directory nanosecond mtime) written via `atomic_write_with_mode(..., Some(0o600))` so `prune_recorded_sessions` skips `read_dir` and per-file `stat` syscalls on unchanged environment directories.
- **Verification tests (`session.rs`):** Added unit tests for ANSI sequences split across 1/3/5/7-byte chunk boundaries, 50 MiB session log streaming redaction + tail reading, and `sessions.idx` creation, reuse, and refresh.

### Core Secrets: `SecretsError`, Unified Atomic Write, Aho-Corasick Redaction & Security Tests (`rust/crates/aibox-core/src/secrets/`)

- **Typed error (`secrets/error.rs`):** Added `SecretsError` (`thiserror`) across `store.rs`, `state.rs`, and `inject.rs`, ensuring `Display` never exposes secret values (only secret names, paths, modes, or static messages), with `From<SecretsError> for String` for boundary callers.
- **Regex-free validation & unified atomic write (`secrets/store.rs`):** Replaced per-call `Regex::new` in `is_valid_secret_name` with direct ASCII byte validation (`^[A-Z_][A-Z0-9_]*$`), and replaced custom temp-file/rename logic in `save_secrets_store` with `util::paths::atomic_write_with_mode(..., Some(0o600))` while preserving `0700` directory permissions.
- **Non-polling inject wait & zeroize (`secrets/inject.rs`, `secrets/mod.rs`, `secrets/state.rs`):** Replaced the 15 ms `try_wait` polling loop with `wait_child_with_timeout` (`child.wait_with_output()` in a worker thread + `mpsc::recv_timeout` + `SIGKILL` on timeout). Added volatile `zeroize_bytes` / `zeroize_string` helpers and applied them to transient secret buffers, JSON payloads, env-file strings, and `SecretsState::drop`.
- **Single-pass Aho-Corasick & streaming redaction (`secrets/redact.rs`):** Implemented `SecretRedactor` (leftmost-longest O(n) multi-pattern Aho-Corasick matcher) and `StreamingRedactor` (buffering `max_secret_len - 1` trailing bytes across arbitrary chunk boundaries).
- **Security & canary tests (`secrets/mod.rs`):** Added tests for `0600`/`0700` creation under permissive `umask(0o000)`, chunk-boundary streaming redaction, non-leakage in history/session/error messages, child timeout killing, buffer zeroization, multi-pattern Aho-Corasick redaction, and an end-to-end canary secret state-directory leak check.

### Core i18n: Zero-Allocation Lookup, Placeholder Validation & Catalog Lint (`rust/crates/aibox-core/src/i18n.rs`, `src/i18n/da.json`)

- **Zero-allocation catalog lookup (`i18n.rs`, `qt_translator.rs`):** Changed `Catalog` and `Entry` storage from `String` to `Box<str>` (`HashMap<Box<str>, Entry>`) and `Catalog::lookup` to return `Option<&str>`, eliminating per-lookup `String` allocations on the core side.
- **Placeholder validation (`i18n.rs`):** Added `extract_placeholders`, `placeholders_match`, `find_placeholder_mismatches`, and `PlaceholderMismatch` to verify `%n` and positional (`%1`, `%2`, ...) placeholders match between source keys and translations while allowing argument reordering.
- **Catalog lint & Danish parity (`tests/i18n_catalog_lint.rs`, `src/i18n/da.json`):** Added `i18n_catalog_lint` to extract all `qsTr("...")` literals from `src/qml/**/*.qml`, fail on missing `da.json` translations, fail on orphan keys in `da.json`/`en.json`, and run `find_placeholder_mismatches` on both catalogs. Added the 21 missing Danish translations to `src/i18n/da.json`.
- **Documentation (`README.md`):** Documented the steps to add a new translation catalog (e.g. `id.json`) and register it in `src/resources.qrc`.

### Core Theme: `Copy` `Rgba` Representation, Palette Cache & Split `palette/` (`rust/crates/aibox-core/src/theme/`)

- **`Copy` color representation (`color.rs`, `palette/mod.rs`, `icon.rs`, `qt_theme.rs`):** Added `struct Rgba(u8, u8, u8, u8)` (`Copy`, `Serialize`, `Display`) and switched `Palette` fields from `String` to `Rgba`, removing 49 `.clone()` calls in `Palette::tokens()` and formatting to `#rrggbb`/`#aarrggbb` only when exported to QML, JSON, or dumps. `icon::render` now reads `Rgba::rgb()` directly without re-parsing hex.
- **Palette cache (`palette/cache.rs`):** Added bounded `PaletteCache` keyed by deterministic FNV-1a hashes of `(ThemeColors, PaletteOptions)` and `(toml_bytes, PaletteOptions)`, wired into `derive_palette_with_options` and `load_colors_and_palette` (shared with `qt_theme.rs`) so file-watcher reloads with unchanged contents skip both parsing and palette derivation.
- **Split `palette/` (`palette/contrast.rs`, `palette/derive.rs`, `palette/cache.rs`, `palette/mod.rs`):** Separated WCAG contrast thresholds, status derivation, and `validate_palette_contrast` into `palette/contrast.rs`, pure derivation into `palette/derive.rs`, and caching into `palette/cache.rs`.
- **Tests & verification:** Added `validate_palette_contrast` coverage across all 23 Omarchy fixtures in `Auto`, `High Contrast`, `Light`, and `Dark` modes, `PaletteCache` hit/miss tests, a `< 1 ms` palette build timing test, and a golden JSON snapshot test (`tests/theme_palette_golden.rs` + `tests/golden/theme_palettes.json`).

### Core Config: Typed `ConfigError`, Schema Versioning & Split Settings (`rust/crates/aibox-core/src/config/`)

- **Typed error (`config/error.rs`):** Added `ConfigError` (`thiserror`) with `Io(#[from] IoError)`, `Parse { path, source }`, and `Invalid { field, reason }` variants, replacing `Result<_, String>` across `config/model.rs` and `config/settings/`. Boundary callers in `aibox-app` (`qt_backend*.rs`) and `golden.rs` convert `ConfigError` to `String`/`QString`.
- **Schema versioning & migration (`config/model.rs`):** Added `CURRENT_SCHEMA_VERSION = 2`, `Config::schema_version`, and sequential `migrate_v1_to_v2` during `apply_object`. Added `load_checked` and `load_from_str_checked` for structured parse/IO error reporting while `load`, `load_from_str`, and `Engine::new` fall back to defaults on broken or truncated configs.
- **No-op write avoidance (`config/model.rs`):** `save` and `save_json_str` compare formatted JSON bytes against the existing file via `write_json_if_changed` and skip `atomic_write` when unchanged.
- **Split settings (`config/settings/`):** Split `settings.rs` into `settings/mod.rs`, `settings/locations.rs`, `settings/catalog.rs`, and `settings/safety.rs`, re-exporting all public items from `config::settings`.
- **Tests:** Added unit tests for `ConfigError` formatting, broken/truncated config fallback, v1-to-v2 schema migration, and skipping disk writes when config content is unchanged.

### Core Utilities: Typed File Errors, Durable Atomic Writes & Regex-Free Path Sanitizing (`rust/crates/aibox-core/src/util/`)

- **Shared error type (`util/error.rs`):** Added `IoError` (`thiserror`), which records the failed operation, path, and `io::Error` cause, so domain error types can wrap it via `#[from]`. A `From<IoError> for String` conversion keeps existing `Result<_, String>` callers working.
- **Durable atomic writes (`util/paths.rs`):** `atomic_write` now returns `IoError` and fsyncs the parent directory after the rename. The new `atomic_write_with_mode` applies a Unix permission mode (e.g. `0o600`) to the temp file before writing, so secret files can use it.
- **Regex-free sanitizing:** `cache_base_path` replaces characters outside `[A-Za-z0-9._-]` by iterating over chars, so it no longer compiles a regex on every call. The output is unchanged.
- **Tests:** Added a randomized `sh -c printf` round-trip test for `shell_quote`, tests that atomic writes replace the file, set the requested mode, and return an error in a read-only directory, a `cache_base_path` test, and doc-tests for `shell_quote`, `shell_array_words`, and `parse_size_bytes`.

### Create Dialog Top Input Headers & Settings Modal Rounded Left Corners (`CreateDialog.qml`, `SettingsPanel.qml`)

- **Stacked input headers (`src/qml/dialogs/CreateDialog.qml`):** Restored stacked section headings (`theme.textPrimary`, `Style.fontBody`, `Font.Medium`) above the `Project / Environment Name` and `Shared Folder Location (optional)` inputs (with the `Shared at: ...` hint right-aligned on the first header row) to match the other section titles in `CreateDialog`.
- **Settings modal left corners (`src/qml/panels/SettingsPanel.qml`):** Inset the `SettingsPanel` body by `Style.hairline` inside `ModalDialog` and gave the left sidebar `radius: Math.min(Style.radiusLg, theme.radius)` (with a straight inner right edge against the divider) so the top-left and bottom-left corners follow the Omarchy window radius and preserve the modal border.

### Hide ScrollBar Indicators Across All ScrollViews (`src/qml/`)

- **Scrollbars hidden (`EnvironmentInspector.qml`, `SettingsPanel.qml`, `SecretsDialog.qml`, `CreateDialog.qml`, `StaleReportDialog.qml`, `ChangeReviewDialog.qml`, `HistoryDialog.qml`, `ComponentGallery.qml`):** Set `ScrollBar.horizontal.policy: ScrollBar.AlwaysOff` and `ScrollBar.vertical.policy: ScrollBar.AlwaysOff` on all `ScrollView`s so wheel/touchpad/keyboard scrolling works without visible scrollbar indicators.

### Secrets Inline Save & History List Surface (`SecretsDialog.qml`, `HistoryDialog.qml`)

- **Secrets inline Save (`src/qml/dialogs/SecretsDialog.qml`):** Moved the `Save` primary button onto the same `RowLayout` to the right of the secret inputs and removed the separate footer row.
- **History list surface (`src/qml/dialogs/HistoryDialog.qml`):** Switched unselected task and session list items from `theme.surfaceCard` to `theme.surfaceInput` to match the environment sidebar.

### Network Card Telemetry Grid (`NetworkModeCard.qml`, `da.json`)

- **Network telemetry summary (`src/qml/panels/NetworkModeCard.qml`, `src/i18n/da.json`):** Added a 2-column monospace telemetry grid (`Receiving`, `Sending`, `Downloaded`, `Uploaded`, `IP Address`, `Interface`) on the right half of the `Network` card (50/50 split with a vertical hairline divider) using existing `backend.selectedEnvironment` metrics, and moved click-to-copy IP from the card header into the `IP Address` telemetry value.

### Inspector Two-Column Workspace & Root Files Row & Card Surface (`EnvironmentInspector.qml`, `EnvironmentList.qml`, `Card.qml`, `ResourceMeterCard.qml`)

- **Shared Workspace & Root Files layout (`src/qml/panels/EnvironmentInspector.qml`):** Placed the `Shared Workspace` and `Root Files` cards side by side in a 2-column `GridLayout` (`Layout.fillHeight: false`) with equal widths and matching compact heights (`Math.max(sharedWorkspaceCard.implicitHeight, rootFilesCard.implicitHeight)`).
- **OS Images row alignment (`src/qml/panels/OsCatalogPanel.qml`):** Aligned the OS `TagBadge` and right-aligned `Cached` / `Not Cached` size text in fixed columns matching `OsCatalogSection.qml`, and gave `Remove Cache` / `Download OS` a uniform `minWidth` (`Style.buttonMinWidth + Style.padModal + Style.gapMd`).
- **Sidebar & card surface (`src/qml/panels/EnvironmentList.qml`, `src/qml/panels/SettingsPanel.qml`, `src/qml/panels/OsCatalogPanel.qml`, `src/qml/panels/settings/OsCatalogSection.qml`, `src/qml/components/Card.qml`, `src/qml/components/ResourceMeterCard.qml`):** Unified the environment list and settings sidebars, all inspector cards (the 3 resource cards, `Shared Workspace`, `Root Files`, `Network`, and `Btrfs Snapshots`), and OS catalog cards on `theme.surfaceInput` (with `theme.surfaceCard` for nested non-downloaded OS icon tiles).

### Disabled Primary Button Contrast & Dialog Polish (`ActionButton.qml`, `CheckRow.qml`, `SecretsDialog.qml`, `MountsDialog.qml`)

- **Disabled button contrast (`src/qml/components/ActionButton.qml`):** Disabled buttons across all variants (including `primary` Save buttons) now render with `theme.surfaceInput`, `theme.borderSubtle` border, and `theme.textDisabled` label without double-dimming via `opacityDisabled`, keeping disabled primary labels legible on both light and dark themes.
- **CheckRow wrapping (`src/qml/components/CheckRow.qml`):** Enabled `Text.WordWrap` and dynamic `implicitHeight` so long checkbox labels (such as workspace path confirmations in `WipeDialog`) wrap cleanly instead of overflowing the row border.
- **Secrets & Mounts dialog layout (`src/qml/dialogs/SecretsDialog.qml`, `src/qml/dialogs/MountsDialog.qml`, `Main.qml`):** Tightened `SecretsDialog` `cardHeight` sizing, switched list row action buttons (`Copy`, `Delete`) to `compact: true`, and removed `Show`/`Hide` buttons and reveal state; moved `MountsDialog` `Read-Only (Safe)` / `Read-Write` chips into `DialogFooter.leftActions` so they align at the same `Style.controlHeight` on the same row as `Add Mount`.

### UI Standardization Phase 7 — QA & Lint (`qml_token_lint.rs`, `qml_smoke.rs`, `theme_matrix.rs`, `Style.qml`, `src/i18n/`)

- **UI standardisation complete; token lint enforced.**
- **Token & layout lint (`rust/crates/aibox-app/tests/qml_token_lint.rs`):** Upgraded from report mode to strict error enforcement across `src/qml/`, rejecting numeric metric literals (outside explicit allowlists for `Style.qml`, `SlidersIcon.qml`, `BrandMark.qml`, and `dev/ComponentGallery.qml`), deprecated tokens, uncapped `Style.radius*` bindings, numeric `cardWidth` expressions, `Cancel` buttons alongside an active `ModalHeader` Close button, and multiple `primary` buttons in a `DialogFooter`.
- **Cleanup (`Style.qml`, `ActionButton.qml`, `src/i18n/`):** Removed all deprecated numeric/alias tokens from `Style.qml`, removed the legacy `"theme"` variant alias from `ActionButton.qml`, wrapped remaining `Style.radius*` bindings in `Math.min(..., theme.radius)` (or `width / 2` for status dots), removed hardcoded `minWidth` literals from dialog buttons, and pruned unused translation keys from `src/i18n/en.json` and `src/i18n/da.json`.
- **Automated QA (`qml_smoke.rs`, `theme_matrix.rs`, `test_helpers.cpp`):** Extended `qml_smoke` to verify Close button click, Esc, and scrim click on every modal dialog as well as `Style.controlHeight` / `Style.controlHeightCompact` on all visible interactive controls; added a 23-theme × light/dark × high-contrast `ComponentGallery` zero-warning check in `theme_matrix.rs`.

### UI Standardization Phase 6 — Settings: standardized panel, sections, and tiered Esc (`SettingsPanel.qml`, `panels/settings/`, `Main.qml`, `HelpOverlay.qml`, `FlatInput.qml`, `src/i18n/`)
* **Settings Panel (`src/qml/panels/SettingsPanel.qml`)**: Switched to `ModalDialog` `size: "lg"` with `Style.modalGutter` height margin, replaced deprecated spacing/font/radius tokens and numeric literals with semantic `Style.qml` tokens, and standardized the left navigation sidebar (`surfaceSidebar`, `surfaceSelected` + `accent` indicator, `fontCaption` uppercase group headers).
* **Settings Subpages (`src/qml/panels/settings/`)**: Rebuilt all six sections (`LocationsSection`, `StoragePoolSection`, `OsCatalogSection`, `CreateOptionsSection`, `SafetySection`, `AppearanceSection`) on `Card`, `SectionHeader`, `FormRow`, `ToggleChip`, `SelectChip`, and `MeterBar`; added `mono` support to `FlatInput.qml`; standardized `OsCatalogSection` rows to `listRowHeight` with 34 px icon tiles and `compact` `Edit`/`Remove` buttons; aligned `CreateOptionsSection` quota rows via `FormRow`. Zero `qml_token_lint` findings across `SettingsPanel.qml` and `panels/settings/`.
* **Tiered `Esc` & Inline Cancel Removal (`OsCatalogSection.qml`, `StoragePoolSection.qml`, `Main.qml`, `HelpOverlay.qml`, `README.md`)**: Removed inline `Cancel` buttons from the OS catalog editor and storage pool resize confirmation; pressing `Esc` closes the active inline form first and closes the Settings modal on the second press. Documented `Esc` in `HelpOverlay.qml` and `README.md`, and added new UI strings to `src/i18n/en.json` and `src/i18n/da.json`.

### Inspector: standardized cards, toggles, and OS catalog rows (`MainContent.qml`, `EnvironmentInspector.qml`, `NetworkModeCard.qml`, `SnapshotsCard.qml`, `OsCatalogPanel.qml`, `ResourceMeterCard.qml`)
* **Environment Inspector (`src/qml/panels/EnvironmentInspector.qml`, `src/qml/components/ResourceMeterCard.qml`)**: Consolidated header actions (`Stop OS` / `Start OS`, `More Actions`, `Enter OS Terminal` / `New from Template`) into a single row to the right of the title, updated `ResourceMeterCard` active percentage labels to use `textMuted` (switching to `warningText`/`dangerText` only at thresholds), rebuilt **Shared Workspace** on `Card` + `KeyValue` (`Host ↔ Guest`), and rebuilt **Root Files** on `Card` with a `ToggleChip` (`Enabled`) instead of an accent button.
* **Network & Snapshots Cards (`src/qml/panels/NetworkModeCard.qml`, `src/qml/panels/SnapshotsCard.qml`, `src/i18n/`)**: Rebuilt both cards on `Card` with `SectionHeader`-aligned actions, standardized `SelectChip` (`Online` / `Allowlist` / `Offline`) and `ToggleChip` (`Record`, `Auto-snapshot`, `Workspace`, `Auto-stop critical`) controls to 34 px (`Style.controlHeight`), used `compact` action buttons in snapshot rows, centered the empty state in `fontSmall` `textMuted`, and removed legacy `*: ON`/`*: OFF` strings from `en.json` and `da.json`.
* **OS Catalog & Main Content (`src/qml/panels/OsCatalogPanel.qml`, `src/qml/panels/MainContent.qml`)**: Standardized `OsCatalogPanel` rows to `listRowHeight` with 34 px icon tiles, `fontBody` names, `fontCaption` mono image IDs, `TagBadge` metadata, and `compact` action buttons, and migrated `MainContent.qml` to semantic `Style.qml` tokens. Zero `qml_token_lint` findings across all Phase 4 files.

### App Shell: standardized header, sidebar, status banner, and first-run modal (`Main.qml`, `AppHeader.qml`, `EnvironmentList.qml`, `StatusBanner.qml`, `FirstRunDialog.qml`)
* **App Header (`src/qml/panels/AppHeader.qml`)**: Migrated to semantic `Style.qml` tokens (`padSection`, `padCard`, `gap`, `fontTitle`, `controlHeight`) with a 34×34 `BrandMark`, `SegmentedTabs`, and standardized right-aligned action buttons without hardcoded widths.
* **Environment Sidebar (`src/qml/panels/EnvironmentList.qml`)**: Adopted `SectionHeader` for the "Environments" title, `Card` for the bottom Storage Pool summary, `Style.gap` list spacing, and semantic tokens for the status dot (`gapMd`/`radiusMd`), typography, and padding.
* **Status Banner (`src/qml/panels/StatusBanner.qml`)**: Standardized to tone-driven `*Surface`/`*Border`/`*Text` colors (`info`, `success`, `warning`, `danger`), `padCard` padding, `radiusMd` corners, `compact` action buttons, and renamed the task cancellation button from `"Cancel"` to `"Cancel Task"` (with translations in `src/i18n/en.json` and `src/i18n/da.json`).
* **Canvas & First-Run Dialog (`src/qml/Main.qml`, `src/qml/panels/MainContent.qml`, `src/qml/panels/EnvironmentInspector.qml`, `src/qml/dialogs/FirstRunDialog.qml`, `src/qml/components/Style.qml`)**: Removed the outer card wrapper from `EnvironmentInspector` so content cards sit directly on `surfaceCanvas` with `padSection` margins/gaps, added default window metric tokens to `Style.qml`, and extracted the inline first-run storage modal from `Main.qml` into `src/qml/dialogs/FirstRunDialog.qml` (registered in `src/resources.qrc`). Zero `qml_token_lint` findings across all Phase 3 files.

### Components: unified control sizes and new layout primitives (`src/qml/components/`, `src/qml/dev/ComponentGallery.qml`)
* **Standardized Existing Components**: Migrated all reusable controls in `src/qml/components/` (`ActionButton`, `FlatInput`, `SegmentedTabs`, `SelectChip`, `SelectCard`, `TagBadge`, `CheckRow`, `ChipInput`, `MenuRow`, `MeterBar`, `ResourceMeterCard`, `ModalHeader`, `ModalDialog`, `BrandMark`, `ThemeDebugOverlay`) to semantic `Style.qml` tokens with zero deprecated tokens or numeric metric literals (aside from `SlidersIcon` geometry).
* **Unified `ActionButton` & `ModalDialog` APIs**: Removed duplicate boolean variant props (`primary`, `danger`, `success`, `accent`) in favor of `variant` (`default`, `primary`, `danger`, `accent`), added `compact` mode and `buttonMinWidth` default (square `34×34` for icon-only buttons), and added `size` (`"sm" | "md" | "lg"`) to `ModalDialog`. Updated existing boolean `ActionButton` callers across panels and dialogs to use `variant`.
* **New Layout & Control Primitives**: Added `Card.qml` (also used as the base for `ResourceMeterCard.qml`), `SectionHeader.qml`, `ToggleChip.qml` (with `ON`/`OFF` translations in `en.json` and `da.json`), `DialogFooter.qml`, `FormRow.qml`, and `KeyValue.qml`, registered in `src/resources.qrc` and showcased in `src/qml/dev/ComponentGallery.qml`.

### Design tokens: semantic scale introduced (`Style.qml`, `palette.rs`, `qml_token_lint.rs`, `ComponentGallery.qml`)
* **Semantic Design Tokens (`src/qml/components/Style.qml`)**: Introduced semantic typography (`fontCaption`, `fontSmall`, `fontBody`, `fontTitle`, `fontHeading`, `fontDisplay`), spacing (`gapXs`, `gapSm`, `gapMd`, `gap`, `padCard`, `padSection`, `padModal`), corner radius (`radiusSm`, `radiusMd`, `radiusLg`), control/button (`controlHeightCompact`, `buttonMinWidth`), and modal width (`modalWidthSm`, `modalWidthMd`, `modalWidthLg`) tokens. Existing numeric tokens retain their values and are marked `// deprecated, remove in phase 7` so visual appearance stays unchanged during incremental migration.
* **Muted Text Contrast Verification (`rust/crates/aibox-core/src/theme/palette.rs`)**: Added `text_muted_meets_contrast_against_card_and_sidebar_across_all_fixtures` verifying `textMuted` >= 4.5:1 against both `surfaceCard` and `surfaceSidebar` across all 23 Omarchy theme fixtures.
* **Report-Mode QML Token Lint (`rust/crates/aibox-app/tests/qml_token_lint.rs`)**: Added `qml_token_lint` integration test in report mode logging deprecated `Style.*` token usages and numeric metric literals per QML file without failing the build.
* **Component Gallery Token Scale (`src/qml/dev/ComponentGallery.qml`)**: Added a **Token Scale** page displaying the new typography, spacing, and radius tokens.

### Build Linker Selection Without Deprecated `ld.gold` Warning (`rust/crates/aibox-app/build.rs`, `pkgbuild/PKGBUILD`)
* Updated `aibox-app/build.rs` to prefer `lld` or `mold` when installed and fall back to standard GNU `ld` (`bfd`) instead of allowing `qt-build-utils` to inject `-fuse-ld=gold`, eliminating the deprecated gold linker compiler warning on systems without `lld`. Added `lld` to `makedepends` in `pkgbuild/PKGBUILD`.

### UI Standardization Phase 5 — Dialogs: single Close action, Cancel removed (`src/qml/dialogs/`, `rust/crates/aibox-app/cpp/shim/test_helpers.cpp`, `src/i18n/`)
* **Single Modal Pattern (`src/qml/dialogs/*.qml`)**: Standardized all 17 modal dialogs to use `ModalDialog.size` (`sm`, `md`, `lg`) instead of numeric `cardWidth` expressions, replaced all deprecated `Style.*` tokens and numeric metric literals with semantic `Style.*` tokens (zero `qml_token_lint` findings under `dialogs/`), and adopted `DialogFooter`, `FormRow`, `CheckRow`, `SelectChip`, and `ToggleChip`.
* **Cancel Removal**: Removed redundant `Cancel` buttons from `CloneDialog`, `WipeDialog`, `RestoreDialog`, `EditQuotasDialog`, and the inline confirmation bar in `StaleReportDialog` (whose `onClosed`/`onDismissed` handlers also reset `staleConfirmAction`), keeping `ModalHeader` Close, `Escape`, and scrim click as the unified close mechanism and preserving `Cancel Creation` in `CreateDialog` while a task is running.
* **Modal Dismiss Verification (`rust/crates/aibox-app/cpp/shim/test_helpers.cpp`, `src/i18n/{en,da}.json`)**: Extended `qml_smoke` to verify that both `Escape` and scrim clicks close every modal dialog with zero QML warnings, and added `Add Port`, `Add Mount`, and `Include Workspace` translations to `en.json` and `da.json`.

### Settings Sidebar UI Polish (`src/qml/panels/SettingsPanel.qml`, `src/i18n/`)
* Modernized the Settings modal sidebar with subtle category headers (`System`, `Environments`, `Interface`), hover feedback (`theme.surfaceRaised`), a left accent indicator pill on the active item (`theme.accent` + `theme.accentText`), `Up`/`Down` keyboard navigation, and a compact footer showing the active polarity and navigation hints.

### Adaptive Theme Polish — Surface Contrast for Default `ActionButton` and `ResourceMeterCard` (`src/qml/components/`)
* Updated default `ActionButton` (e.g. `More Actions`) and `ResourceMeterCard` (`Disk Usage`, `Memory Usage`, `CPU Usage`) resting fill from `theme.surfaceCard` to `theme.surfaceInput` so they visually separate from the surrounding `EnvironmentInspector` `theme.surfaceCard` surface in both light and dark modes.

### Adaptive Theme M9 — Appearance Preferences & Polish (`aibox-core`, `aibox-app`, `src/qml/`, `src/i18n/`)
* **Theme Mode Override & High Contrast (`theme/palette.rs`, `config/{model,settings}.rs`)**: Added `ModeOverride` (`auto`, `light`, `dark`) and `PaletteOptions { mode_override, high_contrast }` with `derive_palette_with_options` / `load_palette_with_options`. When forced opposite to the active Omarchy theme polarity, surfaces are derived from the opposite pole while keeping accent/status hues contrast-guarded. High-contrast mode raises text thresholds to 7:1 (WCAG AAA) and border thresholds to 3:1.
* **Chart Hue Tokens (`theme/palette.rs`, `qt_theme.rs`)**: Exposed `chart1`..`chart4` (`accent`, `orange`, `magenta`, `cyan`) on `Palette` and `Theme` QObject, contrast-guarded at >= 3:1 against `surfaceCard`.
* **Appearance Settings (`qt_backend*.rs`, `SettingsPanel.qml`, `AppearanceSection.qml`)**: Added `themeModeOverride`, `highContrast`, and `reduceMotion` config fields, `Backend::saveAppearanceSettings`, `Theme::setPreferences`, and the new **Appearance** section in Settings with `en.json` and `da.json` translations.
* **Smooth Theme Transitions & Debug Overlay (`Main.qml`, `EnvironmentList.qml`, `EnvironmentInspector.qml`, `ThemeDebugOverlay.qml`, `HelpOverlay.qml`)**: Added 150 ms `ColorAnimation` transitions on the three primary surfaces (window canvas, sidebar, inspector card) gated by `!theme.reduceMotion`, plus a `Ctrl+Shift+T` theme token debug overlay showing the matching `theme.<token>` name and hex value under the cursor.

### Adaptive Theme M8 — QA: Color Lint, Contrast Checks, & Screenshot Matrix (`aibox-core`, `aibox-app`, `cpp/shim/test_helpers.{h,cpp}`)
* **Actionable Contrast Diagnostics (`rust/crates/aibox-core/src/theme/palette.rs`)**: Enhanced `all_23_omarchy_fixtures_satisfy_contrast_contracts` with `assert_contrast` so any failing contrast pair reports the theme fixture name, token pair, actual WCAG ratio, minimum threshold, and hex colors.
* **Strict QML Color Lint (`rust/crates/aibox-app/tests/qml_color_lint.rs`)**: Added an automated integration test scanning all `src/qml/**/*.qml` files in strict mode and failing on hardcoded hex literals (`#rgb`..`#rrggbbaa`), CSS named color literals (allowing `"transparent"`), QML color derivations (`Qt.rgba(`, `Qt.lighter(`, `Qt.darker(`), deprecated `theme.*` raw tokens, and legacy `Style.*` color aliases.
* **Dark-to-Light Live Switch Test (`rust/crates/aibox-app/tests/backend_qobject.rs`)**: Added `test_theme_qobject_live_switch_dark_to_light_emits_changed_once` verifying that overwriting a dark `colors.toml` (`tokyo-night`) with a light fixture (`catppuccin-latte`) emits `Theme::changed` exactly once, flips `isDark` to `false` (`mode == "light"`), and updates semantic surface/text/accent/border tokens.
* **Offscreen Screenshot Matrix (`rust/crates/aibox-app/tests/theme_matrix.rs`, `cpp/shim/test_helpers.{h,cpp}`)**: Added `aibox_test_render_theme_matrix` and an opt-in `#[ignore]` test (`cargo test --test theme_matrix -- --ignored`) that renders `main.png`, `settings.png`, `create.png`, `wipe.png`, `gallery.png`, and `icon.svg` across 7 reference themes (`tokyo-night`, `catppuccin-latte`, `flexoki-light`, `white`, `rose-pine`, `vantablack`, `gruvbox`) into `rust/target/theme-matrix/` with an `index.html` review grid.

### Adaptive Theme M7 — Adaptive Desktop Icon & BrandMark (`pkgbuild/aibox.svg.tpl`, `rust/crates/aibox-core/src/theme/icon.rs`, `rust/crates/aibox-core/src/cli/`, `rust/crates/aibox-app/`, `bin/install`, `bin/uninstall`)
* **SVG Template & Renderer (`pkgbuild/aibox.svg.tpl`, `aibox-core::theme::icon`)**: Added an embedded SVG template (`aibox.svg.tpl`, keeping `pkgbuild/aibox.svg` as the static fallback) and `icon::render(&Palette)` to derive contrast-guarded dark/light backgrounds, shadows, borders, accent-tinted container frame/chevron gradients, and terminal cursor colors while preserving monochrome themes.
* **Atomic Icon Writer & CLI (`aibox theme render-icon [--output PATH] [--theme-dir DIR]`)**: Added `icon::write_icon` with atomic write, identical-content skip to avoid launcher/watcher churn, and `hicolor` icon cache invalidation (`touch` + `gtk-update-icon-cache`), exposed headlessly via `aibox theme render-icon`.
* **Live Window Icon Retinting (`qt_theme.rs`, `qml_helpers.{h,cpp}`)**: Added `aibox_set_window_icon` C++ shim and hooked `Theme::reload()` (in GUI sessions) to re-render the user icon and call `QGuiApplication::setWindowIcon()` on theme changes.
* **Omarchy Theme Hook (`bin/install`, `bin/uninstall`)**: Updated `bin/install` to render the initial icon and install `~/.config/omarchy/hooks/theme-set.d/50-aibox-icon` so launcher icons update on `omarchy-theme-set` even when AIBox is closed; `bin/uninstall` removes the hook cleanly.

### Adaptive Theme M6 — Dialogs, Settings, & Legacy Token Removal (`src/qml/dialogs/`, `src/qml/panels/SettingsPanel.qml`, `src/qml/panels/settings/`, `src/qml/components/Style.qml`, `rust/crates/aibox-app/src/qt_theme.rs`)
* **Settings Migration (`SettingsPanel.qml`, `settings/{Locations,StoragePool,OsCatalog,CreateOptions,Safety}Section.qml`)**: Migrated the Settings sidebar, notice banner, and all five settings sections onto semantic `theme.*` tokens (`surfaceSidebar`, `surfaceInput`, `surfaceCard`, `surfaceSelected`, `surfaceTrack`, `borderSubtle`, `accentSurface`, `accentBorder`, `accentText`, `danger*`, `success*`, `warning*`, `textPrimary`, `textSecondary`, `textMuted`).
* **Dialogs Migration (`dialogs/*.qml`)**: Migrated all modal dialogs (`CreateDialog`, `ChangeReviewDialog`, `EditQuotasDialog`, `CloneDialog`, `MountsDialog`, `PortsDialog`, `EgressDialog`, `HistoryDialog`, `StaleReportDialog`, `BackupsDialog`, `RestoreDialog`, `SecretsDialog`, `HelpOverlay`, `WipeDialog`) to semantic `theme.*` tokens, replacing the remaining `"black"` literal in `WipeDialog` with `theme.onDanger` and preserving non-color status symbols (`+`, `-`, `~`, `✓`) for monochrome themes.
* **Legacy Cleanup (`Style.qml`, `qt_theme.rs`)**: Removed all temporary `Style.*` color aliases from `Style.qml` (leaving only non-color metrics) and removed the deprecated raw color properties (`accentForeground`, `background`, `darkBackground`, `lighterBackground`, `foreground`, `muted`, `green`, `red`, `yellow`) from `Theme` in `qt_theme.rs`.

### Adaptive Theme M5 — Main Panels & Layout (`src/qml/Main.qml`, `src/qml/panels/`)
* **Semantic Token Migration (`Main.qml`, `AppHeader`, `StatusBanner`, `MainContent`, `EnvironmentList`, `EnvironmentInspector`, `NetworkModeCard`, `SnapshotsCard`, `OsCatalogPanel`)**: Migrated the window shell, first-run storage modal, and all main layout panels off legacy `Style.*` color aliases and raw `theme.{foreground,muted,red,green,yellow,darkBackground}` tokens onto contrast-guarded semantic `theme.*` tokens (`surfaceCanvas`, `surfaceSidebar`, `surfaceCard`, `surfaceInput`, `surfaceRaised`, `surfaceSelected`, `surfaceTrack`, `borderSubtle`, `borderStrong`, `textPrimary`, `textSecondary`, `textMuted`, `accentText`, `accentBorder`, `danger*`, `success*`, `warning*`, `scrimStrong`).
* **Elevation & Hierarchy**: Applied `surfaceSidebar` to `EnvironmentList` with hover (`surfaceRaised`/`borderStrong`) and selection (`surfaceSelected`/`accentBorder`) states, `surfaceCanvas` to `AppHeader`, `warning*` semantic tokens to the Offline network mode indicator in `NetworkModeCard`, and `textSecondary`/`textPrimary` hierarchy for headings and primary values (reserving `textMuted` for metadata/hints).

### Adaptive Theme M4 — Reusable Components (`src/qml/components/`, `src/qml/dev/ComponentGallery.qml`)
* **Semantic Token Migration (`src/qml/components/*.qml`)**: Migrated all 15 reusable components (`ActionButton`, `TagBadge`, `ResourceMeterCard`, `ChipInput`, `FlatInput`, `SelectChip`, `SelectCard`, `SegmentedTabs`, `MenuRow`, `CheckRow`, `ModalDialog`, `ModalHeader`, `MeterBar`, `BrandMark`, `SlidersIcon`) off legacy `Style.*` color aliases, raw `theme.{foreground,muted,red,green,yellow,accentForeground,lighterBackground}`, and `Qt.rgba` tinting onto contrast-guarded semantic `theme.*` tokens (`surface*`, `border*`, `text*`, `accent*`, `danger*`, `success*`, `warning*`, `selection*`, `scrim`).
* **Consistent Interaction States**: Replaced hover opacity dimming (`Style.opacityHover`) with `surfaceRaised` / `borderStrong` hover transitions, added keyboard focus borders (`borderFocus`) on `ActionButton`, `FlatInput`, and `ChipInput`, and preserved `Style.opacityDisabled` + `textDisabled` for disabled controls.
* **Component Gallery (`src/qml/dev/ComponentGallery.qml`)**: Added a dev-only gallery loaded via `AIBOX_GALLERY=1` (`rust/crates/aibox-app/cpp/main.cpp`, `src/resources.qrc`) showcasing every reusable component across tones and states.

### Adaptive Theme M3 — `Style.qml` Token Aliases (`src/qml/components/Style.qml`)
* **Semantic Token Aliasing (`src/qml/components/Style.qml`)**: Replaced all 20 hardcoded hex color literals (`surfaceCard`, `surfaceInput`, `surfaceSelected`, `surfaceSidebar`, `surfaceTrack`, `borderSubtle`, `scrim`, `scrimStrong`, `dangerSurface*`, `dangerTint*`, `successSurface*`, `successBorder`, `warning`, `warningSurface*`) with live bindings to the corresponding `theme.*` semantic properties (`theme.surface*`, `theme.borderSubtle`, `theme.scrim*`, `theme.dangerSurface`, `theme.successSurface`, `theme.successBorder`, `theme.warning`, `theme.warningSurface`) so existing `Style.*` call sites adapt immediately to light and dark Omarchy themes ahead of the M4–M6 direct-token migration.

### Adaptive Theme M2 — Qt Theme Bridge & Material Controls (`aibox-app`, `Main.qml`)
* **Semantic `Theme` QObject Properties (`rust/crates/aibox-app/src/qt_theme.rs`)**: Exposed `isDark`, `mode`, and all contrast-guarded semantic `Palette` tokens (`surface*`, `border*`, `text*`, `accent*`/`onAccent`, `danger*`/`onDanger`, `success*`/`onSuccess`, `warning*`/`onWarning`, `info*`/`onInfo`, `scrim`, `scrimStrong`, `selection`, `selectionText`) on `Theme` with `NOTIFY = changed`, while keeping legacy raw color properties marked deprecated through M6.
* **Single-Signal Reload & `theme.name` Watch (`qt_theme.rs`)**: Updated `Theme::reload()` to derive both `ThemeColors` and `Palette`, compare for changes, and emit a single `changed` signal per theme switch. Added `theme.name` and `colors.toml` to `rearm_theme_watches`.
* **Adaptive Material Controls (`src/qml/Main.qml`)**: Bound `ApplicationWindow` `color` to `theme.surfaceCanvas`, `Material.theme` to `theme.isDark ? Material.Dark : Material.Light`, `Material.background` to `theme.surfaceCanvas`, `Material.foreground` to `theme.textPrimary`, and `Material.primary`/`Material.accent` to `theme.accent`.
* **QObject & QML Smoke Tests (`rust/crates/aibox-app/tests/`, `cpp/shim/test_helpers.{h,cpp}`)**: Added `theme_qobject_loads_light_fixture_and_emits_changed_once` in `backend_qobject.rs` and updated `qml_smoke.rs` to verify both dark (`tokyo-night`) and light (`catppuccin-latte`) fixtures with zero QML warnings.

### Adaptive Theme M1 — Core Palette & Color Math (`aibox-core`)
* **Theme Module Split (`rust/crates/aibox-core/src/theme/`)**: Replaced `theme.rs` with `theme/{mod.rs, color.rs, parse.rs, palette.rs}` while preserving all existing `ThemeColors` and `parse_colors` behavior.
* **Color Math & Contrast Guard (`theme/color.rs`)**: Added `Rgb`, `Oklab`, WCAG 2.x `relative_luminance` and `contrast_ratio`, perceptual `to_oklab` / `from_oklab`, `mix`, `oklab_delta_e`, `is_near_gray`, and binary-search `ensure_contrast`.
* **Extended Omarchy Parsing (`theme/parse.rs`)**: Added `read_string_key` for `mode = "light"|"dark"` (with luminance fallback), light-mode defaults (`ThemeColors::default_light`), and parsing for `darker_background`, `light_foreground`, `bright_foreground`, `dark_foreground`, `selection`, `muted`, `orange`, `cyan`, `blue`, and `magenta`.
* **Semantic Palette Derivation (`theme/palette.rs`)**: Added `MixRatios`, `Palette`, `derive_palette`, `load_palette`, `Palette::tokens`, and `Palette::format_dump` for surfaces, borders, text, accent/status families (`accent`, `danger`, `success`, `warning`, `info`), and overlays/selection.
* **CLI Debug Command (`rust/crates/aibox-core/src/cli/`)**: Added `aibox theme dump [path] [--json]` to print derived tokens and WCAG contrast ratios.
* **Fixtures & Tests (`rust/crates/aibox-core/tests/fixtures/themes/`)**: Copied all 23 Omarchy `colors.toml` themes (MIT) and added unit, contrast-contract property, and Tokyo Night OKLab parity tests.

### Rust Migration P19 — Cleanup, Documentation, and Release (`0.3.0`)
* **Architecture Decision**: Core rewritten in Rust (`aibox-core`); QML unchanged (`src/qml/`); `cxx-qt` binding (`aibox-app`) with tiny C++ shims (`rust/crates/aibox-app/cpp/shim/`).
* **Dead Code & FFI Removal (`rust/crates/aibox-ffi`, `src/cpp/`)**: Removed `rust/crates/aibox-ffi`, `src/cpp/rust_bridge.h`, and leftover C++ files (`cli.{h,cpp}`, `theme.{h,cpp}`, `translator.{h,cpp}`). Moved `main.cpp` and `shim/` into `rust/crates/aibox-app/cpp/`, passing `version` and `project_dir` directly from `aibox_app::run_main()` to `aibox_gui_main()`.
* **Version Bump & Docs (`rust/Cargo.toml`, `pkgbuild/PKGBUILD`, `AGENTS.md`, `README.md`, `CATALOG.md`)**: Bumped workspace and `PKGBUILD` version to `0.3.0`. Rewrote `AGENTS.md` for the Rust + `cxx-qt` + Cargo workflow, updated `README.md` build requirements and source layout, and updated `CATALOG.md` file inventory.
* **Verification**: `./bin/install`, `./bin/test`, CLI checks, offscreen GUI grabs, and `git grep` for legacy `qmake`/`.pro`/`src/cpp` references all pass.

### Rust Migration P18 — Rust Test Suite & QML Smoke Test
* **QObject & QML Smoke Integration Tests (`rust/crates/aibox-app/Cargo.toml`, `src/lib.rs`, `src/main.rs`, `src/cpp/shim/test_helpers.{h,cpp}`, `tests/backend_qobject.rs`, `tests/qml_smoke.rs`)**: Exposed `aibox-app` as a library crate (`ensure_cxx_qt_linked`) alongside the `aibox` binary. Ported the remaining 3 `Backend` QObject tests (`primary_action_counts`, `enable_file_access_requires_known_running_environment`, `enter_environment_returns_without_blocking`) to `tests/backend_qobject.rs` and added `tests/qml_smoke.rs` (`qml_smoke_all_dialogs_and_settings_zero_warnings`) to load `qrc:/Main.qml` offscreen in demo mode, open all 5 settings sections and all 17 modal dialogs, and fail on any Qt/QML warning.
* **Legacy Test Harness Removal & Fixture Relocation (`bin/test`, `rust/crates/aibox-core/tests/golden/`, `tests/`, `bin/golden-capture`)**: Moved `tests/golden/*.jsonl` to `rust/crates/aibox-core/tests/golden/` (updating `golden.rs`), removed `tests/aibox_tests.cpp`, `tests/aibox_tests.pro`, `tests/golden_capture/`, and `bin/golden-capture`, and switched `bin/test` to `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --workspace --release --locked` with `QT_QPA_PLATFORM=offscreen`.
* **Verification**: `./bin/install`, `./bin/test` (109 `aibox-core` unit + 1 golden replay + 1 `aibox-app` unit + 3 `backend_qobject` + 1 `qml_smoke` = 115 Rust tests), `aibox --help`, `aibox list`, and offscreen GUI grab (`/tmp/p18-demo.png`) all pass with zero warnings.

### Rust Migration P17c — Task Runner, Async Enter Pipeline, Watchdog/Idle Timers, and C++ `Backend` Removal
* **Task Runner & Enter Flow (`rust/crates/aibox-app/src/qt_backend_task.rs`, `qt_backend.rs`, `aibox-core::util::time::now_epoch_millis`, `src/cpp/shim/qml_helpers.{h,cpp}`)**: Implemented `runShellTask`, `attachExistingTask`, `checkBackgroundTask` (250 ms worker-thread tailing of `task.log` via `CxxQtThread`), `cancelTask`, the non-blocking `enterEnvironment` pipeline (allowlist DNS resolution via `SystemResolver`, prepare script, `incus file push` secret injection, and terminal launch), `evaluateAllWatchdogs`, async `pgrep`-backed `idleAutoStops`, and `notify-send` desktop notifications gated on `aibox_is_gui_session()`. Removed `#![allow(dead_code)]` from `qt_backend.rs`.
* **C++ `Backend` & Unused FFI Removal (`src/cpp/backend*`, `rust/crates/aibox-ffi`)**: Deleted `src/cpp/backend.h`, `src/cpp/backend_helpers.h`, all `src/cpp/backend*.cpp` files, unused callback bridge headers (`command_runner.h`, `host_resolver.h`, `port_predicate.h`), and the 17 unused `aibox-ffi` bridges (retaining only `cli.rs` and `meta.rs` for `main.cpp` until P19). Gated the legacy C++ Qt test harness out of `bin/test` until P18 replaces it.
* **Verification**: `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `./bin/install`, `./bin/test` (109 `aibox-core` + 1 `aibox-app` + 1 golden replay), CLI checks (`aibox --help`, `aibox list`), and offscreen QML grabs (`/tmp/p17c-*.png` for main window, create dialog, settings, OS editor) all pass with zero QML warnings.

### Rust Migration P17b — `Backend` QObject Invokables & Clipboard Shim (Branch `rust-qobject`)
* **Module Invokables (`rust/crates/aibox-app/src/qt_backend_{config,secrets,snapshot,workspace,network,backup,health,provision,sessions}.rs`)**: Ported all 92 non-runner `Q_INVOKABLE` methods on `Backend` to `aibox-core` delegates (including `copyText`, `restoreSnapshot`, `addExtraMount`, `setNetworkMode`, `importEnvironment`, `deleteBackup`, `exportAndWipeEnvironment`, `setEnvironmentIdleStop`, `createEnvironment`, and 2-arg/9-arg `cloneEnvironment` overloads), leaving only `runShellTask`/`cancelTask`/`enterEnvironment` and timers/notifications for P17c.
* **Clipboard & Process Helpers (`src/cpp/shim/clipboard_shim.{h,cpp}`, `qt_backend.rs`)**: Added `aibox_clipboard_set_text` and `aibox_clipboard_image_png` for clipboard text and `pasteImageToWorkspace`, plus `spawn_detached` (`setsid` via `libc`) and `launch_command_in_terminal`.
* **Verification**: `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `./bin/build`, `./bin/test` (109 Rust + 1 golden + 74/74 Qt), and offscreen demo QML grab (`/tmp/p17b-demo.png`) all pass with zero QML warnings.

### Rust Migration P17a — `Backend` QObject State, Properties, and Async Refresh (Branch `rust-qobject`)
* **Variant Converter (`rust/crates/aibox-app/src/variant.rs`)**: Added `serde_json::Value` to `QVariant` / `QList<QVariant>` / `QMap<QString, QVariant>` / `QStringList` converters preserving `i32` and `i64` integers directly without Stage 1 JSON round-tripping.
* **Rust `Backend` QObject (`rust/crates/aibox-app/src/qt_backend.rs`, `build.rs`, `main.rs`, `src/cpp/main.cpp`)**: Implemented the `#[cxx_qt::bridge]` `Backend` QObject wrapping `aibox_core::engine::Engine` and GUI-only state, exposing all 68 `Q_PROPERTY` declarations and 31 signals, `configure` (`ProjectDir`/`WorkspaceDir` + `AIBOX_SELECT_INDEX` + `AIBOX_DEMO_MODE`), `setSelectedIndex`, non-blocking worker-thread `refresh` + 4s polling / 250ms task-watch loop via `CxxQtThread`, and P17b/P17c invokable stubs. Removed `backend*.cpp` from `aibox-app/build.rs` while keeping the C++ facade in `tests/aibox_tests.pro` until P18.
* **Verification**: `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `./bin/build`, `./bin/test` (111 Rust + 74/74 Qt), and offscreen QML grabs (`AIBOX_DEMO_MODE=1` + live mode) all pass with zero QML warnings.

### Rust Migration P16 — `Theme` and `TranslatorState` QObjects in Rust
* **Rust `cxx-qt` QObjects (`rust/crates/aibox-app/src/qt_theme.rs`, `qt_translator.rs`)**: Replaced the C++ `Theme` and `Translator` in the `aibox` binary with `#[cxx_qt::bridge]` QObjects (`Theme` with all 11 QML properties, `changed` signal, `configure` invokable, `notify`-based 80ms debounced filesystem watching, and background `hyprctl` rounding polling; `TranslatorState` with `language` property, `changed` signal, `configure` invokable, and `notify`-based language file watching).
* **Tiny C++ Shims (`src/cpp/shim/translator_shim.{h,cpp}`, `qml_helpers.{h,cpp}`, `src/cpp/main.cpp`)**: Added `TranslatorShim` (`QTranslator` subclass forwarding `isEmpty()` and `translate()` to Rust `TranslatorState`) and `aibox_set_context_property` / `aibox_apply_locale` / `aibox_read_qt_resource` helpers; updated `main.cpp` and `aibox-app/build.rs` while keeping `src/cpp/theme.{h,cpp}` and `translator.{h,cpp}` only in `tests/aibox_tests.pro` until P18.
* **Verification**: `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `./bin/golden-capture --check`, `./bin/install`, `./bin/test` (109 Rust + 74/74 Qt), CLI smoke checks, and offscreen QML checks (`QT_LOGGING_RULES="qt.qml*=true"`) with custom `AIBOX_THEME_DIR` all pass with zero QML warnings.

### Rust Migration P15 — Build Flip to Cargo & `cxx-qt-build`
* **Binary Crate (`rust/crates/aibox-app`)**: Added the `aibox` binary crate driven by `cxx-qt-build` (`0.10.0`), compiling and moc'ing the C++ `QObject` facade (`backend.h`, `theme.h`, `translator.h`, and `src/cpp/*.cpp`), linking Qt6 modules (`Core`, `Gui`, `Qml`, `Quick`, `QuickControls2`, `DBus`, `Network`), and embedding `src/resources.qrc`.
* **Rust Entry Point (`rust/crates/aibox-app/src/main.rs`, `src/cpp/main.cpp`)**: Dispatches `--redact-session` (`aibox_core::session::redact_session_main`) and CLI invocations (`aibox_core::cli::run`) directly in Rust, delegating GUI launches to `extern "C" int aibox_gui_main(int, char**)`.
* **Build & Packaging (`bin/build`, `bin/install`, `pkgbuild/PKGBUILD`, `aibox.pro`, `AGENTS.md`)**: Switched `bin/build`, `bin/install`, and `pkgbuild/PKGBUILD` to build and install `rust/target/release/aibox`, removed `aibox.pro` (keeping `tests/aibox_tests.pro` for the Qt test suite until P18), and updated `AGENTS.md` to reference `rust/crates/aibox-app/build.rs`.

### Rust Migration P14 — Headless CLI in Rust
* **Pure Rust Core (`aibox-core::cli`)**: Ported the hand-written CLI argument parser (`parse.rs`: `parse_command_name`, `parse_args`, `make_usage_error`, `argv_without_program`, `is_cli_invocation`), verbatim usage/help text (`help.rs`: `help_text`), and all subcommand executors (`commands.rs`: `run_list`, `run_check`, `run_enter`, `run_exec`, `run_start`, `run_stop`, `run_snapshot`, `run_snapshots`, `run_restore`, `run_diff`, `require_environment`, `run_pre_steps_for_exec`, `run_script_with_task_lock`) behind a testable `CliIo` trait (`StdCliIo` with `CommandExt::exec` for `enter` and inherited-stdio `Command` for `exec` / `script`) and `QtPrettyFormatter` (`to_json_qt_indented`). Also updated `Engine::new` to initialize `SecretsState` via `set_secrets_store_path("", false)` and `default_secrets_store_path()` to honor `AIBOX_SECRETS_PATH`.
* **FFI & Delegation (`aibox-ffi::cli`)**: Bridged `cli_is_invocation`, `cli_parse_args`, `cli_help_text`, `cli_execute`, and `cli_run` with `CliParsedArgsFfi`; delegated `src/cpp/cli.cpp` (preserving `cli.h` signatures) and updated `src/cpp/main.cpp` to invoke `aibox::ffi::cli_run` directly without constructing a `QCoreApplication`.
* **Verification**: Added 4 Rust CLI unit tests and 72 `"cli"` golden cases (`tests/golden/cli.jsonl`); `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (109 unit tests + golden replay), `./bin/golden-capture --check`, `./bin/install`, `./bin/test` (74/74), `aibox help`/`list`/`check`, and offscreen demo grab all pass.

### Rust Migration P13 — Task Runtime and Headless Engine
* **Pure Rust Core (`aibox-core::task`, `aibox-core::engine`)**: Ported task state file helpers and `task.pid`/`task.meta`/`task.log`/`task.sh` format specification (`task/files.rs`: `read_task_pid`, `write_task_pid`, `read_task_meta`, `write_task_meta`, `clear_task_files`, `admin_fix_command`), task lock management (`task/lock.rs`: `is_pid_alive`, `is_task_lock_active`, `acquire_cli_task_lock`, `release_cli_task_lock`), PTY output progress/stage/exit parsing and attach/check/cancel decisions (`task/progress.rs`: `parse_process_output_chunk`, `feed_process_output_streaming`, `evaluate_attach_existing_task`, `evaluate_check_background_task`, `cancel_running_task`), detached shell task wrapper/launcher (`task/runner.rs`: `prepare_shell_task`), and the synchronous headless `Engine` (`engine.rs`: `Engine::new`, `refresh`, `refresh_sync`, `run_script_blocking`, `selected_environment`).
* **FFI & Delegation (`aibox-ffi::task`)**: Bridged task lock, output parsing, attach/check/cancel decisions, shell task preparation, and `engine_refresh_sync` via `cxx`; delegated `readTaskPid`, `isTaskLockActive`, `acquireCliTaskLock`, `releaseCliTaskLock`, `refreshSync`, `clearTaskFiles`, `handleProcessOutput`, `attachExistingTask`, `checkBackgroundTask`, `cancelTask`, `runShellTask`, and `adminFixCommand` in `src/cpp/backend.cpp` while keeping all `Backend` signatures and Qt timer/process wiring unchanged.
* **Verification**: Added 7 Rust unit tests (including byte-by-byte streaming, attach completion restoration, lock lifecycle, and `Engine` refresh/blocking run) and 12 `"task"` golden cases (`tests/golden/task.jsonl`); `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (105 unit tests + golden replay), `./bin/golden-capture --check`, `./bin/install`, `./bin/test` (74/74), offscreen demo grab, and `aibox list` all pass.

### Rust Migration P12 — Incus List Parsing, OS Catalog, and Demo Data
* **Pure Rust Core (`aibox-core::incus`)**: Ported Incus JSON list parsing, CPU/network counter delta tracking (`CounterState`), `/proc/stat` host core weight distribution, `parse_cpu_count`, `format_percent`, `build_cpu_core_percents`, `environment_by_name`, and `templates` (`list.rs`), OS catalog cache detection (`catalog.rs`: `catalog_for_entries`, `build_os_catalog`), and demo mode state generation (`demo.rs`: `build_demo_state`, `build_demo_state_now`).
* **FFI & Delegation (`aibox-ffi::incus`)**: Bridged `CounterStateHandle`, `parse_incus_list_with_state`, `parse_incus_list_stateless`, `catalog_for_entries_json`, `build_os_catalog_json`, and `build_demo_state_ffi` via `cxx`; delegated `Backend::parseIncusListJson` (`src/cpp/backend_env.cpp`), `Backend::refresh` counter state (`src/cpp/backend.{h,cpp}`), `Backend::catalogForEntries` / `Backend::buildOsCatalog` (`src/cpp/backend.cpp`), and `Backend::loadDemoData` (`src/cpp/backend_demo.cpp`).
* **Verification**: Added 9 Rust unit tests (including a QML key inventory assertion) and 5 `"incus"` golden cases (`tests/golden/incus.jsonl`); `cargo test` (98 unit tests + golden replay), `./bin/golden-capture --check`, `./bin/install`, `./bin/test` (74/74), offscreen demo grab, and `aibox list` all pass.

### Rust Migration P11 — Provisioning, Lifecycle, Enter/Exec, and Resource Scripts
* **Pure Rust Core (`aibox-core::provision`)**: Ported environment creation script and validation/planning (`create.rs`: `create_environment_script`, `prepare_create_environment`), cloning script and validation/planning (`clone.rs`: `clone_environment_script`, `prepare_clone_environment`), lifecycle and enter/exec builders (`lifecycle.rs`: `build_prepare_script`, `start_environment_script`, `stop_environment_script`, `enter_incus_exec_args`, `exec_incus_command_args`, `build_enter_command`, `ai_exec_command_text`, `prepare_toggle_start_stop`, `set_environment_template_script`), and live resource / OS image scripts (`resources.rs`: `update_resources_script`, `validate_and_prepare_update_resources`, `download_os_image_script`, `remove_os_image_script`).
* **FFI & Delegation (`aibox-ffi::provision`)**: Bridged provisioning and lifecycle functions via `cxx` and delegated `src/cpp/backend_provision.cpp` and the script/argv/validation paths in `src/cpp/backend_env.cpp` while keeping the async enter pipeline, `openWorkspaceFolder`, `copyAiCommand`, `runShellTask` dispatch, and all `Backend` signatures unchanged.
* **Verification**: Added 6 Rust unit tests and 25 `"provision"` golden cases (`tests/golden/provision.jsonl`); `cargo test` (89 unit tests + golden replay), `./bin/golden-capture --check`, `./bin/install`, and `./bin/test` (74/74) all pass.

### Rust Migration P10 — Health Checks, Storage Pool, Watchdog, and Idle Auto-Stop
* **Pure Rust Core (`aibox-core::health`)**: Ported host health checks and `CommandRunner` / `SystemRunner` (`checks.rs`: `run_health_checks`, `evaluate_pool_capacity_health`), storage pool parsing, overcommit math, and resize validation/scripts (`pool.rs`: `parse_incus_storage_info`, `sum_root_disk_quotas_bytes`, `evaluate_pool_overcommit_warning`, `pool_warning_for_create`, `pool_warning_for_quota_edit`, `parse_pool_size_spec`, `resize_storage_pool_script`, `validate_storage_pool_resize`, `StoragePoolInfo::to_json`), watchdog hysteresis and batch evaluation (`watchdog.rs`: `evaluate_watchdog`, `evaluate_all_watchdogs`, `watchdog_auto_stop_script`), idle auto-stop overrides, decisions, and policy scripts (`idle.rs`: `parse_idle_stop_override`, `effective_idle_stop_minutes`, `evaluate_idle_decision`, `set_environment_auto_stop_script`, `set_environment_idle_stop_script`), and process/IO limit validators (`limits.rs`: `is_valid_process_limit`, `is_valid_io_limit`).
* **FFI & Delegation (`aibox-ffi::health`, `src/cpp/command_runner.h`)**: Bridged health, pool, watchdog, idle, and limit functions via `cxx` (with `CommandRunnerBridge` adapter) and delegated `src/cpp/backend_health.cpp` and `poolInfoToVariantMap` in `src/cpp/backend_helpers.h` while keeping `evaluateIdleAutoStops` `pgrep`/timer/notification wiring, `setPoolStateForTest`, `runShellTask`, and all `Backend` signatures unchanged.
* **Verification**: Added 10 Rust unit tests and 91 `"health"` golden cases (`tests/golden/health.jsonl`); `cargo test` (83 unit tests + golden replay), `./bin/golden-capture --check`, `./bin/install`, `./bin/test` (74/74), and `aibox check` diff comparison all pass.

### Rust Migration P09 — Backup Export/Import, Stale Report, and Orphan Cleanup
* **Pure Rust Core (`aibox-core::backup`, `aibox-core::health::idle`)**: Ported disk space checks (`has_sufficient_disk_space`, `directory_size_bytes` without following symlinks, `libc::statvfs`-based `available_disk_bytes`, `check_export_space`), archive scanning (`backups_dir_path`, `scan_backup_archives`), export/export-and-wipe/import script builders (`scripts.rs`), staleness classification and stale report generation (`stale.rs`), orphan detection, path-traversal/symlink-escape validation, and delete/admin-delete script builders (`orphans.rs`), plus `parse_active_incus_exec_envs` moved to `health::idle`.
* **FFI & Delegation (`aibox-ffi::backup`)**: Bridged backup, stale, orphan, and idle-exec-env functions via `cxx` and delegated `src/cpp/backend_backup.cpp` while keeping `refreshBackups`, `refreshStaleReport`, `openOrphanFolder`, demo-mode UI state, `runShellTask`, and all `Backend` signatures unchanged.
* **Verification**: Added 7 Rust unit tests (including a stub-`incus` export/import round-trip and hostile orphan-path tests) and 45 `"backup"` golden cases (`tests/golden/backup.jsonl`); `cargo test`, `./bin/golden-capture --check`, `./bin/install`, and `./bin/test` (74/74) all pass.

### Rust Migration P08 — Network Control, Allowlist, Egress Parsing, and Port Forwards
* **Pure Rust Core (`aibox-core::network`)**: Ported allowlist parsing and DNS-rebinding protection (`allowlist.rs`), Incus ACL snippet and network control script builders (`acl.rs`), `journalctl -k` egress log parsing (`egress.rs`), and localhost port-forward validation/scripts (`ports.rs`).
* **FFI & Delegation (`aibox-ffi::network`, `src/cpp/host_resolver.h`)**: Bridged `Backend::HostResolver` via `HostResolverBridge` and delegated `src/cpp/backend_network.cpp`, `aclApplyShellSnippet` in `src/cpp/backend_helpers.h`, and hostname extraction in `src/cpp/backend_env.cpp` (`resolveEnterAllowlistHosts`).
* **Verification**: Added 6 Rust unit tests and 24 `"network"` golden cases (`tests/golden/network.jsonl`); `cargo test`, `./bin/golden-capture --check`, `./bin/install`, and `./bin/test` (74/74) all pass.

### Rust Migration P07b — File Access, Extra Mounts, and Pasted Images
- **Rust core (`rust/crates/aibox-core/src/workspace/{file_access,mounts,paste}.rs`)**: ported localhost SFTP file access state/log/dir path helpers, `/proc/<pid>/cmdline` + `libc::kill` process liveness and stop helpers, `/proc/net/tcp` listening check, port selection (`choose_file_access_port`, `is_host_port_available`), state reconciliation (`apply_file_access_state`), enable validation and script/state builders; extra mount path validation (`validate_extra_mount_path`), device naming (`extra_mount_device_name`), mount/unmount script builders, and `prepare_add_extra_mount`; and pasted image path generation, stale pruning, and container path formatting (`prepare_pasted_image_path`, `prune_stale_pasted_images`, `pasted_image_container_path`), with 4 new Rust unit tests.
- **FFI bridge & C++ facade (`rust/crates/aibox-ffi/src/workspace_workflow.rs`, `src/cpp/port_predicate.h`, `src/cpp/backend_workspace.cpp`, `aibox.pro`)**: bridged file access, extra mount, and paste workflow functions via `cxx` (with `PortPredicate` callback wrapper) and delegated `backend_workspace.cpp` while keeping `QImage`/`QClipboard`, `QProcess::startDetached`, `QDesktopServices`/`xdg-open`, `QTimer` SIGKILL/status-clear timers, and all `Backend` signatures unchanged.
- **Golden fixtures (`tests/golden/workspace.jsonl`, `tests/golden_capture/main.cpp`, `rust/crates/aibox-core/tests/golden.rs`)**: captured 31 workspace workflow golden cases from C++ and enabled `"workspace"` in `golden.rs`.

### Rust Migration P07a — Workspace Change Review (rsync diff)
- **Rust core (`rust/crates/aibox-core/src/workspace/diff.rs`)**: ported workspace diff excludes (`default_diff_excludes`, `parse_diff_excludes`, `build_rsync_exclude_args`), rsync diff argument building (`build_workspace_diff_args`), rsync `--itemize-changes` parsing and workspace enrichment (`parse_rsync_itemize`, `process_workspace_diff_output`), path-traversal guard (`is_safe_relative_path`), binary detection (`is_likely_binary_data`), diff line capping (`cap_diff_lines`), file diff pre-checks/shaping (`prepare_file_diff`, `shape_file_diff_output`), and single-file restore (`restore_single_file`), with 5 new Rust unit tests.
- **FFI bridge & C++ facade (`rust/crates/aibox-ffi/src/workspace_diff.rs`, `src/cpp/backend_workspace.cpp`)**: bridged workspace diff functions via `cxx` and delegated `backend_workspace.cpp` diff/restore logic to Rust while keeping `QProcess` async execution, `m_diffTimeoutTimer`, demo mode, and all `Backend` signatures unchanged.
- **Golden fixtures (`tests/golden/diff.jsonl`, `tests/golden_capture/main.cpp`, `rust/crates/aibox-core/tests/golden.rs`)**: captured 32 diff golden cases from C++ and enabled `"diff"` in `golden.rs`.

### User-Local Passwordless Installer (`./bin/install`)
- **Installer (`bin/install`, `bin/uninstall`, `bin/setup-system`)**: `./bin/install` now checks required packages via `pacman -T` and `cargo` (including `~/.cargo/bin/cargo`) without root, invoking `sudo pacman -S --needed` only if a dependency is actually missing; it then builds `build/aibox` via `./bin/build` and installs the binary, `.desktop` launcher entry (`Exec=$HOME/.local/bin/aibox %f`), SVG icon, and license directly under `~/.local/` (`XDG_BIN_HOME` / `XDG_DATA_HOME`) without `sudo`. Passing `--system` preserves the `makepkg -fsic --noconfirm` Arch package path when needed. `./bin/uninstall` removes the `~/.local/` files as well as any legacy system package, and `./bin/setup-system` now includes all build/runtime packages.
- **Docs (`AGENTS.md`, `README.md`, `CATALOG.md`)**: updated installation workflow documentation to reflect user-local passwordless installation.

### Rust Migration P06 — Snapshot Logic, Btrfs Workspace Snapshots, and Scripts
- **Rust core (`rust/crates/aibox-core/src/snapshot/`)**: ported auto-snapshot naming, timestamp parsing, interval checks, pruning, and decision logic (`auto.rs`), workspace Btrfs paths, `statfs`/inode-256 checks, snapshot listing, orphan detection, and Btrfs/migration snippets (`workspace.rs`), snapshot/wipe script builders (`scripts.rs`), and Incus/`du` snapshot size parsing and cache merging (`mod.rs`), with 7 new Rust unit tests and `libc` `statfs` support.
- **FFI bridge & C++ facade (`rust/crates/aibox-ffi/src/snapshot.rs`, `src/cpp/backend_snapshot.cpp`)**: bridged snapshot functions via `cxx` and delegated `backend_snapshot.cpp` to Rust while keeping QML-invokable `runShellTask`/`QProcess` wrappers (`refreshSelectedSnapshotSizes`, `setEnvironmentAutoSnapshot`, `setEnvironmentWorkspaceSnapshot`, `convertWorkspaceToSubvolume`, `deletePreSubvolBackup`, `createSnapshot`, `restoreSnapshot`, `deleteSnapshot`, `wipeEnvironment`) and all `Backend` signatures unchanged.
- **Golden fixtures (`tests/golden/snapshot.jsonl`, `tests/golden_capture/main.cpp`, `rust/crates/aibox-core/tests/golden.rs`)**: captured 61 snapshot golden cases from C++ and enabled `"snapshot"` in `golden.rs`.

### Rust Migration P05 — Session Recording, Operation History, and `--redact-session`
- **Rust core (`rust/crates/aibox-core/src/session.rs`, `rust/crates/aibox-core/src/history.rs`)**: ported operation history persistence, unread-failure counting, redaction, pruning, and log reading (`history_dir_path`, `history_seen_marker_path`, `load_history_seen_marker`, `recent_failed_history_count`, `load_operation_history`, `record_operation_history`, `read_operation_log`, `clear_operation_history`, `acknowledge_operation_history`), plus session recording path preparation, wrap command builder, ANSI sequence stripping, 200 KB log formatting, 0600-preserving atomic log redaction, per-env/global retention pruning, session scanning, replay command builder, deletion guard, and `--redact-session` entry point (`redact_session_main`), with 6 new Rust unit tests.
- **FFI bridge & C++ facade (`rust/crates/aibox-ffi/src/{history,session}.rs`, `src/cpp/backend_session.cpp`, `src/cpp/main.cpp`)**: bridged history and session functions via `cxx`, delegated `backend_session.cpp` to Rust while keeping `setEnvironmentRecordSessions` (`runShellTask`) and `Backend` signatures unchanged, and delegated `--redact-session` in `main.cpp` directly to `aibox::ffi::redact_session_main` without creating a `QCoreApplication`.
- **Golden fixtures (`tests/golden/session.jsonl`, `tests/golden_capture/main.cpp`, `rust/crates/aibox-core/tests/golden.rs`)**: captured session recording golden cases from C++ and enabled `"session"` in `golden.rs`.

### Rust Migration P04 — Secrets Store, Injection, and Redaction
- **Rust core (`rust/crates/aibox-core/src/secrets/`)**: ported secret name/filename validation (`is_valid_secret_name`, `is_safe_environment_file_name`, `parse_env_secret_names`), path helpers (`default_secrets_store_path`, `instance_secrets_dir_path`, `environment_secrets_store_path`), 0600-enforced atomic store IO (`load_secrets_store`, `load_secret_name_list`, `save_secrets_store`), longest-first redaction (`redact_secrets`, `redact_secrets_bytes`), env-file/temp-file/Incus-push injection (`build_secret_env_file_content`, `create_host_temp_secret_file`, `push_environment_secrets_to_container`), and `SecretsState` + script builders, with 5 unit tests covering all Qt secret test suites.
- **FFI bridge & C++ facade (`rust/crates/aibox-ffi/src/secrets.rs`, `src/cpp/backend_secret.cpp`)**: bridged secrets store, redaction, injection, and state operations via `cxx` and delegated `backend_secret.cpp` to Rust while keeping `QClipboard` copy helpers, signal emission, and all `Backend` signatures unchanged.
- **Golden fixtures (`tests/golden/secrets.jsonl`, `tests/golden_capture/main.cpp`, `rust/crates/aibox-core/tests/golden.rs`)**: captured synthetic secrets golden cases (`sk-test-*`) from C++ and enabled `"secrets"` in `golden.rs`.

### Rust Migration P03 — Theme and Translator Logic
- **Rust core (`rust/crates/aibox-core/src/theme.rs`, `rust/crates/aibox-core/src/i18n.rs`)**: ported Omarchy `colors.toml` parsing (`read_key`, `read_accent`, `parse_colors`, `parse_colors_file`), WCAG contrast calculation (`accent_foreground`), Hyprland rounding JSON parsing (`parse_rounding`), language file reading (`read_language`), CLDR cardinal plural rules (`plural_category`), Omarchy `formatLocale` lookup (`format_locale_name`), and layered translation catalog (`Catalog`), with 7 unit tests and real Omarchy theme fixtures under `rust/crates/aibox-core/tests/fixtures/themes/`.
- **FFI bridge & C++ facade (`rust/crates/aibox-ffi/src/theme.rs`, `rust/crates/aibox-ffi/src/i18n.rs`, `src/cpp/theme.cpp`, `src/cpp/translator.{h,cpp}`)**: bridged theme and i18n functions (with opaque `Box<CatalogHandle>`) via `cxx` and delegated `Theme` and `Translator` logic to Rust while keeping QObject/QTranslator/QFileSystemWatcher/QProcess and all public signatures unchanged.
- **Golden fixtures (`tests/golden/theme.jsonl`, `tests/golden/i18n.jsonl`, `tests/golden_capture/main.cpp`, `rust/crates/aibox-core/tests/golden.rs`)**: captured theme and i18n golden cases from C++ and enabled `"theme"` and `"i18n"` in `golden.rs`.

### Rust Migration P02b — Settings Mutators and Validation
- **Rust core (`rust/crates/aibox-core/src/config/settings.rs`)**: ported all 15 settings mutators and validators (`complete_first_run`, `save_settings`, `save_os_entry`, `remove_os_entry`, `reset_os_catalog`, `save_create_options`, `save_preset`, `reset_create_options`, `save_auto_snapshot_settings`, `save_watchdog_settings`, `stale_day_options`, `save_idle_stop_settings`, `save_stale_settings`, `save_session_recording_settings`, `parse_diff_excludes`, `save_diff_excludes`) with unit tests covering all Qt settings test assertions.
- **FFI bridge & C++ facade (`rust/crates/aibox-ffi/src/settings.rs`, `src/cpp/backend_config.cpp`, `src/cpp/backend.{h,cpp}`)**: bridged settings mutators via `cxx` and delegated all `backend_config.cpp` methods to Rust while keeping signal emission and `Backend` signatures unchanged.
- **Golden fixtures (`tests/golden/settings.jsonl`, `tests/golden_capture/main.cpp`, `rust/crates/aibox-core/tests/golden.rs`)**: captured 44 settings golden cases from C++ and enabled `"settings"` in `golden.rs`.

### Rust Migration P02a — Config Model, Defaults, and Path Resolution
- **Rust core (`rust/crates/aibox-core/src/util/paths.rs`, `rust/crates/aibox-core/src/config/`)**: added path resolution (`default_system_path`, `default_location_file_path`, `configured_system_path`, `save_location_file`, `resolve_project_aibox_dir`, `default_workspace_path`, `default_os_storage_path`, `normalized_path`, `resolve_workspace_path`, `cache_base_path`, `workspace_path_for`, `atomic_write`), defaults (`default_os_entries`, `default_presets`, `default_disk_options`, `default_ram_options`, `default_cpu_options`, `default_diff_excludes`), and `Config` load/save/normalize with `extra` unknown-key preservation.
- **FFI & C++ delegates (`rust/crates/aibox-ffi/src/config.rs`, `src/cpp/backend.cpp`)**: bridged path, default, and config operations via `cxx` and delegated the `Backend` implementations while keeping all signatures unchanged.
- **Verification**: added 50 golden cases in `tests/golden/config.jsonl` and 3 unit tests in `aibox-core`; `cargo fmt`, `cargo clippy`, `cargo test` (15 unit + golden replay), `./bin/golden-capture --check`, `./bin/build`, and `./bin/test` (74 Qt tests) all pass.

### Rust Migration P01 — Shared Utility Helpers in `aibox-core::util`
- **Rust core (`rust/crates/aibox-core/src/util/`)**: implemented `format` (`format_bytes`, `format_byte_rate`, `format_byte_total`), `size` (`parse_size_bytes`, `validated_size_options`, `validated_cpu_options`, `validated_cpu_options_json`), `shell` (`shell_quote`, `shell_array_words`), `csv` (`parse_csv`), `names` (`sanitize_name`), `bytes` (`truncate_log_tail`), `json` (Qt-compatible `Indented`/`Compact` writers), `paths` (`clean_path`, `home_dir`, `xdg_config_dir`, `expand_tilde`), and `time` (`Qt::ISODate` formatters), with 12 unit tests and 95 golden replay cases in `tests/golden/util.jsonl`.
- **FFI & C++ delegates (`rust/crates/aibox-ffi/src/util.rs`, `src/cpp/backend_helpers.h`, `src/cpp/backend.cpp`, `src/cpp/backend_session.cpp`)**: bridged all `util` helpers via `cxx` and delegated the C++ implementations while keeping all signatures unchanged.
- **Verification**: `cargo fmt`, `cargo clippy`, `cargo test`, `./bin/golden-capture --check`, `./bin/build`, and `./bin/test` (74 Qt tests) all pass.

### Rust Migration P00 — Toolchain, Cargo Workspace, FFI Skeleton, and Golden Harness
- **Rust workspace (`rust/Cargo.toml`, `rust/crates/aibox-core/`, `rust/crates/aibox-ffi/`)**: added the `aibox-core` pure Rust library and `aibox-ffi` `cxx` staticlib bridge with `aibox::ffi::core_version()` returning `0.2.0`.
- **C++/qmake integration (`src/cpp/rust_bridge.h`, `aibox.pro`, `tests/aibox_tests.pro`, `src/cpp/main.cpp`)**: added Qt <-> `cxx` conversion helpers, linked `libaibox_ffi.a`, and wired `core_version()` into `app.setApplicationVersion(...)`.
- **Build, test, and golden harness (`bin/build`, `bin/test`, `bin/golden-capture`, `tests/golden_capture/`, `tests/golden/util.jsonl`, `pkgbuild/PKGBUILD`)**: integrated `cargo build`/`cargo test` into the build and test scripts, added `cargo` to `PKGBUILD` `makedepends`, and seeded the golden fixture harness with `util` cases.

### Iteration 56 — CPU Card Expands Per-Core Usage
- **CPU resource card (`src/qml/components/ResourceMeterCard.qml`, `src/qml/panels/EnvironmentInspector.qml`)**: the card gained optional interaction: a pointer cursor, an accent border on hover, a click signal, a tooltip, and an expansion state. The CPU Usage card uses these so a click expands the card in place and a second click collapses it. The card keeps its grid column and only grows taller. The Disk and Memory cards bind their preferred height to the CPU card height, so all three match without stretching to the window height. No new modal is added.
- **Two-column core grid (`src/qml/panels/EnvironmentInspector.qml`)**: the card stays in its own column and only grows in height. The expanded area renders one cell per allocated core in two columns, matching `cpuCores` and the `cpuCorePercents` samples from the backend. Each cell shows `Core N`, its percentage, and a usage bar; values at or above 85% use the alert colour. The card height follows the row count, and stopped environments show zero load.
- **Selection handling (`src/qml/panels/EnvironmentInspector.qml`)**: the expanded environment name is stored instead of a plain flag, so changing the selected environment collapses the card automatically while periodic refreshes leave it open.
- **Localization (`src/i18n/en.json`, `src/i18n/da.json`)**: added the expand and collapse tooltips and the core label.
- **Docs (`README.md`)**: the resource card bullet now mentions the expanding per-core CPU view.
- **Tests (`tests/aibox_tests.cpp`)**: no behaviour change; `./bin/test` passes 74 tests.

### Iteration 55 — Wipe OS Moves into More Actions
- **Environment inspector (`src/qml/panels/EnvironmentInspector.qml`)**: removed the standalone red **Wipe OS** button. The secondary action bar now holds only **Start OS** / **Stop OS** and **More Actions**.
- **Environment Actions dialog (`src/qml/dialogs/EnvironmentActionsDialog.qml`)**: added **Wipe OS** as the last row, marked `danger` so the label, hover surface, and hover border use `theme.red`. Opening it closes the menu and shows the existing wipe confirmation with `deleteWorkspaceOnWipe` reset to false.
- **Localization (`src/i18n/en.json`, `src/i18n/da.json`)**: added the new row detail string.
- **Docs (`README.md`)**: updated the **More Actions** summary.
- **Tests (`tests/aibox_tests.cpp`)**: no behaviour change expected; the wipe flow and the `Delete` shortcut are unchanged.

### Iteration 54 — Remove Provisioning Presets and Duplicate Cancel
- **Create dialog (`src/qml/dialogs/CreateDialog.qml`)**: removed the optional **Provisioning Presets** block and its `Cancel` footer button. The header **Close** action remains the single dismissal control, and **Create Environment** stays right-aligned.
- **Backend (`src/cpp/backend.h`, `backend_env.cpp`, `backend_provision.cpp`, `backend.cpp`, `backend_config.cpp`)**: removed `provisionBundles`, `provisionDirPath`, all provisioning bundle helpers, the create-time provisioning snippet, and the script seeding calls. Create environments no longer run package installation presets after boot.
- **Window state (`src/qml/Main.qml`)**: removed `newEnvProvisionBundles` and `toggleProvisionBundle`.
- **Tests (`tests/aibox_tests.cpp`)**: removed the provisioning bundle test and adjusted the create-script signature. `./bin/test` now runs 74 tests.
- **Docs and translations**: removed provisioning references from `README.md` and unused provisioning strings from `src/i18n/da.json`.

### Iteration 53 — Create Options Layout Polish
- **Create Options (`src/qml/panels/settings/CreateOptionsSection.qml`)**: preset titles and subtitles now share one card instead of three nested boxes. A compact column header aligns **Preset**, **Title**, and **Subtitle**, each preset id renders as an accent badge, rows are separated by hairlines, and every **Save** button aligns with its inputs. The quota card keeps the 120 px label column and moves **Save Options** to the page footer beside **Restore Create Defaults**, so the primary action matches the other settings sections.
- **Chip input (`src/qml/components/ChipInput.qml`)**: values render as `amount + unit + ×` instead of a leading `×`, with slightly taller chips and a trailing remove mark. The capacity limit now appears as a quiet `TagBadge` (for example `4 / 4`) instead of a floating text fragment at the end of the field.
- **Translations (`src/i18n/en.json`, `src/i18n/da.json`)**: added the `Preset` / `Forudindstilling` column label.
- **Tests (`tests/aibox_tests.cpp`)**: no behaviour change; `./bin/test` passes 75 tests.

### Iteration 52 — Backup Export Row Alignment
- **Environment Backups (`src/qml/dialogs/BackupsDialog.qml`)**: the **Include Workspace** toggle and **Export Backup** button now sit on the right edge of the export card. A Qt Quick Layouts quirk kept the row at its implicit width when the fill-width child was a nested `ColumnLayout`, so the row stayed narrow and the buttons floated mid-card. An explicit fill-width spacer, plus elision on the title and size lines, keeps the actions right-aligned at every window size and stops long environment names from pushing them.
- **Tests (`tests/aibox_tests.cpp`)**: no behaviour change; `./bin/test` passes 75 tests.

### Iteration 51 — Global and Environment Secrets
- **Secrets model (`src/cpp/backend_secret.cpp`, `src/cpp/backend.h`)**: `~/.config/aibox/secrets.json` remains the global store managed from **Tools → Secrets**. Each environment can now own secrets in `~/.config/aibox/secrets.d/<env>.json` with the same `0700` directory and `0600` file rules. Global secrets carry a `hiddenInInstances` name list: checked secrets appear in environment dialogs, unchecked ones stay hidden and are not injected. Hiding a global secret also removes its name from every environment's `user.aibox.secrets` binding in one task, so re-enabling never restores a stale invisible assignment. Global names cannot be redefined or deleted from an environment view. New `Backend` members: `reloadEnvironmentSecrets`, `saveEnvironmentSecret`, `deleteEnvironmentSecret`, `copyEnvironmentSecret`, `setEnvironmentLocalSecretEnabled`, `setGlobalSecretInstanceVisible`, and the `environmentSecrets` model.
- **Injection & redaction**: `resolveEnvironmentSecrets` injects assigned global secrets plus every enabled environment-only secret for that environment. Environment-only values join the redaction set, so they never appear in task logs, operation history, or recorded sessions. Wipe and export-and-wipe scripts delete the environment's secret file so secrets do not outlive the environment. Environment-only secrets store a `disabled` name list in the same `0600` file; the checkbox toggles it.
- **UI (`src/qml/dialogs/SecretsDialog.qml`, `EnvironmentActionsDialog.qml`, `EnvironmentInspector.qml`, `ToolsDialog.qml`, `Main.qml`)**: the inspector **Secrets (N)** button moved into **More Actions** as the first row. **Tools → Secrets** manages global secrets and each secret's checkbox controls whether it appears in environment dialogs. **More Actions → Secrets** opens a compact dialog with two sections. There the global checkbox controls injection into that environment, independent of the Tools visibility checkbox; environment-only rows toggle their own enabled state. Environment-only secrets can be deleted; global secrets cannot. The dialog height follows the row count instead of leaving empty space.
- **Localization (`src/i18n/en.json`, `src/i18n/da.json`)**: added strings for the new scopes, read-only notes, and validation errors.
- **Tests (`tests/aibox_tests.cpp`)**: added `environmentSecretsAreScopedAndGlobalSecretsAreReadOnly` (store path and permissions, scopes, global-name rejection, delete protection, resolve isolation, redaction, visibility independence, local enable toggle, wipe cleanup) and `hidingGlobalSecretClearsEnvironmentAssignmentsInDemo` (hiding a global secret removes the environment binding). `./bin/test` passes 75 tests.

### Iteration 50 — Privacy Cleanup of Local Username
- **Repository Metadata (`LICENSE`)**: the copyright holder is now `Muhamad Syaifullah`, replacing the local device username.
- **Documentation (`CATALOG.md`)**: absolute `file:///home/<user>/...` links became repository-relative links, so the file no longer records the author's home directory. The host setup table now refers to the current user instead of a fixed username.
- **Health Checks (`src/cpp/backend_health.cpp`)**: the implicit user fallback no longer hardcodes a personal account. It reads `USER`, then `LOGNAME`, and otherwise reports that the account could not be inspected.
- **Demo Data & Tests (`src/cpp/backend_demo.cpp`, `tests/aibox_tests.cpp`)**: example home paths and fixture users are now generic (`/home/user`, `testuser`). `./bin/test` passes all 73 tests.

### Iteration 49 — UI Simplification & Consolidation
- **Header (`src/qml/panels/AppHeader.qml`, `src/qml/dialogs/ToolsDialog.qml`)**: Secrets, Backups, History, and Stale & Orphans now open from one **Tools** button. The direct `A`, `B`, `H`, and `U` shortcuts keep working, and the help overlay still lists them. The long product subtitle was removed and the second tab is now **OS Images**.
- **Environment inspector (`src/qml/panels/EnvironmentInspector.qml`, `src/qml/dialogs/EnvironmentActionsDialog.qml`)**: only Enter, Start/Stop, More Actions, and Wipe stay on the panel. Clone, Quotas & Limits, Template, Copy AI Command, and Paste Image moved into the shared **Environment Actions** dialog. The RUNNING badge was removed from the title because the list already shows state. When secrets are assigned, a **Secrets (N)** action button sits in the action row next to More Actions; the title shows only the environment name and OS line.
- **Snapshots (`src/qml/panels/SnapshotsCard.qml`)**: the auto-snapshot, workspace snapshot, and critical auto-stop controls are compact chips. Per-environment idle auto-stop and session recording rows were removed; both policies remain in **Settings → Safety** and still honour existing `user.aibox.*` overrides.
- **Removed surfaces**: the Network Monitor card and its 10-second in-container probe, the per-core CPU grid, and the Virtual Display / Run Headless row. The environment list now shows the IP address, and the network card shows a click-to-copy address without a separate monitor card.
- **Standardization**: added `Style.menuRowHeight`, `Style.listRowHeight`, `Style.rowHeight`, and `Style.iconTile`; introduced `MenuRow` for action lists; all modal width expressions follow `Math.min(parent.width - Style.modalGutter, width)`; History uses `ModalHeader`; the settings sidebar dropped step numbers and the config-file footer. The storage-pool footer now has a labelled usage meter and separate used/allocated values, and the environment-row hover tooltip was removed. The base window fits at its 1080×720 default and at 960 px without horizontal clipping.
- **Backend cleanup**: removed the unused network probe (`networkStats`, `refreshNetworkStats`, `parseNetworkProbeOutput`, `networkStatsFromEnvironment`, probe timer/process fields) and `runHeadlessCommand`. Incus counter parsing and rates stay because the environment model still exposes them. `./bin/test` passes 73 tests.

### Iteration 48 — QML Design Tokens & Shared Components
- **Design Tokens (`src/qml/components/Style.qml`)**: every application-owned visual value now lives in one QML singleton: surface and feedback colours, spacing, type scale, font families, corner radii, control heights, badge metrics and modal metrics. The singleton is registered through `src/qml/components/qmldir`; no QML file outside `Style.qml` hardcodes a hex colour, font family or font size. Live Omarchy colours (`theme.*`) and Hyprland rounding still come from the C++ `Theme` object.
- **Shared Modal Shell (`src/qml/components/ModalDialog.qml`)**: all 16 overlays (14 dialogs, Settings, first-run) now declare only their content, card width/height, border tone and dismiss handler. Backdrop, centred card, padding, spacing and click-through blocking live in the shell, so dialogs can no longer drift apart.
- **Shared Controls**: added `TagBadge` (status/count chips), `MeterBar` (progress and usage tracks), `SegmentedTabs` (main tabs and history switcher), `SelectChip`, `SelectCard`, `CheckRow`, `CopyValueRow`, `ResourceMeterCard` and `BrandMark`. `ActionButton`, `FlatInput`, `ModalHeader`, `SlidersIcon` and `ChipInput` now read tokens directly and no longer require an `app` object just to reach colours or fonts.
- **Panel Split**: `MainContent.qml` drops from 2,116 to 118 lines and is composed of `EnvironmentList`, `EnvironmentInspector`, `NetworkModeCard`, `NetworkMonitorCard`, `SnapshotsCard` and `OsCatalogPanel`. `SettingsPanel.qml` drops from 1,831 to 300 lines, with the five sections moved to `panels/settings/`. Every QML file is now below 1,100 lines.
- **No Visual Change**: offscreen screenshots of the main window, create dialog, all five settings sections, the OS editor, first run, a template inspector and all 14 dialogs (Create through its preview flag) were compared before and after. Output is pixel-identical; only transient one-frame capture noise was observed. `./bin/test` passes all 76 tests.

### Iteration 47 — Maintainable Project Structure
- **Repository Layout (`src/`)**: sources are grouped by language: `src/cpp/` for C++ and `src/qml/` for QML, with `src/i18n/` and `src/resources.qrc` unchanged. Build files, tests, and archives were updated to the new paths.
- **Backend Modules (`src/cpp/backend*.cpp`)**: the 9,214-line `backend.cpp` is split into feature translation units (`backend_env`, `backend_config`, `backend_network`, `backend_snapshot`, `backend_secret`, `backend_session`, `backend_backup`, `backend_workspace`, `backend_health`, `backend_provision`, `backend_demo`) that all implement the single `Backend` class. Shared helpers live in `backend_helpers.h`; the largest module is now ~1,370 lines.
- **QML Split (`src/qml/`)**: `Main.qml` drops from 9,607 to about 1,120 lines. Reusable controls moved to `src/qml/components/`, modal overlays to `src/qml/dialogs/`, and layout sections to `src/qml/panels/`. Every dialog and window state was verified pixel-identical against the previous build.
- **Contributor Guide (`AGENTS.md`)**: rewritten in English without emoji, with the repository map, mandatory `./bin/install` and `./bin/test` workflow, code and Omarchy style rules, and file-size guidance.
- **Docs (`CATALOG.md`, `docs/ROADMAP.md`)**: file inventory and path references updated to the new layout.

### Iteration 46 — Copy Port URL from Port Forwarding
- **Copy Action (`src/qml/Main.qml`)**: each port row in the **Port Forwarding** modal now has a **Copy** button next to **Open** and **Remove**. It puts `http://127.0.0.1:<hostPort>` on the host clipboard and shows a status message, so the forwarded address can be pasted into a browser, chat, or config file.
- **Localization (`src/i18n/da.json`)**: added the Danish `Port URL` label.

### Iteration 45 — Clipboard Image Paste Bridge
- **Paste Image (`src/cpp/backend.{h,cpp}`, `src/qml/Main.qml`)**: new **Paste Image** action (`I`) reads an image from the host clipboard, writes it as PNG to `<workspace>/.aibox-paste/paste-<timestamp>.png`, and puts the container path (for example `/home/workspace/.aibox-paste/paste-20261117-105500.png`) on the clipboard. The shared workspace mount delivers the file to the environment, so an agent inside a headless container can read the image after a normal terminal paste.
- **Cleanup & Diff Noise**: pasted images older than seven days are pruned automatically, and `.aibox-paste/` is added to the default diff excludes so change review stays clean.
- **Errors**: empty clipboard, unknown environment, or failed writes report a clear message instead of failing silently.
- **Tests (`tests/aibox_tests.cpp`)**: new `pastedImagesAreStoredInsideWorkspaceAndExcludedFromDiff` test covers PNG storage, unique file names, empty-image rejection, and the new diff exclude.

### Iteration 44 — Non-Blocking Enter OS Terminal
- **Enter Terminal (`src/cpp/backend.{h,cpp}`)**: `Backend::enterEnvironment()` no longer runs the preparation script, secret injection, or allowlist DNS lookups on the GUI thread. Preparation now runs as an asynchronous `QProcess`, hostnames resolve through `QHostInfo::lookupHost`, and the terminal opens from the finished callbacks. The window stays responsive, so the desktop no longer shows the "Application Not Responding" dialog while a stopped container starts or an auto-snapshot runs.
- **Status Bar (`src/cpp/backend.cpp`)**: entering an environment shows `Preparing environment '<name>'...` until the terminal launches, then clears the message or shows the failure reason.
- **Tests (`tests/aibox_tests.cpp`)**: new `enterEnvironmentReturnsWithoutBlocking` test uses a stub `incus` that delays `info`, asserts the call returns in under 500 ms, and verifies the terminal command launches afterwards.

### Iteration 43 — Create Dialog Fits Its Content
- **Create Dialog (`src/qml/Main.qml`)**: the modal height now follows the form instead of a fixed height, so template mode no longer leaves a large empty area below the quota rows. The form still scrolls when fresh-OS creation needs more room than the window offers.
- **Quota Chips (`src/qml/Main.qml`)**: disk, RAM, CPU, process, and I/O chips now share the row width equally and elide long labels, so narrow windows no longer squeeze or clip the chip rows.

### Iteration 42 — Inline Secret Injection Checkbox
- **Secrets Manager (`src/qml/Main.qml`)**: the separate **Inject into '<environment>'** chip panel is removed. Every row in **Global Secrets** now carries the injection checkbox on its left, so one list shows both the stored secrets and their per-environment bindings.
- **Behavior**: the checkbox only appears when an environment is selected; toggling it still calls `Backend::setEnvironmentSecrets`. The "No secrets configured yet" hint stays at the top of the empty list.

### Iteration 41 — Copy Admin Fix Command from Error Banner
- **Error Banner (`src/qml/Main.qml`)**: when an operation fails because container-written files are root-owned, the banner now shows **Copy Fix** next to **Dismiss** and copies the exact `sudo` command to the clipboard.
- **Command Extraction (`src/cpp/backend.{h,cpp}`)**: new `Backend::adminFixCommand()` reads the `Fix (admin): ...` or `run: sudo ...` line from an error message, so paths with spaces stay quoted.
- **Tests (`tests/aibox_tests.cpp`)**: `orphanDeleteScriptsHandleRootOwnedFiles` now covers fix extraction for both message forms and ignores errors without a command.

### Iteration 40 — Custom Shared Location & Workspace-Based Extra Mounts
- **Create Dialog (`src/qml/Main.qml`)**: the form is now scrollable and includes an optional **Shared Folder Location** field with **Browse...** and **Default** actions. The default stays `<workspaceBase>/<name>`; a custom parent produces `<custom>/<name>` and the **Shared at** preview follows the selection.
- **Workspace Resolution (`src/cpp/backend.{h,cpp}`)**: new `Backend::resolveWorkspacePath()` expands `~`, requires an absolute custom parent, rejects system paths, the filesystem root, and the home directory itself, and appends the environment name. `createEnvironment()` and `cloneEnvironment()` accept the optional custom parent.
- **Extra Mounts (`src/cpp/backend.{h,cpp}`, `src/qml/Main.qml`)**: extra host folders from the **Extra Mounts** dialog now mount under `/home/workspace/<name>` instead of `/mnt/<name>`, matching the primary workspace mount.
- **Tests (`tests/aibox_tests.cpp`)**: added `resolveWorkspacePathHandlesCustomBaseAndValidation` for default and custom parents plus invalid-path rejection.
- **Docs & Localization (`README.md`, `docs/ROADMAP.md`, `src/i18n/da.json`)**: updated mount paths, custom shared-location behavior, and Danish UI strings.

### Iteration 39 — Copy Secret Action
- **Secrets Manager (`src/qml/Main.qml`)**: the per-secret **Edit** button is replaced by **Copy**, which copies the stored value to the clipboard. Values can still be changed through the **Add or Update Secret** form at the bottom of the dialog.
- **Clipboard Copy (`src/cpp/backend.{h,cpp}`)**: new `Backend::copySecret` copies a named secret without loading it into the form and reports only the secret name, so the value never appears in the status bar or an operation log.
- **Localization (`src/i18n/da.json`)**: added Danish entries for the copy action and its failure message.

### Iteration 38 — Restore Sidebar Storage Pool Card
- **Storage Pool Card (`src/qml/Main.qml`)**: the environment list again shows the storage pool card (**Pool** usage, **Alloc** quota total, and **Manage**) above the **Stale & Orphans Report** button. Clicking the card opens Configuration → Storage Pool.
- **Disk Usage Card (`src/qml/Main.qml`)**: the pool percentage line is removed from the selected environment **Disk Usage** monitor card, which now reports only that environment's usage against its quota.

### Iteration 37 — Orphan Delete Workspace Cleanup Fix
- **Root-Owned Files (`src/cpp/backend.cpp`)**: orphan workspace cleanup no longer fails silently on container-written files. The delete script reports the exact blocked reason, retries through `sudo -n` when passwordless admin rights exist, and prints a copyable `Fix (admin): sudo rm -rf <path>` hint otherwise.
- **Delete as Admin (`src/cpp/backend.{h,cpp}`, `src/qml/Main.qml`)**: new **Delete as admin** action in the stale-report confirmation bar writes a root cleanup script to `<state>/admin-delete.sh` and opens it through `sudo` in the default terminal, so nested Btrfs subvolumes and root-owned directories are removed with the password prompt visible.
- **ACL Defaults (`src/cpp/backend.cpp`)**: create, clone, convert, and import workspace ACL steps now set the default ACL mask to `rwx`, so directories written later inside the container stay deletable by the host user.
- **Tests (`tests/aibox_tests.cpp`)**: added `orphanDeleteScriptsHandleRootOwnedFiles` covering the sudo fallback, admin hint, nested-subvolume cleanup script, and path validation (73 tests total).
- **Localization (`src/i18n/da.json`)**: added Danish entries for the admin delete action and error messages.

### Iteration 36 — Shared Workspace Mounted in `/home/workspace`
- **Container Mount Path (`src/cpp/backend.{h,cpp}`)**: new environments, clones, and imported backups mount their shared host workspace at `/home/workspace` instead of `/workspace`; the path is exposed through the read-only `containerWorkspacePath` property.
- **Legacy Migration (`src/cpp/backend.cpp`)**: `prepareEnvironmentScript()` and `startEnvironmentScript()` call `workspaceMountMigrationScript()`, which moves an existing `workspace` device from `/workspace` to `/home/workspace` while the environment is stopped; running environments keep their current mount until the next start.
- **Live Path Resolution (`src/cpp/backend.cpp`)**: `enterIncusExecArgs()`, `execIncusCommandArgs()`, the GUI Enter command, and the headless GUI runner read the actual workspace mount path (`incus config device get <env> workspace path`) so both legacy and migrated environments open in the correct directory. `aibox exec` command hints use the parsed `workspaceMountPath`.
- **UI & CLI (`src/qml/Main.qml`, `src/cpp/cli.cpp`)**: workspace labels, snapshot restore descriptions, shortcut help, and CLI usage now render `/home/workspace` from `backend.containerWorkspacePath`.
- **Tests (`tests/aibox_tests.cpp`)**: added `workspaceMountUsesHomePathAndMigratesLegacyEnvironments` covering the create script, migration snippet, and pre-step scripts; updated clone and CLI `--cwd` expectations.
- **Localization (`src/i18n/da.json`)**: updated Danish keys to the `%1` placeholder form.

### Iteration 35 — Root Files in Your File Manager
- **Root File Access (`src/qml/Main.qml`)**: added a **Root Files** row directly below **Shared Workspace** in the environment inspector. When the environment is running, **Enable** starts a localhost-only SFTP listener and the row shows a copyable `sftp://root@127.0.0.1:<port>/` URL (click the URL or use **Open** to hand it to the default file manager). **Disable** stops the listener and restores the off state. The row follows the Omarchy accent/radius conventions, warns on hover about the passwordless localhost-only access, and disables the toggle for templates, stopped environments, and busy tasks.
- **Backend (`src/cpp/backend.{h,cpp}`)**:
  - Added `enableFileAccess()`, `disableFileAccess()`, and `openFileAccess()` plus `fileAccessUrl()` and `chooseFileAccessPort()` helpers. The listener runs detached via `incus file mount <env> --listen 127.0.0.1:<port> --no-auth` (ports 2222–2299, skipping reserved/occupied ports), waits for the socket to reach `LISTEN` before reporting success, and surfaces bind errors from the listener log.
  - Persists `pid`/`port` in `~/aibox/state/file-access/<env>.json`, verifies the process cmdline on every refresh to avoid PID reuse, restores active listeners after an AIBox restart, prunes dead/renamed state, and stops the listener when the environment stops, is wiped, or is export-wiped.
- **Tests (`tests/aibox_tests.cpp`)**: added URL/port-selection coverage and file-access validation for unknown environments (71 tests total).
- **Localization (`src/i18n/da.json`)**: added Danish entries for the Root Files row, security tooltip, SFTP labels, and backend validation/error messages.

### Iteration 34 — Refreshed Environment Rows & Leaner Sidebar
- **Environment Sidebar (`src/qml/Main.qml`)**:
  - Rebuilt the environment row (`68px`) around two aligned lines: the status dot, name, `TEMPLATE`/`RUNNING` badges on top, then a labelled metric line — `DISK`, `RAM`, and the network endpoint — that no longer clips. The left pane grew from `330px` to `352px` to give names and quotas room.
  - Added `compactSize()` / `sizePair()` helpers that collapse `8.2 GiB` / `100GiB` into `8.2G` / `100G`, so disk, memory and IP fit side by side; the IP right-aligns and elides as a single flexible field instead of overlapping its neighbours.
  - Disk usage now warns in colour: the value turns yellow at ≥75% of the quota and red at ≥90% (computed from `diskUsageBytes` / `diskQuotaBytes`). Hovering a row shows a tooltip with the full, un-compacted disk/memory/network values.
  - `NET OFF` stays red and allowlisted rows keep the `ACL · <ip>` prefix; the network label is now a single `networkLabel` property shared by the row and its tooltip.
  - Removed the redundant `POOL` card from the sidebar bottom (live pool usage, allocation and growth stay in **Settings → Storage Pool**) along with the now-unused `openPoolSettings()` helper.
- **Localization (`src/i18n/da.json`)**: added Danish entries for `DISK`, `RAM`, and the row tooltip string.

### Iteration 33 — Aligned Network Monitor & Click-to-Copy IP
- **Resource Cards (`src/qml/Main.qml`)**:
  - Removed the **IPv4 Address** card (with its `ONLINE`/`OFFLINE`/`ALLOWLIST` badge and connection bar) from the task-manager row; the row is now three equal cards (**Disk Usage**, **Memory Usage**, **CPU Usage**, `columns: 3`), so IP status lives only in the **Network Monitor**.
- **Network Monitor (`src/qml/Main.qml`)**:
  - Replaced the `GridLayout` of eight alternating rows with two equal-width `ColumnLayout`s (`Layout.fillWidth` + `Layout.preferredWidth: 0`) so labels and values line up in true left/right pairs: *Ping / Packet Loss*, *Receiving / Sending*, *Downloaded / Uploaded*, *IP Address / Gateway*.
  - **IP Address** and **Gateway** values are now clickable: a `HoverHandler` shows a pointing cursor plus a monochrome copy glyph (two overlapping outlined rectangles, no emoji) and a `TapHandler` copies the value via the new `backend.copyText(text, label)`; the value momentarily renders `Copied` in the theme accent and the status bar reports the copy.
- **Backend (`src/cpp/backend.{h,cpp}`)**:
  - Added `Q_INVOKABLE bool copyText(const QString &text, const QString &label = QString())` — a generic clipboard helper that ignores empty/`—` values, sets `statusMessage` (auto-cleared after 3 s) and returns whether the copy happened.

### Iteration 32 — Chip-Based Create Option Editor
- **Create Options Editor (`src/qml/Main.qml`)**:
  - Replaced the three comma-separated text fields (disk quota, RAM limit, CPU cores) with a new `ChipInput` component: type a number and press `Enter` to store it as a removable chip, capped at **4 choices** per list.
  - Units are appended automatically (`20` → `20GiB`); RAM and disk share the `GiB` suffix, while CPU cores stay plain numbers. Each chip renders the amount in mono text with a muted unit, and `×` removes it.
  - Edits persist immediately through `backend.saveCreateOptions()` (chips are sorted numerically and duplicates, non-numeric input, zero, and over-limit values are rejected with the existing config notice). At least one choice per list is kept, and `Backspace` on an empty input removes the last chip.
  - Added per-field placeholders (e.g. *Type a size, then Enter*) plus a `4 / 4` counter that turns into *Limit 4 — remove one first* on hover, and compacted the card so `Save Options` stays visible at the default window height.
  - Added `diskChoiceDraft`, `ramChoiceDraft`, `cpuChoiceDraft`, `syncCreateChoiceDrafts()`, `sortedChoices()`, and `updateCreateChoices()` helpers; `Restore Create Defaults` now re-syncs the chip drafts.
- **Localization (`src/i18n/da.json`)**:
  - Added Danish entries for the new chip placeholders, limit counter, and validation notices.

### Iteration 33 — Remove Unused Roadmap Docs
- **Documentation (`docs/ROADMAP.md`, `AGENTS.md`)**:
  - Deleted the `docs/` directory because the roadmap duplicate is no longer used; shipped and planned work lives in `CATALOG.md`.
  - Removed the roadmap maintenance step and the `docs/` entry from the repository layout in `AGENTS.md`.

### Iteration 32 — Remove Visible Shortcut Hint Badges
- **Action Controls (`src/qml/components/ActionButton.qml`, `src/qml/components/MenuRow.qml`, `src/qml/components/ModalHeader.qml`)**:
  - Removed the `shortcutHint` property and the badge chip that rendered the key letter next to labels, so buttons and menu rows no longer show `N`, `R`, `Esc`, `Space`, `Del`, and the other key hints.
  - Keyboard shortcuts keep working unchanged; the `?` help overlay and the README shortcut table remain the single place that documents them.

### Iteration 31 — Compact Environment List Card
- **Environment Sidebar (`src/qml/Main.qml`)**:
  - Removed the secondary description line (`<osName> · <preset>`) under the environment name; everything else (status dot, name, `TEMPLATE`/`RUNNING` badges, disk/memory quotas, and network/IP) is unchanged.
  - Card height reduced from `78px` to `64px` and column spacing raised to `8px` so the title row and quota row stay balanced now that only two lines remain, showing more environments per screen.

### Iteration 30 — Split Locations & Storage Pool Settings
- **Configuration Sidebar (`src/qml/Main.qml`)**:
  - Split the former combined **Storage & Locations** page into two responsibility-focused pages: **01 Locations** (system root, workspace folder, OS image storage + `Save Locations`) and **02 Storage Pool** (Incus pool usage, allocated quotas, and loop-file growth).
  - Shifted the remaining pages to **03 OS Catalog**, **04 Create Options**, and **05 Safety**; `settingsSection` indices were renumbered accordingly (`AIBOX_SETTINGS_SECTION` now maps `0` = Locations, `1` = Storage Pool, `2` = OS Catalog, `3` = Create Options, `4` = Safety).
  - Added a **Pool details unavailable** fallback card with a `Refresh` action when `poolInfo.valid` is false, so the new page never renders blank.
- **Localization (`src/i18n/da.json`)**:
  - Added Danish entries for `Locations`, `Storage Pool`, `Storage Locations`, `Incus Storage Pool`, `Incus capacity & growth`, and the new pool page subtitle/fallback strings.

### Iteration 29 — Viewed History Acknowledgement
- **Unread-Only Badge (`src/cpp/backend.{h,cpp}`)**:
  - Added `acknowledgeOperationHistory()` plus a persisted `history-seen` marker (`<stateDir>/history-seen`) that stores the newest viewed history id. `recentFailedHistoryCount` now counts only failed entries newer than that marker, so the red `History (N!)` badge clears once the viewer is opened instead of requiring `Clear History`.
  - The marker is loaded on startup and after a settings/location change, survives restarts, is reset by `clearOperationHistory`, and new failures recorded afterwards raise the badge again.
- **History Dialog (`src/qml/Main.qml`)**:
  - `openHistoryDialog()` now calls `backend.acknowledgeOperationHistory()`, so opening History via `H`/the toolbar clears the notification while all entries and logs stay intact.
- **Tests**:
  - Extended `operationHistoryTruncationAndPersistence` to assert the counter drops to 0 after acknowledgement, history size stays intact across a restart, and a newer failed entry raises the count again (69 passed, 0 failed).

### Iteration 28 — Live Network Monitor Card
- **Network Metrics (`src/cpp/backend.{h,cpp}`)**:
  - `parseIncusListJson` now sums per-interface Incus `state.network.*.counters` (`bytes_received` / `bytes_sent`) and computes sampling-window rates from a `QHash<QString, QPair<qint64,qint64>>` snapshot, exposing `netRxBytes`, `netTxBytes`, `netRxRate`, `netTxRate`, `netRxRateText`, `netTxRateText`, `netRxTotalText`, and `netTxTotalText` per environment (zero extra processes).
  - Added `parseNetworkProbeOutput` (gateway from `ip route`, `/proc/net/route` little-endian hex fallback, `ping` packet-loss and `rtt` avg) and `networkStatsFromEnvironment`; a 10s `refreshNetworkStats` probe runs `incus exec <env> -- sh -c` for the selected running environment and exposes a `networkStats` property (ping, loss, rates, totals, IP, gateway, `active`).
  - Probes are cancelled when the environment stops, goes offline, or the selection changes; the process is guarded against overlap and killed after 9s, and the timer is skipped in demo mode.
- **Network Monitor Card (`src/qml/Main.qml`)**:
  - Added a simple flat panel under the Network Control bar with a live/idle dot, a manual `Probe` button, and a two-column eight-metric grid (Ping, Packet Loss, Receiving, Sending, Downloaded, Uploaded, IP Address, Gateway) using mono values, muted labels, and green/accent traffic accents.
- **Demo & Tests**:
  - Demo environments now carry sample network counters/probe values so the card renders in `AIBOX_DEMO_MODE`.
  - Added `parseNetworkProbeOutputReadsPingGatewayAndLoss`, `parseNetworkProbeOutputHandlesBlockedNetworkAndHexGateway`, `networkCountersProduceRatesAndTotals`, and `networkStatsFollowEnvironmentState` (69 passed, 0 failed).

### Iteration 27 — In-App Storage Pool Growth & Binary Size Labels
- **Grow Loop-Backed Pools (`src/cpp/backend.{h,cpp}`)**:
  - Extended `StoragePoolInfo` with `sizeText` / `resizable`; `refresh()` now also reads `incus storage get default size` (NUL-separated 4th part, failure-tolerant) so AIBox knows whether the pool is a loop file (`30GiB`) or a directory/subvolume pool that follows host free space.
  - Added `parsePoolSizeSpec` (bare numbers mean GiB, accepts `GiB`/`TiB`/`MiB` with `,` or `.` decimals), `validateStoragePoolResize` (grow-only, rejects garbage/unchanged/shrinking sizes, verifies host free space via `availableDiskBytes("/var/lib/incus")`, blocks during running tasks) and `resizeStoragePool` → `resizeStoragePoolScript` running `incus storage set <pool> size=<new>` (Incus truncates the loop file, refreshes the loop device, and runs `btrfs filesystem resize max` itself).
  - Exposed `resizable` / `sizeText` in `poolInfo` so QML can hide growth controls for non-resizable pools.
- **Settings → Storage Pool Card (`src/qml/Main.qml`)**:
  - Added an amber/red-aware **Incus storage pool** card under Settings → **Storage** with usage bar, `used / total`, driver + loop-file size (or directory-backed note), allocated quota sum with unbounded count, and a `Grow pool` row (`FlatInput` + button) that validates first and requires an inline confirmation noting running environments and that pools cannot shrink.
  - Made the sidebar pool footer clickable (`Manage`) to jump straight to the pool card, and reset the resize draft/confirmation whenever the settings modal opens.
- **Binary Size Units (`src/cpp/backend.cpp`, `src/qml/Main.qml`)**:
  - `formatBytes()` now reports `KiB` / `MiB` / `GiB` to match its 1024-based math and the `GiB` quotas shown by Incus, so the Disk Usage card no longer mixes `25.2 GB` with `100GiB` next to a `30.0 GB` pool.
- **Tests (`tests/aibox_tests.cpp`)**:
  - Added `poolResizeSpecValidationAndScript` covering spec parsing (bare GiB, `TiB`/`MiB`, invalid/negative/zero), grow-only validation, non-resizable pools, quoted script generation, and refusal without starting a task; updated size assertions to binary units (65 passed, 0 failed).

### Iteration 26 — Minimal Dark Application Icon Refresh
- **Application Icon (`pkgbuild/aibox.svg`)**:
  - Replaced the bright yellow icon with a dark slate squircle (`512x512` viewBox), subtle rim highlight, and a minimalist blue-accented container frame holding the `>_` prompt to match AIBox and Omarchy's visual style.

### Iteration 25 — Default `~/aibox` System Storage, First-Run Location Setup & Modal UI Consistency
- **System Storage Location (`src/cpp/backend.{h,cpp}`, `src/cpp/cli.cpp`)**:
  - Changed the default system storage root from `.aibox/` to `~/aibox/` (`workspaces/`, `os-images/`, `state/`, `config.json`) without creating `~/aibox` prematurely before first-run confirmation.
  - Added `~/.config/aibox/location.json` (overridable via `AIBOX_LOCATION_FILE`) to persist the user's chosen system storage root across launches, while keeping `--project <dir>` resolution (`<dir>/aibox/`, with legacy fallback to `<dir>/.aibox/` if already present).
- **First-Run Custom Storage Dialog (`src/qml/Main.qml`)**:
  - Updated the initial setup dialog (`firstRunOverlay`) to let users enter or browse (`Browse...` / `Default`) a custom system storage directory before clicking **Continue**, showing a live preview of `Workspaces`, `OS Images`, and `State & Cfg` paths.
- **UI Spacing & Modal Header Unification (`src/qml/Main.qml`)**:
  - Added a reusable `ModalHeader` component with uniform top-right `Close [Esc]` buttons, standardized `FlatInput` height (`34px`) and `10px` action row gaps, and unified margins (`24px`, `22px` in `createOverlay`) and corner radii across modals and inspector bars.
  - Fixed horizontal overflow and alignment issues in Clone Environment (`Shared at:` path elision), Environment Backups (`Browse...` / `Import Backup` alignment and archive name elision), Operation History (`Close [Esc]` button), Settings (removed duplicate close buttons and added System Root card), and the `Shared Workspace` bar.

### Iteration 24 — Roadmap Phase 6: H3 Session Recording (Opt-in)
- **Session Recording Config & Per-Environment Override (`src/cpp/backend.{h,cpp}`)**:
  - Added global `recordSessions` (`false` default) in `config.json` and `Backend::saveSessionRecordingSettings`.
  - Added per-environment Incus config key `user.aibox.record` (`inherit`/`1`/`0`), parsed into `recordOverride` / `recordEnabled` in `parseIncusListJson` and updated via `Backend::setEnvironmentRecordSessions`.
  - Refactored terminal launch via `launchCommandInTerminal` and added `Backend::buildSessionWrapCommand` wrapping interactive Enter shells with `script -q -f --log-out <log> --log-timing <timing> -c <cmd>` (util-linux) with automatic unrecorded fallback when `script` is unavailable.
- **Post-Session Secret Redaction, Retention Pruning & Wipe Cleanup (`src/cpp/main.cpp`, `src/cpp/cli.cpp`, `src/cpp/backend.{h,cpp}`)**:
  - Added fast internal CLI path `aibox --redact-session <logPath> [sessionsDir]` in `src/cpp/main.cpp` and `Backend::redactSessionLogFile` so exited sessions scrub S3 secrets from `secrets.json` without placing secret values on shell scripts or argv (`0600` permissions preserved).
  - Updated CLI `runEnter` (`src/cpp/cli.cpp`) to also wrap interactive shells via `fork()` + `execvp("script", ...)` when enabled and redact/prune immediately on exit.
  - Added `Backend::pruneRecordedSessions` (retaining last 20 sessions per environment and 200 MB total under `<stateDir>/sessions/<env>/<epoch>.{log,timing}`), invoked on GUI startup and after session completion.
  - Updated `wipeEnvironmentScript` and `exportAndWipeEnvironmentScript` (plus sync calls) to remove `<stateDir>/sessions/<env>` on wipe.
- **Sessions Viewer, Terminal Replay & i18n (`src/qml/Main.qml`, `src/i18n/da.json`)**:
  - Added **Session Recording (Opt-in)** toggle in Settings → **Safety** and per-environment `Inherit` / `On` / `Off` pills in the Guardrails card.
  - Added **Tasks** / **Sessions** segmented tabs inside **Operation History (`H`)** (`MODAL 6`) showing ANSI-stripped logs (`Backend::stripAnsiSequences`, `Backend::formatRecordedSessionText` capped at 200 KB), per-session delete, and **Replay in terminal** (`scriptreplay -t <timing> -O <log>`).
  - Added Danish translations in `src/i18n/da.json`.
- **Tests (`tests/aibox_tests.cpp`)**:
  - Added `sessionRecordingConfigOverridesAndWrapBuilder` and `sessionRecordingRetentionAnsiAndRedaction` covering `recordSessions` persistence, `user.aibox.record` overrides, `buildSessionWrapCommand` quoting and missing-`script` fallback, ANSI CSI/OSC/cr/backspace stripping, 200 KB truncation, `redactSessionLogFile`, and count/byte retention pruning (64 passed, 0 failed).

### Iteration 23 — Roadmap Phase 6: H4 Stale Environment Report (`U`)
- **Last-Used Tracking & Staleness Classifier (`src/cpp/backend.{h,cpp}`)**:
  - Recorded Unix epoch timestamps into `user.aibox.lastused` across GUI Enter (`prepareEnvironmentScript`), GUI headless execution (`runHeadlessCommand`), CLI `enter` / `exec` (`prepareEnvironmentScript`), and environment Start (`startEnvironmentScript`).
  - Added global `staleDays` in `config.json` (`30` default, valid `1..3650`, presets `7/14/30/60/90/180`) and `Backend::saveStaleSettings`.
  - Parsed `lastUsedEpoch` and `snapshotTotalBytes` in `Backend::parseIncusListJson` and implemented `Backend::classifyEnvironmentStaleness` (`Fresh`, `Stale`, `Unknown`, `Template`).
- **Orphan Scanner, Verified Export & Wipe, & Safe Cleanup (`src/cpp/backend.{h,cpp}`)**:
  - Added `Backend::detectOrphans` and `Backend::buildStaleReport` listing unused (`>= staleDays`) and unknown-age non-template environments (with disk usage, snapshot count/size, and workspace size) alongside orphan workspace folders, orphan `.snapshots/<env>` directories, and unpaired `-workspace.tar.gz` backup archives.
  - Added `Backend::exportAndWipeEnvironment` / `exportAndWipeEnvironmentScript` chaining `export` with an explicit `[ -s <archive> ]` verification gate so `wipe` aborts automatically if export fails or produces a 0-byte archive.
  - Added `Backend::openOrphanFolder` and path-validated `Backend::deleteOrphan` / `deleteOrphanScript` supporting Btrfs subvolume cleanup for orphaned snapshot dirs.
- **UI & i18n (`src/qml/Main.qml`, `src/i18n/da.json`)**:
  - Added `U` shortcut, `?` Help overlay entry, Sidebar/Empty-state **Stale & Orphans Report (`U`)** buttons, Settings → **Safety** `staleDays` selector, and **MODAL 11** listing stale/unknown environments (**Export & Wipe**, **Wipe**) and orphans (**Open folder**, **Delete** with size confirmation).
  - Added Danish translations in `src/i18n/da.json`.
- **Tests (`tests/aibox_tests.cpp`)**:
  - Added `staleClassificationAndConfigPersistence` and `staleOrphanDetectionAndExportWipeSafety` covering `staleDays` persistence, JSON parsing, `user.aibox.lastused` snippets, fixture tree orphan detection, path-safety guards, and Export & Wipe abort behavior on failed or 0-byte exports (62 passed, 0 failed).

### Iteration 22 — Roadmap Phase 6: H1 Idle Auto-Stop (Opt-in)
- **Global & Per-Environment Idle Policy (`src/cpp/backend.{h,cpp}`)**:
  - Added global `idleStopMinutes` (`0` = Off default, options `0/15/30/60/120`, clamped `0..1440`) in `config.json` and `Backend::saveIdleStopSettings`.
  - Added per-environment Incus keys `user.aibox.idlestop` (`inherit`, `off`, or minutes) and `user.aibox.idlestop_ports` (`1`/`0`), parsed in `parseIncusListJson` and updated via `Backend::setEnvironmentIdleStop`.
- **Idle Evaluator, Blockers & Auto-Snapshot Stop (`src/cpp/backend.{h,cpp}`)**:
  - Added static pure evaluator `Backend::evaluateIdleDecision` plus `parseIdleStopOverride`, `effectiveIdleStopMinutes`, and `parseActiveIncusExecEnvs` (parses `pgrep -u $USER -af "incus exec"` while skipping value-taking Incus flags such as `--cwd`, `--env`, `--user`, `--group`, `--mode`, `--project`, `--target`).
  - Integrated `evaluateIdleAutoStops()` inside `Backend::refresh()` so running non-template, non-busy environments require average CPU `< 3%` over the full window, no active user `incus exec` session, and no port forwards (unless `user.aibox.idlestop_ports` permits it).
  - Sends a 1-minute advance `notify-send` warning before executing a normal stop (`stopEnvironmentScript(name, true)` prepending `buildAutoSnapshotIfNeeded` when enabled, without `--force`), and records the reason `"idle"` in `operation_history.json` (`reason` field propagated through `runShellTask`, detached `task.meta`, and `recordOperationHistory`).
- **UI & i18n (`src/qml/Main.qml`, `src/i18n/da.json`)**:
  - Added **Idle Auto-Stop (Opt-in)** selector card (`Off` / `15m` / `30m` / `60m` / `120m`) in Settings → **Safety** with explicit notice that it works only while the AIBox GUI is running (no background daemon).
  - Added per-environment `Idle Auto-Stop:` override pills (`Inherit`, `Off`, `15m`, `30m`, `60m`) and `Ignore ports: ON/OFF` toggle in the Snapshots & Guardrails card, plus amber `IDLE` pill in Operation History (`H`).
  - Added Danish translations in `src/i18n/da.json`.
- **Tests (`tests/aibox_tests.cpp`)**:
  - Added `idleStopOverridesAndConfigPersistence` and `idleDecisionSyntheticSamplesAndBlockers` covering config defaults/persistence, override precedence, CPU spike reset, 1-minute advance warning, active `incus exec` flag/session parsing, port forward blocking/override, template skipping, and normal auto-snapshot stop script generation (60 passed, 0 failed).

### Iteration 21 — Roadmap Phase 6: H2 Storage Pool Overcommit Guard
- **Pool Capacity & Quota Sum Diagnostics (`src/cpp/backend.{h,cpp}`, `src/cpp/cli.cpp`)**:
  - Added `Backend::parseIncusStorageInfo` supporting `incus storage info default --bytes` (quoted byte counts), human-readable YAML (`1.24GiB` / `30.00GiB`), and JSON fixtures while ignoring `used by:` entries.
  - Added `Backend::sumRootDiskQuotasBytes` reading `expanded_devices.root` (falling back to direct `devices.root`) across instances on pool `default`, excluding a target environment during quota edits and counting unquoted/`Shared` environments; saved `diskQuotaBytes` in `parseIncusListJson`.
  - Refreshed `poolInfo` automatically inside `Backend::refresh()` and added `poolWarningForCreate` / `poolWarningForQuotaEdit` via `evaluatePoolOvercommitWarning` (warns when projected quotas exceed pool capacity or pool usage exceeds 85%).
  - Added `pool_capacity` (`evaluatePoolCapacityHealth`) as check #8 in `Backend::runHealthChecks` and `aibox check [--json]` (`[WARN]` above 85% pool usage, `[FAIL]` above 95% with bytes, percentages, allocated quota sum, and `[OVERCOMMITTED]` indicator in `detail`).
- **UI & i18n (`src/qml/Main.qml`, `src/i18n/da.json`)**:
  - Displayed live `Pool <name>: used / total (pct%)` and `Alloc` in the sidebar storage footer and inside the **Disk Usage** resource card, plus non-blocking amber warning banners in **Create Environment** and **Edit Quotas (`E`)** when overcommitted or >85% full; also fixed `win.borderHover` QML color warning in Secrets checkbox.
  - Added Danish translations in `src/i18n/da.json`.
- **Tests (`tests/aibox_tests.cpp`)**:
  - Added `poolStorageInfoParsingRealFixtures` (using real host `incus storage info default [--bytes]` outputs), `poolOvercommitMathAndUnboundedQuotas`, and `poolCapacityHealthCheckThresholds`, and updated health check count assertions to 8 (58 passed, 0 failed).

### Iteration 20 — Roadmap Phase 5: S3 Secrets Injection (`A`)
- **Secrets Store & Runtime-Only Injection (`src/cpp/backend.{h,cpp}`, `src/cpp/cli.cpp`)**:
  - Added global secret storage at `~/.config/aibox/secrets.json` (overridable via `AIBOX_SECRETS_PATH`), enforcing `0700` directory and `0600` file permissions (refusing group/world-readable files on load and fixing permissions to `0600` on save).
  - Enforced environment variable name validation (`^[A-Z_][A-Z0-9_]*$`) and stored only comma-separated key **names** per environment in `user.aibox.secrets` (never secret values).
  - Documented injection variant: `incus exec --help` selects interactive TTY mode only when both stdin and stdout are terminals, so stdin redirection breaks interactive shells; implemented the `/dev/shm` `tmpfs` fallback across GUI Enter (`Space`), `runHeadlessCommand`, and CLI `enter` / `exec` (`0600` host file in `$XDG_RUNTIME_DIR` -> `incus file push --mode 0400` to `/dev/shm/aibox-<uuid>.env` -> immediate host unlink -> `set -a; . "$AIBOX_ENV"; set +a; rm -f "$AIBOX_ENV"` via positional parameter `$1` inside container).
  - Added `Backend::redactSecrets` / `redactSecretsBytes` (longest-first replacement with `[REDACTED]`) across live task output streaming, detached task logs (`task.log`), and Operation History persistence; updated `aiExecCommandText` (`C`) to refer to `aibox exec <name> -- bash` instead of embedding values.
- **UI & i18n (`src/qml/Main.qml`, `src/i18n/da.json`)**:
  - Added `secretsVisible` modal (`A` shortcut, header **Secrets (A)** button, clickable **SECRETS: N** card badge, `?` overlay entry) with add/edit/delete for global secrets, masked values with per-row reveal toggle, permission error banner, and per-environment secret checkboxes.
  - Added Danish translations in `src/i18n/da.json`.
- **Tests (`tests/aibox_tests.cpp`)**:
  - Added `secretsValidationQuotingAndRedaction`, `secretsFilePermissionsEnforcedAndFixed`, and `secretsExecInjectionRedactsHistoryAndNeverTouchesBackups` (verifying quoting with `/bin/sh`, longest-first redaction, `0600`/`0700` enforcement and repair, CLI `exec` injection + host cleanup, and non-exposure in export/import/clone scripts; 55 passed, 0 failed).

### Iteration 19 — Roadmap Phase 5: S4 Headless CLI
- **Headless CLI (`src/cpp/cli.{h,cpp}`, `src/cpp/main.cpp`)**:
  - Moved CLI parsing and dispatch out of `src/cpp/main.cpp` into `Cli::parseArgs`, `Cli::helpText`, `Cli::execute`, and `Cli::run`, and added `src/cpp/cli.{h,cpp}` to `aibox.pro` and `tests/aibox_tests.pro`.
  - Runs headless commands under `QCoreApplication` (with `QT_QPA_PLATFORMTHEME` unset) so CLI works in plain/SSH terminals without X11, Wayland, or GTK initialization errors.
  - Added subcommands: `enter <env>` (runs GUI pre-steps then replaces process via `execvp("incus", ...)` for clean TTY pass-through), `exec <env> -- <cmd...>` (forwards command exit code), `start <env>`, `stop <env>`, `snapshot <env> [name]`, `snapshots <env> [--json]`, `restore <env> <snap> [--system-only|--workspace-only] [--yes]`, `diff <env> [snap] [--json]`, and `help [cmd]`, alongside existing `list [--json]` and `check [--fix] [--json]`.
  - Added global `--project <dir>` option (selecting the `<dir>/.aibox` directory, matching GUI defaults) and exit code mapping (`0` ok / command exit code, `1` failure, `2` usage error, `3` `task.pid` lock active, `4` env not found or template).
- **Shared Backend Refactoring (`src/cpp/backend.{h,cpp}`)**:
  - Added `Backend::PathMode::ProjectDir` constructor and `Backend::resolveProjectAiboxDir`, `refreshSync`, `isTaskLockActive`, `acquireCliTaskLock` / `releaseCliTaskLock` (enforced in `runShellTask` too), and shared script/diff helpers (`prepareEnvironmentScript`, `startEnvironmentScript`, `stopEnvironmentScript`, `enterIncusExecArgs`, `buildWorkspaceDiffArgs`, `processWorkspaceDiffOutput`) so GUI and CLI never duplicate shell scripts.
- **Tests (`tests/aibox_tests.cpp`)**:
  - Added `cliParsesAllSubcommandsAndFlags`, `cliProjectResolutionAndLockCheck`, and `cliExecAndExitCodesWithStubIncus` (52 passed, 0 failed).

### Iteration 18 — Roadmap Phase 5: S2 Change Review (`V`)
- **Workspace Change Review (`V`)** (`src/cpp/backend.{h,cpp}`):
  - Added `diffExcludes` (persisted in `config.json`, defaulting to `.git/`, `node_modules/`, `target/`, `.venv/`, `__pycache__/`, `build/`), `diffLoading`, `diffEntries`, `diffSummary`, `selectedDiffText`, and `selectedDiffInfo`.
  - Added `loadWorkspaceDiff(envName, snapName)` running `rsync -a -n --delete --itemize-changes --out-format='%i %l %n'` asynchronously with a 30s timeout and 5000-entry cap (`truncated` flag).
  - Added `loadFileDiff(envName, snapName, relPath, status)` validating safe relative paths, skipping files over 512 KB and binary files (NUL byte in first 8192 bytes), handling symlinks, running `diff -u --label snapshot --label current`, and capping output at 2000 lines.
  - Added `restoreSingleFile(envName, snapName, relPath)` copying the selected file or symlink back from `<workspaceBase>/.snapshots/<env>/<snapName>/<relPath>`, logging `"success"` in Operation History, and refreshing the diff list.
- **UI & i18n** (`src/qml/Main.qml`, `src/i18n/da.json`):
  - Added `changeReviewVisible` modal (`V` shortcut, `?` help overlay entry, **Changes** button on workspace/paired snapshot rows, snapshot selector strip, editable exclude rules, split-pane file list + monospace unified diff viewer + **Restore this file** action).
  - Added Danish translations in `src/i18n/da.json`.
- **Tests** (`tests/aibox_tests.cpp`):
  - Added `parseRsyncItemizeHandlesFixturesAndCap`, `diffExcludesBinaryCheckAndLineCapping`, and `realRsyncDetectsAddedModifiedDeletedAndHonorsExcludes` (49 passed, 0 failed).

### Iteration 17 — Roadmap Phase 5: S0 Btrfs Spike & S1 Workspace Snapshots
* **S0 — Host Btrfs Permission Spike**:
  * Verified on `7.2.15-1-omarchy` (`/home` Btrfs without `user_subvol_rm_allowed`): `btrfs subvolume create` and `btrfs subvolume snapshot -r` succeed as unprivileged user; `btrfs subvolume delete` fails with `EPERM`, while `btrfs property set -ts <snap> ro false && rm -rf <snap>` removes subvolumes cleanly when ACLs permit (falling back to printing `sudo btrfs subvolume delete` otherwise).
  * Verified container writes under `shift=true` inherit `user:<host>:rwx` default ACLs and stopped-container workspace swaps work cleanly.
  * Added check #7 (`workspace_subvol_rm`) to `aibox check` suggesting `user_subvol_rm_allowed` when the workspace Btrfs mount lacks it.
* **S1 — Paired `/workspace` Btrfs Subvolume Snapshots (`src/cpp/backend.{h,cpp}`, `src/qml/Main.qml`)**:
  * New, cloned, and imported environments automatically create `/workspace` as a Btrfs subvolume when hosted on Btrfs (`stat -f -c %T` == `btrfs`), falling back to plain directories on non-Btrfs filesystems.
  * Added **Convert to Subvolume** and **Delete `.pre-subvol`** actions for existing plain-directory workspaces on Btrfs.
  * Manual snapshots (`P`) and auto-snapshots before Enter/Start now take paired read-only workspace snapshots under `<workspaceBase>/.snapshots/<env>/<snapName>` (non-fatal warning logged to Operation History on failure), enforce `user.aibox.wssnap` (`WS: ON/OFF`), and prune/delete both halves together.
  * Added the **Restore Snapshot** confirmation dialog offering **Both (System + Workspace)**, **System only**, and **Workspace only**, taking an automatic `pre-restore-<epoch>` safety snapshot and stopping running environments before swapping `/workspace`.
  * Snapshot list rows show a monochrome `W` badge, combined system + workspace sizes, and a **Workspace-only snapshots** (`WS ONLY`) section for safety/orphan snapshots; wiping with workspace deletion removes `.snapshots/<env>/`.
* **Tests & Localization (`tests/aibox_tests.cpp`, `src/i18n/da.json`)**: Added 3 S1 unit test suites and updated health check tests for `workspace_subvol_rm` (46 total tests passing); added Danish translations for all new UI strings.

### Iteration 16 — Roadmap Phase 4: Resilience (Environment Export/Import & `aibox check --fix`)
* **R1 — Environment Export & Import (`src/cpp/backend.{h,cpp}`, `src/qml/Main.qml`)**:
  * Added full instance archive export (`incus export <name> <file>.tar.gz --compression gzip`) and optional separate host `/workspace` archive (`tar -czf ...-workspace.tar.gz`) stored under `<osStoragePath>/backups/`.
  * Added pre-flight disk-space estimation and verification (`checkExportSpace`, `hasSufficientDiskSpace`, `QStorageInfo` with 100 MiB safety margin) before starting export tasks.
  * Added `importEnvironment` (`incus import` to a new name, automatic `user.aibox.template` unset, target workspace binding, and optional workspace tarball extraction) and `deleteBackup`.
  * Added the **Backups (`B`)** header action and modal with archive scanner, workspace inclusion toggle, size/free-space preview, external archive `FileDialog`, and progress/cancel background task execution.
* **R2 — Host Health Diagnostics & Safe Auto-Repair (`src/cpp/backend.{h,cpp}`, `src/cpp/main.cpp`)**:
  * Refactored `aibox check` into 6 modular checks (`groups`, `socket_acl`, `incus_daemon`, `bridge_incusbr0`, `ai_gui_profile`, `ufw_rules` reading `/etc/ufw/user.rules`), supporting both human-readable output and `--json` while preserving `aibox list` unchanged.
  * Added `aibox check --fix` to automatically repair non-root issues (creating `incusbr0`, creating/repairing `ai-gui` nesting/`gpu`/`kvm` devices when the Incus daemon is reachable) and print exact `sudo` remediation commands for privileged fixes.
* **Tests & Localization (`tests/aibox_tests.cpp`, `src/i18n/da.json`)**: Added 6 unit tests covering disk-space safety margins, backup archive pairing, stubbed `incus` export/import round-trip scripts with workspace archives, healthy host stub checks, non-root `--fix` repairs, and root command hints; updated Danish translations and `?` shortcuts overlay.

### Iteration 15 — Roadmap Phase 3: Daily Workflow (Ports, Extra Mounts, Live Quotas, & Provisioning)
* **W1 — Port Forwarding (`src/cpp/backend.{h,cpp}`, `src/qml/Main.qml`)**:
  * Added Incus `proxy` device management bound strictly to `127.0.0.1` (`fwd-<hostPort>`), host port conflict checking via `QTcpServer` (`isHostPortAvailable`), parsed `portForwards` from `incus list --format=json`, and added the **Ports** modal with **Open** (`xdg-open http://127.0.0.1:<hostPort>`) and **Remove**.
* **W2 — Read-Only Extra Mounts (`src/cpp/backend.{h,cpp}`, `src/qml/Main.qml`)**:
  * Added extra host folder mounts under `/mnt/<name>` (`mnt-<name>`, `readonly=true` by default, `shift=true` with fallback) parsed as `extraMounts` in `parseIncusListJson`.
  * Enforced `validateExtraMountPath` blocking `/`, `/etc`, `/home`, `$HOME`, system prefixes, `.ssh`, `.gnupg`, `.aws`, `.kube`, `.docker`, `.aibox`, and AIBox's config/state directories; read-write mounts require an explicit second confirmation toggle (`confirmReadWrite`).
* **W3 — Live Resource Editing (`src/cpp/backend.{h,cpp}`, `src/qml/Main.qml`)**:
  * Added `updateEnvironmentResources` (`E` shortcut & **Quotas** button) to edit CPU cores, RAM limit, disk quota, container `limits.processes`, and `root` I/O bandwidth (`limits.read`/`write`) on existing environments, with a disk-shrink guard preventing resizing below current `diskUsageBytes` and a VM restart note.
* **W4 — Provisioning Presets (`src/cpp/backend.{h,cpp}`, `src/qml/Main.qml`)**:
  * Added selectable **Provisioning Presets** (`Dev Essentials`, `Node.js LTS`, `Python 3`, `Rust`) in **Create Environment** for Debian/Ubuntu (`apt`) and Arch (`pacman`), executed non-fatally via `incus exec` after boot and customizable under `<configDir>/provision/`.
* **Tests & Localization (`tests/aibox_tests.cpp`, `src/i18n/da.json`)**: Added 5 unit tests covering port validation & loopback socket conflict detection, extra mount path deny-list & RW confirmation, live resource script & byte-size parsing, non-fatal provisioning bundle execution, and `parseIncusListJson` extraction of `portForwards`/`extraMounts`; updated Danish translations and `?` shortcuts overlay.

### Iteration 14 — Roadmap Phase 2: Guardrails & Visibility
* **Disk & Resource Watchdog + Snapshot Sizes (`src/cpp/backend.{h,cpp}`, `src/qml/Main.qml`)**:
  * Evaluated per-environment disk thresholds (`watchdog.diskWarn` default `90%`, `watchdog.diskCritical` default `97%`) and sustained CPU/RAM high streaks inside the 4-second `refresh()` loop with a 5-point hysteresis/debounce state machine (`evaluateWatchdog`) so `notify-send` alerts fire once per escalation.
  * Added opt-in critical disk auto-stop (`watchdog.autoStopOnCritical`, per-environment override via `user.aibox.autostop` and `Auto-stop: ON/OFF` toggle), highlighted Disk Usage cards in amber/red on warning/critical levels, and added Watchdog controls to **Configuration → Safety**.
  * Lazily fetches snapshot disk consumption via `/1.0/instances/<name>/snapshots?recursion=1` (`parseSnapshotSizesJson`) for the selected environment and displays each snapshot's size in the rollback list when available.
* **Runaway Limits (`src/cpp/backend.{h,cpp}`, `src/qml/Main.qml`)**:
  * Added **Process Limit** (`limits.processes`, containers only, skipped for `--vm` presets) and **Disk I/O Limit** (`limits.read` / `limits.write` on `root`) selectors in the **Create Environment** dialog, validation helpers (`isValidProcessLimit`, `isValidIoLimit`), and Inspector limit badges (`Pids ... · I/O ...`).
* **Operation History (`src/cpp/backend.{h,cpp}`, `src/qml/Main.qml`)**:
  * Recorded every detached background task (`checkBackgroundTask`, `attachExistingTask`, `cancelTask`) as individual `<id>.json` metadata + tail-capped `200 KB` `<id>.log` files under `<stateDir>/history/` (keeping the newest 50 operations).
  * Added the **History** header button (`H` shortcut, showing failed count badge) and the **Operation History** viewer modal with `OK` / `FAILED` / `CANCELLED` badges, full scrollable task logs, and **Clear History**.
* **Egress Log (`src/cpp/backend.{h,cpp}`, `src/qml/Main.qml`)**:
  * Implemented per-environment kernel bridge ACL egress logging (`aibox-log-<name>` for online mode and `state=logged` rules for allowlist mode), **Record: ON/OFF** toggle (`user.aibox.egresslog`), and **Egress Log** viewer parsing `journalctl -k` (`parseKernelEgressLogs`) aggregated by `(dstIp, proto, port)` with hit counts, last-seen timestamps, and one-click **+ Allow**.
* **Tests & Localization (`tests/aibox_tests.cpp`, `src/i18n/da.json`)**: Added 5 unit tests covering watchdog hysteresis/auto-stop, snapshot sizes & watchdog config, container vs VM runaway limits, egress ACL scripts & kernel log parsing, and operation history truncation/persistence; updated Danish translations and shortcuts overlay (`?`).

### Iteration 13 — Roadmap Phase 1: Auto-Snapshots, Clone & Templates, & Network Control
* **Auto-Snapshots & Retention (`src/cpp/backend.{h,cpp}`, `src/qml/Main.qml`)**:
  * Added automatic `auto-<timestamp>` Btrfs snapshots before **Enter OS Terminal** and **Start OS** with a 5-minute minimum interval (`shouldCreateAutoSnapshot`) and automatic pruning of older `auto-` snapshots while keeping manual snapshots intact (`autoSnapshotsToPrune`).
  * Enabled by default (keeping the last 5 auto-snapshots); configurable globally in **Configuration → Safety** (`autoSnapshotEnabled`, `autoSnapshotKeep` in `config.json`) and overridden per environment via `user.aibox.autosnap` (`Auto: ON` / `Auto: OFF` toggle).
  * Snapshot entries prefixed with `auto-` display a monochrome `AUTO` tag in the rollback list.
* **Clone & Reusable Templates (`src/cpp/backend.{h,cpp}`, `src/qml/Main.qml`)**:
  * Added `cloneEnvironment` (`D` shortcut & **Clone** dialog) using `incus copy --stateless`, creating an isolated host workspace path, unsetting template flags, applying optional quotas, and isolating/copying network ACL configuration.
  * Added **Template** toggle (`T` shortcut & `user.aibox.template=true`) to mark stopped environments as reusable templates with a `TEMPLATE` badge, disabling direct start/enter actions and adding a **Fresh OS / From Template** mode selector in the **Create Environment** dialog.
* **Per-Environment Network Control (`src/cpp/backend.{h,cpp}`, `src/qml/Main.qml`, `aibox.pro`)**:
  * Added `Online`, `Allowlist`, and `Offline` modes plus a one-click **Kill Network / Restore Online** action (`K` shortcut) persisted via `user.aibox.netmode` and `user.aibox.netallow`.
  * `Offline` detaches `eth0` (`type=none`) live while keeping `/workspace` functional; `Allowlist` resolves allowed domains to public IPv4 CIDRs (filtering private/reserved DNS responses against DNS rebinding) and applies a per-instance `aibox-<name>` Incus Network ACL (`security.acls.default.egress.action=drop`), refreshing IPs on start and via **Refresh IPs**, and cleaning up the ACL on wipe.
* **Tests & Localization (`tests/aibox_tests.cpp`, `src/i18n/da.json`)**: Added 5 new unit tests covering auto-snapshot persistence, pruning/interval rules, template/network JSON parsing, clone script workspace isolation, and allowlist DNS-rebinding protection; updated Danish translations and keyboard shortcuts overlay (`?`).

### Iteration 12 — Task Manager Resource Cards, Per-Core CPU View, & Right-Aligned Actions
* **Shared Workspace & Snapshot Right-Aligned Actions (`src/qml/Main.qml`)**: Moved the **Open Shared Folder** (`F`) button into the right side of the **Shared Workspace** box beside the host ⟷ `/workspace` path, and anchored the **Snapshot label** input + **+ Save Snapshot** (`P`) button to the right edge of the Btrfs Snapshots header.
* **Task Manager Resource Monitor Cards (`src/cpp/backend.h`, `src/cpp/backend.cpp`, `src/qml/Main.qml`)**:
  * **Disk Usage**: Extracts `state.disk.root.usage` and `state.disk.root.total` (alongside the configured quota) from `incus list --format=json` to display live disk consumption (`<used> / <quota>`), usage percentage badge, and progress bar.
  * **Memory Usage**: Calculates memory usage percentage against `limits.memory` / `state.memory.total` and displays a live utilization badge and progress bar alongside `<used> / <limit>`.
  * **Per-Core CPU Usage**: Samples container/VM `state.cpu.usage` nanosecond counters across refresh intervals and `/proc/stat` core weights to render individual per-core percentage labels (`C1`, `C2`, `C3`, ...) and mini utilization bars inside the **CPU Usage** card.
  * **Network Status**: Enhances the **IPv4 Address** card with an `ONLINE` / `OFFLINE` status indicator and connection bar.
* **Unit Tests & Localization (`tests/aibox_tests.cpp`, `src/i18n/da.json`)**: Extended `parseIncusJsonExtractsSpecsAndSnapshots` to verify disk usage, memory percentage, CPU delta percentage, and `cpuCorePercents` list generation, and added translation keys for the new resource monitor labels.

### Iteration 11 — Create Environment Script Quoting Fix
* **Preset & Quota Arguments (`src/cpp/backend.cpp`, `src/cpp/backend.h`)**: The generated `task.sh` used to assign the preset flags and quota arguments unquoted (`PROFILE_FLAGS=-p default -p ai-gui`), so bash tried to run `default` as a command and the creation aborted with `line 11: default: command not found`. The script now stores both as properly quoted bash arrays (`PROFILE_ARGS=(...)`, `QUOTA_ARGS=(...)`) that are expanded as `${PROFILE_ARGS[*]}` / `${QUOTA_ARGS[*]}` when calling `incus launch`, and the quota values are no longer double-quoted into the Incus argument. The script builder was extracted into the testable `Backend::createEnvironmentScript()` plus the `shellArrayWords()` helper.
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
* **Agent Guidelines & Permanent Rules ([AGENTS.md](AGENTS.md))**: Added workspace rules and global rules enforcing that any AI agent adding a feature, modifying the UI, or fixing bugs must build (`./bin/build`), test (`./bin/test`), build the Arch package (`makepkg -f`), and update the system (`./bin/install`) so the device is always up to date.
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
* Standardized all project documentation ([README.md](README.md), [CATALOG.md](CATALOG.md)), helper scripts ([bin/setup-system](bin/setup-system), [bin/uninstall](bin/uninstall), `~/.local/bin/ai-box`)), desktop metadata, and UI catalogs to English.

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
* Built the native C++17 & Qt Quick 6 desktop app in `~/Program/aibox`.
* Implemented master-detail environment inspector, 1-click terminal launch (`Space`), shared folder opener (`F`), 1-second Btrfs snapshots (`P`), headless Xvfb test runner, and 1-second OS wipe (`Delete`).
* Added an 8-distro OS catalog with one-tap local image caching.
* Packaged with `PKGBUILD` and installed into the Omarchy App Launcher (`Super + Space`).

### Iteration 1 — Base Incus Infrastructure & `ai-box` CLI
* Installed and configured Incus with a Btrfs storage pool and disk quotas enabled.
* Configured `incusbr0` bridge networking and UFW firewall rules.
* Created the `ai-gui` profile for Intel Iris Xe GPU (`/dev/dri`) and KVM (`/dev/kvm`) passthrough.
* Created the interactive CLI wrapper at `~/.local/bin/ai-box`.
