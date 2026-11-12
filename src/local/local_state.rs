//! Reads the whole pages directory. See [`crate::local::local_page`] for the layout.

use crate::local::local_file::LocalFile;
use crate::local::local_page::{LocalPage, dir_of, page_of_dir};
use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug)]
pub struct LocalState {
    /// All pages, parents before their children. The first one is the home page.
    pub pages: Vec<LocalPage>,
}

impl LocalState {
    pub fn new(dir: &PathBuf, home_page: &str) -> Result<Self> {
        let mut pages = Vec::new();
        let mut files = Vec::new();

        for entry in WalkDir::new(dir).sort_by_file_name() {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            let relative_path = relative_path(entry.path(), dir)?;

            // Skip hidden files like `.gitkeep` or `.DS_Store`.
            if relative_path.split('/').any(|part| part.starts_with('.')) {
                continue;
            }

            // Only lowercase `.md` files are pages. `.MD` and the like are most likely a mistake.
            let extension = Path::new(&relative_path).extension();
            let is_page = extension.is_some_and(|ext| ext == "md");
            if !is_page && extension.is_some_and(|ext| ext.eq_ignore_ascii_case("md")) {
                bail!("{relative_path}: rename it to use the lowercase extension `.md`.");
            }
            if is_page {
                pages.push(LocalPage::new(&relative_path, entry.path(), home_page)?);
            } else {
                files.push(LocalFile::new(relative_path, entry.path())?);
            }
        }

        let mut pages = Self::order_and_validate(pages, home_page)?;
        Self::attach_files(&mut pages, files, home_page)?;
        Ok(Self { pages })
    }

    /// Checks that every page has its parent and sorts parents before their children.
    fn order_and_validate(mut pages: Vec<LocalPage>, home_page: &str) -> Result<Vec<LocalPage>> {
        if !pages.iter().any(|page| page.relative_path == home_page) {
            bail!("The home page {home_page} does not exist.");
        }
        for page in &pages {
            let Some(parent) = &page.parent else { continue };
            if !pages.iter().any(|other| other.relative_path == *parent) {
                bail!(
                    "{} needs the parent page {parent}, which does not exist.",
                    page.relative_path
                );
            }
        }

        // Every page has its own directory, so the depth of a page is the number of directories in its path.
        pages.sort_by_cached_key(|page| {
            let depth = if page.parent.is_none() {
                0
            } else {
                page.relative_path.matches('/').count()
            };
            (depth, page.relative_path.clone())
        });
        Ok(pages)
    }

    /// Gives every file to the page of the directory it lies in.
    fn attach_files(pages: &mut [LocalPage], files: Vec<LocalFile>, home_page: &str) -> Result<()> {
        let index: HashMap<String, usize> = pages
            .iter()
            .enumerate()
            .map(|(index, page)| (page.relative_path.clone(), index))
            .collect();
        for file in files {
            let owner = page_of_dir(dir_of(&file.relative_path), home_page);
            let Some(&page) = index.get(&owner) else {
                bail!(
                    "{}: files belong to the page of their directory, but {owner} does not exist.",
                    file.relative_path
                );
            };
            pages[page].files.push(file);
        }
        Ok(())
    }
}

