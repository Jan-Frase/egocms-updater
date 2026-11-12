//! Downloads an existing site once, to start using this tool on it.

pub mod file_download;
pub mod page_download;

use crate::cms::cms_connection::CmsConnection;
use crate::cms::cms_trait::CmsApi;
use crate::local::local_state::LocalState;
use crate::settings::config::Config;
use crate::settings::credentials::Credentials;
use anyhow::{Context, Result, bail};
use file_download::download_files;
use page_download::{Source, download_pages};
use std::path::Path;

pub fn download(config: &Config, credentials: &Credentials, out_dir: &Path) -> Result<()> {
    // 1. Refuse to overwrite anything before touching the CMS.
    if out_dir.exists() && out_dir.read_dir()?.next().is_some() {
        bail!("{} is not empty.", out_dir.display());
    }

    // 2. Open a connection to the CMS.
    let cms_connection = CmsConnection::open(config, credentials)?;
    let mut site_client = cms_connection.site(&config.site, &config.language);

    // 3. Get the home_id.
    let home_id = match config.home_page_id {
        Some(id) => id,
        None => site_client.root_id()?,
    };

    // 4. Download the pages and the files attached to them.
    let page_types: Vec<&str> = config.page_types.iter().map(String::as_str).collect();
    let from = Source {
        site: &config.site,
        home_id,
        home_page: &config.home_page,
        page_types: &page_types,
    };
    let pages = download_pages(&mut site_client, &from, out_dir)?;
    let files = download_files(&mut site_client, &pages, out_dir)?;

    // 5. Make sure the result can be uploaded as it is.
    LocalState::new(&out_dir.to_path_buf(), &config.home_page).with_context(|| {
        format!(
            "The downloaded pages in {} can't be uploaded as they are.",
            out_dir.display()
        )
    })?;
    println!(
        "\nDownloaded {} pages and {files} files into {}.",
        pages.len(),
        out_dir.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cms::mock_client::{FakeCms, ROOT};
    use crate::upload::sync::{Destination, sync};
    use serde_json::json;

    /// Downloading a site and uploading the result must keep every page and file in place.
    #[test]
    fn a_downloaded_site_can_be_uploaded_again() {
        let mut cms = FakeCms::new();
        let research = cms.add(ROOT, "Research", &json!({}));
        let smash = cms.add(research, "SMASH", &json!({}));
        let links = format!(
            r#"<p><a href="index.php?site=parcio&amp;id={smash}&amp;lang=de">SMASH</a>
               <a href="index.php?pool=PSS+Intro.pdf&amp;site=parcio&amp;lang=de&amp;id={smash}">slides</a></p>"#
        );
        cms.add(
            ROOT,
            "Teaching",
            &json!({ "_contents": { "center": [{ "content1": links }] } }),
        );
        cms.attach(smash, "PSS Intro.pdf", b"slides");

        let dir = tempfile::tempdir().unwrap();
        let page_types = ["page"];
        let from = Source {
            site: "parcio",
            home_id: ROOT,
            home_page: "Home.md",
            page_types: &page_types,
        };
        let pages = download_pages(&mut cms, &from, dir.path()).unwrap();
        download_files(&mut cms, &pages, dir.path()).unwrap();
        let local = LocalState::new(&dir.path().to_path_buf(), "Home.md").unwrap();

        let to = Destination {
            site: "parcio",
            language: "de",
            home_id: ROOT,
            home_page: "Home.md",
            page_type: "page",
            page_types: &page_types,
        };
        let report = sync(&mut cms, &local, &to).unwrap();
        assert_eq!((report.created, report.deleted, report.adopted), (0, 0, 3));
        assert_eq!(
            cms.pool(smash).keys().collect::<Vec<_>>(),
            ["PSS Intro.pdf"]
        );
        let teaching = cms.get(cms.find("Teaching").unwrap()).content().unwrap();
        assert!(
            teaching.contains(&format!("index.php?site=parcio&amp;id={smash}&amp;lang=de")),
            "{teaching}"
        );
        assert!(
            teaching.contains(&format!(
                "pool=PSS+Intro.pdf&amp;site=parcio&amp;lang=de&amp;id={smash}"
            )),
            "{teaching}"
        );

        // From now on, the pages are known and nothing changes.
        cms.writes.clear();
        let report = sync(&mut cms, &local, &to).unwrap();
        assert_eq!(report.unchanged, 4);
        assert_eq!(cms.writes, Vec::<String>::new());
    }
}
