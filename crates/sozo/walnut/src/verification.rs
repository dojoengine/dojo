use std::ffi::OsStr;
use std::io;
use std::path::Path;

use console::{pad_str, Alignment, Style, StyledObject};
use dojo_world::diff::{ResourceDiff, WorldDiff};
use dojo_world::local::ResourceLocal;
use dojo_world::remote::ResourceRemote;
use dojo_world::ResourceType;
use reqwest::StatusCode;
use scarb::core::Workspace;
use serde::Serialize;
use serde_json::Value;
use sozo_scarbext::WorkspaceExt;
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
    world_diff: &WorldDiff,
) -> anyhow::Result<()> {
    let ui = ws.config().ui();
    // Check if rpc_url is localhost
    if rpc_url.contains("localhost") || rpc_url.contains("127.0.0.1") {
        ui.print(" ");
        ui.warn("Verifying classes with Walnut is only supported on hosted networks.");
        ui.print(" ");
        return Ok(());
    }

    // Check if there are any contracts or models in the strategy
    if world_diff.is_synced() {
        ui.print(" ");
        ui.print("🌰 No contracts or models to verify.");
        ui.print(" ");
        return Ok(());
    }

    let _profile_config = ws.load_profile_config()?;

    for (_selector, resource) in world_diff.resources.iter() {
        if resource.resource_type() == ResourceType::Contract {
            match resource {
                ResourceDiff::Created(ResourceLocal::Contract(_contract)) => {
                    // Need to verify created.
                }
                ResourceDiff::Updated(_, ResourceRemote::Contract(_contract)) => {
                    // Need to verify updated.
                }
                _ => {
                    // Synced, we don't need to verify.
                }
            }
        }
    }

    // Notify start of verification
    ui.print(" ");
    ui.print("🌰 Verifying classes with Walnut...");
    ui.print(" ");

    // Retrieve the API key and URL from environment variables
    let _api_key = walnut_get_api_key()?;
    let _api_url = walnut_get_api_url();

    // Collect source code
    // TODO: now it's the same output as scarb, need to update the dojo fork to output the source
    // code, or does scarb supports it already?

    Ok(())
}

fn get_class_name_from_artifact_path(path: &Path, namespace: &str) -> Result<String, Error> {
    let file_name = path.file_stem().and_then(OsStr::to_str).ok_or(Error::InvalidFileName)?;
    let class_name = file_name.strip_prefix(namespace).ok_or(Error::NamespacePrefixNotFound)?;
    Ok(class_name.to_string())
}

#[derive(Debug, Serialize)]
struct VerificationPayload {
    /// The names of the classes we want to verify together with the selector.
    pub class_names: Vec<String>,
    /// The hashes of the Sierra classes.
    pub class_hashes: Vec<String>,
    /// The RPC URL of the network where these classes are declared (can only be a hosted network).
    pub rpc_url: String,
    /// JSON that contains a map where the key is the path to the file and the value is the content
    /// of the file. It should contain all files required to build the Dojo project with Sozo.
    pub source_code: Value,
}

async fn verify_classes(
    payload: VerificationPayload,
    api_url: &str,
    api_key: &str,
) -> Result<String, Error> {
    let res = reqwest::Client::new()
        .post(format!("{api_url}/v1/verify"))
        .header("x-api-key", api_key)
        .json(&payload)
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
