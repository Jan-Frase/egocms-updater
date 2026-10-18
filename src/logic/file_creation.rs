use crate::types::communicator::Communicator;
use crate::types::config::{Config, FILES_DIR};
use anyhow::Context;
use std::fs;

/// Uploads every file in `FILES_DIR` to the media folder of the CMS.
/// Files which already exist (same name in the same folder) get replaced, others get created.
pub fn create_files(config: &Config, communicator: &Communicator) -> anyhow::Result<()> {
    // TODO: Allow recursive folders using WalkDir.
    let files = fs::read_dir(FILES_DIR).with_context(|| format!("Cannot read {FILES_DIR}"))?;
    for entry in files {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .with_context(|| format!("Invalid file name: {}", path.display()))?;
        let bytes = fs::read(&path)?;

        let id = communicator.new_file(&config.media_site_url, config.media_folder_id, bytes, name)?;
        println!("=> Created file: {name} (id {id})");
    }
    Ok(())
}
