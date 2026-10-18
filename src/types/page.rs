use crate::types::communicator::Communicator;
use crate::types::config::MARKDOWN_DIR;
use anyhow::Context;
use itertools::Itertools;
use serde_json::{Value, json};
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
    /// The page as returned by EgoCMS: `{ "field": ..., "extra": ... }`.
    pub json: Value,
}

impl MappedPage {
    #[must_use]
    pub const fn new(content: PageContent, page_id: u64, json: Value) -> Self {
        Self {
            page_id,
            content,
            json,
        }
    }

    /// Replaces the content blocks of the page with a single text block containing `html`.
    /// Everything else in `extra` (layout, language settings, ...) is kept.
    pub fn set_html(&mut self, html: &str) -> anyhow::Result<()> {
        let extra = self
            .json
            .get_mut("extra")
            .and_then(Value::as_object_mut)
            .with_context(|| format!("{} has no `extra` section.", self.content.markdown_name))?;
        extra.insert("_blocks".into(), json!({ "center": ["text"] }));
        extra.insert(
            "_contents".into(),
            json!({ "center": [{ "content1": html, "extra": { "heading": "h2" } }] }),
        );
        Ok(())
    }

    /// Pushes the name, title and (already converted) text to the EgoCMS page.
    pub fn update(&self, communicator: &Communicator) -> anyhow::Result<()> {
        // Only send what changes: the `field` returned by EgoCMS contains a stale serialized copy of `extra`,
        // which would otherwise overwrite the new one.
        let field = json!({
            "name": self.content.get_name()?,
            "title": self.content.title,
        });
        communicator.update_page(self.page_id, &field, &self.json["extra"])?;

        println!("=> Updated: {}", self.content.markdown_name);
        Ok(())
    }
}
