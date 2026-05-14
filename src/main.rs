mod config;
mod logs;
mod paths;
mod rclone;
mod timer;
mod ui;
mod utils;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::config::show_config;
use crate::logs::{ensure_parent_dirs, open_log};
use crate::paths::{APP_NAME, app_paths};
use crate::rclone::{RunMode, run_sync_command};
use crate::timer::{TimerCommand, handle_timer_command};
use crate::ui::{CliResyncMode, interactive_menu};
use crate::utils::{notify, open_in_code, open_trash};

#[derive(Parser)]
#[command(name = APP_NAME, version, about = "Minimal rclone bisync wrapper")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    ShowConfig,
    EditConfig,
    ViewLogs,
    OpenTrash,
    Sync {
        #[arg(long)]
        dry_run: bool,
    },
    Resync {
        #[arg(long)]
        dry_run: bool,
        #[arg(long, value_enum)]
        mode: Option<CliResyncMode>,
    },
    Timer {
        #[command(subcommand)]
        command: TimerSubcommand,
    },
    DailySync,
}

#[derive(Subcommand)]
enum TimerSubcommand {
    Install,
    Remove,
    Status,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        let _ = notify("Syncpair Error", &error.to_string(), true);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let paths = app_paths()?;

    ensure_parent_dirs(&paths)?;

    match cli.command {
        Some(Commands::ShowConfig) => show_config(&paths),
        Some(Commands::EditConfig) => open_in_code(&paths.config_file),
        Some(Commands::ViewLogs) => open_log(&paths),
        Some(Commands::OpenTrash) => open_trash(&paths),
        Some(Commands::Sync { dry_run }) => run_sync_command(&paths, RunMode::Normal, dry_run, true),
        Some(Commands::Resync { dry_run, mode }) => {
            let mode = mode.unwrap_or(CliResyncMode::Newer).into();
            run_sync_command(&paths, RunMode::Resync(mode), dry_run, true)
        }
        Some(Commands::Timer { command }) => handle_timer_command(
            &paths,
            match command {
                TimerSubcommand::Install => TimerCommand::Install,
                TimerSubcommand::Remove => TimerCommand::Remove,
                TimerSubcommand::Status => TimerCommand::Status,
            },
        ),
        Some(Commands::DailySync) => run_sync_command(&paths, RunMode::Normal, false, false),
        None => interactive_menu(&paths),
    }
}
