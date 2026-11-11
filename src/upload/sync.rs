//! Brings the CMS in line with the local files, writing only what changed.
//!
//! Every page created from a file carries a marker (see [`marker`]) with the file's path and the
//! hashes of the files attached to it. That is how pages are found again, and how changed
//! attachments are detected without downloading them. Pages without a marker are never touched.
//!
//! The upload runs in four steps, so a page never links to something that doesn't exist yet:
//! 1. Make sure every page exists, so all pages have an id.
//! 2. Attach new and changed files to their pages.
//! 3. Update the content of the pages that changed.
//! 4. Delete files that are no longer linked, then pages whose file is gone.

use super::links::{Links, Target, pool_name, rewrite_links};
use crate::cms::MARKER_KEY;
use crate::cms::cms_page::CmsPage;
use crate::cms::cms_trait::{CmsApi, marker, marker_extra, page_link, pool_link, set_content};
use crate::local::local_file::LocalFile;
use crate::local::local_page::LocalPage;
use crate::local::local_state::LocalState;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::{self, Write};

/// Where to upload to.
pub struct Destination<'a> {
    pub site: &'a str,
    pub language: &'a str,
    /// The id of the page the home page file is uploaded to.
    pub home_id: u64,
    /// The path of the home page file, e.g. `Home.md`.
    pub home_page: &'a str,
    /// The type of newly created pages.
    pub page_type: &'a str,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub created: usize,
    /// Existing pages without marker that were matched by name and taken over.
    pub adopted: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub deleted: usize,
    pub files_attached: usize,
    pub files_deleted: usize,
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Pages: {} created, {} adopted, {} updated, {} unchanged, {} deleted. Files: {} attached, {} deleted.",
            self.created,
            self.adopted,
            self.updated,
            self.unchanged,
            self.deleted,
            self.files_attached,
            self.files_deleted
        )
    }
}

/// The CMS page of every local page, after step 1.
struct Pages<'a> {
    by_path: HashMap<&'a str, CmsPage>,
    /// Pages created in this run, so their first update isn't reported as a change.
    created: HashSet<&'a str>,
    /// Marked pages whose file is gone.
    orphans: Vec<CmsPage>,
}

/// Uploads `local` (pages ordered parents first, starting with the home page).
pub fn sync(cms: &mut dyn CmsApi, local: &LocalState, to: &Destination) -> Result<Report> {
    // Check all links before touching the CMS, so a broken link changes nothing.
    let links = Links::new(local);
    let pool_files = local
        .pages
        .iter()
        .map(|page| links.pool_files(page))
        .collect::<Result<Vec<_>>>()?;
    for path in unlinked_files(local, &pool_files) {
        eprintln!("Warning: no page links to {path}, so it is not uploaded.");
    }

    let mut report = Report::default();
    let pages = ensure_pages_exist(cms, local, to, &mut report)?;

    let mut stale_files = Vec::new();
    for (local_page, files) in local.pages.iter().zip(&pool_files) {
        let page = &pages.by_path[local_page.relative_path.as_str()];
        let hashes = attach_files(cms, local_page, page, files, &mut stale_files, &mut report)?;

        let html = rewrite_links(
            &local_page.markdown,
            &local_page.relative_path,
            &mut |path| {
                Ok(match links.target(path)? {
                    Target::Page(target) => page_link(
                        to.site,
                        to.language,
                        pages.by_path[target.relative_path.as_str()].id,
                    ),
                    Target::File(file) => pool_link(to.site, to.language, page.id, pool_name(file)),
                })
            },
        )?;
        let is_new = pages.created.contains(local_page.relative_path.as_str());
        update_content(cms, local_page, page, &html, &hashes, is_new, &mut report)?;
    }

    for (id, name, path) in stale_files {
        cms.pool_delete(id, &name)?;
        println!("- removed {name} from {path}");
        report.files_deleted += 1;
    }
    delete_orphans(cms, &pages.orphans, to.home_page, &mut report)?;
    Ok(report)
}

