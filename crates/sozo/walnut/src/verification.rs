use std::ffi::OsStr;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use console::{pad_str, Alignment, Style, StyledObject};
use dojo_world::metadata::get_default_namespace_from_ws;
use dojo_world::migration::strategy::MigrationStrategy;
use futures::future::join_all;
use reqwest::StatusCode;
use scarb::core::Workspace;
use serde::Serialize;
use serde_json::Value;
use walkdir::WalkDir;

use crate::utils::{walnut_get_api_key, walnut_get_api_url};

/// Verifies all classes declared during migration.
/// Only supported on hosted networks (non-localhost).
///
/// This function verifies all contracts and models in the strategy. For every contract and model,
/// it sends a request to the Walnut backend with the class name, class hash, RPC URL, and source
/// code. Walnut will then build the project with Sozo, compare the Sierra bytecode with the
/// bytecode on the network, and if they are equal, it will store the source code and associate it
/// with the class hash.
pub async fn walnut_verify_migration_strategy(
    ws: &Workspace<'_>,
    rpc_url: String,
    migration_strategy: &MigrationStrategy,
) -> Result<()> {
    let ui = ws.config().ui();
    // Check if rpc_url is localhost
    if rpc_url.contains("localhost") || rpc_url.contains("127.0.0.1") {
        ui.print(" ");
        ui.warn("Verifying classes with Walnut is only supported on hosted networks.");
        ui.print(" ");
        return Ok(());
    }

    // its path to a file so `parent` should never return `None`
    let root_dir: &Path = ws.manifest_path().parent().unwrap().as_std_path();
    let default_namespace = get_default_namespace_from_ws(ws)?;

    // Check if there are any contracts or models in the strategy
    if migration_strategy.contracts.is_empty() && migration_strategy.models.is_empty() {
        ui.print(" ");
        ui.print("🌰 No contracts or models to verify.");
        ui.print(" ");
        return Ok(());
    }

    // Notify start of verification
    ui.print(" ");
    ui.print("🌰 Verifying classes with Walnut...");
    ui.print(" ");

    // Retrieve the API key and URL from environment variables
    let api_key = walnut_get_api_key()?;
    let api_url = walnut_get_api_url();

    // Collect source code
    let source_code = collect_source_code(root_dir)?;

    // Prepare verification payloads
    let mut verification_tasks = Vec::new();
    let mut class_tags = Vec::new();

    for contract_migration in &migration_strategy.contracts {
        let class_name = get_class_name_from_artifact_path(
            &contract_migration.artifact_path,
            &default_namespace,
        )?;
        let verification_payload = VerificationPayload {
            class_name: class_name.clone(),
            class_hash: contract_migration.diff.local_class_hash.to_hex_string(),
            rpc_url: rpc_url.clone(),
            source_code: source_code.clone(),
        };
        class_tags.push(contract_migration.diff.tag.clone());
        verification_tasks.push(verify_class(verification_payload, &api_url, &api_key));
    }

    for class_migration in &migration_strategy.models {
        let class_name =
            get_class_name_from_artifact_path(&class_migration.artifact_path, &default_namespace)?;
        let verification_payload = VerificationPayload {
            class_name: class_name.clone(),
            class_hash: class_migration.diff.local_class_hash.to_hex_string(),
            rpc_url: rpc_url.clone(),
            source_code: source_code.clone(),
        };
        class_tags.push(class_migration.diff.tag.clone());
        verification_tasks.push(verify_class(verification_payload, &api_url, &api_key));
    }

    // Run all verification tasks
    let results = join_all(verification_tasks).await;

    for (i, result) in results.into_iter().enumerate() {
        match result {
            Ok(message) => {
                ui.print(subtitle(format!("{}: {}", class_tags[i], message)));
            }
            Err(e) => {
                ui.print(subtitle(format!("{}: {}", class_tags[i], e)));
            }
        }
    }

    Ok(())
}

fn get_class_name_from_artifact_path(path: &Path, namespace: &str) -> Result<String> {
    let file_name =
        path.file_stem().and_then(OsStr::to_str).ok_or_else(|| anyhow!("Invalid file name"))?;
    let class_name = file_name
        .get(namespace.len() + 1..)
        .ok_or_else(|| anyhow!("Namespace prefix not found in file name"))?;
    Ok(class_name.to_string())
}

#[derive(Debug, Serialize)]
struct VerificationPayload {
    /// The name of the class we want to verify together with the selector.
    pub class_name: String,
    /// The hash of the Sierra class.
    pub class_hash: String,
    /// The RPC URL of the network where this class is declared (can only be a hosted network).
    pub rpc_url: String,
    /// JSON that contains a map where the key is the path to the file and the value is the content
    /// of the file. It should contain all files required to build the Dojo project with Sozo.
    pub source_code: Value,
}

async fn verify_class(
    payload: VerificationPayload,
    api_url: &str,
    api_key: &str,
) -> Result<String> {
    let json_payload = serde_json::to_string(&payload)?;

    let url = format!("{api_url}/v1/verify");

    let client = reqwest::Client::new();
    let api_res = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("x-api-key", api_key)
        .body(json_payload)
        .send()
        .await
        .context("Failed to send request to verifier API")?;

    if api_res.status() == StatusCode::OK {
        let message = api_res.text().await.context("Failed to read verifier API response")?;
        Ok(message)
    } else {
        let message = api_res.text().await.context("Failed to verify contract")?;
        Err(anyhow!(message))
    }
}

fn collect_source_code(root_dir: &Path) -> Result<Value> {
    let mut file_data = serde_json::Map::new();

    // Read toml files in the root folder
    for entry in WalkDir::new(root_dir).max_depth(1).follow_links(true) {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(extension) = path.extension() {
                if extension == OsStr::new("toml") {
                    let relative_path = path.strip_prefix(root_dir)?;
                    let file_content = std::fs::read_to_string(path)?;
                    file_data.insert(
                        relative_path.to_string_lossy().into_owned(),
                        serde_json::Value::String(file_content),
                    );
                }
            }
        }
    }

    // Read cairo files in the root/src folder
    let src_dir = root_dir.join("src");
    for entry in WalkDir::new(src_dir.clone()).follow_links(true) {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(extension) = path.extension() {
                if extension == OsStr::new("cairo") {
                    let relative_path = path.strip_prefix(root_dir)?;
                    let file_content = std::fs::read_to_string(path)?;
                    file_data.insert(
                        relative_path.to_string_lossy().into_owned(),
                        serde_json::Value::String(file_content),
                    );
                }
            }
        }
    }

    Ok(serde_json::Value::Object(file_data))
}

fn subtitle<D: AsRef<str>>(message: D) -> String {
    dimmed_message(format!("{} {}", pad_str(">", 3, Alignment::Right, None), message.as_ref()))
        .to_string()
}

fn dimmed_message<D>(message: D) -> StyledObject<D> {
    Style::new().dim().apply_to(message)
}
