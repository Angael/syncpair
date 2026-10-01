# Publishing Guide

This project can be published in a way that later lets you install it with:

```bash
paru -S syncpair
```

That path goes through the AUR.

## Current Project Assumptions

These are important and already reflected in the codebase:

- binary name: `syncpair`
- installed binary path expected by timer logic: `/usr/bin/syncpair`
- config path: `~/.config/syncpair/syncs.toml`
- state/log path: `~/.local/state/syncpair/`
- license: `Unlicense`
- required runtime commands: `rclone`, `systemctl`
- required Arch runtime package dependencies for packaging: `rclone` plus the windowing/GL libraries egui loads at runtime (`libglvnd`, `libxkbcommon`, `wayland`, `libx11`, `libxcursor`, `libxi`, `libxrandr`); `xdg-desktop-portal` optional for the folder picker
- starter package template exists at `packaging/PKGBUILD`

## Why The Timer Uses `/usr/bin/syncpair`

The app can install a `systemd --user` timer.

That timer must point to a stable path.

Bad example:

```text
/home/vanih/repos/syncpair/target/debug/syncpair
```

That path is only valid for a local development build and can break after a rebuild or cleanup.

Good example:

```text
/usr/bin/syncpair
```

That is the stable installed path expected on an Arch package install, so the timer logic writes that path intentionally.

## How AUR Packaging Works

The AUR does not host your built binary directly.

Instead, it hosts packaging files, mainly:

- `PKGBUILD`
- `.SRCINFO`

Those files tell Arch users how to fetch your source, build it, and install it.

`paru -S syncpair` works once:

1. your source code is published somewhere public, usually GitHub
2. an AUR package named `syncpair` exists
3. the `PKGBUILD` points to your release tarball or git repo

## Recommended Release Model

Use a normal source-release model first.

Recommended:

1. Host `syncpair` on GitHub
2. Tag releases like `v0.1.0`
3. Let the AUR package build from the tagged source tarball

This is simpler and more stable than starting with a `-git` package.

You can add `syncpair-git` later if you want rolling installs.

## Suggested Repo Contents Before Publishing

Have these ready:

- `Cargo.toml`
- `Cargo.lock`
- `src/`
- `README.md`
- `LICENSE`
- `syncs.toml.example`

Also recommended:

- make the default config path and runtime dependencies clear in `README.md`
- make it clear that first-run remote setup is not implemented yet and the starter config still hardcodes `onedrive`
- mention required external tools: `rclone`, `systemctl`
- keep `Cargo.lock` committed so packaging stays reproducible

## Release Steps

1. Push the project to GitHub
2. Create a tag, for example `v0.1.0`
3. Create a GitHub release for that tag
4. Download the source tarball URL for the tag
5. Compute its checksum with `b2sum`

Example:

```bash
b2sum syncpair-0.1.0.tar.gz
```

## Create the AUR Package

Create a separate local directory for the AUR package files.

Example package metadata direction:

- package name: `syncpair`
- binary name: `syncpair`
- build dependency: `cargo`
- runtime dependencies: `rclone`, `libglvnd`, `libxkbcommon`, `wayland`, `libx11`, `libxcursor`, `libxi`, `libxrandr`
- license: `Unlicense`
- installed binary path for timer use: `/usr/bin/syncpair`

There is a starter template in `packaging/PKGBUILD`.

Minimal `PKGBUILD` shape:

```bash
pkgname=syncpair
pkgver=0.2.0
pkgrel=1
pkgdesc="Desktop app for personal two-way rclone bisync backups"
arch=('x86_64')
url="https://github.com/YOUR_USER/syncpair"
license=('Unlicense')
depends=('rclone' 'hicolor-icon-theme' 'libglvnd' 'libxkbcommon' 'wayland' 'libx11' 'libxcursor' 'libxi' 'libxrandr')
optdepends=('xdg-desktop-portal: native folder picker in the Folders editor')
makedepends=('cargo')
source=("$pkgname-$pkgver.tar.gz::$url/archive/refs/tags/v$pkgver.tar.gz")
b2sums=('REPLACE_WITH_REAL_SUM')

prepare() {
  export CARGO_TARGET_DIR=target
  cargo fetch --locked
}

build() {
  export CARGO_TARGET_DIR=target
  cargo build --frozen --release
}

check() {
  export CARGO_TARGET_DIR=target
  cargo test --frozen
}

package() {
  install -Dm755 "target/release/syncpair" "$pkgdir/usr/bin/syncpair"
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
  install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
  install -Dm644 syncs.toml.example "$pkgdir/usr/share/doc/$pkgname/syncs.toml.example"
  install -Dm644 packaging/syncpair.desktop "$pkgdir/usr/share/applications/syncpair.desktop"
  for size in 16 32 48 64 128 256 512; do
    install -Dm644 "packaging/icons/hicolor/${size}x${size}/apps/syncpair.png" \
      "$pkgdir/usr/share/icons/hicolor/${size}x${size}/apps/syncpair.png"
  done
}
```

