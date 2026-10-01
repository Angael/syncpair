use anyhow::{anyhow, Context, Result};
use chrono::Local;
use std::fs;
use std::io::{BufRead, BufReader, IsTerminal, Read};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::config::{load_config, validate_config, Config, SyncEntry};
use crate::logs::log_line;
use crate::paths::AppPaths;
use crate::utils::{expand_home, notify, path_slug, require_command};

/// How long rclone gets to finish a graceful (SIGINT) shutdown before it is killed.
const CANCEL_GRACE: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ResyncMode {
    Newer,
    Older,
}

impl ResyncMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Newer => "newer",
            Self::Older => "older",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Normal,
    Resync(ResyncMode),
}

impl RunMode {
    pub fn label(self, dry_run: bool) -> &'static str {
        match (self, dry_run) {
            (RunMode::Normal, false) => "Normal Sync",
            (RunMode::Normal, true) => "Normal Sync (preview)",
            (RunMode::Resync(_), false) => "Resync",
            (RunMode::Resync(_), true) => "Resync (preview)",
        }
    }

    pub fn warning(self) -> &'static str {
        match self {
            RunMode::Normal => {
                "Normal sync can propagate changes and move deletions into dated trash."
            }
            RunMode::Resync(_) => "Resync can overwrite different versions on both sides.",
        }
    }
}

/// What rclone should print besides errors.
#[derive(Clone, Copy)]
pub struct Verbosity {
    /// Emit one-line transfer stats every second (drives progress bars).
    pub progress: bool,
    /// Per-file INFO lines.
    pub verbose: bool,
}

#[derive(Clone, Copy)]
pub struct RunSpec {
    pub mode: RunMode,
    pub dry_run: bool,
    pub verbosity: Verbosity,
}

impl RunSpec {
    pub fn label(self) -> &'static str {
        self.mode.label(self.dry_run)
    }
}

pub enum SyncEvent {
    EntryStarted(usize),
    /// A line of rclone output (stdout or stderr) for the entry currently running.
    Output(String),
    /// Fraction of the current transfer (0..=1) and the raw stats line.
    Progress(usize, f32, String),
    EntryFinished(usize, std::result::Result<(), String>),
}

pub struct RunOutcome {
    pub failures: usize,
    pub cancelled: bool,
    /// Human summary, also written to the log.
    pub message: String,
}

impl RunOutcome {
    pub fn ok(&self) -> bool {
        self.failures == 0 && !self.cancelled
    }
}

/// Runs the selected entries one after another, logging results to the app log.
pub fn run_sync(
    paths: &AppPaths,
    config: &Config,
    spec: RunSpec,
    selected: &[usize],
    cancel: &AtomicBool,
    on_event: &mut dyn FnMut(SyncEvent),
) -> Result<RunOutcome> {
    require_command("rclone")?;
    validate_config(config)?;

    let label = spec.label();
    let mut failures = 0;
    let mut cancelled = false;

    for &index in selected {
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            break;
        }
        let entry = config
            .sync
            .get(index)
            .ok_or_else(|| anyhow!("No sync entry #{index}"))?;
        on_event(SyncEvent::EntryStarted(index));
        let result = run_one_sync(
            paths, &config.remote, entry, spec, cancel,
            &mut |event| on_event(match event {
                Line::Output(line) => SyncEvent::Output(line),
                Line::Progress(fraction, line) => SyncEvent::Progress(index, fraction, line),
            }),
        );
        match result {
            Ok(()) => on_event(SyncEvent::EntryFinished(index, Ok(()))),
            Err(_) if cancel.load(Ordering::Relaxed) => {
                cancelled = true;
                on_event(SyncEvent::EntryFinished(index, Err("Cancelled".into())));
                break;
            }
            Err(error) => {
                failures += 1;
                let message = format!("{error:#}");
                log_line(paths, "error", &format!("{label} failed for {}: {message}", entry.local))?;
                on_event(SyncEvent::EntryFinished(index, Err(message)));
            }
        }
    }

    let (level, message) = if cancelled {
        ("warn", format!("{label} cancelled."))
    } else if failures > 0 {
        ("error", format!("{label} finished with {failures} failed entries."))
    } else {
        ("success", format!("{label} finished successfully"))
    };
    log_line(paths, level, &message)?;

    Ok(RunOutcome { failures, cancelled, message })
}

