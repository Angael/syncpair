use anyhow::{bail, Context, Result};
use chrono::{Local, Months, NaiveDate};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::utils::{expand_home, path_slug};

/// Local trash layout written by sync runs: `<trash>/<YYYY-MM-DD>/<slug of local path>/<relative file>`.
pub struct TrashDay {
    pub date: String,
    pub folders: Vec<TrashFolder>,
}

pub struct TrashFolder {
    /// Directory name inside the dated folder (slug of the sync entry's local path).
    pub slug: String,
    /// Matching `local` from the config, when one still exists.
    pub local: Option<String>,
    pub files: Vec<TrashFile>,
    pub total_bytes: u64,
}

pub struct TrashFile {
    pub path: PathBuf,
    pub relative: PathBuf,
    pub bytes: u64,
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

/// Lists trash contents, newest day first.
pub fn scan_trash(trash_dir: &Path, config: Option<&Config>) -> Result<Vec<TrashDay>> {
    let mut days = Vec::new();
    if !trash_dir.exists() {
        return Ok(days);
    }

    for day in fs::read_dir(trash_dir)? {
        let day = day?;
        if !day.file_type()?.is_dir() {
            continue;
        }
        let mut folders = Vec::new();
        for folder in fs::read_dir(day.path())? {
            let folder = folder?;
            if !folder.file_type()?.is_dir() {
                continue;
            }
            let slug = folder.file_name().to_string_lossy().into_owned();
            let local = config.and_then(|config| {
                config
                    .sync
                    .iter()
                    .find(|entry| path_slug(&entry.local) == slug)
                    .map(|entry| entry.local.clone())
            });
            let root = folder.path();
            let mut files = Vec::new();
            collect_files(&root, &root, &mut files)?;
            if files.is_empty() {
                // Every run pre-creates its backup dir; it stays empty unless rclone moved files in.
                continue;
            }
            files.sort_by(|a, b| a.relative.cmp(&b.relative));
            let total_bytes = files.iter().map(|file| file.bytes).sum();
            folders.push(TrashFolder { slug, local, files, total_bytes });
        }
        folders.sort_by(|a, b| a.slug.cmp(&b.slug));
        days.push(TrashDay {
            date: day.file_name().to_string_lossy().into_owned(),
            folders,
        });
    }

    days.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(days)
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<TrashFile>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_dir() {
            collect_files(root, &path, out)?;
        } else {
            let bytes = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            out.push(TrashFile { path, relative, bytes });
        }
    }
    Ok(())
}

/// Moves a trashed file back to `<local>/<relative>`. Refuses to overwrite an existing file.
pub fn restore_file(file: &TrashFile, local: &str) -> Result<PathBuf> {
    let destination = expand_home(local)?.join(&file.relative);
    if destination.exists() {
        bail!("{} already exists", destination.display());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::rename(&file.path, &destination).is_err() {
        // Trash and target may sit on different filesystems.
        fs::copy(&file.path, &destination)
            .with_context(|| format!("Failed to restore {}", destination.display()))?;
        fs::remove_file(&file.path)?;
    }
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SyncEntry;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("syncpair-{tag}-{unique}"))
    }

    #[test]
    fn cleanup_removes_empty_and_expired_trash_directories() {
        let trash = temp_dir("cleanup");
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

    #[test]
    fn scan_maps_slugs_to_entries_and_restore_never_overwrites() {
        let root = temp_dir("trash");
        let trash = root.join("trash");
        let local = root.join("my notes");
        let local_str = local.to_string_lossy().into_owned();
        let slug = path_slug(&local_str);
        fs::create_dir_all(trash.join("2026-01-02").join(&slug).join("sub")).unwrap();
        fs::write(trash.join("2026-01-02").join(&slug).join("sub/a.txt"), "aa").unwrap();
        fs::write(trash.join("2026-01-02").join(&slug).join("b.txt"), "b").unwrap();
        fs::create_dir_all(trash.join("2026-03-04")).unwrap();
        fs::create_dir_all(local.join("sub")).unwrap();
        fs::write(local.join("b.txt"), "current").unwrap();

        let config = Config {
            remote: "r".into(),
            sync: vec![SyncEntry { local: local_str.clone(), remote: "notes".into() }],
        };
        let days = scan_trash(&trash, Some(&config)).unwrap();
        assert_eq!(days[0].date, "2026-03-04");
        let folder = &days[1].folders[0];
        assert_eq!(folder.local.as_deref(), Some(local_str.as_str()));
        assert_eq!(folder.total_bytes, 3);
        assert_eq!(folder.files[0].relative, PathBuf::from("b.txt"));

        assert!(restore_file(&folder.files[0], &local_str).is_err());
        assert_eq!(fs::read_to_string(local.join("b.txt")).unwrap(), "current");

        restore_file(&folder.files[1], &local_str).unwrap();
        assert_eq!(fs::read_to_string(local.join("sub/a.txt")).unwrap(), "aa");
        assert!(!folder.files[1].path.exists());

        fs::remove_dir_all(root).unwrap();
    }
}
