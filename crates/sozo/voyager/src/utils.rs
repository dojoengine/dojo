//! Utility functions for project analysis and version detection.

use std::path::PathBuf;

/// Default Cairo version used when version detection fails.
const VOYAGER_CAIRO_VERSION_DEFAULT: &str = "2.8.0";

/// Get the project root directory by searching for project markers.
pub fn get_project_root() -> PathBuf {
    // Try to find project root by looking for manifest files
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Search upward from current directory for project markers
    let mut search_dir = current_dir.clone();
    loop {
        // Check for Scarb.toml or manifest_dev.json
        if search_dir.join("Scarb.toml").exists() || search_dir.join("manifest_dev.json").exists() {
            return search_dir;
        }

        // Move to parent directory
        if let Some(parent) = search_dir.parent() {
            search_dir = parent.to_path_buf();
        } else {
            break;
        }
    }

    // Fallback to current directory
    current_dir
}

/// Get Cairo and Scarb versions from project configuration.
pub fn get_project_versions() -> Result<(String, String), anyhow::Error> {
    use std::process::Command;

    // Get Cairo version from scarb metadata
    let cairo_version = if let Ok(output) =
        Command::new("scarb").args(["metadata", "--format-version", "1"]).output()
    {
        if output.status.success() {
            let metadata_str = String::from_utf8(output.stdout)?;
            if let Ok(metadata) = serde_json::from_str::<serde_json::Value>(&metadata_str) {
                metadata["cairo_version"]
                    .as_str()
                    .unwrap_or(VOYAGER_CAIRO_VERSION_DEFAULT)
                    .to_string()
            } else {
                VOYAGER_CAIRO_VERSION_DEFAULT.to_string()
            }
        } else {
            VOYAGER_CAIRO_VERSION_DEFAULT.to_string()
        }
    } else {
        VOYAGER_CAIRO_VERSION_DEFAULT.to_string()
    };

    // Get Scarb version
    let scarb_version = if let Ok(output) = Command::new("scarb").args(["--version"]).output() {
        if output.status.success() {
            let version_str = String::from_utf8(output.stdout)?;
            // Parse "scarb 2.8.0" format
            version_str
                .split_whitespace()
                .nth(1)
                .unwrap_or(VOYAGER_CAIRO_VERSION_DEFAULT)
                .to_string()
        } else {
            VOYAGER_CAIRO_VERSION_DEFAULT.to_string()
        }
    } else {
        VOYAGER_CAIRO_VERSION_DEFAULT.to_string()
    };

    Ok((cairo_version, scarb_version))
}
