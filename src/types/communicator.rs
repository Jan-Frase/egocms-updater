use anyhow::bail;
use reqwest::StatusCode;
use reqwest::blocking::{Client, Response};
use serde::Serialize;
use serde_json::{Value, json};
use std::time::Duration;

/// This struct takes care of communications with the EgoCMS.
/// It implements functions that cover required parts of the EgoCMS API.
pub struct Communicator {
    /// The base URL of the rest api we are trying to communicate with. E.g.: https://localhost/rest/
    pub rest_url: String,
    /// Some API calls are SITE-specific and require the site_url appended to the `rest_url`.
    /// E.g.: https://localhost/rest/materialkit/de
    /// So for this example the site_url should be "materialkit/de/".
    /// This approach fails when multiple languages are supposed to be updated, but that's fine for now.
    pub site_url: String,
    /// The JSON, which defines an EgoCMS page, has one section that is relevant to us. This path defines which section that is.
    client: Client,
}

#[derive(Serialize)]
struct NewChildParameters {
    name: String,
    title: String,
    #[serde(rename = "type")]
    site_type: String,
    inactive: u64,
    nav_hide: u64,
}

// Automatically close the connection when the Communicator gets dropped.
impl Drop for Communicator {
    fn drop(&mut self) {
        // At this point we don't have a clean way of handling potential errors.
        // An alternative would be to have users of this struct call the close manually.
        // However, I think this convenience is worth the slightly suboptimal error handling.
        let result = self.close_session();

        match result {
            Ok(_) => {}
            Err(err) => eprintln!("Error whilst closing the connection: {err}"),
        }
    }
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~
// Public Functions
// ~~~~~~~~~~~~~~~~~~~~~~~~~~
// TODO: These functions return reqwest data types. It might be cleaner to parse the responses into structs or similar first.
impl Communicator {
    /// Initializes a new `Communicator` instance.
    ///
    /// # Parameters
    /// - `rest_url`: The base URL for the REST API.
    /// - `site_url`: The base URL for the website or service.
    /// - `user_id`:  The ID of the user who is authoring the requests. This id can be found in the admin section of EgoCMS by checking: Verwaltung > Rollen > [Click on the username] > Bottom right corner.
    /// - `user_token`: The token used for authenticating the user's session. Can be set per user in the admin section like above.
    ///
    /// # Returns
    /// - `Ok(Communicator)` if the initialization is successful.
    /// - `Err(Error)` if an error occurs while initializing or starting the session.
    pub fn new(
        rest_url: String,
        site_url: String,
        user_id: &str,
        user_token: &str,
        is_test_environment: bool,
    ) -> anyhow::Result<Self> {
        let mut client = Client::builder();

        if is_test_environment {
            client = client.tls_danger_accept_invalid_certs(true);
        }

        // Deleting a page with many subpages can take well over the default 30 seconds.
        let client = client
            .cookie_store(true)
            .timeout(Duration::from_secs(600))
            .build()?;

        Self::start_session(&rest_url, user_id, user_token, &client)?;

        let communicator = Self {
            rest_url,
            site_url,
            client,
        };

        Ok(communicator)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~
    // PUT Functions
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~
    /// Creates a new child-page.
    /// https://hilfe.egocms.com/entwicklung/klassen-_-funktionen/page/newchild
    /// * `id` - The id of the parent.
    /// * `title` - Shown on the page.
    pub fn new_child(
        &mut self,
        parent_id: u64,
        name: &str,
        title: &str,
    ) -> anyhow::Result<Response> {
        let new_child_url = format!(
            "{}{}{}{}",
            self.rest_url, self.site_url, parent_id, "/newChild"
        );

        // Generate the parameters.
        let new_child_parameters = NewChildParameters {
            name: name.to_string(),
            title: title.to_string(),
            site_type: "blog/entry".to_string(),
            inactive: 0,
            nav_hide: 0,
        };
        let json = serde_json::to_value(&new_child_parameters)?;

        let mut wrapped_json = serde_json::Map::new();
        wrapped_json.insert("field".into(), json);

        let result = self
            .client
            .post(new_child_url)
            .json(&wrapped_json)
            .send()?
            .error_for_status()?;

        Ok(result)
    }

    /// Updates `field` and `extra` of a page in one request.
    /// `field` only needs to contain the entries that change, `extra` is replaced completely!
    /// https://hilfe.egocms.com/entwicklung/klassen-_-funktionen/page/update
    pub fn update_page(&self, id: u64, field: &Value, extra: &Value) -> anyhow::Result<Response> {
        let update_url = format!("{}{}{}{}", self.rest_url, self.site_url, id, "/update");
        // The keys of the body are the parameter names of `Page::update($param)`.
        let body = json!({ "param": { "field": field, "extra": extra } });

        let result = self
            .client
            .put(update_url)
            .json(&body)
            .send()?
            .error_for_status()?;
        Ok(result)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~
    // DELETE Functions
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~
    /// Moves a page AND all of its subpages to the trash.
    /// Returns `false` if the page could not be found, e.g. because it was already deleted together with its parent.
    pub fn delete_page(&self, id: u64) -> anyhow::Result<bool> {
        let url = format!("{}{}{}{}", self.rest_url, self.site_url, id, "/delete");
        let response = self.client.delete(url).send()?;

        // EgoCMS can't resolve pages in the trash and answers with 400.
        // Note: it uses 400 for every failed DELETE, so callers have to verify the result themselves.
        if response.status() == StatusCode::BAD_REQUEST {
            return Ok(false);
        }
        response.error_for_status()?;
        Ok(true)
    }

    /// Permanently removes everything in the trash of the site.
    pub fn clear_trashcan(&self) -> anyhow::Result<()> {
        let url = format!("{}{}{}", self.rest_url, self.site_url, "clearTrashcan");
        self.client.post(url).send()?.error_for_status()?;
        Ok(())
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~
    // GET Functions
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~
    /// Returns the full page as `{ "field": ..., "extra": ... }`.
    /// https://hilfe.egocms.com/entwicklung/klassen-_-funktionen/site/getpage
    pub fn get_page(&self, id: u64) -> anyhow::Result<Value> {
        let url = format!("{}{}{}", self.rest_url, self.site_url, "getPage");

        let page: Value = self
            .client
            .get(url)
            .query(&[("id", id)])
            .send()?
            .error_for_status()?
            .json()?;
        if page.is_null() {
            bail!("EgoCMS has no page with id {id}.");
        }
        Ok(page)
    }

    /// Returns the id of the root page of the site. It is the only page that can't be deleted.
    pub fn get_root_id(&self) -> anyhow::Result<u64> {
        let url = format!("{}{}{}", self.rest_url, self.site_url, "getRoot");

        let root: Value = self.client.get(url).send()?.error_for_status()?.json()?;
        root.pointer("/field/id")
            .and_then(Value::as_u64)
            .ok_or_else(|| anyhow::anyhow!("The root page has no id: {root}"))
    }

    /// Returns the ids of all pages of the site.
    /// https://hilfe.egocms.com/entwicklung/klassen-_-funktionen/site/getpages
    pub fn get_all_page_ids(&self) -> anyhow::Result<Vec<u64>> {
        let url = format!("{}{}{}", self.rest_url, self.site_url, "getPages");

        let pages: Vec<Value> = self.client.get(url).send()?.error_for_status()?.json()?;
        pages
            .iter()
            .map(|page| {
                page.pointer("/field/id")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| anyhow::anyhow!("A page has no id: {page}"))
            })
            .collect()
    }

    pub fn get_url(&self, id: u64) -> anyhow::Result<String> {
        let get_extra_url = format!("{}{}{}{}", self.rest_url, self.site_url, id, "/getUrl");

        let json = self
            .client
            .get(get_extra_url)
            .send()?
            .error_for_status()?
            .json()?;

        Ok(json)
    }
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~
// Private Functions
// ~~~~~~~~~~~~~~~~~~~~~~~~~~
impl Communicator {
    /// Starts the session by using the relevant API request.
    /// The request then returns a session cookie which we need to send together with all future requests.
    /// The session cookie is automatically stored and appended by the client.
    /// https://hilfe.egocms.com/entwicklung/json_rest-api/erste-schritte
    fn start_session(
        rest_url: &str,
        user_id: &str,
        user_token: &str,
        client: &Client,
    ) -> anyhow::Result<Response> {
        let start_session_url = format!("{}{}", rest_url, "startSession");
        let params = vec![("user_id", user_id), ("token", user_token)];

        let result = client
            .put(start_session_url)
            .query(&params)
            .send()?
            .error_for_status()?;
        Ok(result)
    }

    /// Closes the session.
    /// Is automatically called when the Communicator goes out of scope.
    fn close_session(&self) -> anyhow::Result<Response> {
        let start_session_url = format!("{}{}", self.rest_url, "closeSession");

        let result = self
            .client
            .put(start_session_url)
            .send()?
            .error_for_status()?;
        Ok(result)
    }
}
