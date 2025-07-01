//! Resources for the MCP server.

use anyhow::Result;
use camino::Utf8PathBuf;
use dojo_world::local::WorldLocal;
use rmcp::model::{ReadResourceResult, ResourceContents};
use rmcp::Error as McpError;
use scarb_interop::Profile;
use serde_json::json;
use smol_str::SmolStr;
use scarb_metadata_ext::MetadataDojoExt;

pub mod abi;
pub mod parser;

pub use abi::*;
pub use parser::*;

/// Converts a toml file into a json file.
pub async fn toml_to_json(toml_path: Utf8PathBuf) -> Result<String, McpError> {
    let content = tokio::fs::read_to_string(toml_path).await.map_err(|e| {
        McpError::internal_error(
            "manifest_read_failed",
            Some(json!({ "reason": format!("Failed to read manifest file: {}", e) })),
        )
    })?;

    let toml_value: toml::Value = toml::from_str(&content).map_err(|e| {
        McpError::internal_error(
            "manifest_parse_failed",
            Some(json!({ "reason": format!("Failed to parse TOML: {}", e) })),
        )
    })?;

    let json_value = serde_json::to_string_pretty(&toml_value).map_err(|e| {
        McpError::internal_error(
            "manifest_serialization_failed",
            Some(json!({ "reason": format!("Failed to serialize manifest: {}", e) })),
        )
    })?;

    Ok(json_value)
}

/// Loads the world local from the manifest path and profile.
pub async fn load_world_local(
    manifest_path: Option<Utf8PathBuf>,
    profile: &str,
) -> Result<WorldLocal, McpError> {
    let manifest_path = manifest_path.as_ref().ok_or_else(|| {
        McpError::internal_error(
            "no_manifest_path",
            Some(json!({ "reason": "No manifest path provided" })),
        )
    })?;

    let profile_enum = match profile {
        "dev" => Profile::DEV,
        "release" => Profile::RELEASE,
        _ => Profile::new(SmolStr::from(profile)).map_err(|e| {
            McpError::internal_error(
                "invalid_profile",
                Some(json!({ "reason": format!("Invalid profile: {}", e) })),
            )
        })?,
    };

    let config =
        Config::builder(manifest_path.clone()).profile(profile_enum).build().map_err(|e| {
            McpError::internal_error(
                "config_build_failed",
                Some(json!({ "reason": format!("Failed to build config: {}", e) })),
            )
        })?;

    let ws = ops::read_workspace(config.manifest_path(), &config).map_err(|e| {
        McpError::internal_error(
            "workspace_read_failed",
            Some(json!({ "reason": format!("Failed to read workspace: {}", e) })),
        )
    })?;

    let world = ws.load_world_local().map_err(|e| {
        McpError::internal_error(
            "world_load_failed",
            Some(json!({ "reason": format!("Failed to load world: {}", e) })),
        )
    })?;

    Ok(world)
}

/// Main resource handler that routes to appropriate resource types.
pub async fn handle_resource(
    uri: &str,
    manifest_path: Option<Utf8PathBuf>,
) -> Result<ReadResourceResult, McpError> {
    match uri {
        "dojo://scarb/manifest" => handle_manifest_resource(manifest_path).await,
        uri if uri.starts_with("dojo://contract/") && uri.ends_with("/abi") => {
            handle_contract_abi_resource(uri, manifest_path).await
        }
        uri if uri.starts_with("dojo://model/") && uri.ends_with("/abi") => {
            handle_model_abi_resource(uri, manifest_path).await
        }
        uri if uri.starts_with("dojo://event/") && uri.ends_with("/abi") => {
            handle_event_abi_resource(uri, manifest_path).await
        }
        _ => Err(McpError::resource_not_found("resource_not_found", Some(json!({ "uri": uri })))),
    }
}

/// Handles the manifest resource.
async fn handle_manifest_resource(
    manifest_path: Option<Utf8PathBuf>,
) -> Result<ReadResourceResult, McpError> {
    let manifest_path = manifest_path.ok_or_else(|| {
        McpError::resource_not_found(
            "no_manifest_path",
            Some(json!({ "reason": "No manifest path provided" })),
        )
    })?;

    let manifest_json = toml_to_json(manifest_path).await?;

    Ok(ReadResourceResult {
        contents: vec![ResourceContents::text(manifest_json, "dojo://scarb/manifest")],
    })
}

/// Handles the contract ABI resource.
async fn handle_contract_abi_resource(
    uri: &str,
    manifest_path: Option<Utf8PathBuf>,
) -> Result<ReadResourceResult, McpError> {
    let (profile, contract_name) = parse_contract_uri(uri)?;
    let world = load_world_local(manifest_path, profile).await?;
    let abi_json = get_contract_abi(&world, contract_name)?;

    Ok(ReadResourceResult { contents: vec![ResourceContents::text(abi_json, uri)] })
}

