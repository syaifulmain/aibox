# AIBox Roadmap

Planning document. Shipped work is recorded in the Changelog in [CATALOG.md](../CATALOG.md); this file only tracks what is planned. When a feature ships, move it to the Changelog and update [README.md](../README.md).

## Current baseline

Implemented (do not re-plan):

- Create, start, stop and wipe containers and VMs with CPU, RAM and disk limits.
- Real-time resource monitor cards.
- Shared `/workspace` with host ACLs.
- OS catalog, image cache and offline image export/import.
- Btrfs snapshots: create, restore, delete.
- Auto-snapshots before Enter and Start, with retention and a per-environment toggle (Iteration 13).
- Clone and reusable templates (Iteration 13).
- Per-environment network mode: Online, IP-based Allowlist through Incus Network ACLs, Offline (Iteration 13).
- Disk & resource watchdog with hysteresis alerts, opt-in critical disk auto-stop, and per-snapshot sizes (Iteration 14).
- Runaway limits (`limits.processes` for containers and `limits.read`/`limits.write` on root) (Iteration 14).
- Operation history viewer with persistent metadata and tail-capped 200 KB logs (Iteration 14).
- Kernel bridge ACL egress log (`Record: ON/OFF`, `journalctl -k` aggregation, and one-click `+ Allow`) (Iteration 14).
- Localhost (`127.0.0.1`) port forwarding with conflict detection and browser open (Iteration 15).
- Read-only extra mounts under `/home/workspace/<name>` with system/sensitive path protection (Iteration 15, updated in Iteration 40).
- Live quota & limit editing with disk-shrink guard (Iteration 15).
- Provisioning presets (`devtools`, `node`, `python`, `rust`) for Debian/Ubuntu/Arch under `.aibox/provision/` (Iteration 15).
- Full environment + `/workspace` backup export & import (`B`) with pre-flight disk free-space guard (Iteration 16).
- Modular `aibox check [--fix] [--json]` host diagnostics and non-root auto-repair (Iteration 16).
- Paired `/workspace` Btrfs subvolume snapshots (auto + manual), System/Workspace/Both restore with `pre-restore` safety snapshots, plain-folder conversion, and `workspace_subvol_rm` health check (Iteration 17: S0 + S1).
- Workspace change review (`V`): async `rsync` diff summary vs any workspace snapshot with configurable exclude patterns, unified `diff -u` inspection, and single-file restore (Iteration 18: S2).
- Headless CLI (`src/cli.{h,cpp}`): `enter`, `exec`, `start`, `stop`, `snapshot`, `snapshots`, `restore`, `diff`, `list`, `check`, `help`, global `--project <dir>`, shared `Backend` script builders, `task.pid` lock checks, and exit codes `0..4` (Iteration 19: S4).
- Secrets injection (`A`): host-only `~/.config/aibox/secrets.json` (`0600`/`0700`), per-environment `user.aibox.secrets` key names, runtime `/dev/shm` `tmpfs` injection for GUI Enter, headless tasks, and CLI `enter`/`exec`, and longest-first `[REDACTED]` task/history scrubbing (Iteration 20: S3).
- Storage pool overcommit guard: live `incus storage info default` capacity + `expanded_devices.root` quota sum in sidebar/Disk card, non-blocking Create & Edit Quotas (`E`) overcommit warnings, and `pool_capacity` in `aibox check` (`>85%` warn, `>95%` fail) (Iteration 21: H2).
- Idle auto-stop (opt-in): global `idleStopMinutes` (`0` default) & per-env `user.aibox.idlestop` / `idlestop_ports`, CPU `<3%` + active `incus exec` + port-forward blockers, 1-minute `notify-send` warning, normal auto-snapshot stop with `idle` history reason (GUI-only, no daemon) (Iteration 22: H1).
- Stale environment report (`U`): `user.aibox.lastused` tracking on Enter/Exec/Start, `staleDays` (`30` default) staleness report with disk/snapshot/workspace sizes, orphan workspace/snapshot/backup scanner, **Open folder**, size-confirmed **Delete orphan**, and verification-gated **Export & Wipe** (Iteration 23: H4).
- Opt-in session recording (`recordSessions` global + `user.aibox.record` override): wraps GUI & CLI Enter with `script -q -f --log-out ... --log-timing ...`, redacts S3 secrets post-session, prunes to 20 sessions/env and 200 MB, inspects ANSI-stripped logs (200 KB cap) & replays via `scriptreplay` in **History (`H`) → Sessions**, and cleans session data on Wipe (Iteration 24: H3).
- Detached background tasks with progress and cancel.
- Omarchy theme, language and keyboard shortcut integration.

## Selection criteria

A feature belongs here only if it does at least one of these:

1. Reduces the damage a runaway or misbehaving agent can do.
2. Lets the user diagnose a failure that is currently invisible.
3. Removes a repeated manual `incus` command the user would otherwise type.
4. Protects data against loss.

