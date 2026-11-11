use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;
use anyhow::Context;
use crate::cms::cms_client::SiteClient;
use crate::cms::cms_page::CmsPage;
use crate::cms::cms_trait::CmsApi;
use crate::local::local_page::LocalPage;
use anyhow::Result;
use crate::cms::cms_connection::CmsConnection;
use crate::config::config::Config;
use crate::config::credentials::Credentials;

pub fn download(config: &Config, credentials: &Credentials, out_dir: &Path) -> Result<()> {
    // 1. Open connection the cms.
    let cms_connection = CmsConnection::open(config, credentials)?;
    let mut site_client = cms_connection.site(&config.site, &config.language);
    let media_client = site_client.media_site()?;

    // 2. Get the home_id.
    let home_id = match config.home_page_id {
        Some(id) => id,
        None => site_client.root_id()?,
    };

    // 3. Gather all cms-pages.
    let mut page_parent_map: HashMap<Arc<CmsPage>, Arc<CmsPage>> = HashMap::new();
    let mut cms_pages: Vec<Arc<CmsPage>> = Vec::new();

    let home_page = Arc::new(site_client.page(home_id)?);
    download_cms_pages(&mut site_client, home_page.clone(), &mut page_parent_map, &mut cms_pages)?;

    // 4. Convert them to local pages.
    let mut local_pages: Vec<LocalPage> = Vec::new();
    for cms_page in cms_pages {
        let parent = page_parent_map.get(cms_page.deref()).context(format!("Failed to get parent for {:?}", cms_page))?;

        let local_page = LocalPage{
            relative_path: "".to_string(),
            name: "".to_string(),
            title: "".to_string(),
            markdown: "".to_string(),
            parent: None,
        };
    }

    // 5. Write them to disk.

    Ok(())
}


fn download_cms_pages(site_client: &mut SiteClient, current_page: Arc<CmsPage>, page_parent_map: &mut HashMap<Arc<CmsPage>, Arc<CmsPage>>, cms_pages: &mut Vec<Arc<CmsPage>>) -> anyhow::Result<()> {
    for child in site_client.children(current_page.id)? {
        let child = Arc::new(child);
        cms_pages.push(child.clone());
        page_parent_map.insert(child.clone(), current_page.clone());

        // Recurse!
        download_cms_pages(site_client, child, page_parent_map, cms_pages)?;
    }
    Ok(())
}
