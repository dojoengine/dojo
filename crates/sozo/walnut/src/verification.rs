use std::ffi::OsStr;
use std::io;
use std::path::Path;

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
use crate::Error;

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
) -> anyhow::Result<()> {
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

fn get_class_name_from_artifact_path(path: &Path, namespace: &str) -> Result<String, Error> {
    let file_name = path.file_stem().and_then(OsStr::to_str).ok_or(Error::InvalidFileName)?;
    let class_name = file_name.strip_prefix(namespace).ok_or(Error::NamespacePrefixNotFound)?;
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
) -> Result<String, Error> {
    let res = reqwest::Client::new()
        .post(format!("{api_url}/v1/verify"))
        .header("Content-Type", "application/json")
        .header("x-api-key", api_key)
        .body(serde_json::to_string(&payload)?)
        .send()
        .await?;

    if res.status() == StatusCode::OK {
        Ok(res.text().await?)
    } else {
        Err(Error::VerificationError(res.text().await?))
    }
}

fn collect_source_code(root_dir: &Path) -> Result<Value, Error> {
    fn collect_files(
        root_dir: &Path,
        search_dir: &Path,
        extension: &str,
        max_depth: Option<usize>,
        file_data: &mut serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), Error> {
        // Set max_depth to usize::MAX if None is provided, matching the default value set by
        // WalkDir::new()
        let max_depth = max_depth.unwrap_or(usize::MAX);
        for entry in WalkDir::new(search_dir).max_depth(max_depth).follow_links(true) {
            let entry = entry.map_err(io::Error::from)?;
            let path = entry.path();
            if path.is_file() {
                if let Some(file_extension) = path.extension() {
                    if file_extension == OsStr::new(extension) {
                        // Safe to unwrap here because we're iterating over files within root_dir,
                        // so path will always have root_dir as a prefix
                        let relative_path = path.strip_prefix(root_dir).unwrap();
                        let file_content = std::fs::read_to_string(path)?;
                        file_data.insert(
                            relative_path.to_string_lossy().into_owned(),
                            serde_json::Value::String(file_content),
                        );
                    }
                }
            }
        }
        Ok(())
    }

    let mut file_data = serde_json::Map::new();
    // Read `.toml` files in the root folder
    collect_files(root_dir, root_dir, "toml", Some(1), &mut file_data)?;
    // Read `.cairo` files in the root/src folder
    collect_files(root_dir, &root_dir.join("src"), "cairo", None, &mut file_data)?;

    Ok(serde_json::Value::Object(file_data))
}

fn subtitle<D: AsRef<str>>(message: D) -> String {
    dimmed_message(format!("{} {}", pad_str(">", 3, Alignment::Right, None), message.as_ref()))
        .to_string()
}

fn dimmed_message<D>(message: D) -> StyledObject<D> {
    Style::new().dim().apply_to(message)
}
