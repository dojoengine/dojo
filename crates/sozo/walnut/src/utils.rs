use std::env;

use crate::{WALNUT_API_URL, WALNUT_API_URL_ENV_VAR};

pub fn walnut_get_api_url() -> String {
    env::var(WALNUT_API_URL_ENV_VAR).unwrap_or_else(|_| WALNUT_API_URL.to_string())
}
