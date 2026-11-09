pub mod config;
pub mod credentials;
mod local;

use crate::config::Config;
use crate::credentials::Credentials;
use anyhow::Result;
use local::local_state::LocalState;
use std::path::Path;

pub fn download(config: &Config, credentials: &Credentials, out_dir: &Path) -> Result<()> {
    todo!()
}

pub fn upload(config: &Config, credentials: &Credentials) -> Result<()> {
    // 1. Gather the local state.
    let local_state = LocalState::new(&config.pages_dir, &config.home_page)?;

    Ok(())
}