/// The local files no page links to.
fn unlinked_files<'a>(
    local: &'a LocalState,
    pool_files: &[BTreeMap<&str, &LocalFile>],
) -> Vec<&'a str> {
    let linked: HashSet<&str> = pool_files
        .iter()
        .flat_map(|files| files.values().map(|file| file.relative_path.as_str()))
        .collect();
    local
        .files
        .iter()
        .map(|file| file.relative_path.as_str())
        .filter(|path| !linked.contains(path))
        .collect()
}

/// Step 1: finds the CMS page of every local page, adopting or creating it if necessary.
fn ensure_pages_exist<'a>(
    cms: &mut dyn CmsApi,
    local: &'a LocalState,
    to: &Destination,
    report: &mut Report,
) -> Result<Pages<'a>> {
    let home = cms.page(to.home_id)?;
    let mut remote: HashMap<String, CmsPage> = std::iter::once(home.clone())
        .chain(cms.descendants(to.home_id)?)
        .filter_map(|page| Some((page.source()?.to_string(), page)))
        .collect();

    let mut pages = Pages {
        by_path: HashMap::new(),
        created: HashSet::new(),
        orphans: Vec::new(),
    };
    let mut children: HashMap<u64, Vec<CmsPage>> = HashMap::new();
    for local_page in &local.pages {
        let path = local_page.relative_path.as_str();
        let known = remote.remove(path);
        let page = match (&local_page.parent, known) {
            (None, _) => home.clone(),
            (Some(_), Some(page)) => page,
            (Some(parent), None) => {
                let parent_id = pages
                    .by_path
                    .get(parent.as_str())
                    .context("Parents are uploaded before their children.")?
                    .id;
                if let Some(page) = adopt(cms, parent_id, &local_page.name, &mut children)? {
                    println!("= adopted {path} (id {})", page.id);
                    report.adopted += 1;
                    page
                } else {
                    let page = cms.new_child(
                        parent_id,
                        &local_page.name,
                        &local_page.title,
                        to.page_type,
                        &marker_extra(path),
                    )?;
                    println!("+ created {path} (id {})", page.id);
                    // A new page has no children, no need to ask the CMS for them.
                    children.insert(page.id, Vec::new());
                    pages.created.insert(path);
                    report.created += 1;
                    page
                }
            }
        };
        pages.by_path.insert(path, page);
    }
    pages.orphans = remote.into_values().collect();
    Ok(pages)
}

/// Finds an existing unmarked page called `name` below `parent_id`, e.g. one made before this
/// tool was used and then downloaded. It is taken over instead of creating a duplicate.
fn adopt(
    cms: &mut dyn CmsApi,
    parent_id: u64,
    name: &str,
    children: &mut HashMap<u64, Vec<CmsPage>>,
) -> Result<Option<CmsPage>> {
    let siblings = match children.entry(parent_id) {
        Entry::Occupied(entry) => entry.into_mut(),
        Entry::Vacant(entry) => entry.insert(cms.children(parent_id)?),
    };
    let found = siblings
        .iter()
        .position(|page| page.name == name && page.source().is_none());
    // Remove it, so it can't be adopted twice.
    Ok(found.map(|index| siblings.remove(index)))
}

