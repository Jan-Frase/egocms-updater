//! A [`CmsApi`] that only reads: writes are skipped, so an upload can show what it would change.

use crate::cms::cms_page::CmsPage;
use crate::cms::cms_trait::CmsApi;
use anyhow::Result;
use serde_json::{Map, Value};

pub struct DryRun<'a> {
    pub cms: &'a mut dyn CmsApi,
}

impl CmsApi for DryRun<'_> {
    fn root_id(&mut self) -> Result<u64> {
        self.cms.root_id()
    }

    fn page(&mut self, id: u64) -> Result<CmsPage> {
        self.cms.page(id)
    }

    fn children(&mut self, id: u64) -> Result<Vec<CmsPage>> {
        self.cms.children(id)
    }

    fn descendants(&mut self, id: u64) -> Result<Vec<CmsPage>> {
        self.cms.descendants(id)
    }

    fn url(&mut self, id: u64) -> Result<String> {
        self.cms.url(id)
    }

    fn pool_list(&mut self, id: u64) -> Result<Vec<String>> {
        self.cms.pool_list(id)
    }

    fn pool_get(&mut self, id: u64, name: &str) -> Result<Vec<u8>> {
        self.cms.pool_get(id, name)
    }

    /// Pretends to create the page. As it doesn't exist, it gets the id 0.
    fn new_child(
        &mut self,
        _parent: u64,
        name: &str,
        title: &str,
        page_type: &str,
        extra: &Map<String, Value>,
    ) -> Result<CmsPage> {
        Ok(CmsPage {
            id: 0,
            name: name.into(),
            title: title.into(),
            page_type: page_type.into(),
            extra: extra.clone(),
        })
    }

    fn update(&mut self, _: u64, _: &str, _: &str, _: &Map<String, Value>) -> Result<()> {
        Ok(())
    }

    fn delete(&mut self, _: u64) -> Result<()> {
        Ok(())
    }

    fn pool_put(&mut self, _: u64, _: &str, _: &[u8]) -> Result<()> {
        Ok(())
    }

    fn pool_delete(&mut self, _: u64, _: &str) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cms::mock_client::{FakeCms, ROOT};
    use crate::local::local_page::LocalPage;
    use crate::local::local_state::LocalState;
    use crate::upload::sync::{Destination, sync};
    use serde_json::json;

    #[test]
    fn reports_the_changes_without_writing() {
        let mut cms = FakeCms::new();
        let old = cms.add(ROOT, "Old", &json!({ "_sync": { "source": "Old/Old.md" } }));
        let local = LocalState {
            pages: vec![
                LocalPage::parse("Home.md", "# Home\n[new](New/New.md)", "Home.md").unwrap(),
                LocalPage::parse("New/New.md", "# New", "Home.md").unwrap(),
            ],
        };
        let to = Destination {
            site: "parcio",
            language: "de",
            home_id: ROOT,
            home_page: "Home.md",
            page_type: "page",
            page_types: &["page"],
        };

        let report = sync(&mut DryRun { cms: &mut cms }, &local, &to).unwrap();
        assert_eq!((report.created, report.updated, report.deleted), (1, 1, 1));
        assert_eq!(cms.writes, Vec::<String>::new());
        assert!(cms.exists(old));
        assert!(cms.find("New").is_none());
    }
}
