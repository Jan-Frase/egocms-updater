//! Converts the existing CMS pages into markdown files, to start using this tool on an existing site.
//!
//! The files follow the layout `LocalState` expects, so the next upload adopts the pages
//! instead of creating new ones.

use crate::cms::cms_client::SiteClient;
use crate::cms::cms_page::CmsPage;
use crate::cms::cms_trait::CmsApi;
use crate::local::local_page::LocalPage;
use crate::local::local_state::LocalState;
use crate::settings::config::Config;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub fn download_pages(
    config: &Config,
    site_client: &mut SiteClient,
    home_id: u64,
    out_dir: &Path,
) -> Result<()> {
    // 1. Gather all CMS pages, convert them to local pages and write them to disk.
    let tree = gather_pages(site_client, home_id)?;
    let local_pages = to_local_pages(&tree, &config.home_page)?;
    write_pages(&local_pages, out_dir)?;

    // 2. Make sure the result is a valid pages' directory.
    LocalState::new(&out_dir.to_path_buf(), &config.home_page).with_context(|| {
        format!(
            "The downloaded pages in {} can't be uploaded as they are.",
            out_dir.display()
        )
    })?;
    println!(
        "\nDownloaded {} pages into {}.",
        local_pages.len(),
        out_dir.display()
    );
    Ok(())
}

/// A CMS page together with its position in the page tree.
struct TreeNode {
    page: CmsPage,
    /// `None` only for the home page.
    parent: Option<u64>,
    has_children: bool,
}

/// Fetches the home page and all pages below it. Parents come before their children.
fn gather_pages(cms: &mut dyn CmsApi, home_id: u64) -> Result<Vec<TreeNode>> {
    let mut nodes = Vec::new();
    let home = cms.page(home_id)?;
    gather_subtree(cms, home, None, &mut nodes)?;
    Ok(nodes)
}

fn gather_subtree(
    cms: &mut dyn CmsApi,
    page: CmsPage,
    parent: Option<u64>,
    nodes: &mut Vec<TreeNode>,
) -> Result<()> {
    let children = cms.children(page.id)?;
    let id = page.id;
    nodes.push(TreeNode {
        page,
        parent,
        has_children: !children.is_empty(),
    });

    // Recurse!
    for child in children {
        gather_subtree(cms, child, Some(id), nodes)?;
    }
    Ok(())
}

/// Decides the file of every page and converts its content to markdown.
fn to_local_pages(tree: &[TreeNode], home_page: &str) -> Result<Vec<LocalPage>> {
    let mut paths: HashMap<u64, String> = HashMap::new();
    let mut local_pages = Vec::new();

    for node in tree {
        let page = &node.page;
        let path = match (node.parent, page.source()) {
            (None, _) => home_page.to_string(),
            // Pages uploaded before keep their file, so the next upload recognizes them.
            (Some(_), Some(source)) => source.to_string(),
            // Pages not uploaded by this tool yet get a path derived from their name.
            (Some(parent_id), None) => {
                let parent_path = paths
                    .get(&parent_id)
                    .context("Parents are converted before their children.")?;
                default_path(
                    children_dir(parent_path, home_page),
                    &page.name,
                    node.has_children,
                )
            }
        };

        let local_page = LocalPage::parse(&path, &file_content(page)?, home_page)
            .with_context(|| format!("Converting page {} (id {}) failed.", page.name, page.id))?;
        paths.insert(page.id, path);
        local_pages.push(local_page);
    }
    Ok(local_pages)
}

/// The markdown file of a page: the title in the first line, then the content.
fn file_content(page: &CmsPage) -> Result<String> {
    let markdown = htmd::convert(page.content().unwrap_or_default())
        .with_context(|| format!("Converting page {} to markdown failed.", page.id))?;
    let markdown = markdown.trim();
    Ok(if markdown.is_empty() {
        format!("# {}\n", page.title)
    } else {
        format!("# {}\n\n{markdown}\n", page.title)
    })
}

