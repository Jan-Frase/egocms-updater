//! Resolves the relative links in the markdown files and converts them to HTML.
//!
//! A link to another `.md` file becomes a link to that page. A link to any other local file
//! points into the pool of the page owning the file, i.e. the page of the file's directory.

use crate::cms::cms_trait::percent_decode;
use crate::local::local_file::LocalFile;
use crate::local::local_page::LocalPage;
use crate::local::local_state::LocalState;
use anyhow::{Context, Result, bail};
use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, html};
use std::collections::HashMap;

/// What a relative link points to.
pub enum Target<'a> {
    Page(&'a LocalPage),
    /// A file, attached to the page `owner`.
    File {
        owner: &'a LocalPage,
        file: &'a LocalFile,
    },
}

/// Looks up the targets of relative links among the local pages and their files.
pub struct Links<'a> {
    pages: HashMap<&'a str, &'a LocalPage>,
    files: HashMap<&'a str, (&'a LocalPage, &'a LocalFile)>,
}

impl<'a> Links<'a> {
    pub fn new(local: &'a LocalState) -> Self {
        Self {
            pages: local
                .pages
                .iter()
                .map(|page| (page.relative_path.as_str(), page))
                .collect(),
            files: local
                .pages
                .iter()
                .flat_map(|page| {
                    page.files
                        .iter()
                        .map(move |file| (file.relative_path.as_str(), (page, file)))
                })
                .collect(),
        }
    }

    /// The target of a path relative to the pages directory, e.g. `Research/SMASH/SMASH.md`.
    pub fn target(&self, path: &str) -> Result<Target<'a>> {
        if let Some(page) = self.pages.get(path) {
            return Ok(Target::Page(page));
        }
        self.files
            .get(path)
            .map(|(owner, file)| Target::File { owner, file })
            .with_context(|| format!("{path} does not exist."))
    }

    /// Checks that every relative link of `page` points to an existing page or file.
    pub fn check(&self, page: &LocalPage) -> Result<()> {
        rewrite_links(&page.markdown, &page.relative_path, &mut |path| {
            self.target(path).map(|_| String::new())
        })?;
        Ok(())
    }
}

/// The name of a file in the pool of its page: the file name, without directories.
pub fn pool_name(file: &LocalFile) -> &str {
    file.relative_path
        .rsplit('/')
        .next()
        .unwrap_or(&file.relative_path)
}

/// Converts `markdown` (the content of the file `current`) to HTML.
///
/// Every relative link or image is resolved against `current` and handed to `resolve`,
/// which returns the URL to use instead. Anchors like `./SMASH.md#goals` are kept.
pub fn rewrite_links(
    markdown: &str,
    current: &str,
    resolve: &mut dyn FnMut(&str) -> Result<String>,
) -> Result<String> {
    let events = Parser::new_ext(
        markdown,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
    )
    .map(|event| {
        Ok(match event {
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => Event::Start(Tag::Link {
                link_type,
                dest_url: rewrite(dest_url, current, resolve)?,
                title,
                id,
            }),
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }) => Event::Start(Tag::Image {
                link_type,
                dest_url: rewrite(dest_url, current, resolve)?,
                title,
                id,
            }),
            other => other,
        })
    })
    .collect::<Result<Vec<_>>>()?;

    let mut html = String::new();
    html::push_html(&mut html, events.into_iter());
    Ok(html)
}

/// Replaces a relative link by the URL `resolve` returns for its target.
fn rewrite<'u>(
    dest_url: CowStr<'u>,
    current: &str,
    resolve: &mut dyn FnMut(&str) -> Result<String>,
) -> Result<CowStr<'u>> {
    if !is_relative(&dest_url) {
        return Ok(dest_url);
    }
    let (path, anchor) = match dest_url.split_once('#') {
        Some((path, anchor)) => (path, Some(anchor)),
        None => (dest_url.as_ref(), None),
    };
    // Paths may be written with escapes, e.g. `Problem%20Statement/Problem%20Statement.md`.
    let target = resolve_relative(current, &percent_decode(path))
        .and_then(|target| resolve(&target))
        .with_context(|| format!("{current}: broken link `{dest_url}`"))?;
    Ok(match anchor {
        Some(anchor) => format!("{target}#{anchor}"),
        None => target,
    }
    .into())
}

/// Links without a scheme (`https:`, `mailto:`, ...) that are neither absolute nor pure anchors
/// point to files in the pages directory.
fn is_relative(url: &str) -> bool {
    !url.is_empty() && !url.contains(':') && !url.starts_with('/') && !url.starts_with('#')
}

