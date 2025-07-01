//! Builds the project in which the MCP server has been started at.
//!
//! Currently, this uses the calls to the `sozo` command line tool, since the `Config` from Scarb
//! is too restrictive.
//! This will change in the next version with proc macros where only `ScarbMetadata` is used.

use anyhow::Result;
use camino::Utf8PathBuf;
use rmcp::model::{CallToolResult, Content};
use scarb_interop::Scarb;
use scarb_metadata::{self, Metadata};
use scarb_metadata_ext::MetadataDojoExt;
use serde_json::json;
use tokio::process::Command as AsyncCommand;
use tracing::debug;

use crate::{LOG_TARGET, McpError, SOZO_PATH};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct BuildRequest {
    #[schemars(description = "Profile to use for build. Default to `dev`.")]
    pub profile: Option<String>,
}

/// Builds the project in which the MCP server has been started at.
///
/// At the moment, the profile is configurable, but not the manifest path,
/// which has been passed to the MCP server initialization.
pub async fn build_project(
    manifest_path: Option<Utf8PathBuf>,
    args: BuildRequest,
) -> Result<CallToolResult, McpError> {
    let profile = &args.profile.unwrap_or("dev".to_string());

    let default_manifest = Utf8PathBuf::from("Scarb.toml");
    let manifest_path = manifest_path.as_ref().unwrap_or(&default_manifest);

    let scarb_metadata = Metadata::load(manifest_path, profile, false).map_err(|e| {
        McpError::internal_error(
            "scarb_metadata_load_failed",
            Some(json!({ "reason": format!("Failed to load scarb metadata: {}", e) })),
        )
    })?;

    scarb_metadata.clean_dir_profile();

    Scarb::build(
        &scarb_metadata.workspace.manifest_path,
        scarb_metadata.current_profile.as_str(),
        // Builds all packages.
        "",
        scarb_interop::Features::AllFeatures,
        vec![],
    ).map_err(|e| {
        McpError::internal_error(
            "scarb_build_failed",
            Some(json!({ "reason": format!("Failed to build project: {}", e) })),
        )
    })?;

    Ok(CallToolResult::success(vec![Content::text("Build successful".to_string())]))
}
