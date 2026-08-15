use crate::communicator::Communicator;
use anyhow::bail;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::fs;
use std::path::Path;

/// This struct represents a single page.
/// It holds the mapping (ie the id and the markdown name), the `extra` section of the pages json and the markdown on disk converted to html.
#[derive(Debug)]
pub struct Page {
    /// Defines which id and markdown file are relevant to this page struct.
    pub mapping: PageToFileMapping,
    /// The `extra` section of the pages JSON as send by the EgoCMS API.
    extra: Value,
    /// The relevant markdown file converted to HTML.
    html: String,
}

/// A simple tuple representing a mapping between EgoCMS page and markdown file.
/// The `page_id` could, for example, be "535".
/// The `markdown_name` could, for example, be "research/julea.md".
#[derive(Debug, Deserialize)]
pub struct PageToFileMapping {
    pub page_id: String,
    pub markdown_name: String,
}

impl Page {
    /// Simple constructor for a page :)
    pub fn new(
        mapping: PageToFileMapping,
        communicator: &Communicator,
        path_to_markdown: &Path,
    ) -> anyhow::Result<Self> {
        // 1. Get the json from the website.
        let mut page_json = communicator
            .get_page(mapping.page_id.as_str())?
            .json::<Value>()?;

        // We are only interested in the `extra` section.
        let extra = page_json
            .as_object_mut()
            .and_then(|obj| obj.remove("extra"))
            .ok_or_else(|| anyhow::anyhow!("Missing 'extra' key!"))?;

        // 2. Create the full markdown path, read it from disk and parse it to html.
        let md_path = path_to_markdown.join(&mapping.markdown_name);
        let markdown = fs::read_to_string(md_path)?;
        let html = markdown::to_html(&markdown);

        // 3. Done :)
        let page = Self {
            mapping,
            extra,
            html,
        };

        Ok(page)
    }

    /// Checks whether the page is up to date or not.
    /// It compares the JSON sent by the EgoCMS API against the markdown content.
    /// The `json_content_path` identifies the relevant part of the JSON body.
    pub fn is_up_to_date(&self, json_content_path: &str) -> anyhow::Result<bool> {
        // Extract the relevant JSON section.
        let online_content = self
            .extra
            .pointer(json_content_path)
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!(format!("The extra section of the pages json: {} does not contain the path: {json_content_path}!", self.extra)))?;

        // Are they the same?
        Ok(online_content == self.html)
    }

    /// Executes the actual updating of the EgoCMS page.
    pub fn update(
        &mut self,
        communicator: &Communicator,
        json_content_path: &str,
    ) -> anyhow::Result<()> {
        // Update the extra JSON.
        match self.extra.pointer_mut(json_content_path) {
            None => bail!("Missing or invalid content field."),
            Some(content) => *content = self.html.clone().into(),
        }

        // Wrap it like this: { extra: $old_extra$ }
        let mut wrapped_extra = Map::new();
        wrapped_extra.insert("extra".into(), self.extra.take());

        // Send the updated and wrapped JSON to EgoCMS.
        communicator.update_extra(self.mapping.page_id.as_str(), &wrapped_extra.into())?;
        Ok(())
    }
}
