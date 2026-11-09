use crate::local::local_file::LocalFile;
use crate::local::local_page::LocalPage;
use anyhow::{Context, Result, bail};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Debug)]
pub struct LocalState {
    pub pages: Vec<LocalPage>,
    pub files: Vec<LocalFile>,
}

impl LocalState {
    pub fn new(dir: &PathBuf, home_page: &str) -> Result<Self> {
        let mut pages = Vec::new();
        let mut files = Vec::new();

        for full_path in WalkDir::new(dir).sort_by_file_name() {
            let full_path = full_path?;
            if !full_path.file_type().is_file() {
                continue;
            }
            let relative_path = full_path.path().strip_prefix(dir)?;
            let full_path = full_path.path();
            let relative_path = relative_path
                .iter()
                .map(|part| part.to_str())
                .collect::<Option<Vec<_>>>()
                .with_context(|| format!("{} is not valid UTF-8.", relative_path.display()))?
                .join("/");

            // Skip hidden files like `.gitkeep` or `.DS_Store`.
            if relative_path.split('/').any(|part| part.starts_with('.')) {
                continue;
            }

            if std::path::Path::new(&relative_path)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
            {
                pages.push(LocalPage::new(&relative_path, full_path, home_page)?);
            } else {
                files.push(LocalFile::new(relative_path, full_path)?);
            }
        }

        let pages = Self::order_and_validate(pages, home_page)?;

        Ok(Self { pages, files })
    }

    /// Checks that the pages form a proper tree and sorts parents before their children.
    fn order_and_validate(
        pages: Vec<LocalPage>,
        home_page: &str,
    ) -> anyhow::Result<Vec<LocalPage>> {
        let sources: HashSet<&str> = pages
            .iter()
            .map(|page| page.relative_path.as_str())
            .collect();
        if !sources.contains(home_page) {
            bail!("The home page {home_page} does not exist.");
        }

        let mut siblings: HashMap<(&str, &str), &str> = HashMap::new();
        for page in &pages {
            let Some(parent) = &page.parent else { continue };
            if !sources.contains(parent.as_str()) {
                bail!(
                    "{} needs the parent page {parent}, which does not exist.",
                    page.relative_path
                );
            }
            if let Some(other) = siblings.insert((parent, &page.name), &page.relative_path) {
                bail!(
                    "{other} and {} would both become the page `{}` below {parent}.",
                    page.relative_path,
                    page.name
                );
            }
        }

        // The depth of a page is the number of its ancestors. Sorting by it puts parents first.
        let parents: HashMap<&str, Option<&str>> = pages
            .iter()
            .map(|page| (page.relative_path.as_str(), page.parent.as_deref()))
            .collect();
        let depths: HashMap<String, usize> = pages
            .iter()
            .map(|page| {
                let ancestors = std::iter::successors(page.parent.as_deref(), |parent| {
                    parents.get(parent).copied().flatten()
                });
                (page.relative_path.clone(), ancestors.count())
            })
            .collect();

        let mut pages = pages;
        pages.sort_by(|a, b| {
            (depths[&a.relative_path], &a.relative_path)
                .cmp(&(depths[&b.relative_path], &b.relative_path))
        });
        Ok(pages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    const HOME: &str = "Home.md";

    fn page(relative_path: &str) -> LocalPage {
        LocalPage::parse(relative_path, "# Title", HOME).unwrap()
    }

    fn sources(pages: &[LocalPage]) -> Vec<&str> {
        pages
            .iter()
            .map(|page| page.relative_path.as_str())
            .collect()
    }

    /// Creates the given files (path, content) in a new temporary directory.
    fn directory(files: &[(&str, &[u8])]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, content) in files {
            let path = dir.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        dir
    }

    fn scan(dir: &Path) -> Result<LocalState> {
        LocalState::new(&dir.to_path_buf(), &HOME.to_string())
    }

    #[test]
    fn orders_parents_first() {
        let pages = vec![
            page("Research/CoSEMoS/Problem Statement.md"),
            page("Research/SMASH.md"),
            page("Research/CoSEMoS/CoSEMoS.md"),
            page("Home.md"),
            page("Research/Research.md"),
        ];
        let ordered = LocalState::order_and_validate(pages, HOME).unwrap();
        assert_eq!(
            sources(&ordered),
            [
                "Home.md",
                "Research/Research.md",
                "Research/CoSEMoS/CoSEMoS.md",
                "Research/SMASH.md",
                "Research/CoSEMoS/Problem Statement.md",
            ]
        );
    }

    #[test]
    fn ordering_is_deterministic_within_a_level() {
        let pages = vec![page("c.md"), page("Home.md"), page("a.md"), page("b.md")];
        let ordered = LocalState::order_and_validate(pages, HOME).unwrap();
        assert_eq!(sources(&ordered), ["Home.md", "a.md", "b.md", "c.md"]);
    }

    #[test]
    fn accepts_a_lonely_home_page() {
        let ordered = LocalState::order_and_validate(vec![page("Home.md")], HOME).unwrap();
        assert_eq!(sources(&ordered), ["Home.md"]);
    }

    #[test]
    fn rejects_a_missing_home_page() {
        let error = LocalState::order_and_validate(vec![page("a.md")], HOME).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("home page Home.md does not exist")
        );
        assert!(LocalState::order_and_validate(vec![], HOME).is_err());
    }

    #[test]
    fn rejects_missing_parents() {
        let pages = vec![page("Home.md"), page("Research/SMASH.md")];
        let error = LocalState::order_and_validate(pages, HOME).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("Research/SMASH.md"), "{message}");
        assert!(message.contains("Research/Research.md"), "{message}");
    }

