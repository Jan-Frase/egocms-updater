use std::fmt::Arguments;
use crate::config::Inputs;

pub mod config;
mod communicator;
mod initialization;
mod new_page_creation;
mod page;

pub fn run(args: Inputs) -> anyhow::Result<()> {
    print_stage("1. Initialize resources");
    let (mut pages, communicator, config) = initialization::init(args)?;

    print_stage("2. Create new pages");
    new_page_creation::push_new_pages(&mut pages, &config, &communicator)?;

    print_stage("3. Update pages");
    // For each tracked page...
    for page in &pages {
        // ... update it, if required.
        page.update(&communicator, &config.json_content_path)?;
    }
    Ok(())
}

fn print_stage(title: &str) {
    println!("{}.", title);
}