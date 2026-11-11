use crate::cms::MARKER_KEY;
use anyhow::Context;
use serde_json::{Map, Value};

/// The parts of an EgoCMS page this tool cares about.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CmsPage {
    pub id: u64,
    pub name: String,
    pub title: String,
    /// The complete `extra` section. It is always sent back as a whole, so nothing gets lost.
    pub extra: Map<String, Value>,
}

impl CmsPage {
    /// Parses the `{ "field": ..., "extra": ... }` JSON returned by EgoCMS.
    pub fn from_json(json: &Value) -> anyhow::Result<Self> {
        let field = json
            .get("field")
            .with_context(|| format!("Not a page: {json}"))?;
        let text = |key: &str| {
            field
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        Ok(Self {
            id: field
                .get("id")
                .and_then(Value::as_u64)
                .with_context(|| format!("Page without id: {json}"))?,
            name: text("name"),
            title: text("title"),
            extra: json
                .get("extra")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default(),
        })
    }

    /// The `source` stored in the marker, i.e., the path of the file this page was created from.
    pub fn source(&self) -> Option<&str> {
        self.marker()?.get("source")?.as_str()
    }

    /// The hash stored in the marker (only used for media files).
    pub fn hash(&self) -> Option<&str> {
        self.marker()?.get("hash")?.as_str()
    }

    fn marker(&self) -> Option<&Map<String, Value>> {
        self.extra.get(MARKER_KEY)?.as_object()
    }

    /// The HTML of the first content block.
    pub fn content(&self) -> Option<&str> {
        self.extra
            .get("_contents")?
            .get("center")?
            .get(0)?
            .get("content1")?
            .as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_pages() {
        let page = CmsPage::from_json(&json!({
            "field": { "id": 7, "name": "SMASH", "title": "Smash!", "extra": "a:1:{stale}" },
            "extra": {
                "_sync": { "source": "Research/SMASH.md", "hash": "sha256:ab" },
                "_contents": { "center": [{ "content1": "<p>hi</p>" }] }
            }
        }))
        .unwrap();
        assert_eq!(page.id, 7);
        assert_eq!(page.name, "SMASH");
        assert_eq!(page.title, "Smash!");
        assert_eq!(page.source(), Some("Research/SMASH.md"));
        assert_eq!(page.hash(), Some("sha256:ab"));
        assert_eq!(page.content(), Some("<p>hi</p>"));
    }

    #[test]
    fn tolerates_pages_without_marker_or_content() {
        let page = CmsPage::from_json(&json!({ "field": { "id": 1 }, "extra": [] })).unwrap();
        assert_eq!(page.source(), None);
        assert_eq!(page.content(), None);
        assert!(page.extra.is_empty());

        assert!(CmsPage::from_json(&json!(null)).is_err());
        assert!(CmsPage::from_json(&json!({ "field": {} })).is_err());
    }
}
