use anyhow::{anyhow, Result};
use http::uri::Uri;
use starknet::core::types::FieldElement;
use starknet::core::utils::cairo_short_string_to_felt;

pub fn str_to_felt(string: &str) -> Result<FieldElement> {
    cairo_short_string_to_felt(string).map_err(|e| {
        anyhow!(format!("Failed to convert string `{}` to cairo short string: {}", string, e))
    })
}

pub fn encode_uri(uri: &str) -> Result<Vec<FieldElement>> {
    let parsed: Uri =
        uri.try_into().map_err(|e| anyhow!("Failed to encode URI `{}`: {}", uri, e))?;

    Ok(parsed
        .to_string()
        .chars()
        .collect::<Vec<_>>()
        .chunks(31)
        .map(|chunk| {
            let s: String = chunk.iter().collect();
            cairo_short_string_to_felt(&s)
        })
        .collect::<Result<Vec<_>, _>>()?)
}
