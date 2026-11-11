//! Resolves the relative links in the markdown files and converts them to HTML.
//!
//! A link to another `.md` file becomes a link to that page. A link to any other local file
//! attaches the file to the linking page (its "pool") and points there.

use crate::local::local_file::LocalFile;
use crate::local::local_page::LocalPage;
use crate::local::local_state::LocalState;
use anyhow::{Context, Result, bail};
use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, html};
use std::collections::{BTreeMap, HashMap};

/// What a relative link points to.
pub enum Target<'a> {
    Page(&'a LocalPage),
    File(&'a LocalFile),
}

/// Looks up the targets of relative links among the local pages and files.
pub struct Links<'a> {
    pages: HashMap<&'a str, &'a LocalPage>,
    files: HashMap<&'a str, &'a LocalFile>,
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
                .files
                .iter()
                .map(|file| (file.relative_path.as_str(), file))
                .collect(),
        }
    }

    /// The target of a path relative to the pages directory, e.g. `Research/SMASH.md`.
    pub fn target(&self, path: &str) -> Result<Target<'a>> {
        if let Some(page) = self.pages.get(path) {
            return Ok(Target::Page(page));
        }
        self.files
            .get(path)
            .map(|file| Target::File(file))
            .with_context(|| format!("{path} does not exist."))
    }

    /// The files `page` links to, by their name in its pool.
    /// Fails for broken links and for two different files that would get the same name.
    pub fn pool_files(&self, page: &LocalPage) -> Result<BTreeMap<&'a str, &'a LocalFile>> {
        let mut files: BTreeMap<&str, &LocalFile> = BTreeMap::new();
        rewrite_links(&page.markdown, &page.relative_path, &mut |path| {
            if let Target::File(file) = self.target(path)? {
                let name = pool_name(file);
                match files.insert(name, file) {
                    Some(other) if other.relative_path != file.relative_path => bail!(
                        "{} and {} would both be attached as {name}.",
                        other.relative_path,
                        file.relative_path
                    ),
                    _ => {}
                }
            }
            Ok(String::new())
        })
        .with_context(|| format!("Checking the links of {} failed.", page.relative_path))?;
        Ok(files)
    }
}

/// The name of a file in the pool of a page: its file name, without directories.
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
    let target = resolve_relative(current, path)
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
/// ("Research/Research.md", "./CoSEMoS/CoSEMoS.md") => "Research/CoSEMoS/CoSEMoS.md"
/// ("Research/JULEA.md",    "../Home.md")           => "Home.md"
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

    fn page(path: &str, markdown: &str) -> LocalPage {
        LocalPage::parse(path, &format!("# Title\n{markdown}"), "Home.md").unwrap()
    }

    fn file(path: &str) -> LocalFile {
        LocalFile {
            relative_path: path.into(),
            bytes: path.as_bytes().to_vec(),
        }
    }

    fn state(pages: Vec<LocalPage>, files: Vec<LocalFile>) -> LocalState {
        LocalState { pages, files }
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
            r("Research/Research.md", "./CoSEMoS/CoSEMoS.md").unwrap(),
            "Research/CoSEMoS/CoSEMoS.md"
        );
        assert_eq!(r("Research/JULEA.md", "../Home.md").unwrap(), "Home.md");
        assert_eq!(r("a/b/c.md", "./../x/./y.md").unwrap(), "a/x/y.md");
        assert!(r("Home.md", "../x.md").is_err());
        assert!(r("a/b.md", "..").is_err());
    }

    #[test]
    fn rewrites_relative_links_and_images() {
        let html = render(
            "[SMASH](./SMASH.md) and ![Plot](plot.png \"A plot\")",
            "Research/Research.md",
        )
        .unwrap();
        assert_eq!(
            html,
            "<p><a href=\"URL(Research/SMASH.md)\">SMASH</a> and \
             <img src=\"URL(Research/plot.png)\" alt=\"Plot\" title=\"A plot\" /></p>\n"
        );
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
    fn reports_broken_links_with_the_file() {
        let local = state(vec![page("Home.md", "[x](./missing.md)")], vec![]);
        let links = Links::new(&local);
        let error = links.pool_files(&local.pages[0]).unwrap_err();
        let message = format!("{error:#}");
        assert!(
            message.contains("Home.md: broken link `./missing.md`"),
            "{message}"
        );
        assert!(message.contains("missing.md does not exist"), "{message}");
    }

    #[test]
    fn distinguishes_pages_and_files() {
        let local = state(vec![page("Home.md", "")], vec![file("a.pdf")]);
        let links = Links::new(&local);
        assert!(matches!(links.target("Home.md"), Ok(Target::Page(_))));
        assert!(matches!(links.target("a.pdf"), Ok(Target::File(_))));
        assert!(links.target("b.pdf").is_err());
    }

    #[test]
    fn collects_the_linked_files_by_pool_name() {
        let local = state(
            vec![
                page(
                    "Home.md",
                    "[slides](Teaching/slides.pdf) ![x](img/plot.png) [again](Teaching/slides.pdf) [page](Teaching/Teaching.md)",
                ),
                page("Teaching/Teaching.md", ""),
            ],
            vec![
                file("Teaching/slides.pdf"),
                file("img/plot.png"),
                file("unused.zip"),
            ],
        );
        let links = Links::new(&local);
        let files = links.pool_files(&local.pages[0]).unwrap();
        let names: Vec<_> = files.keys().copied().collect();
        assert_eq!(names, ["plot.png", "slides.pdf"]);
        assert_eq!(files["slides.pdf"].relative_path, "Teaching/slides.pdf");
        assert!(links.pool_files(&local.pages[1]).unwrap().is_empty());
    }

    #[test]
    fn rejects_two_files_with_the_same_pool_name() {
        let local = state(
            vec![page("Home.md", "![a](a/plot.png) ![b](b/plot.png)")],
            vec![file("a/plot.png"), file("b/plot.png")],
        );
        let error = Links::new(&local).pool_files(&local.pages[0]).unwrap_err();
        assert!(
            format!("{error:#}").contains("would both be attached as plot.png"),
            "{error:#}"
        );
    }

    #[test]
    fn renders_tables_and_strikethrough() {
        let html = render("| a |\n|---|\n| b |\n\n~~old~~", "Home.md").unwrap();
        assert!(html.contains("<table>"));
        assert!(html.contains("<del>old</del>"));
    }
}
