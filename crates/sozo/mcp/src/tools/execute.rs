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
    #[schemars(description = "The address of the contract to execute.")]
    pub contract_address: String,
    #[schemars(description = "The name of the function to execute.")]
    pub function_name: String,
    #[schemars(description = "The calldata to pass to the function. Currently the calldata is \
                              expected to be a list of felts already serialized.")]
    pub calldata: Vec<String>,
}

pub async fn execute_transaction(
    _manifest_path: Option<Utf8PathBuf>,
    args: ExecuteRequest,
) -> Result<CallToolResult, McpError> {
    let profile = &args.profile.unwrap_or("dev".to_string());
    let contract_address = &args.contract_address;
    let function_name = &args.function_name;
    let calldata = args.calldata.iter().map(|x| x.as_str()).join(" ");

    let mut cmd = AsyncCommand::new("sozo");
    cmd.arg("execute")
        .arg("--profile")
        .arg(profile)
        .arg(contract_address)
        .arg(function_name)
        .arg(calldata);

    let output = cmd.output().await.map_err(|e| {
        McpError::internal_error(
            "execute_failed",
            Some(json!({ "reason": format!("Failed to execute sozo transaction: {}", e) })),
        )
    })?;

    if output.status.success() {
        Ok(CallToolResult::success(vec![Content::text("Execute successful".to_string())]))
    } else {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        Ok(CallToolResult::error(vec![Content::text(err)]))
    }
}
