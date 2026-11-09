use anyhow::Result;
use auto_update_egocms::config::Config;
use auto_update_egocms::credentials::Credentials;
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

    /// Set the config file.
    #[arg(short, long, default_value = "./config/config.toml")]
    config_path: PathBuf,
}

#[derive(Parser)]
enum Command {
    /// Upload the contents of ./pages to the cms.
    Upload,
    /// Download the cms into the dir.
    Download { out_dir: PathBuf },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = Config::new(&cli.config_path)?;
    let credentials = Credentials::from_env()?;

    match cli.command {
        Command::Download { out_dir } => download(&config, &credentials, &out_dir),
        Command::Upload => upload(&config, &credentials),
    }
}
