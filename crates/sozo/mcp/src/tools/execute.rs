//! Executes a transaction on the project in which the MCP server has been started at.
//!
//! Currently, this uses the calls to the `sozo` command line tool, since the `Config` from Scarb
//! is too restrictive.
//! This will change in the next version with proc macros where only `ScarbMetadata` is used.

use anyhow::Result;
use camino::Utf8PathBuf;
use itertools::Itertools;
use rmcp::model::{CallToolResult, Content};
use serde_json::json;
use tokio::process::Command as AsyncCommand;

use crate::McpError;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ExecuteRequest {
    #[schemars(description = "Profile to use for execute. Default to `dev`.")]
    pub profile: Option<String>,

    #[schemars(description = "The contract identifier. Can be either a contract address (hex) \
                              or a contract tag (namespace-name).")]
    pub contract: String,

    #[schemars(description = "The name of the function to execute.")]
    pub function_name: String,

    #[schemars(
        description = "The calldata to pass to the function. It supports the Sozo calldata format."
    )]
    pub calldata: Vec<String>,

    #[schemars(
        description = "Optional manifest path. If not provided, uses the server's manifest path."
    )]
    pub manifest_path: Option<String>,
}

pub async fn execute_transaction(
    manifest_path: Option<Utf8PathBuf>,
    args: ExecuteRequest,
) -> Result<CallToolResult, McpError> {
    let profile = &args.profile.unwrap_or("dev".to_string());
    let contract = &args.contract;
    let function_name = &args.function_name;
    let calldata = args.calldata.iter().map(|x| x.as_str()).join(" ");

    let mut cmd = AsyncCommand::new("sozo");
    cmd.arg("execute").arg("--profile").arg(profile);

    // Add manifest path if provided in the request, otherwise use server's manifest path
    if let Some(req_manifest_path) = args.manifest_path {
        cmd.arg("--manifest-path").arg(req_manifest_path);
    } else if let Some(server_manifest_path) = &manifest_path {
        cmd.arg("--manifest-path").arg(server_manifest_path);
    }

    cmd.arg(contract).arg(function_name).arg(calldata);

    let output = cmd.output().await.map_err(|e| {
        McpError::internal_error(
            "execute_failed",
            Some(json!({
                "reason": format!("Failed to execute sozo transaction: {}", e),
                "contract": contract,
                "function": function_name,
                "profile": profile
            })),
        )
    })?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Execute successful\nOutput: {}",
            stdout
        ))]))
    } else {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        Ok(CallToolResult::error(vec![Content::text(format!("Execute failed\nError: {}", err))]))
    }
}
