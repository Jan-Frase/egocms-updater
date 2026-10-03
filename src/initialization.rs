use crate::communicator::Communicator;
use crate::page::{Page, PageToFileMapping};
use std::fs;
use walkdir::WalkDir;
use crate::config::{Config, Inputs, MAPPING_TABLE_PATH, MARKDOWN_DIR};

/// Responsible for loading the core data needed for this application.
/// That is:
/// Vec<Page> - A vector containing all pages, even those not yet pushed to the CMS.
/// Communicator - Responsible for API calls.
/// Config - Stores various core aspects like the URL.
pub fn init(inputs: Inputs) -> anyhow::Result<(Vec<Page>, Communicator, Config)> {
    // 1.2 Load the provided config.
    let config: Config = toml::from_str(&fs::read_to_string(inputs.config_path)?)?;

    // 1.3 Open a connection to EgoCMS's REST API.
    let is_test_environment = config.rest_url.eq("https://localhost/rest/");
    let communicator = Communicator::new(
        config.rest_url.clone(),
        config.site_url.clone(),
        inputs.user_id,
        inputs.user_token,
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
