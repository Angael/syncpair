#!/usr/bin/env bash
# Installs a release build system-wide, mirroring packaging/PKGBUILD.
# Run from the repo root after `cargo build --release`:  sudo packaging/install.sh
set -euo pipefail

cd "$(dirname "$0")/.."
prefix="${PREFIX:-/usr}"

install -Dm755 target/release/syncpair "$prefix/bin/syncpair"
install -Dm644 packaging/syncpair.desktop "$prefix/share/applications/syncpair.desktop"
for size in 16 32 48 64 128 256 512; do
  install -Dm644 "packaging/icons/hicolor/${size}x${size}/apps/syncpair.png" \
    "$prefix/share/icons/hicolor/${size}x${size}/apps/syncpair.png"
done

# Refresh caches so the launcher picks up the entry and icon without a re-login.
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -qtf "$prefix/share/icons/hicolor" || true
command -v update-desktop-database >/dev/null && update-desktop-database -q "$prefix/share/applications" || true
