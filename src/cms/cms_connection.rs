use crate::cms::cms_client::SiteClient;
use crate::settings::config::Config;
use crate::settings::credentials::Credentials;
use anyhow::{Context, bail};
use reqwest::blocking::multipart::{Form, Part};
use reqwest::blocking::{Client, RequestBuilder};
use serde_json::Value;
use std::time::Duration;

/// A logged-in session. It is closed automatically when dropped.
pub struct CmsConnection {
    pub(super) client: Client,
    rest_url: String,
}

impl CmsConnection {
    pub fn open(config: &Config, credentials: &Credentials) -> anyhow::Result<Self> {
        let client = Client::builder()
            .cookie_store(true)
            .danger_accept_invalid_certs(config.accept_invalid_certs)
            // Deleting a page with many subpages can take a while.
            .timeout(Duration::from_secs(300))
            .build()?;
        let connection = Self {
            client,
            rest_url: config.rest_url(),
        };

        let request = connection
            .client
            .put(format!("{}startSession", connection.rest_url))
            .query(&[
                ("user_id", &credentials.user_id),
                ("token", &credentials.token),
            ]);
        Self::send(request).context("Logging in failed. Are the user id and token correct?")?;
        Ok(connection)
    }

    /// A client for one site (e.g. `materialkit`) in one language.
    pub fn site(&self, site: &str, language: &str) -> SiteClient<'_> {
        SiteClient {
            cms_connection: self,
            base: format!("{}{site}/{language}/", self.rest_url),
            language: language.to_string(),
        }
    }

    /// Sends a request and returns the JSON body (`Null` for an empty one).
    ///
    /// EgoCMS reports many failures with a success status, so the body is checked as well.
    pub(super) fn send(request: RequestBuilder) -> anyhow::Result<Value> {
        let response = request.send()?;
        let status = response.status();
        let url = response.url().path().to_string();
        let text = response.text()?;

        if !status.is_success() {
            bail!("{url} failed with {status}: {text}");
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        let value: Value = serde_json::from_str(&text)
            .with_context(|| format!("{url} returned invalid JSON: {text}"))?;
        if let Some(error) = value.get("error") {
            bail!("{url} failed: {error}");
        }
        Ok(value)
    }

    /// The multipart body EgoCMS expects for uploads: `@data` holds the arguments of the PHP method,
    /// and the argument `"@file"` is replaced by the uploaded file.
    pub(super) fn upload_form(arguments: &Value, name: &str, bytes: &[u8]) -> Form {
        Form::new().text("@data", arguments.to_string()).part(
            "upload",
            Part::bytes(bytes.to_vec()).file_name(name.to_string()),
        )
    }
}

impl Drop for CmsConnection {
    fn drop(&mut self) {
        let request = self.client.put(format!("{}closeSession", self.rest_url));
        if let Err(error) = Self::send(request) {
            eprintln!("Closing the session failed: {error:#}");
        }
    }
}
