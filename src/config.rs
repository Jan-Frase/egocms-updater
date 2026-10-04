use anyhow::bail;
use serde::Deserialize;
use std::env;
use std::path::PathBuf;

/// We require 4 parameters:
/// the path to the current binary (supplied by default), the path to the config.toml, the user_id, and the user_token.
const EXPECTED_AMOUNT_OF_ARGUMENTS: usize = 4;

/// Path to the mapping table CSV.
pub const MAPPING_TABLE_PATH: &str = "./config/mapping_table.csv";

/// Directory where the Markdown pages are stored.
pub const MARKDOWN_DIR: &str = "./pages/";

/// Just a small struct that allows serde to parse the config.toml :)
/// All fields here are further documented in the config.toml file.
#[derive(Deserialize)]
pub struct Config {
    pub(crate) rest_url: String,
    pub(crate) site_url: String,
    pub(crate) home_page: String,
    pub(crate) json_content_path: String,
}

/// Small struct holding all the required cli-arguments.
pub struct Inputs {
    pub(crate) config_path: PathBuf,
    pub(crate) user_id: String,
    pub(crate) user_token: String,
}

pub fn parse_arguments() -> anyhow::Result<Inputs> {
    let args: Vec<String> = env::args().collect();

    if args.len() != EXPECTED_AMOUNT_OF_ARGUMENTS {
        println!(
            r"
            Help Message:
            EgoCMS Updater for Parcio Websites
            For more details take a look at the README.md :)

            Usage: cargo run --release -- [path-to-config.toml] [user-id] [user-token]

            Example: cargo run --release -- ./config/config.toml 12345 abcde
            "
        );
        bail!("Invalid number of arguments!");
    }

    // 1. Parse the config.toml path.
    let config_path = PathBuf::from(&args[1]);
    if !config_path.try_exists()? {
        bail!("Config file {} does not exist", config_path.display());
    }

    // 2. Parse the user_id.
    let user_id = args[2].clone();

    // 3. Parse the user_token.
    let user_token = args[3].clone();

    Ok(Inputs {
        config_path,
        user_id,
        user_token,
    })
}
