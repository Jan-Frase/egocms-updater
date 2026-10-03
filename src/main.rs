// TODO:
// In the works:
// - Links via page_id
// - get rid of any unneeded unwrap

// Planned:
// - Media Files via Interface in EgoCMS
// - Title is a separate entry
// - Download EgoCMS pages into dirs and .md files, starting from the manually mapped Home.md
// - Ensure that when updating an existing site, the title and names are updated as well.

// Done:
// Automatically create new pages.
// -> the name of the new page is derived from the file name, the title is the first line in the md

pub mod api_communication;
mod initalization;
mod new_page_creation;
mod page;

use serde::Deserialize;
use std::path::PathBuf;

/// Path to the mapping table CSV.
const MAPPING_TABLE_PATH: &str = "./config/mapping_table.csv";

/// Directory where the Markdown pages are stored.
const MARKDOWN_DIR: &str = "./pages/";

/// Small struct holding all the required cli-arguments.
struct Args {
    config_path: PathBuf,
    user_id: String,
    user_token: String,
}

/// Just a small struct that allows serde to parse the config.toml :)
/// All fields here are further documented in the config.toml file.
#[derive(Deserialize)]
struct Config {
    rest_url: String,
    site_url: String,
    home_page: String,
    json_content_path: String,
}

fn main() -> anyhow::Result<()> {
    let (mut pages, communicator, config) = initalization::init()?;

    println!("=> Success.");
    println!();
    println!("==========================");
    println!("2. Creating New Pages:");
    println!("==========================");

    new_page_creation::push_new_pages(&mut pages, &config, &communicator)?;

    println!("=> Success.");
    println!();
    println!("==========================");
    println!("3. Checking Mapping Table For Correctness:");
    println!("==========================");

    // check_table_and_config_correctness(&mappings, &config.markdown_dir, &communicator)
    //    .context("The current configuration is incorrect!")?;

    println!();
    println!("==========================");
    println!("4. Updating Pages:");
    println!("==========================");
    // For each tracked page...
    for page in pages {
        print!(
            "=> The page: {:?} <-> {}",
            page.mapping.page_id, page.mapping.markdown_name
        );
        // ... update it, if required.
        page.update(&communicator, &config.json_content_path)?;
    }
    println!("Success. Bye :)");
    Ok(())
}
