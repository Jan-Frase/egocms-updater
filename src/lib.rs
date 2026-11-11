mod cms;
mod download;
mod local;
pub mod settings;
mod upload;

use crate::settings::config::Config;
use crate::settings::credentials::Credentials;
use anyhow::Result;
use download::page_download::download_pages;
use std::path::Path;

pub fn download(config: &Config, credentials: &Credentials, out_dir: &Path) -> Result<()> {
    download_pages(config, credentials, out_dir)?;
    Ok(())
}

pub fn upload(config: &Config, credentials: &Credentials) -> Result<()> {
    upload::upload(config, credentials)
}
