use anyhow::{bail, Result};
use clap::ValueEnum;
use dialoguer::{theme::ColorfulTheme, Select};

use crate::config::show_config;
use crate::logs::open_log;
use crate::paths::AppPaths;
use crate::rclone::{run_sync_command, ResyncMode, RunMode};
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
        "Normal Sync",
        "Resync",
        "Edit Config",
        "Show Config",
        "View Logs",
        "Open Trash",
    ];

    if timer_enabled {
        items.push("Remove Daily Timer");
    } else {
        items.push("Install Daily Timer");
    }
    items.push("Daily Timer Status");
    items.push("Normal Sync (preview)");
    items.push("Resync (preview)");

    items.push("Quit");

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select action")
        .items(&items)
        .default(0)
        .interact()?;

    match items[selection] {
        "Normal Sync" => run_sync_command(paths, RunMode::Normal, false, true),
        "Resync" => run_sync_command(paths, RunMode::Resync(prompt_resync_mode()?), false, true),
        "Edit Config" => open_in_code(&paths.config_file),
        "Show Config" => show_config(paths),
        "View Logs" => open_log(paths),
        "Open Trash" => open_trash(paths),
        "Remove Daily Timer" => remove_timer(paths),
        "Install Daily Timer" => install_timer(paths),
        "Daily Timer Status" => {
            println!("{}", timer_status_line(paths));
            Ok(())
        }
        "Normal Sync (preview)" => run_sync_command(paths, RunMode::Normal, true, true),
        "Resync (preview)" => {
            run_sync_command(paths, RunMode::Resync(prompt_resync_mode()?), true, true)
        }
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
