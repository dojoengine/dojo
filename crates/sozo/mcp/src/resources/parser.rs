//! URI parsing utilities for Dojo resources.
#![allow(clippy::comparison_to_empty)]

use rmcp::Error as McpError;
use serde_json::json;

/// Generic function to parse resource URI to extract profile and resource name
/// Expected format: dojo://{resource_type}/{profile}/{name}/abi
fn parse_resource_uri<'a>(
    uri: &'a str,
    resource_type: &str,
) -> Result<(&'a str, &'a str), McpError> {
    let parts: Vec<&str> = uri.split('/').collect();
    let error_code = match resource_type {
        "contract" => "invalid_contract_uri",
        "model" => "invalid_model_uri",
        "event" => "invalid_event_uri",
        _ => "invalid_resource_uri",
    };
    if parts.len() < 5
        || parts[0] != "dojo:"
        || parts[1] != ""
        || parts[2] != resource_type
        || parts[parts.len() - 1] != "abi"
    {
        let expected_format = format!("dojo://{}/{{profile}}/{{name}}/abi", resource_type);
        return Err(McpError::resource_not_found(
            error_code,
            Some(json!({
                "uri": uri,
                "expected_format": expected_format
            })),
        ));
    }

    let profile = parts[3];
    let resource_name = parts[4];

    if profile.is_empty() || resource_name.is_empty() {
        let reason = format!("Profile or {} name is empty", resource_type);
        return Err(McpError::resource_not_found(
            error_code,
            Some(json!({
                "uri": uri,
                "reason": reason
            })),
        ));
    }

    Ok((profile, resource_name))
}

/// Parse contract URI to extract profile and contract name
/// Expected format: dojo://contract/{profile}/{name}/abi
pub fn parse_contract_uri(uri: &str) -> Result<(&str, &str), McpError> {
    parse_resource_uri(uri, "contract")
}

/// Parse model URI to extract profile and model name
/// Expected format: dojo://model/{profile}/{name}/abi
pub fn parse_model_uri(uri: &str) -> Result<(&str, &str), McpError> {
    parse_resource_uri(uri, "model")
}

/// Parse event URI to extract profile and event name
/// Expected format: dojo://event/{profile}/{name}/abi
pub fn parse_event_uri(uri: &str) -> Result<(&str, &str), McpError> {
    parse_resource_uri(uri, "event")
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