/// The path of `path` relative to `dir`, always with `/` as a separator.
fn relative_path(path: &Path, dir: &Path) -> Result<String> {
    let relative = path.strip_prefix(dir)?;
    Ok(relative
        .iter()
        .map(|part| part.to_str())
        .collect::<Option<Vec<_>>>()
        .with_context(|| format!("{} is not valid UTF-8.", relative.display()))?
        .join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const HOME: &str = "Home.md";

    fn page(relative_path: &str) -> LocalPage {
        LocalPage::parse(relative_path, "# Title", HOME).unwrap()
    }

    fn paths(pages: &[LocalPage]) -> Vec<&str> {
        pages
            .iter()
            .map(|page| page.relative_path.as_str())
            .collect()
    }

    fn file_paths(page: &LocalPage) -> Vec<&str> {
        page.files
            .iter()
            .map(|file| file.relative_path.as_str())
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
        LocalState::new(&dir.to_path_buf(), HOME)
    }

    #[test]
    fn orders_parents_first() {
        let pages = vec![
            page("Research/CoSEMoS/Problem Statement/Problem Statement.md"),
            page("Research/SMASH/SMASH.md"),
            page("Research/CoSEMoS/CoSEMoS.md"),
            page("Home.md"),
            page("Research/Research.md"),
            page("Imprint/Imprint.md"),
        ];
        let ordered = LocalState::order_and_validate(pages, HOME).unwrap();
        assert_eq!(
            paths(&ordered),
            [
                "Home.md",
                "Imprint/Imprint.md",
                "Research/Research.md",
                "Research/CoSEMoS/CoSEMoS.md",
                "Research/SMASH/SMASH.md",
                "Research/CoSEMoS/Problem Statement/Problem Statement.md",
            ]
        );
    }

    #[test]
    fn rejects_a_missing_home_page() {
        let error = LocalState::order_and_validate(vec![page("a/a.md")], HOME).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("home page Home.md does not exist")
        );
        assert!(LocalState::order_and_validate(vec![], HOME).is_err());
    }

    #[test]
    fn rejects_missing_parents() {
        let pages = vec![page("Home.md"), page("Research/SMASH/SMASH.md")];
        let error = LocalState::order_and_validate(pages, HOME).unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("Research/SMASH/SMASH.md needs the parent page Research/Research.md"),
            "{message}"
        );
    }

    #[test]
    fn scans_a_directory() {
        let dir = directory(&[
            ("Home.md", b"# Home\nWelcome"),
            ("logo.png", &[9]),
            ("Research/Research.md", b"# Research"),
            ("Research/SMASH/SMASH.md", b"# SMASH"),
            ("Research/SMASH/slides.pdf", &[1, 2, 3]),
            ("Research/SMASH/img/plot.png", &[4]),
        ]);
        // `img/` has no page of its own, so the plot can't belong anywhere.
        assert!(scan(dir.path()).is_err());

        fs::remove_dir_all(dir.path().join("Research/SMASH/img")).unwrap();
        let local = scan(dir.path()).unwrap();
        assert_eq!(
            paths(&local.pages),
            ["Home.md", "Research/Research.md", "Research/SMASH/SMASH.md"]
        );
        assert_eq!(file_paths(&local.pages[0]), ["logo.png"]);
        assert_eq!(file_paths(&local.pages[1]), Vec::<&str>::new());
        assert_eq!(
            local.pages[2].files,
            [LocalFile {
                relative_path: "Research/SMASH/slides.pdf".into(),
                bytes: vec![1, 2, 3]
            }]
        );
    }

    #[test]
    fn reports_files_without_a_page() {
        let dir = directory(&[("Home.md", b"# Home"), ("img/plot.png", &[1])]);
        let error = scan(dir.path()).unwrap_err().to_string();
        assert!(error.contains("img/plot.png"), "{error}");
        assert!(error.contains("img/img.md does not exist"), "{error}");
    }

    #[test]
    fn rejects_pages_that_break_the_layout() {
        for misplaced in ["Imprint.md", "Research/SMASH.md", "Research/Notes/Other.md"] {
            let dir = directory(&[
                ("Home.md", b"# Home"),
                ("Research/Research.md", b"# R"),
                (misplaced, b"# X"),
            ]);
            let error = scan(dir.path()).unwrap_err();
            assert!(error.to_string().contains(misplaced), "{error}");
        }
    }

    #[test]
    fn treats_everything_but_markdown_as_files() {
        let dir = directory(&[
            ("Home.md", b"# Home"),
            ("a.pdf", b"pdf"),
            ("notes.txt", b"txt"),
            ("readme.mdx", b"# not a page"),
            ("no_extension", b"x"),
        ]);
        let local = scan(dir.path()).unwrap();
        assert_eq!(paths(&local.pages), ["Home.md"]);
        assert_eq!(
            file_paths(&local.pages[0]),
            ["a.pdf", "no_extension", "notes.txt", "readme.mdx"]
        );
    }

    #[test]
    fn stops_at_uppercase_markdown_extensions() {
        for name in ["Notes/Notes.MD", "Notes/Notes.Md", "Notes/Notes.mD"] {
            let dir = directory(&[("Home.md", b"# Home"), (name, b"# N")]);
            let error = scan(dir.path()).unwrap_err().to_string();
            assert!(error.contains(name), "{error}");
            assert!(error.contains("lowercase extension"), "{error}");
        }
    }

    #[test]
    fn skips_hidden_files_and_directories() {
        let dir = directory(&[
            ("Home.md", b"# Home"),
            (".DS_Store", b"x"),
            (".git/config", b"x"),
            (".drafts/Notes.MD", b"# N"),
            ("Research/Research.md", b"# R"),
            ("Research/.hidden.pdf", b"x"),
        ]);
        let local = scan(dir.path()).unwrap();
        assert_eq!(paths(&local.pages), ["Home.md", "Research/Research.md"]);
        assert!(local.pages.iter().all(|page| page.files.is_empty()));
    }

    #[test]
    fn fails_without_a_home_page_or_directory() {
        let dir = directory(&[("Research/Research.md", b"# R")]);
        assert!(scan(dir.path()).is_err());
        assert!(scan(&dir.path().join("does-not-exist")).is_err());
    }

    #[test]
    fn reports_invalid_pages_with_their_path() {
        let dir = directory(&[("Home.md", b"# Home"), ("Research/Research.md", b"")]);
        let error = scan(dir.path()).unwrap_err();
        assert!(error.to_string().contains("Research/Research.md"));
    }
}
