# AGENTS.md

Notes for coding agents working in this repo. `README.md`, `PRD.md` and `PUBLISHING.md` cover the product and packaging; this file covers the workflow.

## Project

- Rust desktop app (egui/eframe) around `rclone bisync`, plus headless commands used by the daily systemd timer.
- `src/rclone.rs` runs bisync and turns its output into progress, errors and the stale-lock recovery.
- `src/logs.rs` owns `syncpair.log` and the saved rclone output of failed folders.
- `src/gui/` holds one file per page (`overview`, `folders`, `activity`, `trash`) plus `mod.rs` for app state, modals and toasts.
- `src/timer.rs` writes the systemd user units; they always point at `/usr/bin/syncpair`.

## Checks

```bash
cargo test
cargo clippy --all-targets
```

## Installing changes on this machine

The owner runs the installed build at `/usr/bin/syncpair` (not a pacman package), and the daily timer runs that path too. To install the current working tree:

```bash
scripts/install-local.sh            # cargo test, release build, install, verify
scripts/install-local.sh --no-test
```

- Needs root. `sudo` asks for a password, so the script falls back to `pkexec`, which shows a password dialog on the owner's desktop. Tell the owner to expect it, and don't try to work around it.
- The timer doesn't need reinstalling after an update. An open app window keeps running the old binary until it's restarted; the script says when one is open.
- Only install when the owner asks for it.

## Where to look when a sync fails

- App log: `~/.local/state/syncpair/syncpair.log`. One line per event.
- rclone output of each failed folder: `~/.local/state/syncpair/failures/<unix-secs>.<nanos>.log`, named after the timestamp of its `[error]` log line. The Activity page's *Details* button opens it.
- Scheduled runs: `journalctl --user -u syncpair.service --since today`.
- bisync state and lock files: `~/.cache/rclone/bisync/`. A `*.lck` whose PID isn't a running `rclone` was left by an interrupted run. syncpair removes these automatically and retries once.
- Config: `~/.config/syncpair/syncs.toml`.

When testing sync behaviour, don't touch the owner's real folders. Use a sandbox: point `XDG_CONFIG_HOME`, `XDG_STATE_HOME` and `XDG_CACHE_HOME` at a temp dir, set `remote = ":local"`, and sync between two temp folders. Run it from a temp directory, because with `:local` the remote-side trash path is relative to the current directory.
