use anyhow::{Context, bail};
use std::fs;
use std::path::Path;

/// A Markdown file that should exist as a CMS page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalPage {
    /// Path relative to the pages directory, always with `/` as a separator. E.g. `Research/SMASH.md`.
    pub relative_path: String,
    /// The CMS name of the page (and thus part of its URL): the file name without `.md`.
    pub name: String,
    /// The first line of the file, without the leading `#`.
    pub title: String,
    /// Everything after the first line.
    pub markdown: String,
    /// The relative_path of the parent page. `None` only for the home page.
    pub parent: Option<String>,
}

impl LocalPage {
    /// Reads and parses the markdown file at `full_path`.
    pub fn new(relative_path: &str, full_path: &Path, home_page: &str) -> anyhow::Result<Self> {
        let text = fs::read_to_string(full_path)
            .with_context(|| format!("Cannot read {relative_path}"))?;
        Self::parse(relative_path, &text, home_page)
    }

    /// Turns the content of a markdown file into a page.
    pub(crate) fn parse(relative_path: &str, text: &str, home_page: &str) -> anyhow::Result<Self> {
        let (first_line, markdown) = text.split_once('\n').unwrap_or((text, ""));
        let title = first_line.trim_start_matches('#').trim();
        if title.is_empty() {
            bail!("{relative_path}: the first line has to be the title, e.g. `# My Page`.");
        }

        let file_name = relative_path.rsplit('/').next().unwrap_or(relative_path);
        let name = file_name.strip_suffix(".md").unwrap_or(file_name);
        if name.is_empty() {
            bail!("{relative_path}: the file name must not be empty.");
        }

        Ok(Self {
            relative_path: relative_path.to_string(),
            name: name.to_string(),
            title: title.to_string(),
            markdown: markdown.trim_start_matches(['\r', '\n']).to_string(),
            parent: Self::parent_of(relative_path, home_page),
        })
    }

