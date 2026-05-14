# PRD

## Name
`syncpair`

## Summary
`syncpair` is a minimal Rust CLI that replaces the current `bkp-bi` shell tool with a small, readable wrapper around `rclone bisync` for personal two-way backups across Arch/CatchyOS machines.

## Goal
Keep the current workflow, but make it easier to maintain, package, and read than the shell version.

## Users
- Primary user: me
- Target environment: CatchyOS / Arch-based personal PCs

## Current Status
- A working Rust prototype already exists in this repo
- The codebase is now split into small modules instead of one large `main.rs`
- The project builds successfully with `cargo build`
- Current module layout:
- `src/main.rs`: CLI wiring
- `src/ui.rs`: interactive menu and resync selection
- `src/rclone.rs`: `rclone bisync` execution and output proxying
- `src/timer.rs`: `systemd --user` timer install/remove/status
- `src/logs.rs`: log file creation and writing
- `src/config.rs`: `syncs.toml` parsing and validation
- `src/paths.rs`: XDG paths and file locations
- `src/utils.rs`: notifications, required command checks, path helpers, file opening

## Core Requirements
- Read backup definitions from a clearer config file, renamed from `dirs.conf` to `syncs.toml`
- Validate sync entries before running
- Use `rclone` under the hood for all actual sync work
- Proxy `rclone` stdout/stderr live so the user sees real command output
- Support normal sync, preview/dry-run sync, resync, and resync mode selection (`newer` / `older`)
- Keep dated trash backup directories for real sync runs
- Write logs to an XDG state directory
- Support installing and removing a `systemd --user` timer for scheduled syncs
- Show desktop notifications for start, success, and failure
- Use `code` to open text files such as the config file and log file

## Required Runtime Tools
- `rclone`
- `notify-send`
- `code`
- `systemctl`

Additional file-opening support for trash directory:
- `xdg-open` preferred
- `dolphin` fallback

## UX Requirements
- Keep the program minimal and readable
- Prefer a small interactive CLI with clear action selection
- Use popular Rust crates if they reduce code and improve readability
- Notifications may use `notify-send` if that remains the simplest acceptable implementation
- `code` is required, not optional

## Config Proposal
```toml
remote = "onedrive"

[[sync]]
local = "~/notes"
remote = "notes"

[[sync]]
local = "~/.config/fish"
remote = "Backups/dotfiles/fish"
```

Actual config location:

```text
~/.config/syncpair/syncs.toml
```

Logs and state location:

```text
~/.local/state/syncpair/
```

## Non-Goals
- No custom sync engine
- No GUI
- No bundled `rclone`
- No broad backup platform abstraction beyond `rclone`

## Technical Direction
- Language: Rust stable
- Primary target: `x86_64-unknown-linux-gnu`
- External runtime dependency: `rclone`
- Rust edition: `2021`
- Current crates in use:
- `anyhow = 1.0.102`
- `chrono = 0.4.44`
- `clap = 4.6.1`
- `dialoguer = 0.12.0`
- `dirs = 6.0.0`
- `serde = 1.0.228`
- `toml = 1.1.2`

## CLI Shape
- `syncpair` launches the interactive menu
- `syncpair show-config`
- `syncpair edit-config`
- `syncpair view-logs`
- `syncpair open-trash`
- `syncpair sync [--dry-run]`
- `syncpair resync [--dry-run] [--mode newer|older]`
- `syncpair timer install|remove|status`
- `syncpair daily-sync`

## Important Timer Decision
- The installed user service is expected to run `/usr/bin/syncpair daily-sync`
- This is intentional for packaged installs
- The timer must not capture a temporary development path like `target/debug/syncpair`
- Reason: dev paths can disappear after rebuilds or repo cleanup, but `/usr/bin/syncpair` is the stable installed path on Arch systems

## Packaging Notes
- The program should be buildable in a normal way for Arch/CatchyOS systems
- Later installation via `paru -S syncpair` is feasible by publishing the source and maintaining an AUR package with a `PKGBUILD`
- Initial design should stay Arch-friendly and avoid unnecessary packaging complexity
- Packaging assumptions currently baked into the repo:
- binary name: `syncpair`
- installed binary path: `/usr/bin/syncpair`
- license: `Unlicense`
- required Arch runtime deps: `rclone`, `libnotify`, `code`
- starter AUR packaging file exists at `packaging/PKGBUILD`

## Success Criteria
- Replaces the day-to-day job of `bkp-bi`
- Keeps the codebase small and readable
- Preserves the current rclone-based workflow
- Works cleanly on CatchyOS / Arch machines
- Can later be packaged for AUR distribution

## Next Likely Work
- Test the real runtime flow with local `rclone` config
- Test timer install/remove on an actual user session
- Validate the AUR package locally with `makepkg`
- Optionally add a `syncpair-git` AUR package later
