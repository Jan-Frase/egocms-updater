use crate::cms::MARKER_KEY;
use crate::cms::cms_page::CmsPage;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

/// The subset of the EgoCMS REST API this tool needs. One instance talks to one site.
pub trait CmsApi {
    /// The id of the root page of the site.
    fn root_id(&mut self) -> anyhow::Result<u64>;
    fn page(&mut self, id: u64) -> anyhow::Result<CmsPage>;
    /// Direct (active) children of a page.
    fn children(&mut self, id: u64) -> anyhow::Result<Vec<CmsPage>>;
    /// All (active) pages below a page.
    fn descendants(&mut self, id: u64) -> anyhow::Result<Vec<CmsPage>>;
    fn new_child(
        &mut self,
        parent: u64,
        name: &str,
        title: &str,
        page_type: &str,
        extra: &Map<String, Value>,
    ) -> anyhow::Result<CmsPage>;
    /// Sets name and title, and replaces the whole `extra`.
    fn update(
        &mut self,
        id: u64,
        name: &str,
        title: &str,
        extra: &Map<String, Value>,
    ) -> anyhow::Result<()>;
    /// Moves a page and all its subpages to the trash.
    fn delete(&mut self, id: u64) -> anyhow::Result<()>;
    /// The URL of a page, relative to the server, e.g. `/research/smash`.
    fn url(&mut self, id: u64) -> anyhow::Result<String>;
    /// The names of the files attached to a page (its "pool").
    fn pool_list(&mut self, id: u64) -> anyhow::Result<Vec<String>>;
    /// Attaches a file to a page, replacing an existing file with the same name.
    fn pool_put(&mut self, id: u64, name: &str, bytes: &[u8]) -> anyhow::Result<()>;
    /// Removes a file from a page's pool.
    fn pool_delete(&mut self, id: u64, name: &str) -> anyhow::Result<()>;
}

/// Builds the marker: the page's source file and the hashes of the files in its pool.
/// The hashes are expected to carry a prefix like `sha256:`, as EgoCMS would turn an all-digit string into a number.
pub fn marker(source: &str, pool_hashes: &BTreeMap<String, String>) -> Value {
    let mut marker = json!({ "source": source });
    // EgoCMS drops empty objects anyway, a missing `files` means an empty pool.
    if !pool_hashes.is_empty() {
        marker["files"] = json!(pool_hashes);
    }
    marker
}

/// An `extra` containing nothing but the marker of a page with an empty pool.
pub fn marker_extra(source: &str) -> Map<String, Value> {
    std::iter::once((MARKER_KEY.to_string(), marker(source, &BTreeMap::new()))).collect()
}

/// A link to a page, in the form the EgoCMS editor uses. EgoCMS turns it into the page's URL when rendering.
pub fn page_link(site: &str, language: &str, id: u64) -> String {
    format!("index.php?site={site}&id={id}&lang={language}")
}

/// A link to a file in the pool of a page, in the form the EgoCMS editor uses.
pub fn pool_link(site: &str, language: &str, id: u64, name: &str) -> String {
    format!(
        "index.php?pool={}&site={site}&lang={language}&id={id}",
        form_encode(name)
    )
}

/// Encodes a query value like an HTML form does: `notes v1.txt` => `notes+v1.txt`.
fn form_encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            b' ' => "+".to_string(),
            other => format!("%{other:02X}"),
        })
        .collect()
}

/// Puts `html` into the first content block.
/// Pages without a content block (e.g., newly created ones) get a single text block.
pub fn set_content(extra: &mut Map<String, Value>, html: &str) {
    let existing = extra
        .get_mut("_contents")
        .and_then(|contents| contents.get_mut("center"))
        .and_then(|center| center.get_mut(0))
        .and_then(|block| block.get_mut("content1"));
    if let Some(content) = existing {
        *content = html.into();
        return;
    }
    extra.insert("_blocks".into(), json!({ "center": ["text"] }));
    extra.insert(
        "_contents".into(),
        json!({ "center": [{ "content1": html }] }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_markers() {
        assert_eq!(
            marker("a.md", &BTreeMap::new()),
            json!({ "source": "a.md" })
        );
        let hashes = [("plot.png".to_string(), "sha256:0123".to_string())].into();
        assert_eq!(
            marker("a.md", &hashes),
            json!({ "source": "a.md", "files": { "plot.png": "sha256:0123" } })
        );
        assert_eq!(
            Value::Object(marker_extra("a.md")),
            json!({ "_sync": { "source": "a.md" } })
        );
    }

    #[test]
    fn builds_links_like_the_editor() {
        assert_eq!(
            page_link("parcio", "de", 7),
            "index.php?site=parcio&id=7&lang=de"
        );
        assert_eq!(
            pool_link("parcio", "de", 7, "notes v1.txt"),
            "index.php?pool=notes+v1.txt&site=parcio&lang=de&id=7"
        );
        assert_eq!(
            pool_link("parcio", "de", 7, "Übung #1&2.pdf"),
            "index.php?pool=%C3%9Cbung+%231%262.pdf&site=parcio&lang=de&id=7"
        );
    }

    #[test]
    fn replaces_existing_content_and_keeps_other_blocks() {
        let mut extra = json!({
            "_layout": "default",
            "_blocks": { "center": ["text", "quote"] },
            "_contents": { "center": [
                { "_uid": "x", "content1": "<p>old</p>" },
                { "content1": "quote" }
            ]}
        })
        .as_object()
        .cloned()
        .unwrap();
        set_content(&mut extra, "<p>new</p>");
        assert_eq!(
            Value::Object(extra),
            json!({
                "_layout": "default",
                "_blocks": { "center": ["text", "quote"] },
                "_contents": { "center": [
                    { "_uid": "x", "content1": "<p>new</p>" },
                    { "content1": "quote" }
                ]}
            })
        );
    }

    #[test]
    fn creates_a_text_block_if_there_is_none() {
        let mut extra = Map::new();
        set_content(&mut extra, "<p>new</p>");
        assert_eq!(
            Value::Object(extra),
            json!({
                "_blocks": { "center": ["text"] },
                "_contents": { "center": [{ "content1": "<p>new</p>" }] }
            })
        );
    }
}
