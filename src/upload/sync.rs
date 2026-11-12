//! Brings the CMS in line with the local files, writing only what changed.
//!
//! Every page created from a file carries a marker (see [`marker`]) with the file's path and the
//! hashes of the files attached to it. That is how pages are found again, and how changed
//! attachments are detected without downloading them. Pages without a marker are never touched.
//!
//! The upload runs in four steps, so a page never links to something that doesn't exist yet:
//! 1. Make sure every page exists, so all pages have an id.
//! 2. Attach new and changed files to their pages (all of them, as pages may link to the files of others).
//! 3. Update the content of the pages that changed.
//! 4. Delete files that are gone locally, then pages whose file is gone.

use super::links::{Links, Target, pool_name, rewrite_links};
use crate::cms::MARKER_KEY;
use crate::cms::cms_page::CmsPage;
use crate::cms::cms_trait::{CmsApi, marker, marker_extra, page_link, pool_link, set_content};
use crate::local::local_page::{LocalPage, parent_of};
use crate::local::local_state::LocalState;
use anyhow::{Context, Result, bail};
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
    /// Existing pages of other types (e.g. people pages) are never taken over.
    pub page_types: &'a [&'a str],
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
    for page in &local.pages {
        links.check(page)?;
    }

    let mut report = Report::default();
    let pages = ensure_pages_exist(cms, local, to, &mut report)?;
    let id = |page: &LocalPage| pages.by_path[page.relative_path.as_str()].id;

    let mut stale_files = Vec::new();
    let mut pool_hashes = Vec::new();
    for local_page in &local.pages {
        let page = &pages.by_path[local_page.relative_path.as_str()];
        pool_hashes.push(attach_files(
            cms,
            local_page,
            page,
            &mut stale_files,
            &mut report,
        )?);
    }

    for (local_page, hashes) in local.pages.iter().zip(&pool_hashes) {
        let html = rewrite_links(
            &local_page.markdown,
            &local_page.relative_path,
            &mut |path| {
                Ok(match links.target(path)? {
                    Target::Page(target) => page_link(to.site, to.language, id(target)),
                    Target::File { owner, file } => {
                        pool_link(to.site, to.language, id(owner), pool_name(file))
                    }
                })
            },
        )?;
        let page = &pages.by_path[local_page.relative_path.as_str()];
        let is_new = pages.created.contains(local_page.relative_path.as_str());
        update_content(cms, local_page, page, &html, hashes, is_new, &mut report)?;
    }

    for (id, name, path) in stale_files {
        cms.pool_delete(id, &name)?;
        println!("- removed {name} from {path}");
        report.files_deleted += 1;
    }
    delete_orphans(cms, &pages.orphans, to.home_page, &mut report)?;
    Ok(report)
}

