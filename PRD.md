# PRD

## Product State
- `syncpair` is a Rust desktop app (egui) around `rclone bisync`; running it without arguments opens the window.
- The app covers manual sync/preview/resync with live progress and cancel, in-app config editing (remote + folder pairs), log browsing, trash browsing with restore, and the `systemd --user` daily timer toggle. It launches no external editor, pager, or file manager.
- Headless subcommands (`sync`, `resync`, `timer`, `show-config`, `daily-sync`) remain for the timer and scripts.
- Sync definitions are read from `~/.config/syncpair/syncs.toml` and logs/state live under `~/.local/state/syncpair/`.

## Remaining Product Work
- Replace the hardcoded first-run remote default with a real first-run setup flow.
- Let the user choose the remote when the app creates its initial config instead of always writing `remote = "onedrive"`.
- Test the real runtime flow against a local `rclone` setup.
- Test timer install/remove in a real user session.
- Validate the Arch package locally with `makepkg`.

## Notes
- Normal runs use the `remote` value from `syncs.toml`, editable on the Folders page; the generated starter config still hardcodes `onedrive`.
- First-run remote configuration is planned work, not current behavior.
