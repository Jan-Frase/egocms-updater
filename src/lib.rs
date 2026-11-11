use std::collections::HashMap;
use crate::cms::cms_trait::CmsApi;
mod local;
mod cms;
mod download;
pub mod config;

use anyhow::{Context, Result};
use local::local_state::LocalState;
use std::path::Path;
use std::sync::Arc;
use crate::cms::cms_client::SiteClient;
use crate::cms::cms_connection::CmsConnection;
use crate::cms::cms_page::CmsPage;
use crate::config::config::Config;
use crate::config::credentials::Credentials;
use crate::local::local_page::LocalPage;

pub fn download(config: &Config, credentials: &Credentials, out_dir: &Path) -> Result<()> {
    download::download(config, credentials, out_dir)?;
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
