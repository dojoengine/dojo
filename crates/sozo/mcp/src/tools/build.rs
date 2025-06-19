//! Builds the project in which the MCP server has been started at.
//!
//! Currently, this uses the calls to the `sozo` command line tool, since the `Config` from Scarb
//! is too restrictive.
//! This will change in the next version with proc macros where only `ScarbMetadata` is used.

use anyhow::Result;
use serde_json::Value;
use tokio::process::Command as AsyncCommand;

use crate::AppState;

/// Builds the project in which the MCP server has been started at.
///
/// At the moment, the profile is configurable, but not the manifest path,
/// which has been passed to the MCP server initialization.
pub async fn build_project(args: &Value, state: AppState) -> Result<String, String> {
    let profile = args["profile"].as_str().unwrap_or("dev");

    let mut cmd = AsyncCommand::new("sozo");
    cmd.arg("build").arg("--profile").arg(profile);

    // Add manifest path if provided
    if let Some(manifest_path) = state.manifest_path {
        cmd.arg("--manifest-path").arg(manifest_path);
    }

    let output = cmd.output().await.map_err(|e| format!("Failed to build project: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
