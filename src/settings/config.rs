use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

/// Just a small struct that allows serde to parse the settings.toml :)
/// All fields here are further documented in the settings.toml file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub url: String,
    pub site: String,
    pub language: String,
    #[serde(default = "default_pages_dir")]
    pub pages_dir: PathBuf,
    #[serde(default = "default_home_page")]
    pub home_page: String,
    pub home_page_id: Option<u64>,
    #[serde(default = "default_page_type")]
    pub page_type: String,
    #[serde(default = "default_media_folder")]
    pub media_folder: String,
    #[serde(default)]
    pub accept_invalid_certs: bool,
}

fn default_pages_dir() -> PathBuf {
    PathBuf::from("./pages")
}

fn default_home_page() -> String {
    "Home.md".into()
}

fn default_page_type() -> String {
    "page".into()
}

fn default_media_folder() -> String {
    "egocms-updater".into()
}

impl Config {
    pub fn new(config_path: &PathBuf) -> Result<Self> {
        let text = fs::read_to_string(config_path).context(format!(
            "Failed to read config at {}.",
            config_path.display()
        ))?;
        Self::parse(&text)
    }

    fn parse(text: &str) -> Result<Self> {
        let mut config: Self = toml::from_str(text)?;
        config.url = config.url.trim_end_matches('/').to_string();
        if !config.url.starts_with("http://") && !config.url.starts_with("https://") {
            bail!(
                "`url` must start with http:// or https://, got `{}`.",
                config.url
            );
        }
        if config.home_page.strip_suffix(".md").is_none() {
            bail!(
                "`home_page` must be a .md file, got `{}`.",
                config.home_page
            );
        }
        // The home page owns the top level of the pages directory.
        if config.home_page.contains('/') {
            bail!(
                "`home_page` must lie directly in the pages directory, got `{}`.",
                config.home_page
            );
        }
        Ok(config)
    }

    #[must_use]
    pub fn rest_url(&self) -> String {
        format!("{}/rest/", self.url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_defaults() {
        let config =
            Config::parse("url = \"https://example.org/\"\nsite = \"s\"\nlanguage = \"de\"")
                .unwrap();
        assert_eq!(config.url, "https://example.org");
        assert_eq!(config.rest_url(), "https://example.org/rest/");
        assert_eq!(config.pages_dir, PathBuf::from("./pages"));
        assert_eq!(config.home_page, "Home.md");
        assert_eq!(config.home_page_id, None);
        assert_eq!(config.page_type, "page");
        assert_eq!(config.media_folder, "egocms-updater");
        assert!(!config.accept_invalid_certs);
    }

    #[test]
    fn reads_all_fields() {
        let config = Config::parse(
            r#"
            url = "http://localhost"
            site = "materialkit"
            language = "en"
            pages_dir = "content"
            home_page = "Index.md"
            home_page_id = 56
            page_type = "blog/entry"
            media_folder = "files"
            accept_invalid_certs = true
            "#,
        )
        .unwrap();
        assert_eq!(config.site, "materialkit");
        assert_eq!(config.language, "en");
        assert_eq!(config.pages_dir, PathBuf::from("content"));
        assert_eq!(config.home_page, "Index.md");
        assert_eq!(config.home_page_id, Some(56));
        assert_eq!(config.page_type, "blog/entry");
        assert_eq!(config.media_folder, "files");
        assert!(config.accept_invalid_certs);
    }

    #[test]
    fn rejects_invalid_configs() {
        // Missing site.
        assert!(Config::parse("url = \"https://x\"\nlanguage = \"de\"").is_err());
        // Unknown (e.g. misspelled) field.
        assert!(
            Config::parse("url = \"https://x\"\nsite = \"s\"\nlanguage = \"de\"\nhome = \"x\"")
                .is_err()
        );
        // URL without scheme.
        assert!(Config::parse("url = \"localhost\"\nsite = \"s\"\nlanguage = \"de\"").is_err());
        // Home page with an uppercase extension, which would be rejected when reading the pages.
        assert!(
            Config::parse(
                "url = \"https://x\"\nsite = \"s\"\nlanguage = \"de\"\nhome_page = \"Home.MD\""
            )
            .is_err()
        );
        // Home page in a subdirectory.
        assert!(
            Config::parse(
                "url = \"https://x\"\nsite = \"s\"\nlanguage = \"de\"\nhome_page = \"start/start.md\""
            )
            .is_err()
        );
        // Home page that is not markdown.
        assert!(
            Config::parse(
                "url = \"https://x\"\nsite = \"s\"\nlanguage = \"de\"\nhome_page = \"Home\""
            )
            .is_err()
        );
    }
}
