//! Inspects the project in which the MCP server has been started at.
//!
//! Currently, this uses the calls to the `sozo` command line tool, since the `Config` from Scarb
//! is too restrictive.
//! This will change in the next version with proc macros where only `ScarbMetadata` is used.

use anyhow::Result;
use camino::Utf8PathBuf;
use rmcp::model::{CallToolResult, Content};
use serde_json::{json, Value};
use tokio::process::Command as AsyncCommand;
use tracing::{debug, error};

use crate::{McpError, LOG_TARGET, SOZO_PATH};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct InspectRequest {
    #[schemars(description = "Profile to use for inspect. Default to `dev`.")]
    pub profile: Option<String>,
}

/// Inspects the project in which the MCP server has been started at.
///
/// At the moment, the profile is configurable, but not the manifest path,
/// which has been passed to the MCP server initialization.
pub async fn inspect_project(
    manifest_path: Option<Utf8PathBuf>,
    args: InspectRequest,
) -> Result<CallToolResult, McpError> {
    let profile = &args.profile.unwrap_or("dev".to_string());

    let mut cmd = AsyncCommand::new(SOZO_PATH);

    if let Some(manifest_path) = &manifest_path {
        cmd.arg("--manifest-path").arg(manifest_path);
    }

    debug!(target: LOG_TARGET, profile, manifest_path = ?manifest_path, "Inspecting project.");

    cmd.arg("--profile").arg(profile);
    cmd.arg("inspect");
    cmd.arg("--json");

    let output = cmd.output().await.map_err(|e| {
        McpError::internal_error(
            "inspect_failed",
            Some(json!({ "reason": format!("Failed to inspect project: {}", e) })),
        )
    });

    let output = output?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        match serde_json::from_str::<Value>(&stdout) {
            Ok(json_value) => Ok(CallToolResult::success(vec![Content::json(json_value)?])),
            Err(e) => {
                error!(target: LOG_TARGET, "Failed to parse JSON: {:?}", e);
                Ok(CallToolResult::error(vec![Content::text(e.to_string())]))
            }
        }
    } else {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        error!(target: LOG_TARGET, "Failed to run inspect command: {:?}", err);
        Ok(CallToolResult::error(vec![Content::text(err)]))
    }
}
