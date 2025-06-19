//! Executes a transaction on the project in which the MCP server has been started at.
//!
//! Currently, this uses the calls to the `sozo` command line tool, since the `Config` from Scarb
//! is too restrictive.
//! This will change in the next version with proc macros where only `ScarbMetadata` is used.

use anyhow::Result;
use serde_json::Value;
use tokio::process::Command as AsyncCommand;
use itertools::Itertools;

pub async fn execute_transaction(args: &Value) -> Result<String, String> {
    let function_name = args["function_name"].as_str().ok_or("Missing function_name")?;
    let contract_address =
        args["contract_address"].as_str().ok_or("Missing contract_address")?;
    let calldata = args["calldata"].as_array().ok_or("Missing calldata")?;
    let calldata = calldata.iter().map(|x| x.as_str().unwrap_or("")).join(" ");
    let profile = args["profile"].as_str().unwrap_or("dev");

    let mut cmd = AsyncCommand::new("sozo");
    cmd.arg("execute")
        .arg("--profile")
        .arg(profile)
        .arg(contract_address)
        .arg(function_name)
        .arg(calldata);

    if let Some(calldata) = args["calldata"].as_array() {
        for param in calldata {
            if let Some(param_str) = param.as_str() {
                cmd.arg(param_str);
            }
        }
    }

    let output =
        cmd.output().await.map_err(|e| format!("Failed to execute sozo transaction: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
