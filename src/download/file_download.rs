//! Downloads the files attached to the pages (their pools) into the pages' directories.

use crate::cms::cms_trait::CmsApi;
use crate::download::page_download::DownloadedPage;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Saves the files attached to every page next to the page's markdown file. Returns their number.
pub fn download_files(
    cms: &mut dyn CmsApi,
    pages: &[DownloadedPage],
    out_dir: &Path,
) -> Result<usize> {
    let mut downloaded = 0;
    for DownloadedPage { id, page } in pages {
        for name in cms.pool_list(*id)? {
            if !is_plain_file_name(&name) {
                eprintln!(
                    "Warning: skipping {name} of {}, as it would be misread when uploading.",
                    page.relative_path
                );
                continue;
            }
            let bytes = cms.pool_get(*id, &name)?;
            let path = out_dir.join(page.dir()).join(&name);
            fs::write(&path, bytes).with_context(|| format!("Cannot write {}", path.display()))?;
            println!("Downloaded {}", path.display());
            downloaded += 1;
        }
    }
    Ok(downloaded)
}

/// Whether a pool file can be saved as is: no directories, not hidden (hidden files are ignored when
/// uploading) and no markdown file (which would be read as a page).
fn is_plain_file_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.starts_with('.')
        && !Path::new(name)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cms::mock_client::{FakeCms, ROOT};
    use crate::download::page_download::download_pages;
    use serde_json::json;

    #[test]
    fn saves_the_pools_into_the_page_directories() {
        let mut cms = FakeCms::new();
        let research = cms.add(ROOT, "Research", &json!({}));
        let smash = cms.add(research, "SMASH", &json!({}));
        cms.attach(ROOT, "logo.png", b"logo");
        cms.attach(smash, "PSS-01 Intro.pdf", b"slides");
        cms.attach(smash, "notes.md", b"# would become a page");

        let dir = tempfile::tempdir().unwrap();
        let pages = download_pages(&mut cms, "parcio", ROOT, "Home.md", dir.path()).unwrap();
        assert_eq!(download_files(&mut cms, &pages, dir.path()).unwrap(), 2);

        let read = |path: &str| fs::read(dir.path().join(path)).unwrap();
        assert_eq!(read("logo.png"), b"logo");
        assert_eq!(read("Research/SMASH/PSS-01 Intro.pdf"), b"slides");
        assert!(!dir.path().join("Research/SMASH/notes.md").exists());
    }

    #[test]
    fn recognizes_plain_file_names() {
        assert!(is_plain_file_name("slides.pdf"));
        assert!(is_plain_file_name("PSS-01 Intro.pdf"));
        assert!(!is_plain_file_name(""));
        assert!(!is_plain_file_name("a/b.pdf"));
        assert!(!is_plain_file_name(".hidden"));
        assert!(!is_plain_file_name("notes.md"));
        assert!(!is_plain_file_name("notes.MD"));
    }
}
