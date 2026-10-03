use crate::MARKDOWN_DIR;
use crate::api_communication::Communicator;
use anyhow::bail;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::path::Path;

/// A simple tuple representing a mapping between EgoCMS page and markdown file.
/// The `page_id` could, for example, be "535".
/// The `markdown_name` could, for example, be "Research/JULEA.md".
#[derive(Debug, Deserialize, Serialize)]
pub struct PageToFileMapping {
    pub page_id: Option<String>,
    pub markdown_name: String,
}

/// This struct represents a single page.
/// It holds the mapping (i.e., the id and the markdown name), the `extra` section of the pages json and the markdown on disk converted to html.
#[derive(Debug)]
pub struct Page {
    /// Defines which id and markdown file are relevant to this page struct.
    pub mapping: PageToFileMapping,
    /// The relevant markdown file converted to HTML.
    html: String,
    /// The title shown at the top of the page, extracted from the first line in the .md file.
    pub title: String,
}

impl Page {
    /// Simple constructor for a page :)
    pub fn new(mapping: PageToFileMapping) -> anyhow::Result<Self> {
        // 1. Create the full markdown path, read it from disk and parse it to html.
        let md_path = Path::new(MARKDOWN_DIR).join(&mapping.markdown_name);
        let file = fs::read_to_string(md_path)?;
        // Extract the title.
        let mut lines = file.lines();
        let title = lines.by_ref().take(1).next().unwrap_or_else(|| panic!("The file: {} is empty but is expected to have at least one line to serve as the title.", mapping.markdown_name));
        let title = title.strip_prefix("# ").unwrap_or(title).to_string();

        let markdown = lines.skip(1).join("\n");
        let html = markdown::to_html(&markdown);

        // 2. Done :)
        let page = Self {
            mapping,
            html,
            title,
        };

        Ok(page)
    }

    /// Executes the actual updating of the EgoCMS page.
    pub fn update(
        &self,
        communicator: &Communicator,
        json_content_path: &str,
    ) -> anyhow::Result<()> {
        let page_id = match &self.mapping.page_id {
            None => bail!("Attempted to update a page without an EgoCMS ID."),
            Some(id) => id.as_str(),
        };
        let mut extra = communicator.get_extra(page_id)?;

        if self.is_up_to_date(&extra, json_content_path)? {
            return Ok(());
        }
        // Update the extra JSON.
        match extra.pointer_mut(json_content_path) {
            None => bail!("Missing or invalid content field."),
            Some(content) => *content = self.html.clone().into(),
        }

        // Wrap it like this: { extra: $old_extra$ }
        let mut wrapped_extra = Map::new();
        wrapped_extra.insert("extra".into(), extra.take());

        // Send the updated and wrapped JSON to EgoCMS.
        communicator.update_extra(page_id, &wrapped_extra.into())?;
        Ok(())
    }

    /// Checks whether the page is up to date or not.
    /// It compares the JSON sent by the EgoCMS API against the markdown content.
    /// The `json_content_path` identifies the relevant part of the JSON body.
    fn is_up_to_date(&self, extra: &Value, json_content_path: &str) -> anyhow::Result<bool> {
        // Extract the relevant JSON section.
        let online_content = extra
            .pointer(json_content_path)
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!(format!("The extra section of the pages json: {extra} does not contain the path: {json_content_path}!")))?;

        // Are they the same?
        Ok(online_content == self.html)
    }
}
