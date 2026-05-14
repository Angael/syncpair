use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;

use crate::paths::AppPaths;
use crate::utils::expand_home;

#[derive(Deserialize)]
pub struct Config {
    pub remote: String,
    #[serde(default)]
    pub sync: Vec<SyncEntry>,
}

#[derive(Clone, Deserialize)]
pub struct SyncEntry {
    pub local: String,
    pub remote: String,
}

pub fn load_config(paths: &AppPaths) -> Result<Config> {
    if !paths.config_file.exists() {
        write_default_config(paths)?;
    }

    let raw = fs::read_to_string(&paths.config_file)
        .with_context(|| format!("Failed to read {}", paths.config_file.display()))?;
    let config: Config = toml::from_str(&raw)
        .with_context(|| format!("Failed to parse {}", paths.config_file.display()))?;
    Ok(config)
}

pub fn validate_config(config: &Config) -> Result<()> {
    if config.remote.trim().is_empty() {
        bail!("Config remote must not be empty");
    }
    if config.sync.is_empty() {
        bail!("Config must contain at least one [[sync]] entry");
    }

    let mut local_seen = BTreeSet::new();
    let mut remote_seen = BTreeSet::new();

    for entry in &config.sync {
        if entry.local.trim().is_empty() {
            bail!("Sync entry local path must not be empty");
        }
        if entry.remote.trim().is_empty() {
            bail!("Sync entry remote path must not be empty");
        }

        let local_key = expand_home(&entry.local)?.to_string_lossy().to_string();
        let remote_key = format!("{}:{}", config.remote, entry.remote);

        if !local_seen.insert(local_key) {
            bail!("Duplicate local path in config: {}", entry.local);
        }
        if !remote_seen.insert(remote_key.clone()) {
            bail!("Duplicate remote target in config: {}", remote_key);
        }
    }

    Ok(())
}

pub fn show_config(paths: &AppPaths) -> Result<()> {
    let config = load_config(paths)?;
    validate_config(&config)?;
    println!("remote = {}", config.remote);
    for entry in config.sync {
        println!("{}  {}:{}", entry.local, config.remote, entry.remote);
    }
    Ok(())
}

fn write_default_config(paths: &AppPaths) -> Result<()> {
    fs::create_dir_all(&paths.config_dir)?;
    fs::write(
        &paths.config_file,
        "remote = \"onedrive\"\n\n[[sync]]\nlocal = \"~/notes\"\nremote = \"notes\"\n",
    )?;
    Ok(())
}
