//! Migrates the project in which the MCP server has been started at.
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
pub struct MigrateRequest {
    #[schemars(description = "Profile to use for migrate. Default to `dev`.")]
    pub profile: Option<String>,
}

/// Migrates the project in which the MCP server has been started at.
///
/// At the moment, the profile is configurable, but not the manifest path,
/// which has been passed to the MCP server initialization.
pub async fn migrate_project(
    manifest_path: Option<Utf8PathBuf>,
    args: MigrateRequest,
) -> Result<CallToolResult, McpError> {
    let profile = &args.profile.unwrap_or("dev".to_string());

    let mut cmd = AsyncCommand::new(SOZO_PATH);
    cmd.arg("migrate");
    cmd.arg("--profile").arg(profile);

    if let Some(manifest_path) = &manifest_path {
        cmd.arg("--manifest-path").arg(manifest_path);
    }

    debug!(target: LOG_TARGET, profile, manifest_path = ?manifest_path, "Migrating the project.");

    let output = cmd.output().await.map_err(|e| {
        McpError::internal_error(
            "migrate_failed",
            Some(json!({ "reason": format!("Failed to migrate project: {}", e) })),
        )
    })?;

    if output.status.success() {
        Ok(CallToolResult::success(vec![Content::text("Migrate successful".to_string())]))
    } else {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        Ok(CallToolResult::error(vec![Content::text(err)]))
    }
}
