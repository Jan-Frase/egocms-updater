mod cms;
mod download;
mod local;
pub mod settings;
mod upload;

use crate::settings::config::Config;
use crate::settings::credentials::Credentials;
use anyhow::Result;
use std::path::Path;

pub fn download(config: &Config, credentials: &Credentials, out_dir: &Path) -> Result<()> {
    download::download(config, credentials, out_dir)?;
    Ok(())
}

pub fn upload(config: &Config, credentials: &Credentials, dry_run: bool) -> Result<()> {
    upload::upload(config, credentials, dry_run)
}