/// Handles the model ABI resource.
async fn handle_model_abi_resource(
    uri: &str,
    manifest_path: Option<Utf8PathBuf>,
) -> Result<ReadResourceResult, McpError> {
    let (profile, model_name) = parse_model_uri(uri)?;
    let world = load_world_local(manifest_path, profile).await?;
    let abi_json = get_model_abi(&world, model_name)?;

    Ok(ReadResourceResult { contents: vec![ResourceContents::text(abi_json, uri)] })
}

/// Handles the event ABI resource.
async fn handle_event_abi_resource(
    uri: &str,
    manifest_path: Option<Utf8PathBuf>,
) -> Result<ReadResourceResult, McpError> {
    let (profile, event_name) = parse_event_uri(uri)?;
    let world = load_world_local(manifest_path, profile).await?;
    let abi_json = get_event_abi(&world, event_name)?;

    Ok(ReadResourceResult { contents: vec![ResourceContents::text(abi_json, uri)] })
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use super::*;

    #[tokio::test]
    async fn test_toml_to_json_success() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let toml_content = r#"
[package]
name = "test_project"
version = "0.1.0"
edition = "2023_01"

[dependencies]
dojo = { git = "https://github.com/dojoengine/dojo" }
starknet = "2.4.0"

[dev-dependencies]
cairo-lang-test-plugin = "2.4.0"

[[target.dojo]]
"#;
        temp_file.write_all(toml_content.as_bytes()).unwrap();

        let toml_path = Utf8PathBuf::from_path_buf(temp_file.path().to_path_buf()).unwrap();
        let result = toml_to_json(toml_path).await;

        assert!(result.is_ok());
        let json_str = result.unwrap();

        assert!(json_str.contains("test_project"));
        assert!(json_str.contains("0.1.0"));
        assert!(json_str.contains("2023_01"));
        assert!(json_str.contains("dojo"));
        assert!(json_str.contains("starknet"));
        assert!(json_str.contains("2.4.0"));
    }

    #[tokio::test]
    async fn test_toml_to_json_invalid_toml() {
        let mut temp_file = NamedTempFile::new().unwrap();

        // Missing closing bracket.
        let invalid_toml = r#"
[package
name = "test_project"
version = "0.1.0"
"#;
        temp_file.write_all(invalid_toml.as_bytes()).unwrap();

        let toml_path = Utf8PathBuf::from_path_buf(temp_file.path().to_path_buf()).unwrap();
        let result = toml_to_json(toml_path).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Failed to parse TOML"));
    }

    #[tokio::test]
    async fn test_toml_to_json_nonexistent_file() {
        let nonexistent_path = Utf8PathBuf::from("/non/existent/path/Scarb.toml");
        let result = toml_to_json(nonexistent_path).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_resource_manifest() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let toml_content = r#"
[package]
name = "test_project"
version = "0.1.0"
"#;
        temp_file.write_all(toml_content.as_bytes()).unwrap();

        let manifest_path =
            Some(Utf8PathBuf::from_path_buf(temp_file.path().to_path_buf()).unwrap());
        let result = handle_resource("dojo://scarb/manifest", manifest_path).await;

        assert!(result.is_ok());
        let resource_result = result.unwrap();
        assert_eq!(resource_result.contents.len(), 1);

        let content = &resource_result.contents[0];
        assert!(std::mem::size_of_val(content) > 0);
    }

    #[tokio::test]
    async fn test_handle_resource_unknown_uri() {
        let result = handle_resource("dojo://unknown/resource", None).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("resource_not_found"));
    }

    #[tokio::test]
    async fn test_handle_manifest_resource_no_path() {
        let result = handle_manifest_resource(None).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("No manifest path provided"));
    }

    #[tokio::test]
    async fn test_handle_manifest_resource_with_path() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let toml_content = r#"
[package]
name = "test_project"
version = "0.1.0"
"#;
        temp_file.write_all(toml_content.as_bytes()).unwrap();

        let manifest_path =
            Some(Utf8PathBuf::from_path_buf(temp_file.path().to_path_buf()).unwrap());
        let result = handle_manifest_resource(manifest_path).await;

        assert!(result.is_ok());
        let resource_result = result.unwrap();
        assert_eq!(resource_result.contents.len(), 1);

        let content = &resource_result.contents[0];
        assert!(std::mem::size_of_val(content) > 0); // Just verify it's not empty
    }
}
