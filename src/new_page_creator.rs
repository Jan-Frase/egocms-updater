use std::iter::zip;
use serde::Deserialize;
use walkdir::WalkDir;
use crate::{Config};
use crate::communicator::Communicator;
use crate::page::PageToFileMapping;

#[derive(Deserialize)]
struct NewPageResponse {
    field: NewPageField,
}

#[derive(Deserialize)]
struct NewPageField {
    id: u64,
}

pub fn create_new_pages(mappings: &mut Vec<PageToFileMapping>, config: &Config, communicator: &Communicator) -> anyhow::Result<()> {
    let mut new_file_paths = get_new_md_files(mappings, &config)?;

    let mut parents = get_parents(config, &new_file_paths);

    // Create a new EgoCMS page for each of these paths.
    // I could sort them based on the parent tree, but instead I just loop until all are mapped.
    loop {
        let mut to_remove: Vec<usize> = Vec::new();

        for (index, (file, parent)) in itertools::enumerate(zip(&new_file_paths, &parents)) {
            create_page(mappings, communicator, &mut to_remove, index, file, parent)?;
        }

        // TODO: Emit warning if some pages are left uncreated.
        if to_remove.is_empty() {
            break;
        }

        // remove any now mapped entries from the lists
        for index in to_remove.iter().rev() {
            new_file_paths.remove(*index);
            parents.remove(*index);
        }
    }

    // Write the new mapping_table to disk.
    let mut wtr = csv::WriterBuilder::new()
        .comment(Some(b'#'))
        .from_path(&config.mapping_table)?;
    for mapping in mappings {
        println!("{:?}", mapping);
        wtr.serialize(mapping)?;
    }
    wtr.flush()?;

    // TODO: Add commit to the action.
    Ok(())
}

fn get_new_md_files(mappings: &mut Vec<PageToFileMapping>, config: &&Config) -> anyhow::Result<Vec<String>> {
    // Get all files in the markdown dir.
    let mut new_file_paths: Vec<String> = Vec::new();
    let prefix = config.markdown_dir.to_string_lossy().to_string();
    for entry in WalkDir::new(&config.markdown_dir) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path().to_string_lossy().to_string();
        // This unwrapping has to be safe because WalkDir always appends the config.markdown to the path.
        let path = path.strip_prefix(&prefix).unwrap();
        new_file_paths.push(path.parse()?);
    }

    // Only retain the files that are not in the mapping table yet, i.e.: all files that are new.
    new_file_paths.retain(|path| {
        mappings
            .iter()
            .filter(|mapping| mapping.markdown_name.eq(path))
            .count()
            == 0
    });
    Ok(new_file_paths)
}

fn get_parents(config: &Config, new_file_paths: &Vec<String>) -> Vec<String> {
    // Get the parent for each file.
    let mut parents = Vec::new();
    for file in new_file_paths {
        // Get the parent of each file_path.
        // There are three cases:
        // 1. "2026/Research/blub.md"
        // => The parent is 2026/Research/Research.md
        // 2. "2026/Research/Research.md"
        // => The parent is 2026/2026.md
        // 3. "Research/Research.md"
        // => The parent is the home_page defined in the config.
        let mut sections: Vec<_> = file.split("/").collect();
        // This unwrapping has to be safe as the sections vec, can never be empty.
        let name = sections.pop().unwrap();
        let stem = name.strip_suffix(".md").unwrap();

        let parent = match sections.last() {
            // This covers case 2. and 3.
            // There exists some directory containing this file, and it matches the file name
            Some(&last) if last == stem => {
                let _ = sections.pop();
                match sections.last() {
                    // This covers case 2. some grandparent-dir exists.
                    Some(grandpa) => format!("{grandpa}/{grandpa}.md"),
                    // This covers case 3. no grandparent exists => homepage is parent.
                    None => config.home_page.clone(),
                }
            }
            // This covers case 1.
            // The parent dir name does not match the file name
            _ => format!("{}/{}.md", sections.join("/"), sections.last().unwrap()),
        };

        parents.push(parent);
    }
    parents
}

fn create_page(mappings: &mut Vec<PageToFileMapping>, communicator: &Communicator, to_remove: &mut Vec<usize>, index: usize, file: &String, parent: &String) -> anyhow::Result<()> {
    // Get the mapping entry of the parent.
    let mut parent_id_mapping: Vec<_> = mappings
        .iter()
        .filter(|mapping| mapping.markdown_name.eq(parent))
        .collect();
    // If the parent of this file is not mapped itself, we do not have an id for it.
    if parent_id_mapping.is_empty() {
        // Thus, we skip it.
        return Ok(());
    }

    // Get the id of the parent.
    assert_eq!(parent_id_mapping.len(), 1);

    let parent_mapping = parent_id_mapping.pop().unwrap();
    let parent_id = &parent_mapping.page_id;

    // Generate the new title based on the .md name :)
    // Turn Research/cosemos_problem.md to cosemos_problem.md
    let new_child_name = file.split(r"/").last().unwrap();
    // turns into cosemos_problem
    let new_child_name = new_child_name.strip_suffix(".md").unwrap_or(new_child_name);

    // Create a new page with this id.
    let response = communicator.new_child(parent_id, &new_child_name)?;

    // Extract the id of the new page from the response.
    let new_page_response: NewPageResponse = response.json()?;

    let new_mapping = PageToFileMapping {
        page_id: new_page_response.field.id.to_string(),
        markdown_name: file.clone(),
    };

    mappings.push(new_mapping);
    to_remove.push(index);

    Ok(())
}