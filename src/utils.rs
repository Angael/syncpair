use anyhow::{anyhow, bail, Context, Result};
use chrono::{Local, Months, NaiveDate};
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
            command.env(
                "DBUS_SESSION_BUS_ADDRESS",
                format!("unix:path={}", bus_address.display()),
            );
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

pub fn cleanup_trash(trash_dir: &Path) -> Result<()> {
    if !trash_dir.exists() {
        return Ok(());
    }

    let cutoff = Local::now()
        .date_naive()
        .checked_sub_months(Months::new(2))
        .context("Could not calculate trash retention cutoff")?;

    for entry in fs::read_dir(trash_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let dated_before_cutoff = entry
            .file_name()
            .to_str()
            .and_then(|name| NaiveDate::parse_from_str(name, "%Y-%m-%d").ok())
            .is_some_and(|date| date < cutoff);

        if dated_before_cutoff {
            fs::remove_dir_all(path)?;
        } else {
            remove_empty_dirs(&path)?;
        }
    }

    Ok(())
}

fn remove_empty_dirs(dir: &Path) -> Result<bool> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            remove_empty_dirs(&entry.path())?;
        }
    }

    if fs::read_dir(dir)?.next().is_none() {
        fs::remove_dir(dir)?;
        Ok(true)
    } else {
        Ok(false)
    }
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

#[cfg(test)]
mod tests {
    use super::cleanup_trash;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn cleanup_removes_empty_and_expired_trash_directories() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let trash = std::env::temp_dir().join(format!("syncpair-cleanup-{unique}"));
        let expired = trash.join("2000-01-01");
        let recent = trash.join("2999-01-01");

        fs::create_dir_all(expired.join("contains-data")).unwrap();
        fs::write(expired.join("contains-data/file"), "old").unwrap();
        fs::create_dir_all(recent.join("empty/nested")).unwrap();
        fs::create_dir_all(recent.join("contains-data")).unwrap();
        fs::write(recent.join("contains-data/file"), "keep").unwrap();

        cleanup_trash(&trash).unwrap();

        assert!(!expired.exists());
        assert!(!recent.join("empty").exists());
        assert_eq!(
            fs::read_to_string(recent.join("contains-data/file")).unwrap(),
            "keep"
        );

        fs::remove_dir_all(trash).unwrap();
    }
}
