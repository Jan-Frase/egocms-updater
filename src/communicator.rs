use reqwest::Error;
use reqwest::blocking::{Client, Response};

/// This struct takes care of communications with the EgoCMS.
/// It implements functions that cover required parts of the EgoCMS API.
pub struct Communicator {
    /// The base URL of the rest api we are trying to communicate with. E.g.: https://localhost/rest/
    rest_url: String,
    /// Some API calls are SITE-specific and require the site_url appended to the rest_url.
    /// E.g.: https://localhost/rest/materialkit/de
    /// So for this example the site_url should be "materialkit/de/".
    /// This approach fails when multiple languages are supposed to be updated, but that's fine for now.
    site_url: String,
    /// The ID of the user who is authoring the requests. This id can be found in the admin section
    /// of EgoCMS by checking: Verwaltung > Rollen > [Click on the user name] > Bottom right corner.
    user_id: String,
    /// Can be set per user in the admin section like above.
    user_token: String,
    /// Used to make requests with, holds a pool of connections internally.
    client: Client,
}

// Small internal helper :)
enum ReqestType {
    Get,
    Put,
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
            Err(err) => println!("Error whilst closing the connection: {}", err),
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
    ) -> Result<Communicator, Error> {
        let client = Client::builder()
            // TODO: This is unacceptable in production! It is however required for testing with localhost.
            .tls_danger_accept_invalid_certs(true)
            .cookie_store(true)
            .build()?;

        let communicator = Communicator {
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
    pub fn update_extra(&self, id: &str) -> Result<Response, Error> {
        let update_extra_url =
            format!("{}{}{}{}", self.rest_url, self.site_url, id, "/updateExtra");

        // PHP's array format is strange but this works.
        // TODO: Remove this hard-coded part.
        let params = vec![("extra[_contents/center/0/content1]", "ugabuga")];

        self.send_request(ReqestType::Put, update_extra_url.as_str(), params.into())
    }

    // https://hilfe.egocms.com/entwicklung/klassen-_-funktionen/page/newchild
    pub fn new_child(&self, parent_id: &str) -> Result<Response, Error> {
        let new_child_url = format!(
            "{}{}{}{}",
            self.rest_url, self.site_url, parent_id, "/newChild"
        );

        let params: Vec<(&str, &str)> = vec![
            ("field[name]", "ChildName"),
            ("field[title]", "ChildTitle"),
            ("fields[type", "page"),
            ("inactive", "0"),
            ("nav_hide", "1"),
        ];

        self.send_request(ReqestType::Put, new_child_url.as_str(), params.into())
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~
    // GET Functions
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~
    /// This needs a page id to get a page's information, like it content etc.
    /// https://hilfe.egocms.com/entwicklung/klassen-_-funktionen/site/getpage
    pub fn get_page(&self, id: &str) -> Result<Response, Error> {
        let get_extra_url = format!("{}{}{}", self.rest_url, self.site_url, "getPage");
        let params = vec![("id", id)];

        self.send_request(ReqestType::Get, get_extra_url.as_str(), params.into())
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
    fn start_session(&self) -> Result<Response, Error> {
        let start_session_url = format!("{}{}", self.rest_url, "startSession");
        let params = vec![
            ("user_id", self.user_id.as_str()),
            ("token", self.user_token.as_str()),
        ];

        self.send_request(ReqestType::Put, start_session_url.as_str(), params.into())
    }

    /// Closes the session.
    /// Is automatically called when the Communicator goes out of scope.
    fn close_session(&self) -> Result<Response, Error> {
        let start_session_url = format!("{}{}", self.rest_url, "closeSession");

        self.send_request(ReqestType::Put, start_session_url.as_str(), None)
    }

    /// Small utility function to avoid typing the same lines all the time :)
    /// Can be used to send a GET or POST request either with or without query params.
    fn send_request(
        &self,
        request_type: ReqestType,
        request_url: &str,
        params: Option<Vec<(&str, &str)>>,
    ) -> Result<Response, Error> {
        let mut builder = match request_type {
            ReqestType::Get => self.client.get(request_url),
            ReqestType::Put => self.client.put(request_url),
        };

        if let Some(params) = params {
            builder = builder.query(&params);
        }

        let result = builder.send()?.error_for_status()?;

        Ok(result)
    }
}
