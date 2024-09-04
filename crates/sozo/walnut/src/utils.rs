use anyhow::{anyhow, Result};
use std::env;

use crate::{WALNUT_API_KEY_ENV_VAR, WALNUT_API_URL, WALNUT_API_URL_ENV_VAR};

pub fn walnut_check_api_key() -> Result<()> {
    env::var(WALNUT_API_KEY_ENV_VAR)
        .map_err(|_| {
            anyhow!(
                "Environment variable '{}' is not set. Please set it to your Walnut API key.",
                WALNUT_API_KEY_ENV_VAR
            )
        })
        .map(|_| ())
}

pub fn walnut_get_api_key() -> Result<String> {
    env::var(WALNUT_API_KEY_ENV_VAR).map_err(|_| {
        anyhow!(
            "Environment variable '{}' is not set. Please set it to your Walnut API key.",
            WALNUT_API_KEY_ENV_VAR
        )
    })
}

pub fn walnut_get_api_url() -> String {
    env::var(WALNUT_API_URL_ENV_VAR).unwrap_or_else(|_| WALNUT_API_URL.to_string())
}
