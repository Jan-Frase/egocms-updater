use anyhow::Result;
use auto_update_egocms::settings::config::Config;
use auto_update_egocms::settings::credentials::Credentials;
use auto_update_egocms::{download, upload};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "EgoCMS-Updater")]
#[command(about = "A tool to manage EgoCMS pages as markdown files.")]
struct Cli {
    /// Required: Which action should be executed.
    #[command(subcommand)]
    command: Command,

    /// Set the settings file.
    #[arg(short, long, default_value = "./config/config.toml")]
    config_path: PathBuf,
}

#[derive(Parser)]
enum Command {
    /// Upload the contents of ./pages to the cms.
    Upload {
        /// Only show what would change, without writing anything.
        #[arg(long)]
        dry_run: bool,
    },
    /// Download the cms into the dir.
    Download { out_dir: PathBuf },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = Config::new(&cli.config_path)?;
    let credentials = Credentials::from_env()?;

    match cli.command {
        Command::Download { out_dir } => download(&config, &credentials, &out_dir),
        Command::Upload { dry_run } => upload(&config, &credentials, dry_run),
    }
}
