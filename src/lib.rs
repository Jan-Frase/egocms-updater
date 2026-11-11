use crate::cms::cms_trait::CmsApi;
mod cms;
mod download;
mod local;
pub mod settings;

use crate::cms::cms_connection::CmsConnection;
use crate::settings::config::Config;
use crate::settings::credentials::Credentials;
use anyhow::Result;
use download::page_download::download_pages;
use local::local_state::LocalState;
use std::path::Path;

pub fn download(config: &Config, credentials: &Credentials, out_dir: &Path) -> Result<()> {
    download_pages(config, credentials, out_dir)?;
    Ok(())
}

pub fn upload(config: &Config, credentials: &Credentials) -> Result<()> {
    // 1. Gather the local state.
    let local_state = LocalState::new(&config.pages_dir, &config.home_page)?;

    // 2. Open connection the cms.
    let cms_connection = CmsConnection::open(config, credentials)?;
    let mut site_client = cms_connection.site(&config.site, &config.language);
    let media_client = site_client.media_site()?;

    // 3. Get the home_id.
    let home_id = match config.home_page_id {
        Some(id) => id,
        None => site_client.root_id()?,
    };

    Ok(())
}
