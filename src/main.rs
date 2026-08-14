/// We require 4 parameters:
/// the path to the current binary (supplied by default), the path to the config.toml, the user_id, and the user_token.
const EXPECTED_AMOUNT_OF_ARGUMENTS: usize = 4;

pub mod communicator;
mod page;

use crate::page::{Page, PageToFileMapping};
use anyhow::{Context, bail};
use communicator::Communicator;
use csv::Reader;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::{env, fs};
use walkdir::WalkDir;

/// Small struct holding all the required cli-arguments.
struct Args {
    config_path: PathBuf,
    user_id: String,
    user_token: String,
}

/// Just a small struct that allows serde to parse the config.toml :)
/// All fields here are futher documented in the config.toml file.
#[derive(Deserialize)]
struct Config {
    is_test_environment: String,
    rest_url: String,
    site_url: String,
    mapping_table: PathBuf,
    markdown_dir: PathBuf,
    json_content_path: String,
}

fn main() -> anyhow::Result<()> {
    println!("==========================");
    println!("1. Initializing Resources:");
    println!("==========================");
    print!("1.1. Parsing arguments: ");

    // 1.1 Parse the command line arguments.
    let args = parse_arguments().context("Failed to parse the arguments!")?;

    println!("=> Success.");
    print!("1.2. Loading config: ");

    // 1.2 Load the provided config.
    let config = load_config(args.config_path).context("Failed to load config!")?;

    println!("=> Success.");
    print!("1.3. Loading Mapping table: ");

    // 1.3 Read the table that maps: Markdown-file <-> EgoCMS-page-id.
    let mut csv = load_table(&config).context(format!(
        "Failed to open the CSV table! It was expected to be at {}!",
        config.mapping_table.display()
    ))?;

    println!("=> Success.");
    print!("1.4. Connecting to EgoCMS: ");

    // 1.4 Open a connection to EgoCMS's REST API.
    let communicator = open_connection(
        config.rest_url,
        config.site_url,
        args.user_id,
        args.user_token,
        &config.is_test_environment,
    )
    .context(
        "Failed to open a connection to the EgoCMS REST API. Are the user_id and user_token valid?",
    )?;

    println!("=> Success.");
    println!();
    println!("==========================");
    println!("2. Checking Mapping Table For Correctness:");
    println!("==========================");

    let mappings: Vec<PageToFileMapping> = csv.deserialize().collect::<Result<Vec<_>, _>>()?;
    check_table_and_config_correctness(&mappings, &config.markdown_dir, &communicator)
        .context("The current configuration is incorrect!")?;

    println!();
    println!("==========================");
    println!("3. Updating Pages:");
    println!("==========================");
    // For each tracked page...
    for line in mappings {
        print!("=> The page: {} <-> {}", line.page_id, line.markdown_name);
        // ... create it ...
        let mut page = Page::new(line, &communicator, &config.markdown_dir)?;

        // ... and if we need to change something, do so.
        if page.is_up_to_date(&config.json_content_path)? {
            println!(" -> was up-to-date.");
        } else {
            page.update(&communicator, &config.json_content_path)?;

            println!(" -> was successfully updated!");
        }
    }
    println!("Success. Bye :)");
    Ok(())
}

fn parse_arguments() -> anyhow::Result<Args> {
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

    Ok(Args {
        config_path,
        user_id,
        user_token,
    })
}

fn load_config(path_to_config: PathBuf) -> anyhow::Result<Config> {
    let toml = fs::read_to_string(path_to_config)?;
    let toml: Config = toml::from_str(&toml)?;

    if !toml.mapping_table.try_exists()? {
        bail!(
            "Mapping table {} does not exist",
            toml.mapping_table.display()
        );
    }

    if !toml.markdown_dir.try_exists()? {
        bail!(
            "Markdown directory {} does not exist",
            toml.markdown_dir.display()
        );
    }

    Ok(toml)
}

fn load_table(config: &Config) -> csv::Result<Reader<File>> {
    csv::ReaderBuilder::new()
        // This ignores lines starting with # as a comment.
        .comment(Some(b'#'))
        .from_path(&config.mapping_table)
}

