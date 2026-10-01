use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use toml_edit::{value, ArrayOfTables, DocumentMut, Item, Table};

use crate::paths::AppPaths;
use crate::utils::expand_home;

#[derive(Clone, Deserialize, PartialEq)]
pub struct Config {
    pub remote: String,
    #[serde(default)]
    pub sync: Vec<SyncEntry>,
}

#[derive(Clone, Deserialize, PartialEq)]
pub struct SyncEntry {
    pub local: String,
    pub remote: String,
}

impl Config {
    pub fn remote_target(&self, entry: &SyncEntry) -> String {
        format!("{}:{}", self.remote, entry.remote)
    }
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
        let remote_key = config.remote_target(entry);

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
    for entry in &config.sync {
        println!("{}  {}", entry.local, config.remote_target(entry));
    }
    Ok(())
}

/// Writes `config` back to disk, keeping comments and formatting of the existing file.
/// `[[sync]]` tables are matched to entries by their `local` path, so a comment above a
/// table follows its entry when entries are reordered.
pub fn save_config(paths: &AppPaths, config: &Config) -> Result<()> {
    validate_config(config)?;

    let raw = fs::read_to_string(&paths.config_file).unwrap_or_default();
    let mut doc: DocumentMut = raw.parse().unwrap_or_default();
    let mut previous: Vec<Option<Table>> = doc
        .remove("sync")
        .and_then(|item| item.into_array_of_tables().ok())
        .map(|tables| tables.into_iter().map(Some).collect())
        .unwrap_or_default();

    doc["remote"] = value(config.remote.trim());

    // toml_edit emits tables by their recorded document position, not array order; renumber
    // from where the first `[[sync]]` used to be so reordering in the editor sticks.
    let first_position = previous
        .iter()
        .filter_map(|slot| slot.as_ref()?.position())
        .min()
        .unwrap_or(1);
    let mut tables = ArrayOfTables::new();
    for (offset, entry) in config.sync.iter().enumerate() {
        let reused = previous.iter_mut().find(|slot| {
            slot.as_ref()
                .and_then(|table| table.get("local"))
                .and_then(Item::as_str)
                .is_some_and(|local| local == entry.local)
        });
        let mut table = reused.and_then(Option::take).unwrap_or_default();
        table.set_position(Some(first_position + offset as isize));
        table["local"] = value(entry.local.trim());
        table["remote"] = value(entry.remote.trim());
        tables.push(table);
    }
    doc.insert("sync", Item::ArrayOfTables(tables));

    fs::create_dir_all(&paths.config_dir)?;
    let tmp = paths.config_file.with_extension("toml.tmp");
    fs::write(&tmp, doc.to_string())
        .with_context(|| format!("Failed to write {}", tmp.display()))?;
    fs::rename(&tmp, &paths.config_file)
        .with_context(|| format!("Failed to replace {}", paths.config_file.display()))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_paths() -> (AppPaths, PathBuf) {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("syncpair-config-{unique}"));
        (AppPaths::rooted(&root), root)
    }

    #[test]
    fn save_keeps_comments_attached_to_reordered_entries() {
        let (paths, root) = temp_paths();
        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(
            &paths.config_file,
            "# top comment\nremote = \"onedrive\"\n\n[[sync]]\nlocal = \"~/a\"\nremote = \"a\"\n\n# big one\n[[sync]]\nlocal = \"~/b\"\nremote = \"b\"\n",
        )
        .unwrap();

        let config = Config {
            remote: "gdrive".into(),
            sync: vec![
                SyncEntry { local: "~/c".into(), remote: "c".into() },
                SyncEntry { local: "~/b".into(), remote: "b2".into() },
                SyncEntry { local: "~/a".into(), remote: "a".into() },
            ],
        };
        save_config(&paths, &config).unwrap();

        let written = fs::read_to_string(&paths.config_file).unwrap();
        assert!(written.starts_with("# top comment\nremote = \"gdrive\""));
        assert!(written.contains("# big one\n[[sync]]\nlocal = \"~/b\"\nremote = \"b2\""));
        let order: Vec<usize> = ["~/c", "~/b", "~/a"]
            .iter()
            .map(|local| written.find(&format!("local = \"{local}\"")).unwrap())
            .collect();
        assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{written}");
        assert!(load_config(&paths).unwrap() == config);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn save_rejects_invalid_config_without_touching_file() {
        let (paths, root) = temp_paths();
        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(&paths.config_file, "remote = \"x\"\n").unwrap();

        let config = Config {
            remote: "x".into(),
            sync: vec![
                SyncEntry { local: "~/a".into(), remote: "a".into() },
                SyncEntry { local: "~/a".into(), remote: "b".into() },
            ],
        };
        assert!(save_config(&paths, &config).is_err());
        assert_eq!(fs::read_to_string(&paths.config_file).unwrap(), "remote = \"x\"\n");

        fs::remove_dir_all(root).unwrap();
    }
}
