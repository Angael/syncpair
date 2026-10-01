mod config;
mod gui;
mod logs;
mod paths;
mod rclone;
mod timer;
mod trash;
mod utils;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

use crate::config::show_config;
use crate::logs::ensure_parent_dirs;
use crate::paths::{app_paths, APP_NAME};
use crate::rclone::{run_sync_command, ResyncMode, RunMode};
use crate::timer::{handle_timer_command, TimerCommand};
use crate::trash::cleanup_trash;
use crate::utils::notify;

#[derive(Parser)]
#[command(
    name = APP_NAME,
    version,
    about = "Two-way rclone bisync backups. Run without a command to open the app."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    ShowConfig,
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

#[derive(Clone, Copy, ValueEnum)]
enum CliResyncMode {
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

fn main() {
    let cli = Cli::parse();
    let headless = cli.command.is_some();
    if let Err(error) = run(cli) {
        eprintln!("error: {error:#}");
        if headless {
            let _ = notify("Syncpair Error", &error.to_string(), true).join();
        }
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    let paths = app_paths()?;

    ensure_parent_dirs(&paths)?;
    cleanup_trash(&paths.trash_dir)?;

    match cli.command {
        None => gui::run(paths),
        Some(Commands::ShowConfig) => show_config(&paths),
        Some(Commands::Sync { dry_run }) => run_sync_command(&paths, RunMode::Normal, dry_run),
        Some(Commands::Resync { dry_run, mode }) => {
            let mode = mode.unwrap_or(CliResyncMode::Newer).into();
            run_sync_command(&paths, RunMode::Resync(mode), dry_run)
        }
        Some(Commands::Timer { command }) => handle_timer_command(
            &paths,
            match command {
                TimerSubcommand::Install => TimerCommand::Install,
                TimerSubcommand::Remove => TimerCommand::Remove,
                TimerSubcommand::Status => TimerCommand::Status,
            },
        ),
        Some(Commands::DailySync) => run_sync_command(&paths, RunMode::Normal, false),
    }
}
