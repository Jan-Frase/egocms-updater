// TODO:
// - Ensure that when updating an existing site, the title and names are updated as well.
// - Media Files via Interface in EgoCMS. (keep low effort, not via media area, only direct attached to page)
// - Download EgoCMS pages into dirs and .md files.
// - Figure out how (if at all) to handle md file deletions?
// - Create an action to run the program on pushes.
// - Get rid of any unneeded unwrap.
// - Document usage!

// Maybes:
// - Receive user_id and token via environment vars instead of cli inputs.

// Open Questions:
// - How should I deal with pages not representable by Markdown? I.e.: people pages.

// Notes:
// - Publications can be ignored.

// Done:
// Automatically create new pages.
// -> the name of the new page is derived from the file name, the title is the first line in the md
// - Links via page_id/paths.

use anyhow::Context;
use auto_update_egocms::run;
use auto_update_egocms::types::config::parse_arguments;

fn main() -> anyhow::Result<()> {
    // Parse the command line arguments.
    let inputs = parse_arguments().context("Failed to parse the arguments!")?;
    // Run the application :)
    run(inputs)?;

    Ok(())
}