Notes:

- the egui crates require rustc 1.92 or newer
- the current timer generation logic assumes packaged installs provide `/usr/bin/syncpair`
- the app does read the configured remote from `syncs.toml` (editable on the Folders page), but first-run setup still writes a hardcoded `remote = "onedrive"`
- a first-run remote chooser is planned work for later
- `packaging/syncpair.desktop` is installed so the app shows up in the application menu

## Generate `.SRCINFO`

In the AUR package directory:

```bash
makepkg --printsrcinfo > .SRCINFO
```

## Publish To AUR

1. create an account on `aur.archlinux.org`
2. set up SSH auth for AUR
3. clone the AUR repo for your package name

Example:

```bash
git clone ssh://aur@aur.archlinux.org/syncpair.git
```

4. copy `PKGBUILD` and `.SRCINFO` into that repo
5. commit and push

Example:

```bash
git add PKGBUILD .SRCINFO
git commit -m "Initial release"
git push
```

## Install With Paru

After the AUR package is published:

```bash
paru -S syncpair
```

## Practical Flow

Use this exact path the first time:

1. Push this repo to GitHub
2. Replace `YOUR_USER` in `packaging/PKGBUILD`
3. Tag and publish `v0.1.0`
4. Download the release tarball and compute its `b2sum`
5. Replace `REPLACE_WITH_REAL_SUM` in `packaging/PKGBUILD`
6. Clone the AUR repo with `git clone ssh://aur@aur.archlinux.org/syncpair.git`
7. Copy `packaging/PKGBUILD` into that AUR repo as `PKGBUILD`
8. Run `makepkg --printsrcinfo > .SRCINFO`
9. Optionally run `makepkg -si` locally to verify the package builds and installs
10. Commit and push the AUR repo
11. Install with `paru -S syncpair`

## Practical Commands

Example end-to-end shell flow after you have a GitHub release:

```bash
git clone ssh://aur@aur.archlinux.org/syncpair.git
cp packaging/PKGBUILD syncpair/PKGBUILD
cd syncpair
makepkg --printsrcinfo > .SRCINFO
makepkg -si
git add PKGBUILD .SRCINFO
git commit -m "Initial release"
git push
paru -S syncpair
```

## What To Improve Before AUR

Before publishing, it would be good to:

1. test `makepkg` locally on an Arch machine
2. make sure the GitHub release tarball URL matches the real repo path
3. replace the placeholder checksum in `packaging/PKGBUILD`
4. optionally run `namcap PKGBUILD` and `namcap *.pkg.tar.zst`
5. run a real installed-package check that timer install writes a working user service

## Local Test Checklist Before Publishing

Do these before opening a fresh session or publishing:

1. `cargo build`
2. `cargo run`
3. confirm the window opens and the Folders page saves `syncs.toml` without losing comments
4. confirm desktop notifications appear for `syncpair sync`
5. confirm `syncpair timer install` writes a user service with `/usr/bin/syncpair daily-sync`
6. test `makepkg -si` with the final `PKGBUILD`

## Recommended Packaging Direction

Best first version:

- publish source on GitHub
- create `syncpair` AUR package
- build from release tarballs
- keep dependencies explicit and small

That is the simplest path to reliable `paru -S syncpair` installs on your machines.

## If You Reopen This Project Later

The most important repo files to read first are:

1. `README.md`
2. `PRD.md`
3. `PUBLISHING.md`
4. `packaging/PKGBUILD`
5. `src/timer.rs`

Those capture the current product direction, packaging assumptions, and the timer-path decision.
