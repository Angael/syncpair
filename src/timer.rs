use anyhow::{bail, Result};
use std::env;
use std::fs;
use std::process::Command;

use crate::logs::log_line;
use crate::paths::{AppPaths, SERVICE_FILE_NAME, TIMER_FILE_NAME};
use crate::utils::remove_if_exists;

const INSTALLED_BINARY_PATH: &str = "/usr/bin/syncpair";

pub enum TimerCommand {
    Install,
    Remove,
    Status,
}

#[derive(Clone, Default)]
pub struct TimerState {
    pub enabled: bool,
    pub active: bool,
    /// systemd's human-readable next elapse time, e.g. `Fri 2026-10-02 00:00:00 CEST`.
    pub next_run: Option<String>,
    /// The daily sync service is running right now.
    pub service_running: bool,
}

pub fn handle_timer_command(paths: &AppPaths, command: TimerCommand) -> Result<()> {
    match command {
        TimerCommand::Install => install_timer(paths),
        TimerCommand::Remove => remove_timer(paths),
        TimerCommand::Status => {
            let state = timer_state();
            println!(
                "Daily Backup: {} | Timer: {} | Next: {} | Config: {} | Binary: {INSTALLED_BINARY_PATH}",
                if state.enabled { "enabled" } else { "not installed" },
                if state.active { "active" } else { "inactive" },
                state.next_run.as_deref().unwrap_or("-"),
                paths.config_file.display()
            );
            Ok(())
        }
    }
}

pub fn install_timer(paths: &AppPaths) -> Result<()> {
    fs::create_dir_all(&paths.systemd_dir)?;

    fs::write(
        &paths.service_file,
        format!(
            "[Unit]
Description=syncpair daily sync

[Service]
Type=oneshot
Environment=DBUS_SESSION_BUS_ADDRESS=unix:path=%t/bus
Environment=XDG_RUNTIME_DIR=%t
ExecStart={INSTALLED_BINARY_PATH} daily-sync
"
        ),
    )?;
    fs::write(
        &paths.timer_file,
        "[Unit]
Description=Run syncpair daily

[Timer]
OnCalendar=daily
Persistent=true

[Install]
WantedBy=timers.target
",
    )?;

    import_user_session_environment()?;
    systemctl(&["--user", "daemon-reload"])?;
    systemctl(&["--user", "enable", "--now", TIMER_FILE_NAME])?;
    log_line(paths, "success", "Installed daily timer")?;
    Ok(())
}

pub fn remove_timer(paths: &AppPaths) -> Result<()> {
    let _ = systemctl(&["--user", "disable", "--now", TIMER_FILE_NAME]);
    remove_if_exists(&paths.service_file)?;
    remove_if_exists(&paths.timer_file)?;
    systemctl(&["--user", "daemon-reload"])?;
    log_line(paths, "success", "Removed daily timer")?;
    Ok(())
}

pub fn timer_state() -> TimerState {
    let query = |args: &[&str]| systemctl(args).unwrap_or_default();
    let next_run = query(&[
        "--user",
        "show",
        TIMER_FILE_NAME,
        "--property=NextElapseUSecRealtime",
        "--value",
    ]);
    TimerState {
        enabled: query(&["--user", "is-enabled", TIMER_FILE_NAME]) == "enabled",
        active: query(&["--user", "is-active", TIMER_FILE_NAME]) == "active",
        next_run: (!next_run.is_empty() && next_run != "n/a").then_some(next_run),
        service_running: matches!(
            query(&["--user", "is-active", SERVICE_FILE_NAME]).as_str(),
            "active" | "activating"
        ),
    }
}

fn import_user_session_environment() -> Result<()> {
    const SESSION_VARS: &[&str] = &[
        "DBUS_SESSION_BUS_ADDRESS",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XAUTHORITY",
        "XDG_CURRENT_DESKTOP",
        "XDG_RUNTIME_DIR",
    ];

    let vars = SESSION_VARS
        .iter()
        .copied()
        .filter(|name| env::var_os(name).is_some())
        .collect::<Vec<_>>();

    if vars.is_empty() {
        return Ok(());
    }

    let mut args = vec!["--user", "import-environment"];
    args.extend(&vars);
    systemctl(&args)?;

    let _ = Command::new("dbus-update-activation-environment")
        .arg("--systemd")
        .args(&vars)
        .output();
    Ok(())
}

/// Runs `systemctl` and returns trimmed stdout; on failure the error carries stderr.
fn systemctl(args: &[&str]) -> Result<String> {
    let output = Command::new("systemctl").args(args).output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        match output.status.code() {
            Some(code) if detail.is_empty() => bail!("systemctl {} exited with status {code}", args.join(" ")),
            _ if detail.is_empty() => bail!("systemctl {} terminated by signal", args.join(" ")),
            _ => bail!("systemctl {}: {detail}", args.join(" ")),
        }
    }
}
