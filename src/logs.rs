use anyhow::Result;
use chrono::{DateTime, FixedOffset, Local};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;

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
    writeln!(
        file,
        "{} [{}] {}",
        Local::now().to_rfc3339(),
        level,
        message
    )?;
    Ok(())
}

pub struct LogEntry {
    pub time: Option<DateTime<FixedOffset>>,
    pub level: String,
    pub message: String,
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
    Ok(raw.lines().filter(|line| !line.trim().is_empty()).map(parse_line).collect())
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
        })
    });
    parsed.unwrap_or_else(|| LogEntry {
        time: None,
        level: String::new(),
        message: line.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::parse_line;

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
}
