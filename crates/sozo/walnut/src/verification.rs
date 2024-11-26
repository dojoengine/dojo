use std::ffi::OsStr;
use std::io;
use std::path::Path;

use console::{pad_str, Alignment, Style, StyledObject};
use reqwest::StatusCode;
use scarb::core::Workspace;
use serde::Serialize;
use serde_json::Value;
use walkdir::WalkDir;

use crate::utils::walnut_get_api_url;
use crate::Error;

#[derive(Debug, Serialize)]
struct VerificationPayload {
    /// JSON that contains a map where the key is the path to the file and the value is the content
    /// of the file. It should contain all files required to build the Dojo project with Sozo.
    pub source_code: Value,

    pub cairo_version: String,
}

/// Verifies all classes in the workspace.
///
/// This function verifies all contracts and models in the workspace. It sends a single request to
/// the Walnut backend with the source code. Walnut will then build the project and store
/// the source code associated with the class hashes.
pub async fn walnut_verify(ws: &Workspace<'_>) -> anyhow::Result<()> {
    let ui = ws.config().ui();

    // Notify start of verification
    ui.print(" ");
    ui.print("🌰 Verifying classes with Walnut...");
    ui.print(" ");

    // Retrieve the API key and URL from environment variables
    let api_url = walnut_get_api_url();

    // its path to a file so `parent` should never return `None`
    let root_dir: &Path = ws.manifest_path().parent().unwrap().as_std_path();

    let source_code = _collect_source_code(root_dir)?;
    let cairo_version = scarb::version::get().version;

    let verification_payload =
        VerificationPayload { source_code, cairo_version: cairo_version.to_string() };

    // Send verification request
    match verify_classes(verification_payload, &api_url).await {
        Ok(message) => ui.print(_subtitle(message)),
        Err(e) => ui.print(_subtitle(e.to_string())),
    }

    Ok(())
}

async fn verify_classes(payload: VerificationPayload, api_url: &str) -> Result<String, Error> {
    let res =
        reqwest::Client::new().post(format!("{api_url}/v1/verify")).json(&payload).send().await?;

    if res.status() == StatusCode::OK {
        Ok(res.text().await?)
    } else {
        Err(Error::VerificationError(res.text().await?))
    }
}

fn _collect_source_code(root_dir: &Path) -> Result<Value, Error> {
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

fn _subtitle<D: AsRef<str>>(message: D) -> String {
    _dimmed_message(format!("{} {}", pad_str(">", 3, Alignment::Right, None), message.as_ref()))
        .to_string()
}

fn _dimmed_message<D>(message: D) -> StyledObject<D> {
    Style::new().dim().apply_to(message)
}
