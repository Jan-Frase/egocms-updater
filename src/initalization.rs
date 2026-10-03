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
    println!("==========================");
    println!("1. Initializing Resources:");
    println!("==========================");

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

/// This function does a bunch of sanity checks to avoid silly mistakes.
/// It did get rather long, but oh well.
fn _check_table_and_config_correctness(
    mappings: &[PageToFileMapping],
    markdown_dir: &PathBuf,
    communicator: &Communicator,
) -> anyhow::Result<()> {
    // 1. are all ids and names in the table unique?
    print!("2.1. Are all IDs and names unique? ");
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
    print!("2.2: Does the markdown dir only contain files ending on `.md`?");
    let incorrect_names: Vec<_> = md_file_names
        .iter()
        .filter(|name| {
            let name = Path::new(name);
            name.extension()
                .is_none_or(|ext| !ext.eq_ignore_ascii_case("md")) // Treat files without extension as invalid
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
    print!("2.3: Are all files in the pages dir listed in the mapping table? ");
    let missing_table_entries: Vec<_> = md_file_names
        .iter()
        .filter(|name| !table_md_names.contains(name.as_str()))
        .collect();
    if !missing_table_entries.is_empty() {
        bail!("The files [{missing_table_entries:?}] are not listed in the mapping table!");
    }
    println!("=> Success.");

    // 4. do all mds in the table exist?
    print!("2.4: Do all .mds listed in the table exist? ");
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
    print!("2.5: Do all IDs in the table exist? ");
    let mut missing_ids = Vec::new();
    for id in &table_ids {
        let id = id
            .clone()
            .expect("An entry in the mapping table always has to have an id.");
        let page = communicator
            .get_extra(id.as_str())
            .with_context(|| format!("Failed to fetch page for ID: {id}"))?;

        // The API returns JSON `null` for non-existent pages.
        if page.is_null() {
            missing_ids.push(id);
        }
    }
    if !missing_ids.is_empty() {
        bail!("The IDs [{missing_ids:?} are listed in the table but do not exist.]");
    }
    println!("=> Success.");

    Ok(())
}
