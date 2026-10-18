pub mod types;
pub mod logic;

use crate::types::config::Inputs;
use std::fs;
use crate::logic::*;

pub fn run(args: Inputs) -> anyhow::Result<()> {
    print_stage("1. Initialize resources");
    let (pending_pages, pushed_pages, mut communicator, config) = initialization::init(args)?;

    // let _ = communicator.get_media_url()?;


    print_stage("2. Create new pages");
    let (mut pushed_pages, md_name_to_id_map) =
        page_creation::push_new_pages(pending_pages, pushed_pages, &config, &mut communicator)?;

    print_stage("3. Update files");
    let bytes = fs::read("./files/PPE-2026-05-OpenMP.pdf")?;
    communicator.new_file(56, bytes, "PDF".into())?;

    return Ok(());

    print_stage("4. Convert markdown to html");
    md_to_html_conversion::convert_md_to_html(&mut pushed_pages, &md_name_to_id_map, &communicator)?;

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
