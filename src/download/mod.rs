use crate::cms::cms_connection::CmsConnection;
use crate::cms::cms_trait::CmsApi;
use crate::download::file_download::download_files;
use crate::download::page_download::download_pages;
use crate::settings::config::Config;
use crate::settings::credentials::Credentials;
use anyhow::{Result, bail};
use std::path::Path;

pub mod file_download;
pub mod page_download;

pub fn download(config: &Config, credentials: &Credentials, out_dir: &Path) -> Result<()> {
    // 1. Refuse to overwrite anything before touching the CMS.
    if out_dir.exists() && out_dir.read_dir()?.next().is_some() {
        bail!("{} is not empty.", out_dir.display());
    }

    // 2. Open a connection to the CMS.
    let cms_connection = CmsConnection::open(config, credentials)?;
    let mut site_client = cms_connection.site(&config.site, &config.language);

    // 3. Get the home_id.
    let home_id = match config.home_page_id {
        Some(id) => id,
        None => site_client.root_id()?,
    };

    // 4. Download the pages.
    download_pages(config, &mut site_client, home_id, out_dir)?;

    // 5. Download the files.
    download_files();

    Ok(())
}
