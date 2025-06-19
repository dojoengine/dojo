//! Migrates the project in which the MCP server has been started at.
//!
//! Currently, this uses the calls to the `sozo` command line tool, since the `Config` from Scarb
//! is too restrictive.
//! This will change in the next version with proc macros where only `ScarbMetadata` is used.

use anyhow::Result;
use serde_json::Value;
use tokio::process::Command as AsyncCommand;

pub async fn migrate(args: &Value) -> Result<String, String> {
    let profile = args["profile"].as_str().ok_or("Missing profile")?;

    let mut cmd = AsyncCommand::new("sozo");
    cmd.arg("migrate").arg("--profile").arg(profile);

    let output =
        cmd.output().await.map_err(|e| format!("Failed to execute sozo migrate: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
