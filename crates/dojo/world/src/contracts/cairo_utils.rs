use anyhow::{anyhow, Result};
use starknet::core::types::Felt;
use starknet::core::utils::cairo_short_string_to_felt;

pub fn str_to_felt(string: &str) -> Result<Felt> {
    cairo_short_string_to_felt(string).map_err(|e| {
        anyhow!(format!("Failed to convert string `{}` to cairo short string: {}", string, e))
    })
}
