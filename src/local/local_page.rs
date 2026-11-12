//! A markdown file that becomes a CMS page.
//!
//! The directory structure is the page tree, and every page has its own directory:
//! ```text
//! Home.md                           the home page (`home_page` in the config), owns the top level
//! Research/Research.md              a child of the home page
//! Research/SMASH/SMASH.md           a child of Research
//! Research/SMASH/slides.pdf         attached to SMASH
//! ```
//! So a page's file is always named after its directory, and every other file belongs to the page
//! of the directory it lies in.

use crate::local::local_file::LocalFile;
use anyhow::{Context, bail};
use std::fs;
use std::path::Path;

/// A Markdown file that should exist as a CMS page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalPage {
    /// Path relative to the pages directory, always with `/` as a separator. E.g. `Research/SMASH/SMASH.md`.
    pub relative_path: String,
    /// The CMS name of the page (and thus part of its URL): the file name without `.md`.
    pub name: String,
    /// The first line of the file, without the leading `#`.
    pub title: String,
    /// Everything after the first line.
    pub markdown: String,
    /// The relative_path of the parent page. `None` only for the home page.
    pub parent: Option<String>,
    /// The other files in the page's directory, which are attached to the page.
    pub files: Vec<LocalFile>,
}

impl LocalPage {
    /// Reads and parses the markdown file at `full_path`.
    pub fn new(relative_path: &str, full_path: &Path, home_page: &str) -> anyhow::Result<Self> {
        let text = fs::read_to_string(full_path)
            .with_context(|| format!("Cannot read {relative_path}"))?;
        Self::parse(relative_path, &text, home_page)
    }

    /// Turns the content of a markdown file into a page (without files).
    pub(crate) fn parse(relative_path: &str, text: &str, home_page: &str) -> anyhow::Result<Self> {
        check_page_path(relative_path, home_page)?;

        let (first_line, markdown) = text.split_once('\n').unwrap_or((text, ""));
        let title = first_line.trim_start_matches('#').trim();
        if title.is_empty() {
            bail!("{relative_path}: the first line has to be the title, e.g. `# My Page`.");
        }

        let file_name = relative_path.rsplit('/').next().unwrap_or(relative_path);
        let name = file_name.strip_suffix(".md").unwrap_or(file_name);

        Ok(Self {
            relative_path: relative_path.to_string(),
            name: name.to_string(),
            title: title.to_string(),
            markdown: markdown.trim_start_matches(['\r', '\n']).to_string(),
            parent: parent_of(relative_path, home_page),
            files: Vec::new(),
        })
    }

    /// The directory the page owns. `Research/SMASH/SMASH.md` => `Research/SMASH`, `Home.md` => ``.
    pub fn dir(&self) -> &str {
        dir_of(&self.relative_path)
    }
}

/// The directory a file lies in, relative to the pages directory.
/// `Research/SMASH/slides.pdf` => `Research/SMASH`, `Home.md` => ``.
pub fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// The path of the page owning `dir`. `Research/SMASH` => `Research/SMASH/SMASH.md`, `` => `Home.md`.
pub fn page_of_dir(dir: &str, home_page: &str) -> String {
    match dir.rsplit('/').next() {
        Some(name) if !dir.is_empty() => format!("{dir}/{name}.md"),
        _ => home_page.to_string(),
    }
}

/// The path of the parent page, or `None` for the home page.
/// `Research/SMASH/SMASH.md` => `Research/Research.md`, `Research/Research.md` => `Home.md`.
pub fn parent_of(path: &str, home_page: &str) -> Option<String> {
    if path == home_page {
        return None;
    }
    Some(page_of_dir(dir_of(dir_of(path)), home_page))
}