/// Step 2: attaches new and changed files to the page and returns the hashes of all its files.
/// Files that are no longer linked are only collected in `stale_files`: they are deleted once
/// the content stopped linking to them.
fn attach_files<'a>(
    cms: &mut dyn CmsApi,
    local_page: &'a LocalPage,
    page: &CmsPage,
    files: &BTreeMap<&str, &LocalFile>,
    stale_files: &mut Vec<(u64, String, &'a str)>,
    report: &mut Report,
) -> Result<BTreeMap<String, String>> {
    let hashes: BTreeMap<String, String> = files
        .iter()
        .map(|(name, file)| ((*name).to_string(), sha256(&file.bytes)))
        .collect();

    // Adopted pages have no marker yet, so what their pool contains has to be asked for.
    let known = match page.pool_hashes() {
        Some(known) => known,
        None => cms
            .pool_list(page.id)?
            .into_iter()
            .map(|name| (name, String::new()))
            .collect(),
    };

    for (name, hash) in &hashes {
        if known.get(name) != Some(hash) {
            cms.pool_put(page.id, name, &files[name.as_str()].bytes)?;
            println!("+ attached {name} to {}", local_page.relative_path);
            report.files_attached += 1;
        }
    }
    stale_files.extend(
        known
            .into_keys()
            .filter(|name| !hashes.contains_key(name))
            .map(|name| (page.id, name, local_page.relative_path.as_str())),
    );
    Ok(hashes)
}

/// Step 3: writes name, title, content and marker of the page, if any of them changed.
fn update_content(
    cms: &mut dyn CmsApi,
    local_page: &LocalPage,
    page: &CmsPage,
    html: &str,
    pool_hashes: &BTreeMap<String, String>,
    is_new: bool,
    report: &mut Report,
) -> Result<()> {
    // The home page keeps its name, as renaming it would change the URL of every page.
    let name = if local_page.parent.is_none() {
        &page.name
    } else {
        &local_page.name
    };
    let marker = marker(&local_page.relative_path, pool_hashes);
    let up_to_date = page.name == *name
        && page.title == local_page.title
        && page.extra.get(MARKER_KEY) == Some(&marker)
        && page.content().unwrap_or_default() == html;
    if up_to_date {
        report.unchanged += 1;
        return Ok(());
    }

    let mut extra = page.extra.clone();
    extra.insert(MARKER_KEY.into(), marker);
    set_content(&mut extra, html);
    cms.update(page.id, name, &local_page.title, &extra)?;
    if !is_new {
        println!("~ updated {} (id {})", local_page.relative_path, page.id);
        report.updated += 1;
    }
    Ok(())
}

/// Step 4: moves pages whose file is gone to the trash.
/// Deleting a page deletes its subpages, so pages whose parent is deleted, too, are skipped.
fn delete_orphans(
    cms: &mut dyn CmsApi,
    orphans: &[CmsPage],
    home_page: &str,
    report: &mut Report,
) -> Result<()> {
    let sources: HashSet<&str> = orphans.iter().filter_map(CmsPage::source).collect();
    for orphan in orphans {
        let source = orphan.source().unwrap_or_default();
        report.deleted += 1;
        let parent_deleted = LocalPage::parent_of(source, home_page)
            .is_some_and(|parent| sources.contains(parent.as_str()));
        if !parent_deleted {
            cms.delete(orphan.id)?;
            println!("- deleted {source} (id {})", orphan.id);
        }
    }
    Ok(())
}

