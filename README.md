# syncpair

Desktop app around `rclone bisync` for personal two-way backups.

Running `syncpair` opens a native window (egui, Wayland and X11) with four pages:

- **Overview**: last sync result, *Sync now* with live per-folder progress and cancel, preview/resync actions, the daily schedule toggle, and recent run history.
- **Folders**: edit the remote and the folder pairs in place, with a folder picker; saving keeps comments in `syncs.toml`.
- **Activity**: the sync log, filterable by runs/problems and searchable.
- **Trash**: files a sync replaced or deleted, grouped by day, with per-file restore.

Everything happens inside the app; it never launches an editor, pager, or file manager.
Notifications go straight to the desktop over D-Bus.

Headless commands, used by the daily timer and scripts:

```bash
syncpair sync [--dry-run]
syncpair resync [--dry-run] [--mode newer|older]
syncpair timer install|remove|status
syncpair show-config
syncpair daily-sync      # what syncpair.service runs
```

## Develop

Requirements:

- `rclone`
- `systemctl` (user session) for the daily timer
- `xdg-desktop-portal` for the folder picker (optional; paths can be typed)

Run in dev mode:

```bash
cargo run
```

The config file lives at:

```text
~/.config/syncpair/syncs.toml
```

Remote selection note:

- Sync runs use the `remote` value from `syncs.toml`; change it on the **Folders** page.
- On first run, the app still generates a starter config with a hardcoded `remote = "onedrive"`.

## Test

```bash
cargo check
cargo test
```

You can also test the app manually with:

```bash
cargo run
```

## Install

Build:

```bash
cargo build --release
```

Install the binary to the path used by the timer:

```bash
sudo install -m 755 target/release/syncpair /usr/bin/syncpair
```

Run the app:

```bash
syncpair
```

If you use the daily timer, install or reinstall it from the app after updating `/usr/bin/syncpair`.

To test the timer manually without waiting for the daily schedule:

```bash
systemctl --user start syncpair.service
journalctl --user -u syncpair.service --since "10 minutes ago"
```
