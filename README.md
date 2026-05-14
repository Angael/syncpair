# syncpair

Minimal Rust wrapper around `rclone bisync` for personal two-way backups on Arch/CatchyOS machines.

## What It Does

- reads sync pairs from `~/.config/syncpair/syncs.toml`
- runs `rclone bisync` under the hood
- proxies live `rclone` output to the terminal
- supports normal sync, dry-run preview, and resync
- keeps dated trash backups for real sync runs
- writes logs under `~/.local/state/syncpair/`
- shows desktop notifications through `notify-send`
- opens config and log files with `code`
- installs or removes a `systemd --user` timer for daily syncs

## Requirements

Required runtime commands:

- `rclone`
- `notify-send`
- `code`
- `systemctl`

`xdg-open` or `dolphin` is used for opening the trash directory.

## Config

Path:

```text
~/.config/syncpair/syncs.toml
```

Example:

```toml
remote = "onedrive"

[[sync]]
local = "~/notes"
remote = "notes"

[[sync]]
local = "~/.config/fish"
remote = "Backups/dotfiles/fish"
```

## Usage

Interactive menu:

```bash
syncpair
```

Direct commands:

```bash
syncpair show-config
syncpair edit-config
syncpair view-logs
syncpair open-trash
syncpair sync
syncpair sync --dry-run
syncpair resync --mode newer
syncpair resync --dry-run --mode older
syncpair timer status
syncpair timer install
syncpair timer remove
```

## Timer Behavior

The installed systemd user service runs:

```text
/usr/bin/syncpair daily-sync
```

That path is intentional.

Why:

- when installed from a package, `/usr/bin/syncpair` is the stable binary path
- using the currently running binary path would capture temporary dev paths like `target/debug/syncpair`
- those dev paths can disappear after rebuilds or cleanup, which would break the timer later

## Build

```bash
cargo build
```

## Packaging

For AUR / `paru -S syncpair`, see `PUBLISHING.md`.

There is also a starter `PKGBUILD` template in `packaging/PKGBUILD`.

## License

This project is licensed under the Unlicense.
