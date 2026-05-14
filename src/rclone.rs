use anyhow::{Context, Result, anyhow, bail};
use chrono::Local;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;

use crate::config::{SyncEntry, load_config, validate_config};
use crate::logs::log_line;
use crate::paths::AppPaths;
use crate::utils::{expand_home, notify, path_slug, require_command};

#[derive(Clone, Copy)]
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

#[derive(Clone, Copy)]
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
}

pub fn run_sync_command(paths: &AppPaths, mode: RunMode, dry_run: bool, ask_confirm: bool) -> Result<()> {
    require_command("rclone")?;

    let config = load_config(paths)?;
    validate_config(&config)?;

    if ask_confirm && !dry_run {
        let prompt = match mode {
            RunMode::Normal => "Normal sync can propagate changes and move deletions into dated trash. Continue?",
            RunMode::Resync(_) => "Resync can overwrite different versions on both sides. Continue?",
        };

        if !dialoguer::Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt(prompt)
            .default(false)
            .interact()?
        {
            return Ok(());
        }
    }

    let label = mode.label(dry_run);
    println!("{label}");
    notify("Backup Started", label, false)?;

    let mut failures = 0;
    for entry in &config.sync {
        if let Err(error) = run_one_sync(paths, &config.remote, entry, mode, dry_run) {
            failures += 1;
            eprintln!("error: {error:#}");
            log_line(paths, "error", &format!("{} failed for {}: {error:#}", label, entry.local))?;
        }
    }

    if failures > 0 {
        let message = format!("{label} finished with {failures} failed entries.");
        notify("Backup Error", &message, true)?;
        log_line(paths, "error", &message)?;
        bail!(message);
    }

    notify("Task Finished", "Your backup is complete.", false)?;
    log_line(paths, "success", &format!("{label} finished successfully"))?;
    Ok(())
}

fn run_one_sync(paths: &AppPaths, remote_name: &str, entry: &SyncEntry, mode: RunMode, dry_run: bool) -> Result<()> {
    let local_path = expand_home(&entry.local)?;
    if !local_path.exists() {
        bail!("Local path not found: {}", local_path.display());
    }
    if !local_path.is_dir() {
        bail!("Local path must be a directory for bisync: {}", local_path.display());
    }
    if entry.remote.trim().is_empty() {
        bail!("Remote path is empty for entry: {}", entry.local);
    }

    let remote_target = format!("{remote_name}:{}", entry.remote);
    println!("\nSyncing: {} -> {}", entry.local, remote_target);

    let mut command = Command::new("rclone");
    command.arg("bisync");
    command.arg(&local_path);
    command.arg(&remote_target);
    command.args([
        "-P",
        "--compare",
        "size,modtime",
        "--max-delete",
        "40",
        "--conflict-resolve",
        "newer",
        "--conflict-loser",
        "num",
    ]);
    command.arg("--create-empty-src-dirs");
    command.arg("--recover");
    command.arg("--resilient");

    if dry_run {
        command.arg("--dry-run");
    } else {
        let stamp = Local::now().format("%Y-%m-%d").to_string();
        let local_backup_dir = paths.trash_dir.join(&stamp).join(path_slug(&entry.local));
        let remote_backup_dir = format!("{remote_name}:trash/{stamp}/{}", entry.remote);
        fs::create_dir_all(&local_backup_dir)?;
        command.arg("--backup-dir1");
        command.arg(local_backup_dir);
        command.arg("--backup-dir2");
        command.arg(remote_backup_dir);
    }

    if let RunMode::Resync(resync_mode) = mode {
        command.arg("--resync");
        command.arg("--resync-mode");
        command.arg(resync_mode.as_str());
    }

    run_and_proxy(command, &paths.log_file)
}

fn run_and_proxy(mut command: Command, log_file: &Path) -> Result<()> {
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command.spawn().context("Failed to start command")?;
    let stdout = child.stdout.take().context("Missing stdout pipe")?;
    let stderr = child.stderr.take().context("Missing stderr pipe")?;

    let log_out = log_file.to_path_buf();
    let out_handle = thread::spawn(move || proxy_stream(stdout, false, &log_out));

    let log_err = log_file.to_path_buf();
    let err_handle = thread::spawn(move || proxy_stream(stderr, true, &log_err));

    let status = child.wait()?;
    join_proxy(out_handle)?;
    join_proxy(err_handle)?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!(exit_status_message(status)))
    }
}

fn proxy_stream<R: std::io::Read>(reader: R, stderr: bool, log_file: &Path) -> Result<()> {
    let mut log = OpenOptions::new().create(true).append(true).open(log_file)?;
    for line in BufReader::new(reader).lines() {
        let line = line?;
        if stderr {
            eprintln!("{line}");
        } else {
            println!("{line}");
        }
        writeln!(log, "{} [rclone] {}", Local::now().to_rfc3339(), line)?;
    }
    Ok(())
}

fn join_proxy(handle: thread::JoinHandle<Result<()>>) -> Result<()> {
    handle.join().map_err(|_| anyhow!("Failed to join output thread"))??;
    Ok(())
}

fn exit_status_message(status: ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("Command exited with status {code}"),
        None => "Command terminated by signal".to_string(),
    }
}
