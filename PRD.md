# PRD

## Product State
- `syncpair` is already a working Rust CLI wrapper around `rclone bisync`.
- Current commands cover interactive use, direct sync/resync runs, log viewing, config editing, trash opening, and `systemd --user` timer install/remove/status.
- Sync definitions are read from `~/.config/syncpair/syncs.toml` and logs/state live under `~/.local/state/syncpair/`.

## Remaining Product Work
- Replace the hardcoded first-run remote default with a real first-run setup flow.
- Let the user choose the remote when the app creates its initial config instead of always writing `remote = "onedrive"`.
- Add a supported way to change the configured remote later without manually editing the config.
- Test the real runtime flow against a local `rclone` setup.
- Test timer install/remove in a real user session.
- Validate the Arch package locally with `makepkg`.

## Notes
- Remote selection is only partially implemented today: normal runs use the `remote` value from `syncs.toml`, but the generated starter config still hardcodes `onedrive`.
- First-run remote configuration and later remote switching are planned work, not current behavior.
