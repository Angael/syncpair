use anyhow::{Context, Result, anyhow, bail};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::logs::touch_file;
use crate::paths::AppPaths;

pub fn notify(summary: &str, body: &str, error: bool) -> Result<()> {
    let mut command = Command::new("notify-send");
    if error {
        command.args(["-t", "10000"]);
    }

    // User services launched by systemd may miss the session bus environment.
    let runtime_dir = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .or_else(dirs::runtime_dir);

    if let Some(runtime_dir) = runtime_dir {
        command.env("XDG_RUNTIME_DIR", &runtime_dir);
        if env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
            let bus_address = runtime_dir.join("bus");
            command.env("DBUS_SESSION_BUS_ADDRESS", format!("unix:path={}", bus_address.display()));
        }
    }

    let status = command.arg(summary).arg(body).status();
    match status {
        Ok(_) | Err(_) => Ok(()),
    }
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

pub fn open_trash(paths: &AppPaths) -> Result<()> {
    fs::create_dir_all(&paths.trash_dir)?;
    try_open(["xdg-open", paths.trash_dir.to_string_lossy().as_ref()])
        .or_else(|_| try_open(["dolphin", paths.trash_dir.to_string_lossy().as_ref()]))
}

pub fn open_in_code(path: &Path) -> Result<()> {
    touch_file(path)?;
    try_open(["code", path.to_string_lossy().as_ref()])
}

fn try_open<const N: usize>(args: [&str; N]) -> Result<()> {
    let status = Command::new(args[0])
        .args(&args[1..])
        .status()
        .with_context(|| format!("Failed to run {}", args[0]))?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("{} exited unsuccessfully", args[0]))
    }
}
