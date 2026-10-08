use anyhow::Result;
use chrono::{DateTime, FixedOffset, Local};
use std::fs::{self, File, OpenOptions};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::paths::AppPaths;

/// Saved rclone transcripts of failed folders kept before the oldest are deleted.
const FAILURES_KEPT: usize = 100;

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
    write_line(paths, &Local::now().fixed_offset(), level, message)
}

/// Logs an error and saves the rclone output behind it, shown as *Details* on the Activity page.
pub fn log_failure(paths: &AppPaths, message: &str, output: &[String]) -> Result<()> {
    let time = Local::now().fixed_offset();
    write_line(paths, &time, "error", message)?;
    fs::create_dir_all(&paths.failures_dir)?;
    let mut text = output.join("\n");
    text.push('\n');
    fs::write(paths.failures_dir.join(failure_file_name(&time)), text)?;
    prune_failures(&paths.failures_dir)
}

fn write_line(paths: &AppPaths, time: &DateTime<FixedOffset>, level: &str, message: &str) -> Result<()> {
    touch_file(&paths.log_file)?;
    let mut file = OpenOptions::new().append(true).open(&paths.log_file)?;
    writeln!(file, "{} [{}] {}", time.to_rfc3339(), level, message)?;
    Ok(())
}

/// Offset-independent, so the name can be rebuilt from the parsed log line.
fn failure_file_name(time: &DateTime<FixedOffset>) -> String {
    format!("{}.{:09}.log", time.timestamp(), time.timestamp_subsec_nanos())
}

fn prune_failures(dir: &Path) -> Result<()> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "log"))
        .collect();
    if files.len() <= FAILURES_KEPT {
        return Ok(());
    }
    // Names start with the unix timestamp, so lexical order is chronological.
    files.sort();
    for path in &files[..files.len() - FAILURES_KEPT] {
        let _ = fs::remove_file(path);
    }
    Ok(())
}

pub struct LogEntry {
    pub time: Option<DateTime<FixedOffset>>,
    pub level: String,
    pub message: String,
    /// Saved rclone output for a failed folder, if it was kept.
    pub details: Option<PathBuf>,
}

impl LogEntry {
    /// A finished real (non-preview) sync or resync run.
    pub fn is_run_result(&self) -> bool {
        (self.level == "success" || self.level == "error")
            && self.message.contains("finished")
            && !self.message.contains("(preview)")
    }
}

/// Parses `<rfc3339> [level] message` lines. Lines without that shape (raw rclone output
/// from older versions) become entries with an empty level and no time.
pub fn read_log(paths: &AppPaths) -> Result<Vec<LogEntry>> {
    let raw = fs::read_to_string(&paths.log_file).unwrap_or_default();
    let saved: HashSet<String> = fs::read_dir(&paths.failures_dir)
        .map(|dir| dir.filter_map(|entry| entry.ok()?.file_name().into_string().ok()).collect())
        .unwrap_or_default();
    Ok(raw
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut entry = parse_line(line);
            entry.details = entry
                .time
                .as_ref()
                .map(failure_file_name)
                .filter(|name| saved.contains(name))
                .map(|name| paths.failures_dir.join(name));
            entry
        })
        .collect())
}

fn parse_line(line: &str) -> LogEntry {
    let parsed = line.split_once(' ').and_then(|(stamp, rest)| {
        let time = DateTime::parse_from_rfc3339(stamp).ok()?;
        let rest = rest.strip_prefix('[')?;
        let (level, message) = rest.split_once("] ")?;
        Some(LogEntry {
            time: Some(time),
            level: level.to_string(),
            message: message.to_string(),
            details: None,
        })
    });
    parsed.unwrap_or_else(|| LogEntry {
        time: None,
        level: String::new(),
        message: line.to_string(),
        details: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_structured_and_raw_lines() {
        let entry = parse_line(
            "2026-09-18T00:20:53.240154981+02:00 [error] Normal Sync finished with 1 failed entries.",
        );
        assert_eq!(entry.level, "error");
        assert!(entry.time.is_some());
        assert!(entry.is_run_result());

        let preview = parse_line("2026-09-18T00:20:53+02:00 [success] Normal Sync (preview) finished successfully");
        assert!(!preview.is_run_result());

        let raw = parse_line("Transferred:            9 / 9, 100%");
        assert!(raw.time.is_none());
        assert_eq!(raw.message, "Transferred:            9 / 9, 100%");
    }

    #[test]
    fn failure_output_is_linked_to_its_log_line_and_pruned() {
        let root = std::env::temp_dir().join(format!("syncpair-logs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let paths = AppPaths::rooted(&root);

        log_line(&paths, "success", "Normal Sync finished successfully").unwrap();
        log_failure(&paths, "Normal Sync failed for ~/a: boom", &["line 1".into(), "line 2".into()]).unwrap();

        let entries = read_log(&paths).unwrap();
        assert!(entries[0].details.is_none());
        let details = entries[1].details.as_ref().expect("failure keeps its output");
        assert_eq!(fs::read_to_string(details).unwrap(), "line 1\nline 2\n");

        for index in 0..FAILURES_KEPT + 5 {
            fs::write(paths.failures_dir.join(format!("{index:010}.000000000.log")), "").unwrap();
        }
        prune_failures(&paths.failures_dir).unwrap();
        assert_eq!(fs::read_dir(&paths.failures_dir).unwrap().count(), FAILURES_KEPT);
        assert!(!paths.failures_dir.join("0000000000.000000000.log").exists());

        fs::remove_dir_all(&root).unwrap();
    }
}
