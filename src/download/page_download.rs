//! Converts the existing CMS pages into markdown files, to start using this tool on an existing site.
//!
//! The files follow the layout described in [`crate::local::local_page`], so the next upload adopts
//! the pages instead of creating new ones: every page `name` goes to `<parent dir>/<name>/<name>.md`.

use crate::cms::cms_page::CmsPage;
use crate::cms::cms_trait::{CmsApi, percent_decode};
use crate::local::local_page::{LocalPage, dir_of, file_of_name};
use anyhow::{Context, Result, bail};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

/// A downloaded page, together with the id of the CMS page it came from.
pub struct DownloadedPage {
    pub id: u64,
    pub page: LocalPage,
}

/// What to download.
pub struct Source<'a> {
    pub site: &'a str,
    pub home_id: u64,
    pub home_page: &'a str,
    /// Pages of other types (e.g. people pages) are skipped, together with all pages below them.
    pub page_types: &'a [&'a str],
}

/// Downloads the home page and all pages below it into `out_dir`.
/// Pages markdown can't represent are skipped and listed.
pub fn download_pages(
    cms: &mut dyn CmsApi,
    from: &Source,
    out_dir: &Path,
) -> Result<Vec<DownloadedPage>> {
    let mut skipped = BTreeMap::new();
    let tree = gather_pages(cms, from, &mut skipped)?;
    let pages = to_local_pages(&tree, from.site, from.home_page)?;
    write_pages(&pages, out_dir)?;
    for (reason, names) in skipped {
        eprintln!(
            "Skipped {} pages {reason} (and the pages below them): {}",
            names.len(),
            names.join(", ")
        );
    }
    Ok(pages)
}

/// A CMS page together with its parent.
struct TreeNode {
    page: CmsPage,
    /// `None` only for the home page.
    parent: Option<u64>,
}

/// Fetches the home page and all representable pages below it. Parents come before their children.
/// The names of skipped pages are collected in `skipped`, by the reason they were skipped.
fn gather_pages(
    cms: &mut dyn CmsApi,
    from: &Source,
    skipped: &mut BTreeMap<String, Vec<String>>,
) -> Result<Vec<TreeNode>> {
    let home = cms.page(from.home_id)?;
    // The home page is always downloaded, whatever its type, but it must not lose content.
    if !home.holds_only_text() {
        bail!(
            "The home page {} (id {}) contains more than a single text block, so markdown can't represent it.",
            home.name,
            home.id
        );
    }
    let mut nodes = Vec::new();
    gather_subtree(cms, home, None, from.page_types, &mut nodes, skipped)?;
    Ok(nodes)
}

fn gather_subtree(
    cms: &mut dyn CmsApi,
    page: CmsPage,
    parent: Option<u64>,
    page_types: &[&str],
    nodes: &mut Vec<TreeNode>,
    skipped: &mut BTreeMap<String, Vec<String>>,
) -> Result<()> {
    let id = page.id;
    nodes.push(TreeNode { page, parent });
    for child in cms.children(id)? {
        if let Some(reason) = child.not_representable(page_types) {
            skipped.entry(reason).or_default().push(child.name);
            continue;
        }
        gather_subtree(cms, child, Some(id), page_types, nodes, skipped)?;
    }
    Ok(())
}

/// Decides the file of every page and converts its content to markdown.
fn to_local_pages(tree: &[TreeNode], site: &str, home_page: &str) -> Result<Vec<DownloadedPage>> {
    let paths = assign_paths(tree, home_page)?;
    tree.iter()
        .map(|node| {
            let page = &node.page;
            let path = &paths[&page.id];
            let html = rewrite_cms_links(page.content().unwrap_or_default(), path, &paths, site);
            let markdown = htmd::convert(&html)
                .with_context(|| format!("Converting page {} to markdown failed.", page.id))?;
            let text = format!("# {}\n\n{}\n", page.title, markdown.trim());
            let local_page = LocalPage::parse(path, &text, home_page).with_context(|| {
                format!("Converting page {} (id {}) failed.", page.name, page.id)
            })?;
            Ok(DownloadedPage {
                id: page.id,
                page: local_page,
            })
        })
        .collect()
}

