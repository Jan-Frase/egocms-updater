use serde_json::{json, Value, Map};
use crate::cms::cms_page::CmsPage;
use crate::cms::MARKER_KEY;


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
    /// Uploads a file as a new child of `parent`. Only works on media sites.
    fn new_file(
        &mut self,
        parent: u64,
        name: &str,
        bytes: &[u8],
        extra: &Map<String, Value>,
    ) -> anyhow::Result<CmsPage>;
    /// Replaces the content of an existing file page.
    fn update_file(&mut self, id: u64, name: &str, bytes: &[u8]) -> anyhow::Result<()>;
}


/// Builds the marker. The hash gets a prefix, as EgoCMS would turn an all-digit string into a number.
pub fn marker(source: &str, hash: Option<&str>) -> Value {
    let mut marker = json!({ "source": source });
    if let Some(hash) = hash {
        marker["hash"] = format!("sha256:{hash}").into();
    }
    marker
}

/// An `extra` containing nothing but the marker.
pub fn marker_extra(source: &str, hash: Option<&str>) -> Map<String, Value> {
    std::iter::once((MARKER_KEY.to_string(), marker(source, hash))).collect()
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
    use serde_json::{json, Value, Map};
    use crate::cms::cms_trait::{marker, set_content};

    #[test]
    fn builds_markers() {
        assert_eq!(marker("a.md", None), json!({ "source": "a.md" }));
        assert_eq!(
            marker("a.png", Some("0123")),
            json!({ "source": "a.png", "hash": "sha256:0123" })
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
