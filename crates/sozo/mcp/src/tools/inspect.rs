//! Inspects the project in which the MCP server has been started at.
//!
//! Currently, this uses the calls to the `sozo` command line tool, since the `Config` from Scarb
//! is too restrictive.
//! This will change in the next version with proc macros where only `ScarbMetadata` is used.

use anyhow::Result;
use serde_json::Value;
use tokio::process::Command as AsyncCommand;

pub async fn inspect(args: &Value) -> Result<String, String> {
    let profile = args["profile"].as_str().unwrap_or("dev");

    let output = AsyncCommand::new("sozo")
        .arg("inspect")
        .arg("--profile")
        .arg(profile)
        .output()
        .await
        .map_err(|e| format!("Failed to get contract info: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
