use std::env;

use crate::{Error, WALNUT_API_KEY_ENV_VAR, WALNUT_API_URL, WALNUT_API_URL_ENV_VAR};

pub fn walnut_get_api_key() -> Result<String, Error> {
    env::var(WALNUT_API_KEY_ENV_VAR).map_err(|_| Error::MissingApiKey)
}

pub fn walnut_get_api_url() -> String {
    env::var(WALNUT_API_URL_ENV_VAR).unwrap_or_else(|_| WALNUT_API_URL.to_string())
}