/// Resolves `link` relative to the directory of the file `current`.
/// ("Research/Research.md", "./SMASH/SMASH.md") => "Research/SMASH/SMASH.md"
/// ("Research/SMASH/SMASH.md", "../../Home.md") => "Home.md"
fn resolve_relative(current: &str, link: &str) -> Result<String> {
    let mut parts: Vec<&str> = current.split('/').collect();
    parts.pop();
    for part in link.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    bail!("`..` leaves the pages directory.");
                }
            }
            other => parts.push(other),
        }
    }
    if parts.is_empty() {
        bail!("The link does not point to a file.");
    }
    Ok(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(path: &str, markdown: &str, files: &[&str]) -> LocalPage {
        let mut page = LocalPage::parse(path, &format!("# Title\n{markdown}"), "Home.md").unwrap();
        page.files = files
            .iter()
            .map(|file| LocalFile {
                relative_path: (*file).into(),
                bytes: file.as_bytes().to_vec(),
            })
            .collect();
        page
    }

    /// Resolves every target to `URL(target)`, so tests can see what was resolved.
    fn render(markdown: &str, current: &str) -> Result<String> {
        rewrite_links(markdown, current, &mut |target| {
            Ok(format!("URL({target})"))
        })
    }

    #[test]
    fn resolves_paths_relative_to_the_current_file() {
        let r = resolve_relative;
        assert_eq!(
            r("Research/Research.md", "./SMASH/SMASH.md").unwrap(),
            "Research/SMASH/SMASH.md"
        );
        assert_eq!(
            r("Research/SMASH/SMASH.md", "../../Home.md").unwrap(),
            "Home.md"
        );
        assert_eq!(r("a/b/c.md", "./../x/./y.md").unwrap(), "a/x/y.md");
        assert!(r("Home.md", "../x.md").is_err());
        assert!(r("a/b.md", "..").is_err());
    }

    #[test]
    fn rewrites_relative_links_and_images() {
        let html = render(
            "[SMASH](./SMASH/SMASH.md) and ![Plot](plot.png \"A plot\")",
            "Research/Research.md",
        )
        .unwrap();
        assert_eq!(
            html,
            "<p><a href=\"URL(Research/SMASH/SMASH.md)\">SMASH</a> and \
             <img src=\"URL(Research/plot.png)\" alt=\"Plot\" title=\"A plot\" /></p>\n"
        );
    }

    #[test]
    fn accepts_paths_with_spaces_written_both_ways() {
        // A plain space ends a link in markdown, so it has to be escaped or put in `<…>`.
        for markdown in [
            "[PS](./Problem%20Statement/Problem%20Statement.md)",
            "[PS](<./Problem Statement/Problem Statement.md>)",
        ] {
            let mut targets = Vec::new();
            rewrite_links(markdown, "Research/CoSEMoS/CoSEMoS.md", &mut |target| {
                targets.push(target.to_string());
                Ok(String::new())
            })
            .unwrap();
            assert_eq!(
                targets,
                ["Research/CoSEMoS/Problem Statement/Problem Statement.md"]
            );
        }
    }

    #[test]
    fn keeps_anchors() {
        let html = render("[Goals](../Home.md#goals)", "Research/Research.md").unwrap();
        assert_eq!(html, "<p><a href=\"URL(Home.md)#goals\">Goals</a></p>\n");
    }

    #[test]
    fn leaves_other_links_alone() {
        let markdown = "[a](https://parcio.de) [b](mailto:x@y.z) [c](/absolute) [d](#top)";
        let html = rewrite_links(markdown, "Home.md", &mut |target| {
            bail!("must not resolve {target}")
        })
        .unwrap();
        for url in ["https://parcio.de", "mailto:x@y.z", "/absolute", "#top"] {
            assert!(html.contains(&format!("href=\"{url}\"")), "{html}");
        }
    }

    #[test]
    fn escapes_ampersands_in_urls() {
        let html = rewrite_links("[a](b.md)", "Home.md", &mut |_| {
            Ok("index.php?a=1&b=2".into())
        })
        .unwrap();
        assert_eq!(html, "<p><a href=\"index.php?a=1&amp;b=2\">a</a></p>\n");
    }

    #[test]
    fn finds_pages_and_the_owners_of_files() {
        let local = LocalState {
            pages: vec![
                page("Home.md", "", &["logo.png"]),
                page("Teaching/Teaching.md", "", &["Teaching/slides.pdf"]),
            ],
        };
        let links = Links::new(&local);
        assert!(matches!(
            links.target("Teaching/Teaching.md"),
            Ok(Target::Page(_))
        ));
        let Ok(Target::File { owner, file }) = links.target("Teaching/slides.pdf") else {
            panic!("slides.pdf is a file");
        };
        assert_eq!(owner.relative_path, "Teaching/Teaching.md");
        assert_eq!(pool_name(file), "slides.pdf");
        assert!(links.target("Teaching/missing.pdf").is_err());
    }

    #[test]
    fn checks_links() {
        let local = LocalState {
            pages: vec![
                page(
                    "Home.md",
                    "[t](Teaching/Teaching.md) [s](Teaching/slides.pdf)",
                    &[],
                ),
                page(
                    "Teaching/Teaching.md",
                    "[gone](gone.pdf)",
                    &["Teaching/slides.pdf"],
                ),
            ],
        };
        let links = Links::new(&local);
        assert!(links.check(&local.pages[0]).is_ok());

        let message = format!("{:#}", links.check(&local.pages[1]).unwrap_err());
        assert!(
            message.contains("Teaching/Teaching.md: broken link `gone.pdf`"),
            "{message}"
        );
        assert!(
            message.contains("Teaching/gone.pdf does not exist"),
            "{message}"
        );
    }

    #[test]
    fn renders_tables_and_strikethrough() {
        let html = render("| a |\n|---|\n| b |\n\n~~old~~", "Home.md").unwrap();
        assert!(html.contains("<table>"));
        assert!(html.contains("<del>old</del>"));
    }
}
