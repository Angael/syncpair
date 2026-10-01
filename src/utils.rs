use anyhow::{anyhow, bail, Context, Result};
use notify_rust::{Notification, Timeout};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread::{self, JoinHandle};

pub const APP_ICON: &str = "folder-sync";

/// Sends a desktop notification over D-Bus on a background thread.
///
/// At boot the notification daemon may not be up yet and D-Bus activation can block for
/// tens of seconds; running off-thread keeps that from delaying the sync itself.
/// Join the handle when the process is about to exit and the notification must be sent.
pub fn notify(summary: &str, body: &str, error: bool) -> JoinHandle<()> {
    let mut notification = Notification::new();
    notification
        .appname("Syncpair")
        .icon(APP_ICON)
        .summary(summary)
        .body(body);
    if error {
        notification.timeout(Timeout::Milliseconds(10_000));
    }

    thread::spawn(move || {
        if let Err(error) = notification.show() {
            eprintln!("Failed to show notification: {error}");
        }
    })
}

pub fn require_command(command: &str) -> Result<()> {
    let path_var = env::var_os("PATH").ok_or_else(|| anyhow!("PATH is not set"))?;
    for path in env::split_paths(&path_var) {
        if path.join(command).exists() {
            return Ok(());
        }
    }
    bail!("Missing required command: {command}")
}

pub fn expand_home(path: &str) -> Result<PathBuf> {
    if let Some(stripped) = path.strip_prefix("~/") {
        let home = dirs::home_dir().context("Missing home directory")?;
        return Ok(home.join(stripped));
    }
    if path == "~" {
        return dirs::home_dir().context("Missing home directory");
    }
    Ok(PathBuf::from(path))
}

/// Inverse of [`expand_home`] for display and storage: `/home/u/notes` -> `~/notes`.
pub fn contract_home(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(rest) = path.strip_prefix(&home) {
            return if rest.as_os_str().is_empty() {
                "~".into()
            } else {
                format!("~/{}", rest.display())
            };
        }
    }
    path.display().to_string()
}

pub fn path_slug(path: &str) -> String {
    path.chars()
        .map(|ch| match ch {
            '/' | ' ' => '_',
            c if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' => c,
            _ => '_',
        })
        .collect()
}

pub fn remove_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
