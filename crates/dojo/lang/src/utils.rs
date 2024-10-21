use std::fs;

use anyhow::Result;
use cairo_lang_filesystem::ids::Directory;
use cairo_lang_syntax::node::db::SyntaxGroup;
use dojo_world::config::NamespaceConfig;
use regex::Regex;
use toml::Table;

/// Check if the provided name follows the format rules.
pub fn is_name_valid(name: &str) -> bool {
    Regex::new(r"^[a-zA-Z0-9_]+$").unwrap().is_match(name)
}

/// Get the namespace configuration from the workspace.
// TODO: Ask to Scarb team to expose this information with the new macro system.
pub fn get_namespace_config(db: &dyn SyntaxGroup) -> Result<NamespaceConfig> {
    // Super verbose print, but useful to get the CfgSet.
    // debug!(cfg_set = ?db.cfg_set(), crates = ?db.crates(), "Retrieving namespace
    // configuration.");

    let crates = db.crates();

    if crates.is_empty() {
        return Err(anyhow::anyhow!(
            "No crates found in the workspace, hence no namespace configuration."
        ));
    }

    // Crates[0] is always the root crate that triggered the build origin.
    // In case of a library, crates[0] refers to the lib itself if compiled directly,
    // or the crate using the library otherwise.
    let configuration = match db
        .crate_config(*crates.first().expect("No root crate found in the workspace."))
    {
        Option::Some(cfg) => cfg,
        Option::None => return Err(anyhow::anyhow!("No configuration found for the root crate.")),
    };

    if let Directory::Real(ref path) = configuration.root {
        let config_path = path.parent().unwrap().join("Scarb.toml");

        // Very verbose.
        // tracing::debug!(config_path = %config_path.to_string_lossy(), "Reading Scarb.toml file
        // for namespace config.");

        let config_content = match fs::read_to_string(&config_path) {
            Ok(x) => x,
            Err(e) => return Err(anyhow::anyhow!("Failed to read Scarb.toml file: {e}.")),
        };
        let config = match config_content.parse::<Table>() {
            Ok(x) => x,
            Err(e) => return Err(anyhow::anyhow!("Failed to parse Scarb.toml file: {e}.")),
        };

        if let Some(tool) = config.get("tool").and_then(|t| t.as_table()) {
            if let Some(dojo) = tool.get("dojo").and_then(|d| d.as_table()) {
                if let Some(world) = dojo.get("world").and_then(|w| w.as_table()) {
                    if let Some(namespace_config) =
                        world.get("namespace").and_then(|n| n.as_table())
                    {
                        match toml::from_str::<NamespaceConfig>(&namespace_config.to_string()) {
                            Ok(config) => return config.validate(),
                            Err(e) => {
                                return Err(anyhow::anyhow!(
                                    "Failed to parse namespace configuration of {}: {}",
                                    config_path.to_string_lossy().to_string(),
                                    e
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    // If no namespace is found, we return a default one as it's surely a crate that doesn't
    // need it. This only concern the tests.
    Ok(NamespaceConfig { default: "__NO_NAMESPACE__".into(), mappings: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_name_valid() {
        assert!(is_name_valid("valid_name"));
        assert!(is_name_valid("ValidName"));
        assert!(is_name_valid("valid123"));
        assert!(is_name_valid("_valid"));
        assert!(is_name_valid("valid_"));

        assert!(!is_name_valid(""));
        assert!(!is_name_valid("invalid-name"));
        assert!(!is_name_valid("invalid name"));
        assert!(!is_name_valid("invalid!name"));
        assert!(!is_name_valid("invalid.name"));
    }
}
