use crate::types::communicator::Communicator;
use crate::types::config::{Config, Inputs, MARKDOWN_DIR};
use crate::types::page::PageContent;
use anyhow::Context;
use std::fs;
use walkdir::WalkDir;

/// Responsible for loading the core data needed for this application.
/// That is:
/// Vec<PageContent> - A vector containing all markdown files.
/// Communicator - Responsible for API calls.
/// Config - Stores various core aspects like the URL.
pub fn init(inputs: Inputs) -> anyhow::Result<(Vec<PageContent>, Communicator, Config)> {
    // 1 Load the provided config.
    let config: Config = toml::from_str(&fs::read_to_string(inputs.config_path)?)?;

    // 2 Open a connection to EgoCMS's REST API.
    let is_test_environment = config.rest_url.eq("https://localhost/rest/");
    if is_test_environment {
        eprintln!(
            "WARNING: As the URL is localhost, this is assumed to be a safe test environment. Invalid certificates will be accepted."
        );
    }
    let communicator = Communicator::new(
        config.rest_url.clone(),
        config.site_url.clone(),
        &inputs.user_id,
        &inputs.user_token,
        is_test_environment,
    )
    .context("Connecting to EgoCMS failed! Are the user-id and user-token correct?")?;

    // 3 Load Markdown files.
    let mut markdown_files: Vec<PageContent> = Vec::new();
    let markdown_paths = get_all_markdown_paths()?;
    for file_path in markdown_paths {
        let page = PageContent::new(&file_path)?;
        markdown_files.push(page);
    }

    Ok((markdown_files, communicator, config))
}

/// Gathers all paths of .md files.
fn get_all_markdown_paths() -> anyhow::Result<Vec<String>> {
    // Get all files in the markdown dir.
    let mut new_file_paths: Vec<String> = Vec::new();
    for entry in WalkDir::new(MARKDOWN_DIR) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path().display().to_string();
        // This unwrapping has to be safe because WalkDir always appends the config.markdown to the path.
        let path = path.strip_prefix(MARKDOWN_DIR).unwrap();
        new_file_paths.push(path.parse()?);
    }

    Ok(new_file_paths)
}
