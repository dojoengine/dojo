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

    debug!(target: LOG_TARGET, profile, manifest_path = ?manifest_path, "Building project.");

    let mut cmd = AsyncCommand::new(SOZO_PATH);

    if let Some(manifest_path) = &manifest_path {
        cmd.arg("--manifest-path").arg(manifest_path);
    }

    cmd.arg("--profile").arg(profile);
    cmd.arg("build");

    let output = cmd.output().await.map_err(|e| {
        McpError::internal_error(
            "build_failed",
            Some(json!({ "reason": format!("Failed to build project: {}", e) })),
        )
    })?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let json_obj = serde_json::json!({
            "status": "success",
            "message": "Build successful",
            "stdout": stdout,
            "stderr": stderr
        });
        Ok(CallToolResult::success(vec![Content::json(json_obj)?]))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let json_obj = serde_json::json!({
            "status": "error",
            "message": format!("Build failed with status: {}", output.status),
            "stdout": stdout,
            "stderr": stderr
        });
        Ok(CallToolResult::error(vec![Content::json(json_obj)?]))
    }
}
