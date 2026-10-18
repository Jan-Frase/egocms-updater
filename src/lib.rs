mod initialization;
mod md_to_html_conversion;
mod new_page_creation;
pub mod types;
mod wipe_cms;

use crate::md_to_html_conversion::convert_md_to_html;
use crate::types::config::Inputs;

pub fn run(args: Inputs) -> anyhow::Result<()> {
    print_stage("1. Initialize resources");
    let (pending_pages, mut communicator, config) = initialization::init(args)?;

    print_stage("2. Wipe CMS");
    wipe_cms::wipe_cms()?;

    print_stage("3. Create new pages");
    let (mut pushed_pages, md_name_to_id_map) =
        new_page_creation::push_new_pages(pending_pages, &config, &mut communicator)?;

    print_stage("4. Convert markdown to html");
    convert_md_to_html(&mut pushed_pages, &md_name_to_id_map, &communicator)?;

    print_stage("5. Update pages");
    // For each tracked page...
    for page in &pushed_pages {
        // ... update it, if required.
        page.update(&mut communicator, &config.json_content_path)?;
    }
    Ok(())
}

fn print_stage(title: &str) {
    println!("\n{title}.");
}