Features that only add surface area (extra launchers, grouping, cosmetic views) are listed under [Dropped](#dropped) with the reason.

## Guiding principles

- Incus and Btrfs stay the only engine. No new daemons or resident helpers.
- Containers first. VM support is best effort unless a feature says otherwise.
- Follow the Omarchy design rules in [GEMINI.md](../GEMINI.md): monochrome icons, 34px controls, 10px gaps, shortcuts listed in the `?` overlay.
- Every UI string is added to both `src/i18n/en.json` and `src/i18n/da.json`.
- Every feature ships with unit tests in `tests/aibox_tests.cpp`, passes `./bin/test`, and is verified on the real host.
- New `config.json` keys must have safe defaults so older configs load unchanged.

## Phases

| Phase | Theme | Status | Features |
| :--- | :--- | :--- | :--- |
| 1–4 | Foundation, Guardrails, Daily Workflow, Resilience | Shipped (Iterations 13–16) | All planned phases implemented and verified |
| 5 | Workspace Safety & Scriptability | Shipped (Iterations 17–20) | S0–S4 shipped (Btrfs workspace snapshots, change review, secrets injection, headless CLI) |
| 6 | Hygiene & Forensics | Shipped (Iterations 21–24) | H1–H4 shipped (pool overcommit guard, idle auto-stop, opt-in session recording, stale environment report) |

All planned Phase 1–6 roadmap features are now shipped and documented in [CATALOG.md](../CATALOG.md).

---

## Instructions for the implementing agent

Read this section before touching code. It applies to every Phase 5 and 6 item.

1. **Read first:** [GEMINI.md](../GEMINI.md) / [AGENTS.md](../AGENTS.md) (mandatory workflow), `src/backend.h`, and the relevant parts of `src/backend.cpp` and `src/Main.qml`. Follow the existing patterns:
   - Long operations run as detached background tasks with a generated bash script, `@@STAGE:<pct>:<text>` progress lines, and cancel support (see `createSnapshot`, `restoreSnapshot`, `exportEnvironment`).
   - Per-environment flags live in Incus config keys `user.aibox.*` (see `user.aibox.autosnap`, `user.aibox.autostop`) and are parsed in `parseIncusListJson`.
   - Global settings live in `config.json` and are loaded/saved next to `watchdogAutoStop`. New keys need safe defaults so old configs load unchanged.
   - All shell arguments go through `shellQuote`. Never interpolate unquoted user input into scripts.
   - Pure logic (parsers, script builders, validators) is written as `static` functions so `tests/aibox_tests.cpp` can test them without Incus. Stub `incus`/`btrfs` with fake scripts on `PATH`, as the existing export/import tests do.
2. **UI rules:** monochrome icons only (no emoji), `theme.accent`, `theme.radius`, 34px controls, 10px gaps. Register every new shortcut in the `?` overlay and the README table. Before assigning a key, check that it is not already handled in `src/Main.qml`. The keys suggested below (`A`, `V`, `U`) were free when this was written.
3. **i18n:** every new UI string goes into both `src/i18n/en.json` and `src/i18n/da.json`.
4. **Definition of done (per feature):** unit tests added → `./bin/install` (run once) → `./bin/test` passes → verified on the real host → move the feature from this file to the CATALOG changelog → update README features and shortcuts.
5. **Do not** add daemons, systemd units, or root-only steps to the normal flow. If something needs root, `aibox check` prints the exact command instead of running it.
6. If a spike result contradicts a design below, stop, record the finding under [Open questions](#open-questions), and pick the documented fallback. Do not invent a different architecture.

---

## Phase 5: Workspace Safety & Scriptability

*(All Phase 5 features S0–S4 shipped across Iterations 17–20 and are documented in [CATALOG.md](../CATALOG.md).)*

---

## Phase 6: Hygiene & Forensics

*(All Phase 6 features H1–H4 shipped across Iterations 21–24 and are documented in [CATALOG.md](../CATALOG.md).)*

---

## Dropped

| Item | Reason |
| :--- | :--- |
| GUI app launcher | The terminal and `xdg-open` already cover this; it adds a surface to maintain without removing a manual step. |
| Project groups and tags | Only useful with dozens of environments. Revisit if the list grows. |
| Log and shell viewer inside the GUI | The user already has a real terminal. G4 covers the missing piece, which is logs of finished operations. |
| Scheduler | Incus has native `snapshots.schedule` and `snapshots.expiry`, and F1 already covers the common case. Idle auto-stop is folded into G1 as an opt-in. |

## Known limitations of shipped features

- **Allowlist is IP-based.** Domains behind a CDN can change addresses, so entries may go stale; **Refresh IPs** and refresh-on-start mitigate it, and G3 helps the user see real destinations.
- **Phase 1 UI paths** were verified against Incus commands manually and by unit tests, but should be exercised once on the installed build.

## Cross-cutting work

- **i18n.** Add keys for each feature to both language files in the same change.
- **Shortcuts.** Register new keys in the `?` overlay and the README table.
- **Docs.** Each shipped feature updates the CATALOG changelog and README features list.
- **Definition of done.** Works on the real host, `./bin/install` succeeds, `./bin/test` passes, docs updated.

## Open questions

Answered questions from Phases 1–4 are recorded in the CATALOG changelog. Open for Phases 5–6:

1. S0: Verified on `7.2.15-1-omarchy` (`/home` Btrfs without `user_subvol_rm_allowed`). `btrfs subvolume create`/`snapshot -r` work as user; `btrfs subvolume delete` fails with `EPERM`, while `btrfs property set -ts <snap> ro false && rm -rf <snap>` works when ACLs permit; container writes with `shift=true` are `root:root` with inherited `user:<host>:rwx` ACL; stopped container workspace swap works. Fallback (`workspace_subvol_rm` in `aibox check` + `sudo` command fallback) applied.
2. S1: should **Convert to subvolume** be offered automatically on first launch after upgrade, or only from the environment menu? (Current plan: menu only, plus a one-time hint.)
3. S3: Chosen variant: `/dev/shm/aibox-<uuid>.env` via `incus file push --mode 0400` + immediate host unlink + in-guest `set -a; . "$AIBOX_ENV"; set +a; rm -f "$AIBOX_ENV"`, because `incus exec --help` confirms interactive TTY mode requires both stdin and stdout to be terminals. (Shipped Iteration 20.)
4. H1: is 3% average CPU the right idle threshold for environments running a language server or file watcher? Measure on the real host before shipping.
5. H3: should recordings be excluded from backups (`B`)? (Current plan: yes, they live in the host state directory and are never exported.)