/// Step 1: finds the CMS page of every local page, adopting or creating it if necessary.
fn ensure_pages_exist<'a>(
    cms: &mut dyn CmsApi,
    local: &'a LocalState,
    to: &Destination,
    report: &mut Report,
) -> Result<Pages<'a>> {
    let home = cms.page(to.home_id)?;
    if home.source().is_none() && !home.holds_only_text() {
        bail!(
            "The home page {} (id {}) contains more than a single text block, uploading would replace it.",
            home.name,
            home.id
        );
    }
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
                if let Some(page) = adopt(cms, parent_id, local_page, to.page_types, &mut children)?
                {
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

/// Finds an existing unmarked page with the name of `local_page` below `parent_id`, e.g. one made
/// before this tool was used and then downloaded. It is taken over instead of creating a duplicate.
/// Fails if that page can't be represented as markdown, as uploading would destroy its content.
fn adopt(
    cms: &mut dyn CmsApi,
    parent_id: u64,
    local_page: &LocalPage,
    page_types: &[&str],
    children: &mut HashMap<u64, Vec<CmsPage>>,
) -> Result<Option<CmsPage>> {
    let siblings = match children.entry(parent_id) {
        Entry::Occupied(entry) => entry.into_mut(),
        Entry::Vacant(entry) => entry.insert(cms.children(parent_id)?),
    };
    let Some(index) = siblings
        .iter()
        .position(|page| page.name == local_page.name && page.source().is_none())
    else {
        return Ok(None);
    };
    if let Some(reason) = siblings[index].not_representable(page_types) {
        bail!(
            "{}: there already is a page {} (id {}) {reason}, which markdown can't represent. Rename the file or delete the page.",
            local_page.relative_path,
            local_page.name,
            siblings[index].id
        );
    }
    // Remove it, so it can't be adopted twice.
    Ok(Some(siblings.remove(index)))
}

/// Step 2: attaches the page's new and changed files and returns the hashes of all of them.
/// Files that are gone locally are only collected in `stale_files`: they are deleted once
/// the content stopped linking to them.
fn attach_files<'a>(
    cms: &mut dyn CmsApi,
    local_page: &'a LocalPage,
    page: &CmsPage,
    stale_files: &mut Vec<(u64, String, &'a str)>,
    report: &mut Report,
) -> Result<BTreeMap<String, String>> {
    let files: BTreeMap<&str, &[u8]> = local_page
        .files
        .iter()
        .map(|file| (pool_name(file), file.bytes.as_slice()))
        .collect();
    let hashes: BTreeMap<String, String> = files
        .iter()
        .map(|(name, bytes)| ((*name).to_string(), sha256(bytes)))
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
            cms.pool_put(page.id, name, files[name.as_str()])?;
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
        let parent_deleted =
            parent_of(source, home_page).is_some_and(|parent| sources.contains(parent.as_str()));
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
    use crate::local::local_file::LocalFile;
    use serde_json::json;

    const TO: Destination = Destination {
        site: "parcio",
        language: "de",
        home_id: ROOT,
        home_page: "Home.md",
        page_type: "page",
        page_types: &["page"],
    };

    fn page(path: &str, content: &str, files: &[(&str, &[u8])]) -> LocalPage {
        let mut page = LocalPage::parse(path, content, "Home.md").unwrap();
        page.files = files
            .iter()
            .map(|(path, bytes)| LocalFile {
                relative_path: (*path).into(),
                bytes: bytes.to_vec(),
            })
            .collect();
        page
    }

    /// Home, Teaching (with slides) and a lecture below it (with a plot).
    fn site() -> LocalState {
        LocalState {
            pages: vec![
                page(
                    "Home.md",
                    "# Welcome\nSee [teaching](Teaching/Teaching.md).",
                    &[],
                ),
                page(
                    "Teaching/Teaching.md",
                    "# Teaching\n[PSS](PSS/PSS.md), [slides](slides.pdf)",
                    &[("Teaching/slides.pdf", b"slides")],
                ),
                page(
                    "Teaching/PSS/PSS.md",
                    "# PSS\n![plot](plot.png)",
                    &[("Teaching/PSS/plot.png", b"png")],
                ),
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
        assert_eq!(cms.get(pss).source(), Some("Teaching/PSS/PSS.md"));
        assert_eq!(
            cms.get(pss)
                .pool_hashes()
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            ["plot.png"]
        );
    }

    #[test]
    fn attaches_all_files_before_any_page_links_to_them() {
        let mut cms = FakeCms::new();
        upload(&mut cms, &site());
        assert_eq!(
            cms.writes,
            [
                "new_child Teaching",
                "new_child PSS",
                "pool_put Teaching/slides.pdf",
                "pool_put PSS/plot.png",
                "update Home",
                "update Teaching",
                "update PSS",
            ]
        );
    }

    #[test]
    fn links_into_the_pool_of_another_page() {
        let mut cms = FakeCms::new();
        let mut local = site();
        local.pages[0] = page("Home.md", "# Welcome\n[slides](Teaching/slides.pdf)", &[]);
        upload(&mut cms, &local);
        let teaching = cms.find("Teaching").unwrap();
        assert!(cms.get(ROOT).content().unwrap().contains(&format!(
            "pool=slides.pdf&amp;site=parcio&amp;lang=de&amp;id={teaching}"
        )));
    }

    #[test]
    fn attaches_files_nobody_links_to() {
        let mut cms = FakeCms::new();
        let mut local = site();
        local.pages[0] = page("Home.md", "# Welcome", &[("logo.png", b"logo")]);
        upload(&mut cms, &local);
        assert_eq!(cms.pool(ROOT)["logo.png"], b"logo");
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
        local.pages[2].title = "PSS 2026".into();
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
        local.pages[1].files[0].bytes = b"slides v2".to_vec();
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
    fn deletes_removed_files_after_updating_the_content() {
        let mut cms = FakeCms::new();
        upload(&mut cms, &site());
        cms.writes.clear();

        let mut local = site();
        local.pages[1] = page("Teaching/Teaching.md", "# Teaching\n[PSS](PSS/PSS.md)", &[]);
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
            pages: vec![page("Home.md", "# Welcome", &[])],
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
        cms.attach(teaching, "gone.zip", b"zip");

        let report = upload(&mut cms, &site());
        assert_eq!((report.adopted, report.created), (1, 1));
        assert_eq!(cms.get(teaching).source(), Some("Teaching/Teaching.md"));
        // The pool was unknown: the local file is attached again, the other one deleted.
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
        local.pages[2] = page("Teaching/PSS/PSS.md", "# PSS\n[gone](gone.pdf)", &[]);
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

    #[test]
    fn refuses_to_take_over_pages_markdown_cant_represent() {
        let mut cms = FakeCms::new();
        let teaching = cms.add(
            ROOT,
            "Teaching",
            &json!({ "_blocks": { "center": ["text", "gallery"] } }),
        );
        let error = sync(&mut cms, &site(), &TO).unwrap_err().to_string();
        assert!(
            error.contains("Teaching/Teaching.md: there already is a page Teaching"),
            "{error}"
        );
        assert!(
            error.contains("with more than a single text block"),
            "{error}"
        );
        assert_eq!(cms.writes, Vec::<String>::new());
        assert!(cms.exists(teaching));
    }

    #[test]
    fn refuses_to_replace_an_unmarked_home_page_with_more_than_text() {
        let mut cms = FakeCms::new();
        let home = cms.add(
            ROOT,
            "Start",
            &json!({ "_blocks": { "center": ["slider"] } }),
        );
        let to = Destination {
            home_id: home,
            ..TO
        };
        assert!(sync(&mut cms, &site(), &to).is_err());
        assert_eq!(cms.writes, Vec::<String>::new());
    }

    #[test]
    fn adopts_pages_with_slashes_in_their_name() {
        let mut cms = FakeCms::new();
        let teaching = cms.add(ROOT, "Teaching", &json!({}));
        let winter = cms.add(teaching, "Winter 2026/2027", &json!({}));
        let local = LocalState {
            pages: vec![
                page("Home.md", "# Welcome", &[]),
                page("Teaching/Teaching.md", "# Teaching", &[]),
                page(
                    "Teaching/Winter 2026∕2027/Winter 2026∕2027.md",
                    "# Winter",
                    &[],
                ),
            ],
        };
        let report = upload(&mut cms, &local);
        assert_eq!((report.adopted, report.created), (2, 0));
        assert_eq!(cms.get(winter).name, "Winter 2026/2027");
    }
}
