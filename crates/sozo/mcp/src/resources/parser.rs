//! URI parsing utilities for Dojo resources.
#![allow(clippy::comparison_to_empty)]

use rmcp::Error as McpError;
use serde_json::json;

/// Parse contract URI to extract profile and contract name
/// Expected format: dojo://contract/{profile}/{name}/abi
pub fn parse_contract_uri(uri: &str) -> Result<(&str, &str), McpError> {
    let parts: Vec<&str> = uri.split('/').collect();

    if parts.len() != 5
        || parts[0] != "dojo:"
        || parts[1] != ""
        || parts[2] != "contract"
        || parts[4] != "abi"
    {
        return Err(McpError::resource_not_found(
            "invalid_contract_uri",
            Some(json!({
                "uri": uri,
                "expected_format": "dojo://contract/{profile}/{name}/abi"
            })),
        ));
    }

    let profile = parts[3];
    let contract_name = parts[4].strip_suffix("/abi").unwrap_or(parts[4]);

    if profile.is_empty() || contract_name.is_empty() {
        return Err(McpError::resource_not_found(
            "invalid_contract_uri",
            Some(json!({
                "uri": uri,
                "reason": "Profile or contract name is empty"
            })),
        ));
    }

    Ok((profile, contract_name))
}

/// Parse model URI to extract profile and model name
/// Expected format: dojo://model/{profile}/{name}/abi
pub fn parse_model_uri(uri: &str) -> Result<(&str, &str), McpError> {
    let parts: Vec<&str> = uri.split('/').collect();

    if parts.len() != 5
        || parts[0] != "dojo:"
        || parts[1] != ""
        || parts[2] != "model"
        || parts[4] != "abi"
    {
        return Err(McpError::resource_not_found(
            "invalid_model_uri",
            Some(json!({
                "uri": uri,
                "expected_format": "dojo://model/{profile}/{name}/abi"
            })),
        ));
    }

    let profile = parts[3];
    let model_name = parts[4].strip_suffix("/abi").unwrap_or(parts[4]);

    if profile.is_empty() || model_name.is_empty() {
        return Err(McpError::resource_not_found(
            "invalid_model_uri",
            Some(json!({
                "uri": uri,
                "reason": "Profile or model name is empty"
            })),
        ));
    }

    Ok((profile, model_name))
}

/// Parse event URI to extract profile and event name
/// Expected format: dojo://event/{profile}/{name}/abi
pub fn parse_event_uri(uri: &str) -> Result<(&str, &str), McpError> {
    let parts: Vec<&str> = uri.split('/').collect();

    if parts.len() != 5
        || parts[0] != "dojo:"
        || parts[1] != ""
        || parts[2] != "event"
        || parts[4] != "abi"
    {
        return Err(McpError::resource_not_found(
            "invalid_event_uri",
            Some(json!({
                "uri": uri,
                "expected_format": "dojo://event/{profile}/{name}/abi"
            })),
        ));
    }

    let profile = parts[3];
    let event_name = parts[4].strip_suffix("/abi").unwrap_or(parts[4]);

    if profile.is_empty() || event_name.is_empty() {
        return Err(McpError::resource_not_found(
            "invalid_event_uri",
            Some(json!({
                "uri": uri,
                "reason": "Profile or event name is empty"
            })),
        ));
    }

    Ok((profile, event_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_contract_uri() {
        let (profile, name) = parse_contract_uri("dojo://contract/dev/my_contract/abi").unwrap();
        assert_eq!(profile, "dev");
        assert_eq!(name, "my_contract");
    }

    #[test]
    fn test_parse_model_uri() {
        let (profile, name) = parse_model_uri("dojo://model/release/my_model/abi").unwrap();
        assert_eq!(profile, "release");
        assert_eq!(name, "my_model");
    }

    #[test]
    fn test_parse_event_uri() {
        let (profile, name) = parse_event_uri("dojo://event/dev/my_event/abi").unwrap();
        assert_eq!(profile, "dev");
        assert_eq!(name, "my_event");
    }

    #[test]
    fn test_parse_invalid_contract_uri() {
        let result = parse_contract_uri("dojo://contract/dev/");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_model_uri() {
        let result = parse_model_uri("dojo://model/");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_event_uri() {
        let result = parse_event_uri("dojo://event/dev/my_event");
        assert!(result.is_err());
    }
}
