use anyhow::{anyhow, Result};
use http::uri::Uri;
use starknet::core::types::Felt;
use starknet::core::utils::cairo_short_string_to_felt;

pub fn str_to_felt(string: &str) -> Result<Felt> {
    cairo_short_string_to_felt(string).map_err(|e| {
        anyhow!(format!("Failed to convert string `{}` to cairo short string: {}", string, e))
    })
}

pub fn encode_uri(uri: &str) -> Result<cainome::cairo_serde::ByteArray> {
    let parsed: Uri =
        uri.try_into().map_err(|e| anyhow!("Failed to encode URI `{}`: {}", uri, e))?;

    Ok(cainome::cairo_serde::ByteArray::from_string(parsed.to_string().as_str()).unwrap())
}