/// The path of every page: the home page at the top, every other page in its own directory below its parent.
fn assign_paths(tree: &[TreeNode], home_page: &str) -> Result<HashMap<u64, String>> {
    let mut paths: HashMap<u64, String> = HashMap::new();
    for node in tree {
        let path = match node.parent {
            None => home_page.to_string(),
            Some(parent) => {
                let parent_path = paths
                    .get(&parent)
                    .context("Parents are downloaded before their children.")?;
                let name = file_of_name(&node.page.name);
                match dir_of(parent_path) {
                    "" => format!("{name}/{name}.md"),
                    dir => format!("{dir}/{name}/{name}.md"),
                }
            }
        };
        paths.insert(node.page.id, path);
    }
    Ok(paths)
}

fn write_pages(pages: &[DownloadedPage], out_dir: &Path) -> Result<()> {
    for DownloadedPage { page, .. } in pages {
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

/// Rewrites the links the EgoCMS editor stores into the relative paths the upload expects:
/// - `index.php?site=…&id=7` => the file of page 7, e.g. `../SMASH/SMASH.md`
/// - `index.php?pool=a.pdf&site=…&id=7` => the file in the directory of page 7, e.g. `../SMASH/a.pdf`
///
/// Links to other sites or to pages that weren't downloaded become absolute (`/index.php?…`), so they keep working.
fn rewrite_cms_links(
    html: &str,
    current: &str,
    paths: &HashMap<u64, String>,
    site: &str,
) -> String {
    const ATTRIBUTES: [&str; 2] = ["href=\"", "src=\""];
    let mut result = String::with_capacity(html.len());
    let mut rest = html;
    while let Some((start, attribute)) = ATTRIBUTES
        .iter()
        .filter_map(|attribute| rest.find(attribute).map(|start| (start, attribute.len())))
        .min()
    {
        let value_start = start + attribute;
        let Some(value_len) = rest[value_start..].find('"') else {
            break;
        };
        result.push_str(&rest[..value_start]);
        let value = &rest[value_start..value_start + value_len];
        result.push_str(&rewrite_url(value, current, paths, site));
        rest = &rest[value_start + value_len..];
    }
    result.push_str(rest);
    result
}

/// Rewrites one (HTML escaped) attribute value, see [`rewrite_cms_links`].
fn rewrite_url(value: &str, current: &str, paths: &HashMap<u64, String>, site: &str) -> String {
    let url = value.replace("&amp;", "&");
    let Some(query) = url.strip_prefix("index.php?") else {
        return value.to_string();
    };
    let (query, anchor) = query
        .split_once('#')
        .map_or((query, None), |(query, anchor)| (query, Some(anchor)));
    let params: HashMap<&str, String> = query
        .split('&')
        .filter_map(|param| param.split_once('='))
        .map(|(key, value)| (key, form_decode(value)))
        .collect();

    let same_site = params.get("site").is_none_or(|other| other == site);
    let target = params
        .get("id")
        .and_then(|id| id.parse::<u64>().ok())
        .and_then(|id| paths.get(&id))
        .filter(|_| same_site);
    let rewritten = match (target, params.get("pool")) {
        (Some(page), Some(file)) => match dir_of(page) {
            "" => relative_path(dir_of(current), file),
            dir => relative_path(dir_of(current), &format!("{dir}/{file}")),
        },
        (Some(page), None) => relative_path(dir_of(current), page),
        (None, _) => return format!("/{}", url.replace('&', "&amp;")),
    };
    let rewritten = match anchor {
        Some(anchor) => format!("{rewritten}#{anchor}"),
        None => rewritten,
    };
    rewritten.replace('&', "&amp;")
}

/// The path of `target` relative to the directory `from`, e.g. (`Research/SMASH`, `Home.md`) => `../../Home.md`.
fn relative_path(from: &str, target: &str) -> String {
    let from: Vec<&str> = from.split('/').filter(|part| !part.is_empty()).collect();
    let target: Vec<&str> = target.split('/').collect();
    let (target_dirs, file) = target.split_at(target.len() - 1);
    let common = from
        .iter()
        .zip(target_dirs)
        .take_while(|(a, b)| a == b)
        .count();
    let ups = std::iter::repeat_n("..", from.len() - common);
    ups.chain(target_dirs[common..].iter().copied())
        .chain(file.iter().copied())
        .collect::<Vec<_>>()
        .join("/")
}

/// Decodes a query value like an HTML form: `notes+v1.txt` => `notes v1.txt`, `%C3%9C` => `Ü`.
fn form_decode(value: &str) -> String {
    // A literal `+` is encoded as `%2B`, so replacing `+` first is safe.
    percent_decode(&value.replace('+', " "))
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

    fn source<'a>(page_types: &'a [&'a str]) -> Source<'a> {
        Source {
            site: "parcio",
            home_id: ROOT,
            home_page: HOME,
            page_types,
        }
    }

    /// Downloads the fake CMS into a new temporary directory.
    fn download(cms: &mut FakeCms) -> (tempfile::TempDir, Vec<DownloadedPage>) {
        let dir = tempfile::tempdir().unwrap();
        let types = ["page"];
        let pages = download_pages(cms, &source(&types), dir.path()).unwrap();
        (dir, pages)
    }

    fn paths(pages: &[DownloadedPage]) -> Vec<&str> {
        pages
            .iter()
            .map(|downloaded| downloaded.page.relative_path.as_str())
            .collect()
    }

    #[test]
    fn writes_every_page_into_its_own_directory() {
        let mut cms = FakeCms::new();
        let research = cms.add(
            ROOT,
            "Research",
            &content("<p>We <strong>research</strong>.</p>"),
        );
        let cosemos = cms.add(research, "CoSEMoS", &content("<h2>Goals</h2>"));
        cms.add(cosemos, "Problem Statement", &json!({}));
        cms.add(research, "SMASH", &json!({}));

        let (dir, pages) = download(&mut cms);
        assert_eq!(
            paths(&pages),
            [
                "Home.md",
                "Research/Research.md",
                "Research/CoSEMoS/CoSEMoS.md",
                "Research/CoSEMoS/Problem Statement/Problem Statement.md",
                "Research/SMASH/SMASH.md",
            ]
        );
        assert_eq!(pages[1].id, research);

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
    }

    #[test]
    fn sets_name_title_and_parent() {
        let mut cms = FakeCms::new();
        let research = cms.add(ROOT, "Research", &json!({}));
        cms.add(research, "SMASH", &json!({}));

        let (_dir, pages) = download(&mut cms);
        let smash = &pages[2].page;
        assert_eq!(smash.name, "SMASH");
        assert_eq!(smash.title, "SMASH");
        assert_eq!(smash.parent.as_deref(), Some("Research/Research.md"));
        assert_eq!(pages[0].page.parent, None);
    }

    #[test]
    fn rewrites_page_and_pool_links_into_relative_paths() {
        let paths: HashMap<u64, String> = [
            (1, "Home.md".to_string()),
            (2, "Research/Research.md".to_string()),
            (3, "Research/SMASH/SMASH.md".to_string()),
        ]
        .into();
        let rewrite =
            |html: &str, current: &str| rewrite_cms_links(html, current, &paths, "parcio");

        assert_eq!(
            rewrite(
                r#"<a href="index.php?site=parcio&amp;id=3&amp;lang=de#goals">S</a>"#,
                "Home.md"
            ),
            r#"<a href="Research/SMASH/SMASH.md#goals">S</a>"#
        );
        assert_eq!(
            rewrite(
                r#"<a href="index.php?site=parcio&amp;id=1&amp;lang=de">H</a>"#,
                "Research/SMASH/SMASH.md"
            ),
            r#"<a href="../../Home.md">H</a>"#
        );
        assert_eq!(
            rewrite(
                r#"<img src="index.php?pool=PSS-01+Intro.pdf&amp;width=10&amp;site=parcio&amp;lang=de&amp;id=3">"#,
                "Research/Research.md"
            ),
            r#"<img src="SMASH/PSS-01 Intro.pdf">"#
        );
        assert_eq!(
            rewrite(
                r#"<a href="index.php?pool=logo.png&amp;site=parcio&amp;lang=de&amp;id=1">L</a>"#,
                "Home.md"
            ),
            r#"<a href="logo.png">L</a>"#
        );
    }

    #[test]
    fn makes_other_links_absolute_or_leaves_them_alone() {
        let paths: HashMap<u64, String> = [(1, "Home.md".to_string())].into();
        let rewrite = |html: &str| rewrite_cms_links(html, "Home.md", &paths, "parcio");
        // Another site, or a page that wasn't downloaded (e.g. a person page).
        assert_eq!(
            rewrite(r#"<img src="index.php?id=5&amp;site=parcio_media">"#),
            r#"<img src="/index.php?id=5&amp;site=parcio_media">"#
        );
        assert_eq!(
            rewrite(r#"<a href="index.php?site=parcio&amp;id=99">P</a>"#),
            r#"<a href="/index.php?site=parcio&amp;id=99">P</a>"#
        );
        for html in [
            r#"<a href="https://parcio.de">x</a>"#,
            r#"<a href="/News.html">x</a>"#,
            "<p>no links</p>",
        ] {
            assert_eq!(rewrite(html), html);
        }
    }

    #[test]
    fn computes_relative_paths() {
        assert_eq!(
            relative_path("", "Research/Research.md"),
            "Research/Research.md"
        );
        assert_eq!(
            relative_path("Research", "Research/SMASH/SMASH.md"),
            "SMASH/SMASH.md"
        );
        assert_eq!(relative_path("Research/SMASH", "Home.md"), "../../Home.md");
        assert_eq!(
            relative_path("Research/SMASH", "Research/JULEA/JULEA.md"),
            "../JULEA/JULEA.md"
        );
        assert_eq!(
            relative_path("Research/SMASH", "Research/SMASH/a.pdf"),
            "a.pdf"
        );
    }

    #[test]
    fn decodes_form_values() {
        assert_eq!(form_decode("notes+v1.txt"), "notes v1.txt");
        assert_eq!(form_decode("1%2B1"), "1+1");
        assert_eq!(form_decode("%C3%9Cbung%231%262.pdf"), "Übung#1&2.pdf");
        // Broken escapes are kept as they are.
        assert_eq!(form_decode("100%"), "100%");
        assert_eq!(form_decode("%zz"), "%zz");
    }

    #[test]
    fn downloaded_links_survive_the_markdown_conversion() {
        let mut cms = FakeCms::new();
        let research = cms.add(ROOT, "Research", &json!({}));
        let ps = cms.add(research, "Problem Statement", &json!({}));
        cms.add(
            ROOT,
            "Links",
            &content(&format!(
                r#"<p><a href="index.php?site=parcio&amp;id={ps}&amp;lang=de">PS</a></p>"#
            )),
        );

        let (dir, _pages) = download(&mut cms);
        let text = fs::read_to_string(dir.path().join("Links/Links.md")).unwrap();
        assert_eq!(
            text,
            "# Links\n\n[PS](<../Research/Problem Statement/Problem Statement.md>)\n"
        );
    }

    #[test]
    fn keeps_slashes_in_names() {
        let mut cms = FakeCms::new();
        let teaching = cms.add(ROOT, "Teaching", &json!({}));
        cms.add(teaching, "Winter 2026/2027", &json!({}));
        let (_dir, pages) = download(&mut cms);
        let winter = &pages[2].page;
        assert_eq!(
            winter.relative_path,
            "Teaching/Winter 2026∕2027/Winter 2026∕2027.md"
        );
        assert_eq!(winter.name, "Winter 2026/2027");
    }

    #[test]
    fn skips_pages_markdown_cant_represent_with_everything_below() {
        let mut cms = FakeCms::new();
        let people = cms.add(ROOT, "People", &json!({}));
        cms.set_type(people, "people/list");
        cms.add(people, "Michael Kuhn", &json!({}));
        cms.add(
            ROOT,
            "Gallery",
            &json!({ "_blocks": { "center": ["text", "images"] } }),
        );
        cms.add(
            ROOT,
            "Research",
            &json!({ "_blocks": { "center": ["text"] } }),
        );

        let (dir, pages) = download(&mut cms);
        assert_eq!(paths(&pages), ["Home.md", "Research/Research.md"]);
        assert!(!dir.path().join("People").exists());
    }

    #[test]
    fn refuses_a_home_page_with_more_than_text() {
        let mut cms = FakeCms::new();
        let home = cms.add(
            ROOT,
            "Start",
            &json!({ "_blocks": { "center": ["slider", "text"] } }),
        );
        let dir = tempfile::tempdir().unwrap();
        let types = ["page"];
        let from = Source {
            home_id: home,
            ..source(&types)
        };
        assert!(download_pages(&mut cms, &from, dir.path()).is_err());
    }
}
