use crate::types::communicator::Communicator;
use crate::types::page::MappedPage;
use anyhow::Context;
use pulldown_cmark::html::push_html;
use pulldown_cmark::{CowStr, Event, Parser, Tag};
use std::collections::HashMap;

/// Converts the text inside every MappedPage into HTML.
/// Whilst doing so, it also takes care of converting any relative paths in links to the correct url.
pub fn convert_md_to_html(
    pushed_pages: &mut Vec<MappedPage>,
    md_name_to_id_map: &HashMap<String, u64>,
    communicator: &Communicator,
) -> anyhow::Result<()> {
    for page in pushed_pages {
        let html = convert_single_page(page, md_name_to_id_map, communicator)?;
        page.html = Some(html);
    }
    Ok(())
}

fn convert_single_page(
    page: &MappedPage,
    md_name_to_id_map: &HashMap<String, u64>,
    communicator: &Communicator,
) -> anyhow::Result<String> {
    let events = Parser::new(&page.content.text)
        .map(|event| -> anyhow::Result<Event> {
            match event {
                Event::Start(Tag::Link {
                    link_type,
                    mut dest_url,
                    title,
                    id,
                }) if is_local_md_link(&dest_url) => {
                    convert_local_md_link(&mut dest_url, page, md_name_to_id_map, communicator)?;
                    Ok(Event::Start(Tag::Link {
                        link_type,
                        dest_url,
                        title,
                        id,
                    }))
                }
                other => Ok(other),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut html = String::new();
    push_html(&mut html, events.into_iter());

    Ok(html)
}

fn is_local_md_link(dest_url: &CowStr) -> bool {
    dest_url.ends_with(".md") && (dest_url.starts_with("./") || dest_url.starts_with("../"))
}

fn convert_local_md_link(
    dest_url: &mut CowStr,
    page: &MappedPage,
    md_name_to_id_map: &HashMap<String, u64>,
    communicator: &Communicator,
) -> anyhow::Result<()> {
    // 1. Get the id of the page based on the path
    // dest_url is relative: ./CoSEMoS/CoSEMoS.md, it needs to be made absolute with pages as a base
    let path =
        resolve_link_relative_to_current_file(&page.content.markdown_name, dest_url.as_ref())?;
    let id = md_name_to_id_map.get(&path.clone()).with_context(|| {
        format!(
            "{}: the link `{dest_url}` points to `{path}`, which is not a known page.",
            page.content.markdown_name
        )
    })?; // 2. Get the URL of the page based on the id
    // This returns something like: "/seitentypen/blog/eintrag-1"
    let url = communicator.get_url(*id)?;
    // To finish it, append the prefix.
    let new_url = format!(
        "{}/{}",
        // TODO: Improve the entire URL handling across the project? Its somewhat brittle rn.
        communicator.rest_url.strip_suffix("/rest/").unwrap(),
        url.trim_start_matches('/')
    );

    // 3. Profit!
    println!(
        "On page: {:30}, converted {} to {:?}",
        page.content.markdown_name, dest_url, new_url
    );
    *dest_url = CowStr::from(new_url);
    Ok(())
}

/// Resolves `link` (relative to the file `current`) to a name from the mapping table.
/// ("Research/Research.md", "./CoSEMoS/CoSEMoS.md") => "Research/CoSEMoS/CoSEMoS.md"
/// ("Research/JULEA.md",    "../Home.md")           => "Home.md"
fn resolve_link_relative_to_current_file(current: &str, link: &str) -> anyhow::Result<String> {
    let mut current: Vec<_> = current.split('/').collect();
    let _ = current.pop();

    for link_seq in link.split('/') {
        match link_seq {
            "" | "." => {}
            ".." => {
                current
                    .pop()
                    .ok_or_else(|| anyhow::anyhow!("Invalid path: '..' goes beyond the root"))?;
            }
            other => {
                current.push(other);
            }
        }
    }

    Ok(current.join("/"))
}

#[cfg(test)]
mod tests {
    use crate::md_to_html_conversion::resolve_link_relative_to_current_file;

    #[test]
    fn resolves_links_relative_to_the_current_file() {
        let r = resolve_link_relative_to_current_file;
        assert_eq!(
            r("Research/Research.md", "./CoSEMoS/CoSEMoS.md").unwrap(),
            "Research/CoSEMoS/CoSEMoS.md"
        );
        assert_eq!(r("Research/JULEA.md", "../Home.md").unwrap(), "Home.md");
        assert_eq!(
            r("Home.md", "Research/Research.md").unwrap(),
            "Research/Research.md"
        );
        assert!(r("Home.md", "../x.md").is_err());
    }
}