/// Headless entry point used by `sync`, `resync` and the systemd `daily-sync` service.
pub fn run_sync_command(paths: &AppPaths, mode: RunMode, dry_run: bool) -> Result<()> {
    let config = load_config(paths)?;
    validate_config(&config)?;

    let label = mode.label(dry_run);
    let interactive = std::io::stdout().is_terminal();
    println!("{label}");
    // Detached: a slow notification daemon at boot must not hold up the sync.
    let _ = notify("Backup Started", label, false);

    let selected: Vec<usize> = (0..config.sync.len()).collect();
    let never_cancel = AtomicBool::new(false);
    let outcome = run_sync(
        paths,
        &config,
        RunSpec { mode, dry_run, verbosity: Verbosity { progress: interactive, verbose: false } },
        &selected,
        &never_cancel,
        &mut |event| match event {
            SyncEvent::EntryStarted(index) => {
                let entry = &config.sync[index];
                println!("\nSyncing: {} -> {}", entry.local, config.remote_target(entry));
            }
            SyncEvent::Output(line) => eprintln!("{line}"),
            SyncEvent::Progress(_, _, line) => eprintln!("{line}"),
            SyncEvent::EntryFinished(_, Err(error)) => eprintln!("error: {error}"),
            SyncEvent::EntryFinished(_, Ok(())) => {}
        },
    )?;

    if outcome.ok() {
        let _ = notify("Task Finished", "Your backup is complete.", false).join();
        Ok(())
    } else {
        let _ = notify("Backup Error", &outcome.message, true).join();
        Err(anyhow!(outcome.message))
    }
}

enum Line {
    Output(String),
    Progress(f32, String),
}

fn run_one_sync(
    paths: &AppPaths,
    remote_name: &str,
    entry: &SyncEntry,
    spec: RunSpec,
    cancel: &AtomicBool,
    on_line: &mut dyn FnMut(Line),
) -> Result<()> {
    let local_path = expand_home(&entry.local)?;
    if !local_path.exists() {
        return Err(anyhow!("Local path not found: {}", local_path.display()));
    }
    if !local_path.is_dir() {
        return Err(anyhow!(
            "Local path must be a directory for bisync: {}",
            local_path.display()
        ));
    }

    let mut command = bisync_command(paths, remote_name, entry, &local_path, spec)?;
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().context("Failed to start rclone")?;

    let (sender, receiver) = mpsc::channel::<String>();
    let readers = [
        spawn_line_reader(child.stdout.take(), sender.clone()),
        spawn_line_reader(child.stderr.take(), sender),
    ];

    let mut last_error: Option<String> = None;
    let mut interrupted_at: Option<Instant> = None;
    loop {
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => {
                let text = strip_log_prefix(&line);
                if text.starts_with("ERROR") || text.contains("Bisync aborted") {
                    last_error = Some(text.to_string());
                }
                match stats_fraction(text) {
                    Some(fraction) if spec.verbosity.progress => {
                        on_line(Line::Progress(fraction, text.to_string()))
                    }
                    _ => on_line(Line::Output(line)),
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if cancel.load(Ordering::Relaxed) {
            interrupt(&mut child, &mut interrupted_at);
        }
    }
    for reader in readers {
        let _ = reader.join();
    }

    let status = child.wait().context("Failed to wait for rclone")?;
    if status.success() {
        Ok(())
    } else if interrupted_at.is_some() {
        Err(anyhow!("Cancelled"))
    } else {
        let base = exit_status_message(status);
        Err(match last_error {
            Some(detail) => anyhow!("{base}: {detail}"),
            None => anyhow!(base),
        })
    }
}

fn bisync_command(
    paths: &AppPaths,
    remote_name: &str,
    entry: &SyncEntry,
    local_path: &Path,
    spec: RunSpec,
) -> Result<Command> {
    let remote_target = format!("{remote_name}:{}", entry.remote);
    let mut command = Command::new("rclone");
    command.arg("bisync").arg(local_path).arg(&remote_target);
    command.args([
        "--compare",
        "size,modtime",
        "--max-delete",
        "95",
        "--conflict-resolve",
        "newer",
        "--conflict-loser",
        "num",
        "--create-empty-src-dirs",
        "--recover",
        "--resilient",
    ]);

    if spec.verbosity.progress {
        command.args(["--stats", "1s", "--stats-one-line", "--stats-log-level", "NOTICE"]);
    }
    if spec.verbosity.verbose {
        command.arg("-v");
    }

    if spec.dry_run {
        command.arg("--dry-run");
    } else {
        let stamp = Local::now().format("%Y-%m-%d").to_string();
        let local_backup_dir = paths.trash_dir.join(&stamp).join(path_slug(&entry.local));
        let remote_backup_dir = format!("{remote_name}:trash/{stamp}/{}", entry.remote);
        fs::create_dir_all(&local_backup_dir)?;
        command.arg("--backup-dir1").arg(local_backup_dir);
        command.arg("--backup-dir2").arg(remote_backup_dir);
    }

    if let RunMode::Resync(resync_mode) = spec.mode {
        command.args(["--resync", "--resync-mode", resync_mode.as_str()]);
    }
    Ok(command)
}

fn spawn_line_reader<R: Read + Send + 'static>(
    source: Option<R>,
    sender: mpsc::Sender<String>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let Some(source) = source else { return };
        let mut reader = BufReader::new(source);
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            match reader.read_until(b'\n', &mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buffer);
                    let line = line.trim_end_matches(['\n', '\r']);
                    if !line.is_empty() && sender.send(line.to_string()).is_err() {
                        break;
                    }
                }
            }
        }
    })
}

