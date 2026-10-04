// TODO:
// - Media Files via Interface in EgoCMS
// - Links via page_id
// - Download EgoCMS pages into dirs and .md files
// - get rid of any unneeded unwrap
// - Ensure that when updating an existing site, the title and names are updated as well.
// - Document usage!

// Maybes:
// - Receive user_id and token via environment vars instead of cli inputs.

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
