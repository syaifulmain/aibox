# AIBox

AIBox is a native Qt Quick desktop app for managing isolated Linux environments with Incus on Omarchy/Arch Linux.

![AIBox screenshot](screenshot.png)

## Features

- Create and manage Linux containers and virtual machines with configurable CPU, memory, and disk limits.
- Share a project workspace with environments at `/workspace`.
- Cache OS images, track task progress, and resume background operations.
- Save and restore Btrfs snapshots.
- Follow the active Omarchy theme.

## Requirements

- Omarchy or Arch Linux
- Incus and a Btrfs storage pool
- Qt 6 development tools (`qmake6`) to build

## Setup and install

On a new machine, configure Incus and required host settings:

```bash
./bin/setup-system
```

Build and install the application as an Arch package:

```bash
./bin/install
```

Run the test suite:

```bash
./bin/test
```

## Usage

Launch **AIBox** from the app launcher (`Super` + `Space`). By default, project workspaces, cached images, and settings are stored in `.aibox/` under the launch directory. The configuration menu lets you change storage paths and environment options.

## Keyboard Shortcuts

| Key | Action |
| :--- | :--- |
| `Space` / `Enter` | Enter selected OS terminal in `/workspace` |
| `N` | Create a new isolated OS environment |
| `F` | Open shared workspace folder on Host |
| `S` | Start or stop selected environment |
| `P` | Create instant 1-second Btrfs snapshot |
| `C` | Copy AI agent execution command |
| `Delete` | Wipe / destroy selected environment |
| `1` / `2` | Switch between Environments and OS Catalog |
| `3` | Open the configuration menu |
| `R` | Refresh status and disk usage |
| `?` | Toggle keyboard shortcuts overlay |
| `Q` | Quit AIBox |

## Uninstall

Remove the application while keeping Incus and environments:

```bash
./bin/uninstall
```

To also remove Incus and all environments:

```bash
./bin/uninstall --full
```

## License

See [LICENSE](LICENSE).

> **Note:** Setup and uninstall scripts modify host system configuration. Review them before running.

For architecture and change history, see [CATALOG.md](CATALOG.md). Repository-specific development guidance is in [AGENTS.md](AGENTS.md).