    #[test]
    fn rejects_pages_with_the_same_name_below_the_same_parent() {
        // Both become the page `Research` below the home page.
        let pages = vec![
            page("Home.md"),
            page("Research.md"),
            page("Research/Research.md"),
        ];
        let error = LocalState::order_and_validate(pages, HOME).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("would both become the page `Research`")
        );
    }

    #[test]
    fn allows_the_same_name_below_different_parents() {
        let pages = vec![
            page("Home.md"),
            page("A/A.md"),
            page("B/B.md"),
            page("A/News.md"),
            page("B/News.md"),
        ];
        assert!(LocalState::order_and_validate(pages, HOME).is_ok());
    }

    #[test]
    fn scans_a_directory() {
        let dir = directory(&[
            ("Home.md", b"# Home\nWelcome"),
            ("Research/Research.md", b"# Research"),
            ("Research/diagram.png", &[1, 2, 3]),
            (".gitkeep", b""),
            ("Research/.hidden/secret.md", b"# Secret"),
        ]);

        let local = scan(dir.path()).unwrap();
        assert_eq!(sources(&local.pages), ["Home.md", "Research/Research.md"]);
        assert_eq!(
            local.files,
            [LocalFile {
                relative_path: "Research/diagram.png".into(),
                bytes: vec![1, 2, 3]
            }]
        );
    }

    #[test]
    fn scans_a_directory_without_files() {
        let dir = directory(&[("Home.md", b"# Home")]);
        let local = scan(dir.path()).unwrap();
        assert_eq!(local.pages.len(), 1);
        assert_eq!(local.files, []);
    }

    #[test]
    fn treats_everything_but_markdown_as_files() {
        let dir = directory(&[
            ("Home.md", b"# Home"),
            ("a.pdf", b"pdf"),
            ("notes.txt", b"txt"),
            ("Research/slides.MD", b"# not a page"),
            ("no_extension", b"x"),
        ]);
        let local = scan(dir.path()).unwrap();
        assert_eq!(sources(&local.pages), ["Home.md"]);
        let files: Vec<_> = local
            .files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect();
        assert_eq!(
            files,
            ["Research/slides.MD", "a.pdf", "no_extension", "notes.txt"]
        );
    }

    #[test]
    fn uses_slashes_in_relative_paths_for_nested_directories() {
        let dir = directory(&[
            ("Home.md", b"# Home"),
            ("Research/Research.md", b"# R"),
            ("Research/CoSEMoS/CoSEMoS.md", b"# C"),
            ("Research/CoSEMoS/img/a.png", b"png"),
        ]);
        let local = scan(dir.path()).unwrap();
        assert!(sources(&local.pages).contains(&"Research/CoSEMoS/CoSEMoS.md"));
        assert_eq!(local.files[0].relative_path, "Research/CoSEMoS/img/a.png");
    }

    #[test]
    fn skips_hidden_files_and_directories() {
        let dir = directory(&[
            ("Home.md", b"# Home"),
            (".DS_Store", b"x"),
            (".git/config", b"x"),
            ("Research/.hidden.md", b"# Hidden"),
            ("Research/Research.md", b"# R"),
        ]);
        let local = scan(dir.path()).unwrap();
        assert_eq!(sources(&local.pages), ["Home.md", "Research/Research.md"]);
        assert_eq!(local.files, []);
    }

    #[test]
    fn fails_without_a_home_page() {
        let dir = directory(&[("Research/Research.md", b"# R")]);
        assert!(scan(dir.path()).is_err());
    }

    #[test]
    fn fails_for_missing_directories() {
        let dir = tempfile::tempdir().unwrap();
        assert!(scan(&dir.path().join("does-not-exist")).is_err());
    }

    #[test]
    fn reports_invalid_pages_with_their_path() {
        let dir = directory(&[("Home.md", b"# Home"), ("Research/Research.md", b"")]);
        let error = scan(dir.path()).unwrap_err();
        assert!(error.to_string().contains("Research/Research.md"));
    }

    #[test]
    fn fails_for_pages_without_a_parent() {
        let dir = directory(&[("Home.md", b"# Home"), ("Research/SMASH.md", b"# S")]);
        assert!(scan(dir.path()).is_err());
    }
}
