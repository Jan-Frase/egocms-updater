//! An in-memory [`Cms`] for tests.

use anyhow::{Context, bail};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use crate::cms::cms_page::CmsPage;
use crate::cms::cms_trait::CmsApi;

struct Entry {
    page: CmsPage,
    parent: Option<u64>,
    deleted: bool,
    file: Option<Vec<u8>>,
}

pub struct FakeCms {
    entries: BTreeMap<u64, Entry>,
    next_id: u64,
    /// Every write request, e.g. `update 3`, so tests can check what was sent.
    pub writes: Vec<String>,
}

pub const ROOT: u64 = 1;

impl Default for FakeCms {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeCms {
    /// A site containing only its root page.
    pub fn new() -> Self {
        let mut cms = Self {
            entries: BTreeMap::new(),
            next_id: ROOT,
            writes: Vec::new(),
        };
        cms.insert(None, "Home", "Home", Map::new());
        cms
    }

    /// Adds a page directly, without logging it as a write. `extra` has to be a JSON object.
    pub fn add(&mut self, parent: u64, name: &str, extra: &Value) -> u64 {
        let extra = extra.as_object().cloned().unwrap_or_default();
        self.insert(Some(parent), name, name, extra)
    }

    fn insert(
        &mut self,
        parent: Option<u64>,
        name: &str,
        title: &str,
        extra: Map<String, Value>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let page = CmsPage {
            id,
            name: name.into(),
            title: title.into(),
            extra,
        };
        self.entries.insert(
            id,
            Entry {
                page,
                parent,
                deleted: false,
                file: None,
            },
        );
        id
    }

    fn entry(&self, id: u64) -> anyhow::Result<&Entry> {
        self.entries
            .get(&id)
            .filter(|entry| !entry.deleted)
            .with_context(|| format!("Invalid parameters: no page {id}"))
    }

    fn entry_mut(&mut self, id: u64) -> anyhow::Result<&mut Entry> {
        self.entries
            .get_mut(&id)
            .filter(|entry| !entry.deleted)
            .with_context(|| format!("Invalid parameters: no page {id}"))
    }

    /// The current state of a page, panics if it doesn't exist (anymore).
    pub fn get(&self, id: u64) -> &CmsPage {
        &self.entry(id).unwrap().page
    }

    pub fn file(&self, id: u64) -> Option<&[u8]> {
        self.entry(id).ok()?.file.as_deref()
    }

    pub fn exists(&self, id: u64) -> bool {
        self.entry(id).is_ok()
    }

    pub fn find(&self, name: &str) -> Option<u64> {
        self.entries
            .values()
            .find(|entry| !entry.deleted && entry.page.name == name)
            .map(|entry| entry.page.id)
    }

    fn child_ids(&self, id: u64) -> Vec<u64> {
        self.entries
            .values()
            .filter(|entry| !entry.deleted && entry.parent == Some(id))
            .map(|entry| entry.page.id)
            .collect()
    }
}

impl CmsApi for FakeCms {
    fn root_id(&mut self) -> anyhow::Result<u64> {
        Ok(ROOT)
    }

    fn page(&mut self, id: u64) -> anyhow::Result<CmsPage> {
        Ok(self.entry(id)?.page.clone())
    }

    fn children(&mut self, id: u64) -> anyhow::Result<Vec<CmsPage>> {
        self.entry(id)?;
        Ok(self
            .child_ids(id)
            .into_iter()
            .map(|child| self.get(child).clone())
            .collect())
    }

    fn descendants(&mut self, id: u64) -> anyhow::Result<Vec<CmsPage>> {
        let mut result = Vec::new();
        for child in self.children(id)? {
            let child_id = child.id;
            result.push(child);
            result.extend(self.descendants(child_id)?);
        }
        Ok(result)
    }

    fn new_child(
        &mut self,
        parent: u64,
        name: &str,
        title: &str,
        _page_type: &str,
        extra: &Map<String, Value>,
    ) -> anyhow::Result<CmsPage> {
        self.entry(parent)?;
        self.writes.push(format!("new_child {name}"));
        let id = self.insert(Some(parent), name, title, extra.clone());
        Ok(self.get(id).clone())
    }

    fn update(
        &mut self,
        id: u64,
        name: &str,
        title: &str,
        extra: &Map<String, Value>,
    ) -> anyhow::Result<()> {
        self.writes.push(format!("update {name}"));
        let page = &mut self.entry_mut(id)?.page;
        page.name = name.into();
        page.title = title.into();
        page.extra = extra.clone();
        Ok(())
    }

    fn delete(&mut self, id: u64) -> anyhow::Result<()> {
        if self.entry(id)?.parent.is_none() {
            bail!("Can't delete root page.");
        }
        let name = self.get(id).name.clone();
        self.writes.push(format!("delete {name}"));
        let mut stack = vec![id];
        while let Some(current) = stack.pop() {
            stack.extend(self.child_ids(current));
            self.entry_mut(current)?.deleted = true;
        }
        Ok(())
    }

    fn url(&mut self, id: u64) -> anyhow::Result<String> {
        let mut names = Vec::new();
        let mut current = self.entry(id)?;
        while let Some(parent) = current.parent {
            names.push(current.page.name.to_lowercase());
            current = self.entry(parent)?;
        }
        names.reverse();
        Ok(format!("/{}", names.join("/")))
    }

    fn new_file(
        &mut self,
        parent: u64,
        name: &str,
        bytes: &[u8],
        extra: &Map<String, Value>,
    ) -> anyhow::Result<CmsPage> {
        self.entry(parent)?;
        self.writes.push(format!("new_file {name}"));
        let id = self.insert(Some(parent), name, name, extra.clone());
        self.entry_mut(id)?.file = Some(bytes.to_vec());
        Ok(self.get(id).clone())
    }

    fn update_file(&mut self, id: u64, name: &str, bytes: &[u8]) -> anyhow::Result<()> {
        self.writes.push(format!("update_file {name}"));
        self.entry_mut(id)?.file = Some(bytes.to_vec());
        Ok(())
    }
}