/// Ask rclone to stop gracefully (bisync handles SIGINT), escalating to SIGKILL after a grace period.
fn interrupt(child: &mut Child, interrupted_at: &mut Option<Instant>) {
    match interrupted_at {
        None => {
            // SAFETY: plain kill(2) on our own child's pid; no memory is touched.
            unsafe {
                libc::kill(child.id() as libc::pid_t, libc::SIGINT);
            }
            *interrupted_at = Some(Instant::now());
        }
        Some(start) if start.elapsed() > CANCEL_GRACE => {
            let _ = child.kill();
        }
        Some(_) => {}
    }
}

/// `2026/10/01 10:00:00 NOTICE: text` -> `NOTICE: text`.
fn strip_log_prefix(line: &str) -> &str {
    let bytes = line.as_bytes();
    let looks_dated = bytes.len() > 20
        && bytes[4] == b'/'
        && bytes[7] == b'/'
        && bytes[10] == b' '
        && bytes[13] == b':'
        && bytes[19] == b' ';
    if looks_dated {
        &line[20..]
    } else {
        line
    }
}

/// Recognises a `--stats-one-line` line such as
/// `NOTICE:    1.2 MiB / 3.4 MiB, 35%, 500 KiB/s, ETA 4s` and returns its completed fraction
/// (`0.0` while rclone reports `-` because nothing has been sized yet).
fn stats_fraction(line: &str) -> Option<f32> {
    if !line.contains(" / ") || !line.contains(", ETA ") {
        return None;
    }
    let percent = line
        .split(',')
        .map(str::trim)
        .find_map(|token| token.strip_suffix('%')?.parse::<f32>().ok())
        .unwrap_or(0.0);
    Some((percent / 100.0).clamp(0.0, 1.0))
}

fn exit_status_message(status: ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("rclone exited with status {code}"),
        None => "rclone terminated by signal".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stats_lines_only() {
        let line = "2026/10/01 10:00:00 NOTICE:    1.234 MiB / 3.400 MiB, 35%, 500 KiB/s, ETA 4s";
        let text = strip_log_prefix(line);
        assert_eq!(&text[..7], "NOTICE:");
        assert_eq!(stats_fraction(text), Some(0.35));
        assert_eq!(stats_fraction("NOTICE: 0 B / 0 B, -, 0 B/s, ETA -"), Some(0.0));
        assert_eq!(stats_fraction("INFO  : file100%.txt: Copied (new)"), None);
    }
}
