use reqwest::blocking::{Client, Response};
use serde_json::Value;

/// This struct takes care of communications with the EgoCMS.
/// It implements functions that cover required parts of the EgoCMS API.
pub struct Communicator {
    /// The base URL of the rest api we are trying to communicate with. E.g.: https://localhost/rest/
    rest_url: String,
    /// Some API calls are SITE-specific and require the site_url appended to the `rest_url`.
    /// E.g.: https://localhost/rest/materialkit/de
    /// So for this example the site_url should be "materialkit/de/".
    /// This approach fails when multiple languages are supposed to be updated, but that's fine for now.
    site_url: String,
    /// The ID of the user who is authoring the requests. This id can be found in the admin section
    /// of EgoCMS by checking: Verwaltung > Rollen > [Click on the username] > Bottom right corner.
    user_id: String,
    /// Can be set per user in the admin section like above.
    user_token: String,
    /// The JSON which defines a EgoCMS page has one section that is relevant to us. This path defines which section that is.
    client: Client,
}

// Automatically close the connection when the Communicator gets dropped.
impl Drop for Communicator {
    fn drop(&mut self) {
        // At this point we don't have a clean way of handling potential errors.
        // An alternative would be to have users of this struct call the close manually.
        // However, I think this convenience is worth the slight suboptimal error handling.
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
impl Communicator {
    /// Initializes a new `Communicator` instance.
    ///
    /// # Parameters
    /// - `rest_url`: The base URL for the REST API.
    /// - `site_url`: The base URL for the website or service.
    /// - `user_id`: The user identifier for authentication purposes.
    /// - `user_token`: The token used for authenticating the user's session.
    ///
    /// # Returns
    /// - `Ok(Communicator)` if the initialization is successful.
    /// - `Err(Error)` if an error occurs while initializing or starting the session.
    pub fn new(
        rest_url: String,
        site_url: String,
        user_id: String,
        user_token: String,
        is_test_environment: bool,
    ) -> anyhow::Result<Self> {
        let mut client = Client::builder();

        if is_test_environment {
            client = client.tls_danger_accept_invalid_certs(true);
        }

        let client = client.cookie_store(true).build()?;

        let communicator = Self {
            rest_url,
            site_url,
            user_id,
            user_token,
            client,
        };

        communicator.start_session()?;

        Ok(communicator)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~
    // PUT Functions
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~
    /// This fully replaces the contents of the extra part of the page!
    /// It should thus be used by first getting `extra` modifying it, and then updating :)
    /// https://hilfe.egocms.com/entwicklung/klassen-_-funktionen/page/updateextra
    pub fn update_extra(&self, id: &str, new_extra: &Value) -> anyhow::Result<Response> {
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

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~
    // GET Functions
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~
    /// This needs a page id to get a page's information, like it content etc.
    /// https://hilfe.egocms.com/entwicklung/klassen-_-funktionen/site/getpage
    pub fn get_page(&self, id: &str) -> anyhow::Result<Response> {
        let get_extra_url = format!("{}{}{}", self.rest_url, self.site_url, "getPage");
        let params = vec![("id", id)];

        let result = self
            .client
            .get(get_extra_url)
            .query(&params)
            .send()?
            .error_for_status()?;
        Ok(result)
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
    fn start_session(&self) -> anyhow::Result<Response> {
        let start_session_url = format!("{}{}", self.rest_url, "startSession");
        let params = vec![
            ("user_id", self.user_id.as_str()),
            ("token", self.user_token.as_str()),
        ];

        let result = self
            .client
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
