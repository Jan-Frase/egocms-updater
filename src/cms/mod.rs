pub mod cms_page;
pub(crate) mod cms_trait;
pub(crate) mod cms_connection;
pub mod cms_client;
mod mock_client;

/// Key in a page's `extra` where this tool stores its marker.
/// Pages without it were not created by this tool and are never modified or deleted.
pub const MARKER_KEY: &str = "_sync";
