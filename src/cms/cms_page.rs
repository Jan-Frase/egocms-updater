use crate::cms::MARKER_KEY;
use anyhow::Context;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// The parts of an EgoCMS page this tool cares about.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CmsPage {
    pub id: u64,
    pub name: String,
    pub title: String,
    /// The page type, e.g. `page` or `blog/entry`.
    pub page_type: String,
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
            page_type: text("type"),
            extra: json
                .get("extra")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default(),
        })
    }

    /// Why the page can't be represented as a markdown file, or `None` if it can:
    /// its type has to be one of `page_types`, and its content at most a single text block.
    pub fn not_representable(&self, page_types: &[&str]) -> Option<String> {
        if !page_types.contains(&self.page_type.as_str()) {
            Some(format!("of type `{}`", self.page_type))
        } else if !self.holds_only_text() {
            Some("with more than a single text block".into())
        } else {
            None
        }
    }

    /// Whether the content of the page is at most a single text block, i.e. all a markdown file can hold.
    pub fn holds_only_text(&self) -> bool {
        let Some(areas) = self.extra.get("_blocks").and_then(Value::as_object) else {
            return true;
        };
        areas.iter().all(|(area, blocks)| match blocks.as_array() {
            Some(blocks) if !blocks.is_empty() => area == "center" && *blocks == ["text"],
            _ => true,
        })
    }

    /// The `source` stored in the marker, i.e., the path of the file this page was created from.
    pub fn source(&self) -> Option<&str> {
        self.marker()?.get("source")?.as_str()
    }

    /// The hashes of the files in the page's pool, as stored in the marker by the last upload.
    /// `None` if the page has no marker, i.e., what its pool contains is unknown.
    pub fn pool_hashes(&self) -> Option<BTreeMap<String, String>> {
        let files = self.marker()?.get("files").and_then(Value::as_object);
        Some(
            files
                .into_iter()
                .flatten()
                .filter_map(|(name, hash)| Some((name.clone(), hash.as_str()?.to_string())))
                .collect(),
        )
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
            "field": { "id": 7, "name": "SMASH", "title": "Smash!", "type": "page", "extra": "a:1:{stale}" },
            "extra": {
                "_sync": { "source": "Research/SMASH.md", "files": { "plot.png": "sha256:ab" } },
                "_contents": { "center": [{ "content1": "<p>hi</p>" }] }
            }
        }))
        .unwrap();
        assert_eq!(page.id, 7);
        assert_eq!(page.name, "SMASH");
        assert_eq!(page.title, "Smash!");
        assert_eq!(page.page_type, "page");
        assert_eq!(page.source(), Some("Research/SMASH.md"));
        assert_eq!(
            page.pool_hashes(),
            Some([("plot.png".to_string(), "sha256:ab".to_string())].into())
        );
        assert_eq!(page.content(), Some("<p>hi</p>"));
    }

    #[test]
    fn tolerates_pages_without_marker_or_content() {
        let page = CmsPage::from_json(&json!({ "field": { "id": 1 }, "extra": [] })).unwrap();
        assert_eq!(page.source(), None);
        assert_eq!(page.pool_hashes(), None);
        assert_eq!(page.content(), None);
        assert!(page.extra.is_empty());

        assert!(CmsPage::from_json(&json!(null)).is_err());

        // A marker without files means the pool is known to be empty (EgoCMS drops empty objects).
        let page = CmsPage::from_json(
            &json!({ "field": { "id": 1 }, "extra": { "_sync": { "source": "a.md" } } }),
        )
        .unwrap();
        assert_eq!(page.pool_hashes(), Some(BTreeMap::new()));
        assert!(CmsPage::from_json(&json!({ "field": {} })).is_err());
    }

    #[test]
    fn recognizes_pages_with_only_text() {
        let page = |extra: Value| {
            CmsPage::from_json(&json!({ "field": { "id": 1 }, "extra": extra })).unwrap()
        };
        assert!(page(json!({})).holds_only_text());
        assert!(page(json!({ "_blocks": { "center": ["text"] } })).holds_only_text());
        assert!(page(json!({ "_blocks": { "center": [], "left": [] } })).holds_only_text());

        assert!(!page(json!({ "_blocks": { "center": ["text", "text"] } })).holds_only_text());
        assert!(!page(json!({ "_blocks": { "center": ["2columns"] } })).holds_only_text());
        assert!(
            !page(json!({ "_blocks": { "center": ["text"], "right": ["image"] } }))
                .holds_only_text()
        );
    }

    #[test]
    fn explains_why_a_page_is_not_representable() {
        let types = ["page"];
        let page = |page_type: &str, extra: Value| {
            CmsPage::from_json(&json!({ "field": { "id": 1, "type": page_type }, "extra": extra }))
                .unwrap()
        };
        assert_eq!(page("page", json!({})).not_representable(&types), None);
        assert_eq!(
            page("person", json!({})).not_representable(&types).unwrap(),
            "of type `person`"
        );
        assert_eq!(
            page(
                "page",
                json!({ "_blocks": { "center": ["text", "image"] } })
            )
            .not_representable(&types)
            .unwrap(),
            "with more than a single text block"
        );
    }
}
