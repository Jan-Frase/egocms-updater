// TODO:
// In the works:
// - Links via page_id
// - get rid of any unneeded unwrap

// Planned:
// - Media Files via Interface in EgoCMS
// - Download EgoCMS pages into dirs and .md files
// - Ensure that when updating an existing site, the title and names are updated as well.

// Done:
// Automatically create new pages.
// -> the name of the new page is derived from the file name, the title is the first line in the md

use anyhow::Context;
use auto_update_egocms::config::parse_arguments;
use auto_update_egocms::run;

fn main() -> anyhow::Result<()> {
    // Parse the command line arguments.
    let inputs = parse_arguments().context("Failed to parse the arguments!")?;
    // Run the application :)
    run(inputs)?;

    Ok(())
}