/// Checks that `path` follows the layout: the home page at the top, all others named after their directory.
fn check_page_path(path: &str, home_page: &str) -> anyhow::Result<()> {
    if path == home_page {
        return Ok(());
    }
    let dir = dir_of(path);
    if dir.is_empty() {
        let name = path.strip_suffix(".md").unwrap_or(path);
        bail!("{path}: pages need their own directory, move it to {name}/{name}.md.");
    }
    let expected = page_of_dir(dir, home_page);
    if path != expected {
        bail!("{path}: a page has to be named after its directory, like {expected}.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "Home.md";

    #[test]
    fn maps_directories_to_pages() {
        assert_eq!(dir_of("Research/SMASH/slides.pdf"), "Research/SMASH");
        assert_eq!(dir_of("Home.md"), "");
        assert_eq!(
            page_of_dir("Research/SMASH", HOME),
            "Research/SMASH/SMASH.md"
        );
        assert_eq!(page_of_dir("Research", HOME), "Research/Research.md");
        assert_eq!(page_of_dir("", HOME), "Home.md");
    }

    #[test]
    fn finds_parents() {
        let parent = |path| parent_of(path, HOME);
        assert_eq!(parent("Home.md"), None);
        assert_eq!(parent("Research/Research.md").as_deref(), Some("Home.md"));
        assert_eq!(
            parent("Research/SMASH/SMASH.md").as_deref(),
            Some("Research/Research.md")
        );
        assert_eq!(
            parent("Research/CoSEMoS/Problem Statement/Problem Statement.md").as_deref(),
            Some("Research/CoSEMoS/CoSEMoS.md")
        );
    }

    #[test]
    fn enforces_the_layout() {
        let check = |path| check_page_path(path, HOME);
        assert!(check("Home.md").is_ok());
        assert!(check("Research/Research.md").is_ok());
        assert!(check("a/b/c/c.md").is_ok());

        let error = check("Imprint.md").unwrap_err().to_string();
        assert!(error.contains("move it to Imprint/Imprint.md"), "{error}");
        let error = check("Research/SMASH.md").unwrap_err().to_string();
        assert!(error.contains("like Research/Research.md"), "{error}");
        assert!(check("Research/Home.md").is_err());
    }

    #[test]
    fn parses_title_name_and_body() {
        let page = LocalPage::parse(
            "Research/SMASH/SMASH.md",
            "# SMASH: Storage\n\nSome *text*.\n",
            HOME,
        )
        .unwrap();
        assert_eq!(page.relative_path, "Research/SMASH/SMASH.md");
        assert_eq!(page.name, "SMASH");
        assert_eq!(page.title, "SMASH: Storage");
        assert_eq!(page.markdown, "Some *text*.\n");
        assert_eq!(page.parent.as_deref(), Some("Research/Research.md"));
        assert_eq!(page.dir(), "Research/SMASH");
        assert_eq!(page.files, []);
    }

    #[test]
    fn parses_the_home_page() {
        let home = LocalPage::parse(HOME, "# Welcome", HOME).unwrap();
        assert_eq!(home.name, "Home");
        assert_eq!(home.parent, None);
        assert_eq!(home.dir(), "");
    }

    #[test]
    fn accepts_titles_without_hash_and_windows_line_endings() {
        let page = LocalPage::parse("a/a.md", "Plain title\r\n\r\nbody", HOME).unwrap();
        assert_eq!(page.title, "Plain title");
        assert_eq!(page.markdown, "body");
        // Any heading level and a missing space after the `#` work, too.
        assert_eq!(
            LocalPage::parse("a/a.md", "##Sub", HOME).unwrap().title,
            "Sub"
        );
    }

    #[test]
    fn handles_bodies() {
        let markdown = |text| LocalPage::parse("a/a.md", text, HOME).unwrap().markdown;
        assert_eq!(markdown("# A"), "");
        assert_eq!(markdown("# A\n"), "");
        assert_eq!(markdown("# A\nfirst\n\nsecond\n"), "first\n\nsecond\n");
    }

    #[test]
    fn rejects_missing_titles() {
        assert!(LocalPage::parse("a/a.md", "", HOME).is_err());
        assert!(LocalPage::parse("a/a.md", "#   \nbody", HOME).is_err());
        assert!(LocalPage::parse("a/a.md", "\n# Title below a blank line", HOME).is_err());
    }

    #[test]
    fn error_messages_name_the_file() {
        let error = LocalPage::parse("Research/Research.md", "", HOME).unwrap_err();
        assert!(error.to_string().starts_with("Research/Research.md:"));
    }

    #[test]
    fn reads_pages_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SMASH.md");
        fs::write(&path, "# SMASH\nbody").unwrap();

        let page = LocalPage::new("Research/SMASH/SMASH.md", &path, HOME).unwrap();
        assert_eq!(page.title, "SMASH");
        assert_eq!(page.markdown, "body");
    }

    #[test]
    fn reports_unreadable_files() {
        let dir = tempfile::tempdir().unwrap();
        let error = LocalPage::new("x/x.md", &dir.path().join("missing.md"), HOME).unwrap_err();
        assert!(error.to_string().contains("Cannot read x/x.md"));

        // Invalid UTF-8 is an error, too, instead of garbled content.
        let path = dir.path().join("binary.md");
        fs::write(&path, [0xff, 0xfe, 0x00]).unwrap();
        assert!(LocalPage::new("b/b.md", &path, HOME).is_err());
    }
}
