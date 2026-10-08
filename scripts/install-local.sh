#!/usr/bin/env bash
# Builds the working tree and installs it over this machine's /usr/bin/syncpair.
#
#   scripts/install-local.sh            test, build, install
#   scripts/install-local.sh --no-test  skip `cargo test`
#
# Root is obtained with sudo when it needs no password, otherwise with pkexec (a desktop
# password dialog, so it also works from a non-interactive shell), otherwise interactive sudo.
set -euo pipefail

cd "$(dirname "$0")/.."
repo="$PWD"
run_tests=1
for arg in "$@"; do
  case "$arg" in
    --no-test) run_tests=0 ;;
    -h|--help) sed -n '2,8p' "$0"; exit 0 ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

step() { printf '\n==> %s\n' "$*"; }

if (( run_tests )); then
  step "cargo test"
  cargo test --locked
fi

step "cargo build --release"
cargo build --release --locked

step "installing to /usr (binary, desktop entry, icons)"
installer="$repo/packaging/install.sh"
if (( EUID == 0 )); then
  "$installer"
elif sudo -n true 2>/dev/null; then
  sudo "$installer"
elif command -v pkexec >/dev/null && [[ -n "${WAYLAND_DISPLAY:-}${DISPLAY:-}" ]]; then
  echo "A password dialog should appear on the desktop."
  pkexec "$installer"
else
  sudo "$installer"
fi

step "verifying"
if ! cmp -s target/release/syncpair /usr/bin/syncpair; then
  echo "error: /usr/bin/syncpair does not match target/release/syncpair" >&2
  exit 1
fi
echo "/usr/bin/syncpair now matches $(git rev-parse --short HEAD)$(git diff --quiet HEAD -- || echo '+uncommitted changes')"

# The timer runs /usr/bin/syncpair, so it picks up the new binary without reinstalling.
/usr/bin/syncpair timer status || true

if systemctl --user is-active --quiet syncpair.service; then
  echo "note: a scheduled sync is running; it finishes on the old binary."
fi
if pgrep -x syncpair >/dev/null; then
  echo "note: syncpair is open ($(pgrep -x syncpair | tr '\n' ' ')); restart it to use the new version."
fi