fn write_pages(pages: &[LocalPage], out_dir: &Path) -> Result<()> {
    for page in pages {
        let path = out_dir.join(&page.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = if page.markdown.is_empty() {
            format!("# {}\n", page.title)
        } else {
            format!("# {}\n\n{}", page.title, page.markdown)
        };
        fs::write(&path, content).with_context(|| format!("Cannot write {}", path.display()))?;
        println!("Downloaded {}", page.relative_path);
    }
    Ok(())
}

/// Pages with children become `name/name.md` with the children next to it, others `name.md`.
fn default_path(dir: &str, name: &str, has_children: bool) -> String {
    let file = if has_children {
        format!("{name}/{name}.md")
    } else {
        format!("{name}.md")
    };
    if dir.is_empty() {
        file
    } else {
        format!("{dir}/{file}")
    }
}

/// The directory the children of the page stored at `parent_path` belong into.
/// The children of the home page live at the top level, all others next to their parent,
/// which is always an index file like `Research/Research.md`.
fn children_dir<'a>(parent_path: &'a str, home_page: &str) -> &'a str {
    if parent_path == home_page {
        return "";
    }
    parent_path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cms::mock_client::{FakeCms, ROOT};
    use serde_json::{Value, json};

    const HOME: &str = "Home.md";

    fn content(html: &str) -> Value {
        json!({ "_contents": { "center": [{ "content1": html }] } })
    }

    fn marked(source: &str) -> Value {
        json!({ "_sync": { "source": source } })
    }

    /// Runs the whole download (without the connection) into a new temporary directory.
    fn download_into_temp(cms: &mut FakeCms) -> (tempfile::TempDir, Vec<LocalPage>) {
        let dir = tempfile::tempdir().unwrap();
        let tree = gather_pages(cms, ROOT).unwrap();
        let pages = to_local_pages(&tree, HOME).unwrap();
        write_pages(&pages, dir.path()).unwrap();
        (dir, pages)
    }

    fn paths(pages: &[LocalPage]) -> Vec<&str> {
        pages
            .iter()
            .map(|page| page.relative_path.as_str())
            .collect()
    }

    #[test]
    fn writes_the_page_tree_as_markdown() {
        let mut cms = FakeCms::new();
        let research = cms.add(
            ROOT,
            "Research",
            &content("<p>We <strong>research</strong>.</p>"),
        );
        let cosemos = cms.add(research, "CoSEMoS", &content("<h2>Goals</h2>"));
        cms.add(cosemos, "Problem Statement", &json!({}));
        cms.add(research, "SMASH", &content("<p>Storage</p>"));
        cms.add(ROOT, "Imprint", &json!({}));

        let (dir, pages) = download_into_temp(&mut cms);
        assert_eq!(
            paths(&pages),
            [
                "Home.md",
                "Research/Research.md",
                "Research/CoSEMoS/CoSEMoS.md",
                "Research/CoSEMoS/Problem Statement.md",
                "Research/SMASH.md",
                "Imprint.md",
            ]
        );

        let read = |path: &str| fs::read_to_string(dir.path().join(path)).unwrap();
        assert_eq!(read("Home.md"), "# Home\n");
        assert_eq!(
            read("Research/Research.md"),
            "# Research\n\nWe **research**.\n"
        );
        assert_eq!(
            read("Research/CoSEMoS/CoSEMoS.md"),
            "# CoSEMoS\n\n## Goals\n"
        );
        assert_eq!(read("Imprint.md"), "# Imprint\n");
    }

    #[test]
    fn the_result_can_be_read_back_unchanged() {
        let mut cms = FakeCms::new();
        let research = cms.add(ROOT, "Research", &content("<p>Text</p>"));
        cms.add(research, "SMASH", &content("<ul><li>a</li><li>b</li></ul>"));

        let (dir, pages) = download_into_temp(&mut cms);
        let local = LocalState::new(&dir.path().to_path_buf(), HOME).unwrap();
        assert_eq!(local.pages, pages);
        assert_eq!(local.files, []);
    }

    #[test]
    fn sets_name_title_and_parent() {
        let mut cms = FakeCms::new();
        let research = cms.add(ROOT, "Research", &json!({}));
        cms.add(research, "SMASH", &json!({}));

        let (_dir, pages) = download_into_temp(&mut cms);
        let smash = &pages[2];
        assert_eq!(smash.name, "SMASH");
        assert_eq!(smash.title, "SMASH");
        assert_eq!(smash.parent.as_deref(), Some("Research/Research.md"));
        assert_eq!(pages[0].parent, None);
    }

    #[test]
    fn keeps_the_files_of_uploaded_pages() {
        let mut cms = FakeCms::new();
        let research = cms.add(ROOT, "Research", &marked("Research/Research.md"));
        // Without children it would normally become `Research/CoSEMoS.md`.
        cms.add(research, "CoSEMoS", &marked("Research/CoSEMoS/CoSEMoS.md"));

        let (_dir, pages) = download_into_temp(&mut cms);
        assert_eq!(
            paths(&pages),
            [
                "Home.md",
                "Research/Research.md",
                "Research/CoSEMoS/CoSEMoS.md"
            ]
        );
    }

    #[test]
    fn derives_paths() {
        assert_eq!(default_path("", "A", false), "A.md");
        assert_eq!(default_path("", "A", true), "A/A.md");
        assert_eq!(default_path("X", "A", false), "X/A.md");
        assert_eq!(children_dir("Home.md", HOME), "");
        assert_eq!(children_dir("Research/Research.md", HOME), "Research");
        assert_eq!(children_dir("a/b/b.md", HOME), "a/b");
        // A home page in a subdirectory still has its children at the top level.
        assert_eq!(children_dir("start/start.md", "start/start.md"), "");
    }
}
