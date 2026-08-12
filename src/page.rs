use crate::communicator::Communicator;
use anyhow::bail;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::fs;
use std::path::Path;

#[derive(Debug)]
pub struct Page {
    pub mapping: PageToFileMapping,
    extra: Value,
    markdown: String,
}

#[derive(Debug, Deserialize)]
pub struct PageToFileMapping {
    pub page_id: String,
    pub markdown_name: String,
}

impl Page {
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

        // 2. Get the markdown.
        let md_path = path_to_markdown.join(&mapping.markdown_name);
        let markdown = fs::read_to_string(md_path)?;

        // 3. Done :)
        let page = Self {
            mapping,
            extra,
            markdown,
        };

        Ok(page)
    }

    pub fn is_up_to_date(&self) -> anyhow::Result<bool> {
        // Extract the relevant JSON section.
        let online_content = self
            .extra
            // TODO: Update when I get access to the real PARCIO sites.
            .pointer("/_contents/center/0/content1")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid content1 field."))?;

        // Are they the same?
        Ok(online_content == self.markdown)
    }

    pub fn update(&mut self, communicator: &Communicator) -> anyhow::Result<()> {
        // Update the extra JSON.
        // TODO: Update when I get access to the real PARCIO sites.
        match self.extra.pointer_mut("/_contents/center/0/content1") {
            None => bail!("Missing or invalid content1 field."),
            Some(content1) => *content1 = self.markdown.clone().into(),
        }

        // Wrap it like this: { extra: ... }
        let mut wrapped_extra = Map::new();
        wrapped_extra.insert("extra".into(), self.extra.take());

        // Send the updated JSON to EgoCMS.
        communicator.update_extra(self.mapping.page_id.as_str(), &wrapped_extra.into())?;
        Ok(())
    }
}
