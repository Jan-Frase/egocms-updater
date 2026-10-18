use reqwest::blocking::multipart::{Form, Part};
use reqwest::blocking::{Client, Response};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use anyhow::bail;

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
    /// Caches the result of `get_page` for each id. Useful to avoid unneeded requests.
    page_cache: HashMap<u64, Value>,
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
// This should be split into two, one very thin wrapper around the api calls, one helper struct.
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

        let client = client.cookie_store(true).build()?;

        let page_cache = HashMap::new();

        Self::start_session(&rest_url, user_id, user_token, &client)?;

        let communicator = Self {
            rest_url,
            site_url,
            client,
            page_cache,
        };

        Ok(communicator)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~
    // POST/PUT Functions
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~
    /// Creates a new child-page.
    /// https://hilfe.egocms.com/entwicklung/klassen-_-funktionen/page/newchild
    /// * `id` - The id of the parent.
    /// * `title` - Shown on the page.
    /// * `parent_extra` - The extra of the parent, will be copied.
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

        // Get the parents extra.
        let parent_extra: Value = self.get_extra(parent_id)?;

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
        // I copy the extra of the parent to avoid the new child lacking some entries I expect later on when pushing the .md to the page.
        wrapped_json.insert("extra".into(), parent_extra);

        let result = self
            .client
            .post(new_child_url)
            .json(&wrapped_json)
            .send()?
            .error_for_status()?;

        Ok(result)
    }

    /// Uploads a new file to the media site and returns the id of the created file page.
    /// Files live on the media site (e.g. `materialkit_media/de/`), NOT on the content site:
    /// on a content site EgoCMS silently does nothing but still answers with 201.
    /// * `media_site_url` - E.g. "materialkit_media/de/".
    /// * `parent_id` - The id of the media folder the file is created in.
    pub fn new_file(
        &self,
        media_site_url: &str,
        parent_id: u64,
        bytes: Vec<u8>,
        name: &str,
    ) -> anyhow::Result<u64> {
        let url = format!("{}{}{}/newFile", self.rest_url, media_site_url, parent_id);
        let form = Form::new()
            .text("@data", json!(["@file", name, {"title": name}]).to_string())
            .part("upload", Part::bytes(bytes).file_name(name.to_string()));

        let result: Value = self
            .client
            .post(url)
            .multipart(form)
            .send()?
            .error_for_status()?
            .json()?;

        // EgoCMS answers 201 even if it refused the file, in which case the body is empty/null.
        result
            .pointer("/field/id")
            .and_then(Value::as_u64)
            .ok_or_else(|| anyhow::anyhow!("Error creating file {name}. EgoCMS returned: {result}"))
    }

    /// This fully replaces the contents of the extra part of the page!
    /// It should thus be used by first getting `extra` modifying it, and then updating :)
    /// https://hilfe.egocms.com/entwicklung/klassen-_-funktionen/page/updateextra
    pub fn update_extra(&self, id: u64, new_extra: &Value) -> anyhow::Result<Response> {
        let update_extra_url =
            format!("{}{}{}{}", self.rest_url, self.site_url, id, "/updateExtra");

        let result = self
            .client
            .put(update_extra_url)
            .json(new_extra)
            .send()?
            .error_for_status()?;
        Ok(result)
    }

    pub fn update_field(&self, id: u64, field: &Value) -> anyhow::Result<Response> {
        let update_extra_url =
            format!("{}{}{}{}", self.rest_url, self.site_url, id, "/updateField");

        let result = self
            .client
            .put(update_extra_url)
            .json(&field)
            .send()?
            .error_for_status()?;
        Ok(result)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~
    // GET Functions
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~
    pub fn get_media_url(&mut self) -> anyhow::Result<Value> {
        let rest_url = &self.rest_url;
        let site_url = &self.site_url;
        let method_name = "/getMediaUrl";
        let get_extra_url = format!("{rest_url}{site_url}{method_name}");

        let json = self
            .client
            .get(get_extra_url)
            .send()?
            .error_for_status()?
            .json()?;

        println!("{json}");

        Ok(json)
    }

    /// This needs a page id to get a page's information, like it content etc.
    /// https://hilfe.egocms.com/entwicklung/klassen-_-funktionen/site/getpage
    pub fn get_extra(&mut self, id: u64) -> anyhow::Result<Value> {
        let mut result = self.get_page(id)?;

        // We are only interested in the `extra` section.
        let extra = result
            .as_object_mut()
            .and_then(|obj| obj.remove("extra"))
            .ok_or_else(|| anyhow::anyhow!("Missing 'extra' key!"))?;

        Ok(extra)
    }

    /// Gets the name of a page.
    pub fn get_field(&mut self, id: u64) -> anyhow::Result<Value> {
        let mut result = self.get_page(id)?;

        let name = result
            .as_object_mut()
            .and_then(|obj| obj.remove("field"))
            .ok_or_else(|| anyhow::anyhow!("Mising `field` entry."))?;

        Ok(name)
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

    fn get_page(&mut self, id: u64) -> anyhow::Result<Value> {
        let result = match self.page_cache.entry(id) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.get().clone(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                let get_extra_url = format!("{}{}{}", self.rest_url, self.site_url, "getPage");
                let params = vec![("id", id)];

                let result: Value = self
                    .client
                    .get(get_extra_url)
                    .query(&params)
                    .send()?
                    .error_for_status()?
                    .json()?;
                entry.insert(result).clone()
            }
        };

        Ok(result)
    }
}