    /// Returns the source of the parent page, or `None` for the home page.
    fn parent_of(source: &str, home_page: &str) -> Option<String> {
        if source == home_page {
            return None;
        }
        let mut dirs: Vec<&str> = source.split('/').collect();
        let file = dirs.pop()?;
        let stem = file.strip_suffix(".md").unwrap_or(file);

        // `Research/Research.md` is the "index" of its directory, so its parent lives one level up.
        if dirs.last() == Some(&stem) {
            dirs.pop();
        }

        // Construct the parents relative_path.
        #[allow(clippy::option_if_let_else)]
        let parent = match dirs.last() {
            Some(last) => {
                let remainder = dirs.join("/");
                format!("{remainder}/{last}.md")
            }
            None => home_page.to_string(),
        };

        Some(parent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "Home.md";

    #[test]
    fn finds_parents() {
        let parent = |source| LocalPage::parent_of(source, HOME);
        assert_eq!(parent("Home.md"), None);
        assert_eq!(parent("blub.md").as_deref(), Some("Home.md"));
        assert_eq!(parent("Research/Research.md").as_deref(), Some("Home.md"));
        assert_eq!(
            parent("Research/SMASH.md").as_deref(),
            Some("Research/Research.md")
        );
        assert_eq!(
            parent("Research/CoSEMoS/CoSEMoS.md").as_deref(),
            Some("Research/Research.md")
        );
        assert_eq!(
            parent("Research/CoSEMoS/Problem Statement.md").as_deref(),
            Some("Research/CoSEMoS/CoSEMoS.md")
        );
        assert_eq!(
            parent("2026/Research/Research.md").as_deref(),
            Some("2026/2026.md")
        );
        // A different home page in a subdirectory is still the root.
        assert_eq!(
            LocalPage::parent_of("start/start.md", "start/start.md"),
            None
        );
    }

    #[test]
    fn finds_parents_of_deeply_nested_pages() {
        let parent = |source| LocalPage::parent_of(source, HOME);
        // The parent is the index file of the containing directory.
        assert_eq!(parent("a/b/c/d.md").as_deref(), Some("a/b/c/c.md"));
        // The index of a directory belongs to the index of the directory above.
        assert_eq!(parent("a/b/c/c.md").as_deref(), Some("a/b/b.md"));
        assert_eq!(parent("a/b/b.md").as_deref(), Some("a/a.md"));
        assert_eq!(parent("a/a.md").as_deref(), Some("Home.md"));
        // Same stem as a directory further up, but not the index of its own directory.
        assert_eq!(parent("a/b/a.md").as_deref(), Some("a/b/b.md"));
    }

    #[test]
    fn parses_title_name_and_body() {
        let page = LocalPage::parse(
            "Research/SMASH.md",
            "# SMASH: Storage\n\nSome *text*.\n",
            HOME,
        )
        .unwrap();
        assert_eq!(page.relative_path, "Research/SMASH.md");
        assert_eq!(page.name, "SMASH");
        assert_eq!(page.title, "SMASH: Storage");
        assert_eq!(page.markdown, "Some *text*.\n");
        assert_eq!(page.parent.as_deref(), Some("Research/Research.md"));
    }

    #[test]
    fn accepts_titles_without_hash_and_windows_line_endings() {
        let page = LocalPage::parse("a.md", "Plain title\r\n\r\nbody", HOME).unwrap();
        assert_eq!(page.title, "Plain title");
        assert_eq!(page.markdown, "body");

        // Any heading level and a missing space after the `#` work, too.
        assert_eq!(
            LocalPage::parse("a.md", "##Sub", HOME).unwrap().title,
            "Sub"
        );
    }

    #[test]
    fn handles_pages_without_body() {
        assert_eq!(LocalPage::parse("a.md", "# A", HOME).unwrap().markdown, "");
        assert_eq!(
            LocalPage::parse("a.md", "# A\n", HOME).unwrap().markdown,
            ""
        );
    }

    #[test]
    fn keeps_blank_lines_and_content_in_the_body() {
        let page = LocalPage::parse("a.md", "# A\nfirst\n\nsecond\n", HOME).unwrap();
        assert_eq!(page.markdown, "first\n\nsecond\n");
    }

    #[test]
    fn names_are_the_file_name_without_extension() {
        let name = |path| LocalPage::parse(path, "# T", HOME).unwrap().name;
        assert_eq!(name("a.md"), "a");
        assert_eq!(name("x/Problem Statement.md"), "Problem Statement");
        assert_eq!(name("x/v1.2.md"), "v1.2");
    }

    #[test]
    fn the_home_page_has_no_parent() {
        assert_eq!(LocalPage::parse(HOME, "# Home", HOME).unwrap().parent, None);
    }

    #[test]
    fn rejects_missing_titles() {
        assert!(LocalPage::parse("a.md", "", HOME).is_err());
        assert!(LocalPage::parse("a.md", "#   \nbody", HOME).is_err());
        assert!(LocalPage::parse("a.md", "\n# Title below a blank line", HOME).is_err());
    }

    #[test]
    fn rejects_empty_names() {
        assert!(LocalPage::parse(".md", "# T", HOME).is_err());
    }

    #[test]
    fn error_messages_name_the_file() {
        let error = LocalPage::parse("Research/SMASH.md", "", HOME).unwrap_err();
        assert!(error.to_string().starts_with("Research/SMASH.md:"));
    }

    #[test]
    fn reads_pages_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SMASH.md");
        fs::write(&path, "# SMASH\nbody").unwrap();

        let page = LocalPage::new("Research/SMASH.md", &path, HOME).unwrap();
        assert_eq!(page.title, "SMASH");
        assert_eq!(page.markdown, "body");
        assert_eq!(page.relative_path, "Research/SMASH.md");
    }

    #[test]
    fn reports_unreadable_files() {
        let dir = tempfile::tempdir().unwrap();
        let error = LocalPage::new("x.md", &dir.path().join("missing.md"), HOME).unwrap_err();
        assert!(error.to_string().contains("Cannot read x.md"));

        // Invalid UTF-8 is an error, too, instead of garbled content.
        let path = dir.path().join("binary.md");
        fs::write(&path, [0xff, 0xfe, 0x00]).unwrap();
        assert!(LocalPage::new("binary.md", &path, HOME).is_err());
    }
}
