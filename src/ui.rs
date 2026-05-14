use anyhow::{Result, bail};
use clap::ValueEnum;
use dialoguer::{Select, theme::ColorfulTheme};

use crate::config::show_config;
use crate::logs::open_log;
use crate::paths::AppPaths;
use crate::rclone::{ResyncMode, RunMode, run_sync_command};
use crate::timer::{install_timer, remove_timer, timer_is_enabled, timer_status_line};
use crate::utils::{open_in_code, open_trash};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CliResyncMode {
    Newer,
    Older,
}

impl From<CliResyncMode> for ResyncMode {
    fn from(value: CliResyncMode) -> Self {
        match value {
            CliResyncMode::Newer => ResyncMode::Newer,
            CliResyncMode::Older => ResyncMode::Older,
        }
    }
}

pub fn interactive_menu(paths: &AppPaths) -> Result<()> {
    let timer_enabled = timer_is_enabled();
    let mut items = vec![
        "Show Config",
        "Edit Config",
        "View Logs",
        "Open Trash",
        "Normal Sync (preview)",
        "Normal Sync",
        "Resync (preview)",
        "Resync",
    ];

    if timer_enabled {
        items.push("Remove Daily Timer");
    } else {
        items.push("Install Daily Timer");
    }

    items.push("Quit");

    println!("{}", timer_status_line(paths));

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select action")
        .items(&items)
        .default(0)
        .interact()?;

    match items[selection] {
        "Show Config" => show_config(paths),
        "Edit Config" => open_in_code(&paths.config_file),
        "View Logs" => open_log(paths),
        "Open Trash" => open_trash(paths),
        "Normal Sync (preview)" => run_sync_command(paths, RunMode::Normal, true, true),
        "Normal Sync" => run_sync_command(paths, RunMode::Normal, false, true),
        "Resync (preview)" => run_sync_command(paths, RunMode::Resync(prompt_resync_mode()?), true, true),
        "Resync" => run_sync_command(paths, RunMode::Resync(prompt_resync_mode()?), false, true),
        "Remove Daily Timer" => remove_timer(paths),
        "Install Daily Timer" => install_timer(paths),
        "Quit" => Ok(()),
        _ => bail!("Unknown selection"),
    }
}

fn prompt_resync_mode() -> Result<ResyncMode> {
    let items = ["newer", "older"];
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select resync mode")
        .items(&items)
        .default(0)
        .interact()?;

    Ok(match selection {
        0 => ResyncMode::Newer,
        1 => ResyncMode::Older,
        _ => bail!("Invalid resync mode selection"),
    })
}
