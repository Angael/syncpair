use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub const APP_NAME: &str = "syncpair";
pub const CONFIG_FILE_NAME: &str = "syncs.toml";
pub const LOG_FILE_NAME: &str = "syncpair.log";
pub const SERVICE_FILE_NAME: &str = "syncpair.service";
pub const TIMER_FILE_NAME: &str = "syncpair.timer";

#[derive(Clone)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
    pub state_dir: PathBuf,
    pub log_file: PathBuf,
    pub trash_dir: PathBuf,
    pub systemd_dir: PathBuf,
    pub service_file: PathBuf,
    pub timer_file: PathBuf,
}

pub fn app_paths() -> Result<AppPaths> {
    let config_root = dirs::config_dir().context("Missing XDG config dir")?;
    let state_root = dirs::state_dir().unwrap_or_else(|| config_root.join("../state"));
    Ok(AppPaths::new(&config_root, &state_root))
}

impl AppPaths {
    fn new(config_root: &Path, state_root: &Path) -> Self {
        let config_dir = config_root.join(APP_NAME);
        let state_dir = state_root.join(APP_NAME);
        let systemd_dir = config_root.join("systemd/user");

        AppPaths {
            config_file: config_dir.join(CONFIG_FILE_NAME),
            config_dir,
            log_file: state_dir.join(LOG_FILE_NAME),
            trash_dir: state_dir.join("trash"),
            state_dir,
            service_file: systemd_dir.join(SERVICE_FILE_NAME),
            timer_file: systemd_dir.join(TIMER_FILE_NAME),
            systemd_dir,
        }
    }

    #[cfg(test)]
    pub fn rooted(root: &Path) -> Self {
        Self::new(&root.join("config"), &root.join("state"))
    }
}
