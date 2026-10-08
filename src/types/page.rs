use crate::types::communicator::Communicator;
use crate::types::config::MARKDOWN_DIR;
use anyhow::{Context, bail};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::path::Path;

/// A simple tuple representing a mapping between EgoCMS page and a markdown file.
///
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
    pub html: Option<String>,
}

impl MappedPage {
    #[must_use]
    pub const fn new(content: PageContent, page_id: u64) -> Self {
        Self {
            page_id,
            content,
            html: None,
        }
    }

    pub fn new_from_mapping(mapping: &PageToFileMapping) -> anyhow::Result<Self> {
        let content = PageContent::new(mapping.markdown_name.as_str())?;
        let page_id = mapping.page_id;

        Ok(Self {
            page_id,
            content,
            html: None,
        })
    }

    /// Executes the actual updating of the EgoCMS page.
    pub fn update(
        &self,
        communicator: &mut Communicator,
        json_content_path: &str,
    ) -> anyhow::Result<()> {
        let mut extra = communicator.get_extra(self.page_id)?;
        let mut field = communicator.get_field(self.page_id)?;

        if self.is_up_to_date(&extra, &field, json_content_path)? {
            return Ok(());
        }

        // 1. Update the extra.
        match extra.pointer_mut(json_content_path) {
            None => bail!("Missing or invalid content field."),
            Some(content) => *content = self.html.clone().into(),
        }

        // Wrap it like this: { extra: $old_extra$ }
        let mut wrapped_extra = Map::new();
        wrapped_extra.insert("extra".into(), extra.take());

        // Send the updated and wrapped JSON to EgoCMS.
        communicator.update_extra(self.page_id, &wrapped_extra.into())?;

        // 2. Update the field.
        *field.pointer_mut("/name").unwrap() = self.content.get_name()?.into();
        *field.pointer_mut("/title").unwrap() = self.content.title.clone().into();
        let mut wrapped_field = Map::new();
        wrapped_field.insert("field".into(), field.take());
        communicator.update_field(self.page_id, wrapped_field.into())?;

        println!("=> Updated: {}", self.content.markdown_name);
        Ok(())
    }

    /// Checks whether the page is up to date or not.
    /// It compares the JSON sent by the EgoCMS API against the markdown content.
    /// The `json_content_path` identifies the relevant part of the JSON body.
    fn is_up_to_date(&self, extra: &Value, field: &Value, json_content_path: &str) -> anyhow::Result<bool> {
        // 1. Compare the text.
        // Extract the relevant JSON section.
        let online_content = extra
            .pointer(json_content_path)
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!(format!("The extra section of the pages json: {extra} does not contain the path: {json_content_path}!")))?;

        // Are they the same?
        let is_text_equal = self.html.as_ref().unwrap().eq(&online_content.to_string());

        // 2. Compare name and title.
        let name = field
            .pointer("/name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!(format!("The field section of the pages json: {field} does not contain the path: `name`!")))?;
        let name = name.strip_suffix(".md").unwrap_or(name);
        let title = field
            .pointer("/title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!(format!("The field section of the pages json: {field} does not contain the path: `title`!")))?;

        let is_name_equal = self.content.get_name()?.eq(name);
        let is_title_equal = self.content.title.eq(&title);

        Ok(is_text_equal && is_name_equal && is_title_equal)
    }
}
