use crate::types::communicator::Communicator;
use anyhow::bail;

/// Deletes every page of the site and empties the trash, so we start from a clean slate.
/// Only the root page survives, as EgoCMS doesn't allow deleting it. Its content is overwritten later.
pub fn wipe_cms(communicator: &Communicator) -> anyhow::Result<()> {
    let root_id = communicator.get_root_id()?;

    let mut deleted = 0;
    for id in communicator.get_all_page_ids()? {
        // Deleting a page also deletes its subpages, so pages listed later might already be gone.
        if id != root_id && communicator.delete_page(id)? {
            deleted += 1;
        }
    }
    communicator.clear_trashcan()?;

    // Make sure the "page is already gone" assumption from above actually held.
    let remaining: Vec<u64> = communicator
        .get_all_page_ids()?
        .into_iter()
        .filter(|id| *id != root_id)
        .collect();
    if !remaining.is_empty() {
        bail!("Failed to delete the pages: {remaining:?}");
    }

    println!("=> Deleted {deleted} pages (and their subpages).");
    Ok(())
}
