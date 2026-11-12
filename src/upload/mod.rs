//! Uploads the local pages and the files they link to.

mod links;
pub mod sync;

use crate::cms::cms_connection::CmsConnection;
use crate::cms::cms_trait::CmsApi;
use crate::local::local_state::LocalState;
use crate::settings::config::Config;
use crate::settings::credentials::Credentials;
use anyhow::Result;
use sync::{Destination, sync};

pub fn upload(config: &Config, credentials: &Credentials) -> Result<()> {
    // Read and check everything locally before touching the CMS.
    let local = LocalState::new(&config.pages_dir, &config.home_page)?;
    let files: usize = local.pages.iter().map(|page| page.files.len()).sum();
    println!(
        "Found {} pages and {files} files in {}.",
        local.pages.len(),
        config.pages_dir.display()
    );

    let connection = CmsConnection::open(config, credentials)?;
    let mut site = connection.site(&config.site, &config.language);
    let home_id = match config.home_page_id {
        Some(id) => id,
        None => site.root_id()?,
    };

    let destination = Destination {
        site: &config.site,
        language: &config.language,
        home_id,
        home_page: &config.home_page,
        page_type: &config.page_type,
    };
    let report = sync(&mut site, &local, &destination)?;
    println!("\n{report}");
    Ok(())
}
