//! Resources for the MCP server.

use anyhow::Result;
use camino::Utf8PathBuf;
use dojo_world::local::{ResourceLocal, WorldLocal};
use rmcp::{
    Error as McpError, RoleServer, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model::*,
    service::RequestContext,
    tool, tool_handler, tool_router, transport,
};
use scarb::compiler::Profile;
use scarb::core::Config;
use scarb::ops;
use serde_json::{Value, json};
use smol_str::SmolStr;
use sozo_scarbext::WorkspaceExt;
use std::future::Future;
use tokio::process::Command as AsyncCommand;
use toml;
use tracing::{debug, error};

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
