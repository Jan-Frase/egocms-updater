use crate::api_communication::Communicator;
use crate::page::Page;
use crate::{Config, MAPPING_TABLE_PATH};
use anyhow::{Context, bail};
use serde::Deserialize;

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
    pages: &mut [Page],
    config: &Config,
    communicator: &Communicator,
) -> anyhow::Result<()> {
    // A page can only be created once its parent has an id. Instead of sorting by the parent tree,
    // we keep looping over the unmapped pages until a full pass creates nothing new.
    loop {
        let mut created_any = false;
        let mut unmapped_left = false;

        for index in 0..pages.len() {
            // If it is already mapped, skip.
            if pages[index].mapping.page_id.is_some() {
                continue;
            }

            let parent_name = get_parent(config, &pages[index].mapping.markdown_name)?;

            // Look up the parent's id. `None` means the parent is missing or not created yet.
            let parent_id = pages
                .iter()
                .find(|page| page.mapping.markdown_name == parent_name)
                .and_then(|page| page.mapping.page_id.clone());

            let Some(parent_id) = parent_id else {
                unmapped_left = true;
                continue;
            };

            let new_id = create_page(communicator, &pages[index], &parent_id)?;
            println!(
                "=> Created: {} (id {new_id}) below {parent_name}",
                pages[index].mapping.markdown_name
            );
            pages[index].mapping.page_id = Some(new_id);
            created_any = true;
        }

        if !unmapped_left {
            break;
        }
        if !created_any {
            let stuck: Vec<&str> = pages
                .iter()
                .filter(|page| page.mapping.page_id.is_none())
                .map(|page| page.mapping.markdown_name.as_str())
                .collect();
            bail!(
                "Could not create the pages {stuck:?}: their parent page is missing from the mapping table or does not exist."
            );
        }
    }

    // Write the new mapping table to disk.
    let mut wtr = csv::WriterBuilder::new()
        .comment(Some(b'#'))
        .from_path(MAPPING_TABLE_PATH)?;
    for page in pages.iter() {
        wtr.serialize(&page.mapping)?;
    }
    wtr.flush()?;

    // TODO: Add commit to the action.
    Ok(())
}

/// Returns the markdown name of the page that is the parent of `markdown_name`.
fn get_parent(config: &Config, markdown_name: &str) -> anyhow::Result<String> {
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

/// Creates the page in the CMS below `parent_id` and returns the new page id.
fn create_page(
    communicator: &Communicator,
    page: &Page,
    parent_id: &str,
) -> anyhow::Result<String> {
    // Research/cosemos_problem.md => cosemos_problem
    let file_name = page
        .mapping
        .markdown_name
        .rsplit('/')
        .next()
        .context("Empty markdown name.")?;
    let name = file_name.strip_suffix(".md").unwrap_or(file_name);

    let response = communicator.new_child(parent_id, name, &page.title)?;
    let new_page_response: NewPageResponse = response.json()?;

    Ok(new_page_response.field.id.to_string())
}
