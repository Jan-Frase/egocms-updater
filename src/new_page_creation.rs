use crate::types::communicator::Communicator;
use crate::types::config::{Config, MAPPING_TABLE_PATH};
use crate::types::page::{MappedPage, PageContent, PageToFileMapping};
use anyhow::{Context, bail};
use serde::Deserialize;
use std::collections::HashMap;

// TODO: Move this into communicator
#[derive(Deserialize)]
struct NewPageResponse {
    field: NewPageField,
}

#[derive(Deserialize)]
struct NewPageField {
    id: u64,
}

/// Creates an EgoCMS page for every `Page` that has no `page_id` yet,
/// stores the returned id in the page's mapping, and rewrites the mapping table.
pub fn push_new_pages(
    mut pending_pages: Vec<PageContent>,
    mut pushed_pages: Vec<MappedPage>,
    config: &Config,
    communicator: &mut Communicator,
) -> anyhow::Result<(Vec<MappedPage>, HashMap<String, u64>)> {
    // Map the markdown path of a page to its id.
    let mut md_name_to_id_map: HashMap<String, u64> = pushed_pages
        .iter()
        .map(|page| (page.content.markdown_name.clone(), page.page_id))
        .collect();
    // Write the new mapping table to disk.
    let mut wtr = csv::WriterBuilder::new()
        .comment(Some(b'#'))
        .from_path(MAPPING_TABLE_PATH)?;
    for page in &pushed_pages {
        wtr.serialize(PageToFileMapping {
            page_id: page.page_id,
            markdown_name: page.content.markdown_name.clone(),
        })?;
    }
    wtr.flush()?;
    // A page can only be created once its parent has an id. Instead of sorting by the parent tree,
    // we keep looping over the unmapped pages until a full pass creates nothing new.
    while !pending_pages.is_empty() {
        let pending_length_before = pending_pages.len();
        let mut still_pending = Vec::new();

        for pending_page in pending_pages {
            let parent_name = get_parent_name(config, &pending_page.markdown_name)?;

            // Look up the parent's id. `None` means the parent is missing or not created yet.
            match md_name_to_id_map.get(&parent_name) {
                Some(parent_id) => {
                    // Create the page via the API.
                    let newly_mapped_page = create_page(communicator, pending_page, *parent_id)?;
                    // Logging :)
                    println!(
                        "=> Created: {} (id {}) below {parent_name}",
                        newly_mapped_page.content.markdown_name, newly_mapped_page.page_id
                    );
                    // Update the parent_name -> id map.
                    md_name_to_id_map.insert(
                        newly_mapped_page.content.markdown_name.clone(),
                        newly_mapped_page.page_id,
                    );
                    // Write the change to the csv file.
                    wtr.serialize(PageToFileMapping {
                        page_id: newly_mapped_page.page_id,
                        markdown_name: newly_mapped_page.content.markdown_name.clone(),
                    })?;
                    wtr.flush()?;
                    // Actually push the change :)
                    pushed_pages.push(newly_mapped_page);
                }
                None => still_pending.push(pending_page),
            }
        }

        // Abort if nothing was created.
        if still_pending.len() == pending_length_before {
            bail!(
                "Could not create the pages {:?}: parent missing.",
                still_pending
                    .iter()
                    .map(|p| &p.markdown_name)
                    .collect::<Vec<_>>()
            );
        }

        pending_pages = still_pending;
    }

    Ok((pushed_pages, md_name_to_id_map))
}

/// Returns the markdown name of the page that is the parent of `markdown_name`.
fn get_parent_name(config: &Config, markdown_name: &str) -> anyhow::Result<String> {
    // There are four cases:
    // 1. "2026/Research/blub.md"     => parent is 2026/Research/Research.md
    // 2. "2026/Research/Research.md" => parent is 2026/2026.md
    // 3. "Research/Research.md"      => parent is the home_page from the config
    // 4. "blub.md"                   => parent is the home_page from the config
    let mut sections: Vec<&str> = markdown_name.split('/').collect();
    let name = sections.pop().context("Empty markdown name.")?;
    let stem = name
        .strip_suffix(".md")
        .with_context(|| format!("{markdown_name} does not end in `.md`."))?;

    let parent = match sections.last() {
        // Case 4: the file lies directly in the pages dir.
        None => config.home_page.clone(),
        // Cases 2 and 3: the file is the "index" of its directory.
        Some(&last) if last == stem => {
            sections.pop();
            sections.last().map_or_else(
                || config.home_page.clone(),
                |grandpa| {
                    format!(
                        "{}/{grandpa}/{grandpa}.md",
                        sections[..sections.len() - 1].join("/")
                    )
                    .trim_start_matches('/')
                    .to_string()
                },
            )
        }
        // Case 1: the parent is the index of the containing directory.
        Some(&last) => format!("{}/{last}.md", sections.join("/")),
    };

    Ok(parent)
}

/// Creates the page in the CMS below `parent_id` and returns the newly mapped page.
fn create_page(
    communicator: &mut Communicator,
    pending_page: PageContent,
    parent_id: u64,
) -> anyhow::Result<MappedPage> {
    let name = pending_page.get_name()?;

    let response = communicator.new_child(parent_id, name, &pending_page.title)?;
    let new_page_response: NewPageResponse = response.json()?;
    let new_page_id = new_page_response.field.id;
    let mapped_page = MappedPage::new(pending_page, new_page_id);
    Ok(mapped_page)
}
