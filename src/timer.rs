use anyhow::{anyhow, bail, Result};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::process::{Command, ExitStatus};

use crate::logs::log_line;
use crate::paths::{AppPaths, APP_NAME, TIMER_FILE_NAME};
use crate::utils::remove_if_exists;

const INSTALLED_BINARY_PATH: &str = "/usr/bin/syncpair";

pub enum TimerCommand {
    Install,
    Remove,
    Status,
}

pub fn handle_timer_command(paths: &AppPaths, command: TimerCommand) -> Result<()> {
    match command {
        TimerCommand::Install => install_timer(paths),
        TimerCommand::Remove => remove_timer(paths),
        TimerCommand::Status => {
            println!("{}", timer_status_line(paths));
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
ExecStart={} daily-sync
",
            INSTALLED_BINARY_PATH
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
    run_systemctl(["--user", "daemon-reload"])?;
    run_systemctl(["--user", "enable", "--now", TIMER_FILE_NAME])?;
    log_line(paths, "success", "Installed daily timer")?;
    Ok(())
}

pub fn remove_timer(paths: &AppPaths) -> Result<()> {
    let _ = run_systemctl(["--user", "disable", "--now", TIMER_FILE_NAME]);
    remove_if_exists(&paths.service_file)?;
    remove_if_exists(&paths.timer_file)?;
    run_systemctl(["--user", "daemon-reload"])?;
    log_line(paths, "success", "Removed daily timer")?;
    Ok(())
}

pub fn timer_status_line(paths: &AppPaths) -> String {
    let enabled = systemctl_output(["--user", "is-enabled", TIMER_FILE_NAME])
        .unwrap_or_else(|_| "not installed".into());
    let active = systemctl_output(["--user", "is-active", TIMER_FILE_NAME])
        .unwrap_or_else(|_| "inactive".into());
    format!(
        "Daily Backup: {enabled} | Timer: {active} | Config: {} | Binary: /usr/bin/{APP_NAME}",
        paths.config_file.display()
    )
}

pub fn timer_is_enabled() -> bool {
    systemctl_output(["--user", "is-enabled", TIMER_FILE_NAME])
        .map(|status| status == "enabled")
        .unwrap_or(false)
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

    run_systemctl_import_environment(&vars)?;
    let _ = run_dbus_update_activation_environment(&vars);
    Ok(())
}

fn systemctl_output<const N: usize>(args: [&str; N]) -> Result<String> {
    let output = Command::new("systemctl").args(args).output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        bail!("systemctl failed")
    }
}

fn run_systemctl<const N: usize>(args: [&str; N]) -> Result<()> {
    let status = Command::new("systemctl").args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!(exit_status_message(status)))
    }
}

fn run_systemctl_import_environment(vars: &[&str]) -> Result<()> {
    let status = Command::new("systemctl")
        .args(["--user", "import-environment"])
        .args(vars)
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!(exit_status_message(status)))
    }
}

fn run_dbus_update_activation_environment(vars: &[&str]) -> Result<()> {
    let status = Command::new("dbus-update-activation-environment")
        .arg("--systemd")
        .args(vars.iter().map(OsStr::new))
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!(exit_status_message(status)))
    }
}

fn exit_status_message(status: ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("Command exited with status {code}"),
        None => "Command terminated by signal".to_string(),
    }
}
