# AIBox — Developer and AI Agent Guidelines

AIBox is a native Qt Quick desktop application that manages isolated Linux
environments (Incus containers and virtual machines) on Omarchy and Arch Linux.
This file is the contract for every human or AI agent that changes the codebase.
`CLAUDE.md` and `GEMINI.md` are symlinks to this file, so keep one source of
truth.

Rust migration in progress: Rust sources live under `rust/`; run `cargo fmt`, `cargo clippy` and `cargo test` as part of `./bin/test`.

## Mandatory workflow for every change

Run these steps for any feature, UI change, behaviour change, or bug fix. Do not
declare the work finished before all three steps pass.

1. Install and update the local system build:

   ```bash
   ./bin/install
   ```

   This builds the application, packages it as a native Arch Linux package, and
   installs it. That covers `/usr/bin/aibox`, the desktop entry, and the
   `Super + Space` launcher. Run it once per change set. Do not run `makepkg`
   separately and do not run `./bin/install` a second time. If installation
   stops at a system authentication prompt, report that installation is
   incomplete and ask the user to run `./bin/install` locally.

2. Run the test suite:

   ```bash
   ./bin/test
   ```

   All tests must pass before you report completion. If a test fails, fix the
   cause; do not delete or skip the test.

3. Update documentation:

   - Add a changelog entry in [CATALOG.md](CATALOG.md).
   - Update [README.md](README.md) when a feature, shortcut, dependency, or
     option changes.

Fast inner loop, for iteration only: `./bin/build` compiles into `build/`
without packaging. Always finish with `./bin/install` and `./bin/test`.

## Repository layout

```
aibox.pro                 qmake6 project file for the application
bin/                      build, install, test, uninstall, system setup scripts
pkgbuild/                 PKGBUILD, desktop entry, icon for the Arch package
src/cpp/                  C++ sources and headers
src/qml/                  QML user interface, Main.qml is the window shell
src/qml/components/       reusable QML controls (buttons, inputs, modal header)
src/qml/dialogs/          modal overlays (create, clone, wipe, history, secrets)
src/qml/panels/           layout sections (header, banner, content, settings)
src/i18n/                 translation catalogs (en.json, da.json)
src/resources.qrc         Qt resource manifest for QML and catalogs
tests/                    offscreen Qt Test suite
```

Key C++ entry points:

- `src/cpp/main.cpp`: process entry point. Dispatches the headless CLI path,
  the session redaction helper, and the QML GUI.
- `src/cpp/cli.{h,cpp}`: CLI parsing and subcommands (`list`, `check`, `enter`,
  `exec`, `start`, `stop`, `snapshot`, `snapshots`, `restore`, `diff`, `help`).
- `src/cpp/backend.{h,cpp}`: the `Backend` class that owns all state and actions.
  Its implementation is split across feature modules (`backend_*.cpp`) that all
  define members of the same class. `backend.h` remains the single declaration,
  and `backend_helpers.h` holds the shared internal helpers.
- `src/cpp/theme.{h,cpp}`: live watcher for the active Omarchy theme
  (`colors.toml`) and Hyprland window rounding.
- `src/cpp/translator.{h,cpp}`: live language loader
  (`~/.config/omarchy/language`).
- `src/qml/Main.qml`: application window, state properties, shortcuts, and the
  composition of the extracted QML files.
- `src/qml/components/`, `src/qml/dialogs/`, `src/qml/panels/`: reusable
  controls, modal overlays, and layout sections. Extracted files receive the
  window with an `app` property instead of referencing the `win` id.
- `tests/aibox_tests.cpp`: offscreen Qt Test suite, run by `./bin/test`.

When you add a source file, register it in `aibox.pro` and in
`tests/aibox_tests.pro`. When you add or move a QML file, register it in
`src/resources.qrc` with an alias that matches its import path.

## Architecture notes

- Framework: C++17 with Qt Quick 6 and `qmake6`. No CMake.
- Isolation: Incus system containers and VMs, Btrfs quotas, KVM
  (`/dev/kvm`), and Intel Iris Xe GPU passthrough (`/dev/dri`).
- Progress streaming: long operations run under a pseudo-terminal
  (`script -q -e -c`) and report staged progress to QML.
- Background tasks: detached sessions (`setsid`) with a PID/task lock, so the
  GUI can restart and reattach to a running operation.
- Configuration and state live under the project `.aibox/` directory or the
  configured system path, never inside the repository history.
- Secrets are stored on the host (`~/.config/aibox/secrets.json`, mode `0600`)
  and injected only at execution time. Never write secret values to logs,
  workspaces, snapshots, or fixtures.

## Code style

- C++: C++17, Qt naming (`camelCase` methods, `m_` members), 4-space indent,
  braces on the same line, `QStringLiteral` for literal strings. Follow the
  surrounding file.
- QML: 4-space indent, one property per line, `id` first, bindings over
  assignments. Reusable controls belong in `src/qml/components`. Never hardcode
  colours, font families, font sizes, spacing, radii or control heights: use
  `Style.qml`, the design-token singleton. Dialogs build on `ModalDialog` and
  only declare their own content. Pass the window reference only when a control
  needs window functions, not just colours or fonts.
- Every new user-visible string uses `qsTr()` and is added to both
  `src/i18n/en.json` and `src/i18n/da.json`.
- Comments explain why, not what. Keep them short and in English.

## Omarchy design conventions

- No emoji and no coloured gear icons. Use simple monochrome icons that follow
  the Omarchy accent colour (`theme.accent`), such as the `SlidersIcon` QML
  component, drawn from lines and knobs. An icon must stay readable without
  colour.
- Read the accent from `theme.accent`, never hardcode a highlight colour.
- Follow the Hyprland window radius (`theme.radius`) for rounded corners.
- Use `JetBrainsMono Nerd Font` for technical text and `Adwaita Sans` for
  general UI text.
- Keep control metrics uniform: 34 px control height, 10 px gaps.
- Every keyboard shortcut must be listed in the `?` help overlay and in the
  README shortcut table.

## Maintaining the codebase

- Keep files reviewable. One file should not grow past roughly 1500 lines. When
  a file grows past that, split it by feature, not by chance.
- Extend an existing module when the feature belongs to it. Create a new module
  when a feature area has no natural home.
- Prefer one shared helper over duplicated logic. Paths, shell quoting, size
  formatting, and JSON access already exist in the backend helpers; reuse them.
- Keep `Backend` as the only source of truth for state that QML reads. Do not
  add parallel state in QML when a backend property can serve.
- After a refactor, verify behaviour, not just compilation: build, run the test
  suite, and exercise the affected screen.
- Documentation that references file paths must stay accurate. Update the path
  in the same change that moves the file.

## Definition of done

- `./bin/install` and `./bin/test` succeed.
- No new compiler warnings and no QML warnings (binding loops, undefined
  references, type errors).
- Changelog and README are updated.
- New shortcuts appear in the help overlay.
- No secrets, local paths, or build artifacts are committed.