/// The hash stored in the marker. The prefix stops EgoCMS from turning all-digit hashes into numbers.
fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::from("sha256:"), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cms::mock_client::{FakeCms, ROOT};
    use serde_json::json;

    const TO: Destination = Destination {
        site: "parcio",
        language: "de",
        home_id: ROOT,
        home_page: "Home.md",
        page_type: "page",
    };

    fn page(path: &str, content: &str) -> LocalPage {
        LocalPage::parse(path, content, "Home.md").unwrap()
    }

    fn file(path: &str, bytes: &[u8]) -> LocalFile {
        LocalFile {
            relative_path: path.into(),
            bytes: bytes.to_vec(),
        }
    }

    /// Home, Teaching (with slides) and a lecture below it.
    fn site() -> LocalState {
        LocalState {
            pages: vec![
                page(
                    "Home.md",
                    "# Welcome\nSee [teaching](./Teaching/Teaching.md).",
                ),
                page(
                    "Teaching/Teaching.md",
                    "# Teaching\n[PSS](PSS.md), [slides](slides.pdf)",
                ),
                page("Teaching/PSS.md", "# PSS\n![plot](img/plot.png)"),
            ],
            files: vec![
                file("Teaching/slides.pdf", b"slides"),
                file("Teaching/img/plot.png", b"png"),
            ],
        }
    }

    fn upload(cms: &mut FakeCms, local: &LocalState) -> Report {
        sync(cms, local, &TO).unwrap()
    }

    #[test]
    fn hashes_with_a_prefix() {
        assert_eq!(
            sha256(b"abc"),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn finds_unlinked_files() {
        let mut local = site();
        local.files.push(file("Teaching/old.pdf", b"old"));
        let links = Links::new(&local);
        let pool_files: Vec<_> = local
            .pages
            .iter()
            .map(|page| links.pool_files(page).unwrap())
            .collect();
        assert_eq!(unlinked_files(&local, &pool_files), ["Teaching/old.pdf"]);
    }

    #[test]
    fn uploads_everything_to_an_empty_site() {
        let mut cms = FakeCms::new();
        let report = upload(&mut cms, &site());
        assert_eq!(
            report,
            Report {
                created: 2,
                updated: 1,
                files_attached: 2,
                ..Report::default()
            }
        );

        let teaching = cms.find("Teaching").unwrap();
        let pss = cms.find("PSS").unwrap();
        assert_eq!(
            cms.get(ROOT).content(),
            Some(format!("<p>See <a href=\"index.php?site=parcio&amp;id={teaching}&amp;lang=de\">teaching</a>.</p>\n").as_str())
        );
        assert_eq!(
            cms.get(pss).content(),
            Some(format!("<p><img src=\"index.php?pool=plot.png&amp;site=parcio&amp;lang=de&amp;id={pss}\" alt=\"plot\" /></p>\n").as_str())
        );
        assert_eq!(cms.pool(teaching)["slides.pdf"], b"slides");
        assert_eq!(cms.pool(pss)["plot.png"], b"png");
        assert_eq!(
            cms.get(pss)
                .pool_hashes()
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            ["plot.png"]
        );
        assert_eq!(cms.get(pss).source(), Some("Teaching/PSS.md"));
    }

    #[test]
    fn attaches_files_before_linking_to_them() {
        let mut cms = FakeCms::new();
        upload(&mut cms, &site());
        assert_eq!(
            cms.writes,
            [
                "new_child Teaching",
                "new_child PSS",
                "update Home",
                "pool_put Teaching/slides.pdf",
                "update Teaching",
                "pool_put PSS/plot.png",
                "update PSS",
            ]
        );
    }

    #[test]
    fn a_second_run_writes_nothing() {
        let mut cms = FakeCms::new();
        upload(&mut cms, &site());
        cms.writes.clear();

        let report = upload(&mut cms, &site());
        assert_eq!(
            report,
            Report {
                unchanged: 3,
                ..Report::default()
            }
        );
        assert_eq!(cms.writes, Vec::<String>::new());
    }

    #[test]
    fn updates_only_the_changed_page() {
        let mut cms = FakeCms::new();
        upload(&mut cms, &site());
        cms.writes.clear();

        let mut local = site();
        local.pages[2] = page("Teaching/PSS.md", "# PSS 2026\n![plot](img/plot.png) New.");
        let report = upload(&mut cms, &local);
        assert_eq!((report.updated, report.unchanged), (1, 2));
        assert_eq!(cms.writes, ["update PSS"]);
        assert_eq!(cms.get(cms.find("PSS").unwrap()).title, "PSS 2026");
    }

    #[test]
    fn reattaches_changed_files() {
        let mut cms = FakeCms::new();
        upload(&mut cms, &site());
        cms.writes.clear();

        let mut local = site();
        local.files[0] = file("Teaching/slides.pdf", b"slides v2");
        let report = upload(&mut cms, &local);
        assert_eq!(report.files_attached, 1);
        // The marker with the new hash has to be written, too.
        assert_eq!(
            cms.writes,
            ["pool_put Teaching/slides.pdf", "update Teaching"]
        );
        assert_eq!(
            cms.pool(cms.find("Teaching").unwrap())["slides.pdf"],
            b"slides v2"
        );
    }

    #[test]
    fn deletes_unlinked_files_after_updating_the_content() {
        let mut cms = FakeCms::new();
        upload(&mut cms, &site());
        cms.writes.clear();

        let mut local = site();
        local.pages[1] = page("Teaching/Teaching.md", "# Teaching\n[PSS](PSS.md)");
        let report = upload(&mut cms, &local);
        assert_eq!(report.files_deleted, 1);
        assert_eq!(
            cms.writes,
            ["update Teaching", "pool_delete Teaching/slides.pdf"]
        );
        assert!(cms.pool(cms.find("Teaching").unwrap()).is_empty());
    }

    #[test]
    fn deletes_pages_whose_file_is_gone_with_one_request() {
        let mut cms = FakeCms::new();
        upload(&mut cms, &site());
        let teaching = cms.find("Teaching").unwrap();
        let pss = cms.find("PSS").unwrap();
        cms.writes.clear();

        let local = LocalState {
            pages: vec![page("Home.md", "# Welcome")],
            files: vec![],
        };
        let report = upload(&mut cms, &local);
        assert_eq!(report.deleted, 2);
        assert_eq!(cms.writes, ["update Home", "delete Teaching"]);
        assert!(!cms.exists(teaching));
        assert!(!cms.exists(pss));
    }

    #[test]
    fn never_touches_unmarked_pages() {
        let mut cms = FakeCms::new();
        let people = cms.add(ROOT, "People", &json!({ "_contents": "custom" }));
        cms.attach(people, "photo.jpg", b"jpg");
        upload(&mut cms, &site());
        assert_eq!(cms.get(people).extra["_contents"], "custom");
        assert_eq!(cms.pool(people).len(), 1);
    }

    #[test]
    fn adopts_existing_pages_and_cleans_up_their_pool() {
        let mut cms = FakeCms::new();
        let teaching = cms.add(ROOT, "Teaching", &json!({}));
        cms.attach(teaching, "slides.pdf", b"old slides");
        cms.attach(teaching, "unlinked.zip", b"zip");

        let report = upload(&mut cms, &site());
        assert_eq!((report.adopted, report.created), (1, 1));
        assert_eq!(cms.get(teaching).source(), Some("Teaching/Teaching.md"));
        // The pool was unknown: the linked file is replaced, the unlinked one deleted.
        assert_eq!(
            cms.pool(teaching).keys().collect::<Vec<_>>(),
            ["slides.pdf"]
        );
        assert_eq!(cms.pool(teaching)["slides.pdf"], b"slides");
    }

    #[test]
    fn changes_nothing_if_a_link_is_broken() {
        let mut cms = FakeCms::new();
        let mut local = site();
        local.pages[2] = page("Teaching/PSS.md", "# PSS\n[gone](gone.pdf)");
        let error = sync(&mut cms, &local, &TO).unwrap_err();
        assert!(format!("{error:#}").contains("gone.pdf"), "{error:#}");
        assert_eq!(cms.writes, Vec::<String>::new());
    }

    #[test]
    fn uses_a_configured_home_page_and_keeps_its_name() {
        let mut cms = FakeCms::new();
        let group = cms.add(ROOT, "Group", &json!({}));
        let to = Destination {
            home_id: group,
            ..TO
        };
        let report = sync(&mut cms, &site(), &to).unwrap();
        assert_eq!(report.created, 2);
        assert_eq!(cms.get(group).name, "Group");
        assert_eq!(cms.get(group).title, "Welcome");
        assert_eq!(
            cms.url(cms.find("PSS").unwrap()).unwrap(),
            "/group/teaching/pss"
        );
    }
}
