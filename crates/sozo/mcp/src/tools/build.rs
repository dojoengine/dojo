//! Builds the project in which the MCP server has been started at.
//!
//! Currently, this uses the calls to the `sozo` command line tool, since the `Config` from Scarb
//! is too restrictive.
//! This will change in the next version with proc macros where only `ScarbMetadata` is used.

use anyhow::Result;
use camino::Utf8PathBuf;
use rmcp::model::{CallToolResult, Content};
use serde_json::json;
use tokio::process::Command as AsyncCommand;
use tracing::debug;

use crate::{McpError, LOG_TARGET, SOZO_PATH};

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

    let mut cmd = AsyncCommand::new(SOZO_PATH);
    cmd.arg("build");
    cmd.arg("--profile").arg(profile);

    if let Some(manifest_path) = &manifest_path {
        cmd.arg("--manifest-path").arg(manifest_path);
    }

    debug!(target: LOG_TARGET, profile, manifest_path = ?manifest_path, "Building project.");

    let output = cmd.output().await.map_err(|e| {
        McpError::internal_error(
            "build_failed",
            Some(json!({ "reason": format!("Failed to build project: {}", e) })),
        )
    })?;

    if output.status.success() {
        Ok(CallToolResult::success(vec![Content::text("Build successful".to_string())]))
    } else {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        Ok(CallToolResult::error(vec![Content::text(err)]))
    }
}