fn open_connection(
    rest_url: String,
    site_url: String,
    user_id: String,
    user_token: String,
    is_test_environment: &str,
) -> anyhow::Result<Communicator> {
    let is_test_environment = is_test_environment.eq("true");
    Communicator::new(rest_url, site_url, user_id, user_token, is_test_environment)
}

/// This function does a bunch of sanity checks to avoid silly mistakes.
/// It did get rather long, but oh well.
fn check_table_and_config_correctness(
    mappings: &[PageToFileMapping],
    markdown_dir: &PathBuf,
    communicator: &Communicator,
) -> anyhow::Result<()> {
    // 1. are all ids and names in the table unique?
    print!("Are all IDs and names unique? ");
    let mut table_ids = HashSet::new();
    let mut table_md_names = HashSet::new();
    let duplicate_ids: Vec<&PageToFileMapping> = mappings
        .iter()
        .filter(|line| !table_ids.insert(line.page_id.clone()))
        .collect();
    let duplicate_names: Vec<&PageToFileMapping> = mappings
        .iter()
        .filter(|line| !table_md_names.insert(line.markdown_name.clone()))
        .collect();

    if !duplicate_ids.is_empty() || !duplicate_names.is_empty() {
        bail!(
            "The table contains duplicate entries! Duplicate-IDs: [{duplicate_ids:?}], Duplicate-Names: [{duplicate_names:?}]"
        );
    }
    println!("=> Success.");

    // Get the relative path of all files in the directory.
    let mut md_file_names = Vec::new();
    for entry in WalkDir::new(markdown_dir) {
        let entry = entry?;
        if entry.file_type().is_file() {
            md_file_names.push(
                entry
                    .path()
                    .strip_prefix(markdown_dir)?
                    .to_string_lossy()
                    .to_string(),
            );
        }
    }

    // 2. are all files in the dir .md
    print!("Does the markdown dir only contain files ending on `.md`?");
    let incorrect_names: Vec<_> = md_file_names
        .iter()
        .filter(|name| {
            let name = Path::new(name);
            !name.extension().unwrap().eq_ignore_ascii_case("md")
        })
        .collect();
    if !incorrect_names.is_empty() {
        bail!(
            "The markdown dir: {} contains the files: [{incorrect_names:?}] which does not end in `.md`!",
            markdown_dir.display(),
        );
    }
    println!("=> Success.");

    // 3. are all .md in the dir listed in the table?
    print!("Are all files in the pages dir listed in the mapping table? ");
    let missing_table_entries: Vec<_> = md_file_names
        .iter()
        .filter(|name| !table_md_names.contains(name.as_str()))
        .collect();
    if !missing_table_entries.is_empty() {
        bail!("The files [{missing_table_entries:?}] are not listed in the mapping table!");
    }
    println!("=> Success.");

    // 4. do all mds in the table exist?
    print!("Do all .mds listed in the table exist? ");
    let missing_markdown_files: Vec<_> = table_md_names
        .iter()
        .filter(|md_name| !md_file_names.contains(md_name))
        .collect();
    if !missing_markdown_files.is_empty() {
        bail!(
            "The files [{missing_markdown_files:?}] are listed in the table but do not exist in {}!",
            markdown_dir.display()
        );
    }
    println!("=> Success.");

    // 5. do all ids in the table exist?
    print!("Do all IDs in the table exist? ");
    let mut missing_ids = Vec::new();
    for id in &table_ids {
        let page = communicator
            .get_page(id.as_str())
            .with_context(|| format!("Failed to fetch page for ID: {id}"))?;

        let json: Value = page
            .json()
            .with_context(|| format!("Failed to parse JSON response for ID: {id}"))?;

        // The API returns JSON `null` for non-existent pages.
        if json.is_null() {
            missing_ids.push(id);
        }
    }
    if !missing_ids.is_empty() {
        bail!("The IDs [{missing_ids:?} are listed in the table but do not exist.]");
    }
    println!("=> Success.");

    Ok(())
}
