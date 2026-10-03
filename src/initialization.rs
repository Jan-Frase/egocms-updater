use crate::api_communication::Communicator;
use crate::page::{Page, PageToFileMapping};
use crate::{Args, Config, MAPPING_TABLE_PATH, MARKDOWN_DIR};
use anyhow::{Context, bail};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::{env, fs};
use walkdir::WalkDir;

/// We require 4 parameters:
/// the path to the current binary (supplied by default), the path to the config.toml, the user_id, and the user_token.
const EXPECTED_AMOUNT_OF_ARGUMENTS: usize = 4;

/// Responsible for loading the core data needed for this application.
/// That is:
/// Vec<Page> - A vector containing all pages, even those not yet pushed to the CMS.
/// Communicator - Responsible for API calls.
/// Config - Stores various core aspects like the URL.
pub fn init() -> anyhow::Result<(Vec<Page>, Communicator, Config)> {
    // 1.1 Parse the command line arguments.
    let args = parse_arguments().context("Failed to parse the arguments!")?;

    // 1.2 Load the provided config.
    let config: Config = toml::from_str(&fs::read_to_string(args.config_path)?)?;

    // 1.3 Open a connection to EgoCMS's REST API.
    let is_test_environment = config.rest_url.eq("https://localhost/rest/");
    let communicator = Communicator::new(
        config.rest_url.clone(),
        config.site_url.clone(),
        args.user_id,
        args.user_token,
        is_test_environment,
    )?;

    // 1.4 Read the table that maps: Markdown-file <-> EgoCMS-page-id.
    let mut csv = csv::ReaderBuilder::new()
        // This ignores lines starting with # as a comment.
        .comment(Some(b'#'))
        .from_path(MAPPING_TABLE_PATH)?;

    // 1.5 Create pages based on mapping table.
    let mappings: Vec<PageToFileMapping> = csv.deserialize().collect::<Result<Vec<_>, _>>()?;
    let mut pages: Vec<Page> = Vec::new();
    for mapping in mappings {
        pages.push(Page::new(mapping)?);
    }
    drop(csv);

    // 1.6 Create not yet mapped pages.
    let new_files = get_new_md_files(&pages)?;
    for new_file in new_files {
        let mapping = PageToFileMapping {
            page_id: None,
            markdown_name: new_file,
        };
        let page = Page::new(mapping)?;
        pages.push(page);
    }

    Ok((pages, communicator, config))
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

/// Gathers all paths of .md files that haven't yet been pushed to the CMS.
fn get_new_md_files(pages: &[Page]) -> anyhow::Result<Vec<String>> {
    // Get all files in the markdown dir.
    let mut new_file_paths: Vec<String> = Vec::new();
    for entry in WalkDir::new(MARKDOWN_DIR) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path().to_string_lossy().to_string();
        // This unwrapping has to be safe because WalkDir always appends the config.markdown to the path.
        let path = path.strip_prefix(MARKDOWN_DIR).unwrap();
        new_file_paths.push(path.parse()?);
    }

    // Only retain the files that are not in the mapping table yet, i.e.: all files that are new.
    new_file_paths.retain(|path| {
        pages
            .iter()
            .filter(|page| page.mapping.markdown_name.eq(path))
            .count()
            == 0
    });
    Ok(new_file_paths)
}
