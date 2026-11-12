//! Uploads the local pages and the files they link to.

mod dry_run;
mod links;
pub mod sync;

use crate::cms::cms_connection::CmsConnection;
use crate::cms::cms_trait::CmsApi;
use crate::local::local_state::LocalState;
use crate::settings::config::Config;
use crate::settings::credentials::Credentials;
use anyhow::Result;
use dry_run::DryRun;
use sync::{Destination, sync};

/// Uploads the pages. With `dry_run`, it only shows what would change, without writing anything.
pub fn upload(config: &Config, credentials: &Credentials, dry_run: bool) -> Result<()> {
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

    let page_types: Vec<&str> = config.page_types.iter().map(String::as_str).collect();
    let destination = Destination {
        site: &config.site,
        language: &config.language,
        home_id,
        home_page: &config.home_page,
        page_type: &config.page_type,
        page_types: &page_types,
    };
    let report = if dry_run {
        println!("Dry run: nothing is written to the CMS.");
        sync(&mut DryRun { cms: &mut site }, &local, &destination)?
    } else {
        sync(&mut site, &local, &destination)?
    };
    println!("\n{report}");
    Ok(())
}
