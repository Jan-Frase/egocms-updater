use anyhow::{Context, Result};
use std::env;

/// Environment variable holding the EgoCMS user id.
pub const USER_ID_VAR: &str = "EGOCMS_USER_ID";
/// Environment variable holding the EgoCMS API token of that user.
pub const TOKEN_VAR: &str = "EGOCMS_TOKEN";

/// The login of the EgoCMS user the tool acts as.
pub struct Credentials {
    pub user_id: String,
    pub token: String,
}

impl Credentials {
    /// Reads the credentials from the environment, so they never end up in the repository.
    pub fn from_env() -> Result<Self> {
        let read = |name: &str| {
            env::var(name)
                .ok()
                .filter(|value| !value.is_empty())
                .with_context(|| format!("The environment variable {name} is not set."))
        };
        Ok(Self {
            user_id: read(USER_ID_VAR)?,
            token: read(TOKEN_VAR)?,
        })
    }
}
