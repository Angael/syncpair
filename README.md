# syncpair

Rust wrapper around `rclone bisync` for personal sync jobs.

## Develop

Requirements:

- `rclone`
- `notify-send`
- `systemctl`
- `code` for `Edit Config`
- `less` for `View Logs`

Run in dev mode:

```bash
cargo run
```

The config file lives at:

```text
~/.config/syncpair/syncs.toml
```

Remote selection note:

- Current sync runs use the `remote` value from `syncs.toml`.
- On first run, the app still generates a starter config with a hardcoded `remote = "onedrive"`.
- A proper first-run remote setup flow, and an easier way to change the remote later, are planned for a later update.

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
