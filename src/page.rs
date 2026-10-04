use crate::communicator::Communicator;
use crate::config::MARKDOWN_DIR;
use anyhow::{Context, bail};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::path::Path;

/// A simple tuple representing a mapping between EgoCMS page and a markdown file.
/// Useful for reading and writing the csv file :)
/// The `page_id` could, for example, be "535".
/// The `markdown_name` could, for example, be "Research/JULEA.md".
#[derive(Debug, Deserialize, Serialize)]
pub struct PageToFileMapping {
    pub page_id: u64,
    pub markdown_name: String,
}

/// This struct represents a markdown file.
/// It is not yet connected to a CMS page.
#[derive(Debug)]
pub struct PageContent {
    pub markdown_name: String,
    /// The relevant markdown file converted to HTML.
    html: String,
    /// The title shown at the top of the page, extracted from the first line in the .md file.
    pub title: String,
}

impl PageContent {
    /// Simple constructor for a page :)
    pub fn new(markdown_path: &str) -> anyhow::Result<Self> {
        // 1. Create the full markdown path, read it from disk and parse it to html.
        let file = fs::read_to_string(Path::new(MARKDOWN_DIR).join(markdown_path))?;

        // 2. Extract the title.
        let mut lines = file.lines();
        let title = lines.by_ref().take(1).next().context(format!("The file: {markdown_path} is empty but is expected to have at least one line to serve as the title."))?;
        let title = title.strip_prefix("# ").unwrap_or(title).to_string();
        // Join the remaining lines and convert them to html.
        let html = markdown::to_html(&lines.join("\n"));

        // 2. Done :)
        let markdown_path = markdown_path.to_string();
        let page = Self {
            markdown_name: markdown_path,
            html,
            title,
        };

        Ok(page)
    }
}

/// This struct represents a markdown file mapped to a CMS page.
/// Works through composition.
#[derive(Debug)]
pub struct MappedPage {
    pub page_id: u64,
    pub content: PageContent,
}

impl MappedPage {
    pub const fn new(content: PageContent, page_id: u64) -> Self {
        Self { page_id, content }
    }

    pub fn new_from_mapping(mapping: &PageToFileMapping) -> anyhow::Result<Self> {
        let content = PageContent::new(mapping.markdown_name.as_str())?;
        let page_id = mapping.page_id;

        Ok(Self { page_id, content })
    }

    /// Executes the actual updating of the EgoCMS page.
    pub fn update(
        &self,
        communicator: &Communicator,
        json_content_path: &str,
    ) -> anyhow::Result<()> {
        let mut extra = communicator.get_extra(self.page_id)?;

        if self.is_up_to_date(&extra, json_content_path)? {
            return Ok(());
        }
        // Update the extra JSON.
        match extra.pointer_mut(json_content_path) {
            None => bail!("Missing or invalid content field."),
            Some(content) => *content = self.content.html.clone().into(),
        }

        // Wrap it like this: { extra: $old_extra$ }
        let mut wrapped_extra = Map::new();
        wrapped_extra.insert("extra".into(), extra.take());

        // Send the updated and wrapped JSON to EgoCMS.
        communicator.update_extra(self.page_id, &wrapped_extra.into())?;
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
        Ok(online_content == self.content.html)
    }
}
