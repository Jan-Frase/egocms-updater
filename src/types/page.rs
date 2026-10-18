use crate::types::communicator::Communicator;
use crate::types::config::MARKDOWN_DIR;
use anyhow::{Context, bail};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::path::Path;

/// This struct represents a markdown file.
/// It is not yet connected to a CMS page.
#[derive(Debug)]
pub struct PageContent {
    pub markdown_name: String,
    /// The relevant text, at the beginning this is the raw md text, then it's converted to html.
    pub text: String,
    /// The title shown at the top of the page, extracted from the first line in the .md file.
    pub title: String,
}

impl PageContent {
    /// Simple constructor for a page :)
    pub fn new(markdown_name: &str) -> anyhow::Result<Self> {
        // 1. Create the full markdown path, read it from disk and parse it to html.
        let file = fs::read_to_string(Path::new(MARKDOWN_DIR).join(markdown_name))?;

        // 2. Extract the title.
        let mut lines = file.lines();
        let title = lines.by_ref().take(1).next().context(format!("The file: {markdown_name} is empty but is expected to have at least one line to serve as the title."))?;
        let title = title.strip_prefix("# ").unwrap_or(title).to_string();
        // Join the remaining lines
        let markdown = lines.join("\n");

        // 2. Done :)
        let markdown_name = markdown_name.to_string();
        let page = Self {
            markdown_name,
            text: markdown,
            title,
        };

        Ok(page)
    }

    pub fn get_name(&self) -> anyhow::Result<&str> {
        // Research/cosemos_problem.md => cosemos_problem
        let file_name = self
            .markdown_name
            .rsplit('/')
            .next()
            .context("Empty markdown name.")?;
        let name = file_name.strip_suffix(".md").unwrap_or(file_name);
        Ok(name)
    }
}

/// This struct represents a markdown file mapped to a CMS page.
/// Works through composition.
#[derive(Debug)]
pub struct MappedPage {
    pub page_id: u64,
    pub content: PageContent,
    pub json: Value,
}

impl MappedPage {
    #[must_use]
    pub const fn new(content: PageContent, page_id: u64, json: Value) -> Self {
        Self {
            page_id,
            content,
            json
        }
    }

    /// Executes the actual updating of the EgoCMS page.
    pub fn update(
        &mut self,
        communicator: &mut Communicator,
        json_content_path: &str,
    ) -> anyhow::Result<()> {
        // 1. Update the field.
        // This must happen first: `field` contains a stale serialized copy of `extra`,
        // so updating it after wards would revert the text we just wrote.
        *self.json.pointer_mut("/field/name").unwrap() = self.content.get_name()?.into();
        *self.json.pointer_mut("/field/title").unwrap() = self.content.title.clone().into();

        // Wrap it.
        let mut wrapped_field = Map::new();
        wrapped_field.insert("field".into(), self.json.take());

        // Update.
        communicator.update_field(self.page_id, &Value::from(wrapped_field))?;

        // 2. Update the extra.
        match extra.pointer_mut(json_content_path) {
            None => bail!("Missing or invalid content field."),
            Some(content) => *content = self.html.clone().into(),
        }

        // Wrap it like this: { extra: $old_extra$ }
        let mut wrapped_extra = Map::new();
        wrapped_extra.insert("extra".into(), extra.take());

        // Send the updated and wrapped JSON to EgoCMS.
        communicator.update_extra(self.page_id, &wrapped_extra.into())?;

        println!("=> Updated: {}", self.content.markdown_name);
        Ok(())
    }
}
