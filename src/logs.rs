use anyhow::{Result, anyhow};
use chrono::Local;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::Command;

use crate::paths::AppPaths;

pub fn ensure_parent_dirs(paths: &AppPaths) -> Result<()> {
    fs::create_dir_all(&paths.config_dir)?;
    fs::create_dir_all(&paths.state_dir)?;
    touch_file(&paths.log_file)?;
    Ok(())
}

pub fn touch_file(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if !path.exists() {
        File::create(path)?;
    }
    Ok(())
}

pub fn log_line(paths: &AppPaths, level: &str, message: &str) -> Result<()> {
    touch_file(&paths.log_file)?;
    let mut file = OpenOptions::new().append(true).open(&paths.log_file)?;
    writeln!(file, "{} [{}] {}", Local::now().to_rfc3339(), level, message)?;
    Ok(())
}

pub fn open_log(paths: &AppPaths) -> Result<()> {
    touch_file(&paths.log_file)?;

    let pager = [
        ["less", "-+F", &paths.log_file.to_string_lossy()],
        ["more", &paths.log_file.to_string_lossy(), ""],
    ];

    for args in pager {
        let mut command = Command::new(args[0]);
        for arg in args[1..].iter().filter(|arg| !arg.is_empty()) {
            command.arg(arg);
        }

        match command.status() {
            Ok(status) if status.success() => return Ok(()),
            Ok(_) => return Err(anyhow!("{} exited unsuccessfully", args[0])),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        }
    }

    Err(anyhow!("No supported pager found. Install less or more."))
}
