use crate::config::Inputs;

mod communicator;
pub mod config;
mod initialization;
mod new_page_creation;
mod page;

pub fn run(args: Inputs) -> anyhow::Result<()> {
    print_stage("1. Initialize resources");
    let (pending_pages, pushed_pages, communicator, config) = initialization::init(args)?;

    print_stage("2. Create new pages");
    let pushed_pages =
        new_page_creation::push_new_pages(pending_pages, pushed_pages, &config, &communicator)?;

    print_stage("3. Update pages");
    // For each tracked page...
    for page in &pushed_pages {
        // ... update it, if required.
        page.update(&communicator, &config.json_content_path)?;
    }
    Ok(())
}

fn print_stage(title: &str) {
    println!("{title}.");
}
