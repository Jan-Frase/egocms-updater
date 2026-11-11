use crate::cms::cms_connection::CmsConnection;
use crate::cms::cms_page::CmsPage;
use crate::cms::cms_trait::CmsApi;
use anyhow::{Context, bail};
use reqwest::blocking::RequestBuilder;
use serde_json::{Map, Value, json};

pub struct SiteClient<'a> {
    pub(super) cms_connection: &'a CmsConnection,
    /// E.g. `https://localhost/rest/materialkit/de/`.
    pub(super) base: String,
    pub(super) language: String,
}

impl SiteClient<'_> {
    /// A client for the media site belonging to this site, where files have to be stored.
    pub fn media_site(&self) -> anyhow::Result<Self> {
        let site = CmsConnection::send(self.get("getMediaSite"))?;
        let name = site
            .get("name")
            .and_then(Value::as_str)
            .context("This site has no media site, so files can't be uploaded.")?;
        Ok(self.cms_connection.site(name, &self.language))
    }

    fn get(&self, path: &str) -> RequestBuilder {
        self.cms_connection
            .client
            .get(format!("{}{path}", self.base))
    }

    fn post(&self, path: &str) -> RequestBuilder {
        self.cms_connection
            .client
            .post(format!("{}{path}", self.base))
    }

    fn pages(&self, path: &str) -> anyhow::Result<Vec<CmsPage>> {
        match CmsConnection::send(self.get(path))? {
            Value::Null => Ok(Vec::new()),
            Value::Array(pages) => pages.iter().map(CmsPage::from_json).collect(),
            other => bail!("Expected a list of pages, got: {other}"),
        }
    }
}

impl CmsApi for SiteClient<'_> {
    fn root_id(&mut self) -> anyhow::Result<u64> {
        Ok(CmsPage::from_json(&CmsConnection::send(self.get("getRoot"))?)?.id)
    }

    fn page(&mut self, id: u64) -> anyhow::Result<CmsPage> {
        let json = CmsConnection::send(self.get("getPage").query(&[("id", id)]))?;
        if json.is_null() {
            bail!("There is no page with the id {id}.");
        }
        CmsPage::from_json(&json)
    }

    fn children(&mut self, id: u64) -> anyhow::Result<Vec<CmsPage>> {
        self.pages(&format!("{id}/getChildren"))
    }

    fn descendants(&mut self, id: u64) -> anyhow::Result<Vec<CmsPage>> {
        self.pages(&format!("{id}/getDescendants"))
    }

    fn new_child(
        &mut self,
        parent: u64,
        name: &str,
        title: &str,
        page_type: &str,
        extra: &Map<String, Value>,
    ) -> anyhow::Result<CmsPage> {
        let body = json!({
            "field": { "name": name, "title": title, "type": page_type, "inactive": 0, "nav_hide": 0 },
            "extra": extra,
        });
        let json = CmsConnection::send(self.post(&format!("{parent}/newChild")).json(&body))?;
        CmsPage::from_json(&json).with_context(|| format!("Creating the page {name} failed."))
    }

    fn update(
        &mut self,
        id: u64,
        name: &str,
        title: &str,
        extra: &Map<String, Value>,
    ) -> anyhow::Result<()> {
        // Only name and title are sent as `field`: the `field` returned by EgoCMS contains
        // an outdated copy of `extra`, which would overwrite the new one.
        let body =
            json!({ "param": { "field": { "name": name, "title": title }, "extra": extra } });
        let request = self
            .cms_connection
            .client
            .put(format!("{}{id}/update", self.base))
            .json(&body);
        CmsConnection::send(request)?;
        Ok(())
    }

    fn delete(&mut self, id: u64) -> anyhow::Result<()> {
        let request = self
            .cms_connection
            .client
            .delete(format!("{}{id}/delete", self.base));
        CmsConnection::send(request)?;
        Ok(())
    }

    fn url(&mut self, id: u64) -> anyhow::Result<String> {
        match CmsConnection::send(self.get(&format!("{id}/getUrl")))? {
            Value::String(url) => Ok(url),
            other => bail!("Expected the URL of page {id}, got: {other}"),
        }
    }

    fn new_file(
        &mut self,
        parent: u64,
        name: &str,
        bytes: &[u8],
        extra: &Map<String, Value>,
    ) -> anyhow::Result<CmsPage> {
        // newFile($source, $name, $options)
        let arguments = json!(["@file", name, { "title": name, "extra": extra }]);
        let request = self
            .post(&format!("{parent}/newFile"))
            .multipart(CmsConnection::upload_form(&arguments, name, bytes));
        // On failure (e.g. a forbidden file type) EgoCMS still answers with 201, but without a page.
        CmsPage::from_json(&CmsConnection::send(request)?)
            .with_context(|| format!("Uploading {name} failed."))
    }

    fn update_file(&mut self, id: u64, name: &str, bytes: &[u8]) -> anyhow::Result<()> {
        // updateFile($source, $name)
        let arguments = json!(["@file", name]);
        let request = self
            .post(&format!("{id}/updateFile"))
            .multipart(CmsConnection::upload_form(&arguments, name, bytes));
        match CmsConnection::send(request)? {
            Value::Bool(true) => Ok(()),
            other => bail!("Replacing the file {name} failed: {other}"),
        }
    }
}
