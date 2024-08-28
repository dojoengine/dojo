use anyhow::{anyhow, Context, Result};
use console::{pad_str, Alignment, Style, StyledObject};
use dojo_world::{metadata::get_default_namespace_from_ws, migration::strategy::MigrationStrategy};
use futures::future::join_all;
use reqwest::StatusCode;
use scarb::core::Workspace;
use serde::Serialize;
use serde_json::Value;
use std::{ffi::OsStr, path::PathBuf};
use walkdir::WalkDir;

pub async fn walnut_verify_migration_strategy(
    ws: &Workspace<'_>,
    rpc_url: String,
    migration_strategy: &MigrationStrategy,
) -> Result<()> {
    let ui = ws.config().ui();
    // Check if rpc_url is localhost
    if rpc_url.contains("localhost") || rpc_url.contains("127.0.0.1") {
        ui.print(" ");
        ui.print("Verifying classes with Walnut is only supported on hosted networks.");
        ui.print(" ");
        return Ok(());
    }

    // its path to a file so `parent` should never return `None`
    let root_dir: PathBuf = ws.manifest_path().parent().unwrap().to_path_buf().into();
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
    let api_key = std::env::var("WALNUT_API_KEY").context("WALNUT_API_KEY not set")?;
    let api_url =
        std::env::var("WALNUT_API_URL").unwrap_or_else(|_| "https://api.walnut.dev".to_string());

    // Collect source code
    let source_code = collect_source_code(&root_dir)?;

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

fn get_class_name_from_artifact_path(path: &PathBuf, namespace: &str) -> Result<String> {
    let file_name =
        path.file_stem().and_then(OsStr::to_str).ok_or_else(|| anyhow!("Invalid file name"))?;
    let class_name = file_name
        .get(namespace.len() + 1..)
        .ok_or_else(|| anyhow!("Namespace prefix not found in file name"))?;
    Ok(class_name.to_string())
}

#[derive(Debug, Serialize)]
struct VerificationPayload {
    pub class_name: String,
    pub class_hash: String,
    pub rpc_url: String,
    pub source_code: Value,
}

async fn verify_class(
    payload: VerificationPayload,
    api_url: &str,
    api_key: &str,
) -> Result<String> {
    // Serialize the payload to a JSON string for the POST request
    let json_payload = serde_json::to_string(&payload)?;

    let url = format!("{api_url}/v1/verify");

    // Send the POST request to the API
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

fn collect_source_code(root_dir: &PathBuf) -> Result<Value> {
    let mut file_data = serde_json::Map::new();

    // Read toml files in the root folder
    for entry in WalkDir::new(root_dir.clone()).max_depth(1).follow_links(true) {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(extension) = path.extension() {
                if extension == OsStr::new("toml") {
                    let relative_path = path.strip_prefix(root_dir.clone())?;
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
                    let relative_path = path.strip_prefix(root_dir.clone())?;
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